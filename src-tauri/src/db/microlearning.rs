//! Daily-challenge + learner-streak DB helpers (Phase 4 Wave 0 RED shell).
//!
//! Mirrors the helper layout used by `db/blocks.rs` and `db/lab_progress.rs`:
//! row structs (internal, no IPC serde derives) + free functions taking
//! `&Connection`. Plan 02 fills the bodies; Wave 0 only lands the typed
//! contract so Plans 03+ can grow against a stable surface.

use rusqlite::Connection;

// ── Row structs (internal — IPC payloads live in commands/microlearning.rs) ──

/// In-memory representation of a `daily_challenges` row. Schema lands in
/// migration v007 (see `db/migrations/v007_microlearning.rs` for the post-
/// condition contract). Composite PK is `(learner_id, challenge_date)`.
#[derive(Debug, Clone)]
pub struct DailyChallengeRow {
    pub learner_id: String,
    pub challenge_date: String,
    pub block_id: String,
    pub module_id: String,
    pub track_id: String,
    pub block_type: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// In-memory representation of a `learner_streaks` row. PK is `learner_id`.
/// Mirrors `learning_tracks.streak_days` semantics from Phase 1 but at
/// learner-global scope (D-06).
#[derive(Debug, Clone)]
pub struct LearnerStreakRow {
    pub learner_id: String,
    pub streak_days: i32,
    pub last_activity_date: Option<String>,
    pub updated_at: String,
}

// ── CRUD helpers (daily_challenges) ──

/// Returns the persisted daily-challenge row for the given learner + date,
/// or `Ok(None)` if no row exists. Plan 02's IPC layer calls this first so
/// the selection algorithm only runs on cache miss (Pitfall 3).
pub fn get_daily_challenge_for_date(
    _conn: &Connection,
    _learner_id: &str,
    _challenge_date: &str,
) -> Result<Option<DailyChallengeRow>, String> {
    todo!("Plan 02: SELECT * FROM daily_challenges WHERE learner_id = ?1 AND challenge_date = ?2")
}

/// Inserts a new daily-challenge row. Plan 02 uses INSERT OR ABORT — the
/// composite PK means a second insert for (learner, date) must fail loudly
/// because the algorithm should never re-run mid-day.
pub fn insert_daily_challenge(
    _conn: &Connection,
    _row: &DailyChallengeRow,
) -> Result<(), String> {
    todo!("Plan 02: INSERT INTO daily_challenges (...) VALUES (...)")
}

/// Marks the daily challenge complete. Returns the ISO `completed_at`
/// timestamp the row was stamped with so the IPC layer can echo it back to
/// the frontend without a second read.
pub fn mark_daily_challenge_completed(
    _conn: &Connection,
    _learner_id: &str,
    _challenge_date: &str,
) -> Result<String, String> {
    todo!("Plan 02: UPDATE daily_challenges SET completed_at = datetime('now') WHERE ... RETURNING completed_at")
}

/// Marks the daily challenge started (sets `started_at` if NULL). Called
/// when the learner opens `/daily/today`. Idempotent — second call is a
/// no-op so re-mounts don't reset `started_at`.
pub fn mark_daily_challenge_started(
    _conn: &Connection,
    _learner_id: &str,
    _challenge_date: &str,
) -> Result<(), String> {
    todo!("Plan 02: UPDATE daily_challenges SET started_at = COALESCE(started_at, datetime('now')) WHERE ...")
}

// ── Streak helpers (learner_streaks) ──

/// Reads the global streak row for a learner. Returns a zeroed
/// `LearnerStreakRow` on a brand-new learner (no row yet) so callers don't
/// need to special-case the first-ever activity.
pub fn get_learner_streak(
    _conn: &Connection,
    _learner_id: &str,
) -> Result<LearnerStreakRow, String> {
    todo!("Plan 02: SELECT * FROM learner_streaks WHERE learner_id = ?1 — return zeroed row if absent")
}

/// Sibling of `commands::learning::update_streak` (`commands/learning.rs:968`)
/// scoped to the global `learner_streaks` table. R3 forbids modifying the
/// per-track helper's signature; this is a copy-adapt, not a generalization.
///
/// Same four-branch semantics: first-ever activity (streak=1), same calendar
/// day (no-op, return current), within 24h different day (streak+=1), gap >
/// 24h (reset to 1). Plan 02 copies the body verbatim with the table swap.
pub fn update_global_streak(
    _conn: &Connection,
    _learner_id: &str,
) -> Result<i32, String> {
    todo!("Plan 02: mirror update_streak body at commands/learning.rs:968 — swap learning_tracks → learner_streaks, track_id → learner_id")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(crate::db::schema::CREATE_TABLES).unwrap();
        crate::db::migrations::apply_migrations(&conn).unwrap();
        conn
    }

    /// MICRO-03 — round-trip insert + read.
    /// Plan 02 fills the helpers; this test seeds the dependent learner /
    /// track / module / module_block rows then inserts a daily_challenge row
    /// and asserts `get_daily_challenge_for_date` returns the same payload.
    #[test]
    #[ignore = "Plan 02 implements"]
    fn insert_and_get_daily_challenge_roundtrip() {
        let conn = fresh_conn();
        let row = DailyChallengeRow {
            learner_id: "learner-1".into(),
            challenge_date: "2026-06-15".into(),
            block_id: "blk-1".into(),
            module_id: "mod-1".into(),
            track_id: "trk-1".into(),
            block_type: "section".into(),
            started_at: None,
            completed_at: None,
        };
        insert_daily_challenge(&conn, &row).expect("insert ok");
        let fetched = get_daily_challenge_for_date(&conn, "learner-1", "2026-06-15")
            .expect("ok")
            .expect("row must exist");
        assert_eq!(fetched.block_id, "blk-1");
        assert_eq!(fetched.completed_at, None);
    }

    /// MICRO-04 — first activity sets streak_days = 1. Mirrors the
    /// `update_streak_first_activity` pattern at
    /// `commands/learning.rs:1598` but at learner-global scope.
    #[test]
    #[ignore = "Plan 02 implements"]
    fn update_global_streak_first_activity_sets_1() {
        let conn = fresh_conn();
        // Plan 02 fixture: insert learner_profile row keyed "learner-1".
        let streak = update_global_streak(&conn, "learner-1").expect("ok");
        assert_eq!(streak, 1, "first activity ever must yield streak=1");
    }

    /// MICRO-04 — same-day idempotency. Mirrors the same-day branch at
    /// `commands/learning.rs:997-1000` (return current_streak without
    /// incrementing). Critical per Pitfall 2.
    #[test]
    #[ignore = "Plan 02 implements"]
    fn update_global_streak_same_day_idempotent() {
        let conn = fresh_conn();
        // Plan 02 fixture: pre-seed learner_streaks row with streak_days=3,
        // last_activity_date = date('now').
        let streak = update_global_streak(&conn, "learner-1").expect("ok");
        assert_eq!(
            streak, 3,
            "same calendar day must be a no-op and return current streak unchanged"
        );
    }
}
