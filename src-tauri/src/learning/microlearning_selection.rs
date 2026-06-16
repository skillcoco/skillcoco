//! Transitional shim — Phase 7 Wave 4 (07-04) moved the
//! microlearning daily-challenge selection algorithm to
//! `learnforge_core::microlearning`. The rusqlite-backed
//! `MicrolearningStore` impl lives at
//! `crate::storage_impl::microlearning::SqliteMicrolearningStore`.
//!
//! This file remains so existing callers
//! (`commands/microlearning.rs::select_daily_challenge`) continue to
//! compile and link unchanged. It also keeps the legacy
//! `Result<_, String>` error type so command handlers' `?` operators
//! don't need a rewrite — Wave 10 will switch callers to the
//! typed [`learnforge_core::microlearning::MicrolearningError`].
//!
//! No `#[deprecated]` (R5 / Pitfall 6 — rustc silently ignores it on
//! `pub use`). Wave 10 grep-and-rewrite is the eventual cleanup.

use chrono::Utc;
use rusqlite::Connection;

// Re-export the pure types so call sites can keep referring to
// `learning::microlearning_selection::Candidate` etc. unchanged.
pub use learnforge_core::microlearning::{
    Candidate, CandidateModule, MicrolearningError, MicrolearningStore, BKT_LOWER, BKT_UPPER,
    DECAY_DAYS_CAP_MULT, DECAY_HALF_LIFE_DAYS, RECENCY_PENALTY_HOURS, W_DECAY, W_RECENCY,
    W_SR_DUE,
};

use crate::storage_impl::microlearning::SqliteMicrolearningStore;

/// Legacy signature preserved for existing call sites in
/// `commands/microlearning.rs`. Internally constructs a
/// [`SqliteMicrolearningStore`] adapter, supplies `chrono::Utc::now()`
/// at the call site (A5 — production clock injection happens here, not
/// inside the algorithm), and downgrades the typed
/// [`MicrolearningError`] to a `String` so existing `?` operators in
/// the callers still type-check.
///
/// Wave 10 (`07-10-PLAN.md`) is the moment to update callers to invoke
/// `learnforge_core::microlearning::select_daily_challenge` directly
/// with their own clock + typed error.
pub fn select_daily_challenge(
    conn: &Connection,
    learner_id: &str,
) -> Result<Option<Candidate>, String> {
    let store = SqliteMicrolearningStore(conn);
    learnforge_core::microlearning::select_daily_challenge(&store, learner_id, Utc::now())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    //! Cross-crate integration tests — exercise the full
    //! `&Connection` → `SqliteMicrolearningStore` →
    //! `learnforge_core::microlearning::select_daily_challenge` seam.
    //!
    //! Pure-stub tests live in `learnforge-core/src/microlearning.rs`;
    //! per-method rusqlite tests live in
    //! `src-tauri/src/storage_impl/microlearning.rs`. These tests
    //! cover end-to-end behavior through the shim signature the
    //! commands callers rely on.

    use super::*;
    use rusqlite::{params, Connection};

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(crate::db::schema::CREATE_TABLES).unwrap();
        crate::db::migrations::apply_migrations(&conn).unwrap();
        conn
    }

    fn seed_active_learner_path_module(conn: &Connection) -> (String, String, String, String) {
        let learner_id = "learner-1".to_string();
        let track_id = "trk-1".to_string();
        let path_id = "pth-1".to_string();
        let module_id = "mod-1".to_string();

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES (?1, 'Tester')",
            params![&learner_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal, status)
             VALUES (?1, ?2, 'Rust', 'programming', 'Learn Rust', 'active')",
            params![&track_id, &learner_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES (?1, ?2)",
            params![&path_id, &track_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES (?1, ?2, 'Module 1')",
            params![&module_id, &path_id],
        )
        .unwrap();
        (learner_id, track_id, path_id, module_id)
    }

    fn insert_module_progress(
        conn: &Connection,
        learner_id: &str,
        module_id: &str,
        mastery: f64,
        last_bkt_update_at: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO module_progress
                (id, module_id, learner_id, status, mastery_level, last_bkt_update_at)
             VALUES (?1, ?2, ?3, 'in_progress', ?4, ?5)",
            params![
                uuid::Uuid::new_v4().to_string(),
                module_id,
                learner_id,
                mastery,
                last_bkt_update_at,
            ],
        )
        .unwrap();
    }

    fn insert_block(
        conn: &Connection,
        block_id: &str,
        module_id: &str,
        ordering: i32,
        block_type: &str,
        status: &str,
    ) {
        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![block_id, module_id, ordering, block_type, status],
        )
        .unwrap();
    }

    /// MICRO-02 — module in the [0.3, 0.7) BKT zone is selected.
    #[test]
    fn selects_block_in_bkt_zone_via_shim() {
        let conn = fresh_conn();
        let (learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.5, None);
        insert_block(&conn, "blk-1", &module_id, 0, "section", "ready");

        let result = select_daily_challenge(&conn, &learner_id).expect("ok");
        let cand = result.expect("must return Some(Candidate) when module is in zone");
        assert_eq!(cand.module_id, module_id);
        assert_eq!(cand.block_id, "blk-1");
    }

    /// MICRO-02 — mastered modules (mastery >= 0.7) are excluded.
    #[test]
    fn excludes_mastered_modules_via_shim() {
        let conn = fresh_conn();
        let (learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.8, None);
        insert_block(&conn, "blk-1", &module_id, 0, "section", "ready");

        let result = select_daily_challenge(&conn, &learner_id).expect("ok");
        assert!(
            result.is_none(),
            "mastered modules (>=0.7) must not produce a candidate"
        );
    }

    /// MICRO-02 — modules with no module_progress row (never-seen) are excluded.
    #[test]
    fn excludes_never_seen_modules_via_shim() {
        let conn = fresh_conn();
        let (learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        // NO insert_module_progress — the JOIN with module_progress eliminates this row.
        insert_block(&conn, "blk-1", &module_id, 0, "section", "ready");

        let result = select_daily_challenge(&conn, &learner_id).expect("ok");
        assert!(
            result.is_none(),
            "never-seen modules (no module_progress row) must not produce a candidate"
        );
    }

    /// MICRO-02 — recency penalty: blk-1 seen 1h ago, blk-2 picked instead.
    #[test]
    fn applies_recency_penalty_via_shim() {
        let conn = fresh_conn();
        let (learner_id, track_id, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.5, None);
        insert_block(&conn, "blk-1", &module_id, 0, "section", "ready");
        insert_block(&conn, "blk-2", &module_id, 1, "section", "ready");

        conn.execute(
            "INSERT INTO daily_challenges
                (learner_id, challenge_date, block_id, module_id, track_id, block_type, created_at)
             VALUES (?1, '2026-06-14', ?2, ?3, ?4, 'section', datetime('now', '-1 hours'))",
            params![&learner_id, "blk-1", &module_id, &track_id],
        )
        .unwrap();

        let cand = select_daily_challenge(&conn, &learner_id)
            .expect("ok")
            .expect("must return the un-penalized block");
        assert_eq!(
            cand.block_id, "blk-2",
            "recently-seen blk-1 must be deprioritized; blk-2 picked instead"
        );
    }

    /// MICRO-02 — SR-due preference signal flows through the shim.
    #[test]
    fn prefers_sr_due_modules_via_shim() {
        let conn = fresh_conn();
        let (learner_id, _, path_id, _) = seed_active_learner_path_module(&conn);

        conn.execute("UPDATE modules SET id = 'mod-A' WHERE id = 'mod-1'", [])
            .unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('mod-B', ?1, 'Module B')",
            params![&path_id],
        )
        .unwrap();

        insert_module_progress(&conn, &learner_id, "mod-A", 0.5, None);
        insert_module_progress(&conn, &learner_id, "mod-B", 0.5, None);
        insert_block(&conn, "blk-A", "mod-A", 0, "section", "ready");
        insert_block(&conn, "blk-B", "mod-B", 0, "section", "ready");

        // mod-A has an SR card due NOW (far in the past)
        conn.execute(
            "INSERT INTO sr_cards (id, module_id, concept, front, back, next_review)
             VALUES ('sr-1', 'mod-A', 'concept-x', 'front', 'back', '2020-01-01 00:00:00')",
            [],
        )
        .unwrap();

        let cand = select_daily_challenge(&conn, &learner_id)
            .expect("ok")
            .expect("must return the SR-due module's block");
        assert_eq!(cand.module_id, "mod-A");
        assert_eq!(cand.block_id, "blk-A");
    }

    /// Determinism — smaller `ordering` wins on score tie.
    #[test]
    fn picks_block_with_lowest_ordering_on_tie_via_shim() {
        let conn = fresh_conn();
        let (learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.5, None);
        insert_block(&conn, "blk-late", &module_id, 5, "section", "ready");
        insert_block(&conn, "blk-early", &module_id, 1, "section", "ready");

        let cand = select_daily_challenge(&conn, &learner_id)
            .expect("ok")
            .expect("ok");
        assert_eq!(
            cand.block_id, "blk-early",
            "tie-break: smaller ordering must win (got {:?})",
            cand
        );
    }
}
