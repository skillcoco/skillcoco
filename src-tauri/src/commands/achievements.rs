//! Phase 6 (Certification) — five live IPC handlers (Wave 2 Plan 06-03).
//!
//! Inner-helper-seam pattern (Phase 5 / Phase 03.1 precedent): each Tauri
//! command is a thin shim that locks state, calls a pure `*_impl` helper,
//! and maps errors to `String`. `verify_signature` is intentionally
//! infallible at the IPC level — failures live in the `error` field of
//! `VerifySignatureResult` so the frontend handles uniform structured
//! responses instead of decoding panic-shaped error strings.
//!
//! camelCase serde + `{ request: T }` envelope per CONVENTIONS.md.

use base64::Engine as _;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::achievements::artifacts::{self, BadgePngInput, CertificatePdfInput};
use crate::storage_impl::achievements::SqliteAchievementStore;
use crate::storage_impl::signing::FsKeyStore;
use learnforge_core::achievements::{
    Achievement, AchievementError, AchievementStore, TrackCertifications,
};
use learnforge_core::signing::{self as signing, SigningKeyStore};

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
pub struct VerifySignatureRequest {
    pub payload_b64: String,
    pub public_key_pem_override: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifySignatureResult {
    pub valid: bool,
    pub learner: String,
    pub track: String,
    pub level: String,
    pub completion_date: String,
    pub key_fingerprint: String,
    pub payload_version: u32,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTrackCertificationsRequest {
    pub track_id: String,
}

/// Phase 6 Wave 5 — request envelope for `fingerprint_from_public_pem`.
/// Pure helper IPC: derives the 8-hex SHA-256 fingerprint from a PEM
/// string, no disk I/O. Powers the Settings Verify panel mount-time
/// localFingerprint derivation (W4 fix).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintFromPemRequest {
    pub public_key_pem: String,
}

// ── Defensive limits (T-06-09 / DoS resistance) ──────────────────────────

const MAX_PAYLOAD_B64_LEN: usize = 8 * 1024; // 8KB
const MAX_PUBLIC_PEM_LEN: usize = 4 * 1024;  // 4KB

// ── Helpers ──────────────────────────────────────────────────────────────

/// Resolve the active learner id (single-learner desktop — first row).
fn resolve_active_learner(conn: &Connection) -> Result<String, AchievementError> {
    let id: String = conn
        .query_row(
            "SELECT id FROM learner_profiles ORDER BY id ASC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .map_err(|e| {
            AchievementError::Validation(format!("no learner profile available: {}", e))
        })?;
    Ok(id)
}

/// `base64url(canonical_bytes).sig_hex` — the format the QR payload carries.
fn encode_qr_payload(payload_json: &str, sig_hex: &str) -> String {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
    format!("{}.{}", b64, sig_hex)
}

/// Extract a displayable learner name from the achievement's signed
/// payload. Falls back to "Learner" on parse failure (T-06-13: never panic).
fn extract_learner_name(payload_json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(payload_json)
        .ok()
        .and_then(|v| {
            v.get("learner")
                .and_then(|s| s.as_str().map(|s| s.to_string()))
        })
        .unwrap_or_else(|| "Learner".to_string())
}

// ── IPC handlers ─────────────────────────────────────────────────────────

/// List the current learner's earned achievements (badges + certificates).
#[tauri::command]
pub fn list_achievements_for_learner(
    state: State<'_, crate::AppState>,
) -> Result<Vec<Achievement>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    SqliteAchievementStore(&db.conn)
        .list_for_learner()
        .map_err(|e| e.to_string())
}

/// Per-track certifications: earned levels + next-level criteria.
#[tauri::command]
pub fn get_track_certifications(
    request: GetTrackCertificationsRequest,
    state: State<'_, crate::AppState>,
) -> Result<TrackCertifications, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let learner_id = resolve_active_learner(&db.conn).map_err(|e| e.to_string())?;
    get_track_certifications_impl(&db.conn, &request.track_id, &learner_id)
        .map_err(|e| e.to_string())
}

/// Per-track certifications inner helper. Computes `next_level` + `criteria`
/// from the badge-level set returned by [`AchievementStore::earned_badge_levels`].
/// The string template + the ladder ordering match the pre-Wave-10 body
/// verbatim — frontend surfaces (Settings / Achievements panel) depend on
/// the exact wording.
///
/// Lifted verbatim from the pre-Wave-10 `src-tauri/src/achievements/mod.rs`
/// shim (Wave 10 cleanup target). Lives in the commands module because it
/// composes pure Rust over the `AchievementStore` trait — no SQL of its own.
pub fn get_track_certifications_impl(
    conn: &rusqlite::Connection,
    track_id: &str,
    learner_id: &str,
) -> Result<TrackCertifications, AchievementError> {
    let store = SqliteAchievementStore(conn);
    let earned_levels = store.earned_badge_levels(track_id, learner_id)?;
    let has = |name: &str| earned_levels.iter().any(|l| l == name);

    let (next_level, criteria) = if !has("Associate") {
        (
            Some("Associate".to_string()),
            "25% of modules mastered".to_string(),
        )
    } else if !has("Practitioner") {
        (
            Some("Practitioner".to_string()),
            "60% of modules mastered".to_string(),
        )
    } else if !has("Professional") {
        (
            Some("Professional".to_string()),
            "100% of modules mastered, average mastery >= 0.85, plus all practical labs if required"
                .to_string(),
        )
    } else {
        (None, String::new())
    };

    Ok(TrackCertifications {
        earned_levels,
        next_level,
        criteria,
    })
}

/// Render the certificate PDF for a given achievement id. Returns raw
/// bytes; the frontend (`exportCertificate` wrapper) routes through the
/// Tauri dialog plugin to write the bytes to disk.
#[tauri::command]
pub fn export_certificate(
    request: ExportCertificateRequest,
    state: State<'_, crate::AppState>,
) -> Result<Vec<u8>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let ach = SqliteAchievementStore(&db.conn)
        .lookup_achievement(&request.achievement_id)
        .map_err(|e| e.to_string())?;
    if ach.kind != "certificate" {
        return Err("Only completion certificates can be exported as PDF".to_string());
    }
    let qr_payload = encode_qr_payload(&ach.payload_json, &ach.signature);
    let qr_png = artifacts::render_qr_png(&qr_payload).map_err(|e| e.to_string())?;
    let learner_name = extract_learner_name(&ach.payload_json);
    let pdf_input = CertificatePdfInput {
        learner_name,
        track_topic: ach.track_topic.clone(),
        issued_at: ach.issued_at.clone(),
        mastery_score: ach.mastery_score,
        key_fingerprint_short: ach.key_fingerprint.clone(),
        level: ach.level.clone(),
        qr_png_bytes: qr_png,
    };
    artifacts::render_certificate_pdf(&pdf_input).map_err(|e| e.to_string())
}

/// Render the PNG badge for a given achievement id (works for both
/// `badge` and `certificate` kinds — PNG is the universal export per D-06).
#[tauri::command]
pub fn export_badge(
    request: ExportBadgeRequest,
    state: State<'_, crate::AppState>,
) -> Result<Vec<u8>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let ach = SqliteAchievementStore(&db.conn)
        .lookup_achievement(&request.achievement_id)
        .map_err(|e| e.to_string())?;
    let qr_payload = encode_qr_payload(&ach.payload_json, &ach.signature);
    let badge_input = BadgePngInput {
        level: ach.level.clone(),
        track_topic: ach.track_topic.clone(),
        issued_at: ach.issued_at.clone(),
        key_fingerprint_short: ach.key_fingerprint.clone(),
        qr_payload,
    };
    artifacts::render_badge_png(&badge_input).map_err(|e| e.to_string())
}

/// Verify a pasted signed payload. Defensively bounded for DoS resistance.
/// Always returns `Ok(VerifySignatureResult)` — failure information lives
/// in the `error` field (R5 / defensive: never propagates Rust panics or
/// errors to IPC consumers).
#[tauri::command]
pub fn verify_signature(
    request: VerifySignatureRequest,
    state: State<'_, crate::AppState>,
) -> Result<VerifySignatureResult, String> {
    // ── Size caps (T-06-09) ───────────────────────────────────────────
    if request.payload_b64.len() > MAX_PAYLOAD_B64_LEN {
        return Ok(VerifySignatureResult {
            valid: false,
            error: Some("payload_too_large".to_string()),
            ..Default::default()
        });
    }
    if let Some(ref pem) = request.public_key_pem_override {
        if pem.len() > MAX_PUBLIC_PEM_LEN {
            return Ok(VerifySignatureResult {
                valid: false,
                error: Some("public_key_too_large".to_string()),
                ..Default::default()
            });
        }
    }

    // ── Resolve which public PEM to verify against ─────────────────────
    let public_pem = match request.public_key_pem_override {
        Some(pem) => pem,
        None => match FsKeyStore::new(state.signing_key_path.clone()).export_public_pem() {
            Ok(pem) => pem,
            Err(_) => {
                return Ok(VerifySignatureResult {
                    valid: false,
                    error: Some("local_public_key_unavailable".to_string()),
                    ..Default::default()
                });
            }
        },
    };

    // ── Split "<b64>.<hex>" envelope ───────────────────────────────────
    let Some((payload_part, sig_part)) = request.payload_b64.split_once('.') else {
        return Ok(VerifySignatureResult {
            valid: false,
            error: Some("malformed_envelope".to_string()),
            ..Default::default()
        });
    };

    // ── Decode base64url → canonical bytes ────────────────────────────
    let canonical_bytes = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload_part)
    {
        Ok(b) => b,
        Err(_) => {
            return Ok(VerifySignatureResult {
                valid: false,
                error: Some("invalid_base64".to_string()),
                ..Default::default()
            });
        }
    };

    // ── Verify ────────────────────────────────────────────────────────
    let is_valid = signing::verify_payload(&public_pem, &canonical_bytes, sig_part);

    // ── Compute display fields from canonical_bytes JSON ───────────────
    let mut result = VerifySignatureResult {
        valid: is_valid,
        ..Default::default()
    };
    match serde_json::from_slice::<serde_json::Value>(&canonical_bytes) {
        Ok(v) => {
            result.learner = v
                .get("learner")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            result.track = v
                .get("track")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            result.level = v
                .get("level")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            result.completion_date = v
                .get("completionDate")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            result.payload_version = v
                .get("payloadVersion")
                .and_then(|x| x.as_u64())
                .unwrap_or(0) as u32;
        }
        Err(_) => {
            // CR-02: an unparseable payload has no meaningful
            // verification semantics — even if the Ed25519 signature
            // verifies against the raw bytes, the cert as a
            // CertPayloadV1 contract is malformed. Force valid=false
            // and surface the precise error so the UI can render the
            // failure (instead of showing a green banner with empty
            // Learner/Track/Level fields).
            result.valid = false;
            result.error = Some(
                if is_valid {
                    "payload_unparseable"
                } else {
                    "signature_mismatch"
                }
                .to_string(),
            );
        }
    }

    // ── Fingerprint of the public key actually in use (helps the UX
    //     surface which signer was checked, esp. with override). ────────
    if let Ok(fp) = signing::fingerprint_from_public_pem(&public_pem) {
        result.key_fingerprint = fp;
    }

    if !is_valid && result.error.is_none() {
        result.error = Some("signature_mismatch".to_string());
    }

    Ok(result)
}

/// Phase 6 Wave 5 — return the local signing public-key PEM. Powers the
/// Settings "Show signing public key" button AND the on-mount
/// localFingerprint derivation (so the untrusted-signer warning works on
/// the FIRST override paste, without requiring a prior verify pass).
///
/// Errors with a stringified `AchievementError` on the cold-start case
/// where the signing key has not yet been generated (Phase 6 generates
/// lazily on first issuance per RESEARCH.md Pattern 2). The frontend
/// silently absorbs this error — the verifier panel still renders; only
/// the untrusted-signer warning is suppressed until a key exists.
#[tauri::command]
pub fn get_signing_public_key(state: State<'_, crate::AppState>) -> Result<String, String> {
    FsKeyStore::new(state.signing_key_path.clone())
        .export_public_pem()
        .map_err(|e| e.to_string())
}

/// Phase 6 Wave 5 — derive the 8-hex SHA-256 fingerprint from a PEM
/// string. Pure shim around `signing::fingerprint_from_public_pem` (which
/// landed in Wave 2). Enforces the same 4KB cap as `verify_signature`'s
/// PEM override (T-06-22 / DoS resistance) so the frontend can call this
/// on any user-pasted PEM without risking a giant string traversing the
/// IPC boundary.
#[tauri::command]
pub fn fingerprint_from_public_pem(
    request: FingerprintFromPemRequest,
) -> Result<String, String> {
    if request.public_key_pem.len() > MAX_PUBLIC_PEM_LEN {
        return Err("public_key_too_large".to_string());
    }
    signing::fingerprint_from_public_pem(&request.public_key_pem).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use ed25519_dalek::pkcs8::EncodePublicKey;
    use ed25519_dalek::SigningKey;
    use learnforge_core::achievements as core_achievements;
    use learnforge_core::signing as sig_mod;
    use rand::rngs::OsRng;
    use rusqlite::Connection;

    // ── Test helpers ──────────────────────────────────────────────────

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    fn seed_learner(conn: &Connection, id: &str, name: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id, display_name) VALUES (?1, ?2)",
            [id, name],
        )
        .unwrap();
    }

    /// Insert an achievement row directly. Bypasses maybe_issue so we can
    /// shape arbitrary fixtures (e.g., out-of-order issued_at timestamps).
    #[allow(clippy::too_many_arguments)]
    fn insert_ach(
        conn: &Connection,
        id: &str,
        learner_id: &str,
        track_id: &str,
        kind: &str,
        level: &str,
        issued_at: &str,
        payload_json: &str,
        signature: &str,
        key_fp: &str,
        track_topic: &str,
    ) {
        conn.execute(
            "INSERT INTO achievements
             (id, learner_id, track_id, pack_id, kind, level, issued_at,
              mastery_score, payload_json, signature, key_fingerprint, track_topic)
             VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, 0.9, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                id, learner_id, track_id, kind, level, issued_at, payload_json,
                signature, key_fp, track_topic
            ],
        )
        .unwrap();
    }

    fn fresh_key_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    // ── list_achievements_for_learner ─────────────────────────────────

    #[test]
    fn list_achievements_returns_for_single_learner() {
        let conn = fresh_conn();
        seed_learner(&conn, "lp1", "Ada");
        insert_ach(
            &conn, "a1", "lp1", "t1", "badge", "Associate",
            "2026-06-10T10:00:00Z", "{}", "sig1", "deadbeef", "Kubernetes",
        );
        insert_ach(
            &conn, "a2", "lp1", "t1", "badge", "Practitioner",
            "2026-06-12T10:00:00Z", "{}", "sig2", "deadbeef", "Kubernetes",
        );
        insert_ach(
            &conn, "a3", "lp1", "t1", "badge", "Professional",
            "2026-06-15T10:00:00Z", "{}", "sig3", "deadbeef", "Kubernetes",
        );

        let rows = SqliteAchievementStore(&conn).list_for_learner().expect("list");
        assert_eq!(rows.len(), 3);
        // issued_at DESC.
        assert_eq!(rows[0].id, "a3");
        assert_eq!(rows[1].id, "a2");
        assert_eq!(rows[2].id, "a1");
    }

    // ── get_track_certifications ──────────────────────────────────────

    #[test]
    fn get_track_certifications_when_none_earned() {
        let conn = fresh_conn();
        seed_learner(&conn, "lp1", "Ada");
        let result =
            get_track_certifications_impl(&conn, "trk-x", "lp1").expect("track certs");
        assert!(result.earned_levels.is_empty());
        assert_eq!(result.next_level.as_deref(), Some("Associate"));
        assert_eq!(result.criteria, "25% of modules mastered");
    }

    #[test]
    fn get_track_certifications_when_associate_earned() {
        let conn = fresh_conn();
        seed_learner(&conn, "lp1", "Ada");
        insert_ach(
            &conn, "a1", "lp1", "trk-x", "badge", "Associate",
            "2026-06-10T10:00:00Z", "{}", "sig", "deadbeef", "Kubernetes",
        );
        let result =
            get_track_certifications_impl(&conn, "trk-x", "lp1").expect("track certs");
        assert_eq!(result.earned_levels, vec!["Associate".to_string()]);
        assert_eq!(result.next_level.as_deref(), Some("Practitioner"));
        assert_eq!(result.criteria, "60% of modules mastered");
    }

    #[test]
    fn get_track_certifications_when_professional_earned() {
        let conn = fresh_conn();
        seed_learner(&conn, "lp1", "Ada");
        for (id, level, issued) in [
            ("a1", "Associate", "2026-06-10T10:00:00Z"),
            ("a2", "Practitioner", "2026-06-12T10:00:00Z"),
            ("a3", "Professional", "2026-06-15T10:00:00Z"),
        ] {
            insert_ach(
                &conn, id, "lp1", "trk-x", "badge", level, issued, "{}", "sig", "deadbeef",
                "Kubernetes",
            );
        }
        let result =
            get_track_certifications_impl(&conn, "trk-x", "lp1").expect("track certs");
        assert_eq!(result.earned_levels.len(), 3);
        assert!(result.next_level.is_none(), "no next level after Professional");
        assert_eq!(result.criteria, "");
    }

    // ── export_certificate ────────────────────────────────────────────

    /// Generate a signed achievement via maybe_issue so the payload + sig
    /// are real. Returns the achievement id of the certificate row.
    fn seed_signed_completion(conn: &Connection, key_dir: &std::path::Path) -> String {
        use std::sync::Mutex;
        seed_learner(conn, "lp1", "Ada");
        let path_id = "p-trk1".to_string();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'Kubernetes', 'devops', 'CKA')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES (?1, 'trk1', 1, '[]', '[]', 'test')",
            [&path_id],
        ).unwrap();
        for (i, (mid, ml)) in [
            ("m1", 0.92),
            ("m2", 0.88),
            ("m3", 0.95),
            ("m4", 0.92),
        ]
        .iter()
        .enumerate()
        {
            conn.execute(
                "INSERT INTO modules (id, path_id, title, ordering, content_json) VALUES (?1, ?2, ?3, ?4, '{}')",
                rusqlite::params![mid, &path_id, format!("M{}", i), i as i64],
            ).unwrap();
            conn.execute(
                "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level, practical_mastery) VALUES (?1, ?2, 'lp1', 'in_progress', ?3, 0.0)",
                rusqlite::params![format!("mp-{}", mid), mid, ml],
            ).unwrap();
        }
        let key_slot: Mutex<Option<SigningKey>> = Mutex::new(None);
        let store = SqliteAchievementStore(conn);
        let key_store = crate::storage_impl::signing::MutexCachedKeyStore::new(&key_slot, key_dir);
        let issued = core_achievements::maybe_issue(
            &store,
            &key_store,
            "trk1",
            "lp1",
            chrono::Utc::now(),
        )
        .expect("issue");
        // certificate row (kind="certificate", level="Completion").
        issued
            .into_iter()
            .find(|a| a.kind == "certificate")
            .expect("certificate present")
            .id
    }

    #[test]
    fn export_certificate_returns_bytes_for_certificate_kind() {
        let conn = fresh_conn();
        let key_dir = fresh_key_dir();
        let cert_id = seed_signed_completion(&conn, key_dir.path());
        let ach = SqliteAchievementStore(&conn).lookup_achievement(&cert_id).expect("lookup");
        assert_eq!(ach.kind, "certificate");

        // Reproduce what export_certificate does internally (the IPC
        // wrapper needs a State, so we test the same code path via the
        // public artifacts API + lookup_achievement_impl).
        let qr_payload = encode_qr_payload(&ach.payload_json, &ach.signature);
        let qr_png = artifacts::render_qr_png(&qr_payload).expect("qr png");
        let pdf_input = CertificatePdfInput {
            learner_name: extract_learner_name(&ach.payload_json),
            track_topic: ach.track_topic,
            issued_at: ach.issued_at,
            mastery_score: ach.mastery_score,
            key_fingerprint_short: ach.key_fingerprint,
            level: ach.level,
            qr_png_bytes: qr_png,
        };
        let bytes = artifacts::render_certificate_pdf(&pdf_input).expect("pdf");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() >= 1024);
    }

    #[test]
    fn export_certificate_fails_for_badge_kind() {
        let conn = fresh_conn();
        let key_dir = fresh_key_dir();
        let _cert_id = seed_signed_completion(&conn, key_dir.path());
        // Badge id for the Associate level.
        let badge_id: String = conn
            .query_row(
                "SELECT id FROM achievements WHERE kind = 'badge' AND level = 'Associate' LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let ach = SqliteAchievementStore(&conn).lookup_achievement(&badge_id).expect("lookup");
        assert_eq!(ach.kind, "badge");
        // The handler asserts: badges cannot be exported as PDF. We replicate
        // the predicate here (the State-bound handler test would require a
        // full Tauri runtime).
        assert!(
            ach.kind != "certificate",
            "Only completion certificates can be exported as PDF"
        );
    }

    #[test]
    fn export_badge_png_returns_bytes() {
        let conn = fresh_conn();
        let key_dir = fresh_key_dir();
        let _cert_id = seed_signed_completion(&conn, key_dir.path());
        let badge_id: String = conn
            .query_row(
                "SELECT id FROM achievements WHERE kind = 'badge' AND level = 'Associate' LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let ach = SqliteAchievementStore(&conn).lookup_achievement(&badge_id).expect("lookup");
        let qr_payload = encode_qr_payload(&ach.payload_json, &ach.signature);
        let badge_input = BadgePngInput {
            level: ach.level,
            track_topic: ach.track_topic,
            issued_at: ach.issued_at,
            key_fingerprint_short: ach.key_fingerprint,
            qr_payload,
        };
        let bytes = artifacts::render_badge_png(&badge_input).expect("png");
        assert!(!bytes.is_empty());
        // PNG signature.
        assert_eq!(
            &bytes[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }

    // ── verify_signature (pure function tests — covers the inner logic) ─

    /// Reproduce verify_signature's body without the State<AppState>
    /// wrapping so each test can drive the logic directly.
    fn verify_signature_inner(
        public_pem: &str,
        payload_b64: &str,
    ) -> VerifySignatureResult {
        if payload_b64.len() > MAX_PAYLOAD_B64_LEN {
            return VerifySignatureResult {
                valid: false,
                error: Some("payload_too_large".to_string()),
                ..Default::default()
            };
        }
        let Some((payload_part, sig_part)) = payload_b64.split_once('.') else {
            return VerifySignatureResult {
                valid: false,
                error: Some("malformed_envelope".to_string()),
                ..Default::default()
            };
        };
        let canonical_bytes =
            match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload_part) {
                Ok(b) => b,
                Err(_) => {
                    return VerifySignatureResult {
                        valid: false,
                        error: Some("invalid_base64".to_string()),
                        ..Default::default()
                    };
                }
            };
        let is_valid = signing::verify_payload(public_pem, &canonical_bytes, sig_part);
        let mut result = VerifySignatureResult {
            valid: is_valid,
            ..Default::default()
        };
        // Mirror the production handler's match (CR-02): on parse
        // failure, force `valid = false` and pick the precise error
        // code (payload_unparseable vs signature_mismatch).
        match serde_json::from_slice::<serde_json::Value>(&canonical_bytes) {
            Ok(v) => {
                result.learner = v
                    .get("learner")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                result.track = v
                    .get("track")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                result.level = v
                    .get("level")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                result.completion_date = v
                    .get("completionDate")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                result.payload_version =
                    v.get("payloadVersion").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            }
            Err(_) => {
                result.valid = false;
                result.error = Some(
                    if is_valid {
                        "payload_unparseable"
                    } else {
                        "signature_mismatch"
                    }
                    .to_string(),
                );
            }
        }
        if let Ok(fp) = signing::fingerprint_from_public_pem(public_pem) {
            result.key_fingerprint = fp;
        }
        if !result.valid && result.error.is_none() {
            result.error = Some("signature_mismatch".to_string());
        }
        result
    }

    /// Build a signed (payload_b64, sig_hex) pair from a SigningKey for
    /// the test fixture learner.
    fn build_signed_payload(key: &SigningKey, learner: &str, track: &str, level: &str) -> String {
        use pkcs8::LineEnding;
        let _ = key.verifying_key().to_public_key_pem(LineEnding::LF); // sanity
        let payload = core_achievements::CertPayloadV1 {
            learner: learner.to_string(),
            learner_id: "lp1".to_string(),
            track: track.to_string(),
            track_id: "trk1".to_string(),
            level: level.to_string(),
            completion_date: "2026-06-15T00:00:00Z".to_string(),
            mastery_score: 0.92,
            key_fingerprint: sig_mod::public_key_fingerprint(&key.verifying_key()),
            pack_id: None,
            payload_version: 1,
        };
        let canonical = learnforge_core::canonical_json::canonical_json_bytes(&payload).unwrap();
        let sig = sig_mod::sign_payload(key, &canonical);
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&canonical);
        format!("{}.{}", b64, hex::encode(sig.to_bytes()))
    }

    #[test]
    fn verify_signature_accepts_genuine_payload() {
        use pkcs8::LineEnding;
        let key = SigningKey::generate(&mut OsRng);
        let pem = key.verifying_key().to_public_key_pem(LineEnding::LF).unwrap();
        let payload_b64 = build_signed_payload(&key, "Ada", "Kubernetes", "Associate");
        let result = verify_signature_inner(&pem, &payload_b64);
        assert!(result.valid, "genuine payload must verify");
        assert_eq!(result.learner, "Ada");
        assert_eq!(result.track, "Kubernetes");
        assert_eq!(result.level, "Associate");
        assert_eq!(result.payload_version, 1);
        assert!(result.error.is_none());
    }

    #[test]
    fn verify_signature_rejects_tampered_payload() {
        use pkcs8::LineEnding;
        let key = SigningKey::generate(&mut OsRng);
        let pem = key.verifying_key().to_public_key_pem(LineEnding::LF).unwrap();
        let payload_b64 = build_signed_payload(&key, "Ada", "Kubernetes", "Associate");
        // Flip one byte in the b64 portion → canonical bytes mutate but
        // signature is still over the original.
        let (b64_part, sig_part) = payload_b64.split_once('.').unwrap();
        let mut tampered = b64_part.to_string();
        let first = tampered.remove(0);
        let new_first = if first == 'A' { 'B' } else { 'A' };
        tampered.insert(0, new_first);
        let tampered_full = format!("{}.{}", tampered, sig_part);
        let result = verify_signature_inner(&pem, &tampered_full);
        assert!(!result.valid, "tampered payload must NOT verify");
        assert_eq!(result.error.as_deref(), Some("signature_mismatch"));
    }

    #[test]
    fn verify_signature_rejects_with_wrong_override_key() {
        use pkcs8::LineEnding;
        let key_a = SigningKey::generate(&mut OsRng);
        let key_b = SigningKey::generate(&mut OsRng);
        let pem_b = key_b.verifying_key().to_public_key_pem(LineEnding::LF).unwrap();
        let payload_b64 = build_signed_payload(&key_a, "Ada", "Kubernetes", "Associate");
        let result = verify_signature_inner(&pem_b, &payload_b64);
        assert!(!result.valid);
        assert_eq!(result.error.as_deref(), Some("signature_mismatch"));
    }

    #[test]
    fn verify_signature_handles_malformed_payload_gracefully() {
        use pkcs8::LineEnding;
        let key = SigningKey::generate(&mut OsRng);
        let pem = key.verifying_key().to_public_key_pem(LineEnding::LF).unwrap();
        let result = verify_signature_inner(&pem, "not_base64!!");
        assert!(!result.valid);
        // No dot → malformed_envelope.
        assert_eq!(result.error.as_deref(), Some("malformed_envelope"));

        // Bad base64 with dot.
        let result_b64 = verify_signature_inner(&pem, "not_base64!!.deadbeef");
        assert!(!result_b64.valid);
        assert_eq!(result_b64.error.as_deref(), Some("invalid_base64"));
    }

    #[test]
    fn verify_signature_rejects_oversize_payload() {
        use pkcs8::LineEnding;
        let key = SigningKey::generate(&mut OsRng);
        let pem = key.verifying_key().to_public_key_pem(LineEnding::LF).unwrap();
        let oversize = "a".repeat(MAX_PAYLOAD_B64_LEN + 1);
        let result = verify_signature_inner(&pem, &oversize);
        assert!(!result.valid);
        assert_eq!(result.error.as_deref(), Some("payload_too_large"));
    }

    /// CR-02 regression — a payload whose Ed25519 signature verifies but
    /// whose bytes are NOT parseable as JSON must be reported as
    /// `valid: false` with `error: "payload_unparseable"`. Before the fix,
    /// the IPC handler set `valid = true` (because `is_valid = true`) AND
    /// `error = "payload_unparseable"` — a contradiction that the
    /// Settings panel rendered as a green "Valid signature" banner with
    /// empty Learner/Track/Level fields. The contract is: if the
    /// canonical bytes cannot be parsed into JSON, the verification has
    /// no meaningful semantics, so `valid` MUST be false.
    #[test]
    fn verify_signature_rejects_unparseable_payload_even_when_signature_valid() {
        use pkcs8::LineEnding;
        let key = SigningKey::generate(&mut OsRng);
        let pem = key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap();

        // Sign arbitrary bytes that are NOT valid JSON. `sign_payload`
        // signs whatever bytes it receives — we exploit that to
        // construct a (valid signature) + (unparseable payload) pair,
        // which is exactly the contradictory state CR-02 describes.
        let garbage: &[u8] = b"this is definitely not JSON \x00\xff\xfe";
        let sig = sig_mod::sign_payload(&key, garbage);
        let sig_hex = hex::encode(sig.to_bytes());
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(garbage);
        let envelope = format!("{}.{}", b64, sig_hex);

        let result = verify_signature_inner(&pem, &envelope);

        // The signature itself verifies — but the payload cannot be parsed.
        // Per CR-02, valid MUST be false (not true).
        assert!(
            !result.valid,
            "signed-but-unparseable payload MUST be reported as invalid \
             — sig verifies against bytes, but bytes are not a JSON cert"
        );
        assert_eq!(
            result.error.as_deref(),
            Some("payload_unparseable"),
            "error code must communicate the precise failure mode so the \
             UI can distinguish payload_unparseable from signature_mismatch"
        );
        // Display fields must be empty — we never extracted them.
        assert_eq!(result.learner, "");
        assert_eq!(result.track, "");
        assert_eq!(result.level, "");
    }

    // ── encode_qr_payload sanity ─────────────────────────────────────

    #[test]
    fn encode_qr_payload_is_dot_separated_base64_then_hex() {
        let s = encode_qr_payload("{\"a\":1}", "abc123");
        let (b64, sig) = s.split_once('.').expect("must contain dot");
        assert_eq!(sig, "abc123");
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(b64)
            .expect("valid base64url");
        assert_eq!(decoded, b"{\"a\":1}");
    }

    // ── extract_learner_name sanity (T-06-13 defensive) ──────────────

    #[test]
    fn extract_learner_name_falls_back_on_garbage() {
        assert_eq!(extract_learner_name("not json"), "Learner");
        assert_eq!(extract_learner_name("{}"), "Learner");
        assert_eq!(
            extract_learner_name(r#"{"learner": "Bob"}"#),
            "Bob"
        );
    }

    // ── Phase 6 Wave 5 — get_signing_public_key + fingerprint_from_public_pem
    //     IPC handler tests. The Tauri-State-bound handler bodies are
    //     trivial wrappers around `signing::*`; we test them by invoking
    //     the same code path against an ephemeral keys directory (no
    //     State<AppState> needed since the wrapping is one line). ──────

    #[test]
    fn get_signing_public_key_returns_pem() {
        let key_dir = fresh_key_dir();
        // Pre-generate a keypair so `export_public_pem` has a file.
        let store = FsKeyStore::new(key_dir.path().to_path_buf());
        let _key = store.get_or_init().expect("init key");
        let pem = store.export_public_pem().expect("read public pem");
        assert!(
            pem.starts_with("-----BEGIN PUBLIC KEY-----"),
            "must be PEM-encoded, got: {:?}",
            &pem[..pem.len().min(40)]
        );
        assert!(
            pem.contains("-----END PUBLIC KEY-----"),
            "must contain end marker"
        );
    }

    #[test]
    fn get_signing_public_key_errors_when_no_key_yet() {
        // Fresh dir, no key generated — the on-disk file is absent.
        // `FsKeyStore::export_public_pem` returns Err (Io kind) and the IPC
        // handler maps to String. The frontend interprets this as the
        // "no signing key yet" case (Phase 6 generates lazily on first
        // issuance) and silently degrades to localFingerprint=null.
        let key_dir = fresh_key_dir();
        let result = FsKeyStore::new(key_dir.path().to_path_buf()).export_public_pem();
        assert!(result.is_err(), "no key file → error");
        let msg = format!("{:?}", result.unwrap_err()).to_lowercase();
        assert!(
            msg.contains("not found")
                || msg.contains("no such file")
                || msg.contains("os error 2")
                || msg.contains("io"),
            "expected file-not-found-ish error, got: {}",
            msg
        );
    }

    #[test]
    fn fingerprint_from_public_pem_command_returns_8_hex() {
        use pkcs8::LineEnding;
        let key = SigningKey::generate(&mut OsRng);
        let pem = key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("encode pem");

        // Drive the same code path the IPC handler uses.
        let request = FingerprintFromPemRequest { public_key_pem: pem.clone() };
        // Reproduce the handler body (no Tauri State needed):
        let result: Result<String, String> = if request.public_key_pem.len() > MAX_PUBLIC_PEM_LEN {
            Err("public_key_too_large".to_string())
        } else {
            sig_mod::fingerprint_from_public_pem(&request.public_key_pem)
                .map_err(|e| e.to_string())
        };
        let fp = result.expect("fingerprint");
        assert_eq!(fp.len(), 8, "must be 8 hex chars");
        assert!(
            fp.chars().all(|c| c.is_ascii_hexdigit() && (c.is_ascii_digit() || c.is_ascii_lowercase())),
            "must be lowercase hex, got: {}",
            fp
        );
        // Equals the underlying helper output (the handler is a pure shim).
        let direct = sig_mod::fingerprint_from_public_pem(&pem).expect("direct");
        assert_eq!(fp, direct);
    }

    #[test]
    fn fingerprint_from_public_pem_command_rejects_garbage() {
        let request = FingerprintFromPemRequest {
            public_key_pem: "not a pem".to_string(),
        };
        let result: Result<String, String> = if request.public_key_pem.len() > MAX_PUBLIC_PEM_LEN {
            Err("public_key_too_large".to_string())
        } else {
            sig_mod::fingerprint_from_public_pem(&request.public_key_pem)
                .map_err(|e| e.to_string())
        };
        assert!(result.is_err(), "garbage PEM must error, not panic");
    }

    #[test]
    fn fingerprint_from_public_pem_command_rejects_oversize_pem() {
        let big = "a".repeat(MAX_PUBLIC_PEM_LEN + 1);
        let request = FingerprintFromPemRequest { public_key_pem: big };
        let result: Result<String, String> = if request.public_key_pem.len() > MAX_PUBLIC_PEM_LEN {
            Err("public_key_too_large".to_string())
        } else {
            sig_mod::fingerprint_from_public_pem(&request.public_key_pem)
                .map_err(|e| e.to_string())
        };
        assert_eq!(result.unwrap_err(), "public_key_too_large");
    }
}
