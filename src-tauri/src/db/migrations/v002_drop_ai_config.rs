//! Migration v002: Drop the legacy `ai_config` table.
//!
//! FIX-03: The `ai_config` table is dead code — all authentication flows through
//! `AuthState` (credentials store in auth/mod.rs). This migration drops the table
//! on existing databases to eliminate the confusion for contributors and remove the
//! dead SQL surface.
//!
//! Note: The CREATE TABLE in schema.rs is also removed so new databases never
//! create it. This migration handles the upgrade path for existing users.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 2;
pub const NAME: &str = "drop_ai_config";

/// Drop the legacy ai_config table and its default row.
///
/// Safe to run even if the table was already removed (DROP TABLE IF EXISTS).
pub fn up(conn: &Connection) -> Result<()> {
    conn.execute_batch("DROP TABLE IF EXISTS ai_config")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Verify that v002 drops the ai_config table on a DB that has it.
    #[test]
    fn test_v002_drops_ai_config_table() {
        let conn = Connection::open_in_memory().unwrap();

        // Create a fake ai_config table to simulate an existing database
        conn.execute_batch(
            "CREATE TABLE ai_config (
                id INTEGER PRIMARY KEY,
                provider_type TEXT NOT NULL DEFAULT 'claude'
            );
            INSERT INTO ai_config (id) VALUES (1);",
        )
        .unwrap();

        // Verify it exists
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='ai_config'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "ai_config table should exist before migration");

        // Run the migration
        up(&conn).expect("v002 migration must succeed");

        // Verify it's gone
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='ai_config'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "ai_config table must be dropped by v002");
    }

    /// Verify that v002 is idempotent — safe to run when table doesn't exist.
    #[test]
    fn test_v002_idempotent_when_table_absent() {
        let conn = Connection::open_in_memory().unwrap();
        // No ai_config table — should succeed silently
        up(&conn).expect("v002 must not fail when ai_config table is already absent");
    }
}
