//! Phase 6 (Certification) — IPC handler shells.
//!
//! Wave 0 declares the five IPC commands the frontend will call. Each body
//! returns `Err("Wave 2".to_string())` so the handlers COMPILE and can be
//! registered in `tauri::generate_handler!` if Wave 2 chooses to enable
//! them; until Wave 2 lands, leaving them registered would surface "Wave 2"
//! errors to the UI, so the registration in `lib.rs` is deferred to
//! Plan 06-03 per the plan's instruction.
//!
//! camelCase serde + `{ request: T }` envelope per CONVENTIONS.md.

use serde::{Deserialize, Serialize};

use crate::achievements::{Achievement, TrackCertifications};

// ── Request / Result types ────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCertificateRequest {
    pub achievement_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBadgeRequest {
    pub achievement_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyCertificateRequest {
    pub payload_b64: String,
    pub public_key_pem_override: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyCertificateResult {
    pub valid: bool,
    pub learner: String,
    pub track: String,
    pub level: String,
    pub completion_date: String,
    pub key_fingerprint: String,
    pub payload_version: u32,
}

// ── IPC handler shells ────────────────────────────────────────────────────

/// List the current learner's earned achievements (badges + certificates).
/// Wave 2 (Plan 06-03) wires this to `list_for_learner_impl`.
#[tauri::command]
pub fn list_achievements_for_learner(
    _state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<Achievement>, String> {
    Err("Wave 2 implements list_achievements_for_learner".to_string())
}

/// Per-track certification status: earned levels, next level, criteria.
/// Wave 2 (Plan 06-03) wires this to `get_track_certifications_impl`.
#[tauri::command]
pub fn get_track_certifications(
    _track_id: String,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<TrackCertifications, String> {
    Err("Wave 2 implements get_track_certifications".to_string())
}

/// Render a PDF certificate to bytes. Frontend converts to Blob for save-as.
#[tauri::command]
pub fn export_certificate_pdf(
    _request: ExportCertificateRequest,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<u8>, String> {
    Err("Wave 2 implements export_certificate_pdf".to_string())
}

/// Render a PNG skill-level badge to bytes (transparent bg, QR + optional
/// brand mark per D-06 amend). Text labels deferred to Phase 14.
#[tauri::command]
pub fn export_badge_png(
    _request: ExportBadgeRequest,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<u8>, String> {
    Err("Wave 2 implements export_badge_png".to_string())
}

/// Verify a pasted base64-encoded signed payload against either the local
/// public key (default) or a user-provided PEM override.
#[tauri::command]
pub fn verify_certificate(
    _request: VerifyCertificateRequest,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<VerifyCertificateResult, String> {
    Err("Wave 2 implements verify_certificate".to_string())
}
