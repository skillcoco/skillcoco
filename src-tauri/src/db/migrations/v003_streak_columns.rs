//! Migration v003: Add streak_days and last_activity_date columns to learning_tracks.
//!
//! Uses PRAGMA table_info to check column existence before ALTER TABLE — SQLite
//! has no IF NOT EXISTS guard for ALTER TABLE, so we check manually.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 3;
pub const NAME: &str = "streak_columns";

pub fn up(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "learning_tracks", "streak_days")? {
        conn.execute(
            "ALTER TABLE learning_tracks ADD COLUMN streak_days INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !column_exists(conn, "learning_tracks", "last_activity_date")? {
        conn.execute(
            "ALTER TABLE learning_tracks ADD COLUMN last_activity_date TEXT",
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
    use crate::db::schema;
    use crate::db::migrations::{apply_migrations, current_version};
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        conn
    }

    #[test]
    fn v003_adds_streak_columns() {
        // After apply_migrations on a fresh DB, learning_tracks must have both columns.
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");

        let version = current_version(&conn).unwrap();
        assert_eq!(version, 3, "current_version must be 3 after v003 is registered");

        // Verify streak_days column exists by inserting a row with it
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal, streak_days) \
             VALUES ('t1', 'lp1', 'Kubernetes', 'devops', 'Pass CKA', 5)",
            [],
        ).expect("streak_days column must exist after v003");

        // Verify last_activity_date column exists
        let streak: i64 = conn.query_row(
            "SELECT streak_days FROM learning_tracks WHERE id = 't1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(streak, 5);
    }

    #[test]
    fn v003_idempotent() {
        // Running apply_migrations twice must not fail with "duplicate column".
        let conn = fresh_conn();
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM schema_migrations",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 3, "exactly 3 rows in schema_migrations after idempotent double-apply");
    }
}
