use rusqlite::{Connection, Result};

pub const VERSION: i32 = 4;
pub const NAME: &str = "module_blocks";

pub fn up(_conn: &Connection) -> Result<()> {
    // Wave 1 (03-02 Task 1) implements this.
    // Stub returns Ok without creating the table so Wave 0 tests FAIL.
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
    }
}
