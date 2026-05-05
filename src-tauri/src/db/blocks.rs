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

/// Convert BlockStatus to its string representation for DB storage.
pub fn status_to_str(s: &BlockStatus) -> &'static str {
    match s {
        BlockStatus::Pending => "pending",
        BlockStatus::Generating => "generating",
        BlockStatus::Ready => "ready",
        BlockStatus::Failed => "failed",
    }
}

/// Convert BlockType to its string representation for DB storage.
pub fn block_type_to_str(t: &BlockType) -> &'static str {
    match t {
        BlockType::Section => "section",
        BlockType::Text => "text",
        BlockType::Callout => "callout",
        BlockType::Quiz => "quiz",
        BlockType::FlashCards => "flash_cards",
    }
}

/// Insert a block row into module_blocks.
pub fn insert_block(conn: &Connection, b: &ModuleBlock) -> Result<()> {
    conn.execute(
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
    )?;
    Ok(())
}

/// Query all blocks for a module, ordered by ordering ASC.
pub fn list_blocks_by_module(conn: &Connection, module_id: &str) -> Result<Vec<ModuleBlock>> {
    let mut stmt = conn.prepare(
        "SELECT id, module_id, ordering, block_type, status,
                params_json, payload_json, source_anchors_json, metadata_json, retry_count,
                created_at, updated_at
         FROM module_blocks
         WHERE module_id = ?1
         ORDER BY ordering ASC",
    )?;
    let blocks = stmt
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
        })?
        .collect::<Result<Vec<ModuleBlock>>>()?;
    Ok(blocks)
}

/// Get a single block by ID. Returns None if not found.
pub fn get_block(conn: &Connection, block_id: &str) -> Result<Option<ModuleBlock>> {
    let mut stmt = conn.prepare(
        "SELECT id, module_id, ordering, block_type, status,
                params_json, payload_json, source_anchors_json, metadata_json, retry_count,
                created_at, updated_at
         FROM module_blocks
         WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([block_id], |row| {
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
    })?;
    match rows.next() {
        Some(Ok(block)) => Ok(Some(block)),
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
}

/// Update payload and status for a single block.
pub fn update_block_payload(
    conn: &Connection,
    id: &str,
    status: BlockStatus,
    payload_json: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE module_blocks SET status=?1, payload_json=?2, updated_at=datetime('now') WHERE id=?3",
        rusqlite::params![status_to_str(&status), payload_json, id],
    )?;
    Ok(())
}

/// Count blocks for a module.
pub fn count_blocks_by_module(conn: &Connection, module_id: &str) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM module_blocks WHERE module_id = ?1",
        [module_id],
        |r| r.get(0),
    )
}

/// Delete all blocks for a module. Returns rows affected.
/// Used by `regenerate_module` in 03-03.
pub fn delete_blocks_by_module(conn: &Connection, module_id: &str) -> Result<usize> {
    conn.execute(
        "DELETE FROM module_blocks WHERE module_id = ?1",
        [module_id],
    )
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

    /// Insert a parent module row so foreign key constraints are satisfied.
    fn insert_parent_module(conn: &Connection, module_id: &str) {
        // Insert learner profile -> track -> path -> module chain
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

    /// Serde test: serialized JSON must contain camelCase keys.
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

        // Verify round-trip with DB
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
