//! Migration v014: Drop the obsolete UNIQUE index on (module_id, video_id).
//!
//! Phase 11 CR-01. Migration v012 created
//! `CREATE UNIQUE INDEX idx_lesson_videos_module_video ON lesson_videos(module_id, video_id)`
//! back when the cache was keyed per-MODULE. v013 re-scoped caching to
//! per-SECTION but never dropped that unique index.
//!
//! With `VIDEO_RESULT_LIMIT = 1` the LLM will routinely pick the SAME
//! high-quality video as the best reference for two different sections of one
//! module. On a FRESH install the still-live UNIQUE `(module_id, video_id)`
//! index makes the second section's `INSERT OR IGNORE` collide and get
//! silently dropped, so that section's cache stays empty forever and the panel
//! vanishes.
//!
//! Migrations are immutable history — we do NOT edit v012's DDL. Instead we
//! drop the index forward here. `DROP INDEX IF EXISTS` is idempotent and safe
//! on BOTH:
//!   - a fresh install where v012 created the index, and
//!   - an already-migrated DB (schema_migrations=13) that predates the index
//!     ever existing (nothing to drop → no-op).

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 14;
pub const NAME: &str = "drop_module_video_unique_index";

pub fn up(conn: &Connection) -> Result<()> {
    // Idempotent + safe whether or not the index exists.
    conn.execute_batch("DROP INDEX IF EXISTS idx_lesson_videos_module_video;")?;
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

    /// Returns true if the named index exists in the DB.
    fn index_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
            rusqlite::params![name],
            |row| row.get::<_, i32>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false)
    }

    fn seed_module(conn: &Connection) -> String {
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) \
             VALUES ('tr1', 'lp1', 'T', 'devops', 'G')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES ('path1', 'tr1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('m1', 'path1', 'Mod')",
            [],
        )
        .unwrap();
        "m1".to_string()
    }

    #[test]
    fn v014_current_version_is_14() {
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");
        let version = current_version(&conn).unwrap();
        assert_eq!(version, 19, "current_version must be 19 after v014..v019 are applied");
    }

    #[test]
    fn v014_unique_module_video_index_is_gone_on_fresh_install() {
        // The bug: v012's UNIQUE (module_id, video_id) index must NOT survive
        // on a fresh install after all migrations run.
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");
        assert!(
            !index_exists(&conn, "idx_lesson_videos_module_video"),
            "the obsolete UNIQUE (module_id, video_id) index must be dropped by v014"
        );
    }

    #[test]
    fn v014_two_sections_can_cache_the_same_video_id() {
        // The exact CR-01 bug: two sections of the same module each caching the
        // SAME video_id. With the old UNIQUE (module_id, video_id) index the
        // second insert would collide and be silently ignored, leaving section B
        // permanently empty. After v014 both inserts must persist.
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");
        let module_id = seed_module(&conn);

        // Section A caches video "vidX".
        conn.execute(
            "INSERT OR IGNORE INTO lesson_videos \
             (id, module_id, section_id, video_id, title, channel_title, relevance_score, status) \
             VALUES ('lv-a', ?1, 'sec-A', 'vidX', 'T', 'C', 0.9, 'ready')",
            rusqlite::params![module_id],
        )
        .unwrap();

        // Section B of the SAME module caches the SAME video "vidX".
        let inserted = conn
            .execute(
                "INSERT OR IGNORE INTO lesson_videos \
                 (id, module_id, section_id, video_id, title, channel_title, relevance_score, status) \
                 VALUES ('lv-b', ?1, 'sec-B', 'vidX', 'T', 'C', 0.9, 'ready')",
                rusqlite::params![module_id],
            )
            .unwrap();

        assert_eq!(
            inserted, 1,
            "second section's INSERT for the same video_id must succeed (no UNIQUE collision)"
        );

        let sec_a: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM lesson_videos WHERE module_id = ?1 AND section_id = 'sec-A'",
                rusqlite::params![module_id],
                |row| row.get(0),
            )
            .unwrap();
        let sec_b: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM lesson_videos WHERE module_id = ?1 AND section_id = 'sec-B'",
                rusqlite::params![module_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sec_a, 1, "section A retains its cached video");
        assert_eq!(sec_b, 1, "section B caches the same video independently");
    }

    #[test]
    fn v014_idempotent_double_apply() {
        // Double-apply is a no-op: schema_migrations has exactly 14 rows and
        // running the drop twice never errors.
        let conn = fresh_conn();
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        // Directly re-run up() to prove DROP INDEX IF EXISTS is idempotent even
        // when the index is already gone.
        up(&conn).expect("up() must be idempotent when the index is absent");

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 19, "exactly 19 rows in schema_migrations after double-apply (v014..v019)");
    }
}
