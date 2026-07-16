//! Wave 6 (Plan 07-06) — rusqlite-backed impl of
//! [`skillcoco_core::blocks::BlockStore`].
//!
//! Bodies lifted verbatim from `src-tauri/src/db/blocks.rs:68-185`
//! (pre-Wave-6). The error envelope is rewrapped from
//! `rusqlite::Error` → [`BlocksError`] at the trust boundary so
//! `skillcoco-core` never depends on rusqlite (D-02 anti-leakage).
//!
//! ## Orphan-rule note (Wave 2/3/4/5 pattern repeat)
//!
//! `impl BlockStore for &Connection` would trigger
//! `error[E0117]: only traits defined in the current crate can be
//! implemented for arbitrary types` — both the trait (in
//! `skillcoco-core`) and `Connection` (in `rusqlite`) are foreign to
//! `src-tauri`. Wrapping `&Connection` in a local newtype satisfies the
//! orphan rule with zero runtime cost.
//!
//! ## Trust boundary (T-07-05 / T-07-16 / T-07-17)
//!
//! `rusqlite::Error::QueryReturnedNoRows` is mapped to
//! [`BlocksError::NotFound`] **only** in the trait methods that semantically
//! treat absence as an error (none currently — `get_by_id` preserves the
//! pre-Wave-6 `Result<Option<_>>` semantic). All other rusqlite errors
//! are stringified into [`BlocksError::Db`].
//!
//! ## Schema (`module_blocks` table — preserved across the move)
//!
//! Columns map 1:1 to [`ModuleBlock`] fields. `block_type` and `status`
//! are TEXT containing the snake_case enum names (see
//! [`block_type_to_str`] / [`status_to_str`]).

use skillcoco_core::blocks::{
    status_to_str, BlockStatus, BlockStore, BlocksError, ModuleBlock,
};
use rusqlite::Connection;

/// Zero-cost newtype wrapper around `&Connection` that carries the
/// rusqlite-backed [`BlockStore`] impl.
///
/// ## Orphan-rule recipe
///
/// `impl BlockStore for &Connection` directly would violate E0117
/// because both the trait and the target type are foreign to
/// `src-tauri`. The local newtype satisfies the orphan rule with no
/// runtime cost (`#[repr(transparent)]` not strictly necessary for a
/// `&T` newtype — Rust already represents `&T` and a single-field
/// tuple struct around `&T` identically on the wire).
pub struct SqliteBlockStore<'a>(pub &'a Connection);

impl<'a> BlockStore for SqliteBlockStore<'a> {
    fn insert(&self, b: &ModuleBlock) -> Result<(), BlocksError> {
        self.0
            .execute(
                "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
                     params_json, payload_json, source_anchors_json, metadata_json, retry_count,
                     created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                rusqlite::params![
                    b.id,
                    b.module_id,
                    b.ordering,
                    b.block_type,
                    b.status,
                    b.params_json,
                    b.payload_json,
                    b.source_anchors_json,
                    b.metadata_json,
                    b.retry_count,
                    b.created_at,
                    b.updated_at
                ],
            )
            .map_err(|e| BlocksError::Db(e.to_string()))?;
        Ok(())
    }

    fn list_for_module(&self, module_id: &str) -> Result<Vec<ModuleBlock>, BlocksError> {
        let mut stmt = self
            .0
            .prepare(
                "SELECT id, module_id, ordering, block_type, status,
                        params_json, payload_json, source_anchors_json, metadata_json, retry_count,
                        created_at, updated_at
                 FROM module_blocks
                 WHERE module_id = ?1
                 ORDER BY ordering ASC",
            )
            .map_err(|e| BlocksError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([module_id], |row| {
                Ok(ModuleBlock {
                    id: row.get(0)?,
                    module_id: row.get(1)?,
                    ordering: row.get(2)?,
                    block_type: row.get(3)?,
                    status: row.get(4)?,
                    params_json: row.get(5)?,
                    payload_json: row.get(6)?,
                    source_anchors_json: row.get(7)?,
                    metadata_json: row.get(8)?,
                    retry_count: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })
            .map_err(|e| BlocksError::Db(e.to_string()))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| BlocksError::Db(e.to_string()))?);
        }
        Ok(out)
    }

    fn get_by_id(&self, block_id: &str) -> Result<Option<ModuleBlock>, BlocksError> {
        let mut stmt = self
            .0
            .prepare(
                "SELECT id, module_id, ordering, block_type, status,
                        params_json, payload_json, source_anchors_json, metadata_json, retry_count,
                        created_at, updated_at
                 FROM module_blocks
                 WHERE id = ?1",
            )
            .map_err(|e| BlocksError::Db(e.to_string()))?;
        let mut rows = stmt
            .query_map([block_id], |row| {
                Ok(ModuleBlock {
                    id: row.get(0)?,
                    module_id: row.get(1)?,
                    ordering: row.get(2)?,
                    block_type: row.get(3)?,
                    status: row.get(4)?,
                    params_json: row.get(5)?,
                    payload_json: row.get(6)?,
                    source_anchors_json: row.get(7)?,
                    metadata_json: row.get(8)?,
                    retry_count: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })
            .map_err(|e| BlocksError::Db(e.to_string()))?;
        match rows.next() {
            Some(Ok(block)) => Ok(Some(block)),
            Some(Err(e)) => Err(BlocksError::Db(e.to_string())),
            None => Ok(None),
        }
    }

    fn update_payload(
        &self,
        id: &str,
        status: BlockStatus,
        payload_json: &str,
    ) -> Result<(), BlocksError> {
        self.0
            .execute(
                "UPDATE module_blocks SET status=?1, payload_json=?2, updated_at=datetime('now') WHERE id=?3",
                rusqlite::params![status_to_str(&status), payload_json, id],
            )
            .map_err(|e| BlocksError::Db(e.to_string()))?;
        Ok(())
    }

    fn count_for_module(&self, module_id: &str) -> Result<i64, BlocksError> {
        self.0
            .query_row(
                "SELECT COUNT(*) FROM module_blocks WHERE module_id = ?1",
                [module_id],
                |r| r.get(0),
            )
            .map_err(|e| BlocksError::Db(e.to_string()))
    }

    fn delete_for_module(&self, module_id: &str) -> Result<usize, BlocksError> {
        self.0
            .execute(
                "DELETE FROM module_blocks WHERE module_id = ?1",
                [module_id],
            )
            .map_err(|e| BlocksError::Db(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    //! Integration tests against an in-memory `Connection`. The
    //! corresponding pure-stub tests (covering the trait surface in
    //! isolation) live in `skillcoco-core/src/blocks.rs::tests`.

    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    fn sample_block(module_id: &str) -> ModuleBlock {
        ModuleBlock {
            id: uuid::Uuid::new_v4().to_string(),
            module_id: module_id.to_string(),
            ordering: 0,
            block_type: "section".to_string(),
            status: "pending".to_string(),
            params_json: "{}".to_string(),
            payload_json: "{}".to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id":null}"#.to_string(),
            retry_count: 0,
            created_at: "2026-05-05T00:00:00Z".to_string(),
            updated_at: "2026-05-05T00:00:00Z".to_string(),
        }
    }

    /// Insert a parent module row so foreign key constraints are satisfied.
    fn insert_parent_module(conn: &Connection, module_id: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id) VALUES ('lp-test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO learning_tracks (id, learner_id, topic, domain_module) VALUES ('trk-test', 'lp-test', 'test', 'test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO learning_paths (id, track_id) VALUES ('path-test', 'trk-test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO modules (id, path_id, title) VALUES (?1, 'path-test', 'Test Module')",
            [module_id],
        )
        .unwrap();
    }

    #[test]
    fn insert_and_list_for_module() {
        let conn = fresh_conn();
        insert_parent_module(&conn, "mod-002");
        let store = SqliteBlockStore(&conn);

        for i in 0..3i32 {
            let mut block = sample_block("mod-002");
            block.id = format!("blk-{}", i);
            block.ordering = i;
            store.insert(&block).unwrap();
        }

        let blocks = store.list_for_module("mod-002").unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].ordering, 0);
        assert_eq!(blocks[1].ordering, 1);
        assert_eq!(blocks[2].ordering, 2);
    }

    #[test]
    fn get_by_id_returns_some_then_none() {
        let conn = fresh_conn();
        insert_parent_module(&conn, "mod-005");
        let store = SqliteBlockStore(&conn);

        let mut block = sample_block("mod-005");
        block.id = "blk-find-me".to_string();
        store.insert(&block).unwrap();

        let got = store.get_by_id("blk-find-me").unwrap();
        assert!(got.is_some(), "must find the inserted block");
        assert_eq!(got.unwrap().module_id, "mod-005");

        let missing = store.get_by_id("blk-never-inserted").unwrap();
        assert!(missing.is_none(), "missing block must yield Ok(None)");
    }

    #[test]
    fn update_payload_advances_status() {
        let conn = fresh_conn();
        insert_parent_module(&conn, "mod-006");
        let store = SqliteBlockStore(&conn);

        let mut block = sample_block("mod-006");
        block.id = "blk-upd".to_string();
        store.insert(&block).unwrap();

        let new_payload = r##"{"markdown":"# Hello"}"##;
        store
            .update_payload("blk-upd", BlockStatus::Ready, new_payload)
            .unwrap();

        let updated = store.get_by_id("blk-upd").unwrap().unwrap();
        assert_eq!(updated.status, "ready", "status must advance to ready");
        assert_eq!(
            updated.payload_json, new_payload,
            "payload_json must be updated"
        );
    }

    #[test]
    fn count_for_module_returns_row_count() {
        let conn = fresh_conn();
        insert_parent_module(&conn, "mod-007");
        let store = SqliteBlockStore(&conn);

        assert_eq!(store.count_for_module("mod-007").unwrap(), 0);

        for i in 0..5i32 {
            let mut block = sample_block("mod-007");
            block.id = format!("blk-count-{}", i);
            block.ordering = i;
            store.insert(&block).unwrap();
        }
        assert_eq!(store.count_for_module("mod-007").unwrap(), 5);
    }

    #[test]
    fn delete_for_module_returns_rows_affected() {
        let conn = fresh_conn();
        insert_parent_module(&conn, "mod-008");
        let store = SqliteBlockStore(&conn);

        for i in 0..4i32 {
            let mut block = sample_block("mod-008");
            block.id = format!("blk-del-{}", i);
            block.ordering = i;
            store.insert(&block).unwrap();
        }

        let removed = store.delete_for_module("mod-008").unwrap();
        assert_eq!(removed, 4, "all four blocks must be deleted");
        assert_eq!(store.count_for_module("mod-008").unwrap(), 0);

        let removed_again = store.delete_for_module("mod-008").unwrap();
        assert_eq!(removed_again, 0, "second delete must be a no-op");
    }

    #[test]
    fn sqlite_block_store_is_object_safe() {
        let conn = fresh_conn();
        insert_parent_module(&conn, "mod-dyn");
        let store = SqliteBlockStore(&conn);
        let dynstore: &dyn BlockStore = &store;

        let mut block = sample_block("mod-dyn");
        block.id = "blk-dyn".to_string();
        dynstore.insert(&block).unwrap();

        assert_eq!(dynstore.count_for_module("mod-dyn").unwrap(), 1);
    }
}
