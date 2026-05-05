use rusqlite::{Connection, Result};

pub const VERSION: i32 = 4;
pub const NAME: &str = "module_blocks";

pub fn up(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS module_blocks (
            id                  TEXT PRIMARY KEY,
            module_id           TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
            ordering            INTEGER NOT NULL DEFAULT 0,
            block_type          TEXT NOT NULL,
            status              TEXT NOT NULL DEFAULT 'pending',
            params_json         TEXT NOT NULL DEFAULT '{}',
            payload_json        TEXT NOT NULL DEFAULT '{}',
            source_anchors_json TEXT NOT NULL DEFAULT '[]',
            metadata_json       TEXT NOT NULL DEFAULT '{"concept_id": null}',
            retry_count         INTEGER NOT NULL DEFAULT 0,
            created_at          TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at          TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_blocks_module_ordering
            ON module_blocks(module_id, ordering);
        CREATE INDEX IF NOT EXISTS idx_blocks_module_status
            ON module_blocks(module_id, status);
        "#,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    #[test]
    fn v004_module_blocks_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("first apply");
        apply_migrations(&conn).expect("second apply (idempotent)");

        // Assert table exists
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='module_blocks'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "module_blocks table must exist after v004");

        // Assert required columns
        let mut stmt = conn.prepare("PRAGMA table_info(module_blocks)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|c| c.ok())
            .collect();
        for required in [
            "id",
            "module_id",
            "ordering",
            "block_type",
            "status",
            "params_json",
            "payload_json",
            "source_anchors_json",
            "metadata_json",
            "retry_count",
            "created_at",
            "updated_at",
        ] {
            assert!(
                cols.contains(&required.to_string()),
                "module_blocks missing column: {}",
                required
            );
        }

        // Assert both indexes exist
        let idx_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name IN ('idx_blocks_module_ordering', 'idx_blocks_module_status')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_count, 2, "both module_blocks indexes must exist after v004");
    }
}
