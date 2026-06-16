//! Phase 6 (Certification) — achievements module entrypoint.
//!
//! Wave 1 (Plan 06-02) fills the threshold logic, signing, and
//! `maybe_issue`. Wave 2 (Plan 06-03) lands IPC handlers + artifact
//! rendering.

pub mod artifacts;
pub mod signing;
pub mod threshold;

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use ed25519_dalek::SigningKey;
use rusqlite::Connection;

/// Persisted achievement row. Mirrors the `achievements` table v009 1:1,
/// with camelCase serde for IPC. D-12 + R4 + R5.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub id: String,
    pub learner_id: String,
    pub track_id: String,
    pub pack_id: Option<String>,
    pub kind: String,
    pub level: String,
    pub issued_at: String,
    pub mastery_score: f64,
    pub payload_json: String,
    pub signature: String,
    pub key_fingerprint: String,
    pub track_topic: String,
}

/// Per-track certification status. Wave 2 (Plan 06-03) populates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackCertifications {
    pub earned_levels: Vec<String>,
    pub next_level: Option<String>,
    pub criteria: String,
}

/// V1 signed-payload contract — see `docs/CERT-PAYLOAD-V1.md`.
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
    /// if/when it switches to JWS-EdDSA.
    pub payload_version: u32,
}

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

/// (learner_display, track_topic snapshot per R4, pack_id snapshot).
/// pack_id is None for Wave 1 (no explicit column on learning_tracks yet —
/// follow-up will wire it).
fn lookup_context(
    conn: &Connection,
    track_id: &str,
    learner_id: &str,
) -> Result<(String, String, Option<String>), AchievementError> {
    let learner_display: String = conn
        .query_row(
            "SELECT COALESCE(display_name, 'Learner') FROM learner_profiles WHERE id = ?1",
            [learner_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "Learner".to_string());
    let track_topic: String = conn
        .query_row(
            "SELECT topic FROM learning_tracks WHERE id = ?1",
            [track_id],
            |r| r.get(0),
        )
        .map_err(|e| AchievementError::Validation(format!("track {} not found: {}", track_id, e)))?;
    Ok((learner_display, track_topic, None))
}

/// Lazy-init the SigningKey in `Mutex<Option<SigningKey>>` (Pattern 2).
/// Returns a fresh clone-via-bytes so callers don't hold the mutex while
/// signing.
fn get_or_load_into_mutex(
    state_key: &Mutex<Option<SigningKey>>,
    key_path: &Path,
) -> Result<SigningKey, AchievementError> {
    {
        let guard = state_key
            .lock()
            .map_err(|_| AchievementError::Validation("signing key mutex poisoned".into()))?;
        if let Some(k) = guard.as_ref() {
            return Ok(SigningKey::from_bytes(&k.to_bytes()));
        }
    }
    let key = signing::get_or_init_key(key_path)?;
    let mut guard = state_key
        .lock()
        .map_err(|_| AchievementError::Validation("signing key mutex poisoned".into()))?;
    if guard.is_none() {
        *guard = Some(SigningKey::from_bytes(&key.to_bytes()));
    }
    Ok(SigningKey::from_bytes(&key.to_bytes()))
}

/// Build CertPayloadV1 + canonical bytes + signature for one (kind, level).
#[allow(clippy::too_many_arguments)]
fn build_signed_achievement(
    key: &SigningKey,
    key_fingerprint: &str,
    learner_display: &str,
    learner_id: &str,
    track_topic: &str,
    track_id: &str,
    pack_id: Option<&str>,
    kind: &str,
    level: &str,
    mastery_score: f64,
    issued_at: &str,
) -> Result<Achievement, AchievementError> {
    let payload = CertPayloadV1 {
        learner: learner_display.to_string(),
        learner_id: learner_id.to_string(),
        track: track_topic.to_string(),
        track_id: track_id.to_string(),
        level: level.to_string(),
        completion_date: issued_at.to_string(),
        mastery_score,
        key_fingerprint: key_fingerprint.to_string(),
        pack_id: pack_id.map(|s| s.to_string()),
        payload_version: 1,
    };
    let canonical = signing::canonical_json_bytes(&payload)?;
    let sig = signing::sign_payload(key, &canonical);
    let sig_hex = hex::encode(sig.to_bytes());
    let payload_json = String::from_utf8(canonical)
        .map_err(|_| AchievementError::Validation("non-utf8 canonical".into()))?;

    Ok(Achievement {
        id: uuid::Uuid::new_v4().to_string(),
        learner_id: learner_id.to_string(),
        track_id: track_id.to_string(),
        pack_id: pack_id.map(|s| s.to_string()),
        kind: kind.to_string(),
        level: level.to_string(),
        issued_at: issued_at.to_string(),
        mastery_score,
        payload_json,
        signature: sig_hex,
        key_fingerprint: key_fingerprint.to_string(),
        track_topic: track_topic.to_string(),
    })
}

/// Issuance entry point — called from BKT path after `became_completed`.
/// Computes the aggregate, determines missing levels, INSERTs OR IGNOREs
/// freshly-signed rows. Idempotent on repeat (empty Vec). R4 immutability:
/// existing rows are never updated.
pub fn maybe_issue(
    conn: &Connection,
    track_id: &str,
    learner_id: &str,
    signing_key: &Mutex<Option<SigningKey>>,
    key_path: &Path,
) -> Result<Vec<Achievement>, AchievementError> {
    let agg = threshold::track_mastery_aggregate(conn, track_id, learner_id)?;
    if agg.modules_total == 0 {
        return Ok(Vec::new());
    }
    let now_met = threshold::levels_met(&agg);
    if now_met.is_empty() {
        return Ok(Vec::new());
    }

    // Fetch existing levels for this (learner, track).
    let already: HashSet<String> = {
        let mut stmt = conn.prepare(
            "SELECT level FROM achievements WHERE learner_id = ?1 AND track_id = ?2",
        )?;
        let rows = stmt
            .query_map([learner_id, track_id], |r| r.get::<_, String>(0))?
            .filter_map(Result::ok)
            .collect();
        rows
    };

    // Determine new (kind, level) tuples to insert.
    let mut to_issue: Vec<(&'static str, &'static str)> = Vec::new();
    for level in &now_met {
        if !already.contains(*level) {
            to_issue.push(("badge", level));
        }
    }
    // Completion certificate when Professional is in now_met AND not
    // already issued.
    if now_met.contains(&"Professional") && !already.contains("Completion") {
        to_issue.push(("certificate", "Completion"));
    }
    if to_issue.is_empty() {
        return Ok(Vec::new());
    }

    let (learner_display, track_topic, pack_id) =
        lookup_context(conn, track_id, learner_id)?;

    let key = get_or_load_into_mutex(signing_key, key_path)?;
    let pub_fp = signing::public_key_fingerprint(&key.verifying_key());

    let issued_at = chrono::Utc::now().to_rfc3339();
    let mut issued: Vec<Achievement> = Vec::new();

    for (kind, level) in to_issue {
        let ach = build_signed_achievement(
            &key,
            &pub_fp,
            &learner_display,
            learner_id,
            &track_topic,
            track_id,
            pack_id.as_deref(),
            kind,
            level,
            agg.avg_mastery,
            &issued_at,
        )?;
        let changed = conn.execute(
            "INSERT OR IGNORE INTO achievements
                (id, learner_id, track_id, pack_id, kind, level, issued_at,
                 mastery_score, payload_json, signature, key_fingerprint, track_topic)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                ach.id,
                ach.learner_id,
                ach.track_id,
                ach.pack_id,
                ach.kind,
                ach.level,
                ach.issued_at,
                ach.mastery_score,
                ach.payload_json,
                ach.signature,
                ach.key_fingerprint,
                ach.track_topic,
            ],
        )?;
        // Only push when the row was actually inserted. INSERT OR IGNORE
        // returns 0 changes when the UNIQUE constraint suppresses the row.
        if changed > 0 {
            issued.push(ach);
        }
    }
    Ok(issued)
}

/// Look up a single Achievement row by id. `Err(Validation)` on miss.
pub fn lookup_achievement_impl(
    conn: &Connection,
    achievement_id: &str,
) -> Result<Achievement, AchievementError> {
    conn.query_row(
        "SELECT id, learner_id, track_id, pack_id, kind, level, issued_at,
                mastery_score, payload_json, signature, key_fingerprint, track_topic
         FROM achievements WHERE id = ?1",
        [achievement_id],
        |r| {
            Ok(Achievement {
                id: r.get(0)?,
                learner_id: r.get(1)?,
                track_id: r.get(2)?,
                pack_id: r.get(3)?,
                kind: r.get(4)?,
                level: r.get(5)?,
                issued_at: r.get(6)?,
                mastery_score: r.get(7)?,
                payload_json: r.get(8)?,
                signature: r.get(9)?,
                key_fingerprint: r.get(10)?,
                track_topic: r.get(11)?,
            })
        },
    )
    .map_err(|e| {
        AchievementError::Validation(format!("achievement {} not found: {}", achievement_id, e))
    })
}

/// List the active learner's achievements in `issued_at DESC` order.
///
/// Single-learner desktop: when `learner_profiles` has a single row (the
/// canonical Phase 6 state), we return every achievement. If multiple
/// profiles exist (multi-tenant future), the caller is expected to resolve
/// the active learner upstream and filter via SQL — this impl just streams
/// the table.
pub fn list_for_learner_impl(conn: &Connection) -> Result<Vec<Achievement>, AchievementError> {
    let mut stmt = conn.prepare(
        "SELECT id, learner_id, track_id, pack_id, kind, level, issued_at,
                mastery_score, payload_json, signature, key_fingerprint, track_topic
         FROM achievements
         ORDER BY issued_at DESC, id ASC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Achievement {
                id: r.get(0)?,
                learner_id: r.get(1)?,
                track_id: r.get(2)?,
                pack_id: r.get(3)?,
                kind: r.get(4)?,
                level: r.get(5)?,
                issued_at: r.get(6)?,
                mastery_score: r.get(7)?,
                payload_json: r.get(8)?,
                signature: r.get(9)?,
                key_fingerprint: r.get(10)?,
                track_topic: r.get(11)?,
            })
        })?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    Ok(rows)
}

/// Per-track earned-levels + next-level criteria.
///
/// Reads only `kind='badge'` rows (the three skill ladder rungs); the
/// completion certificate is independent. `learner_id` is required to
/// filter — Phase 6 is single-learner, but the impl stays multi-tenant-ready.
pub fn get_track_certifications_impl(
    conn: &Connection,
    track_id: &str,
    learner_id: &str,
) -> Result<TrackCertifications, AchievementError> {
    let mut stmt = conn.prepare(
        "SELECT level FROM achievements
         WHERE learner_id = ?1 AND track_id = ?2 AND kind = 'badge'",
    )?;
    let earned_levels: Vec<String> = stmt
        .query_map([learner_id, track_id], |r| r.get::<_, String>(0))?
        .filter_map(Result::ok)
        .collect();

    let has = |name: &str| earned_levels.iter().any(|l| l == name);

    let (next_level, criteria) = if !has("Associate") {
        (Some("Associate".to_string()), "25% of modules mastered".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;
    use std::path::PathBuf;
    use std::sync::Mutex;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    /// modules: (module_id, mastery_level, practical_required, practical_mastery).
    fn seed_track(
        conn: &Connection,
        track_id: &str,
        learner_id: &str,
        topic: &str,
        modules: &[(&str, f64, bool, f64)],
    ) {
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id, display_name) VALUES (?1, 'Alice')",
            [learner_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES (?1, ?2, ?3, 'devops', 'Cert')",
            rusqlite::params![track_id, learner_id, topic],
        ).unwrap();
        let path_id = format!("p-{}", track_id);
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES (?1, ?2, 1, '[]', '[]', 'test')",
            rusqlite::params![path_id, track_id],
        ).unwrap();
        for (i, (mid, ml, pr, pm)) in modules.iter().enumerate() {
            let content_json = if *pr { r#"{"practical_required": true}"# } else { "{}" };
            conn.execute(
                "INSERT INTO modules (id, path_id, title, ordering, content_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![mid, path_id, format!("M{}", i), i as i64, content_json],
            ).unwrap();
            conn.execute(
                "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level, practical_mastery) VALUES (?1, ?2, ?3, 'in_progress', ?4, ?5)",
                rusqlite::params![format!("mp-{}", mid), mid, learner_id, ml, pm],
            ).unwrap();
        }
    }

    fn empty_key_slot() -> Mutex<Option<SigningKey>> { Mutex::new(None) }
    fn fresh_key_dir() -> tempfile::TempDir { tempfile::tempdir().expect("tempdir") }

    /// 1/4 modules above 0.7 = 25% (Associate only).
    const ONE_OF_FOUR: &[(&str, f64, bool, f64)] = &[
        ("m1", 0.75, false, 0.0),
        ("m2", 0.40, false, 0.0),
        ("m3", 0.30, false, 0.0),
        ("m4", 0.30, false, 0.0),
    ];

    #[test]
    fn issues_associate_on_first_module_completion() {
        let conn = fresh_conn();
        seed_track(&conn, "trk1", "lp1", "Kubernetes", ONE_OF_FOUR);
        let key_slot = empty_key_slot();
        let key_dir = fresh_key_dir();
        let issued = maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("issue");
        assert_eq!(issued.len(), 1, "1/4 = 25% — Associate only");
        let a = &issued[0];
        assert_eq!(a.level, "Associate");
        assert_eq!(a.kind, "badge");
        assert_eq!(a.signature.len(), 128, "Ed25519 sig hex = 64 bytes * 2");
        assert_eq!(a.key_fingerprint.len(), 8);
        assert_eq!(a.track_topic, "Kubernetes");
    }

    #[test]
    fn idempotent_on_second_call() {
        let conn = fresh_conn();
        seed_track(&conn, "trk1", "lp1", "Kubernetes", ONE_OF_FOUR);
        let key_slot = empty_key_slot();
        let key_dir = fresh_key_dir();
        let first = maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("first");
        let second = maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("second");
        assert_eq!(first.len(), 1);
        assert!(second.is_empty(), "second call must be a no-op");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM achievements WHERE learner_id='lp1' AND track_id='trk1'",
            [], |r| r.get(0)
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn professional_emits_badge_and_certificate() {
        let conn = fresh_conn();
        // All 4 mastered; avg 0.9125 >= 0.85; m2 lab passed (0.80).
        seed_track(&conn, "trk1", "lp1", "Kubernetes", &[
            ("m1", 0.90, false, 0.0),
            ("m2", 0.88, true, 0.80),
            ("m3", 0.95, false, 0.0),
            ("m4", 0.92, false, 0.0),
        ]);
        let issued = maybe_issue(&conn, "trk1", "lp1", &empty_key_slot(), fresh_key_dir().path()).expect("issue");
        // 4 rows: 3 badges + 1 certificate.
        assert_eq!(issued.len(), 4);
        let kinds: std::collections::HashSet<(String, String)> =
            issued.iter().map(|a| (a.kind.clone(), a.level.clone())).collect();
        assert!(kinds.contains(&("badge".into(), "Associate".into())));
        assert!(kinds.contains(&("badge".into(), "Practitioner".into())));
        assert!(kinds.contains(&("badge".into(), "Professional".into())));
        assert!(kinds.contains(&("certificate".into(), "Completion".into())));
    }

    #[test]
    fn professional_blocked_when_practical_lab_missing() {
        let conn = fresh_conn();
        // m2 practical_required=true but practical_mastery 0.3 < 0.7.
        seed_track(&conn, "trk1", "lp1", "Kubernetes", &[
            ("m1", 0.90, false, 0.0),
            ("m2", 0.88, true, 0.30),
            ("m3", 0.95, false, 0.0),
            ("m4", 0.92, false, 0.0),
        ]);
        let issued = maybe_issue(&conn, "trk1", "lp1", &empty_key_slot(), fresh_key_dir().path()).expect("issue");
        let levels: Vec<&str> = issued.iter().map(|a| a.level.as_str()).collect();
        assert!(levels.contains(&"Associate") && levels.contains(&"Practitioner"));
        assert!(!levels.contains(&"Professional"), "missing labs blocks Professional");
        assert!(!levels.contains(&"Completion"));
    }

    /// R4 / D-04 — mastery decay must NOT re-issue or mutate prior rows.
    #[test]
    fn immutability_under_decay() {
        let conn = fresh_conn();
        seed_track(&conn, "trk1", "lp1", "Kubernetes", ONE_OF_FOUR);
        let key_slot = empty_key_slot();
        let key_dir = fresh_key_dir();
        let first = maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("issue");
        assert_eq!(first.len(), 1);
        let original = first[0].clone();

        // Decay m1 below 0.7.
        conn.execute(
            "UPDATE module_progress SET mastery_level = 0.40 WHERE module_id = 'm1' AND learner_id = 'lp1'",
            [],
        ).unwrap();

        let second = maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("re-issue");
        assert!(second.is_empty(), "decay must NOT trigger re-issuance");

        let row_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM achievements WHERE learner_id='lp1' AND track_id='trk1' AND level='Associate'",
            [], |r| r.get(0)
        ).unwrap();
        assert_eq!(row_count, 1, "row must remain after decay");

        let (sig_db, score_db): (String, f64) = conn.query_row(
            "SELECT signature, mastery_score FROM achievements WHERE id = ?1",
            [&original.id], |r| Ok((r.get(0)?, r.get(1)?))
        ).unwrap();
        assert_eq!(sig_db, original.signature, "signature unchanged");
        assert!((score_db - original.mastery_score).abs() < 1e-9, "snapshot unchanged");
    }

    /// Canonical bytes of payload_json verify against the on-disk public key.
    #[test]
    fn signed_payload_round_trips() {
        let conn = fresh_conn();
        seed_track(&conn, "trk1", "lp1", "Kubernetes", ONE_OF_FOUR);
        let key_slot = empty_key_slot();
        let key_dir = fresh_key_dir();
        let issued = maybe_issue(&conn, "trk1", "lp1", &key_slot, key_dir.path()).expect("issue");
        assert_eq!(issued.len(), 1);
        let public_pem = signing::read_public_pem(key_dir.path()).expect("public pem");
        let a = &issued[0];
        assert!(signing::verify_payload(&public_pem, a.payload_json.as_bytes(), &a.signature));
        let mut tampered = a.payload_json.clone();
        tampered.push(' ');
        assert!(!signing::verify_payload(&public_pem, tampered.as_bytes(), &a.signature));
    }

    #[test]
    fn empty_track_returns_no_issuance() {
        let conn = fresh_conn();
        conn.execute("INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'T')", []).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk-empty', 'lp1', 'X', 'd', 'g')",
            []
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES ('p-empty', 'trk-empty', 1, '[]', '[]', 'test')",
            []
        ).unwrap();
        let issued = maybe_issue(&conn, "trk-empty", "lp1", &empty_key_slot(), fresh_key_dir().path()).expect("issue");
        assert!(issued.is_empty(), "no modules = no achievements");
    }

    /// Wave 0 RED test — turns GREEN here.
    #[test]
    fn maybe_issue_idempotent() {
        let conn = fresh_conn();
        seed_track(&conn, "trk-x", "lnr-x", "Topic", ONE_OF_FOUR);
        let key_slot: Mutex<Option<SigningKey>> = Mutex::new(None);
        let key_dir = fresh_key_dir();
        let _key_path: PathBuf = key_dir.path().to_path_buf();
        let first = maybe_issue(&conn, "trk-x", "lnr-x", &key_slot, key_dir.path()).expect("first");
        let second = maybe_issue(&conn, "trk-x", "lnr-x", &key_slot, key_dir.path()).expect("second");
        assert!(!first.is_empty());
        assert!(second.is_empty(), "second call must yield no new achievements");
    }
}
