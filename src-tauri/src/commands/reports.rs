//! Phase 18 (18-03) — IPC handlers for signed skill-report assembly + export.
//!
//! Wires the pure `learnforge_core::reports::assemble` algorithm to real
//! data via `SqliteReportStore` (the per-track `ReportStore` impl) and the
//! existing `MutexCachedKeyStore` signing rail (same key as certificates,
//! D-11). Follows the exact `state.db.lock()` -> store -> core-fn ->
//! `.map_err` chain as `commands::achievements::export_certificate`.
//!
//! `export_report_json` writes the ENTIRE `ReportEnvelopeV1` as canonical
//! JSON bytes — `{ payload, signatureHex, keyFingerprint }` — the fixed
//! on-disk shape the Verify panel (18-06) and forge-sign (18-07) both
//! parse byte-for-byte.
//!
//! camelCase serde per FIX-02 / CONVENTIONS.md.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::storage_impl::reports::SqliteReportStore;
use crate::storage_impl::signing::MutexCachedKeyStore;
use learnforge_core::canonical_json::canonical_json_bytes;
use learnforge_core::reports::{self as reports, ReportEnvelopeV1, ReportError, ReportScope};
use learnforge_core::signing::SigningKeyStore;

// ── Defensive limits ──────────────────────────────────────────────────────

/// Cap for the exported report JSON, used by the 18-06 paste/import path.
/// 64KB comfortably covers a whole-profile report (D-02: ~8-15 capabilities
/// per course, several courses) while bounding DoS exposure.
pub const MAX_REPORT_JSON_LEN: usize = 64 * 1024;

// ── Request types ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssembleReportRequest {
    /// "track" | "whole-profile".
    pub scope: String,
    #[serde(default)]
    pub track_id: Option<String>,
    /// D-10 confirm-at-export learner name, baked into the signed payload.
    pub learner_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReportRequest {
    /// "track" | "whole-profile".
    pub scope: String,
    #[serde(default)]
    pub track_id: Option<String>,
    /// D-10 confirm-at-export learner name, baked into the signed payload.
    pub learner_name: String,
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Resolve the active learner id (single-learner desktop — first row).
/// Mirrors `commands::achievements::resolve_active_learner`.
fn resolve_active_learner(conn: &Connection) -> Result<String, ReportError> {
    conn.query_row(
        "SELECT id FROM learner_profiles ORDER BY id ASC LIMIT 1",
        [],
        |r| r.get(0),
    )
    .map_err(|e| ReportError::Validation(format!("no learner profile available: {}", e)))
}

/// Parse the wire `scope` string into a `ReportScope`. `"track"` requires
/// `track_id` to be present; `"whole-profile"` ignores it.
fn parse_scope(scope: &str, track_id: &Option<String>) -> Result<ReportScope, ReportError> {
    match scope {
        "track" => {
            let id = track_id
                .clone()
                .ok_or_else(|| ReportError::Validation("track scope requires trackId".to_string()))?;
            Ok(ReportScope::Track(id))
        }
        "whole-profile" => Ok(ReportScope::WholeProfile),
        other => Err(ReportError::Validation(format!("unknown report scope: {}", other))),
    }
}

/// Shared inner assembly path for both IPC handlers — resolves the learner,
/// builds the store + key store, calls `reports::assemble`, and overrides
/// `payload.learner_name` with the confirm-at-export name (D-10).
fn assemble_report_inner(
    conn: &Connection,
    signing_key: &std::sync::Mutex<Option<ed25519_dalek::SigningKey>>,
    signing_key_path: &std::path::Path,
    scope: &str,
    track_id: &Option<String>,
    learner_name: &str,
) -> Result<ReportEnvelopeV1, ReportError> {
    let learner_id = resolve_active_learner(conn)?;
    let scope = parse_scope(scope, track_id)?;

    let store = SqliteReportStore(conn);
    let key_store = MutexCachedKeyStore::new(signing_key, signing_key_path);

    let mut envelope = reports::assemble(&store, &key_store, scope, &learner_id, chrono::Utc::now())?;
    // D-10 — learner identity is confirm-at-export; bake the confirmed name
    // into the signed payload. NOTE: this happens BEFORE assemble() signs
    // in normal flow would be ideal, but assemble() doesn't take a learner
    // name parameter (core stays learner-name-agnostic); the override here
    // means the signature covers the *empty* placeholder learner_name from
    // assemble(). Re-sign is required so the confirmed name is inside the
    // signed region (T-18-07).
    envelope.payload.learner_name = learner_name.to_string();
    let canonical = canonical_json_bytes(&envelope.payload)?;
    let key = key_store.get_or_init()?;
    let sig = learnforge_core::signing::sign_payload(&key, &canonical);
    envelope.signature_hex = hex::encode(sig.to_bytes());

    Ok(envelope)
}

// ── IPC handlers ─────────────────────────────────────────────────────────

/// Assemble and sign a skill report for the given scope. Returns the full
/// `ReportEnvelopeV1` (payload + signatureHex + keyFingerprint) for the
/// resolved (server-side) active learner.
#[tauri::command]
pub fn assemble_skill_report(
    request: AssembleReportRequest,
    state: State<'_, crate::AppState>,
) -> Result<ReportEnvelopeV1, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    assemble_report_inner(
        &db.conn,
        &state.signing_key,
        &state.signing_key_path,
        &request.scope,
        &request.track_id,
        &request.learner_name,
    )
    .map_err(|e| e.to_string())
}

/// Assemble, sign, and serialize a skill report as canonical JSON bytes —
/// the ENTIRE `ReportEnvelopeV1` (`{ payload, signatureHex, keyFingerprint }`),
/// never just the payload. This is the literal on-disk file the Verify
/// panel (18-06) and forge-sign (18-07) parse.
#[tauri::command]
pub fn export_report_json(
    request: ExportReportRequest,
    state: State<'_, crate::AppState>,
) -> Result<Vec<u8>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let envelope = assemble_report_inner(
        &db.conn,
        &state.signing_key,
        &state.signing_key_path,
        &request.scope,
        &request.track_id,
        &request.learner_name,
    )
    .map_err(|e| e.to_string())?;

    canonical_json_bytes(&envelope).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;
