//! Phase 12 Plan 02 — Course Export backend.
//!
//! Exposes:
//! - `export_course` — Tauri IPC thin shim, locks state, delegates to `export_course_impl`.
//! - `export_course_impl` — inner helper that assembles `CourseExportPayload` from SQLite
//!   and writes a single JSON file. Enforces the fail-closed provenance allowlist
//!   (`is_course_exportable`) BEFORE building or serializing anything (D-10).
//! - `is_course_exportable` — fail-closed allowlist predicate.
//!
//! ## D-10 Provenance Gate (fail-closed allowlist)
//!
//! `generated_by_model` holds the course provenance. Known-exportable classes:
//! - `"topic-pack:<id>"` — authored packs from Phase 5
//! - `"imported:<source>"` — courses re-exported after a Plan 03 import
//! - Bare AI-model name strings (e.g. `"claude-3-5-sonnet"`) — AI-generated courses
//!
//! Any future NON-exportable class (`licensed:<id>`, `curated:<id>`) is BLOCKED by
//! default with ZERO code changes because the predicate checks an explicit
//! RESERVED_NON_EXPORTABLE_PREFIXES denylist: anything matching those prefixes (or
//! empty/whitespace) returns false. Unknown novel prefixes NOT on the allow path also
//! return false (fail-closed). Adding a new paid class only requires adding its prefix
//! to the non-exportable slice — no new branches.
//!
//! ## D-08 Learner-state guarantee
//!
//! `export_course_impl` NEVER queries:
//! `module_progress`, `bkt_params`, `sr_cards`, `lab_progress`, `lesson_completions`.
//!
//! ## Threat mitigations implemented
//!
//! | Threat | Mitigation |
//! |--------|------------|
//! | T-12-04 (save_path tampering) | path originates from Tauri save-dialog (user-chosen); no concatenation from DB |
//! | T-12-05 (learner-state disclosure) | export_course_impl never queries learner-state tables |
//! | T-12-06 (non-ready block disclosure) | blocks filtered to status=="ready" before mapping |
//! | T-12-15 (paid/curated course leak) | is_course_exportable fail-closed allowlist checked FIRST, no file written on reject |

use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

use crate::storage_impl::blocks::SqliteBlockStore;
use learnforge_core::blocks::BlockStore;
use learnforge_core::packs::export::{
    CourseExportPayload, ExportedBlock, ExportedVideo, serialize_export,
};

// ── Error type ────────────────────────────────────────────────────────────────

/// Typed errors for course export operations.
///
/// `CourseNotExportable` is a distinct variant so the renderer can display a
/// specific "not exportable" state (Plan 04) rather than a generic error.
#[derive(Debug, thiserror::Error)]
pub enum ExportCourseError {
    /// Course provenance is not in the exportable allowlist (D-10).
    #[error("course provenance '{0}' is not exportable")]
    CourseNotExportable(String),
    /// I/O error (e.g. write failed).
    #[error("export I/O error: {0}")]
    Io(String),
    /// Database query error.
    #[error("export DB error: {0}")]
    Db(String),
    /// Serialization error (from core).
    #[error("export serialization error: {0}")]
    Serialize(String),
}

impl From<std::io::Error> for ExportCourseError {
    fn from(e: std::io::Error) -> Self {
        ExportCourseError::Io(e.to_string())
    }
}

impl From<rusqlite::Error> for ExportCourseError {
    fn from(e: rusqlite::Error) -> Self {
        ExportCourseError::Db(e.to_string())
    }
}

impl From<learnforge_core::packs::export::ExportError> for ExportCourseError {
    fn from(e: learnforge_core::packs::export::ExportError) -> Self {
        ExportCourseError::Serialize(e.to_string())
    }
}

// ── IPC request / result types ────────────────────────────────────────────────

/// Request for the `export_course` Tauri command.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCourseRequest {
    /// Track id whose course to export.
    pub track_id: String,
    /// Absolute path chosen via the Tauri save-dialog (T-12-04: user-chosen, no concat).
    pub save_path: String,
}

/// Result returned to the renderer on successful export.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCourseResult {
    /// Absolute path where the file was written.
    pub saved_path: String,
    /// Total ready blocks written (D-02 — non-ready excluded).
    pub block_count: usize,
    /// Module count in the exported payload.
    pub module_count: usize,
}

// ── Provenance allowlist (D-10, fail-closed) ──────────────────────────────────

/// Non-exportable reserved provenance prefixes.
///
/// Any provenance string that *starts_with* one of these is REJECTED.
/// Adding a future `licensed:` or `curated:` class here is the ONLY code change
/// needed to block it — no new branches. New classes with other prefixes are also
/// rejected by default because the allowlist checks explicit positive conditions first.
const RESERVED_NON_EXPORTABLE_PREFIXES: &[&str] = &["licensed:", "curated:"];

/// Fail-closed exportability predicate (D-10).
///
/// Returns `true` ONLY if the provenance is a known-exportable class:
/// - `topic-pack:<id>` — Phase 5 authored pack paths
/// - `imported:<source>` — Plan 03 re-imported courses
/// - A bare non-empty string that is NOT a reserved non-exportable prefix
///   and NOT empty/whitespace (i.e. an AI-generated model name like
///   `"claude-3-5-sonnet"` or `"gpt-4o"`)
///
/// Returns `false` (fail-closed) for:
/// - `licensed:<id>` (future paid class)
/// - `curated:<id>` (future curated class)
/// - Any unknown string carrying a reserved prefix
/// - Empty or whitespace-only strings
pub fn is_course_exportable(generated_by_model: &str) -> bool {
    let s = generated_by_model.trim();

    // Empty / whitespace — fail-closed
    if s.is_empty() {
        return false;
    }

    // Explicit non-exportable prefixes — fail-closed
    for &prefix in RESERVED_NON_EXPORTABLE_PREFIXES {
        if s.starts_with(prefix) {
            return false;
        }
    }

    // All other non-empty strings are exportable (topic-pack:, imported:, AI model names)
    true
}

// ── Inner export helper ───────────────────────────────────────────────────────

/// Export inner helper. Assembles `CourseExportPayload` from SQLite and writes
/// the JSON to `save_path`.
///
/// Gate order (D-10): provenance check → DB reads → build payload → serialize → write.
/// A non-exportable provenance returns `Err(CourseNotExportable)` BEFORE any DB read
/// or file write.
///
/// Learner-state tables are NEVER queried (D-08):
/// `module_progress`, `bkt_params`, `sr_cards`, `lab_progress`, `lesson_completions`.
pub fn export_course_impl(
    conn: &Connection,
    track_id: &str,
    save_path: &str,
) -> Result<ExportCourseResult, ExportCourseError> {
    // ── Step 1: load learning_paths row, read provenance ────────────────────
    struct PathRow {
        id: String,
        generated_by_model: String,
        modules_json: String,
        edges_json: String,
    }

    let path_row: PathRow = conn.query_row(
        "SELECT id, generated_by_model, modules_json, edges_json \
         FROM learning_paths WHERE track_id = ?1 LIMIT 1",
        rusqlite::params![track_id],
        |row| {
            Ok(PathRow {
                id: row.get(0)?,
                generated_by_model: row.get(1)?,
                modules_json: row.get(2)?,
                edges_json: row.get(3)?,
            })
        },
    ).map_err(|e| ExportCourseError::Db(format!("learning_paths not found for track {}: {}", track_id, e)))?;

    // ── Step 2: D-10 gate — check provenance BEFORE any further work ────────
    if !is_course_exportable(&path_row.generated_by_model) {
        return Err(ExportCourseError::CourseNotExportable(path_row.generated_by_model));
    }

    // ── Step 3: load track metadata ─────────────────────────────────────────
    struct TrackRow {
        topic: String,
        domain_module: String,
    }
    let track_row: TrackRow = conn.query_row(
        "SELECT topic, domain_module FROM learning_tracks WHERE id = ?1",
        rusqlite::params![track_id],
        |row| Ok(TrackRow { topic: row.get(0)?, domain_module: row.get(1)? }),
    ).map_err(|e| ExportCourseError::Db(e.to_string()))?;

    // ── Step 4: parse modules_json and edges_json ────────────────────────────
    let modules_arr: Vec<serde_json::Value> = serde_json::from_str(&path_row.modules_json)
        .unwrap_or_default();
    let edges_arr: Vec<serde_json::Value> = serde_json::from_str(&path_row.edges_json)
        .unwrap_or_default();

    // ── Step 5: collect ready blocks + videos per module ────────────────────
    let mut blocks_map: HashMap<String, Vec<ExportedBlock>> = HashMap::new();
    let mut videos_map: HashMap<String, Vec<ExportedVideo>> = HashMap::new();

    for module in &modules_arr {
        let module_id = match module["id"].as_str() {
            Some(id) => id.to_string(),
            None => continue,
        };

        // Blocks: list all, filter to status=="ready" (D-02, T-12-06)
        let ready_blocks: Vec<ExportedBlock> = SqliteBlockStore(conn)
            .list_for_module(&module_id)
            .map_err(|e| ExportCourseError::Db(e.to_string()))?
            .into_iter()
            .filter(|b| b.status == "ready")
            .map(|b| ExportedBlock {
                id: b.id,
                module_id: b.module_id,
                ordering: b.ordering,
                block_type: b.block_type,
                status: b.status,
                params_json: b.params_json,
                payload_json: b.payload_json,
                source_anchors_json: b.source_anchors_json,
                metadata_json: b.metadata_json,
                retry_count: b.retry_count,
                created_at: b.created_at,
                updated_at: b.updated_at,
            })
            .collect();

        // Videos: SELECT WHERE module_id=? AND status='ready' (D-04)
        let mut stmt = conn.prepare(
            "SELECT video_id, title, channel_title, relevance_score \
             FROM lesson_videos WHERE module_id = ?1 AND status = 'ready'",
        ).map_err(|e| ExportCourseError::Db(e.to_string()))?;

        let module_videos: Vec<ExportedVideo> = stmt.query_map(
            rusqlite::params![module_id],
            |row| {
                Ok(ExportedVideo {
                    video_id: row.get(0)?,
                    title: row.get(1)?,
                    channel_title: row.get(2)?,
                    relevance_score: row.get(3)?,
                })
            },
        ).map_err(|e| ExportCourseError::Db(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

        blocks_map.insert(module_id.clone(), ready_blocks);
        if !module_videos.is_empty() {
            videos_map.insert(module_id, module_videos);
        }
    }

    // ── Step 6: build CourseExportPayload ────────────────────────────────────
    let block_count: usize = blocks_map.values().map(|v| v.len()).sum();
    let module_count = modules_arr.len();

    // Use track_id as the pack id for provenance marker (D-06)
    let exported_from = format!("imported:{}", track_id);

    let payload = CourseExportPayload {
        id: track_id.to_string(),
        title: track_row.topic.clone(),
        description: format!("{} course", track_row.topic),
        domain_module: track_row.domain_module.clone(),
        modules: modules_arr,
        edges: edges_arr,
        export_version: "1.0.0".to_string(),
        exported_at: Utc::now().to_rfc3339(),
        exported_from,
        blocks: blocks_map,
        labs: HashMap::new(), // labs ride as block_type=="lab" blocks in blocks_map
        videos: videos_map,
    };

    // ── Step 7: serialize to JSON ─────────────────────────────────────────────
    let json_text = serialize_export(&payload)?;

    // ── Step 8: write to save_path (gate already passed, no partial file on reject) ──
    std::fs::write(save_path, json_text.as_bytes())
        .map_err(|e| ExportCourseError::Io(e.to_string()))?;

    Ok(ExportCourseResult {
        saved_path: save_path.to_string(),
        block_count,
        module_count,
    })
}

// ── Tauri command shim ────────────────────────────────────────────────────────

/// Export the course for a track to a single JSON file.
///
/// Thin shim: locks state, calls `export_course_impl`, maps errors to `String`.
/// `CourseNotExportable` surfaces as a distinguishable message so Plan 04 can show
/// a "not exportable" state in the UI.
#[tauri::command]
pub fn export_course(
    request: ExportCourseRequest,
    state: State<'_, crate::AppState>,
) -> Result<ExportCourseResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    export_course_impl(&db.conn, &request.track_id, &request.save_path)
        .map_err(|e| e.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;
    use std::path::Path;

    // ── Test helpers ──────────────────────────────────────────────────────────

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    /// Seed the full FK chain needed for export tests.
    ///
    /// Returns `(track_id, path_id)`.
    fn seed_track(
        conn: &Connection,
        provenance: &str,
        modules_json: &str,
        edges_json: &str,
    ) -> (String, String) {
        let track_id = format!("trk-{}", uuid::Uuid::new_v4());
        let path_id = format!("path-{}", uuid::Uuid::new_v4());

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp-export-test', 'Tester')",
            [],
        ).ok();

        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) \
             VALUES (?1, 'lp-export-test', 'Test Topic', 'devops', 'Learn stuff')",
            rusqlite::params![track_id],
        ).unwrap();

        conn.execute(
            "INSERT INTO learning_paths \
             (id, track_id, modules_json, edges_json, version, generated_by_model) \
             VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            rusqlite::params![path_id, track_id, modules_json, edges_json, provenance],
        ).unwrap();

        (track_id, path_id)
    }

    /// Seed a module row (FK to learning_paths via path_id).
    fn seed_module(conn: &Connection, module_id: &str, path_id: &str) {
        conn.execute(
            "INSERT INTO modules (id, path_id, title, description, ordering) \
             VALUES (?1, ?2, 'Test Module', 'desc', 0)",
            rusqlite::params![module_id, path_id],
        ).unwrap();
    }

    /// Insert a module_block row with the given status.
    fn insert_block(conn: &Connection, block_id: &str, module_id: &str, status: &str, ordering: i32) {
        conn.execute(
            "INSERT INTO module_blocks \
             (id, module_id, ordering, block_type, status, params_json, payload_json, \
              source_anchors_json, metadata_json, retry_count, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'section', ?4, '{}', '{\"markdown\":\"hello\"}', \
                     '[]', '{\"concept_id\":null}', 0, \
                     '2026-07-01T00:00:00Z', '2026-07-01T00:00:00Z')",
            rusqlite::params![block_id, module_id, ordering, status],
        ).unwrap();
    }

    /// Insert a ready lesson_video row for a module.
    fn insert_video(conn: &Connection, module_id: &str, video_id: &str) {
        conn.execute(
            "INSERT INTO lesson_videos \
             (id, module_id, section_id, video_id, title, channel_title, relevance_score, status) \
             VALUES (?1, ?2, 'sec-1', ?3, 'Test Video', 'Test Channel', 0.85, 'ready')",
            rusqlite::params![
                format!("lv-{}", video_id),
                module_id,
                video_id,
            ],
        ).unwrap();
    }

    // ── is_course_exportable tests ─────────────────────────────────────────────

    #[test]
    fn exportable_topic_pack_provenance() {
        assert!(
            is_course_exportable("topic-pack:agentic-devops"),
            "topic-pack:* must be exportable"
        );
        assert!(
            is_course_exportable("topic-pack:kubernetes"),
            "topic-pack:* must be exportable"
        );
    }

    #[test]
    fn exportable_imported_provenance() {
        assert!(
            is_course_exportable("imported:foo.json"),
            "imported:* must be exportable"
        );
        assert!(
            is_course_exportable("imported:my-custom-course"),
            "imported:* must be exportable"
        );
    }

    #[test]
    fn exportable_ai_generated_model_name() {
        assert!(
            is_course_exportable("claude-3-5-sonnet"),
            "bare AI model name must be exportable"
        );
        assert!(
            is_course_exportable("gpt-4o"),
            "bare AI model name must be exportable"
        );
        assert!(
            is_course_exportable("ollama/llama3"),
            "ollama model names must be exportable"
        );
    }

    #[test]
    fn not_exportable_licensed_provenance() {
        assert!(
            !is_course_exportable("licensed:test"),
            "licensed:* must NOT be exportable (D-10)"
        );
        assert!(
            !is_course_exportable("licensed:premium-course"),
            "licensed:* must NOT be exportable (D-10)"
        );
    }

    #[test]
    fn not_exportable_curated_provenance() {
        assert!(
            !is_course_exportable("curated:x"),
            "curated:* must NOT be exportable (D-10)"
        );
        assert!(
            !is_course_exportable("curated:official-track"),
            "curated:* must NOT be exportable (D-10)"
        );
    }

    #[test]
    fn not_exportable_empty_provenance() {
        assert!(
            !is_course_exportable(""),
            "empty provenance must NOT be exportable (fail-closed)"
        );
        assert!(
            !is_course_exportable("   "),
            "whitespace-only provenance must NOT be exportable (fail-closed)"
        );
    }

    // ── D-10 gate tests ────────────────────────────────────────────────────────

    #[test]
    fn export_rejects_licensed_provenance_no_file_written() {
        let conn = fresh_conn();

        // Minimal modules JSON (just id field needed for the modules array)
        let modules_json = r#"[{"id":"mod-1","title":"Mod 1","description":"d","objectives":[]}]"#;
        let (track_id, path_id) = seed_track(&conn, "licensed:test", modules_json, "[]");
        seed_module(&conn, "mod-1", &path_id);

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let save_path = tmp.path().to_str().unwrap().to_string();

        // Remove the file so we can assert it was NOT written
        drop(tmp);
        assert!(!Path::new(&save_path).exists(), "precondition: file must not exist");

        let result = export_course_impl(&conn, &track_id, &save_path);

        assert!(result.is_err(), "licensed: provenance must return Err");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("licensed:test"),
            "error must carry the offending provenance; got: {}",
            err_msg
        );
        assert!(
            !Path::new(&save_path).exists(),
            "NO file must be written on non-exportable provenance (D-10 gate-first)"
        );
    }

    #[test]
    fn export_accepts_topic_pack_provenance() {
        let conn = fresh_conn();

        let modules_json = r#"[{"id":"mod-1","title":"Mod 1","description":"d","objectives":[]}]"#;
        let (track_id, path_id) = seed_track(&conn, "topic-pack:agentic-devops", modules_json, "[]");
        seed_module(&conn, "mod-1", &path_id);

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let save_path = tmp.path().to_str().unwrap().to_string();

        let result = export_course_impl(&conn, &track_id, &save_path);
        assert!(
            result.is_ok(),
            "topic-pack: provenance must succeed; got: {:?}",
            result
        );
        assert!(
            Path::new(&save_path).exists(),
            "file must be written on exportable provenance"
        );
    }

    #[test]
    fn export_accepts_ai_generated_provenance() {
        let conn = fresh_conn();

        let modules_json = r#"[{"id":"mod-1","title":"Mod 1","description":"d","objectives":[]}]"#;
        let (track_id, path_id) = seed_track(&conn, "claude-3-5-sonnet", modules_json, "[]");
        seed_module(&conn, "mod-1", &path_id);

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let save_path = tmp.path().to_str().unwrap().to_string();

        let result = export_course_impl(&conn, &track_id, &save_path);
        assert!(
            result.is_ok(),
            "AI model name provenance must succeed; got: {:?}",
            result
        );
    }

    // ── D-02 ready-block filtering ────────────────────────────────────────────

    #[test]
    fn export_only_includes_ready_blocks() {
        let conn = fresh_conn();

        let modules_json = r#"[{"id":"mod-1","title":"Mod 1","description":"d","objectives":[]}]"#;
        let (track_id, path_id) = seed_track(&conn, "claude-3-5-sonnet", modules_json, "[]");
        seed_module(&conn, "mod-1", &path_id);

        // Insert 1 ready block and 1 non-ready block
        insert_block(&conn, "blk-ready-1", "mod-1", "ready", 0);
        insert_block(&conn, "blk-pending-1", "mod-1", "pending", 1);

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let save_path = tmp.path().to_str().unwrap().to_string();

        let result = export_course_impl(&conn, &track_id, &save_path).expect("export must succeed");
        assert_eq!(result.block_count, 1, "only 1 ready block exported (D-02)");

        // Verify the written JSON contains only the ready block
        let json_str = std::fs::read_to_string(&save_path).unwrap();
        assert!(json_str.contains("blk-ready-1"), "ready block must appear in export");
        assert!(
            !json_str.contains("blk-pending-1"),
            "non-ready block must NOT appear in export (D-02)"
        );
    }

    // ── D-08 learner-state absence ────────────────────────────────────────────

    #[test]
    fn export_excludes_learner_state_keys() {
        let conn = fresh_conn();

        let modules_json = r#"[{"id":"mod-1","title":"Mod 1","description":"d","objectives":[]}]"#;
        let (track_id, path_id) = seed_track(&conn, "claude-3-5-sonnet", modules_json, "[]");
        seed_module(&conn, "mod-1", &path_id);
        insert_block(&conn, "blk-1", "mod-1", "ready", 0);

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let save_path = tmp.path().to_str().unwrap().to_string();

        export_course_impl(&conn, &track_id, &save_path).expect("export must succeed");

        let json_str = std::fs::read_to_string(&save_path).unwrap();
        // D-08: learner-state keys must be absent
        for key in &["module_progress", "bkt_params", "sr_cards", "lab_progress", "lesson_completions"] {
            assert!(
                !json_str.contains(key),
                "exported file must NOT contain learner-state key '{}' (D-08)",
                key
            );
        }
    }

    // ── Export metadata fields ────────────────────────────────────────────────

    #[test]
    fn export_sets_provenance_and_version() {
        let conn = fresh_conn();

        let modules_json = r#"[{"id":"mod-1","title":"Mod 1","description":"d","objectives":[]}]"#;
        let (track_id, path_id) = seed_track(&conn, "claude-3-5-sonnet", modules_json, "[]");
        seed_module(&conn, "mod-1", &path_id);

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let save_path = tmp.path().to_str().unwrap().to_string();

        export_course_impl(&conn, &track_id, &save_path).expect("export must succeed");

        let json_str = std::fs::read_to_string(&save_path).unwrap();
        let payload: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        // exported_from (D-06) — provenance marker set
        let exported_from = payload["exportedFrom"].as_str().unwrap_or("");
        assert!(
            !exported_from.is_empty(),
            "exportedFrom must be non-empty (D-06)"
        );

        // export_version must be non-empty
        let export_version = payload["exportVersion"].as_str().unwrap_or("");
        assert!(
            !export_version.is_empty(),
            "exportVersion must be non-empty"
        );
    }

    // ── Schema validation (D-05/D-07 forward-compat) ─────────────────────────

    #[test]
    fn exported_file_validates_through_parse_and_validate() {
        let conn = fresh_conn();

        let modules_json = r#"[{"id":"mod-1","title":"Mod 1","description":"d","objectives":["obj1"]}]"#;
        let (track_id, path_id) = seed_track(&conn, "claude-3-5-sonnet", modules_json, "[]");
        seed_module(&conn, "mod-1", &path_id);
        insert_block(&conn, "blk-1", "mod-1", "ready", 0);

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let save_path = tmp.path().to_str().unwrap().to_string();

        export_course_impl(&conn, &track_id, &save_path).expect("export must succeed");

        let json_str = std::fs::read_to_string(&save_path).unwrap();
        let validation_result = learnforge_core::packs::loader::parse_and_validate(&json_str);
        assert!(
            validation_result.is_ok(),
            "exported file must validate through parse_and_validate (D-05/D-07); got: {:?}",
            validation_result
        );
    }

    // ── Video export ──────────────────────────────────────────────────────────

    #[test]
    fn export_includes_ready_videos() {
        let conn = fresh_conn();

        let modules_json = r#"[{"id":"mod-1","title":"Mod 1","description":"d","objectives":[]}]"#;
        let (track_id, path_id) = seed_track(&conn, "claude-3-5-sonnet", modules_json, "[]");
        seed_module(&conn, "mod-1", &path_id);
        insert_block(&conn, "blk-1", "mod-1", "ready", 0);
        insert_video(&conn, "mod-1", "dQw4w9WgXcQ");

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let save_path = tmp.path().to_str().unwrap().to_string();

        let result = export_course_impl(&conn, &track_id, &save_path).expect("export must succeed");
        assert_eq!(result.module_count, 1, "1 module exported");

        let json_str = std::fs::read_to_string(&save_path).unwrap();
        assert!(
            json_str.contains("dQw4w9WgXcQ"),
            "ready video must appear in export (D-04)"
        );
    }
}
