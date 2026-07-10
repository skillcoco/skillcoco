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

#[cfg(test)]
#[path = "reports_tests.rs"]
mod tests;
