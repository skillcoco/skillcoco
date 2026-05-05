use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};

/// Block type taxonomy — serialized as snake_case strings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    Section,
    Text,
    Callout,
    Quiz,
    FlashCards,
}

/// Block generation/content status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BlockStatus {
    Pending,
    Generating,
    Ready,
    Failed,
}

/// Database row for module_blocks. Crosses the Tauri IPC boundary — must use camelCase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleBlock {
    pub id: String,
    pub module_id: String,
    pub ordering: i32,
    pub block_type: String,        // serialized BlockType (snake_case variant)
    pub status: String,            // serialized BlockStatus
    pub params_json: String,
    pub payload_json: String,
    pub source_anchors_json: String,
    pub metadata_json: String,
    pub retry_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// Insert a block row into module_blocks.
/// Wave 1 (03-02 Task 1) implements the SQL; stub panics so Wave 0 tests FAIL.
pub fn insert_block(_conn: &Connection, _block: &ModuleBlock) -> Result<()> {
    todo!("Wave 1 (03-02) implements insert_block")
}

/// Query all blocks for a module, ordered by ordering ASC.
/// Wave 1 (03-02 Task 1) implements the SQL; stub panics so Wave 0 tests FAIL.
pub fn list_blocks_by_module(_conn: &Connection, _module_id: &str) -> Result<Vec<ModuleBlock>> {
    todo!("Wave 1 (03-02) implements list_blocks_by_module")
}

/// Update payload and status for a single block.
/// Wave 1 (03-02 Task 1) implements the SQL; stub panics so Wave 0 tests FAIL.
pub fn update_block_payload(
    _conn: &Connection,
    _id: &str,
    _status: &str,
    _payload_json: &str,
) -> Result<()> {
    todo!("Wave 1 (03-02) implements update_block_payload")
}

#[cfg(test)]
mod tests {
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

    /// Serde test: serialized JSON must contain camelCase keys.
    /// This test FAILS in Wave 0 because insert_block calls todo!() before we can verify DB round-trip.
    #[test]
    fn test_module_block_camel_case() {
        let block = sample_block("mod-001");
        // Verify serde shape — this itself passes; but the test calls insert_block to trigger the
        // Wave 0 failure signal (insert_block is a todo stub).
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

        // Wave 0 FAIL trigger: insert_block panics with todo — test is marked FAIL.
        let conn = fresh_conn();
        insert_block(&conn, &block).unwrap();
    }

    /// CRUD test: insert then query by module_id, ordered by ordering.
    /// FAILS in Wave 0 because insert_block panics with todo!().
    #[test]
    fn block_insert_and_query() {
        let conn = fresh_conn();
        let block = sample_block("mod-002");

        // insert_block panics — test FAILS by panic (counts as test failure).
        insert_block(&conn, &block).unwrap();

        let blocks = list_blocks_by_module(&conn, "mod-002").unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id, block.id);
    }

    /// PACK-04 concept-graph forward-link contract (ROADMAP success criterion #7).
    ///
    /// After v004 migration, any inserted row must have metadata_json defaulting to
    /// {"concept_id": null} so Phase 2 PATH-02 can link without a schema change.
    ///
    /// FAILS in Wave 0 because v004 up() is a stub that does not create the table;
    /// GREEN in 03-02 Task 1 when the real DDL with DEFAULT '{"concept_id":null}' lands.
    #[test]
    fn test_module_blocks_metadata_default_concept_id_null() {
        let conn = fresh_conn();

        // v004 stub does not create the table — this INSERT will fail if the table is missing.
        // Even if the table existed, the DEFAULT is not set in Wave 0, so the assertion fails.
        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status)
             VALUES ('blk-001', 'mod-003', 0, 'section', 'pending')",
            [],
        )
        .expect("INSERT must succeed after v004 creates the table (fails in Wave 0 stub)");

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
            "metadata_json.concept_id must default to null"
        );
    }
}
