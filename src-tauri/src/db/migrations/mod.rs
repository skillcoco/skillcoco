//! # Schema Migrations Framework
//!
//! Version-gated, idempotent migration runner for LearnForge's SQLite database.
//!
//! ## How to add a new migration
//!
//! 1. Create `src-tauri/src/db/migrations/vNNN_descriptive_name.rs`:
//!    ```rust
//!    use rusqlite::{Connection, Result};
//!    pub fn up(_conn: &Connection) -> Result<()> {
//!        conn.execute_batch("ALTER TABLE foo ADD COLUMN bar TEXT")?;
//!        Ok(())
//!    }
//!    pub const NAME: &str = "descriptive_name";
//!    pub const VERSION: i32 = NNN;
//!    ```
//! 2. Register the migration in `registered_migrations()` in this file:
//!    ```rust
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
        assert_eq!(version, 11, "After all migrations, version must be 11 (v1..v10 + v11 track_browse_mode)");
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
        assert_eq!(count, 11, "Idempotent: exactly eleven rows in schema_migrations (v1..v10 + v11 track_browse_mode)");
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
        // v1 and v2 were pre-inserted; apply_migrations runs v3..v11.
        // Max is now 11.
        assert_eq!(version, 11, "current_version returns MAX(version) = 11 after v3..v11 applied");
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
}
