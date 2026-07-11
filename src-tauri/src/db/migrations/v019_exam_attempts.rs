//! Migration v019 — exam_attempts table (Wave 0 RED scaffold)
//!
//! Phase 19 (EXAM-01..04): timed/scored exam-run persistence. Every exam
//! attempt (start, submit, timeout) is a history row — never upserted — so
//! unlimited retakes (D-05) each keep their own score/timestamp and the
//! Phase 18 report evidence ledger can read the BEST attempt (D-06)
//! alongside a full history.
//!
//! Wave 0 (19-01) intentionally ships a NO-OP `up()`. The real
//! `CREATE TABLE` body — and the accompanying migration-count assertion
//! bump (18 -> 19) in `db/migrations/mod.rs` — land together in 19-02's
//! commit (Phase 18 lesson: bumping the assertion in a different commit
//! than the table creation breaks the atomic invariant, hit twice).
//!
//! Expected schema (19-02 implements):
//! `exam_attempts(id, learner_id, module_id, block_id, started_at,
//! deadline_at, finished_at, status, score_percent, passed,
//! step_verdicts_json, total_steps)` plus
//! `idx_exam_attempts_learner_module` on (learner_id, module_id).

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 19;
pub const NAME: &str = "exam_attempts";

/// Wave 0 NO-OP. 19-02 replaces this body with the real `CREATE TABLE IF
/// NOT EXISTS exam_attempts (...)` + index, in the SAME commit that bumps
/// the `registered_migrations()` registration and the 18->19 assertion
/// literals in `db/migrations/mod.rs`.
pub fn up(_conn: &Connection) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(crate::db::schema::CREATE_TABLES)
            .expect("baseline tables");
        conn
    }

    /// RED until 19-02 — exam_attempts table must exist with the full
    /// column set + the learner/module index after v019::up() runs
    /// (once it's wired into `registered_migrations()` and its body is
    /// no longer a no-op).
    #[test]
    fn v019_creates_exam_attempts_table_with_required_columns() {
        let conn = fresh_conn();
        up(&conn).expect("19-02 must create exam_attempts table");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='exam_attempts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "19-02 must create exam_attempts table — v019::up() is still a Wave 0 no-op"
        );

        let mut stmt = conn.prepare("PRAGMA table_info(exam_attempts)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|c| c.ok())
            .collect();
        for required in [
            "id",
            "learner_id",
            "module_id",
            "block_id",
            "started_at",
            "deadline_at",
            "finished_at",
            "status",
            "score_percent",
            "passed",
            "step_verdicts_json",
            "total_steps",
        ] {
            assert!(
                cols.iter().any(|c| c == required),
                "19-02 must add exam_attempts column: {} (v019::up() is still a no-op)",
                required
            );
        }

        let idx_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_exam_attempts_learner_module'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            idx_exists, 1,
            "19-02 must create idx_exam_attempts_learner_module"
        );
    }

    /// History, not upsert (D-05 unlimited retakes) — inserting two attempt
    /// rows for the same (learner_id, module_id, block_id) tuple must
    /// produce TWO rows, never a single upserted row. RED until 19-02.
    #[test]
    fn v019_allows_multiple_attempts_per_learner_module_block() {
        let conn = fresh_conn();
        up(&conn).expect("19-02 must create exam_attempts table");

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Tester')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'Kubernetes', 'devops', 'Learn K8s')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path1', 'trk1', '[]', '[]', 'test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('mod1', 'path1', 'Module 1')",
            [],
        )
        .unwrap();

        for i in 0..2 {
            conn.execute(
                "INSERT INTO exam_attempts
                    (id, learner_id, module_id, block_id, started_at, deadline_at,
                     status, score_percent, passed, step_verdicts_json, total_steps)
                 VALUES (?1, 'lp1', 'mod1', 'blk1', datetime('now'), datetime('now', '+45 minutes'),
                    'in_progress', 0.0, 0, '[]', 2)",
                rusqlite::params![format!("ea{}", i)],
            )
            .expect("19-02 must create exam_attempts table before rows can insert");
        }

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM exam_attempts WHERE learner_id = 'lp1' AND module_id = 'mod1' AND block_id = 'blk1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 2,
            "two exam attempts for the same learner+module+block must produce two rows (D-05)"
        );
    }
}
