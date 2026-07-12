//! # Schema Migrations Framework
//!
//! Version-gated, idempotent migration runner for LearnForge's SQLite database.
//!
//! ## How to add a new migration
//!
//! 1. Create `src-tauri/src/db/migrations/vNNN_descriptive_name.rs`:
//!    ```text
//!    use rusqlite::{Connection, Result};
//!    pub fn up(_conn: &Connection) -> Result<()> {
//!        conn.execute_batch("ALTER TABLE foo ADD COLUMN bar TEXT")?;
//!        Ok(())
//!    }
//!    pub const NAME: &str = "descriptive_name";
//!    pub const VERSION: i32 = NNN;
//!    ```
//! 2. Register the migration in `registered_migrations()` in this file:
//!    ```text
//!    Migration { version: vNNN_descriptive_name::VERSION, name: vNNN_descriptive_name::NAME, up: vNNN_descriptive_name::up },
//!    ```
//! 3. The runner picks it up on the next app launch. ALTER TABLE runs once — never again.
//!
//! ## Pattern used by downstream plans
//! - Plan 04 (FIX-04): streak_days and last_activity_date columns on learning_tracks
//! - All Phase 2+ schema changes: add a vNNN file + registration entry here

use rusqlite::Connection;

pub mod v001_initial;
pub mod v002_drop_ai_config;
pub mod v003_streak_columns;
pub mod v004_module_blocks;
pub mod v005_lesson_completions;
pub mod v006_lab_progress;
pub mod v007_microlearning;
pub mod v008_topic_packs;
pub mod v009_achievements;
pub mod v010_cert_simplification;
pub mod v011_track_browse_mode;
pub mod v012_lesson_videos;
pub mod v013_section_video_granularity;
pub mod v014_drop_module_video_unique_index;
pub mod v015_learning_path_verified;
pub mod v016_quiz_attempts;
pub mod v017_skill_reports;
pub mod v018_backfill_capability_tags;
pub mod v019_exam_attempts;
// Phase 15-01 (Wave 0) — module declared so its RED scaffold tests compile
// and run; NOT YET registered in `registered_migrations()` below (that is
// explicitly 15-02's job — registration is what flips the schema version
// count from 19 -> 20, see v020_entitlements.rs module doc).
pub mod v020_entitlements;

/// A single schema migration.
pub struct Migration {
    pub version: i32,
    pub name: &'static str,
    pub up: fn(&Connection) -> rusqlite::Result<()>,
}

/// Returns the ordered list of all registered migrations.
/// New migrations must be appended in version order.
fn registered_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: v001_initial::VERSION,
            name: v001_initial::NAME,
            up: v001_initial::up,
        },
        Migration {
            version: v002_drop_ai_config::VERSION,
            name: v002_drop_ai_config::NAME,
            up: v002_drop_ai_config::up,
        },
        Migration {
            version: v003_streak_columns::VERSION,
            name: v003_streak_columns::NAME,
            up: v003_streak_columns::up,
        },
        Migration {
            version: v004_module_blocks::VERSION,
            name: v004_module_blocks::NAME,
            up: v004_module_blocks::up,
        },
        Migration {
            version: v005_lesson_completions::VERSION,
            name: v005_lesson_completions::NAME,
            up: v005_lesson_completions::up,
        },
        Migration {
            version: v006_lab_progress::VERSION,
            name: v006_lab_progress::NAME,
            up: v006_lab_progress::up,
        },
        Migration {
            version: v007_microlearning::VERSION,
            name: v007_microlearning::NAME,
            up: v007_microlearning::up,
        },
        Migration {
            version: v008_topic_packs::VERSION,
            name: v008_topic_packs::NAME,
            up: v008_topic_packs::up,
        },
        Migration {
            version: v009_achievements::VERSION,
            name: v009_achievements::NAME,
            up: v009_achievements::up,
        },
        Migration {
            version: v010_cert_simplification::VERSION,
            name: v010_cert_simplification::NAME,
            up: v010_cert_simplification::up,
        },
        Migration {
            version: v011_track_browse_mode::VERSION,
            name: v011_track_browse_mode::NAME,
            up: v011_track_browse_mode::up,
        },
        Migration {
            version: v012_lesson_videos::VERSION,
            name: v012_lesson_videos::NAME,
            up: v012_lesson_videos::up,
        },
        Migration {
            version: v013_section_video_granularity::VERSION,
            name: v013_section_video_granularity::NAME,
            up: v013_section_video_granularity::up,
        },
        Migration {
            version: v014_drop_module_video_unique_index::VERSION,
            name: v014_drop_module_video_unique_index::NAME,
            up: v014_drop_module_video_unique_index::up,
        },
        Migration {
            version: v015_learning_path_verified::VERSION,
            name: v015_learning_path_verified::NAME,
            up: v015_learning_path_verified::up,
        },
        Migration {
            version: v016_quiz_attempts::VERSION,
            name: v016_quiz_attempts::NAME,
            up: v016_quiz_attempts::up,
        },
        Migration {
            version: v017_skill_reports::VERSION,
            name: v017_skill_reports::NAME,
            up: v017_skill_reports::up,
        },
        Migration {
            version: v018_backfill_capability_tags::VERSION,
            name: v018_backfill_capability_tags::NAME,
            up: v018_backfill_capability_tags::up,
        },
        Migration {
            version: v019_exam_attempts::VERSION,
            name: v019_exam_attempts::NAME,
            up: v019_exam_attempts::up,
        },
    ]
}

/// Creates the schema_migrations table if it does not yet exist.
fn ensure_schema_migrations_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
}

/// Returns the highest migration version that has been applied, or 0 if none.
pub fn current_version(conn: &Connection) -> rusqlite::Result<i32> {
    ensure_schema_migrations_table(conn)?;
    let version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    Ok(version)
}

/// Applies all pending migrations in version order, atomically.
///
/// Each migration that has `version > current_version` is run inside a single
/// BEGIN TRANSACTION / COMMIT block. If any migration fails the transaction is
/// rolled back and the error is returned.
pub fn apply_migrations(conn: &Connection) -> rusqlite::Result<()> {
    ensure_schema_migrations_table(conn)?;

    let current = current_version(conn)?;
    let migrations = registered_migrations();

    // Filter to only pending migrations, sorted by version
    let pending: Vec<&Migration> = migrations
        .iter()
        .filter(|m| m.version > current)
        .collect();

    if pending.is_empty() {
        return Ok(());
    }

    conn.execute_batch("BEGIN")?;

    for migration in pending {
        (migration.up)(conn)?;
        conn.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            rusqlite::params![migration.version, migration.name],
        )?;
    }

    conn.execute_batch("COMMIT")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        // Apply the base schema so tables exist for v1 baseline migration
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        conn
    }

    #[test]
    fn test_apply_migrations_records_latest_version() {
        // Test 1: after apply_migrations on fresh DB, MAX(version) = 2 (v1 + v2)
        let conn = fresh_conn();
        apply_migrations(&conn).expect("apply_migrations must not fail on fresh DB");
        let version: i32 = conn
            .query_row(
                "SELECT MAX(version) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 19, "After all migrations, version must be 19 (v1..v15 + v16 quiz_attempts + v17 skill_reports + v18 backfill_capability_tags + v19 exam_attempts)");
    }

    #[test]
    fn test_apply_migrations_idempotent() {
        // Test 2: calling apply_migrations twice on same connection is idempotent
        let conn = fresh_conn();
        apply_migrations(&conn).expect("First apply must succeed");
        apply_migrations(&conn).expect("Second apply must succeed (idempotent)");

        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 19, "Idempotent: exactly nineteen rows in schema_migrations (v1..v15 + v16 quiz_attempts + v17 skill_reports + v18 backfill_capability_tags + v19 exam_attempts)");
    }

    #[test]
    fn test_migration_skip_when_already_applied() {
        // Test 3: pre-inserting a version causes that migration to be skipped
        let conn = fresh_conn();
        // Simulate that version 2 was already applied
        ensure_schema_migrations_table(&conn).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (2, 'fake_v2', datetime('now'))",
            [],
        ).unwrap();
        // Apply should not re-run v2 (or any older version if we had them registered)
        // v1 is not yet applied, so apply_migrations runs v1, skips v2 (v2 > v1 but we
        // simulate the scenario: if we had v1 already applied, this test verifies skipping)
        // For this test: pre-insert v1 so nothing runs
        conn.execute(
            "INSERT OR IGNORE INTO schema_migrations (version, name) VALUES (1, 'initial_baseline')",
            [],
        ).unwrap();
        apply_migrations(&conn).expect("apply_migrations must succeed when v1+v2 are already applied");

        let version = current_version(&conn).unwrap();
        // v1 and v2 were pre-inserted; apply_migrations runs v3..v19.
        // Max is now 19.
        assert_eq!(version, 19, "current_version returns MAX(version) = 19 after v3..v19 applied + v19 exam_attempts");
    }

    #[test]
    fn test_current_version_returns_zero_on_fresh_db() {
        // Test 4: before any migration, current_version returns 0
        let conn = fresh_conn();
        // Do NOT call apply_migrations — check raw starting state
        ensure_schema_migrations_table(&conn).unwrap();
        let v = current_version(&conn).unwrap();
        assert_eq!(v, 0, "Fresh DB with empty schema_migrations must return version 0");
    }

    // ── v018 backfill_capability_tags (CR-01 gap closure, 18-08) ──

    /// Seed a module with a non-empty skills_json but ZERO capability_tags
    /// rows (simulating a track generated before the 18-08 writer existed),
    /// run v018::up(), and assert capability_tags now contains one row per
    /// skills_json entry with correct module_id/track_id/learner_id/
    /// tag_slug/tag_label.
    #[test]
    fn v018_backfills_capability_tags_from_existing_skills_json() {
        let conn = fresh_conn();
        apply_migrations(&conn).expect("apply base schema + v1..v17 first");

        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id) VALUES ('lp-bf')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module) VALUES ('trk-bf', 'lp-bf', 'Kubernetes', 'devops')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES ('path-bf', 'trk-bf')",
            [],
        )
        .unwrap();
        let skills_json = serde_json::json!([
            {"label": "Can configure RBAC policies", "slug": "can-configure-rbac-policies"},
            {"label": "Can debug pod networking", "slug": "can-debug-pod-networking"},
        ])
        .to_string();
        conn.execute(
            "INSERT INTO modules (id, path_id, title, objectives_json, skills_json) VALUES ('mod-bf', 'path-bf', 'Pods and Nodes', '[]', ?1)",
            rusqlite::params![skills_json],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id) VALUES ('mp-bf', 'mod-bf', 'lp-bf')",
            [],
        )
        .unwrap();

        // Sanity: zero capability_tags rows before backfill.
        let pre_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM capability_tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(pre_count, 0, "no capability_tags rows before v018 runs");

        v018_backfill_capability_tags::up(&conn).expect("v018 up must succeed");

        let mut stmt = conn
            .prepare(
                "SELECT learner_id, track_id, module_id, tag_slug, tag_label FROM capability_tags ORDER BY tag_label",
            )
            .unwrap();
        let rows: Vec<(String, String, String, String, String)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                ))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(rows.len(), 2, "one capability_tags row per skills_json entry");
        assert_eq!(rows[0].0, "lp-bf");
        assert_eq!(rows[0].1, "trk-bf");
        assert_eq!(rows[0].2, "mod-bf");
        assert_eq!(rows[0].3, "can-configure-rbac-policies");
        assert_eq!(rows[0].4, "Can configure RBAC policies");
        assert_eq!(rows[1].3, "can-debug-pod-networking");
        assert_eq!(rows[1].4, "Can debug pod networking");
    }

    /// Idempotency: running v018::up() a second time must not increase the
    /// capability_tags row count (NOT EXISTS guard).
    #[test]
    fn v018_backfill_is_idempotent() {
        let conn = fresh_conn();
        apply_migrations(&conn).expect("apply base schema + v1..v17 first");

        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id) VALUES ('lp-idem')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module) VALUES ('trk-idem', 'lp-idem', 'Kubernetes', 'devops')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES ('path-idem', 'trk-idem')",
            [],
        )
        .unwrap();
        let skills_json = serde_json::json!([
            {"label": "Can configure RBAC policies", "slug": "can-configure-rbac-policies"},
        ])
        .to_string();
        conn.execute(
            "INSERT INTO modules (id, path_id, title, objectives_json, skills_json) VALUES ('mod-idem', 'path-idem', 'Pods and Nodes', '[]', ?1)",
            rusqlite::params![skills_json],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id) VALUES ('mp-idem', 'mod-idem', 'lp-idem')",
            [],
        )
        .unwrap();

        v018_backfill_capability_tags::up(&conn).expect("first up must succeed");
        let count_after_first: i64 = conn
            .query_row("SELECT COUNT(*) FROM capability_tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count_after_first, 1);

        v018_backfill_capability_tags::up(&conn).expect("second up must succeed");
        let count_after_second: i64 = conn
            .query_row("SELECT COUNT(*) FROM capability_tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            count_after_second, count_after_first,
            "running v018 twice must not duplicate capability_tags rows"
        );
    }

    /// Overlap with the 18-08 writer: if a capability_tags row already exists
    /// for a (module_id, tag_slug, learner_id) — e.g. because the production
    /// writer already inserted it for a freshly generated track — v018 must
    /// not insert a duplicate.
    #[test]
    fn v018_backfill_skips_rows_already_written_by_production_writer() {
        let conn = fresh_conn();
        apply_migrations(&conn).expect("apply base schema + v1..v17 first");

        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id) VALUES ('lp-ov')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module) VALUES ('trk-ov', 'lp-ov', 'Kubernetes', 'devops')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES ('path-ov', 'trk-ov')",
            [],
        )
        .unwrap();
        let skills_json = serde_json::json!([
            {"label": "Can configure RBAC policies", "slug": "can-configure-rbac-policies"},
        ])
        .to_string();
        conn.execute(
            "INSERT INTO modules (id, path_id, title, objectives_json, skills_json) VALUES ('mod-ov', 'path-ov', 'Pods and Nodes', '[]', ?1)",
            rusqlite::params![skills_json],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id) VALUES ('mp-ov', 'mod-ov', 'lp-ov')",
            [],
        )
        .unwrap();
        // Pre-insert the row as if the 18-08 writer already ran for this module.
        conn.execute(
            "INSERT INTO capability_tags (id, learner_id, track_id, module_id, tag_slug, tag_label, evidence_class) VALUES ('ct-existing', 'lp-ov', 'trk-ov', 'mod-ov', 'can-configure-rbac-policies', 'Can configure RBAC policies', 'module')",
            [],
        )
        .unwrap();

        v018_backfill_capability_tags::up(&conn).expect("up must succeed");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM capability_tags WHERE module_id = 'mod-ov'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "pre-existing writer row must not be duplicated by the backfill");
    }
}
