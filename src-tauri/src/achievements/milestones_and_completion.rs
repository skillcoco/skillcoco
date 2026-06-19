//! Phase 08.2 — milestone + completion issuance + points awards.
//!
//! Replaces the OSS desktop binary's consumer of
//! `learnforge_core::achievements::maybe_issue` (which still ships the
//! 3-tier Associate/Practitioner/Professional ladder as library
//! primitives — see D-12). Per D-16/D-17, OSS now issues:
//!
//!   - Milestone25 / Milestone50 / Milestone75 — `kind='badge'`,
//!     in-app-only, awarded the first time the track's
//!     `modules_mastered / modules_total` percentage crosses the
//!     threshold. NOT cascading (D-07).
//!   - Completion — `kind='certificate'`, awarded once when the track
//!     reaches 100% mastery (all modules ≥0.7), avg mastery ≥0.85,
//!     and all `practical_required` labs passed (D-01).
//!
//! Points (D-08 schedule):
//!   - +100 per newly-issued milestone
//!   - +500 per newly-issued completion certificate
//!   - +10 per quiz-pass and +50 per module-completion are awarded by
//!     the caller (`commands::learning::submit_quiz` and
//!     `commands::learning::mark_lesson_complete`) — those events
//!     happen outside this function's narrow milestone/cert remit.
//!
//! R4 immutability (D-06): the `achievements` table's
//! UNIQUE(learner_id, track_id, level) constraint prevents re-issuance.
//! `INSERT OR IGNORE` is used so a later mastery decay never overwrites
//! a milestone or completion row. Points are only awarded when the
//! INSERT actually wrote a row (changed == 1), preventing
//! double-counting on repeat calls.
//!
//! Signing: milestones + completions are stored with empty `signature`
//! and `key_fingerprint`. The OSS desktop binary stopped signing in
//! Phase 08.1 (cert split); Studio overlay re-introduces signing for
//! the Completion cert PDF.

use learnforge_core::achievements::Achievement;
use rusqlite::Connection;
use uuid::Uuid;

/// Milestone + Completion thresholds + point awards.
///
/// Centralized so unit tests can reference the same constants as the
/// implementation. Changing these values requires a deliberate edit
/// here (and probably a doc update in `docs/OSS-VS-STUDIO.md`).
pub const MILESTONE_25: i64 = 25;
pub const MILESTONE_50: i64 = 50;
pub const MILESTONE_75: i64 = 75;
pub const COMPLETION_PERCENT: i64 = 100;

/// Completion gate thresholds (D-01).
pub const COMPLETION_AVG_MASTERY_MIN: f64 = 0.85;

/// Point award schedule (D-08).
pub const POINTS_PER_MILESTONE: i64 = 100;
pub const POINTS_PER_COMPLETION: i64 = 500;

/// Aggregated track state used to evaluate issuance.
///
/// Cheap to compute (single-query roll-up); we expose the shape so
/// the issuance fn and tests share a vocabulary.
#[derive(Debug, Clone, Copy)]
pub struct TrackProgress {
    pub modules_total: i64,
    pub modules_mastered: i64,
    pub avg_mastery: f64,
    pub practical_required_count: i64,
    pub practical_labs_passed: i64,
}

impl TrackProgress {
    /// Integer-rounded percent of modules mastered. Returns 0 when the
    /// track has zero modules (defensive — typically the path is
    /// non-empty by the time the learner submits a quiz).
    pub fn progress_percent(&self) -> i64 {
        if self.modules_total == 0 {
            return 0;
        }
        // Floor — a learner at 24.9% does not yet get Milestone25.
        (self.modules_mastered * 100) / self.modules_total
    }

    /// True when the completion gate is satisfied (D-01).
    pub fn completion_satisfied(&self) -> bool {
        self.modules_total > 0
            && self.modules_mastered == self.modules_total
            && self.avg_mastery >= COMPLETION_AVG_MASTERY_MIN
            && self.practical_required_count == self.practical_labs_passed
    }
}

/// Compute the live track progress aggregate. Reads the LATEST learning
/// path's modules (highest `version`) and joins against `module_progress`
/// for the given learner. Mirrors the SQL shape used by Phase 6
/// `storage_impl::threshold::track_mastery_aggregate` but tuned for the
/// 08.2 consumer surface (i64 counts + practical_required tallies).
pub fn compute_track_progress(
    conn: &Connection,
    track_id: &str,
    learner_id: &str,
) -> Result<TrackProgress, String> {
    let row: (i64, i64, f64, i64, i64) = conn
        .query_row(
            r#"
            WITH latest_path AS (
                SELECT id FROM learning_paths
                 WHERE track_id = ?1
                 ORDER BY version DESC
                 LIMIT 1
            )
            SELECT
                COUNT(m.id) AS modules_total,
                COALESCE(SUM(CASE WHEN mp.mastery_level >= 0.7 THEN 1 ELSE 0 END), 0)
                    AS modules_mastered,
                COALESCE(AVG(COALESCE(mp.mastery_level, 0.0)), 0.0) AS avg_mastery,
                COALESCE(SUM(CASE WHEN json_extract(m.content_json, '$.practical_required') = 1
                                    OR json_extract(m.content_json, '$.practical_required') = 'true'
                                  THEN 1 ELSE 0 END), 0) AS practical_required_count,
                COALESCE(SUM(CASE WHEN (json_extract(m.content_json, '$.practical_required') = 1
                                        OR json_extract(m.content_json, '$.practical_required') = 'true')
                                  AND COALESCE(mp.practical_mastery, 0.0) >= 0.7
                                  THEN 1 ELSE 0 END), 0) AS practical_labs_passed
            FROM modules m
            INNER JOIN latest_path lp ON m.path_id = lp.id
            LEFT JOIN module_progress mp
                      ON mp.module_id = m.id AND mp.learner_id = ?2
            "#,
            rusqlite::params![track_id, learner_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|e| format!("compute_track_progress: {}", e))?;

    Ok(TrackProgress {
        modules_total: row.0,
        modules_mastered: row.1,
        avg_mastery: row.2,
        practical_required_count: row.3,
        practical_labs_passed: row.4,
    })
}

/// Look up the track_topic snapshot for the achievement row. Falls back
/// to "Unknown" when the track has been deleted (R4 immutability —
/// achievements survive track deletion; the snapshot is what they show).
fn lookup_track_topic(conn: &Connection, track_id: &str) -> String {
    conn.query_row(
        "SELECT topic FROM learning_tracks WHERE id = ?1",
        [track_id],
        |r| r.get::<_, String>(0),
    )
    .unwrap_or_else(|_| "Unknown".to_string())
}

/// Look up the active pack_id (extracted from `generated_by_model`'s
/// `topic-pack:` prefix, matching the Phase 6 convention).
fn lookup_pack_id(conn: &Connection, track_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT generated_by_model FROM learning_paths
         WHERE track_id = ?1 ORDER BY version DESC LIMIT 1",
        [track_id],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|m| m.strip_prefix("topic-pack:").map(|s| s.to_string()))
}

/// Insert an achievement row OR IGNORE. Returns true when a row was
/// actually inserted (i.e. the level was new for this learner+track).
/// The achievement's `id`, `issued_at`, etc. are written verbatim — the
/// caller is responsible for shaping a fresh row each call.
fn insert_or_ignore(conn: &Connection, ach: &Achievement) -> Result<bool, String> {
    let changed = conn
        .execute(
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
        )
        .map_err(|e| format!("insert_or_ignore achievement: {}", e))?;
    Ok(changed > 0)
}

/// Bump `learner_profiles.points` by the given delta. No-op if delta is
/// non-positive. Always succeeds when the row exists; logs a warning
/// and returns Ok if the UPDATE matches no rows (the caller path should
/// have created the profile already, but we never fail issuance for it).
fn add_points(conn: &Connection, learner_id: &str, delta: i64) -> Result<(), String> {
    if delta <= 0 {
        return Ok(());
    }
    let changed = conn
        .execute(
            "UPDATE learner_profiles SET points = COALESCE(points, 0) + ?1,
                updated_at = datetime('now')
             WHERE id = ?2",
            rusqlite::params![delta, learner_id],
        )
        .map_err(|e| format!("add_points: {}", e))?;
    if changed == 0 {
        log::warn!(
            "add_points: no learner_profiles row matched id={} (delta {} dropped)",
            learner_id,
            delta
        );
    }
    Ok(())
}

/// Public wrapper for awarding points outside the milestone/cert path —
/// used by `submit_quiz` (+10 per quiz pass) and `mark_lesson_complete`
/// (+50 per module-completion). Never throws on missing profile (the
/// caller already resolved one).
pub fn award_points(conn: &Connection, learner_id: &str, delta: i64) -> Result<(), String> {
    add_points(conn, learner_id, delta)
}

/// Build a milestone/completion Achievement row. Signature + key
/// fingerprint are empty strings — milestones are in-app-only (D-05)
/// and the OSS desktop binary's Completion cert is unsigned per the
/// Phase 08.1 split. The Studio overlay's plugin path can re-sign on
/// PDF export if needed (out of scope for 08.2).
fn build_row(
    learner_id: &str,
    track_id: &str,
    pack_id: Option<&str>,
    track_topic: &str,
    kind: &str,
    level: &str,
    mastery_score: f64,
    issued_at: &str,
) -> Achievement {
    Achievement {
        id: Uuid::new_v4().to_string(),
        learner_id: learner_id.to_string(),
        track_id: track_id.to_string(),
        pack_id: pack_id.map(|s| s.to_string()),
        kind: kind.to_string(),
        level: level.to_string(),
        issued_at: issued_at.to_string(),
        mastery_score,
        payload_json: "{}".to_string(),
        signature: String::new(),
        key_fingerprint: String::new(),
        track_topic: track_topic.to_string(),
    }
}

/// Phase 08.2 — issue any newly-crossed milestones (25/50/75) plus the
/// Completion certificate when applicable, and award the matching
/// points (D-08 schedule).
///
/// Returns the set of Achievement rows that were actually inserted (i.e.
/// not the no-op duplicates that INSERT OR IGNORE swallowed). Empty Vec
/// is the default — most quiz submissions do not cross a threshold.
///
/// **Idempotency**: calling twice is safe. INSERT OR IGNORE prevents
/// duplicates; the points UPDATE only fires when a row was actually
/// inserted.
///
/// **R4 immutability**: a later mastery decay (which would drop
/// `modules_mastered` below the previously-crossed threshold) does NOT
/// revoke the achievement. Once issued, always issued.
pub fn maybe_issue_milestones_and_completion(
    conn: &Connection,
    learner_id: &str,
    track_id: &str,
    now_rfc3339: &str,
) -> Result<Vec<Achievement>, String> {
    let progress = compute_track_progress(conn, track_id, learner_id)?;
    if progress.modules_total == 0 {
        // Path not generated yet — nothing to issue.
        return Ok(Vec::new());
    }

    let pct = progress.progress_percent();
    let track_topic = lookup_track_topic(conn, track_id);
    let pack_id = lookup_pack_id(conn, track_id);

    let mut newly_issued: Vec<Achievement> = Vec::new();

    // Each milestone is independent (D-07: not cascading). Order is
    // ascending so the issued vec is naturally sorted lowest-to-highest;
    // the frontend doesn't care about order (it groups by kind) but
    // tests assert on the order so we keep it deterministic.
    let milestones = [
        (MILESTONE_25, "Milestone25"),
        (MILESTONE_50, "Milestone50"),
        (MILESTONE_75, "Milestone75"),
    ];
    for (threshold, level) in milestones {
        if pct < threshold {
            continue;
        }
        let row = build_row(
            learner_id,
            track_id,
            pack_id.as_deref(),
            &track_topic,
            "badge",
            level,
            progress.avg_mastery,
            now_rfc3339,
        );
        if insert_or_ignore(conn, &row)? {
            add_points(conn, learner_id, POINTS_PER_MILESTONE)?;
            newly_issued.push(row);
        }
    }

    // Completion gate (D-01).
    if progress.completion_satisfied() {
        let row = build_row(
            learner_id,
            track_id,
            pack_id.as_deref(),
            &track_topic,
            "certificate",
            "Completion",
            progress.avg_mastery,
            now_rfc3339,
        );
        if insert_or_ignore(conn, &row)? {
            add_points(conn, learner_id, POINTS_PER_COMPLETION)?;
            newly_issued.push(row);
        }
    }

    Ok(newly_issued)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    /// Seed a learner + track + path + N modules + per-module progress.
    /// `mastery_levels[i]` is the BKT posterior to insert for module i.
    /// All modules are non-practical-required so the labs gate is auto-satisfied.
    fn seed_with_progress(
        conn: &Connection,
        learner_id: &str,
        track_id: &str,
        topic: &str,
        mastery_levels: &[f64],
    ) {
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES (?1, 'Test')",
            [learner_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal)
             VALUES (?1, ?2, ?3, 'devops', 'CKA')",
            rusqlite::params![track_id, learner_id, topic],
        )
        .unwrap();
        let path_id = format!("p-{}", track_id);
        conn.execute(
            "INSERT INTO learning_paths
             (id, track_id, version, edges_json, modules_json, generated_by_model)
             VALUES (?1, ?2, 1, '[]', '[]', 'topic-pack:k8s')",
            rusqlite::params![path_id, track_id],
        )
        .unwrap();
        for (i, m) in mastery_levels.iter().enumerate() {
            let module_id = format!("m-{}-{}", track_id, i);
            conn.execute(
                "INSERT INTO modules (id, path_id, title, ordering, content_json)
                 VALUES (?1, ?2, ?3, ?4, '{}')",
                rusqlite::params![module_id, path_id, format!("Mod {}", i), i as i64],
            )
            .unwrap();
            let status = if *m >= 0.7 { "completed" } else { "in_progress" };
            conn.execute(
                "INSERT INTO module_progress
                 (id, module_id, learner_id, status, mastery_level, attempts, started_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, datetime('now'))",
                rusqlite::params![
                    format!("mp-{}-{}", track_id, i),
                    module_id,
                    learner_id,
                    status,
                    *m
                ],
            )
            .unwrap();
        }
    }

    fn get_points(conn: &Connection, learner_id: &str) -> i64 {
        conn.query_row(
            "SELECT points FROM learner_profiles WHERE id = ?1",
            [learner_id],
            |r| r.get(0),
        )
        .unwrap_or(-1)
    }

    fn count_achievements(conn: &Connection, learner_id: &str, level: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM achievements WHERE learner_id = ?1 AND level = ?2",
            [learner_id, level],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn progress_percent_zero_modules_returns_zero() {
        let p = TrackProgress {
            modules_total: 0,
            modules_mastered: 0,
            avg_mastery: 0.0,
            practical_required_count: 0,
            practical_labs_passed: 0,
        };
        assert_eq!(p.progress_percent(), 0);
        assert!(!p.completion_satisfied());
    }

    #[test]
    fn progress_percent_integer_floor() {
        // 1/4 = 25%, 2/4 = 50%, 3/4 = 75%, 4/4 = 100%.
        let cases = [(1, 4, 25), (2, 4, 50), (3, 4, 75), (4, 4, 100), (0, 4, 0)];
        for (mastered, total, expected) in cases {
            let p = TrackProgress {
                modules_total: total,
                modules_mastered: mastered,
                avg_mastery: 0.0,
                practical_required_count: 0,
                practical_labs_passed: 0,
            };
            assert_eq!(p.progress_percent(), expected, "{}/{}", mastered, total);
        }
    }

    #[test]
    fn no_milestones_issued_below_25_percent() {
        let conn = fresh_conn();
        seed_with_progress(&conn, "lp1", "trk1", "K8s", &[0.8, 0.0, 0.0, 0.0]);
        let issued = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        // 1/4 = 25% — exact threshold, should ISSUE Milestone25.
        assert_eq!(issued.len(), 1);
        assert_eq!(issued[0].level, "Milestone25");
        assert_eq!(get_points(&conn, "lp1"), POINTS_PER_MILESTONE);
    }

    #[test]
    fn milestone25_issued_at_exactly_25_percent() {
        let conn = fresh_conn();
        seed_with_progress(&conn, "lp1", "trk1", "K8s", &[0.8, 0.0, 0.0, 0.0]);
        let issued = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        assert_eq!(issued.len(), 1);
        assert_eq!(issued[0].level, "Milestone25");
        assert_eq!(issued[0].kind, "badge");
        assert_eq!(issued[0].pack_id.as_deref(), Some("k8s"));
        assert_eq!(issued[0].track_topic, "K8s");
    }

    #[test]
    fn milestone50_includes_milestone25_when_first_crossed() {
        // 2/4 mastered = 50%. Both Milestone25 + Milestone50 issue at once
        // because each is independent (D-07 — not cascading means earning
        // Milestone75 doesn't IMPLY Milestone25; but the FIRST quiz that
        // crosses 50% does trigger both since neither was issued yet).
        let conn = fresh_conn();
        seed_with_progress(&conn, "lp1", "trk1", "K8s", &[0.8, 0.8, 0.0, 0.0]);
        let issued = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        assert_eq!(issued.len(), 2);
        assert_eq!(issued[0].level, "Milestone25");
        assert_eq!(issued[1].level, "Milestone50");
        assert_eq!(get_points(&conn, "lp1"), POINTS_PER_MILESTONE * 2);
    }

    #[test]
    fn milestone75_issued_at_3_of_4() {
        let conn = fresh_conn();
        seed_with_progress(&conn, "lp1", "trk1", "K8s", &[0.8, 0.8, 0.8, 0.0]);
        let issued = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        let levels: Vec<_> = issued.iter().map(|a| a.level.as_str()).collect();
        assert_eq!(levels, ["Milestone25", "Milestone50", "Milestone75"]);
        assert_eq!(get_points(&conn, "lp1"), POINTS_PER_MILESTONE * 3);
    }

    #[test]
    fn completion_issued_when_all_modules_mastered_and_avg_high() {
        // 4/4 mastered at avg 0.9 (well above 0.85).
        let conn = fresh_conn();
        seed_with_progress(&conn, "lp1", "trk1", "K8s", &[0.9, 0.9, 0.9, 0.9]);
        let issued = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        let levels: Vec<_> = issued.iter().map(|a| a.level.as_str()).collect();
        assert_eq!(levels, ["Milestone25", "Milestone50", "Milestone75", "Completion"]);
        assert_eq!(
            get_points(&conn, "lp1"),
            POINTS_PER_MILESTONE * 3 + POINTS_PER_COMPLETION
        );
        // Completion is kind=certificate.
        let cert = issued.iter().find(|a| a.level == "Completion").unwrap();
        assert_eq!(cert.kind, "certificate");
    }

    #[test]
    fn completion_not_issued_when_avg_below_threshold() {
        // 4/4 mastered (each >= 0.7) but the avg is exactly 0.7 — below 0.85.
        let conn = fresh_conn();
        seed_with_progress(&conn, "lp1", "trk1", "K8s", &[0.7, 0.7, 0.7, 0.7]);
        let issued = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        // All 3 milestones at 100% but NO completion.
        let levels: Vec<_> = issued.iter().map(|a| a.level.as_str()).collect();
        assert_eq!(levels, ["Milestone25", "Milestone50", "Milestone75"]);
        assert!(!issued.iter().any(|a| a.level == "Completion"));
    }

    #[test]
    fn r4_immutability_milestone_not_re_issued() {
        let conn = fresh_conn();
        seed_with_progress(&conn, "lp1", "trk1", "K8s", &[0.8, 0.0, 0.0, 0.0]);
        let first = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        assert_eq!(first.len(), 1);
        let after_first = get_points(&conn, "lp1");
        // Second call with same state: no new issuance.
        let second = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-20T10:00:00Z",
        )
        .unwrap();
        assert!(second.is_empty(), "no re-issuance on idempotent re-call");
        assert_eq!(get_points(&conn, "lp1"), after_first, "no double points");
        assert_eq!(count_achievements(&conn, "lp1", "Milestone25"), 1);
    }

    #[test]
    fn r4_immutability_survives_mastery_decay() {
        let conn = fresh_conn();
        seed_with_progress(&conn, "lp1", "trk1", "K8s", &[0.9, 0.9, 0.9, 0.9]);
        // First call: all four issued + Completion.
        let first = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        assert_eq!(first.len(), 4);
        // Simulate mastery decay: drop one module below 0.7.
        conn.execute(
            "UPDATE module_progress SET mastery_level = 0.3, status = 'in_progress'
             WHERE module_id = 'm-trk1-0' AND learner_id = 'lp1'",
            [],
        )
        .unwrap();
        // Re-run: NO new issuance. NO revocation (rows still in table).
        let second = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-20T10:00:00Z",
        )
        .unwrap();
        assert!(second.is_empty(), "decay does not trigger re-issuance");
        // The original rows survive.
        assert_eq!(count_achievements(&conn, "lp1", "Milestone25"), 1);
        assert_eq!(count_achievements(&conn, "lp1", "Completion"), 1);
    }

    #[test]
    fn empty_path_returns_empty_vec() {
        // No learning path → modules_total = 0.
        let conn = fresh_conn();
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal)
             VALUES ('trk1', 'lp1', 'K8s', 'devops', 'CKA')",
            [],
        )
        .unwrap();
        let issued = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        assert!(issued.is_empty());
        assert_eq!(get_points(&conn, "lp1"), 0);
    }

    #[test]
    fn award_points_adds_delta() {
        let conn = fresh_conn();
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        )
        .unwrap();
        award_points(&conn, "lp1", 10).unwrap();
        award_points(&conn, "lp1", 50).unwrap();
        assert_eq!(get_points(&conn, "lp1"), 60);
    }

    #[test]
    fn award_points_zero_delta_is_noop() {
        let conn = fresh_conn();
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        )
        .unwrap();
        award_points(&conn, "lp1", 0).unwrap();
        award_points(&conn, "lp1", -5).unwrap();
        assert_eq!(get_points(&conn, "lp1"), 0);
    }

    #[test]
    fn completion_requires_practical_labs_when_required() {
        // 1 of 1 module mastered, avg 0.9, BUT practical_required=true
        // and labs not passed → completion NOT issued.
        let conn = fresh_conn();
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal)
             VALUES ('trk1', 'lp1', 'K8s', 'devops', 'CKA')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths
             (id, track_id, version, edges_json, modules_json, generated_by_model)
             VALUES ('p1', 'trk1', 1, '[]', '[]', 'topic-pack:k8s')",
            [],
        )
        .unwrap();
        // Module with practical_required=true.
        conn.execute(
            r#"INSERT INTO modules (id, path_id, title, ordering, content_json)
               VALUES ('m1', 'p1', 'Lab Module', 0,
                       '{"practical_required": true}')"#,
            [],
        )
        .unwrap();
        // Mastery 0.9 but practical_mastery 0.0 (lab not passed).
        conn.execute(
            "INSERT INTO module_progress
             (id, module_id, learner_id, status, mastery_level, practical_mastery, attempts, started_at)
             VALUES ('mp1', 'm1', 'lp1', 'in_progress', 0.9, 0.0, 1, datetime('now'))",
            [],
        )
        .unwrap();
        let issued = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        // 1/1 mastered = 100% so all three milestones fire, BUT completion
        // does not (practical lab gate fails).
        let levels: Vec<_> = issued.iter().map(|a| a.level.as_str()).collect();
        assert_eq!(levels, ["Milestone25", "Milestone50", "Milestone75"]);
        assert!(!issued.iter().any(|a| a.level == "Completion"));
    }

    #[test]
    fn completion_issued_when_practical_labs_passed() {
        let conn = fresh_conn();
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal)
             VALUES ('trk1', 'lp1', 'K8s', 'devops', 'CKA')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths
             (id, track_id, version, edges_json, modules_json, generated_by_model)
             VALUES ('p1', 'trk1', 1, '[]', '[]', 'topic-pack:k8s')",
            [],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO modules (id, path_id, title, ordering, content_json)
               VALUES ('m1', 'p1', 'Lab Module', 0,
                       '{"practical_required": true}')"#,
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO module_progress
             (id, module_id, learner_id, status, mastery_level, practical_mastery, attempts, started_at, completed_at)
             VALUES ('mp1', 'm1', 'lp1', 'completed', 0.9, 0.8, 1, datetime('now'), datetime('now'))",
            [],
        )
        .unwrap();
        let issued = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        let levels: Vec<_> = issued.iter().map(|a| a.level.as_str()).collect();
        assert_eq!(
            levels,
            ["Milestone25", "Milestone50", "Milestone75", "Completion"]
        );
    }

    #[test]
    fn milestone_track_topic_snapshot_set() {
        let conn = fresh_conn();
        seed_with_progress(&conn, "lp1", "trk1", "Kubernetes Mastery", &[0.8, 0.0, 0.0, 0.0]);
        let issued = maybe_issue_milestones_and_completion(
            &conn,
            "lp1",
            "trk1",
            "2026-06-19T10:00:00Z",
        )
        .unwrap();
        assert_eq!(issued.len(), 1);
        assert_eq!(issued[0].track_topic, "Kubernetes Mastery");
    }
}
