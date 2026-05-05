use crate::auth::AuthState;
use crate::db::blocks::{
    count_blocks_by_module, insert_block, list_blocks_by_module, ModuleBlock,
};
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── IPC Request / Response structs ──
// All structs cross the Tauri IPC boundary and MUST use camelCase serde.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateModuleBlocksRequest {
    pub module_id: String,
    pub track_id: String,
    pub module_title: String,
    pub objectives: Vec<String>,
    pub learner_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateModuleBlocksResult {
    pub blocks: Vec<ModuleBlock>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegenerateLessonRequest {
    pub block_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegenerateModuleRequest {
    pub module_id: String,
    pub track_id: String,
}

// ── Core helpers ──

/// Wrap a legacy modules.content markdown blob as a single section block.
///
/// Called on first open of a module that has zero rows in module_blocks.
/// Idempotent: returns Ok(None) without inserting if blocks already exist.
/// Emits metadata_json='{"concept_id": null}' — PACK-04 concept-graph forward-link.
pub fn wrap_legacy_content_as_block(
    conn: &rusqlite::Connection,
    module_id: &str,
) -> Result<Option<ModuleBlock>, String> {
    // Idempotent: if any blocks exist, do nothing
    let count = count_blocks_by_module(conn, module_id).map_err(|e| e.to_string())?;
    if count > 0 {
        return Ok(None);
    }

    // Read legacy content from modules.content column
    let content: Option<String> = conn
        .query_row(
            "SELECT content FROM modules WHERE id = ?1",
            [module_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(|e| e.to_string())?;

    let markdown = match content {
        Some(c) if !c.is_empty() => c,
        _ => return Ok(None), // no legacy content to wrap
    };

    let now = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "markdown": markdown,
        "wordCount": 0
    })
    .to_string();

    let block = ModuleBlock {
        id: uuid::Uuid::new_v4().to_string(),
        module_id: module_id.to_string(),
        ordering: 0,
        block_type: "section".to_string(),
        status: "ready".to_string(),
        params_json: "{}".to_string(), // empty params = legacy marker for ModuleView banner
        payload_json: payload,
        source_anchors_json: "[]".to_string(),
        metadata_json: r#"{"concept_id": null}"#.to_string(), // PACK-04 concept-graph forward-link
        retry_count: 0,
        created_at: now.clone(),
        updated_at: now,
    };

    insert_block(conn, &block).map_err(|e| e.to_string())?;
    Ok(Some(block))
}

/// Internal: generate blocks or return cached result.
///
/// Cache-hit path (Wave 1 / 03-02): if ALL rows are status=ready, return them immediately
/// without any LLM call. Mixed / empty paths are stubs for Wave 2 (03-03).
///
/// The DB lock is acquired and dropped BEFORE any async calls.
pub async fn generate_module_blocks_inner(
    db_lock: &std::sync::Mutex<crate::db::Database>,
    _auth: &AuthState,
    req: GenerateModuleBlocksRequest,
) -> Result<GenerateModuleBlocksResult, String> {
    // Acquire lock, do all sync DB work, drop lock before any .await
    let blocks_result = {
        let db = db_lock.lock().map_err(|e| e.to_string())?;
        let conn = &db.conn;

        // Cache check
        let existing = list_blocks_by_module(conn, &req.module_id).map_err(|e| e.to_string())?;

        if !existing.is_empty() {
            let all_ready = existing.iter().all(|b| b.status == "ready");
            if all_ready {
                // PACK-04 hot path: return immediately, no LLM call
                return Ok(GenerateModuleBlocksResult { blocks: existing });
            }
            // Mixed states: return existing + resume pending (03-03 implements)
            return Ok(GenerateModuleBlocksResult { blocks: existing });
        }

        // No blocks: try legacy wrap shim
        match wrap_legacy_content_as_block(conn, &req.module_id)? {
            Some(legacy_block) => {
                return Ok(GenerateModuleBlocksResult {
                    blocks: vec![legacy_block],
                });
            }
            None => {}
        }

        // Truly empty module — 03-03 implements PagePlanner dispatch
        Vec::<ModuleBlock>::new()
    };
    // DB lock dropped here before any .await

    // 03-03 picks up here for fresh PagePlanner + parallel section generation
    if blocks_result.is_empty() {
        return Err(
            "Wave 2 (03-03) implements fresh module generation via PagePlanner".to_string(),
        );
    }

    Ok(GenerateModuleBlocksResult {
        blocks: blocks_result,
    })
}

// ── Tauri commands ──

/// Return cached blocks for a module (no LLM call).
#[tauri::command]
pub async fn get_module_blocks(
    module_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ModuleBlock>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    list_blocks_by_module(&db.conn, &module_id).map_err(|e| e.to_string())
}

/// Generate (or return cached) blocks for a module.
#[tauri::command]
pub async fn generate_module_blocks(
    req: GenerateModuleBlocksRequest,
    state: State<'_, AppState>,
    auth: State<'_, AuthState>,
) -> Result<GenerateModuleBlocksResult, String> {
    generate_module_blocks_inner(&state.db, &auth, req).await
}

/// Regenerate a single lesson block — Wave 2 (03-03) implements.
#[tauri::command]
pub async fn regenerate_lesson(
    _req: RegenerateLessonRequest,
    _state: State<'_, AppState>,
    _auth: State<'_, AuthState>,
) -> Result<ModuleBlock, String> {
    Err("Wave 2 (03-03) implements regenerate_lesson".to_string())
}

/// Regenerate all blocks for a module via a fresh PagePlanner pass — Wave 2 (03-03) implements.
#[tauri::command]
pub async fn regenerate_module(
    _req: RegenerateModuleRequest,
    _state: State<'_, AppState>,
    _auth: State<'_, AuthState>,
) -> Result<GenerateModuleBlocksResult, String> {
    Err("Wave 2 (03-03) implements regenerate_module".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::blocks::{insert_block, BlockStatus};
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

    /// Insert the minimal parent rows (profile -> track -> path -> module) so FK constraints pass.
    fn seed_module(conn: &Connection, module_id: &str, legacy_content: Option<&str>) {
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
            "INSERT OR IGNORE INTO modules (id, path_id, title, content) VALUES (?1, 'path-test', 'Test Module', ?2)",
            rusqlite::params![module_id, legacy_content],
        )
        .unwrap();
    }

    /// Serde test: GenerateModuleBlocksRequest serializes to camelCase.
    #[test]
    fn test_generate_blocks_request_camel_case() {
        let req = GenerateModuleBlocksRequest {
            module_id: "mod-1".to_string(),
            track_id: "trk-1".to_string(),
            module_title: "Kubernetes Pods".to_string(),
            objectives: vec!["Understand pods".to_string()],
            learner_level: "beginner".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("moduleId"), "must serialize to moduleId");
        assert!(json.contains("trackId"), "must serialize to trackId");
        assert!(json.contains("moduleTitle"), "must serialize to moduleTitle");
        assert!(json.contains("learnerLevel"), "must serialize to learnerLevel");
    }

    /// Serde test: GenerateModuleBlocksResult serializes to camelCase.
    #[test]
    fn test_generate_blocks_result_camel_case() {
        let result = GenerateModuleBlocksResult { blocks: vec![] };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("blocks"), "must serialize blocks field");
    }

    /// Legacy wrap shim: DB has modules.content="# Legacy", zero module_blocks rows.
    /// Call wrap_legacy_content_as_block, assert exactly one section block inserted.
    #[test]
    fn legacy_wrap_shim() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-legacy", Some("# Legacy Content"));

        let result = wrap_legacy_content_as_block(&conn, "mod-legacy").unwrap();
        assert!(result.is_some(), "must return Some(block) when legacy content exists");

        let block = result.unwrap();
        assert_eq!(block.block_type, "section", "must create section block");
        assert_eq!(block.status, "ready", "legacy block must be status=ready");
        assert_eq!(block.params_json, "{}", "params_json must be '{{}}' (legacy marker)");

        // payload_json must contain the legacy markdown verbatim
        let payload: serde_json::Value = serde_json::from_str(&block.payload_json).unwrap();
        assert!(
            payload["markdown"].as_str().unwrap().contains("# Legacy Content"),
            "payload_json must contain legacy markdown"
        );

        // Assert exactly 1 row in module_blocks
        let count = count_blocks_by_module(&conn, "mod-legacy").unwrap();
        assert_eq!(count, 1, "exactly 1 block must exist after wrap");
    }

    /// legacy_wrap_idempotent: calling wrap twice returns Ok(None) second time; only 1 row.
    #[test]
    fn legacy_wrap_idempotent() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-idem", Some("# Idempotent Test"));

        let first = wrap_legacy_content_as_block(&conn, "mod-idem").unwrap();
        assert!(first.is_some(), "first call must return Some");

        let second = wrap_legacy_content_as_block(&conn, "mod-idem").unwrap();
        assert!(second.is_none(), "second call must return None (already wrapped)");

        let count = count_blocks_by_module(&conn, "mod-idem").unwrap();
        assert_eq!(count, 1, "exactly 1 row must exist after two wrap calls");
    }

    /// Cache hit: pre-seed 8 ready blocks, call generate_module_blocks_inner,
    /// assert blocks returned and NO LLM call made (ai_request_call_count == 0).
    ///
    /// The cached-fetch path returns immediately when all blocks have status=ready.
    #[tokio::test]
    async fn cached_blocks_returned_immediately() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        seed_module_on_conn(&conn, "mod-cached", None);

        // Pre-seed 8 ready blocks
        for i in 0..8i32 {
            let block = ModuleBlock {
                id: format!("blk-{}", i),
                module_id: "mod-cached".to_string(),
                ordering: i,
                block_type: "section".to_string(),
                status: "ready".to_string(),
                params_json: "{}".to_string(),
                payload_json: format!("{{\"markdown\":\"Section {}\"}}", i),
                source_anchors_json: "[]".to_string(),
                metadata_json: r#"{"concept_id": null}"#.to_string(),
                retry_count: 0,
                created_at: "2026-05-05T00:00:00Z".to_string(),
                updated_at: "2026-05-05T00:00:00Z".to_string(),
            };
            insert_block(&conn, &block).unwrap();
        }

        // Wrap in Mutex<Database> for generate_module_blocks_inner
        let db = wrap_conn_in_db_mutex(conn);
        let auth_dir = tempfile::tempdir().unwrap();
        let auth = crate::auth::AuthState::new(&auth_dir.path().to_path_buf());

        let req = GenerateModuleBlocksRequest {
            module_id: "mod-cached".to_string(),
            track_id: "trk-test".to_string(),
            module_title: "Test".to_string(),
            objectives: vec![],
            learner_level: "beginner".to_string(),
        };

        let result = generate_module_blocks_inner(&db, &auth, req).await.unwrap();
        assert_eq!(result.blocks.len(), 8, "must return all 8 cached blocks");
        // All blocks should be ready — no LLM was called (if it were, it would Err
        // since auth has no credentials, and the result would be Err not Ok)
        assert!(
            result.blocks.iter().all(|b| b.status == "ready"),
            "all returned blocks must be status=ready"
        );
    }

    /// get_module_blocks_returns_ordered: pre-seed 3 blocks with ordering 2, 0, 1;
    /// list_blocks_by_module returns them in ASC ordering.
    #[test]
    fn get_module_blocks_returns_ordered() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-ord", None);

        let orderings = [2i32, 0, 1];
        for (i, ord) in orderings.iter().enumerate() {
            let block = ModuleBlock {
                id: format!("blk-ord-{}", i),
                module_id: "mod-ord".to_string(),
                ordering: *ord,
                block_type: "section".to_string(),
                status: "ready".to_string(),
                params_json: "{}".to_string(),
                payload_json: "{}".to_string(),
                source_anchors_json: "[]".to_string(),
                metadata_json: r#"{"concept_id": null}"#.to_string(),
                retry_count: 0,
                created_at: "2026-05-05T00:00:00Z".to_string(),
                updated_at: "2026-05-05T00:00:00Z".to_string(),
            };
            insert_block(&conn, &block).unwrap();
        }

        let blocks = list_blocks_by_module(&conn, "mod-ord").unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].ordering, 0, "first block must have ordering=0");
        assert_eq!(blocks[1].ordering, 1, "second block must have ordering=1");
        assert_eq!(blocks[2].ordering, 2, "third block must have ordering=2");
    }

    /// Atomic lesson regeneration stub — Wave 2 (03-03) implements.
    #[test]
    fn regenerate_lesson_atomic() {
        panic!("WAVE 2 STUB — implement regenerate_lesson_inner then assert atomic replacement");
    }

    /// Semaphore cap stub — Wave 2 (03-03) implements.
    #[test]
    fn parallel_generation_semaphore_cap() {
        panic!("WAVE 2 STUB — implement semaphore-limited parallel generator then assert concurrency <= 3");
    }

    // ── Test helpers ──

    fn seed_module_on_conn(conn: &Connection, module_id: &str, legacy_content: Option<&str>) {
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
            "INSERT OR IGNORE INTO modules (id, path_id, title, content) VALUES (?1, 'path-test', 'Test Module', ?2)",
            rusqlite::params![module_id, legacy_content],
        )
        .unwrap();
    }

    /// Wrap a raw Connection into a Mutex<Database> for testing generate_module_blocks_inner.
    fn wrap_conn_in_db_mutex(conn: Connection) -> std::sync::Mutex<crate::db::Database> {
        std::sync::Mutex::new(crate::db::Database { conn })
    }
}
