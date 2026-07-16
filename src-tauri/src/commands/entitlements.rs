// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)

//! Phase 15 (15-04) — IPC layer composing the Wave 1 entitlement primitives
//! (`entitlements::redeem`, `entitlements::download`, `entitlements::fingerprint`)
//! into two Tauri commands:
//!
//! - `redeem_license` — validates a license key against the Hub and returns
//!   the confirm-dialog payload. Performs NO download (D-03 staged-confirm
//!   split).
//! - `download_and_import_pack` — fetches the buyer-stamped pack, imports it
//!   through the UNCHANGED `import_course_impl` Step 3.5 gate
//!   (`commands::course_io`), records the entitlement row, and stamps
//!   `learning_paths.pack_id` for attribution (D-08).
//!
//! Follows the project's `_impl` shim shape: `_impl` fns take
//! `&std::sync::Mutex<Database>` directly (test-driveable without a Tauri
//! app), and no `MutexGuard` is ever held across a network `.await`
//! (T-15-15).
//!
//! `import_course_impl` and the rest of the Step 3.5/Step 4 signature-check
//! logic in `course_io.rs` are NOT modified by this file — this is a new
//! CALLER of the existing gate, not a second check path (RESEARCH Pitfalls
//! 1 & 3). `pack_id` path-traversal sanitization is NOT re-implemented here
//! — it is centralized inside `entitlements::download::download_and_store`
//! (15-03) at the point of the literal path join (T-15-14); this file only
//! propagates that layer's rejection.

use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::commands::course_io::import_course_impl;
use crate::db::Database;
use crate::entitlements::download::download_and_store;
use crate::entitlements::fingerprint::sha256_fingerprint;
use crate::entitlements::redeem::{call_redeem_endpoint, RedeemLicenseRequest, RedeemLicenseResult};
use crate::storage_impl::entitlements::{EntitlementRow, SqliteEntitlementStore};
use course_io::ImportCourseResult;

use crate::commands::course_io;

// ── Request/response types ──────────────────────────────────────────────

/// WR-06 — machine-readable error payload for the redeem-flow IPC boundary.
///
/// Previously both commands mapped every failure to `e.to_string()`, so the
/// frontend's `classifyError` had to substring-match the human Display copy
/// — the exact string-matching anti-pattern T-15-09 removed from the
/// backend. This struct serializes as `{ "kind": ..., "message": ... }`:
/// the frontend classifies EXCLUSIVELY on `kind` (stable machine code) and
/// renders its own locked UI-SPEC copy — `message` is diagnostic context,
/// never rendered into the DOM (T-15-16).
#[derive(Debug, Clone, Serialize)]
pub struct RedeemIpcError {
    /// Stable machine code: `invalid_key` | `already_redeemed` | `revoked`
    /// | `issuer_unreachable` | `malformed_response` | `pack_too_large`
    /// | `generic`.
    pub kind: String,
    /// Human-readable detail (the typed error's Display copy, or the raw
    /// underlying error text for `generic`). Diagnostic only — the UI never
    /// renders this (T-15-16).
    pub message: String,
}

impl RedeemIpcError {
    /// Local (non-Hub) failures: lock poisoning, storage errors, import
    /// rejections. The frontend renders its generic fallback copy.
    fn generic(message: impl std::fmt::Display) -> Self {
        Self {
            kind: "generic".to_string(),
            message: message.to_string(),
        }
    }
}

impl From<crate::entitlements::RedeemLicenseError> for RedeemIpcError {
    fn from(err: crate::entitlements::RedeemLicenseError) -> Self {
        use crate::entitlements::RedeemLicenseError as E;
        let kind = match &err {
            E::InvalidKey => "invalid_key",
            E::AlreadyRedeemed => "already_redeemed",
            E::Revoked => "revoked",
            E::IssuerUnreachable => "issuer_unreachable",
            E::MalformedResponse(_) => "malformed_response",
            E::PackTooLarge => "pack_too_large",
        };
        Self {
            kind: kind.to_string(),
            message: err.to_string(),
        }
    }
}

/// `redeem_license` IPC request. Wire shape matches
/// `entitlements::redeem::RedeemLicenseRequest` (camelCase over IPC).
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemLicenseIpcRequest {
    pub license_key: String,
    pub device_fingerprint: String,
}

/// WR-04 (D-06) — manual Debug impl so `{:?}` can never leak the raw
/// license key into a future log/error/panic message.
impl std::fmt::Debug for RedeemLicenseIpcRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedeemLicenseIpcRequest")
            .field("license_key", &"<redacted>")
            .field("device_fingerprint", &self.device_fingerprint)
            .finish()
    }
}

/// `download_and_import_pack` IPC request. Carries forward everything the
/// confirm-stage (`redeem_license`'s result) already fetched from the Hub,
/// so this second call can both import AND record the entitlement without a
/// second Hub round-trip.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadAndImportPackRequest {
    pub download_url: String,
    pub pack_id: String,
    pub issuer_id: String,
    pub issuer_name: String,
    pub buyer_name: String,
    pub order_id: String,
    pub redeemed_at: String,
    /// Used ONLY to compute the fingerprint, then dropped — never inserted,
    /// logged, or embedded in an error string (D-06).
    pub license_key: String,
}

/// WR-04 (D-06) — manual Debug impl so `{:?}` can never leak the raw
/// license key into a future log/error/panic message.
impl std::fmt::Debug for DownloadAndImportPackRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadAndImportPackRequest")
            .field("download_url", &self.download_url)
            .field("pack_id", &self.pack_id)
            .field("issuer_id", &self.issuer_id)
            .field("issuer_name", &self.issuer_name)
            .field("buyer_name", &self.buyer_name)
            .field("order_id", &self.order_id)
            .field("redeemed_at", &self.redeemed_at)
            .field("license_key", &"<redacted>")
            .finish()
    }
}

// ── redeem_license ──────────────────────────────────────────────────────

/// Read an optional Hub base-URL override from the active learner's
/// `preferences_json`. Falls back to the production Hub default when
/// absent/malformed (Claude's Discretion per A2).
fn read_hub_url_config(conn: &rusqlite::Connection) -> String {
    const DEFAULT_HUB_URL: &str = "https://hub.learnforge.dev";

    let prefs_json: Option<String> = conn
        .query_row(
            "SELECT preferences_json FROM learner_profiles ORDER BY id ASC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    let Some(prefs_json) = prefs_json else {
        return DEFAULT_HUB_URL.to_string();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&prefs_json) else {
        return DEFAULT_HUB_URL.to_string();
    };
    v.get("hubUrl")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_HUB_URL.to_string())
}

/// Validate `request.license_key` against the Hub and return the
/// confirm-dialog payload. Performs NO download (D-03) — the DB is locked
/// ONLY to read the Hub base-URL config, then the guard is dropped before
/// the network `.await`.
async fn redeem_license_impl(
    db: &std::sync::Mutex<Database>,
    request: &RedeemLicenseIpcRequest,
) -> Result<RedeemLicenseResult, RedeemIpcError> {
    let hub_base_url = {
        let conn_guard = db.lock().map_err(RedeemIpcError::generic)?;
        read_hub_url_config(&conn_guard.conn)
    };

    let redeem_request = RedeemLicenseRequest {
        license_key: request.license_key.clone(),
        device_fingerprint: request.device_fingerprint.clone(),
    };

    call_redeem_endpoint(&hub_base_url, &redeem_request)
        .await
        .map_err(RedeemIpcError::from)
}

/// Thin shim over `redeem_license_impl`. Validates a license key and
/// returns the confirm-dialog data (packId/issuerName/buyerName/orderId/
/// downloadUrl/redeemedAt) — no download happens here (D-03).
#[tauri::command]
pub async fn redeem_license(
    request: RedeemLicenseIpcRequest,
    state: State<'_, crate::AppState>,
) -> Result<RedeemLicenseResult, RedeemIpcError> {
    redeem_license_impl(state.db.as_ref(), &request).await
}

// ── download_and_import_pack ────────────────────────────────────────────

/// WR-01 — record the entitlement row and stamp `learning_paths.pack_id`
/// in ONE `BEGIN IMMEDIATE` transaction. Previously these were two
/// separately-committed writes: a crash (or constraint failure) between
/// them left an imported track with no attribution stamp — or an
/// entitlements row `get_entitlement_for_track` could never resolve. On
/// any failure the whole pair rolls back.
///
/// `SqliteEntitlementStore::insert` upserts on `pack_id` conflict, so a
/// re-redeem of the same pack refreshes attribution instead of erroring
/// after the import already committed.
fn record_entitlement_and_stamp(
    conn: &rusqlite::Connection,
    row: &EntitlementRow,
    track_id: &str,
) -> Result<(), String> {
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| e.to_string())?;
    let writes = (|| -> Result<(), String> {
        SqliteEntitlementStore(conn).insert(row)?;
        // D-08 attribution join — follow-up UPDATE so import_course_impl
        // stays byte-identical; import_course_impl does not know about
        // pack_id.
        conn.execute(
            "UPDATE learning_paths SET pack_id = ?1 WHERE track_id = ?2",
            rusqlite::params![row.pack_id, track_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })();
    match writes {
        Ok(()) => conn
            .execute_batch("COMMIT")
            .map_err(|e| e.to_string()),
        Err(e) => {
            // Best-effort rollback — the original error is what matters.
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Fetch the buyer-stamped pack via `download_and_store` (OUTSIDE any DB
/// lock), then import it through the UNCHANGED `import_course_impl` Step
/// 3.5 gate, insert an `EntitlementRow`, and stamp
/// `learning_paths.pack_id` for attribution (D-08). Does NOT re-sanitize
/// `pack_id` — `download_and_store` owns that guard (T-15-14); a malicious
/// `packId` surfaces as that layer's rejection error, propagated verbatim.
async fn download_and_import_pack_impl(
    db: &std::sync::Mutex<Database>,
    app_data_dir: &std::path::Path,
    request: &DownloadAndImportPackRequest,
) -> Result<ImportCourseResult, RedeemIpcError> {
    // Network I/O first — no DB lock held across this `.await` (T-15-15).
    let retained_path = download_and_store(&request.download_url, &request.pack_id, app_data_dir)
        .await
        .map_err(RedeemIpcError::from)?;

    // Re-lock the DB ONLY for the synchronous import + entitlement-record +
    // attribution-stamp work below — no further `.await` inside this scope.
    let conn_guard = db.lock().map_err(RedeemIpcError::generic)?;

    let import_result =
        import_course_impl(&conn_guard.conn, &retained_path).map_err(RedeemIpcError::generic)?;

    let entitlement_row = EntitlementRow {
        pack_id: request.pack_id.clone(),
        issuer_id: request.issuer_id.clone(),
        issuer_name: request.issuer_name.clone(),
        buyer_name: request.buyer_name.clone(),
        order_id: request.order_id.clone(),
        redeemed_at: request.redeemed_at.clone(),
        // Raw license_key is used ONLY here to compute the fingerprint, then
        // dropped — never persisted, logged, or error-embedded (D-06).
        key_fingerprint: sha256_fingerprint(&request.license_key),
    };
    // WR-01 — entitlement record + attribution stamp are ONE transaction
    // (rolls back together; upserts on a same-pack re-redeem).
    record_entitlement_and_stamp(&conn_guard.conn, &entitlement_row, &import_result.track_id)
        .map_err(RedeemIpcError::generic)?;

    Ok(import_result)
}

/// Thin shim over `download_and_import_pack_impl`. Fetches the buyer-stamped
/// pack, imports it through the unchanged gate, caches the entitlement, and
/// stamps `learning_paths.pack_id`.
#[tauri::command]
pub async fn download_and_import_pack(
    request: DownloadAndImportPackRequest,
    state: State<'_, crate::AppState>,
    app_handle: tauri::AppHandle,
) -> Result<ImportCourseResult, RedeemIpcError> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(RedeemIpcError::generic)?;
    download_and_import_pack_impl(state.db.as_ref(), &app_data_dir, &request).await
}

// ── recover_redeemed_pack (CR-01 stranded-purchase local recovery) ────────

/// `recover_redeemed_pack` IPC request.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverRedeemedPackRequest {
    /// Used ONLY to compute the fingerprint for the local cache lookup —
    /// never sent over the network, persisted, or logged (D-06).
    pub license_key: String,
}

/// WR-04 (D-06) — manual Debug impl so `{:?}` can never leak the raw
/// license key.
impl std::fmt::Debug for RecoverRedeemedPackRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecoverRedeemedPackRequest")
            .field("license_key", &"<redacted>")
            .finish()
    }
}

/// CR-01 — outcome of a successful local recovery: the track the pack
/// resolves to, and whether it was already in the library (vs re-imported
/// from the retained artifact just now).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredPack {
    pub track_id: String,
    pub already_imported: bool,
}

/// CR-01 — stranded-purchase local recovery probe, called when the Hub
/// rejects a key as `already_redeemed`. Entirely offline (zero network —
/// re-redeeming would just burn another Hub round trip on the same
/// rejection):
///
/// 1. Resolve the local entitlements row by the key's SHA-256 fingerprint
///    (the only key-derived value ever persisted, D-06). No row → `None`:
///    the key was redeemed elsewhere/never completed here — the UI renders
///    contact-the-issuer guidance.
/// 2. Row + a `learning_paths.pack_id` stamp → the course is ALREADY in
///    the library; return the existing track.
/// 3. Row but no track (deleted, or a prior partial failure) → re-import
///    the retained artifact (D-07) through the UNCHANGED
///    `import_course_impl` Step 3.5 gate. A gate rejection or missing
///    artifact is a clean `None`, never a partial state.
///
/// Residual limitation (documented in 15-REVIEW.md): a download that
/// failed BEFORE any artifact/entitlement row landed can only be recovered
/// server-side (idempotent re-redeem is a Hub contract concern).
fn recover_redeemed_pack_impl(
    db: &std::sync::Mutex<Database>,
    app_data_dir: &std::path::Path,
    license_key: &str,
) -> Result<Option<RecoveredPack>, RedeemIpcError> {
    let fingerprint = sha256_fingerprint(license_key);
    let conn_guard = db.lock().map_err(RedeemIpcError::generic)?;

    let store = SqliteEntitlementStore(&conn_guard.conn);
    let Some(row) = store
        .find_by_key_fingerprint(&fingerprint)
        .map_err(RedeemIpcError::generic)?
    else {
        return Ok(None);
    };

    // Step 2 — already imported on this device?
    let track_id: Option<String> = conn_guard
        .conn
        .query_row(
            "SELECT track_id FROM learning_paths WHERE pack_id = ?1",
            rusqlite::params![row.pack_id],
            |r| r.get(0),
        )
        .ok();
    if let Some(track_id) = track_id {
        return Ok(Some(RecoveredPack {
            track_id,
            already_imported: true,
        }));
    }

    // Step 3 — retained artifact re-import through the UNCHANGED gate.
    let Ok(artifact) =
        crate::entitlements::download::retained_artifact_path(app_data_dir, &row.pack_id)
    else {
        // An unclean pack_id can never map to a retained artifact.
        return Ok(None);
    };
    if !artifact.exists() {
        return Ok(None);
    }
    let Ok(import_result) = import_course_impl(&conn_guard.conn, &artifact.to_string_lossy())
    else {
        // Tampered/untrusted artifact — the gate rejected it; nothing to
        // recover locally, and no partial state was written.
        return Ok(None);
    };
    // WR-01 — refresh the entitlement row + stamp atomically.
    record_entitlement_and_stamp(&conn_guard.conn, &row, &import_result.track_id)
        .map_err(RedeemIpcError::generic)?;
    Ok(Some(RecoveredPack {
        track_id: import_result.track_id,
        already_imported: false,
    }))
}

/// Thin shim over `recover_redeemed_pack_impl`. Local-only (zero network).
#[tauri::command]
pub fn recover_redeemed_pack(
    request: RecoverRedeemedPackRequest,
    state: State<'_, crate::AppState>,
    app_handle: tauri::AppHandle,
) -> Result<Option<RecoveredPack>, RedeemIpcError> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(RedeemIpcError::generic)?;
    recover_redeemed_pack_impl(state.db.as_ref(), &app_data_dir, &request.license_key)
}

// ── get_entitlement_for_track (15-06, D-08 buyer attribution) ─────────────

/// `get_entitlement_for_track` IPC request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetEntitlementForTrackRequest {
    pub track_id: String,
}

/// Display-only attribution fields surfaced to the renderer. Deliberately
/// excludes `key_fingerprint` — that field never crosses the IPC boundary
/// (T-15-19).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementAttribution {
    pub issuer_name: String,
    pub buyer_name: String,
    pub order_id: String,
}

/// Resolve the buyer-attribution row for `track_id`, entirely from local
/// SQLite (zero network — ENT-04's offline attribution proof). Joins
/// `learning_paths.pack_id` (stamped by `download_and_import_pack_impl`,
/// D-08) to the `entitlements` table via `SqliteEntitlementStore::
/// find_by_pack_id`. A track with no `pack_id`, or a `pack_id` with no
/// entitlements row, is a clean `Ok(None)` — most tracks are unlicensed, so
/// a miss is the expected common case, never an error.
fn get_entitlement_for_track_impl(
    db: &std::sync::Mutex<Database>,
    track_id: &str,
) -> Result<Option<EntitlementAttribution>, String> {
    let conn_guard = db.lock().map_err(|e| e.to_string())?;

    let pack_id: Option<String> = conn_guard
        .conn
        .query_row(
            "SELECT pack_id FROM learning_paths WHERE track_id = ?1",
            rusqlite::params![track_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    let Some(pack_id) = pack_id else {
        return Ok(None);
    };

    let store = SqliteEntitlementStore(&conn_guard.conn);
    let row = store.find_by_pack_id(&pack_id)?;

    Ok(row.map(|r| EntitlementAttribution {
        issuer_name: r.issuer_name,
        buyer_name: r.buyer_name,
        order_id: r.order_id,
    }))
}

/// Thin shim over `get_entitlement_for_track_impl`. Local-only read — no
/// network I/O, no `key_fingerprint` exposure.
#[tauri::command]
pub fn get_entitlement_for_track(
    request: GetEntitlementForTrackRequest,
    state: State<'_, crate::AppState>,
) -> Result<Option<EntitlementAttribution>, String> {
    get_entitlement_for_track_impl(state.db.as_ref(), &request.track_id)
}

#[cfg(test)]
#[path = "entitlements_tests.rs"]
mod tests;
