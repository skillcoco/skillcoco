//! Microlearning daily-challenge selection algorithm (Phase 4 Wave 0 RED shell).
//!
//! Pure selection function over a SQLite connection. No `AppState`, no Mutex —
//! the IPC handler holds the lock and calls this directly. Mirrors the shape of
//! `learning::adaptive::update_mastery` (`adaptive.rs:33`) and
//! `commands::learning::update_streak` (`commands/learning.rs:968`).
//!
//! Wave 0 lands the constants + Candidate struct + `select_daily_challenge`
//! signature with `unimplemented!()` body + 5 ignored RED tests. Plan 02 fills
//! the algorithm and flips the tests' `#[ignore]` to active.

use crate::learning::adaptive::MASTERY_THRESHOLD;
use rusqlite::Connection;

// ── Tuning constants (per RESEARCH §"Selection Algorithm" lines 513-517, Q5 lock) ──

/// BKT decay half-life in days. After this many days since `last_bkt_update_at`,
/// a module's decay score doubles. Plan 02 reads `module_progress.last_bkt_update_at`
/// (added by migration v007) to compute the elapsed delta.
pub const DECAY_HALF_LIFE_DAYS: f64 = 3.0;

/// Recency penalty window. Blocks seen in `daily_challenges` within this many
/// hours are excluded from selection (D-03: "don't re-show what was done in the
/// last 48h").
pub const RECENCY_PENALTY_HOURS: i64 = 48;

/// Weight on the BKT-decay signal.
pub const W_DECAY: f64 = 1.0;

/// Weight on the SR-due signal (slight bias toward review).
pub const W_SR_DUE: f64 = 1.2;

/// Weight on the recency penalty (hard penalty if seen within
/// `RECENCY_PENALTY_HOURS`).
pub const W_RECENCY: f64 = -100.0;

/// Lower bound of the BKT candidate window (D-05 — "struggle zone").
pub const BKT_LOWER: f64 = 0.3;

/// Upper bound of the BKT candidate window. Reuses
/// `learning::adaptive::MASTERY_THRESHOLD = 0.7` as the single source of truth
/// for the mastered/not-mastered boundary.
pub const BKT_UPPER: f64 = MASTERY_THRESHOLD;

/// Result of the selection algorithm. NOT serialized — internal type that the
/// IPC layer translates into a `DailyChallengePayload` for the frontend.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub block_id: String,
    pub module_id: String,
    pub track_id: String,
    pub block_type: String,
    pub score: f64,
}

/// Picks today's micro-challenge block for a learner.
///
/// Returns `Ok(None)` when no candidate exists (empty 0.3–0.7 BKT zone, or
/// every candidate block was already seen within `RECENCY_PENALTY_HOURS`).
///
/// Wave 0 stub: `unimplemented!()` so the type surface compiles. Plan 02 lands
/// the real algorithm. Tests below carry `#[ignore]` so cargo test surfaces
/// the contract without failing the RED gate.
pub fn select_daily_challenge(
    _conn: &Connection,
    _learner_id: &str,
) -> Result<Option<Candidate>, String> {
    unimplemented!("Plan 02 implements: BKT-decay + SR-due blended score over candidate blocks")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Build an in-memory DB with full base schema + migrations applied.
    /// Plan 02 will use this helper to seed fixtures per test.
    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(crate::db::schema::CREATE_TABLES).unwrap();
        crate::db::migrations::apply_migrations(&conn).unwrap();
        conn
    }

    /// MICRO-02 — module in the [0.3, 0.7] BKT zone is selected.
    /// Plan 02 seeds: one learner, one module with mastery_level=0.5, one
    /// section block. Asserts `Some(Candidate { module_id: "mod-1", .. })`.
    #[test]
    #[ignore = "Plan 02 implements"]
    fn selects_block_in_bkt_zone() {
        let conn = fresh_conn();
        // Plan 02 fixture: insert learner, track, path, module, module_progress
        // with mastery_level=0.5 and last_bkt_update_at, and one module_blocks row.
        let result = select_daily_challenge(&conn, "learner-1").expect("ok");
        let cand = result.expect("must return Some(Candidate) when module is in zone");
        assert_eq!(cand.module_id, "mod-1");
    }

    /// MICRO-02 — mastered modules (mastery >= 0.7) are excluded.
    /// Plan 02 seeds mastery=0.8 → returns None (no candidates).
    #[test]
    #[ignore = "Plan 02 implements"]
    fn excludes_mastered_modules() {
        let conn = fresh_conn();
        let result = select_daily_challenge(&conn, "learner-1").expect("ok");
        assert!(
            result.is_none(),
            "mastered modules (>=0.7) must not produce a candidate"
        );
    }

    /// MICRO-02 — modules with no module_progress row (never-seen) are excluded.
    /// Plan 02 seeds a module without any module_progress row → returns None.
    #[test]
    #[ignore = "Plan 02 implements"]
    fn excludes_never_seen_modules() {
        let conn = fresh_conn();
        let result = select_daily_challenge(&conn, "learner-1").expect("ok");
        assert!(
            result.is_none(),
            "never-seen modules (no module_progress row) must not produce a candidate"
        );
    }

    /// MICRO-02 — recency penalty: a block seen in daily_challenges within
    /// `RECENCY_PENALTY_HOURS` is excluded; the same module's other block is
    /// picked instead.
    #[test]
    #[ignore = "Plan 02 implements"]
    fn applies_recency_penalty_within_48h() {
        let conn = fresh_conn();
        // Plan 02 fixture: one module in [0.3, 0.7], two blocks; daily_challenges
        // row 1h ago references block-1. Algorithm must skip block-1 and return
        // block-2.
        let cand = select_daily_challenge(&conn, "learner-1")
            .expect("ok")
            .expect("must return the un-penalized block");
        assert_eq!(cand.block_id, "blk-2");
    }

    /// MICRO-02 — when two candidate modules tie on decay, the one with an
    /// SR-due card scores higher (W_SR_DUE > W_DECAY for that branch).
    #[test]
    #[ignore = "Plan 02 implements"]
    fn prefers_sr_due_modules() {
        let conn = fresh_conn();
        // Plan 02 fixture: module-A has sr_cards with next_review <= now,
        // module-B does not. Both at mastery 0.5 with identical last_bkt_update_at.
        // Algorithm returns module-A's block.
        let cand = select_daily_challenge(&conn, "learner-1")
            .expect("ok")
            .expect("must return the SR-due module's block");
        assert_eq!(cand.module_id, "mod-A");
    }
}
