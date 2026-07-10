//! Signed skill report assembly — Phase 18.
//!
//! Composes existing rails (`canonical_json` + `signing`, Phase 7 Wave 5)
//! into a byte-stable, tamper-evident `ReportEnvelopeV1`. Modeled
//! structurally on `crate::achievements` (`maybe_issue`, `AchievementError`,
//! `CertPayloadV1`, `AchievementStore`) — this is composition, zero new
//! crypto.
//!
//! ## What lives here
//!
//! - Payload types (`ReportPayloadV1`, `CapabilityRow`, `EvidenceItem`,
//!   `EvidenceClass`, `ReportMetadata`, `ReportScope`, `ReportEnvelopeV1`)
//!   — all `#[serde(rename_all = "camelCase")]` per FIX-02.
//! - [`ReportStore`] trait — per-track granularity so [`assemble`] can do
//!   the D-04 whole-profile merge + track attribution itself. The
//!   rusqlite-backed impl lives in `src-tauri/src/storage_impl/reports.rs`.
//! - [`normalize_tag`] — Pitfall 4 capability-tag slug normalization
//!   (lowercase + whitespace/hyphen collapse) so cross-track merge dedupes
//!   casing/whitespace variants of the same capability tag.
//! - [`bands::band_for`] — D-05 mastery-band pure predicate (re-exported
//!   via the sibling `bands` module).
//! - [`assemble`] — the pure algorithm that stitches knowledge + practical
//!   + evidence together, does the D-04 whole-profile merge, and signs via
//!   the existing Ed25519 rail (A5 clock injection — explicit `now`,
//!   never `Utc::now()` inline).
//!
//! ## What does NOT live here
//!
//! - PDF rendering (`ReportPdfInput`, printpdf ops) — stays in
//!   `src-tauri/src/reports/artifacts.rs` per WR-01 / D-03 amendment
//!   (printpdf is not WASM-portable).
//! - `From<rusqlite::Error> for ReportError` — lives in src-tauri so the
//!   trait surface stays pure.
//! - Org/issuer countersigning (D-12) — the `issuer` field is a reserved
//!   `Option<serde_json::Value>` slot, always `None` this phase.

pub mod bands;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::canonical_json::{canonical_json_bytes, CanonicalJsonError};
use crate::signing::{public_key_fingerprint, sign_payload, SigningError, SigningKeyStore};

// ── Errors ───────────────────────────────────────────────────────────────

/// Errors raised by the report-assembly algorithm.
///
/// Same variant shape as `crate::achievements::AchievementError` minus
/// `Qr` (no QR embedding in reports) — `Db`/`Io` are stringified envelopes
/// populated by the rusqlite-backed [`ReportStore`] impl in src-tauri; the
/// pure crypto / canonical-JSON variants are covered by `#[from]`
/// conversions.
#[derive(Debug, Error)]
pub enum ReportError {
    /// Stringified database error from the storage impl side.
    #[error("database error: {0}")]
    Db(String),

    /// I/O error (populated only by FS-backed callers on the src-tauri
    /// side).
    #[error("io error: {0}")]
    Io(String),

    /// PKCS#8 / PEM encoding or decoding failure.
    #[error("pkcs8 / pem error: {0}")]
    Pkcs8(String),

    /// Signature operation failure (sign/verify).
    #[error("signature error: {0}")]
    Signature(String),

    /// JSON serialization / deserialization failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// PDF rendering failure (only ever raised by the src-tauri renderer;
    /// included here so the unified envelope type works across both pure
    /// and impure layers).
    #[error("pdf error: {0}")]
    Pdf(String),

    /// Validation / business-rule failure.
    #[error("validation error: {0}")]
    Validation(String),
}

impl From<SigningError> for ReportError {
    fn from(e: SigningError) -> Self {
        match e {
            SigningError::InvalidSignature => ReportError::Signature("invalid signature".to_string()),
            SigningError::KeyEncoding(msg) => ReportError::Pkcs8(msg),
            SigningError::Io(msg) => ReportError::Io(msg),
            SigningError::Canonical(c) => ReportError::Validation(c.to_string()),
        }
    }
}

impl From<CanonicalJsonError> for ReportError {
    fn from(e: CanonicalJsonError) -> Self {
        ReportError::Validation(e.to_string())
    }
}

// ── Evidence types ───────────────────────────────────────────────────────

/// Evidence class for one itemized proof point in a capability's evidence
/// ledger (D-06). Reserves an `exam` variant (D-07) — unused until Phase
/// 19 exam-sim results plug in; present now so the schema never breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceClass {
    /// A quiz attempt/result.
    Quiz,
    /// A completed hands-on lab.
    Lab,
    /// An earned certificate/badge.
    Cert,
    /// Module-level completion evidence (fallback for untagged content).
    Module,
    /// Reserved (D-07) — Phase 19 exam-sim results. Unused this phase.
    Exam,
}

/// One itemized evidence entry beneath a [`CapabilityRow`]'s summary
/// (D-06 — summary + evidence ledger). Carries `track_id`/`track_topic`
/// so whole-profile merged rows retain per-item track attribution (D-04).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceItem {
    /// Evidence class (quiz/lab/cert/module/exam).
    pub class: EvidenceClass,
    /// Human-readable label ("Quiz: RBAC basics").
    pub label: String,
    /// Free-text detail (score, steps completed, AI-judge verdict, etc.).
    pub detail: String,
    /// RFC-3339 timestamp the evidence was produced.
    pub date: String,
    /// Which track this evidence item came from. `None` only for
    /// synthetic/aggregate items with no single-track origin.
    pub track_id: Option<String>,
    /// Display topic of the originating track (D-04 attribution;
    /// human-readable pair to `track_id`).
    pub track_topic: Option<String>,
}

// ── Mastery + capability row ─────────────────────────────────────────────

/// One mastery dimension (knowledge or practical) — a named band (D-05)
/// paired with the raw percentage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasteryDimension {
    /// Named band (`Novice`/`Working`/`Proficient`/`Mastered`) per
    /// [`bands::band_for`].
    pub band: String,
    /// Raw mastery fraction (`0.0..=1.0`).
    pub pct: f64,
}

/// One capability row in a signed report. Knowledge mastery is always
/// present; practical mastery is `Option` so capabilities with no lab
/// content report "not assessed" (serializes as `null`), never `0%`.
///
/// `contributing_tracks` lists every track that fed this row — a single
/// entry for a Track-scope report, N entries for a merged WholeProfile
/// row (D-04 attribution).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityRow {
    /// Stable normalized slug (see [`normalize_tag`]) — merge key for
    /// whole-profile aggregation.
    pub slug: String,
    /// Human-readable capability label ("Can configure RBAC policies").
    pub label: String,
    /// Knowledge mastery dimension (BKT-derived).
    pub knowledge: MasteryDimension,
    /// Practical mastery dimension (lab-derived). `None` == "not
    /// assessed" — this capability has no lab content.
    pub practical: Option<MasteryDimension>,
    /// Every track that contributed to this row (D-04 attribution).
    pub contributing_tracks: Vec<String>,
    /// Itemized evidence ledger (D-06) backing this row's summary.
    pub evidence: Vec<EvidenceItem>,
}

// ── Report metadata + payload + envelope ─────────────────────────────────

/// Report metadata (D-09) — generated date, app version, pack provenance,
/// verified-issuer state (Phase 14 badge data).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportMetadata {
    /// RFC-3339 timestamp the report was assembled (from the injected
    /// `now`, never `Utc::now()` inline — A5 clock injection).
    pub generated_at: String,
    /// App version at export time (`env!("CARGO_PKG_VERSION")`).
    pub app_version: String,
    /// Source pack provenance string, if the underlying track came from a
    /// topic pack (e.g. `licensed:<pack_id>|<licensor>`).
    pub pack_provenance: Option<String>,
    /// Verified-issuer state (Phase 14 badge data), if applicable.
    pub verified_issuer: Option<String>,
}

/// Report scope — a single track, or the learner's whole profile
/// (D-04 — both variants ship).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportScope {
    /// Per-track report for the given track id.
    Track(String),
    /// Whole-profile report merging every track by capability tag.
    WholeProfile,
}

/// V1 signed skill-report payload contract. Independent from
/// `CertPayloadV1` (Phase 6) — not a retrofit; the two payload kinds
/// diverge in shape and purpose.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPayloadV1 {
    /// Learner display name, confirmed at export time (D-10).
    pub learner_name: String,
    /// Learner profile id (UUID).
    pub learner_id: String,
    /// Human-readable scope label (track topic, or "Whole Profile").
    pub scope_label: String,
    /// Per-capability rows (D-01/D-02/D-04/D-05/D-06).
    pub capabilities: Vec<CapabilityRow>,
    /// Report metadata (D-09).
    pub metadata: ReportMetadata,
    /// Reserved issuer/countersign slot (D-12) — org-grade trust arriving
    /// with team kit/Hub. Always `None` this phase.
    pub issuer: Option<serde_json::Value>,
    /// 8-hex SHA-256 fingerprint of the signing public key.
    pub key_fingerprint: String,
    /// Dispatch tag — `1` for this phase's payload shape.
    pub payload_version: u32,
}

/// The exact on-disk export shape (D-11) — what `export_report_json`
/// writes and what the Verify panel + forge-sign both parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEnvelopeV1 {
    /// The signed payload.
    pub payload: ReportPayloadV1,
    /// Hex-encoded Ed25519 signature over `canonical_json_bytes(&payload)`.
    pub signature_hex: String,
    /// 8-hex SHA-256 fingerprint of the signing public key (duplicated
    /// from `payload.key_fingerprint` at the envelope level for
    /// verify-without-parsing-payload convenience).
    pub key_fingerprint: String,
}

// ── Capability-tag normalization (Pitfall 4) ──────────────────────────────

/// Normalize a raw capability-tag string into a stable slug so cross-track
/// merge dedupes casing/whitespace/hyphenation variants of the same
/// capability (Pitfall 4).
///
/// Lowercases the input, collapses any run of non-alphanumeric characters
/// to a single `-`, and trims leading/trailing `-`.
///
/// # Example
///
/// ```
/// use learnforge_core::reports::normalize_tag;
///
/// let a = normalize_tag("Can Configure RBAC Policies");
/// let b = normalize_tag("can configure rbac policies");
/// let c = normalize_tag("  can-configure  RBAC   policies ");
/// assert_eq!(a, b);
/// assert_eq!(b, c);
/// assert_eq!(a, "can-configure-rbac-policies");
/// ```
pub fn normalize_tag(raw: &str) -> String {
    let lower = raw.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut last_was_sep = true; // treat leading run as already-separated (trims leading '-')
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('-');
            last_was_sep = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[cfg(test)]
#[path = "_tests.rs"]
mod tests;
