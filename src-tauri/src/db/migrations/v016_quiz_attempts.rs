//! Migration v016 — quiz_attempts table
//!
//! Phase 18 (REP-01): durable per-quiz-attempt persistence. Quiz results were
//! previously returned transiently from `submit_quiz` and never written to
//! disk (RESEARCH Pitfall 2). This migration adds a history table — one row
//! per submission, never upserted — so the evidence ledger (D-06) has quiz
//! history to read from.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 16;
pub const NAME: &str = "quiz_attempts";

/// Apply the v016 migration.
///
/// CREATE TABLE IF NOT EXISTS quiz_attempts with columns:
/// (id, learner_id, module_id, block_id, score_percent, passed, completed_at).
/// Plus an index on (learner_id, module_id) for evidence-ledger reads.
pub fn up(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS quiz_attempts (
            id             TEXT PRIMARY KEY,
            learner_id     TEXT NOT NULL REFERENCES learner_profiles(id),
            module_id      TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
            block_id       TEXT NOT NULL,
            score_percent  REAL NOT NULL,
            passed         INTEGER NOT NULL,
            completed_at   TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_quiz_attempts_learner_module
            ON quiz_attempts(learner_id, module_id);
        "#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    /// REP-01 — v016 idempotency. After two applies:
    ///  - quiz_attempts table exists with columns (learner_id, module_id,
    ///    block_id, score_percent, passed, completed_at)
    ///  - idx_quiz_attempts_learner_module exists
    #[test]
    fn v016_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        // quiz_attempts table exists
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='quiz_attempts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "quiz_attempts table must exist after v016");

        // Required columns exist
        let mut stmt = conn.prepare("PRAGMA table_info(quiz_attempts)").unwrap();
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
            "score_percent",
            "passed",
            "completed_at",
        ] {
            assert!(
                cols.iter().any(|c| c == required),
                "quiz_attempts missing column: {}",
                required
            );
        }

        // Index exists
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_quiz_attempts_learner_module'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "idx_quiz_attempts_learner_module must exist after v016");
    }

    /// History, not upsert: inserting two attempt rows for the same
    /// (learner_id, module_id, block_id) tuple must produce TWO rows.
    #[test]
    fn v016_allows_multiple_attempts_per_quiz() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("apply must succeed");

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Tester')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'Rust', 'programming', 'Learn Rust')",
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
                "INSERT INTO quiz_attempts (id, learner_id, module_id, block_id, score_percent, passed) VALUES (?1, 'lp1', 'mod1', 'blk1', ?2, 1)",
                rusqlite::params![format!("qa{}", i), 100.0],
            )
            .unwrap();
        }

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM quiz_attempts WHERE learner_id = 'lp1' AND module_id = 'mod1' AND block_id = 'blk1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2, "two submissions of the same quiz must produce two rows");
    }
}
