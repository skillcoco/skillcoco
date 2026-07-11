//! Migration v015: Add verified/issuer_name columns to learning_paths.
//!
//! Phase 14 gap-closure (14-06, CR-01, D-14). `import_course_txn` already
//! computes `VerifiedImport { verified, issuer_name }` from the pack-trust
//! chain-of-trust gate, but nothing persisted it — the frontend "verified
//! licensor" badge in TrackView was dead code end-to-end because `get_path`
//! had no column to read back after an app restart.
//!
//! Uses PRAGMA table_info to check column existence before ALTER TABLE —
//! SQLite has no IF NOT EXISTS guard for ALTER TABLE (same pattern as v011).
//!
//! `verified` defaults to 0 (unverified) and `issuer_name` is nullable with
//! no default — legacy/unsigned rows never get backfilled to verified=1
//! (fail-closed: absence of proof shows no badge, T-14-06-02/04).

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 15;
pub const NAME: &str = "learning_path_verified";

pub fn up(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "learning_paths", "verified")? {
        conn.execute(
            "ALTER TABLE learning_paths ADD COLUMN verified INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !column_exists(conn, "learning_paths", "issuer_name")? {
        conn.execute(
            "ALTER TABLE learning_paths ADD COLUMN issuer_name TEXT",
            [],
        )?;
    }
    Ok(())
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
    use crate::db::migrations::{apply_migrations, current_version};
    use crate::db::schema;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        conn
    }

    fn seed_track(conn: &Connection, track_id: &str) {
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) \
             VALUES (?1, 'lp1', 'Kubernetes', 'devops', 'Pass CKA')",
            rusqlite::params![track_id],
        )
        .unwrap();
    }

    #[test]
    fn v015_adds_verified_and_issuer_name_columns_and_version_is_15() {
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");

        let version = current_version(&conn).unwrap();
        assert_eq!(version, 19, "current_version must be 19 after v015..v019 are applied");

        seed_track(&conn, "t1");

        // Insert row with explicit verified=1/issuer_name='X'
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, verified, issuer_name) \
             VALUES ('p1', 't1', 1, 'X')",
            [],
        )
        .expect("verified/issuer_name columns must exist after v015");

        let (verified, issuer_name): (i64, Option<String>) = conn
            .query_row(
                "SELECT verified, issuer_name FROM learning_paths WHERE id = 'p1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(verified, 1);
        assert_eq!(issuer_name.as_deref(), Some("X"));
    }

    #[test]
    fn v015_default_verified_is_zero_and_issuer_name_is_null() {
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");

        seed_track(&conn, "t2");

        // Insert without specifying verified/issuer_name (legacy/unsigned row)
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES ('p2', 't2')",
            [],
        )
        .unwrap();

        let (verified, issuer_name): (i64, Option<String>) = conn
            .query_row(
                "SELECT verified, issuer_name FROM learning_paths WHERE id = 'p2'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(verified, 0, "verified must default to 0");
        assert_eq!(issuer_name, None, "issuer_name must default to NULL");
    }

    #[test]
    fn v015_idempotent_double_apply() {
        let conn = fresh_conn();
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 19, "exactly 19 rows in schema_migrations after idempotent double-apply (v015..v019 added)");
    }
}
