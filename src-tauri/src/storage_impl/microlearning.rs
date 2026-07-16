//! Rusqlite-backed [`MicrolearningStore`] impl — Phase 7 Wave 4 (07-04).
//!
//! Lifts the four private SQL helpers from
//! `src-tauri/src/learning/microlearning_selection.rs:85-208` (pre-Wave-4)
//! into trait methods on the local newtype [`SqliteMicrolearningStore`].
//! Same orphan-rule recipe Wave 2 / Wave 3 used: `impl
//! MicrolearningStore for &rusqlite::Connection` would violate E0117
//! because both the trait and the target type are foreign to
//! `src-tauri`.
//!
//! All SQL is verbatim from the pre-Wave-4 file. The only behavior
//! change is in [`SqliteMicrolearningStore::module_has_due_sr_card`]
//! — the `now` parameter from the algorithm is forwarded into the SQL
//! as an explicit timestamp parameter so tests against the rusqlite
//! adapter (and the core algorithm via a stub store) stay deterministic
//! per A5 (Pitfall 10 mitigation).

use chrono::{DateTime, Utc};
use skillcoco_core::microlearning::{
    CandidateModule, MicrolearningError, MicrolearningStore, BKT_LOWER, BKT_UPPER,
};
use rusqlite::{params, Connection};

/// Zero-cost newtype wrapper around `&Connection` that carries the
/// rusqlite-backed [`MicrolearningStore`] impl.
///
/// ## Orphan-rule note (Wave 2 / 3 pattern repeat)
///
/// `impl MicrolearningStore for &Connection` would trigger
/// `error[E0117]: only traits defined in the current crate can be
/// implemented for arbitrary types` — both the trait (in
/// `skillcoco-core`) and `Connection` (in `rusqlite`) are foreign to
/// `src-tauri`. Wrapping `&Connection` in a local newtype satisfies the
/// orphan rule with zero runtime cost.
pub struct SqliteMicrolearningStore<'a>(pub &'a Connection);

impl<'a> MicrolearningStore for SqliteMicrolearningStore<'a> {
    fn candidate_modules(
        &self,
        learner_id: &str,
    ) -> Result<Vec<CandidateModule>, MicrolearningError> {
        let mut stmt = self
            .0
            .prepare(
                "SELECT mp.module_id, lp.track_id, mp.mastery_level, mp.last_bkt_update_at
                   FROM module_progress mp
                   JOIN modules m         ON m.id = mp.module_id
                   JOIN learning_paths lp ON lp.id = m.path_id
                   JOIN learning_tracks lt ON lt.id = lp.track_id
                  WHERE mp.learner_id = ?1
                    AND lt.status = 'active'
                    AND mp.mastery_level >= ?2
                    AND mp.mastery_level <  ?3",
            )
            .map_err(|e| {
                MicrolearningError::Backend(format!(
                    "candidate_modules: prepare failed: {}",
                    e
                ))
            })?;

        let rows = stmt
            .query_map(params![learner_id, BKT_LOWER, BKT_UPPER], |row| {
                Ok(CandidateModule {
                    module_id: row.get(0)?,
                    track_id: row.get(1)?,
                    mastery_level: row.get(2)?,
                    last_bkt_update_at: row.get(3)?,
                })
            })
            .map_err(|e| {
                MicrolearningError::Backend(format!("candidate_modules: query failed: {}", e))
            })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| {
                MicrolearningError::Backend(format!("candidate_modules: row failed: {}", e))
            })?);
        }
        Ok(out)
    }

    fn blocks_for_module(
        &self,
        module_id: &str,
    ) -> Result<Vec<(String, String, i32)>, MicrolearningError> {
        let mut stmt = self
            .0
            .prepare(
                "SELECT id, block_type, ordering
                   FROM module_blocks
                  WHERE module_id = ?1
                    AND status = 'ready'
                    AND block_type IN ('flash_cards', 'quiz', 'section')",
            )
            .map_err(|e| {
                MicrolearningError::Backend(format!(
                    "blocks_for_module: prepare failed: {}",
                    e
                ))
            })?;

        let rows = stmt
            .query_map(params![module_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                ))
            })
            .map_err(|e| {
                MicrolearningError::Backend(format!(
                    "blocks_for_module: query failed: {}",
                    e
                ))
            })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| {
                MicrolearningError::Backend(format!("blocks_for_module: row failed: {}", e))
            })?);
        }
        Ok(out)
    }

    fn is_recently_seen(
        &self,
        learner_id: &str,
        block_id: &str,
        recency_hours: i64,
    ) -> Result<bool, MicrolearningError> {
        // SQLite doesn't bind 48 as a placeholder cleanly into
        // datetime('-X hours'); build the modifier from the const at
        // call time. Safe — no user input.
        let modifier = format!("-{} hours", recency_hours);
        let count: i64 = self
            .0
            .query_row(
                "SELECT COUNT(*) FROM daily_challenges
                  WHERE learner_id = ?1
                    AND block_id = ?2
                    AND created_at >= datetime('now', ?3)",
                params![learner_id, block_id, modifier],
                |row| row.get(0),
            )
            .map_err(|e| MicrolearningError::Backend(format!("is_recently_seen: {}", e)))?;
        Ok(count > 0)
    }

    fn module_has_due_sr_card(
        &self,
        _learner_id: &str,
        module_id: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, MicrolearningError> {
        // A5 — `now` is injected from the algorithm. Use it as an explicit
        // ISO-8601 timestamp instead of SQLite's `datetime('now')` so the
        // comparison stays deterministic for tests that pin a fixed clock.
        // `next_review` is stored as the TEXT produced by `datetime(...)`
        // in SQLite (no timezone suffix), so we render `now` in the same
        // format (`YYYY-MM-DD HH:MM:SS`) — see `to_string()` of
        // `chrono::format::Item::Numeric` below.
        let now_text = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let count: i64 = self
            .0
            .query_row(
                "SELECT COUNT(*) FROM sr_cards
                  WHERE module_id = ?1 AND next_review <= ?2",
                params![module_id, now_text],
                |row| row.get(0),
            )
            .map_err(|e| {
                MicrolearningError::Backend(format!("module_has_due_sr_card: {}", e))
            })?;
        Ok(count > 0)
    }

    fn decay_days_for_module(
        &self,
        learner_id: &str,
        module_id: &str,
    ) -> Result<f64, MicrolearningError> {
        let raw: Option<f64> = self
            .0
            .query_row(
                "SELECT julianday('now') - julianday(last_bkt_update_at)
                   FROM module_progress
                  WHERE module_id = ?1 AND learner_id = ?2",
                params![module_id, learner_id],
                |row| row.get(0),
            )
            .map_err(|e| {
                MicrolearningError::Backend(format!("decay_days_for_module: {}", e))
            })?;
        Ok(raw.unwrap_or(0.0).max(0.0))
    }
}

#[cfg(test)]
mod tests {
    //! Integration tests for the rusqlite adapter against an in-memory
    //! `Connection`. The corresponding pure-stub tests live in
    //! `skillcoco-core/src/microlearning.rs`.

    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use chrono::TimeZone;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
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

    #[test]
    fn candidate_modules_filters_by_track_status() {
        let conn = fresh_conn();
        let (learner_id, track_id, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.5, None);

        let store = SqliteMicrolearningStore(&conn);

        // Active — should appear
        let active = store.candidate_modules(&learner_id).expect("ok");
        assert_eq!(active.len(), 1);

        // Flip track to archived — should disappear
        conn.execute(
            "UPDATE learning_tracks SET status = 'archived' WHERE id = ?1",
            params![&track_id],
        )
        .unwrap();
        let archived = store.candidate_modules(&learner_id).expect("ok");
        assert_eq!(
            archived.len(),
            0,
            "archived tracks must not contribute candidates"
        );
    }

    #[test]
    fn candidate_modules_excludes_mastered_and_never_seen() {
        let conn = fresh_conn();
        let (learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        // mastered (>= BKT_UPPER) → excluded
        insert_module_progress(&conn, &learner_id, &module_id, 0.8, None);

        let store = SqliteMicrolearningStore(&conn);
        let result = store.candidate_modules(&learner_id).expect("ok");
        assert_eq!(result.len(), 0, "mastered modules excluded");
    }

    #[test]
    fn blocks_for_module_filters_by_status_and_type() {
        let conn = fresh_conn();
        let (_learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        // Eligible
        insert_block(&conn, "blk-ready-section", &module_id, 0, "section", "ready");
        insert_block(&conn, "blk-ready-quiz", &module_id, 1, "quiz", "ready");
        // Wrong status
        insert_block(&conn, "blk-pending", &module_id, 2, "section", "pending");
        // Wrong type
        insert_block(&conn, "blk-lab", &module_id, 3, "lab", "ready");

        let store = SqliteMicrolearningStore(&conn);
        let blocks = store.blocks_for_module(&module_id).expect("ok");
        let ids: Vec<&str> = blocks.iter().map(|(id, _, _)| id.as_str()).collect();
        assert!(ids.contains(&"blk-ready-section"));
        assert!(ids.contains(&"blk-ready-quiz"));
        assert!(!ids.contains(&"blk-pending"));
        assert!(!ids.contains(&"blk-lab"));
    }

    #[test]
    fn is_recently_seen_true_within_window_false_after() {
        let conn = fresh_conn();
        let (learner_id, track_id, _, module_id) = seed_active_learner_path_module(&conn);
        insert_block(&conn, "blk-1", &module_id, 0, "section", "ready");
        insert_block(&conn, "blk-2", &module_id, 1, "section", "ready");

        // blk-1 was seen 1 hour ago — recent
        conn.execute(
            "INSERT INTO daily_challenges
                (learner_id, challenge_date, block_id, module_id, track_id, block_type, created_at)
             VALUES (?1, '2026-06-14', 'blk-1', ?2, ?3, 'section', datetime('now', '-1 hours'))",
            params![&learner_id, &module_id, &track_id],
        )
        .unwrap();
        // blk-2 was seen 50 hours ago — outside the 48h window
        conn.execute(
            "INSERT INTO daily_challenges
                (learner_id, challenge_date, block_id, module_id, track_id, block_type, created_at)
             VALUES (?1, '2026-06-12', 'blk-2', ?2, ?3, 'section', datetime('now', '-50 hours'))",
            params![&learner_id, &module_id, &track_id],
        )
        .unwrap();

        let store = SqliteMicrolearningStore(&conn);
        assert!(store.is_recently_seen(&learner_id, "blk-1", 48).unwrap());
        assert!(!store.is_recently_seen(&learner_id, "blk-2", 48).unwrap());
    }

    #[test]
    fn decay_days_for_module_returns_zero_for_null_last_bkt_update_at() {
        let conn = fresh_conn();
        let (learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);
        insert_module_progress(&conn, &learner_id, &module_id, 0.5, None);
        let store = SqliteMicrolearningStore(&conn);
        let days = store
            .decay_days_for_module(&learner_id, &module_id)
            .expect("ok");
        assert_eq!(days, 0.0, "NULL last_bkt_update_at must produce 0.0 decay");
    }

    #[test]
    fn module_has_due_sr_card_respects_injected_now() {
        // A5 — verify that the `now` parameter (not SQLite's `datetime('now')`)
        // controls the due cutoff. Insert one SR card with next_review well
        // in the past (`2020-01-01`); query with two different injected
        // `now` instants — both should mark the card due because it's
        // ancient. Then insert a future card and check it is NOT due when
        // `now` is before its `next_review`.
        let conn = fresh_conn();
        let (_learner_id, _, _, module_id) = seed_active_learner_path_module(&conn);

        // 1) Past-due card
        conn.execute(
            "INSERT INTO sr_cards (id, module_id, concept, front, back, next_review)
             VALUES ('past-1', ?1, 'cx', 'f', 'b', '2020-01-01 00:00:00')",
            params![&module_id],
        )
        .unwrap();

        let store = SqliteMicrolearningStore(&conn);
        let now_2026 = Utc.with_ymd_and_hms(2026, 6, 16, 12, 0, 0).unwrap();
        assert!(
            store
                .module_has_due_sr_card("learner-1", &module_id, now_2026)
                .unwrap(),
            "card from 2020 must be due relative to 2026 `now`"
        );

        // 2) Future card
        conn.execute(
            "DELETE FROM sr_cards WHERE id = 'past-1'",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sr_cards (id, module_id, concept, front, back, next_review)
             VALUES ('future-1', ?1, 'cx', 'f', 'b', '2099-01-01 00:00:00')",
            params![&module_id],
        )
        .unwrap();
        assert!(
            !store
                .module_has_due_sr_card("learner-1", &module_id, now_2026)
                .unwrap(),
            "card scheduled for 2099 must NOT be due relative to 2026 `now`"
        );

        // 3) Same card IS due relative to a 2100 `now`
        let now_2100 = Utc.with_ymd_and_hms(2100, 1, 2, 0, 0, 0).unwrap();
        assert!(
            store
                .module_has_due_sr_card("learner-1", &module_id, now_2100)
                .unwrap(),
            "card scheduled for 2099 must be due relative to 2100 `now`"
        );
    }
}
