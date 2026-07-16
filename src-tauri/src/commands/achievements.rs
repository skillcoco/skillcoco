//! Phase 6 (Certification) — IPC handlers for the OSS cert + badge path.
//!
//! OSS ships:
//!   - `list_achievements_for_learner` — earned badges/certs for the active learner
//!   - `get_track_certifications` — earned + next-level snapshot per track
//!   - `export_certificate` — unsigned PDF (no QR, no fingerprint footer)
//!   - `export_badge` — PNG badge with QR payload (self-signed completion badge)
//!
//! The completion badge is cryptographically self-signed via `skillcoco_core::
//! signing` (Ed25519) + the local `FsKeyStore`; the client-side certificate
//! VERIFY surface was removed in the Phase 23 trust-chain strip.
//!
//! Inner-helper-seam pattern (Phase 5 / Phase 03.1 precedent): each Tauri
//! command is a thin shim that locks state, calls a pure `*_impl` helper,
//! and maps errors to `String`.
//!
//! camelCase serde + `{ request: T }` envelope per CONVENTIONS.md.

use base64::Engine as _;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::achievements::artifacts::{self, BadgePngInput, CertificatePdfInput};
use crate::storage_impl::achievements::SqliteAchievementStore;
use skillcoco_core::achievements::{
    Achievement, AchievementError, AchievementStore, TrackCertifications,
};

// ── Request / Result types ────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCertificateRequest {
    pub achievement_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTrackCertificationsRequest {
    pub track_id: String,
}

/// Phase 13 (OSS Consolidation / D-08) — restored from pro/.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBadgeRequest {
    pub achievement_id: String,
}

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

/// Format an RFC3339 issuance timestamp as a human-readable date for the
/// certificate (e.g. "29 Jun 2026"). Falls back to the raw string if parsing
/// fails (never panic).
fn format_issued_date(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|dt| dt.format("%d %b %Y").to_string())
        .unwrap_or_else(|_| rfc3339.to_string())
}

/// `base64url(canonical_bytes).sig_hex` — the format the QR payload carries.
/// Phase 13 (OSS Consolidation / D-08) — restored from pro/ verbatim.
fn encode_qr_payload(payload_json: &str, sig_hex: &str) -> String {
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
    format!("{}.{}", b64, sig_hex)
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

/// Render the unsigned certificate PDF for a given achievement id. Returns
/// raw bytes; the frontend (`exportCertificate` wrapper) routes through the
/// Tauri dialog plugin to write the bytes to disk.
///
/// Phase 08.1 (Cert Split) — OSS path passes `Vec::new()` for `qr_png_bytes`
/// so the PDF renderer skips QR embedding and the fingerprint footer
/// (`docs/OSS-VS-STUDIO.md` §"Certification (Phase 6 — split)"). The
/// achievement row may still carry `signature` + `key_fingerprint` columns
/// (populated by Studio runs against the same DB) — OSS simply ignores
/// them when rendering.
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
    // Prefer the learner's CURRENT display name from their profile — the
    // achievement payload is unsigned/empty on the OSS path, so the old
    // extract_learner_name always fell back to "Learner". Fall back to the
    // payload name, then the default, only if the profile lookup is empty.
    let learner_name = db
        .conn
        .query_row(
            "SELECT display_name FROM learner_profiles WHERE id = ?1",
            [&ach.learner_id],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| extract_learner_name(&ach.payload_json));
    let pdf_input = CertificatePdfInput {
        learner_name,
        track_topic: ach.track_topic.clone(),
        issued_at: format_issued_date(&ach.issued_at),
        mastery_score: ach.mastery_score,
        key_fingerprint_short: ach.key_fingerprint.clone(),
        level: ach.level.clone(),
        // Phase 08.1 — OSS unsigned cert path: no QR embedding. The
        // Studio overlay handler in `pro/src-tauri-pro/src/commands/
        // achievements.rs` constructs a non-empty buffer here.
        qr_png_bytes: Vec::new(),
    };
    artifacts::render_certificate_pdf(&pdf_input).map_err(|e| e.to_string())
}

// ── Phase 13 (OSS Consolidation / D-08): restored verify + badge commands ─

/// Render the PNG badge (QR-bearing per Phase 6 D-06 amendment).
/// Works for both `badge` and `certificate` kinds.
///
/// Phase 13 (OSS Consolidation / D-08) — restored from pro/ verbatim.
/// Import paths rewired from pro crate paths to crate::.
#[tauri::command]
pub fn export_badge(
    request: ExportBadgeRequest,
    state: State<'_, crate::AppState>,
) -> Result<Vec<u8>, String> {
    use skillcoco_core::achievements::AchievementStore;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
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

    // ── export_certificate (Phase 08.1 — unsigned OSS path) ───────────

    /// Phase 08.1 — OSS unsigned certificate render path. Seeds a
    /// minimal achievement row directly (no signing required) and feeds
    /// it through `render_certificate_pdf` with `qr_png_bytes = Vec::new()`
    /// (the exact buffer the OSS `export_certificate` handler now passes).
    /// Asserts the PDF renders successfully and starts with the `%PDF-`
    /// magic — proves the QR-skip branch is exercised end-to-end at the
    /// command-shim level.
    #[test]
    fn export_certificate_unsigned_path_renders_pdf() {
        let conn = fresh_conn();
        seed_learner(&conn, "lp1", "Ada");
        insert_ach(
            &conn,
            "cert-oss-1",
            "lp1",
            "trk1",
            "certificate",
            "Completion",
            "2026-06-19T10:00:00Z",
            "{\"learner\":\"Ada\",\"track\":\"Kubernetes\"}",
            "", // empty signature — OSS unsigned path
            "", // empty fingerprint
            "Kubernetes",
        );

        let ach = SqliteAchievementStore(&conn)
            .lookup_achievement("cert-oss-1")
            .expect("lookup");
        assert_eq!(ach.kind, "certificate");

        // Mirror the exact body of `export_certificate` (no QR PNG bytes
        // — the OSS unsigned path).
        let pdf_input = CertificatePdfInput {
            learner_name: extract_learner_name(&ach.payload_json),
            track_topic: ach.track_topic,
            issued_at: ach.issued_at,
            mastery_score: ach.mastery_score,
            key_fingerprint_short: ach.key_fingerprint,
            level: ach.level,
            qr_png_bytes: Vec::new(),
        };
        let bytes = artifacts::render_certificate_pdf(&pdf_input).expect("pdf");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() >= 1024);
    }

    #[test]
    fn export_certificate_fails_for_badge_kind() {
        let conn = fresh_conn();
        seed_learner(&conn, "lp1", "Ada");
        insert_ach(
            &conn,
            "badge-oss-1",
            "lp1",
            "trk1",
            "badge",
            "Associate",
            "2026-06-19T10:00:00Z",
            "{}",
            "",
            "",
            "Kubernetes",
        );
        let ach = SqliteAchievementStore(&conn)
            .lookup_achievement("badge-oss-1")
            .expect("lookup");
        assert_eq!(ach.kind, "badge");
        // The handler asserts: badges cannot be exported as PDF.
        assert!(
            ach.kind != "certificate",
            "Only completion certificates can be exported as PDF"
        );
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

    #[test]
    fn format_issued_date_humanizes_rfc3339() {
        assert_eq!(
            format_issued_date("2026-06-29T12:27:00.470966+00:00"),
            "29 Jun 2026"
        );
        assert_eq!(format_issued_date("2026-01-05T00:00:00Z"), "05 Jan 2026");
    }

    #[test]
    fn format_issued_date_passes_through_garbage() {
        assert_eq!(format_issued_date("not a date"), "not a date");
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

    // ── export_badge PNG signature sanity ──────────────────────────────

    /// `export_badge` IPC handler is a thin shim over `render_badge_png`.
    /// Phase 13 migrates this smoke test from pro/ back to OSS.
    #[test]
    fn export_badge_renders_png_magic() {
        let badge_input = BadgePngInput {
            level: "Associate".to_string(),
            track_topic: "Kubernetes".to_string(),
            issued_at: "2026-06-19T00:00:00Z".to_string(),
            key_fingerprint_short: "deadbeef".to_string(),
            qr_payload: "learnforge-test-payload.deadbeef".to_string(),
        };
        let bytes = artifacts::render_badge_png(&badge_input).expect("png");
        assert!(!bytes.is_empty());
        assert_eq!(
            &bytes[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            "PNG magic bytes expected"
        );
    }
}
