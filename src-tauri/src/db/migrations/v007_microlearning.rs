//! Migration v007 — microlearning (Phase 4 Wave 0 RED scaffold)
//!
//! Wave 0 lands the migration *registration* + *idempotency test* only.
//! The `up()` body is intentionally a no-op until Plan 02 fills it.
//!
//! When Plan 02 lands, `up()` must produce:
//!   - `module_progress.last_bkt_update_at TEXT NULL` column (idempotent ALTER)
//!   - `daily_challenges` table (composite PK on (learner_id, challenge_date),
//!     `block_id` FK to `module_blocks(id)` with ON DELETE CASCADE per R5)
//!   - `learner_streaks` table (PK `learner_id`)
//!
//! The `v007_idempotent` test asserts the post-condition Plan 02 must satisfy.
//! Today it FAILS — that is the RED contract.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 7;
pub const NAME: &str = "microlearning";

/// Apply the v007 migration.
///
/// Wave 0 (this plan): intentional no-op. The empty body proves the
/// registration framework can carry a v7 entry; the idempotency test
/// below fails on the missing schema until Plan 02 fills the body.
pub fn up(_conn: &Connection) -> Result<()> {
    Ok(())
}

/// Check whether `column` exists in `table` by querying PRAGMA table_info.
/// Copied verbatim from `v006_lab_progress.rs:55-64` — Plan 02 will use it
/// to gate the `module_progress.last_bkt_update_at` ALTER.
#[allow(dead_code)] // unused until Plan 02 fills `up()`
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
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    /// MICRO-01..05 — v007 idempotency contract.
    ///
    /// Asserts the six post-conditions Plan 02's `up()` body must satisfy:
    /// 1. `module_progress.last_bkt_update_at` column exists (TEXT, nullable)
    /// 2. `daily_challenges` table exists
    /// 3. `learner_streaks` table exists
    /// 4. `daily_challenges` PK is composite `(learner_id, challenge_date)`
    /// 5. `learner_streaks` PK is `learner_id`
    /// 6. `daily_challenges.block_id` FK has `ON DELETE CASCADE` (R5)
    ///
    /// Wave 0 RED state: this test FAILS — the up() body is empty so the tables
    /// and column don't exist yet. Plan 02 turns it green.
    #[test]
    fn v007_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        // ── 1. module_progress.last_bkt_update_at column exists ──
        let mut stmt = conn
            .prepare("PRAGMA table_info(module_progress)")
            .unwrap();
        let mp_cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|c| c.ok())
            .collect();
        assert!(
            mp_cols.contains(&"last_bkt_update_at".to_string()),
            "module_progress.last_bkt_update_at column must exist after v007 (got cols: {:?})",
            mp_cols
        );

        // ── 2. daily_challenges table exists ──
        let dc_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='daily_challenges'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dc_count, 1, "daily_challenges table must exist after v007");

        // ── 3. learner_streaks table exists ──
        let ls_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='learner_streaks'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ls_count, 1, "learner_streaks table must exist after v007");

        // ── 4. daily_challenges PK is composite (learner_id, challenge_date) ──
        let mut stmt = conn.prepare("PRAGMA table_info(daily_challenges)").unwrap();
        let dc_pk_cols: Vec<String> = stmt
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
        for required in ["learner_id", "challenge_date"] {
            assert!(
                dc_pk_cols.contains(&required.to_string()),
                "daily_challenges PK must include {} (got {:?})",
                required,
                dc_pk_cols
            );
        }

        // ── 5. learner_streaks PK is learner_id ──
        let mut stmt = conn.prepare("PRAGMA table_info(learner_streaks)").unwrap();
        let ls_pk_cols: Vec<String> = stmt
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
        assert_eq!(
            ls_pk_cols,
            vec!["learner_id".to_string()],
            "learner_streaks PK must be exactly learner_id (got {:?})",
            ls_pk_cols
        );

        // ── 6. daily_challenges.block_id FK has ON DELETE CASCADE (R5) ──
        let mut stmt = conn
            .prepare("PRAGMA foreign_key_list(daily_challenges)")
            .unwrap();
        let fks: Vec<(String, String, String)> = stmt
            .query_map([], |r| {
                // PRAGMA foreign_key_list columns:
                //  0 id, 1 seq, 2 table, 3 from, 4 to, 5 on_update, 6 on_delete, 7 match
                let table: String = r.get(2)?;
                let from: String = r.get(3)?;
                let on_delete: String = r.get(6)?;
                Ok((table, from, on_delete))
            })
            .unwrap()
            .filter_map(|c| c.ok())
            .collect();
        let block_fk = fks
            .iter()
            .find(|(_, from, _)| from == "block_id")
            .expect("daily_challenges.block_id must have an FK declared");
        assert_eq!(
            block_fk.0, "module_blocks",
            "daily_challenges.block_id FK must reference module_blocks"
        );
        assert_eq!(
            block_fk.2.to_uppercase(),
            "CASCADE",
            "daily_challenges.block_id FK must be ON DELETE CASCADE (R5) — got {:?}",
            block_fk
        );
    }
}
