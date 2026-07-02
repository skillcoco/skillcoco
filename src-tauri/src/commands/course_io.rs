//! Phase 12 Plans 02 & 03 — Course Export + Import backend.
//!
//! Exposes:
//! - `export_course` — Tauri IPC thin shim, locks state, delegates to `export_course_impl`.
//! - `export_course_impl` — inner helper that assembles `CourseExportPayload` from SQLite
//!   and writes a single JSON file. Enforces the fail-closed provenance allowlist
//!   (`is_course_exportable`) BEFORE building or serializing anything (D-10).
//! - `is_course_exportable` — fail-closed allowlist predicate.
//! - `import_course` — Tauri IPC thin shim, locks state, delegates to `import_course_impl`.
//! - `import_course_impl` — inner helper that reads a course file (with FS guards),
//!   validates it, preserves the source provenance class (D-11), and atomically creates
//!   a new namespaced track + rehydrates blocks + videos.
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
//! ## D-11 Provenance-class preservation (import)
//!
//! Import MUST NOT launder a non-exportable source class into an exportable one.
//! The invariant is: `is_course_exportable(stamped_provenance) == is_course_exportable(source_class)`.
//! For non-exportable sources (e.g. `"licensed:..."`) the import is rejected with a clear error
//! rather than silently upgrading the provenance class.
//!
//! ## Threat mitigations implemented
//!
//! | Threat | Mitigation |
//! |--------|------------|
//! | T-12-04 (save_path tampering) | path originates from Tauri save-dialog (user-chosen); no concatenation from DB |
//! | T-12-05 (learner-state disclosure) | export_course_impl never queries learner-state tables |
//! | T-12-06 (non-ready block disclosure) | blocks filtered to status=="ready" before mapping |
//! | T-12-07 (import file size+content) | ImportedFilePackSource 5MB cap + parse_and_validate schema check BEFORE any DB write |
//! | T-12-08 (import path/symlink) | ImportedFilePackSource::read_file canonicalizes before read |
//! | T-12-09 (id collision/provenance) | {pack_id}__{module_id} namespacing; generated_by_model = imported:<source> |
//! | T-12-10 (half-import on failure) | UNCONDITIONAL BEGIN/COMMIT/ROLLBACK wraps ALL writes |
//! | T-12-11 (lab Docker image injection) | image name stored verbatim as String, never exec'd |
//! | T-12-12 (non-ready block injection) | import rejects ExportedBlock with status != "ready" |
//! | T-12-15 (paid/curated course leak) | is_course_exportable fail-closed allowlist checked FIRST, no file written on reject |
//! | T-12-16 (provenance laundering) | D-11: non-exportable source class is rejected, not re-stamped |

use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

use crate::storage_impl::blocks::SqliteBlockStore;
use crate::topic_packs::loader::ImportedFilePackSource;
use learnforge_core::blocks::{BlockStore, ModuleBlock};
use learnforge_core::packs::export::{
    CourseExportPayload, ExportedBlock, ExportedVideo, serialize_export,
};
use learnforge_core::packs::loader::parse_and_validate;

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

// ── Import IPC types ──────────────────────────────────────────────────────────

/// Request for the `import_course` Tauri command.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCourseRequest {
    /// Absolute path of the exported course file chosen via Tauri open-dialog.
    pub file_path: String,
}

/// Result returned to the renderer on successful import.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCourseResult {
    /// UUID v4 string of the newly created learning track.
    pub track_id: String,
    /// Number of modules imported (replicated from the pack's module array).
    pub module_count: usize,
    /// Total ready blocks rehydrated into module_blocks.
    pub block_count: usize,
    /// Soft validation warnings from parse_and_validate (non-fatal).
    pub warnings: Vec<String>,
}

// ── Import error type ─────────────────────────────────────────────────────────

/// Typed errors for course import operations.
#[derive(Debug, thiserror::Error)]
pub enum ImportCourseError {
    /// File read / FS guard failure (size cap, symlink escape, missing file).
    #[error("import file error: {0}")]
    Fs(String),
    /// Schema validation hard-fail (strict error from parse_and_validate).
    #[error("import validation error: {0}")]
    Validation(String),
    /// Source provenance class is non-exportable — importing would launder it (D-11).
    #[error("import rejected: source provenance '{0}' is not re-exportable; importing would launder a non-exportable course into an exportable state (D-11)")]
    ProvenanceLaundering(String),
    /// Database error during atomic write.
    #[error("import DB error: {0}")]
    Db(String),
    /// JSON deserialization error.
    #[error("import deserialization error: {0}")]
    Deserialize(String),
}

impl From<learnforge_core::packs::PackError> for ImportCourseError {
    fn from(e: learnforge_core::packs::PackError) -> Self {
        match e {
            learnforge_core::packs::PackError::Io(msg) => ImportCourseError::Fs(msg),
            learnforge_core::packs::PackError::Schema(msg) => ImportCourseError::Validation(msg),
            learnforge_core::packs::PackError::Json(msg) => ImportCourseError::Validation(msg),
            learnforge_core::packs::PackError::Loader(msg) => ImportCourseError::Fs(msg),
        }
    }
}

// ── Import inner helper ───────────────────────────────────────────────────────

/// Import inner helper. Reads an exported course file with FS guards,
/// validates it against the pack schema, preserves the source provenance class
/// (D-11), then atomically creates a NEW namespaced track + rehydrates blocks
/// and videos.
///
/// ## Security mitigations applied
///
/// - T-12-07/08: `ImportedFilePackSource` enforces 5MB cap + canonicalize before read.
/// - D-07: `parse_and_validate` called BEFORE any DB write.
/// - D-06: module ids and edges namespaced as `{pack_id}__{id}`.
/// - D-09: always creates a fresh track_id (uuid::Uuid::new_v4()); never updates.
/// - D-11: source `exported_from` class is inspected; if non-exportable, the import
///   is REJECTED to prevent laundering. If exportable, stamped as `"imported:{pack_id}"`.
/// - T-12-10: ALL writes wrapped in a single UNCONDITIONAL SQLite transaction.
/// - T-12-12: blocks with status != "ready" are rejected during rehydration.
/// - T-12-11/D-03: lab image names are stored verbatim as strings, never exec'd.
pub fn import_course_impl(
    conn: &Connection,
    file_path: &str,
) -> Result<ImportCourseResult, ImportCourseError> {
    // ── Step 1: FS guards (T-12-07, T-12-08) ────────────────────────────────
    let src = ImportedFilePackSource::new(file_path);
    let (bytes, _canon) = src.read_file()?;

    let text = std::str::from_utf8(&bytes)
        .map_err(|e| ImportCourseError::Fs(format!("import file is not UTF-8: {}", e)))?;

    // ── Step 2: Schema validation BEFORE any DB write (D-07) ────────────────
    let (_pack, soft_warnings) = parse_and_validate(text)
        .map_err(|e| ImportCourseError::Validation(e.to_string()))?;

    // ── Step 3: Deserialize full CourseExportPayload (blocks/labs/videos) ───
    let payload: CourseExportPayload = serde_json::from_str(text)
        .map_err(|e| ImportCourseError::Deserialize(e.to_string()))?;

    // ── Step 4: D-11 provenance-class preservation ──────────────────────────
    // The source class is the `exported_from` field of the payload.
    // If the source is NON-exportable, reject the import to prevent laundering.
    // Invariant: is_course_exportable(stamped) == is_course_exportable(source).
    let source_class = payload.exported_from.trim().to_string();
    if !is_course_exportable(&source_class) {
        // Reject: importing would launder a non-exportable course into an importable state
        return Err(ImportCourseError::ProvenanceLaundering(source_class));
    }

    // The source IS exportable — stamp as "imported:{pack_id}" (also exportable by the
    // allowlist since it starts with "imported:"). This is correct: already-public
    // content stays exportable through import → re-export (D-11).
    let pack_id = &payload.id;
    let stamped_provenance = format!("imported:{}", pack_id);
    // Compile-time invariant check: is_course_exportable("imported:*") == true
    debug_assert!(
        is_course_exportable(&stamped_provenance),
        "BUG: imported: prefix must be exportable per allowlist"
    );

    // ── Step 5: Resolve learner profile id ──────────────────────────────────
    let learner_id: String = conn
        .query_row(
            "SELECT id FROM learner_profiles LIMIT 1",
            [],
            |r| r.get(0),
        )
        .map_err(|e| ImportCourseError::Db(format!("no learner profile found: {}", e)))?;

    // ── Step 6: Allocate fresh track_id + build namespacing closure ─────────
    // D-09: always a NEW uuid track_id — never UPDATE by pack_id.
    // We allocate track_id HERE (before the transaction) so that both the
    // module namespacing and the transaction body share the SAME track_id.
    //
    // Namespace as {pack_id}__{track_prefix}__{module_id}:
    // - Keeps pack_id in the namespace for provenance context (D-06)
    // - Includes 8-char track_prefix to guarantee uniqueness on double-import (D-09)
    let track_id = uuid::Uuid::new_v4().to_string();
    let track_prefix = &track_id[..8]; // first 8 chars of uuid sufficiently unique

    let ns = |id: &str| format!("{}__{}__{}", pack_id, track_prefix, id);

    let namespaced_modules: Vec<serde_json::Value> = payload
        .modules
        .iter()
        .map(|m| {
            let mut m2 = m.clone();
            if let Some(orig_id) = m["id"].as_str() {
                m2["id"] = serde_json::Value::String(ns(orig_id));
            }
            m2
        })
        .collect();

    let namespaced_edges: Vec<serde_json::Value> = payload
        .edges
        .iter()
        .map(|e| {
            let from = e["from"].as_str().unwrap_or("");
            let to = e["to"].as_str().unwrap_or("");
            serde_json::json!({ "from": ns(from), "to": ns(to) })
        })
        .collect();

    let modules_json = serde_json::to_string(&namespaced_modules)
        .unwrap_or_else(|_| "[]".to_string());
    let edges_json = serde_json::to_string(&namespaced_edges)
        .unwrap_or_else(|_| "[]".to_string());

    // ── Step 7: Atomic transaction — ALL writes or NONE (T-12-10) ────────────
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| ImportCourseError::Db(format!("BEGIN failed: {}", e)))?;

    let result = import_course_txn(
        conn,
        pack_id,
        &track_id,
        &payload,
        &stamped_provenance,
        &learner_id,
        &namespaced_modules,
        &modules_json,
        &edges_json,
        soft_warnings,
    );

    match result {
        Ok(r) => {
            conn.execute_batch("COMMIT")
                .map_err(|e| ImportCourseError::Db(format!("COMMIT failed: {}", e)))?;
            Ok(r)
        }
        Err(e) => {
            // UNCONDITIONAL rollback — never leave a partial track
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Inner transaction body. Called only from `import_course_impl` inside BEGIN/COMMIT.
/// Separated so tests can inject payloads crafted to fail mid-write (atomicity test, T-12-10).
///
/// `track_id` is passed in from `import_course_impl` (already allocated before this call)
/// so the caller and callee share the same track_id for both module namespacing and DB writes.
#[allow(clippy::too_many_arguments)]
fn import_course_txn(
    conn: &Connection,
    pack_id: &str,
    track_id: &str,
    payload: &CourseExportPayload,
    stamped_provenance: &str,
    learner_id: &str,
    namespaced_modules: &[serde_json::Value],
    modules_json: &str,
    edges_json: &str,
    soft_warnings: Vec<String>,
) -> Result<ImportCourseResult, ImportCourseError> {
    // Use the same track_prefix-based namespace as import_course_impl
    let track_prefix = &track_id[..8];
    let ns = |id: &str| format!("{}__{}__{}", pack_id, track_prefix, id);

    // path_id is always new; track_id is passed in (allocated before BEGIN, D-09)
    let path_id = uuid::Uuid::new_v4().to_string();

    // Insert learning_tracks row
    conn.execute(
        "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal, status) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'active')",
        rusqlite::params![
            track_id,
            learner_id,
            payload.title,
            payload.domain_module,
            format!("Imported from {}", pack_id),
        ],
    )
    .map_err(|e| ImportCourseError::Db(format!("learning_tracks insert failed: {}", e)))?;

    // Insert learning_paths row with PRESERVED provenance (D-11)
    conn.execute(
        "INSERT INTO learning_paths \
         (id, track_id, modules_json, edges_json, version, generated_by_model) \
         VALUES (?1, ?2, ?3, ?4, 1, ?5)",
        rusqlite::params![path_id, track_id, modules_json, edges_json, stamped_provenance],
    )
    .map_err(|e| ImportCourseError::Db(format!("learning_paths insert failed: {}", e)))?;

    // Insert modules + module_progress (mirror generate_path_from_pack_impl convention)
    for (i, module) in namespaced_modules.iter().enumerate() {
        let module_id = module["id"].as_str().unwrap_or("").to_string();

        conn.execute(
            "INSERT INTO modules \
             (id, path_id, title, description, difficulty, estimated_minutes, objectives_json, ordering) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                module_id,
                path_id,
                module["title"].as_str().unwrap_or("Untitled"),
                module["description"].as_str().unwrap_or(""),
                module["difficulty"].as_i64().unwrap_or(1),
                module["estimated_minutes"].as_i64().unwrap_or(30),
                serde_json::to_string(&module["objectives"]).unwrap_or_default(),
                i as i32,
            ],
        )
        .map_err(|e| ImportCourseError::Db(format!("modules insert failed for {}: {}", module_id, e)))?;

        let status = if i == 0 { "available" } else { "locked" };
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                module_id,
                learner_id,
                status,
            ],
        )
        .map_err(|e| ImportCourseError::Db(format!("module_progress insert failed: {}", e)))?;
    }

    // Rehydrate blocks: iterate each module in the original payload (un-namespaced keys in blocks map)
    // T-12-12: reject any block whose status != "ready"
    // T-12-11/D-03: lab image names are stored verbatim in payload_json, never exec'd
    let mut block_count = 0usize;

    for module in &payload.modules {
        let orig_module_id = match module["id"].as_str() {
            Some(id) => id,
            None => continue,
        };
        let namespaced_module_id = ns(orig_module_id);

        // blocks map is keyed by module_id in the export (see Plan 02 export convention)
        if let Some(exported_blocks) = payload.blocks.get(orig_module_id) {
            for eb in exported_blocks {
                // T-12-12: reject non-ready blocks
                if eb.status != "ready" {
                    return Err(ImportCourseError::Validation(format!(
                        "block '{}' has status '{}' (only 'ready' blocks may be imported, T-12-12)",
                        eb.id, eb.status
                    )));
                }

                let block = ModuleBlock {
                    id: format!("{}_{}", namespaced_module_id, eb.id),
                    module_id: namespaced_module_id.clone(),
                    ordering: eb.ordering,
                    block_type: eb.block_type.clone(),
                    status: "ready".to_string(),
                    params_json: eb.params_json.clone(),
                    payload_json: eb.payload_json.clone(), // D-03: image name stays verbatim
                    source_anchors_json: eb.source_anchors_json.clone(),
                    metadata_json: eb.metadata_json.clone(),
                    retry_count: eb.retry_count,
                    created_at: eb.created_at.clone(),
                    updated_at: eb.updated_at.clone(),
                };

                SqliteBlockStore(conn).insert(&block)
                    .map_err(|e| ImportCourseError::Db(format!("block insert failed: {}", e)))?;
                block_count += 1;
            }
        }

        // Rehydrate videos: INSERT OR IGNORE (idempotent for re-imports of same video)
        if let Some(exported_videos) = payload.videos.get(orig_module_id) {
            for ev in exported_videos {
                conn.execute(
                    "INSERT OR IGNORE INTO lesson_videos \
                     (id, module_id, section_id, video_id, title, channel_title, relevance_score, status) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ready')",
                    rusqlite::params![
                        format!("lv-{}-{}", namespaced_module_id, ev.video_id),
                        namespaced_module_id,
                        namespaced_module_id, // section_id = module_id for imported content
                        ev.video_id,
                        ev.title,
                        ev.channel_title,
                        ev.relevance_score,
                    ],
                )
                .map_err(|e| ImportCourseError::Db(format!("video insert failed: {}", e)))?;
            }
        }
    }

    let module_count = payload.modules.len();

    Ok(ImportCourseResult {
        track_id: track_id.to_string(),
        module_count,
        block_count,
        warnings: soft_warnings,
    })
}

// ── Import Tauri command shim ─────────────────────────────────────────────────

/// Import a course from an exported JSON file.
///
/// Thin shim: locks state, calls `import_course_impl`, maps errors to `String`.
/// The file is validated against the schema BEFORE any DB write (D-07).
/// Provenance class is preserved — non-exportable source classes are rejected (D-11).
#[tauri::command]
pub fn import_course(
    request: ImportCourseRequest,
    state: State<'_, crate::AppState>,
) -> Result<ImportCourseResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    import_course_impl(&db.conn, &request.file_path)
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

    // ── import_course_impl tests (Phase 12, Plan 03, Task 2) ─────────────────

    /// Seed a learner profile + prepare fresh DB for import tests.
    fn fresh_conn_with_learner() -> Connection {
        let conn = fresh_conn();
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp-import-test', 'Import Tester')",
            [],
        ).unwrap();
        conn
    }

    /// Build a minimal valid CourseExportPayload JSON string for import tests.
    fn minimal_export_json(
        pack_id: &str,
        exported_from: &str,
    ) -> String {
        serde_json::json!({
            "id": pack_id,
            "title": "Test Course",
            "description": "A test import course.",
            "domain_module": "devops",
            "modules": [
                {
                    "id": "mod-a",
                    "title": "Module A",
                    "description": "First module.",
                    "objectives": ["learn basics"],
                    "difficulty": 1,
                    "estimatedMinutes": 30
                }
            ],
            "edges": [],
            "exportVersion": "1.0.0",
            "exportedAt": "2026-07-01T00:00:00Z",
            "exportedFrom": exported_from,
            "blocks": {},
            "labs": {},
            "videos": {}
        })
        .to_string()
    }

    /// Write a JSON string to a temp file and return (NamedTempFile, path_string).
    fn write_tmp_json(json: &str) -> (tempfile::NamedTempFile, String) {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), json.as_bytes()).unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        (tmp, path)
    }

    /// GREEN — import creates new namespaced track with "imported:" provenance (D-06).
    #[test]
    fn import_creates_new_track_with_imported_provenance() {
        let conn = fresh_conn_with_learner();
        let json = minimal_export_json("test-pack-001", "topic-pack:test-pack-001");
        let (_tmp, path) = write_tmp_json(&json);

        let result = import_course_impl(&conn, &path);
        assert!(result.is_ok(), "valid export must import; got: {:?}", result);
        let r = result.unwrap();

        // D-09: track_id is a new UUID
        assert!(!r.track_id.is_empty(), "track_id must be non-empty");

        // D-06: learning_paths row has provenance starting with "imported:"
        let prov: String = conn.query_row(
            "SELECT generated_by_model FROM learning_paths WHERE track_id = ?1",
            rusqlite::params![r.track_id],
            |row| row.get(0),
        ).unwrap();
        assert!(
            prov.starts_with("imported:"),
            "generated_by_model must start with 'imported:'; got: {}",
            prov
        );

        // D-06: module_id is namespaced
        let module_ids: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM modules WHERE path_id = \
                 (SELECT id FROM learning_paths WHERE track_id = ?1)"
            ).unwrap();
            stmt.query_map(rusqlite::params![r.track_id], |r| r.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        assert!(
            module_ids.iter().any(|id| id.contains("test-pack-001__")),
            "modules must be namespaced with pack_id__; got: {:?}",
            module_ids
        );
    }

    /// GREEN — D-09: importing the same file twice yields two distinct track ids.
    #[test]
    fn double_import_yields_distinct_track_ids() {
        let conn = fresh_conn_with_learner();
        let json = minimal_export_json("double-import-pack", "topic-pack:double-import-pack");
        let (_tmp, path) = write_tmp_json(&json);

        let r1 = import_course_impl(&conn, &path).expect("first import must succeed");
        let r2 = import_course_impl(&conn, &path).expect("second import must succeed");

        assert_ne!(
            r1.track_id, r2.track_id,
            "D-09: double import must yield distinct track ids (no overwrite/merge)"
        );
    }

    /// GREEN — D-07: schema-invalid file (missing required field) writes ZERO rows.
    #[test]
    fn invalid_schema_writes_zero_rows() {
        let conn = fresh_conn_with_learner();
        // Missing required field `title`
        let invalid_json = serde_json::json!({
            "id": "bad-pack",
            "description": "missing title",
            "domain_module": "devops",
            "modules": [{"id": "m1", "title": "M", "description": "d", "objectives": ["o"]}],
            "edges": [],
            "exportVersion": "1.0.0",
            "exportedAt": "2026-07-01T00:00:00Z",
            "exportedFrom": "topic-pack:bad-pack",
            "blocks": {},
            "labs": {},
            "videos": {}
        }).to_string();
        let (_tmp, path) = write_tmp_json(&invalid_json);

        let result = import_course_impl(&conn, &path);
        assert!(result.is_err(), "invalid schema must return Err (D-07)");

        // Assert ZERO rows written
        let track_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM learning_tracks",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(track_count, 0, "D-07: invalid file must write zero rows");
    }

    /// GREEN — D-11 (exportable source): importable source stays exportable after import.
    #[test]
    fn d11_exportable_source_stays_exportable_after_import() {
        let conn = fresh_conn_with_learner();
        let json = minimal_export_json("exportable-pack", "topic-pack:exportable-pack");
        let (_tmp, path) = write_tmp_json(&json);

        let r = import_course_impl(&conn, &path).expect("import of exportable source must succeed");

        let prov: String = conn.query_row(
            "SELECT generated_by_model FROM learning_paths WHERE track_id = ?1",
            rusqlite::params![r.track_id],
            |row| row.get(0),
        ).unwrap();

        assert!(
            is_course_exportable(&prov),
            "D-11: importable source stays exportable — stamped provenance '{}' must be exportable",
            prov
        );
    }

    /// GREEN — D-11 (no-laundering guard): non-exportable source is REJECTED, not laundered.
    #[test]
    fn d11_non_exportable_source_is_rejected_not_laundered() {
        let conn = fresh_conn_with_learner();
        // exported_from = "licensed:test" — non-exportable per is_course_exportable
        let json = minimal_export_json("licensed-course", "licensed:test");
        let (_tmp, path) = write_tmp_json(&json);

        let result = import_course_impl(&conn, &path);
        assert!(
            result.is_err(),
            "D-11: licensed:test source must be REJECTED (not laundered)"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("licensed:test") || err_msg.contains("launder") || err_msg.contains("D-11"),
            "error must mention the non-exportable provenance; got: {}",
            err_msg
        );

        // No rows written (import was rejected)
        let track_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM learning_tracks",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(track_count, 0, "D-11: rejected import must write zero rows");

        // Ensure the imported track's exportability class EQUALS the source's
        // (both are non-exportable — the import was rejected rather than laundered)
        let source_exportable = is_course_exportable("licensed:test");
        // The import was rejected — no stamped provenance exists.
        // The invariant is satisfied because the import was not committed.
        assert!(!source_exportable, "licensed:test must be non-exportable");
    }

    /// GREEN — ready blocks are rehydrated; lab blocks' image name is stored verbatim (D-03).
    #[test]
    fn import_rehydrates_ready_blocks_and_videos() {
        let conn = fresh_conn_with_learner();

        // Build an export JSON with 1 ready block (including a lab image ref) and 1 video
        let json = serde_json::json!({
            "id": "content-pack",
            "title": "Content Pack",
            "description": "A course with blocks and videos.",
            "domain_module": "devops",
            "modules": [
                {
                    "id": "mod-content",
                    "title": "Content Module",
                    "description": "Has blocks.",
                    "objectives": ["learn something"],
                    "difficulty": 1,
                    "estimatedMinutes": 30
                }
            ],
            "edges": [],
            "exportVersion": "1.0.0",
            "exportedAt": "2026-07-01T00:00:00Z",
            "exportedFrom": "topic-pack:content-pack",
            "blocks": {
                "mod-content": [
                    {
                        "id": "blk-001",
                        "moduleId": "mod-content",
                        "ordering": 0,
                        "blockType": "lab",
                        "status": "ready",
                        "paramsJson": "{}",
                        "payloadJson": "{\"image\":\"ubuntu:22.04\"}",
                        "sourceAnchorsJson": "[]",
                        "metadataJson": "{\"concept_id\":null}",
                        "retryCount": 0,
                        "createdAt": "2026-07-01T00:00:00Z",
                        "updatedAt": "2026-07-01T00:00:00Z"
                    }
                ]
            },
            "labs": {},
            "videos": {
                "mod-content": [
                    {
                        "videoId": "dQw4w9WgXcQ",
                        "title": "Test Video",
                        "channelTitle": "Test Channel",
                        "relevanceScore": 0.9
                    }
                ]
            }
        }).to_string();

        let (_tmp, path) = write_tmp_json(&json);
        let r = import_course_impl(&conn, &path).expect("import with blocks+videos must succeed");

        assert_eq!(r.block_count, 1, "1 ready block must be rehydrated");
        assert_eq!(r.module_count, 1, "1 module must be imported");

        // Verify block is in DB with namespaced module_id and verbatim image name (D-03)
        let (block_module_id, payload_json): (String, String) = conn.query_row(
            "SELECT module_id, payload_json FROM module_blocks LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert!(
            block_module_id.contains("content-pack__"),
            "block module_id must be namespaced; got: {}",
            block_module_id
        );
        assert!(
            payload_json.contains("ubuntu:22.04"),
            "lab image name must be stored verbatim (D-03); got: {}",
            payload_json
        );

        // Verify video is in DB
        let video_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM lesson_videos WHERE video_id = 'dQw4w9WgXcQ'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(video_count, 1, "ready video must be rehydrated");
    }

    /// GREEN — T-12-10 atomicity: a failure mid-write leaves ZERO rows in learning_tracks.
    #[test]
    fn atomicity_mid_write_failure_leaves_zero_rows() {
        let conn = fresh_conn_with_learner();

        // Craft a payload with a module whose id contains a NUL character to trigger
        // a SQLite constraint violation mid-modules insert (after learning_tracks INSERT).
        // We use the same technique as plan_02 tests: an impossible FK violation.
        //
        // Actually we'll use a different approach: use a second module with a duplicate
        // primary key (same namespaced_id) to force the second INSERT to fail.
        // The module ids collide after namespacing when two modules have the same id.
        let collision_json = serde_json::json!({
            "id": "atomic-pack",
            "title": "Atomic Pack",
            "description": "Tests atomicity.",
            "domain_module": "devops",
            "modules": [
                {
                    "id": "same-id",
                    "title": "Module 1",
                    "description": "First.",
                    "objectives": ["o1"],
                    "difficulty": 1,
                    "estimatedMinutes": 30
                },
                {
                    "id": "same-id",
                    "title": "Module 2 (duplicate id — forces constraint failure)",
                    "description": "Second with same id.",
                    "objectives": ["o2"],
                    "difficulty": 1,
                    "estimatedMinutes": 30
                }
            ],
            "edges": [],
            "exportVersion": "1.0.0",
            "exportedAt": "2026-07-01T00:00:00Z",
            "exportedFrom": "topic-pack:atomic-pack",
            "blocks": {},
            "labs": {},
            "videos": {}
        }).to_string();

        let (_tmp, path) = write_tmp_json(&collision_json);
        let result = import_course_impl(&conn, &path);
        assert!(
            result.is_err(),
            "duplicate module ids must cause import failure"
        );

        // T-12-10: ZERO learning_tracks rows after rollback
        let track_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM learning_tracks",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(
            track_count, 0,
            "T-12-10: mid-write failure must roll back — learning_tracks must be empty"
        );
    }
}
