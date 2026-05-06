//! Migration v006 — lab_progress table + module_progress.practical_mastery
//!
//! Adds the practical-mastery dimension and per-lab progress storage required
//! by Phase 03.1 (LAB-08). Mirrors the v005_lesson_completions.rs pattern:
//! idempotent ALTER (via column-existence guard) + CREATE TABLE IF NOT EXISTS
//! + CREATE INDEX IF NOT EXISTS.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 6;
pub const NAME: &str = "lab_progress";

/// Apply the v006 migration.
///
/// 1. ALTER TABLE module_progress ADD COLUMN practical_mastery REAL NOT NULL
///    DEFAULT 0.0 — guarded with a column-existence check so a second apply
///    doesn't fail with "duplicate column name" (mirrors v003_streak_columns
///    pattern).
/// 2. CREATE TABLE IF NOT EXISTS lab_progress with composite PK
///    (learner_id, module_id, block_id) and a metadata_json TEXT NOT NULL
///    DEFAULT '{}' column for the last AI-judge verdict (per
///    03.1-RESEARCH.md Open Question #8).
/// 3. CREATE INDEX IF NOT EXISTS on module_id and block_id for fast aggregate
///    queries (practical_mastery recompute).
pub fn up(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "module_progress", "practical_mastery")? {
        conn.execute(
            "ALTER TABLE module_progress ADD COLUMN practical_mastery REAL NOT NULL DEFAULT 0.0",
            [],
        )?;
    }

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS lab_progress (
            learner_id          TEXT NOT NULL REFERENCES learner_profiles(id),
            module_id           TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
            block_id            TEXT NOT NULL REFERENCES module_blocks(id) ON DELETE CASCADE,
            current_step        INTEGER NOT NULL DEFAULT 0,
            completed_step_ids  TEXT NOT NULL DEFAULT '[]',
            total_steps         INTEGER NOT NULL DEFAULT 0,
            metadata_json       TEXT NOT NULL DEFAULT '{}',
            last_updated        TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (learner_id, module_id, block_id)
        );
        CREATE INDEX IF NOT EXISTS idx_lab_progress_module
            ON lab_progress(module_id);
        CREATE INDEX IF NOT EXISTS idx_lab_progress_block
            ON lab_progress(block_id);
        "#,
    )
}

/// Check whether `column` exists in `table` by querying PRAGMA table_info.
fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for c in cols {
        if c? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    /// LAB-08 — v006 idempotency. After two applies:
    ///  - module_progress.practical_mastery REAL DEFAULT 0.0 column exists
    ///  - lab_progress table exists with PK (learner_id, module_id, block_id)
    ///  - metadata_json TEXT NOT NULL DEFAULT '{}' column exists
    ///  - idx_lab_progress_module + idx_lab_progress_block exist
    #[test]
    fn v006_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        // module_progress.practical_mastery column exists with default 0.0
        let mut stmt = conn
            .prepare("PRAGMA table_info(module_progress)")
            .unwrap();
        let cols: Vec<(String, String, Option<String>)> = stmt
            .query_map([], |r| {
                let name: String = r.get(1)?;
                let ty: String = r.get(2)?;
                let dflt: Option<String> = r.get(4)?;
                Ok((name, ty, dflt))
            })
            .unwrap()
            .filter_map(|c| c.ok())
            .collect();
        let practical = cols
            .iter()
            .find(|(n, _, _)| n == "practical_mastery")
            .expect("module_progress.practical_mastery column must exist after v006");
        assert!(
            practical.1.to_uppercase().contains("REAL"),
            "practical_mastery must be REAL, got {:?}",
            practical
        );
        assert!(
            practical
                .2
                .as_deref()
                .map(|d| d.contains("0"))
                .unwrap_or(false),
            "practical_mastery default must be 0.0, got {:?}",
            practical.2
        );

        // lab_progress table exists
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='lab_progress'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "lab_progress table must exist after v006");

        // PK is (learner_id, module_id, block_id)
        let mut stmt = conn.prepare("PRAGMA table_info(lab_progress)").unwrap();
        let lab_cols: Vec<(i32, String)> = stmt
            .query_map([], |r| {
                let pk: i32 = r.get(5)?;
                let name: String = r.get(1)?;
                Ok((pk, name))
            })
            .unwrap()
            .filter_map(|c| c.ok())
            .collect();

        let pk_cols: Vec<&String> = lab_cols
            .iter()
            .filter(|(pk, _)| *pk > 0)
            .map(|(_, n)| n)
            .collect();
        for required in ["learner_id", "module_id", "block_id"] {
            assert!(
                pk_cols.iter().any(|n| n.as_str() == required),
                "lab_progress PK missing: {}",
                required
            );
        }

        // metadata_json TEXT NOT NULL DEFAULT '{}' column exists
        // (per RESEARCH § Open Question #8 — last AI-judge verdict)
        assert!(
            lab_cols.iter().any(|(_, n)| n == "metadata_json"),
            "lab_progress.metadata_json column must exist after v006"
        );

        // Both indexes exist
        for idx in ["idx_lab_progress_module", "idx_lab_progress_block"] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    [idx],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "{} must exist after v006", idx);
        }
    }
}
