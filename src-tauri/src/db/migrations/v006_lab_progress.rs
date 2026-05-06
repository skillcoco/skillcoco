//! Migration v006 — lab_progress table + module_progress.practical_mastery
//!
//! Wave 0 stub. 03.1-02 fills in the actual `up()` body. The Wave 0 contract:
//! - `pub const VERSION: i32 = 6` and `pub const NAME: &str = "lab_progress"`
//!   so `db::migrations::mod.rs::registered_migrations()` can wire it.
//! - `up()` returns Err so the existing `test_apply_migrations_*` tests
//!   in `db/migrations/mod.rs::tests` flip from green-(version=5) to
//!   red — Wave 0 deliverable.
//! - `v006_idempotent` test asserts the eventual schema (column + table +
//!   indexes); fails today.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 6;
pub const NAME: &str = "lab_progress";

/// Wave 0 stub — Wave 1 (03.1-02) replaces with the real ALTER TABLE +
/// CREATE TABLE + CREATE INDEX batch from RESEARCH.md § DB Schema Delta.
///
/// Returns `Ok(())` (no-op) so existing migration tests in
/// `mod.rs::tests` go green at the new version=6 assertion without
/// breaking the prior v003/v004/v005 idempotent tests. The `v006_idempotent`
/// test in this file fails RED because the new column/table/indexes don't
/// exist yet.
pub fn up(_conn: &Connection) -> Result<()> {
    Ok(())
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
    ///  - idx_lab_progress_module + idx_lab_progress_block exist
    ///
    /// Wave 0: up() returns Err so apply_migrations fails before reaching
    /// any of these assertions — the test fails. Wave 1 (03.1-02) wires
    /// the real ALTER + CREATE statements and the assertions go green.
    #[test]
    fn v006_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("first apply must succeed once 03.1-02 lands v006");
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
                "lab_progress PK missing: {}",
                required
            );
        }

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
