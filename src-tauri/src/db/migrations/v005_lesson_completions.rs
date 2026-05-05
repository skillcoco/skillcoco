use rusqlite::{Connection, Result};

pub const VERSION: i32 = 5;
pub const NAME: &str = "lesson_completions";

pub fn up(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS lesson_completions (
            learner_id   TEXT NOT NULL,
            module_id    TEXT NOT NULL,
            block_id     TEXT NOT NULL,
            completed_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (learner_id, module_id, block_id)
        );
        CREATE INDEX IF NOT EXISTS idx_lesson_completions_module
            ON lesson_completions(module_id);
        "#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    #[test]
    fn v005_lesson_completions_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("first apply");
        apply_migrations(&conn).expect("second apply (idempotent)");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='lesson_completions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "lesson_completions table must exist after v005");

        // Assert PRIMARY KEY composition (learner_id, module_id, block_id)
        let mut stmt = conn.prepare("PRAGMA table_info(lesson_completions)").unwrap();
        let pk_cols: Vec<String> = stmt
            .query_map([], |r| {
                let pk: i32 = r.get(5)?;
                let name: String = r.get(1)?;
                Ok((pk, name))
            })
            .unwrap()
            .filter_map(|c| c.ok())
            .filter(|(pk, _)| *pk > 0)
            .map(|(_, n)| n)
            .collect();
        for required in ["learner_id", "module_id", "block_id"] {
            assert!(
                pk_cols.contains(&required.to_string()),
                "lesson_completions PK missing: {}",
                required
            );
        }

        // Assert idx_lesson_completions_module exists
        let idx_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_lesson_completions_module'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_count, 1, "idx_lesson_completions_module must exist");
    }
}
