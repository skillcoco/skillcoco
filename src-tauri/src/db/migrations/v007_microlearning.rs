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
/// Idempotently performs three operations in order:
///
/// 1. **ALTER TABLE module_progress ADD COLUMN last_bkt_update_at TEXT** —
///    guarded with `column_exists` (mirrors v006_lab_progress pattern). Nullable
///    so existing rows remain valid until the BKT update path touches them.
///
/// 2. **CREATE TABLE IF NOT EXISTS learner_streaks** — PK `learner_id`, FK
///    CASCADE to `learner_profiles(id)`. Sibling of `learning_tracks.streak_days`
///    at learner-global scope (D-06).
///
/// 3. **CREATE TABLE IF NOT EXISTS daily_challenges** — composite PK
///    `(learner_id, challenge_date)`, FK CASCADE on all four foreign keys
///    (learner_id, block_id, module_id, track_id) per R5. Indexes:
///    `idx_daily_challenges_block` (supports FK CASCADE traversal),
///    `idx_daily_challenges_recency` (supports the 48h recency lookup the
///    selection algorithm runs).
pub fn up(conn: &Connection) -> Result<()> {
    // 1. module_progress.last_bkt_update_at — idempotent ALTER
    if !column_exists(conn, "module_progress", "last_bkt_update_at")? {
        conn.execute(
            "ALTER TABLE module_progress ADD COLUMN last_bkt_update_at TEXT",
            [],
        )?;
    }

    // 2 + 3. learner_streaks + daily_challenges — both CREATE TABLE IF NOT EXISTS
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS learner_streaks (
            learner_id          TEXT PRIMARY KEY REFERENCES learner_profiles(id) ON DELETE CASCADE,
            streak_days         INTEGER NOT NULL DEFAULT 0,
            last_activity_date  TEXT,
            updated_at          TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS daily_challenges (
            learner_id          TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE CASCADE,
            challenge_date      TEXT NOT NULL,
            block_id            TEXT NOT NULL REFERENCES module_blocks(id) ON DELETE CASCADE,
            module_id           TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
            track_id            TEXT NOT NULL REFERENCES learning_tracks(id) ON DELETE CASCADE,
            block_type          TEXT NOT NULL,
            started_at          TEXT,
            completed_at        TEXT,
            created_at          TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (learner_id, challenge_date)
        );

        CREATE INDEX IF NOT EXISTS idx_daily_challenges_block
            ON daily_challenges(block_id);
        CREATE INDEX IF NOT EXISTS idx_daily_challenges_recency
            ON daily_challenges(learner_id, block_id, created_at);
        "#,
    )?;

    Ok(())
}

/// Check whether `column` exists in `table` by querying PRAGMA table_info.
/// Copied verbatim from `v006_lab_progress.rs:55-64`.
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
