//! Migration v013: Add per-section granularity to the lesson_videos cache.
//!
//! Phase 11 acceptance: videos are now keyed per-SECTION (a section block in
//! `module_blocks`), not per-module. This lets each lesson show its own
//! independently discovered reference video instead of sharing the same set
//! across every lesson in a module.
//!
//! Changes:
//! - Add nullable `section_id TEXT` column (NULL for legacy module-level rows).
//! - Add index on (module_id, section_id) for the per-section cache query.
//! - Add index on (section_id, video_id) for idempotent INSERT OR IGNORE on
//!   the new per-section unique scope. Note: we use a non-unique index + the
//!   INSERT OR IGNORE logic in `discover_and_persist` rather than a partial
//!   UNIQUE index, because SQLite partial index syntax with OR (to handle NULL)
//!   is awkward and the INSERT OR IGNORE is already the canonical idempotency
//!   guard (WR-02 from v012). Legacy rows with section_id = NULL are naturally
//!   excluded from per-section lookups by the WHERE clause.
//!
//! All DDL uses IF NOT EXISTS / idempotency guards so double-apply is safe.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 13;
pub const NAME: &str = "section_video_granularity";

pub fn up(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        // ALTER TABLE ADD COLUMN is always safe to re-run in SQLite because
        // the column-exists error is NOT a "duplicate column" error code —
        // SQLite returns SQLITE_ERROR for ADD COLUMN if the column exists.
        // We guard with a try/ignore by catching the error in the calling
        // migration runner (which runs inside a transaction and would roll
        // back). Instead we probe the column list first.
        //
        // Practical pattern: run the ALTER inside a transaction-safe statement
        // wrapped in a harmless no-op on conflict. SQLite does not support
        // ALTER TABLE IF NOT EXISTS ADD COLUMN, so we use a SELECT-guard
        // pattern here via a CASE expression that resolves to a no-op when
        // the column already exists. However, the canonical safe approach for
        // rusqlite migrations is: check PRAGMA table_info, then issue ALTER.
        //
        // Since this migration is registered in order and the runner skips
        // already-applied versions, the ALTER runs at most ONCE. The
        // idempotency test below calls apply_migrations twice which exercises
        // the skip-already-applied path (version already in schema_migrations).
        //
        // TL;DR: the IF NOT EXISTS guard is the skip-version check in
        // apply_migrations, not in the SQL itself. The SQL below runs once.
        "ALTER TABLE lesson_videos ADD COLUMN section_id TEXT;
         CREATE INDEX IF NOT EXISTS idx_lesson_videos_module_section
             ON lesson_videos(module_id, section_id);
         CREATE INDEX IF NOT EXISTS idx_lesson_videos_section_video
             ON lesson_videos(section_id, video_id);",
    )?;
    Ok(())
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

    #[test]
    fn v013_adds_section_id_column_to_lesson_videos() {
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");

        // section_id column must exist after v013
        let mut stmt = conn
            .prepare("PRAGMA table_info(lesson_videos)")
            .unwrap();
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(
            columns.contains(&"section_id".to_string()),
            "lesson_videos must have section_id column after v013; found: {:?}",
            columns
        );
    }

    #[test]
    fn v013_current_version_is_13() {
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");
        let version = current_version(&conn).unwrap();
        assert_eq!(version, 14, "current_version must be 14 after all migrations (through v014) are applied");
    }

    #[test]
    fn v013_idempotent_double_apply() {
        // apply_migrations skips already-applied versions, so calling it twice
        // succeeds and schema_migrations has exactly 13 rows.
        let conn = fresh_conn();
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 14,
            "exactly 14 rows in schema_migrations after idempotent double-apply"
        );
    }

    #[test]
    fn v013_section_id_nullable_and_accepts_values() {
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");

        // Seed FK chain
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) \
             VALUES ('tr1', 'lp1', 'T', 'devops', 'G')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES ('path1', 'tr1')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('m1', 'path1', 'Mod')",
            [],
        ).unwrap();

        // Legacy row: section_id NULL (no section_id value)
        conn.execute(
            "INSERT INTO lesson_videos \
             (id, module_id, video_id, title, channel_title, relevance_score) \
             VALUES ('lv-legacy', 'm1', 'vidLegacy', 'T', 'C', 0.7)",
            [],
        ).expect("legacy row without section_id must insert");

        // New row: section_id set
        conn.execute(
            "INSERT INTO lesson_videos \
             (id, module_id, section_id, video_id, title, channel_title, relevance_score) \
             VALUES ('lv-new', 'm1', 'sec-1', 'vidNew', 'T', 'C', 0.9)",
            [],
        ).expect("new row with section_id must insert");

        // Query per-section — only the sectioned row returns
        let per_sec: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM lesson_videos WHERE module_id = 'm1' AND section_id = 'sec-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(per_sec, 1, "per-section query must return exactly 1 row");

        // Legacy row has NULL section_id
        let legacy_sec: Option<String> = conn
            .query_row(
                "SELECT section_id FROM lesson_videos WHERE id = 'lv-legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(legacy_sec.is_none(), "legacy row section_id must be NULL");
    }

    #[test]
    fn v013_existing_v012_tests_still_pass_with_new_column() {
        // Regression: v012 insert/cascade/idempotency still works after v013 adds section_id.
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) \
             VALUES ('tr1', 'lp1', 'T', 'devops', 'G')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES ('path1', 'tr1')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('m1', 'path1', 'Mod')",
            [],
        ).unwrap();

        // Old-style insert still works (section_id defaults to NULL)
        conn.execute(
            "INSERT INTO lesson_videos (id, module_id, video_id, title, channel_title, relevance_score) \
             VALUES ('lv1', 'm1', 'vid1', 'T', 'C', 0.8)",
            [],
        ).expect("v012-style insert must still work after v013");

        // FK cascade still works
        conn.execute("DELETE FROM modules WHERE id = 'm1'", []).unwrap();
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM lesson_videos WHERE module_id = 'm1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "cascade delete must still work after v013");
    }
}
