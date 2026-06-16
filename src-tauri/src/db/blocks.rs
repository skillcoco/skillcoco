//! Transitional shim — Phase 7 Wave 6 (07-06) moved the block taxonomy
//! to [`learnforge_core::blocks`]. The rusqlite-backed [`BlockStore`] impl
//! lives in [`crate::storage_impl::blocks::SqliteBlockStore`].
//!
//! ## What's here now
//!
//! - **Re-exports** of [`BlockType`], [`BlockStatus`], [`ModuleBlock`],
//!   [`BlocksError`], [`BlockStore`], and the [`block_type_to_str`] /
//!   [`status_to_str`] helpers from `learnforge_core::blocks` — so every
//!   `use crate::db::blocks::*` call site compiles unchanged.
//! - **Legacy free-fn facades** (`insert_block`, `list_blocks_by_module`,
//!   `get_block`, `update_block_payload`, `count_blocks_by_module`,
//!   `delete_blocks_by_module`) that delegate to the trait impl via
//!   `SqliteBlockStore(conn)`. **Zero call-site churn** for
//!   `commands/blocks.rs` (96.7KB, the most-called IPC surface in the
//!   codebase), `commands/ai.rs:502`, `labs/{eval,session,state}.rs`, and
//!   `commands/learning.rs:309`.
//!
//! ## Wave 10 cleanup
//!
//! Wave 10 grep-and-rewrites the call sites to call the trait directly
//! (`SqliteBlockStore(conn).insert(...)`) and deletes this shim. The
//! pub-use does **NOT** use `#[deprecated]` because rustc silently
//! ignores it on `pub use` items (R5 / Pitfall 6).
//!
//! ## Integration tests
//!
//! The CRUD-against-in-memory-Connection tests retained at the bottom
//! of this file now exercise the trait impl via the legacy facades —
//! same test bodies as pre-Wave-6, proving the shim preserves
//! end-to-end behavior. Pure type-level tests (serde round-trips,
//! enum-to-str helpers) moved to `learnforge-core/src/blocks.rs`.

pub use learnforge_core::blocks::{
    block_type_to_str, status_to_str, BlockStatus, BlockStore, BlockType, BlocksError,
    ModuleBlock,
};

use crate::storage_impl::blocks::SqliteBlockStore;
use rusqlite::Connection;

/// Legacy facade — delegates to [`BlockStore::insert`].
/// Wave 10 deletes this; callers switch to `SqliteBlockStore(conn).insert(...)`.
pub fn insert_block(conn: &Connection, b: &ModuleBlock) -> Result<(), BlocksError> {
    SqliteBlockStore(conn).insert(b)
}

/// Legacy facade — delegates to [`BlockStore::list_for_module`].
/// Wave 10 deletes this; callers switch to `SqliteBlockStore(conn).list_for_module(...)`.
pub fn list_blocks_by_module(
    conn: &Connection,
    module_id: &str,
) -> Result<Vec<ModuleBlock>, BlocksError> {
    SqliteBlockStore(conn).list_for_module(module_id)
}

/// Legacy facade — delegates to [`BlockStore::get_by_id`].
/// Wave 10 deletes this; callers switch to `SqliteBlockStore(conn).get_by_id(...)`.
pub fn get_block(
    conn: &Connection,
    block_id: &str,
) -> Result<Option<ModuleBlock>, BlocksError> {
    SqliteBlockStore(conn).get_by_id(block_id)
}

/// Legacy facade — delegates to [`BlockStore::update_payload`].
/// Wave 10 deletes this; callers switch to `SqliteBlockStore(conn).update_payload(...)`.
pub fn update_block_payload(
    conn: &Connection,
    id: &str,
    status: BlockStatus,
    payload_json: &str,
) -> Result<(), BlocksError> {
    SqliteBlockStore(conn).update_payload(id, status, payload_json)
}

/// Legacy facade — delegates to [`BlockStore::count_for_module`].
/// Wave 10 deletes this; callers switch to `SqliteBlockStore(conn).count_for_module(...)`.
pub fn count_blocks_by_module(conn: &Connection, module_id: &str) -> Result<i64, BlocksError> {
    SqliteBlockStore(conn).count_for_module(module_id)
}

/// Legacy facade — delegates to [`BlockStore::delete_for_module`].
/// Wave 10 deletes this; callers switch to `SqliteBlockStore(conn).delete_for_module(...)`.
/// Used by `regenerate_module` in 03-03.
pub fn delete_blocks_by_module(conn: &Connection, module_id: &str) -> Result<usize, BlocksError> {
    SqliteBlockStore(conn).delete_for_module(module_id)
}

#[cfg(test)]
mod tests {
    //! Integration tests through the legacy free-fn facades — these
    //! exercise the trait impl via the shim and prove that the
    //! end-to-end behavior is preserved across the Wave 6 move.
    //!
    //! Pure type-level tests (serde round-trips, enum-to-str coverage)
    //! moved to `learnforge-core/src/blocks.rs::tests`.

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

    /// Serde IPC contract: serialized JSON must contain camelCase keys.
    /// (Re-asserted here as a cross-crate integration test; the unit-level
    /// guarantee lives in `learnforge-core/src/blocks.rs::tests::module_block_serializes_camel_case`.)
    #[test]
    fn test_module_block_camel_case() {
        let block = sample_block("mod-001");
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("moduleId"), "must serialize to moduleId");
        assert!(json.contains("blockType"), "must serialize to blockType");
        assert!(json.contains("paramsJson"), "must serialize to paramsJson");
        assert!(json.contains("payloadJson"), "must serialize to payloadJson");
        assert!(json.contains("sourceAnchorsJson"), "must serialize to sourceAnchorsJson");
        assert!(json.contains("metadataJson"), "must serialize to metadataJson");
        assert!(json.contains("retryCount"), "must serialize to retryCount");
        assert!(json.contains("createdAt"), "must serialize to createdAt");
        assert!(json.contains("updatedAt"), "must serialize to updatedAt");

        // Verify round-trip with DB through the shim
        let conn = fresh_conn();
        insert_parent_module(&conn, "mod-001");
        insert_block(&conn, &block).unwrap();
        let rows = list_blocks_by_module(&conn, "mod-001").unwrap();
        assert_eq!(rows.len(), 1);
        let round_tripped = serde_json::to_string(&rows[0]).unwrap();
        assert!(round_tripped.contains("moduleId"));
    }

    /// CRUD test: insert 3 blocks ordered 0,1,2, list returns them in ASC order.
    #[test]
    fn block_insert_and_query() {
        let conn = fresh_conn();
        insert_parent_module(&conn, "mod-002");

        for i in 0..3i32 {
            let mut block = sample_block("mod-002");
            block.id = format!("blk-{}", i);
            block.ordering = i;
            insert_block(&conn, &block).unwrap();
        }

        let blocks = list_blocks_by_module(&conn, "mod-002").unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].ordering, 0);
        assert_eq!(blocks[1].ordering, 1);
        assert_eq!(blocks[2].ordering, 2);
    }

    /// PACK-04 concept-graph forward-link contract (ROADMAP success criterion #7).
    ///
    /// After v004 migration, any inserted row without explicit metadata_json must default
    /// to '{"concept_id": null}' so Phase 2 PATH-02 can link without a schema change.
    #[test]
    fn test_module_blocks_metadata_default_concept_id_null() {
        let conn = fresh_conn();
        insert_parent_module(&conn, "mod-003");

        // INSERT without specifying metadata_json — relies on the SQL DEFAULT
        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status)
             VALUES ('blk-001', 'mod-003', 0, 'section', 'pending')",
            [],
        )
        .expect("INSERT must succeed after v004 creates the table");

        let metadata_json: String = conn
            .query_row(
                "SELECT metadata_json FROM module_blocks WHERE id = 'blk-001'",
                [],
                |r| r.get(0),
            )
            .expect("Row must be queryable");

        let v: serde_json::Value =
            serde_json::from_str(&metadata_json).expect("metadata_json must be valid JSON");
        assert!(
            v.get("concept_id").is_some(),
            "metadata_json must contain concept_id key"
        );
        assert!(
            v["concept_id"].is_null(),
            "metadata_json.concept_id must default to null (Phase 2 PATH-02 populates this slot)"
        );
    }

    /// Block status round-trip: insert with Pending, query, deserialize back.
    #[test]
    fn block_status_round_trip() {
        let conn = fresh_conn();
        insert_parent_module(&conn, "mod-004");

        let mut block = sample_block("mod-004");
        block.id = "blk-status-rt".to_string();
        block.status = status_to_str(&BlockStatus::Pending).to_string();
        insert_block(&conn, &block).unwrap();

        let fetched = get_block(&conn, "blk-status-rt").unwrap().unwrap();
        assert_eq!(fetched.status, "pending");
        // Deserialize status string back to enum
        let status: BlockStatus = serde_json::from_str(&format!("\"{}\"", fetched.status)).unwrap();
        assert_eq!(status, BlockStatus::Pending);
    }

    /// LAB-01 — BlockType::Lab serializes / deserializes as "lab" string.
    /// (Smoke / re-export integration; canonical guarantee in core.)
    #[test]
    fn block_type_lab_serializes_as_lab() {
        let bt = BlockType::Lab;
        let json = serde_json::to_string(&bt).unwrap();
        assert_eq!(json, "\"lab\"", "BlockType::Lab must serialize as \"lab\"");

        let back: BlockType = serde_json::from_str("\"lab\"").unwrap();
        assert_eq!(back, BlockType::Lab, "round-trip back to Lab variant");
    }

    /// LAB-01 — block_type_to_str arm for Lab returns "lab".
    #[test]
    fn block_type_to_str_lab_arm() {
        assert_eq!(block_type_to_str(&BlockType::Lab), "lab");
    }

    /// update_block_payload advances status and replaces payload.
    #[test]
    fn update_block_payload_advances_status() {
        let conn = fresh_conn();
        insert_parent_module(&conn, "mod-005");

        let mut block = sample_block("mod-005");
        block.id = "blk-upd".to_string();
        block.status = "pending".to_string();
        insert_block(&conn, &block).unwrap();

        let new_payload = r##"{"markdown":"# Hello"}"##;
        update_block_payload(&conn, "blk-upd", BlockStatus::Ready, new_payload).unwrap();

        let updated = get_block(&conn, "blk-upd").unwrap().unwrap();
        assert_eq!(updated.status, "ready", "status must advance to ready");
        assert_eq!(updated.payload_json, new_payload, "payload_json must be updated");
    }
}
