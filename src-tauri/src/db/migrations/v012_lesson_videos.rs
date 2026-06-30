//! Migration v012: Create lesson_videos table for per-lesson video cache.
//!
//! Backs the indefinite per-lesson video cache introduced in Phase 11
//! (video-enriched lessons, D-04). Videos are fetched once from YouTube
//! and ranked by an LLM; results live in this table until a manual refresh.
//!
//! The table is module-scoped (module_id FK → modules.id ON DELETE CASCADE)
//! so orphaned cache rows are automatically removed when a module is deleted.
//! All DDL uses IF NOT EXISTS / IF NOT EXISTS guards so double-apply is safe.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 12;
pub const NAME: &str = "lesson_videos";

pub fn up(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS lesson_videos (
            id              TEXT PRIMARY KEY,
            module_id       TEXT NOT NULL,
            video_id        TEXT NOT NULL,
            title           TEXT NOT NULL,
            channel_title   TEXT NOT NULL,
            relevance_score REAL NOT NULL,
            status          TEXT NOT NULL DEFAULT 'ready',
            fetched_at      TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (module_id) REFERENCES modules(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_lesson_videos_module_id ON lesson_videos(module_id);",
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
    fn v012_creates_lesson_videos_table_and_version_is_12() {
        // After apply_migrations on a fresh DB, current_version == 12 AND
        // the lesson_videos table exists with the expected columns.
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");

        let version = current_version(&conn).unwrap();
        assert_eq!(version, 12, "current_version must be 12 after v012 is applied");

        // Verify table exists by counting its columns via PRAGMA table_info
        let mut stmt = conn
            .prepare("PRAGMA table_info(lesson_videos)")
            .unwrap();
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        let expected_cols = [
            "id",
            "module_id",
            "video_id",
            "title",
            "channel_title",
            "relevance_score",
            "status",
            "fetched_at",
        ];
        for col in &expected_cols {
            assert!(
                columns.contains(&col.to_string()),
                "lesson_videos must have column '{}'",
                col
            );
        }
        assert_eq!(
            columns.len(),
            8,
            "lesson_videos must have exactly 8 columns"
        );
    }

    #[test]
    fn v012_insert_and_fk_cascade_delete() {
        // Insert a lesson_video row and verify it is removed when the parent module is deleted.
        let conn = fresh_conn();
        apply_migrations(&conn).expect("migrations must succeed");

        // Seed required FK chain:
        //   learner_profiles → learning_tracks → learning_paths → modules → lesson_videos
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) \
             VALUES ('tr1', 'lp1', 'Kubernetes', 'devops', 'Pass CKA')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES ('path1', 'tr1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('m1', 'path1', 'Pods')",
            [],
        )
        .unwrap();

        // Insert a lesson_video row
        conn.execute(
            "INSERT INTO lesson_videos (id, module_id, video_id, title, channel_title, relevance_score) \
             VALUES ('lv1', 'm1', 'dQw4w9WgXcQ', 'Kubernetes Pods Explained', 'TechChannel', 0.92)",
            [],
        )
        .expect("INSERT into lesson_videos must succeed");

        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM lesson_videos WHERE module_id = 'm1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "one lesson_video row must exist");

        // Delete the parent module — CASCADE must remove the child row
        conn.execute("DELETE FROM modules WHERE id = 'm1'", []).unwrap();

        let count_after: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM lesson_videos WHERE module_id = 'm1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count_after, 0,
            "lesson_videos row must be cascade-deleted when parent module is removed"
        );
    }

    #[test]
    fn v012_idempotent_double_apply() {
        // apply_migrations twice succeeds; schema_migrations has exactly 12 rows.
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
            count, 12,
            "exactly 12 rows in schema_migrations after idempotent double-apply"
        );
    }
}
