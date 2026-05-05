use crate::db::blocks::ModuleBlock;
use serde::{Deserialize, Serialize};

// ── IPC Request / Response structs ──
// All structs cross the Tauri IPC boundary and MUST use camelCase serde.

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateModuleBlocksRequest {
    pub module_id: String,
    pub track_id: String,
    pub module_title: String,
    pub objectives: Vec<String>,
    pub learner_level: String,
}

#[derive(Debug, Serialize, Deserialize)]
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

// ── Command stub signatures ──
// All return Err until Wave 2 (03-03) implements the real logic.

/// Generate (or return cached) blocks for a module.
/// Wave 2 (03-03 Task 1) implements the two-phase PagePlanner pipeline.
pub async fn generate_module_blocks(
    _req: GenerateModuleBlocksRequest,
) -> Result<GenerateModuleBlocksResult, String> {
    Err("Wave 2 (03-03) implements generate_module_blocks".to_string())
}

/// Return the cached blocks for a module (no LLM call).
/// Wave 2 (03-03 Task 1) implements the DB query.
pub async fn get_module_blocks(_module_id: String) -> Result<Vec<ModuleBlock>, String> {
    Err("Wave 2 (03-03) implements get_module_blocks".to_string())
}

/// Regenerate a single lesson block.
/// Wave 2 (03-03 Task 2) implements atomically.
pub async fn regenerate_lesson(_req: RegenerateLessonRequest) -> Result<ModuleBlock, String> {
    Err("Wave 2 (03-03) implements regenerate_lesson".to_string())
}

/// Regenerate all blocks for a module via a fresh PagePlanner pass.
/// Wave 2 (03-03 Task 2) implements atomically.
pub async fn regenerate_module(
    _req: RegenerateModuleRequest,
) -> Result<GenerateModuleBlocksResult, String> {
    Err("Wave 2 (03-03) implements regenerate_module".to_string())
}

/// Wrap a legacy modules.content markdown blob as a single section block.
/// Called when a module has no rows in module_blocks.
/// Wave 2 (03-03 Task 1) implements the INSERT logic.
pub fn wrap_legacy_content_as_block(
    _conn: &rusqlite::Connection,
    _module_id: &str,
) -> Result<Option<ModuleBlock>, String> {
    todo!("Wave 2 (03-03) implements wrap_legacy_content_as_block")
}

/// Internal: generate blocks or return cached result.
/// Used by tests to exercise the cache path without full Tauri state.
pub fn generate_module_blocks_inner(
    _conn: &rusqlite::Connection,
    _module_id: &str,
) -> Result<Vec<ModuleBlock>, String> {
    todo!("Wave 2 (03-03) implements generate_module_blocks_inner")
}

/// Internal: regenerate a single lesson block.
pub fn regenerate_lesson_inner(
    _conn: &rusqlite::Connection,
    _block_id: &str,
) -> Result<ModuleBlock, String> {
    todo!("Wave 2 (03-03) implements regenerate_lesson_inner")
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

    /// Serde test: GenerateModuleBlocksRequest serializes to camelCase.
    /// PASSES in Wave 0 because struct fields are fully declared (FIX-02 contract).
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
    /// PASSES in Wave 0 because struct fields are fully declared (FIX-02 contract).
    #[test]
    fn test_generate_blocks_result_camel_case() {
        let result = GenerateModuleBlocksResult { blocks: vec![] };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("blocks"), "must serialize blocks field");
    }

    /// Legacy wrap shim: DB with modules.content and zero module_blocks rows.
    /// Call wrap_legacy_content_as_block, assert exactly one section block inserted.
    /// FAILS in Wave 0 because wrap_legacy_content_as_block is a todo!() stub.
    #[test]
    fn legacy_wrap_shim() {
        // wrap_legacy_content_as_block panics with todo!() — test FAILS by panic.
        let _conn = fresh_conn();
        panic!("WAVE 2 STUB — implement wrap_legacy_content_as_block then assert section block inserted");
    }

    /// Cache hit: pre-seed 8 ready blocks, call generate_module_blocks_inner,
    /// assert NO LLM call and the 8 blocks returned.
    /// FAILS in Wave 0 because generate_module_blocks_inner is a todo!() stub.
    #[test]
    fn cached_blocks_returned_immediately() {
        // generate_module_blocks_inner panics with todo!() — test FAILS by panic.
        let _conn = fresh_conn();
        panic!("WAVE 2 STUB — implement generate_module_blocks_inner cache path then assert no LLM call");
    }

    /// Atomic lesson regeneration: pre-seed 8 blocks, regenerate one block,
    /// assert that block is replaced and the other 7 are untouched.
    /// FAILS in Wave 0 because regenerate_lesson_inner is a todo!() stub.
    #[test]
    fn regenerate_lesson_atomic() {
        // regenerate_lesson_inner panics with todo!() — test FAILS by panic.
        let _conn = fresh_conn();
        panic!("WAVE 2 STUB — implement regenerate_lesson_inner then assert atomic replacement");
    }

    /// Concurrency cap: drive 8 simulated sections, assert max concurrent LLM calls <= 3.
    /// FAILS in Wave 0 because the semaphore-limited dispatcher is not implemented.
    #[test]
    fn parallel_generation_semaphore_cap() {
        // No dispatcher yet — test FAILS by panic.
        panic!("WAVE 2 STUB — implement semaphore-limited parallel generator then assert concurrency <= 3");
    }
}
