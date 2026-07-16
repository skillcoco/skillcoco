//! Migration v011: Add browse_mode column to learning_tracks.
//!
//! Persists the per-track "linear vs free" browsing preference introduced
//! in Phase 10. Uses PRAGMA table_info to check column existence before
//! ALTER TABLE — SQLite has no IF NOT EXISTS guard for ALTER TABLE.
//!
//! Default is 'linear' (D-01: ship linear as the default mode).
//! Column lives on learning_tracks (per-track grain, D-02 — not
//! preferences_json which is per-learner grain on learner_profiles).

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 11;
pub const NAME: &str = "track_browse_mode";

pub fn up(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "learning_tracks", "browse_mode")? {
        conn.execute(
            "ALTER TABLE learning_tracks ADD COLUMN browse_mode TEXT NOT NULL DEFAULT 'linear'",
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
    fn v011_adds_browse_mode_column_and_version_is_11() {
        // After apply_migrations on a fresh DB, browse_mode column exists AND
        // an INSERT supplying browse_mode='free' succeeds.
        // current_version reflects the latest registered migration (12 after Phase 11 added v012).
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");

        let version = current_version(&conn).unwrap();
        assert!(version >= 11, "current_version must be >= 11 (v011 applied); currently {}", version);

        // Seed a learner profile for FK
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        ).unwrap();

        // Insert row with explicit browse_mode='free'
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal, browse_mode) \
             VALUES ('t1', 'lp1', 'Kubernetes', 'devops', 'Pass CKA', 'free')",
            [],
        ).expect("browse_mode column must exist after v011");

        let mode: String = conn.query_row(
            "SELECT browse_mode FROM learning_tracks WHERE id = 't1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(mode, "free");
    }

    #[test]
    fn v011_default_browse_mode_is_linear() {
        // A row inserted WITHOUT browse_mode reads back browse_mode == 'linear'
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        ).unwrap();

        // Insert without specifying browse_mode
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) \
             VALUES ('t2', 'lp1', 'Rust', 'backend', 'Learn Rust')",
            [],
        ).unwrap();

        let mode: String = conn.query_row(
            "SELECT browse_mode FROM learning_tracks WHERE id = 't2'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(mode, "linear", "browse_mode must default to 'linear'");
    }

    #[test]
    fn v011_idempotent_double_apply() {
        // apply_migrations twice succeeds; schema_migrations has exactly 11 rows.
        let conn = fresh_conn();
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM schema_migrations",
            [],
            |row| row.get(0),
        ).unwrap();
        // v012 + v013 + v014 were added in Phase 11; v015 added in Phase 14 (14-06);
        // v016 added in Phase 18 (18-01); v020 added in Phase 15 (15-02).
        // v017/v018 were removed in the Phase 20 reports strip and v019 in the
        // Phase 21 exam strip — schema_migrations has 17 rows after full apply
        // (MAX version stays 20; the gaps are tolerated by the runner).
        assert_eq!(count, 17, "exactly 17 rows in schema_migrations after idempotent double-apply (v017/v018 removed in reports strip, v019 removed in exam strip)");
    }
}
