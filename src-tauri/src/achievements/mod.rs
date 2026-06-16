//! Phase 6 (Certification) — achievements module entrypoint.
//!
//! Wave 0 (Plan 06-01) lands API surface only: every public function panics
//! with `unimplemented!("Wave N")` or returns a sentinel error. The point of
//! Wave 0 is to make the test surface compile and pin the file layout so
//! later waves only fill bodies.
//!
//! Wave 1 (Plan 06-02): v009 migration body + key lifecycle.
//! Wave 2 (Plan 06-03): `maybe_issue` body + IPC handlers + PDF/PNG bodies.
//! Wave 3+: surface integration (Dashboard / TrackView / PackPicker / Settings).
//!
//! D-04 (immutability) + R4 (track-topic snapshot) + R5 (forward-compat) all
//! live in this module's public types — see `Achievement` and
//! `CertPayloadV1`.

pub mod artifacts;
pub mod signing;
pub mod threshold;

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

use ed25519_dalek::SigningKey;
use rusqlite::Connection;

/// Persisted achievement row. Mirrors the `achievements` table v009 (Wave 1)
/// 1:1, with camelCase serde for IPC. D-12 + R4 + R5 dictate the field set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    /// UUID primary key.
    pub id: String,
    /// FK -> `learner_profiles.id`.
    pub learner_id: String,
    /// FK -> `learning_tracks.id`. NOT cascading on delete (R4).
    pub track_id: String,
    /// Source topic-pack id (snapshot at issuance). Null for AI-generated tracks.
    pub pack_id: Option<String>,
    /// "badge" or "certificate".
    pub kind: String,
    /// One of Associate / Practitioner / Professional / Completion.
    pub level: String,
    /// ISO 8601 UTC issuance timestamp.
    pub issued_at: String,
    /// Average mastery across the track AT THE TIME of issuance (D-04 snapshot).
    pub mastery_score: f64,
    /// The full canonical-JSON signed payload, base64url-encoded.
    pub payload_json: String,
    /// Hex-encoded Ed25519 signature of `payload_json`'s canonical bytes.
    pub signature: String,
    /// First 8 hex chars of SHA-256(verifying_key.to_public_key_der()).
    pub key_fingerprint: String,
    /// R4 — snapshot of the track's topic at issuance time so the cert is
    /// readable even after the track is deleted.
    pub track_topic: String,
}

/// Per-track certification status: which levels have been earned, what is
/// next, and the textual criteria for the next level. Phase 6 Wave 4
/// (TrackView indicator) reads this.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackCertifications {
    pub earned_levels: Vec<String>,
    pub next_level: Option<String>,
    pub criteria: String,
}

/// V1 signed-payload contract — see `docs/CERT-PAYLOAD-V1.md`. Phase 14
/// hosted verifier MUST honor this byte-for-byte.
///
/// **Canonical JSON rule:** object keys MUST be sorted lexicographically
/// before signing (R1 — Pitfall 2 mitigation). See
/// `signing::canonical_json_bytes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertPayloadV1 {
    pub learner: String,
    pub learner_id: String,
    pub track: String,
    pub track_id: String,
    pub level: String,
    pub completion_date: String,
    pub mastery_score: f64,
    pub key_fingerprint: String,
    pub pack_id: Option<String>,
    /// Dispatch tag — `1` for Phase 6 v1 payloads. Phase 14 introduces `2`
    /// if/when it switches to JWS-EdDSA. NEVER drop this field.
    pub payload_version: u32,
}

/// Errors any achievement-module operation may produce. Wave 1+ fills the
/// `From<...>` chains; Wave 0 declares the surface for compile-time
/// expectations.
#[derive(Debug, thiserror::Error)]
pub enum AchievementError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("pkcs8 / pem error: {0}")]
    Pkcs8(String),
    #[error("signature error: {0}")]
    Signature(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("pdf error: {0}")]
    Pdf(String),
    #[error("qr error: {0}")]
    Qr(String),
    #[error("validation error: {0}")]
    Validation(String),
}

/// Issuance entry point. Called from the BKT mastery-update path AFTER
/// `became_completed == true` per Pattern 4 in 06-RESEARCH.md.
///
/// Wave 0 stub: panics. The downstream `maybe_issue_idempotent` test asserts
/// the panic shape so Wave 1 can replace the body with a real impl.
pub fn maybe_issue(
    _conn: &Connection,
    _track_id: &str,
    _learner_id: &str,
    _signing_key: &Mutex<Option<SigningKey>>,
    _key_path: &Path,
) -> Result<Vec<Achievement>, AchievementError> {
    unimplemented!("Plan 06-03 (Wave 2) implements maybe_issue")
}

/// Read-all entry point for the learner's earned achievements.
/// Wave 2 (Plan 06-03) fills the body with `SELECT * FROM achievements
/// WHERE learner_id = ? ORDER BY issued_at DESC`.
pub fn list_for_learner_impl(
    _conn: &Connection,
) -> Result<Vec<Achievement>, AchievementError> {
    unimplemented!("Plan 06-03 (Wave 2) implements list_for_learner_impl")
}

/// Per-track earned / next-level lookup. Wave 2 (Plan 06-03) fills.
pub fn get_track_certifications_impl(
    _conn: &Connection,
    _track_id: &str,
) -> Result<TrackCertifications, AchievementError> {
    unimplemented!("Plan 06-03 (Wave 2) implements get_track_certifications_impl")
}

#[cfg(test)]
mod tests {
    //! Wave 0 RED contract tests for the module entrypoint. Each test
    //! captures the API surface Wave 1+ must satisfy. Today they FAIL via
    //! `unimplemented!()` panics — that is the RED state.

    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex;

    fn fresh_conn() -> rusqlite::Connection {
        rusqlite::Connection::open_in_memory().unwrap()
    }

    #[test]
    #[ignore = "Plan 06-03 (Wave 2) implements maybe_issue"]
    fn maybe_issue_idempotent() {
        // RED contract: calling `maybe_issue` twice with identical inputs
        // must return the same Achievement set the second time (no new
        // duplicates). Wave 0 panics with unimplemented! — flipping to a
        // real assertion lands with the Wave 2 implementation.
        let conn = fresh_conn();
        let key_slot: Mutex<Option<SigningKey>> = Mutex::new(None);
        let key_path = PathBuf::from("/tmp/learnforge-test-key");
        let first =
            maybe_issue(&conn, "trk-x", "lnr-x", &key_slot, &key_path).expect("first call");
        let second =
            maybe_issue(&conn, "trk-x", "lnr-x", &key_slot, &key_path).expect("second call");
        // Wave 2 invariant: second call yields no NEW achievements
        // because the UNIQUE(learner_id, track_id, level) constraint
        // makes INSERT OR IGNORE a no-op.
        assert!(
            second.is_empty(),
            "second maybe_issue must yield no NEW achievements (got {} — first had {})",
            second.len(),
            first.len()
        );
    }
}
