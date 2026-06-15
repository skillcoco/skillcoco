//! Migration v008 — topic_packs registry (Phase 5 Wave 0 RED scaffold).
//!
//! Wave 0 (Plan 05-01) lands the migration *registration* + *idempotency test*
//! only. The `up()` body is intentionally a no-op until Plan 05-02 fills it.
//!
//! When Plan 05-02 lands, `up()` MUST produce:
//!   - `topic_packs` table with columns:
//!       id TEXT PRIMARY KEY,
//!       title TEXT NOT NULL,
//!       source TEXT NOT NULL,                       -- 'bundled' | 'skill'
//!       enabled INTEGER NOT NULL DEFAULT 1,
//!       pack_version TEXT NOT NULL DEFAULT '1.0',
//!       last_loaded_at TEXT NOT NULL,
//!       validation_status TEXT NOT NULL,            -- 'ok' | 'warnings' | 'errors'
//!       validation_messages_json TEXT NOT NULL DEFAULT '[]'
//!   - Index `idx_topic_packs_source_enabled (source, enabled)`.
//!
//! The `v008_idempotent` test asserts the post-conditions Plan 05-02 must
//! satisfy. Today it FAILS — that is the RED contract.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 8;
pub const NAME: &str = "topic_packs";

/// Apply the v008 migration.
///
/// Wave 0 (Plan 05-01) leaves this as a no-op stub. Plan 05-02 fills the body
/// per the documented post-conditions above.
pub fn up(_conn: &Connection) -> Result<()> {
    // Wave 1 (Plan 05-02) implements the schema.
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    /// PACK-10 — v008 idempotency contract.
    ///
    /// Asserts the post-conditions Plan 05-02's `up()` body must satisfy:
    /// 1. `topic_packs` table exists
    /// 2. Columns include `id, title, source, enabled, pack_version,
    ///    last_loaded_at, validation_status, validation_messages_json`
    /// 3. Index `idx_topic_packs_source_enabled` exists
    ///
    /// Wave 0 (Plan 05-01) RED state: this test FAILS — the up() body is
    /// empty so the table and columns don't exist yet. Plan 05-02 turns
    /// it GREEN per CONTEXT.md D-10.
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
        assert_eq!(
            tp_count, 1,
            "Wave 1 (Plan 05-02) must implement v008_topic_packs::up — see CONTEXT.md D-10: topic_packs table must exist"
        );

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
                "Wave 1 (Plan 05-02) must add column `{}` to topic_packs (got {:?})",
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
        assert_eq!(
            idx_count, 1,
            "Wave 1 (Plan 05-02) must create idx_topic_packs_source_enabled (D-10)"
        );
    }
}
