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

use crate::reports::artifacts::{ReportCapabilityRow, ReportPdfInput};
use crate::storage_impl::reports::SqliteReportStore;
use crate::storage_impl::signing::MutexCachedKeyStore;
use learnforge_core::canonical_json::canonical_json_bytes;
use learnforge_core::reports::{
    self as reports, CapabilityRow, ReportEnvelopeV1, ReportError, ReportPayloadV1, ReportScope,
};
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

/// Same wire shape as `ExportReportRequest` — a distinct type so the PDF
/// and JSON export IPC surfaces can diverge independently later without a
/// shared-request-type refactor.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReportPdfRequest {
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
    // The core is DB-agnostic and can only label a track scope with its id
    // (learnforge-core reports::assemble). Managers read this label in the
    // PDF title — swap in the human-readable track topic before the re-sign
    // below so the signature covers the displayed label.
    if let Some(id) = track_id {
        if envelope.payload.scope_label == *id {
            let topic: Option<String> = conn
                .query_row(
                    "SELECT topic FROM learning_tracks WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(topic) = topic {
                envelope.payload.scope_label = topic;
            }
        }
    }
    let canonical = canonical_json_bytes(&envelope.payload)?;
    let key = key_store.get_or_init()?;
    let sig = learnforge_core::signing::sign_payload(&key, &canonical);
    envelope.signature_hex = hex::encode(sig.to_bytes());

    Ok(envelope)
}

/// Format one dimension's display string per D-05: "{Band} · {pct}%".
fn format_dimension(band: &str, pct: f64) -> String {
    format!("{} · {:.0}%", band, pct * 100.0)
}

/// Format one evidence item into a single flattened display line for the
/// PDF renderer's evidence ledger (D-06). Time-to-mastery never appears as
/// its own numeric column (D-08) — this function only ever emits label +
/// detail + date context text.
fn format_evidence_line(item: &learnforge_core::reports::EvidenceItem) -> String {
    format!("{} — {} ({})", item.label, item.detail, item.date)
}

/// Transform an assembled `ReportPayloadV1` into the src-tauri-side
/// `ReportPdfInput` the renderer consumes. T-18-10 mitigation: this is the
/// ONLY place band/% display strings are derived for the PDF, and it
/// derives them from the SAME assembled payload `export_report_json`
/// serializes — no independent score computation.
fn report_payload_to_pdf_input(payload: &ReportPayloadV1) -> ReportPdfInput {
    let scope_label = if payload.scope_label.trim().is_empty() {
        "Whole Profile".to_string()
    } else {
        payload.scope_label.clone()
    };

    let capabilities = payload
        .capabilities
        .iter()
        .map(capability_row_to_pdf_row)
        .collect();

    ReportPdfInput {
        learner_name: payload.learner_name.clone(),
        scope_label,
        generated_at: payload.metadata.generated_at.clone(),
        app_version: payload.metadata.app_version.clone(),
        pack_provenance: payload.metadata.pack_provenance.clone(),
        verified_issuer: payload.metadata.verified_issuer.clone(),
        key_fingerprint_short: payload.key_fingerprint.clone(),
        capabilities,
    }
}

fn capability_row_to_pdf_row(row: &CapabilityRow) -> ReportCapabilityRow {
    let knowledge_display = format_dimension(&row.knowledge.band, row.knowledge.pct);
    let practical_display = match &row.practical {
        Some(dim) => format_dimension(&dim.band, dim.pct),
        // D-05 — "not assessed" (no lab content for this capability),
        // NEVER a bare "0%".
        None => "Not assessed".to_string(),
    };
    let evidence_lines = row.evidence.iter().map(format_evidence_line).collect();

    ReportCapabilityRow {
        label: row.label.clone(),
        knowledge_display,
        practical_display,
        evidence_lines,
    }
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

/// Assemble the signed report then render it to a paginated, manager-
/// readable PDF (REP-01 PDF half). The `ReportPdfInput` fed to the
/// renderer is derived from the SAME assembled payload
/// `export_report_json` serializes (T-18-10) — no independent score
/// computation for the PDF path.
#[tauri::command]
pub fn export_report_pdf(
    request: ExportReportPdfRequest,
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

    let pdf_input = report_payload_to_pdf_input(&envelope.payload);
    crate::reports::artifacts::render_report_pdf(&pdf_input).map_err(|e| e.to_string())
}

// ── D-13: org evidence submission (fire-and-forget, offline-safe) ────────

/// Wire request for `submit_evidence_report` — same scope/trackId/
/// learnerName shape as `ExportReportRequest` (mirrors the frontend
/// `SubmitEvidenceReportRequest` type in `src/lib/tauri-commands.ts`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitEvidenceReportRequest {
    /// "track" | "whole-profile".
    pub scope: String,
    #[serde(default)]
    pub track_id: Option<String>,
    /// D-10 confirm-at-export learner name, baked into the signed payload.
    pub learner_name: String,
}

/// Outcome of a `submit_evidence_report` call. `accepted: false` covers
/// EVERY non-2xx outcome (offline, timeout, non-http scheme, no URL
/// configured) — none of these are learner-blocking errors (D-13).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitEvidenceReportResult {
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_id: Option<String>,
}

/// Signature block of the `/v1/evidence/reports` wire contract
/// (`.planning/notes/entitlement-api-contract.md`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceSignature {
    alg: &'static str,
    sig: String,
    key_fingerprint: String,
}

/// `POST /v1/evidence/reports` body — the report is an OPAQUE signed
/// envelope (payload + signatureHex + keyFingerprint); the report server
/// never needs to understand LearnForge's mastery model, only verify the
/// signature (entitlement-api-contract.md).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceSubmissionBody {
    learner_id: String,
    skill_id: String,
    report: ReportEnvelopeV1,
    signature: EvidenceSignature,
}

/// Minimal ack shape from the report server on 2xx.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceAckResponse {
    #[serde(default)]
    accepted: bool,
    #[serde(default)]
    report_id: Option<String>,
}

/// Read `reportServerUrl`/`reportServerToken` from the active learner's
/// `preferences_json` (same storage surface `SettingsReportServerSection`
/// writes via `update_profile`). Returns `(None, _)` when no URL is
/// configured — callers treat this as "queued, not an error" per D-13.
fn read_report_server_config(conn: &Connection, learner_id: &str) -> (Option<String>, String) {
    let prefs_json: Option<String> = conn
        .query_row(
            "SELECT preferences_json FROM learner_profiles WHERE id = ?1",
            [learner_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    let Some(prefs_json) = prefs_json else {
        return (None, String::new());
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&prefs_json) else {
        return (None, String::new());
    };
    let url = v
        .get("reportServerUrl")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let token = v
        .get("reportServerToken")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    (url, token)
}

/// Insert a queued row into `pending_evidence_submissions` (18-01's D-13
/// durable retry queue) so a failed/offline/no-URL submission is never
/// silently dropped. NEVER logs `signature_json`'s token — the token is
/// not part of the signature block at all (it travels as a bearer header,
/// never persisted to the queue table).
fn enqueue_pending_submission(
    conn: &Connection,
    envelope: &ReportEnvelopeV1,
    signature: &EvidenceSignature,
    report_server_url: &str,
) -> Result<(), String> {
    let payload_json = serde_json::to_string(envelope).map_err(|e| e.to_string())?;
    let signature_json = serde_json::to_string(signature).map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO pending_evidence_submissions
         (id, payload_json, signature_json, report_server_url, attempts)
         VALUES (?1, ?2, ?3, ?4, 0)",
        rusqlite::params![id, payload_json, signature_json, report_server_url],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Prepared submission state — everything derived synchronously from the
/// DB (assembly, signing, config read) BEFORE the async network call.
/// Splitting this out lets the caller drop its `std::sync::MutexGuard`
/// (not `Send`) before the `.await` boundary, and lets tests exercise the
/// preparation step without any network dependency.
struct PreparedSubmission {
    envelope: ReportEnvelopeV1,
    signature: EvidenceSignature,
    report_server_url: Option<String>,
    report_server_token: String,
}

/// Synchronous preparation step: assemble + sign the report, then read
/// the report-server config for that learner. No network I/O — safe to
/// call while holding a `std::sync::MutexGuard`.
fn prepare_submission(
    conn: &Connection,
    signing_key: &std::sync::Mutex<Option<ed25519_dalek::SigningKey>>,
    signing_key_path: &std::path::Path,
    scope: &str,
    track_id: &Option<String>,
    learner_name: &str,
) -> Result<PreparedSubmission, String> {
    let envelope = assemble_report_inner(
        conn,
        signing_key,
        signing_key_path,
        scope,
        track_id,
        learner_name,
    )
    .map_err(|e| e.to_string())?;
    let learner_id = envelope.payload.learner_id.clone();
    let (report_server_url, report_server_token) = read_report_server_config(conn, &learner_id);
    let signature = EvidenceSignature {
        alg: "ed25519",
        sig: envelope.signature_hex.clone(),
        key_fingerprint: envelope.key_fingerprint.clone(),
    };
    Ok(PreparedSubmission {
        envelope,
        signature,
        report_server_url,
        report_server_token,
    })
}

/// Shared implementation for `submit_evidence_report` — takes the DB
/// mutex + signing state directly (NOT `tauri::State`) so tests can drive
/// it with a plain in-memory `Database` behind an `Arc<Mutex<_>>`, no
/// Tauri app instance required. Locks `db` ONLY for synchronous DB work
/// (`prepare_submission`, and the queue-on-failure insert), never across
/// the network `.await` — `rusqlite::Connection` is `Send` but NOT
/// `Sync`, so neither the `std::sync::MutexGuard` NOR a borrowed
/// `&Connection` may cross an await point in a future the Tauri async
/// runtime must treat as `Send`.
async fn submit_evidence_report_impl(
    db: &std::sync::Mutex<crate::db::Database>,
    signing_key: &std::sync::Mutex<Option<ed25519_dalek::SigningKey>>,
    signing_key_path: &std::path::Path,
    request: &SubmitEvidenceReportRequest,
) -> Result<SubmitEvidenceReportResult, String> {
    let prepared = {
        let conn_guard = db.lock().map_err(|e| e.to_string())?;
        prepare_submission(
            &conn_guard.conn,
            signing_key,
            signing_key_path,
            &request.scope,
            &request.track_id,
            &request.learner_name,
        )?
    };

    // No URL configured — not an error, a "nothing to do" signal (D-13).
    let Some(report_server_url) = prepared.report_server_url.clone() else {
        return Ok(SubmitEvidenceReportResult {
            accepted: false,
            report_id: None,
        });
    };

    // T-18-19 — reject non-http(s) schemes BEFORE the reqwest call (basic
    // SSRF hygiene; the URL is the org's own server, an intentional
    // container-registry-style feature, not an open redirect target).
    let is_http_scheme =
        report_server_url.starts_with("http://") || report_server_url.starts_with("https://");
    if !is_http_scheme {
        let conn_guard = db.lock().map_err(|e| e.to_string())?;
        enqueue_pending_submission(&conn_guard.conn, &prepared.envelope, &prepared.signature, &report_server_url)?;
        return Ok(SubmitEvidenceReportResult {
            accepted: false,
            report_id: None,
        });
    }

    let body = EvidenceSubmissionBody {
        learner_id: prepared.envelope.payload.learner_id.clone(),
        skill_id: prepared.envelope.payload.scope_label.clone(),
        report: prepared.envelope.clone(),
        signature: EvidenceSignature {
            alg: prepared.signature.alg,
            sig: prepared.signature.sig.clone(),
            key_fingerprint: prepared.signature.key_fingerprint.clone(),
        },
    };
    let endpoint = format!("{}/v1/evidence/reports", report_server_url.trim_end_matches('/'));

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            let conn_guard = db.lock().map_err(|e| e.to_string())?;
            enqueue_pending_submission(&conn_guard.conn, &prepared.envelope, &prepared.signature, &report_server_url)?;
            return Ok(SubmitEvidenceReportResult {
                accepted: false,
                report_id: None,
            });
        }
    };

    // Network I/O — no lock held across this `.await` (T-18-19 / Send
    // safety). The token is used ONLY here, as a bearer header value —
    // never string-interpolated into a log/error message (T-18-18).
    let send_result = client
        .post(&endpoint)
        .bearer_auth(&prepared.report_server_token)
        .json(&body)
        .send()
        .await;

    match send_result {
        Ok(resp) if resp.status().is_success() => {
            let ack: EvidenceAckResponse = resp.json().await.unwrap_or(EvidenceAckResponse {
                accepted: true,
                report_id: None,
            });
            Ok(SubmitEvidenceReportResult {
                accepted: ack.accepted,
                report_id: ack.report_id,
            })
        }
        // Any non-2xx or network-level failure: queue, don't propagate as
        // a learner-blocking Err (D-13 fire-and-forget). Re-lock briefly
        // ONLY for this synchronous insert — never across the `.await`
        // above.
        _ => {
            let conn_guard = db.lock().map_err(|e| e.to_string())?;
            enqueue_pending_submission(&conn_guard.conn, &prepared.envelope, &prepared.signature, &report_server_url)?;
            Ok(SubmitEvidenceReportResult {
                accepted: false,
                report_id: None,
            })
        }
    }
}

/// Assemble, sign, and POST a skill report to the learner's configured
/// `reportServerUrl` (Settings). Thin `tauri::command` shim over
/// `submit_evidence_report_impl`.
#[tauri::command]
pub async fn submit_evidence_report(
    request: SubmitEvidenceReportRequest,
    state: State<'_, crate::AppState>,
) -> Result<SubmitEvidenceReportResult, String> {
    submit_evidence_report_impl(
        state.db.as_ref(),
        &state.signing_key,
        &state.signing_key_path,
        &request,
    )
    .await
}

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;
