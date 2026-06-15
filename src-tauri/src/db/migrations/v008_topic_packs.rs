//! Migration v008 — topic_packs registry (Phase 5 Wave 1 implementation).
//!
//! Wave 0 (Plan 05-01) landed the migration *registration* + *idempotency test*.
//! Wave 1 (Plan 05-02) fills the `up()` body and adds CHECK-constraint +
//! re-apply idempotency tests.
//!
//! `up()` produces:
//!   - `topic_packs` table with columns:
//!       id TEXT PRIMARY KEY,
//!       title TEXT NOT NULL,
//!       source TEXT NOT NULL CHECK (source IN ('bundled', 'skill')),
//!       enabled INTEGER NOT NULL DEFAULT 1,
//!       pack_version TEXT NOT NULL DEFAULT '1.0',
//!       last_loaded_at TEXT NOT NULL DEFAULT (datetime('now')),
//!       validation_status TEXT NOT NULL CHECK (validation_status IN ('ok', 'warnings', 'errors')),
//!       validation_messages_json TEXT NOT NULL DEFAULT '[]'
//!   - Index `idx_topic_packs_source_enabled (source, enabled)`.
//!
//! Idempotent via `CREATE TABLE IF NOT EXISTS` + `CREATE INDEX IF NOT EXISTS`.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 8;
pub const NAME: &str = "topic_packs";

/// Apply the v008 migration. Idempotent.
pub fn up(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS topic_packs (
            id                       TEXT PRIMARY KEY,
            title                    TEXT NOT NULL,
            source                   TEXT NOT NULL CHECK (source IN ('bundled', 'skill')),
            enabled                  INTEGER NOT NULL DEFAULT 1,
            pack_version             TEXT NOT NULL DEFAULT '1.0',
            last_loaded_at           TEXT NOT NULL DEFAULT (datetime('now')),
            validation_status        TEXT NOT NULL CHECK (validation_status IN ('ok', 'warnings', 'errors')),
            validation_messages_json TEXT NOT NULL DEFAULT '[]'
        );

        CREATE INDEX IF NOT EXISTS idx_topic_packs_source_enabled
            ON topic_packs(source, enabled);
        "#,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    /// PACK-10 — v008 idempotency + post-condition contract.
    ///
    /// Asserts:
    /// 1. `topic_packs` table exists
    /// 2. Columns include all 8 expected names
    /// 3. Index `idx_topic_packs_source_enabled` exists
    /// 4. Re-applying migrations is idempotent (no error)
    #[test]
    fn v008_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        // ── 1. topic_packs table exists ──
        let tp_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='topic_packs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tp_count, 1, "topic_packs table must exist after v008");

        // ── 2. expected columns ──
        let mut stmt = conn.prepare("PRAGMA table_info(topic_packs)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|c| c.ok())
            .collect();
        for required in [
            "id",
            "title",
            "source",
            "enabled",
            "pack_version",
            "last_loaded_at",
            "validation_status",
            "validation_messages_json",
        ] {
            assert!(
                cols.contains(&required.to_string()),
                "v008 must add column `{}` to topic_packs (got {:?})",
                required,
                cols
            );
        }

        // ── 3. idx_topic_packs_source_enabled exists ──
        let idx_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_topic_packs_source_enabled'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_count, 1, "v008 must create idx_topic_packs_source_enabled");
    }

    /// CHECK (source IN ('bundled','skill')) constraint is enforced —
    /// inserting an out-of-domain source must fail.
    #[test]
    fn v008_source_check_constraint() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("apply migrations");

        // Valid insert — bundled
        conn.execute(
            "INSERT INTO topic_packs (id, title, source, validation_status) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["p1", "T1", "bundled", "ok"],
        )
        .expect("valid bundled insert must succeed");

        // Valid insert — skill
        conn.execute(
            "INSERT INTO topic_packs (id, title, source, validation_status) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["p2", "T2", "skill", "ok"],
        )
        .expect("valid skill insert must succeed");

        // Invalid source — must fail
        let res = conn.execute(
            "INSERT INTO topic_packs (id, title, source, validation_status) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["p3", "T3", "bogus", "ok"],
        );
        assert!(
            res.is_err(),
            "CHECK (source IN ('bundled','skill')) must reject 'bogus'"
        );
    }

    /// CHECK (validation_status IN ('ok','warnings','errors')) is enforced.
    #[test]
    fn v008_validation_status_check_constraint() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("apply migrations");

        // Valid
        for vs in ["ok", "warnings", "errors"] {
            conn.execute(
                "INSERT INTO topic_packs (id, title, source, validation_status) \
                 VALUES (?1, ?2, 'bundled', ?3)",
                rusqlite::params![format!("p-{}", vs), "T", vs],
            )
            .unwrap_or_else(|e| panic!("valid status `{}` must insert: {}", vs, e));
        }

        // Invalid validation_status — must fail
        let res = conn.execute(
            "INSERT INTO topic_packs (id, title, source, validation_status) \
             VALUES (?1, ?2, 'bundled', ?3)",
            rusqlite::params!["px", "Tx", "in-progress"],
        );
        assert!(
            res.is_err(),
            "CHECK (validation_status IN ('ok','warnings','errors')) must reject 'in-progress'"
        );
    }
}
