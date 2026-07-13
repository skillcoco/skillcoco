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
//! For non-exportable sources (e.g. `"licensed:..."`) the source class is PRESERVED VERBATIM as
//! `generated_by_model` — the import succeeds and the track remains non-exportable.
//! Re-export of that track still returns `CourseNotExportable` (no laundering possible).
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
//! | T-12-16 (provenance laundering) | D-11: non-exportable source class is preserved verbatim (not re-stamped to imported:), so it stays non-exportable — no laundering |

use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::Manager;
use tauri::State;

use crate::storage_impl::blocks::SqliteBlockStore;
use crate::topic_packs::loader::ImportedFilePackSource;
use learnforge_core::blocks::{BlockStore, ModuleBlock};
use learnforge_core::pack_trust::{self, PackTrustError};
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

// ── Provenance tier (D-09, Step 3.5 gate) ─────────────────────────────────────

/// Provenance tier for the Step 3.5 signature gate (D-09).
///
/// `Reserved` provenance classes (`licensed:`/`curated:`) REQUIRE a valid
/// signature to import. `Open` classes (free/imported/AI-generated) import
/// unsigned exactly as today, but if they DO carry a signature it is still
/// fully verified (D-10 verify-if-present).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenanceTier {
    /// `licensed:` / `curated:` — signature required.
    Reserved,
    /// Everything else (free/imported/AI model names) — signature optional.
    Open,
}

/// Classify a RAW `exported_from` string into a provenance tier (D-09).
///
/// Reuses the SAME `RESERVED_NON_EXPORTABLE_PREFIXES` list as
/// `is_course_exportable` — no parallel prefix check (14-RESEARCH Pitfall 4).
/// MUST be called on the raw string BEFORE Step 4 transforms/namespaces it.
pub fn provenance_tier(exported_from: &str) -> ProvenanceTier {
    let s = exported_from.trim();
    for &prefix in RESERVED_NON_EXPORTABLE_PREFIXES {
        if s.starts_with(prefix) {
            return ProvenanceTier::Reserved;
        }
    }
    ProvenanceTier::Open
}

/// Result of a successful Step 3.5 verification gate pass-through.
///
/// `verified` is `true` only when a signature block was present AND the full
/// chain-of-trust check passed. Unsigned `Open`-tier packs pass through with
/// `verified=false` / `issuer_name=None` (D-09 zero-friction).
#[derive(Debug, Clone, Default)]
pub struct VerifiedImport {
    pub verified: bool,
    pub issuer_name: Option<String>,
}

/// Step 3.5 verification gate (TRUST-01/02/03, D-09/D-10).
///
/// Runs BEFORE `BEGIN IMMEDIATE` so a reject writes ZERO rows (T-14-11).
///
/// Gate logic:
/// 1. If the pack JSON carries a top-level `signature` key, verify the FULL
///    chain of trust regardless of provenance tier (D-10 verify-if-present).
///    A failing chain rejects with the mapped [`ImportCourseError`] variant.
/// 2. Else if the tier is `Reserved` (`licensed:`/`curated:`), reject with
///    `SignatureRequired` (D-09 — reserved content must be signed).
/// 3. Else (Open tier, unsigned), pass through unchanged (zero friction).
///
/// `root_pem` is injected so tests can pass the committed fixture root PEM;
/// production callers pass `pack_trust::BUNDLED_ROOT_PUBLIC_PEM` (D-06/D-08 —
/// the single canonical trust anchor, no local `include_str!` copy here).
fn verify_import_gate(
    root_pem: &str,
    exported_from: &str,
    pack_value: &serde_json::Value,
) -> Result<VerifiedImport, ImportCourseError> {
    let has_signature = pack_value
        .as_object()
        .map(|obj| obj.contains_key("signature"))
        .unwrap_or(false);

    if has_signature {
        pack_trust::verify_pack(root_pem, pack_value)?;

        // Chain verified — surface the issuer name for the frontend badge (D-14).
        let issuer_name = pack_value
            .get("signature")
            .and_then(|s| s.get("issuerCert"))
            .and_then(|c| c.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());

        return Ok(VerifiedImport {
            verified: true,
            issuer_name,
        });
    }

    if provenance_tier(exported_from) == ProvenanceTier::Reserved {
        return Err(ImportCourseError::SignatureRequired);
    }

    // Open tier, unsigned — imports exactly as today (D-09 zero friction).
    Ok(VerifiedImport::default())
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

        // Videos: SELECT WHERE module_id=? AND status='ready' (D-04).
        // Keyed by SECTION_ID (matching the schema description and the frontend
        // cache lookup, which queries with sectionId = block.id) — import writes
        // section_id = the namespaced block id so the lookup hits post-import.
        let mut stmt = conn.prepare(
            "SELECT section_id, video_id, title, channel_title, relevance_score \
             FROM lesson_videos WHERE module_id = ?1 AND status = 'ready'",
        ).map_err(|e| ExportCourseError::Db(e.to_string()))?;

        let section_videos: Vec<(String, ExportedVideo)> = stmt.query_map(
            rusqlite::params![module_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ExportedVideo {
                        video_id: row.get(1)?,
                        title: row.get(2)?,
                        channel_title: row.get(3)?,
                        relevance_score: row.get(4)?,
                    },
                ))
            },
        ).map_err(|e| ExportCourseError::Db(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();

        blocks_map.insert(module_id.clone(), ready_blocks);
        for (section_id, video) in section_videos {
            videos_map.entry(section_id).or_default().push(video);
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
    /// `true` when the pack carried a signature block and the FULL chain of
    /// trust verified successfully (TRUST-01, D-14 — drives the frontend
    /// verified badge). `false` for unsigned Open-tier imports.
    pub verified: bool,
    /// Publisher name from the verified issuer cert, when `verified == true`.
    pub issuer_name: Option<String>,
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
    /// Source provenance class is non-exportable — retained for API stability and potential
    /// future strict-mode use. Import no longer rejects; it preserves the source class verbatim
    /// (D-11). See `import_course_impl` Step 4.
    #[allow(dead_code)]
    #[error("import rejected: source provenance '{0}' is not re-exportable; importing would launder a non-exportable course into an exportable state (D-11)")]
    ProvenanceLaundering(String),
    /// Database error during atomic write.
    #[error("import DB error: {0}")]
    Db(String),
    /// JSON deserialization error.
    #[error("import deserialization error: {0}")]
    Deserialize(String),
    /// D-11 taxonomy: the pack body no longer matches its signature (edited after signing).
    #[error("This pack was modified after it was signed, so it can't be trusted. Re-download it from the original source.")]
    SignatureTampered,
    /// D-11 taxonomy: the issuer cert isn't signed by the app's trusted root key.
    #[error("This pack's publisher isn't recognized by LearnForge, so the pack can't be verified.")]
    UntrustedPublisher,
    /// D-11 taxonomy: a Reserved-tier (`licensed:`/`curated:`) pack has no signature (D-09).
    #[error("This pack needs a publisher signature to be imported, but it doesn't have one.")]
    SignatureRequired,
    /// D-11 taxonomy: the signature block itself is structurally invalid
    /// (malformed cert, malformed signature hex, non-object pack, canonicalization
    /// failure). Technical detail stays in the field for logs, not the primary message.
    #[error("The pack's signature data is malformed and can't be checked: {0}")]
    MalformedSignatureBlock(String),
}

/// Variant-by-variant remap (D-11) — mirrors the `From<PackError>` recipe below.
/// Never `.to_string().contains(..)` string-matched; callers match on the typed
/// `ImportCourseError` variant directly.
impl From<PackTrustError> for ImportCourseError {
    fn from(e: PackTrustError) -> Self {
        match e {
            PackTrustError::TamperedPack => ImportCourseError::SignatureTampered,
            PackTrustError::UntrustedIssuer => ImportCourseError::UntrustedPublisher,
            PackTrustError::MissingSignature => ImportCourseError::SignatureRequired,
            PackTrustError::MalformedCert(msg) => ImportCourseError::MalformedSignatureBlock(msg),
            PackTrustError::MalformedSignature => {
                ImportCourseError::MalformedSignatureBlock("malformed signature hex".to_string())
            }
            PackTrustError::NotAnObject => {
                ImportCourseError::MalformedSignatureBlock("pack is not a JSON object".to_string())
            }
            PackTrustError::Canonicalize(msg) => ImportCourseError::MalformedSignatureBlock(msg),
        }
    }
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
/// - D-11: source `exported_from` class is inspected; if non-exportable, the source class is
///   preserved verbatim as the stamped provenance (import succeeds, re-export still blocked).
///   If exportable, stamped as `"imported:{pack_id}"`.
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

    // ── Step 3.5: pack_trust verification gate (TRUST-01/02/03, D-09/D-10) ──
    // Runs BEFORE Step 4 reads the RAW exported_from for the tier decision
    // (14-RESEARCH Pitfall 4 — Step 4 below re-derives source_class from the
    // SAME payload.exported_from, unmodified by this gate) and BEFORE
    // `BEGIN IMMEDIATE` (Step 7) so a reject writes ZERO rows (T-14-11).
    //
    // Parse the RAW JSON value (not the deserialized struct) — verify_pack
    // operates on the whole pack value per D-03, including any fields
    // CourseExportPayload doesn't model (e.g. `signature`).
    let raw_value: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| ImportCourseError::Deserialize(e.to_string()))?;
    let verified_import = verify_import_gate(
        pack_trust::BUNDLED_ROOT_PUBLIC_PEM,
        &payload.exported_from,
        &raw_value,
    )?;

    // ── Step 4: D-11 provenance-class preservation ──────────────────────────
    // The source class is the `exported_from` field of the payload.
    // Invariant: is_course_exportable(stamped) == is_course_exportable(source).
    //
    // For NON-exportable sources (e.g. "licensed:sfd402"): PRESERVE the source class
    // verbatim as the stamped provenance — import succeeds, re-export still blocked.
    // For exportable sources: stamp as "imported:{pack_id}" (still exportable, D-06).
    //
    // This replaces the old reject-on-non-exportable behavior.  The ProvenanceLaundering
    // variant is retained for API stability / potential future strict-mode use.
    let source_class = payload.exported_from.trim().to_string();
    let pack_id = &payload.id;
    let stamped_provenance = if !is_course_exportable(&source_class) {
        // Non-exportable source: preserve verbatim so the track stays non-exportable
        // (e.g. "licensed:sfd402" → "licensed:sfd402").  Re-export will still return
        // CourseNotExportable because is_course_exportable("licensed:*") == false.
        source_class.clone()
    } else {
        // Exportable source: stamp as "imported:{pack_id}" (D-06).
        // "imported:*" is exportable by the allowlist — correct for already-public content.
        format!("imported:{}", pack_id)
    };
    // D-11 invariant: exportability class is preserved across import, never laundered.
    debug_assert_eq!(
        is_course_exportable(&stamped_provenance),
        is_course_exportable(&source_class),
        "D-11: import must preserve the exportability class, never launder"
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
        &verified_import,
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
    verified_import: &VerifiedImport,
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

    // Insert learning_paths row with PRESERVED provenance (D-11) and the
    // verified/issuer_name fields from the Step 3.5 verification gate (D-14,
    // 14-06 CR-01) so the frontend badge survives an app restart.
    conn.execute(
        "INSERT INTO learning_paths \
         (id, track_id, modules_json, edges_json, version, generated_by_model, verified, issuer_name) \
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7)",
        rusqlite::params![
            path_id,
            track_id,
            modules_json,
            edges_json,
            stamped_provenance,
            verified_import.verified,
            verified_import.issuer_name,
        ],
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

                let new_block_id = format!("{}_{}", namespaced_module_id, eb.id);
                let block = ModuleBlock {
                    id: new_block_id.clone(),
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

                // Rehydrate SECTION-keyed videos: the export map is keyed by
                // section_id == original block id. Write section_id = the NEW
                // (namespaced) block id so the frontend cache lookup
                // (module_id, sectionId=block.id) hits after import.
                if let Some(section_vids) = payload.videos.get(&eb.id) {
                    for ev in section_vids {
                        conn.execute(
                            "INSERT OR IGNORE INTO lesson_videos \
                             (id, module_id, section_id, video_id, title, channel_title, relevance_score, status) \
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ready')",
                            rusqlite::params![
                                format!("lv-{}-{}", new_block_id, ev.video_id),
                                namespaced_module_id,
                                new_block_id,
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
        }

        // Legacy fallback: videos keyed by MODULE id (pre-section-keying exports).
        // section_id = namespaced module id (original Plan 03 behavior).
        if let Some(exported_videos) = payload.videos.get(orig_module_id) {
            for ev in exported_videos {
                conn.execute(
                    "INSERT OR IGNORE INTO lesson_videos \
                     (id, module_id, section_id, video_id, title, channel_title, relevance_score, status) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ready')",
                    rusqlite::params![
                        format!("lv-{}-{}", namespaced_module_id, ev.video_id),
                        namespaced_module_id,
                        namespaced_module_id, // section_id = module_id for legacy imports
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
        verified: verified_import.verified,
        issuer_name: verified_import.issuer_name.clone(),
    })
}

// ── Import Tauri command shim ─────────────────────────────────────────────────

/// Import a course from an exported JSON file.
///
/// Thin shim: locks state, calls `import_course_impl`, maps errors to `String`.
/// The file is validated against the schema BEFORE any DB write (D-07).
/// Provenance class is preserved — non-exportable source classes are PRESERVED VERBATIM
/// as the stamped provenance (import succeeds, re-export still blocked by D-10 gate) (D-11).
#[tauri::command]
pub fn import_course(
    request: ImportCourseRequest,
    state: State<'_, crate::AppState>,
) -> Result<ImportCourseResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    import_course_impl(&db.conn, &request.file_path)
        .map_err(|e| e.to_string())
}

// ── Starter packs (LIB-04, LIB-02, D-12, D-13) ───────────────────────────────
//
// Bundled starter packs are ordinary `CourseExportPayload` files shipped
// under `resources/starter-packs/` (see `tauri.conf.json` `bundle.resources`
// and `resources/starter-packs/README.md`). `list_starter_packs` enumerates
// them offline (D-12 — no Hub catalog fetch). `start_starter_pack` resolves
// a chosen pack id to its bundled file INSIDE that directory (path-traversal
// guarded) and calls `import_course_impl` UNCHANGED (D-13 — no special-case
// bypass for bundled content; same fail-closed gate as a file-picker import).

/// Lightweight metadata describing one bundled starter pack, surfaced to the
/// Library UI as a tile (LIB-04). Parsed from the same `CourseExportPayload`
/// JSON that `start_starter_pack` later imports — never a separate manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StarterPackMeta {
    /// Pack identifier (matches the file's `id` field and its filename stem).
    pub id: String,
    /// Pack title.
    pub title: String,
    /// Pack description.
    pub description: String,
    /// Number of modules in the pack.
    pub module_count: usize,
}

/// Resolve the bundled `resources/starter-packs` directory under the given
/// Tauri resource dir. Errors if the directory is absent (e.g. dev build
/// missing the bundled resources, or a packaging regression).
fn resolve_starter_packs_dir(resource_dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let dir = resource_dir.join("resources").join("starter-packs");
    if !dir.is_dir() {
        return Err(format!(
            "starter-packs resource directory not found at {:?}",
            dir
        ));
    }
    Ok(dir)
}

/// Enumerate `*.json` files in `starter_packs_dir` and parse each into a
/// [`StarterPackMeta`]. A single malformed file is logged and SKIPPED —
/// never fails the whole batch (partial listing beats an empty Library tile
/// section).
fn list_starter_packs_impl(
    starter_packs_dir: &std::path::Path,
) -> Result<Vec<StarterPackMeta>, String> {
    let entries = std::fs::read_dir(starter_packs_dir)
        .map_err(|e| format!("failed to read starter-packs dir: {}", e))?;

    let mut packs = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                log::warn!("[starter_packs] failed to read dir entry: {}", e);
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("[starter_packs] failed to read {:?}: {}", path, e);
                continue;
            }
        };

        let payload: CourseExportPayload = match serde_json::from_str(&text) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("[starter_packs] failed to parse {:?}: {}", path, e);
                continue;
            }
        };

        packs.push(StarterPackMeta {
            id: payload.id,
            title: payload.title,
            description: payload.description,
            module_count: payload.modules.len(),
        });
    }

    Ok(packs)
}

/// Resolve `pack_id` to a bundled file INSIDE `starter_packs_dir`, then start
/// it through the UNCHANGED `import_course_impl` gate (D-13).
///
/// Traversal guard (T-16-01): rejects any `pack_id` containing `..`, `/`, or
/// a leading path separator BEFORE any path is built, then canonicalizes the
/// resolved path and asserts it is still inside `starter_packs_dir` — belt
/// and suspenders against any encoding trick that might slip past the naive
/// string check.
fn start_starter_pack_impl(
    conn: &Connection,
    starter_packs_dir: &std::path::Path,
    pack_id: &str,
) -> Result<ImportCourseResult, ImportCourseError> {
    // ── Traversal guard (T-16-01) — reject before building any path ────────
    if pack_id.is_empty()
        || pack_id.contains("..")
        || pack_id.contains('/')
        || pack_id.contains('\\')
        || pack_id.starts_with('.')
    {
        return Err(ImportCourseError::Fs(format!(
            "invalid starter pack id: {:?}",
            pack_id
        )));
    }

    let candidate = starter_packs_dir.join(format!("{}.json", pack_id));

    // Canonicalize the starter-packs dir itself so the containment check
    // below is comparing two canonical paths (the candidate file may not
    // exist yet at this point, so canonicalize the parent dir, not the file).
    let canonical_dir = std::fs::canonicalize(starter_packs_dir)
        .map_err(|e| ImportCourseError::Fs(format!("starter-packs dir error: {}", e)))?;

    let canonical_candidate = std::fs::canonicalize(&candidate)
        .map_err(|_| ImportCourseError::Fs(format!("unknown starter pack id: {:?}", pack_id)))?;

    if !canonical_candidate.starts_with(&canonical_dir) {
        return Err(ImportCourseError::Fs(format!(
            "resolved starter pack path escapes the starter-packs dir: {:?}",
            canonical_candidate
        )));
    }

    let path_str = canonical_candidate
        .to_str()
        .ok_or_else(|| ImportCourseError::Fs("starter pack path is not valid UTF-8".to_string()))?;

    // D-13: NEW caller of the UNCHANGED gate — no fork, no bypass.
    import_course_impl(conn, path_str)
}

// ── Starter pack Tauri command shims ─────────────────────────────────────────

/// List bundled starter packs (LIB-04). Offline — no Hub/server call.
#[tauri::command]
pub fn list_starter_packs(app_handle: tauri::AppHandle) -> Result<Vec<StarterPackMeta>, String> {
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?;
    let starter_packs_dir = resolve_starter_packs_dir(&resource_dir)?;
    list_starter_packs_impl(&starter_packs_dir)
}

/// Start a bundled starter pack (LIB-02). Resolves `pack_id` to its bundled
/// file and imports it through the unchanged `import_course_impl` gate
/// (D-13 — no special-case bypass for bundled content).
#[tauri::command]
pub fn start_starter_pack(
    pack_id: String,
    state: State<'_, crate::AppState>,
    app_handle: tauri::AppHandle,
) -> Result<ImportCourseResult, String> {
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?;
    let starter_packs_dir = resolve_starter_packs_dir(&resource_dir).map_err(|e| e.to_string())?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    start_starter_pack_impl(&db.conn, &starter_packs_dir, &pack_id).map_err(|e| e.to_string())
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

    // ENT-03 fail-closed baseline (Phase 15) — MUST stay byte-identical; do not modify.
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

    /// Real runtime tracks carry bare-UUID ids (do NOT match the authored slug
    /// pattern) and the onboarding wizard's default `domain_module` "general"
    /// (not in the authored enum). Because these files carry `exportVersion`,
    /// the schema must relax the authored id/domain constraints — import
    /// re-namespaces every id anyway (D-06), so the format gate has no safety
    /// value here. Round-trip (D-05) must not reject real exported courses.
    #[test]
    fn exported_file_with_runtime_uuid_ids_and_general_domain_validates() {
        let json = serde_json::json!({
            "id": "7cfb3736-2f14-41df-9707-54939b863574",
            "title": "Real Course",
            "description": "A real AI-generated course",
            "domain_module": "general",
            "modules": [{
                "id": "7a862c91-cf13-4160-a13f-e60d361fa833",
                "title": "Module 1",
                "description": "desc",
                "objectives": ["learn"]
            }],
            "edges": [{
                "from": "7a862c91-cf13-4160-a13f-e60d361fa833",
                "to": "7a862c91-cf13-4160-a13f-e60d361fa833"
            }],
            "exportVersion": "1.0.0",
            "exportedAt": "2026-07-01T00:00:00Z",
            "exportedFrom": "imported:7cfb3736"
        })
        .to_string();

        let r = learnforge_core::packs::loader::parse_and_validate(&json);
        assert!(
            r.is_ok(),
            "exported course with bare-UUID ids + 'general' domain must validate (D-05); got: {:?}",
            r
        );
    }

    /// Strictness must be preserved for AUTHORED packs (no `exportVersion`):
    /// a bare-UUID id and a non-enum domain must still be rejected.
    #[test]
    fn authored_pack_with_uuid_id_still_rejected() {
        let json = serde_json::json!({
            "id": "7cfb3736-2f14-41df-9707-54939b863574",
            "title": "Bad Authored Pack",
            "description": "no exportVersion => strict authored rules apply",
            "domain_module": "general",
            "modules": [{
                "id": "mod-1",
                "title": "M",
                "description": "d",
                "objectives": ["o"]
            }]
        })
        .to_string();

        let r = learnforge_core::packs::loader::parse_and_validate(&json);
        assert!(
            r.is_err(),
            "authored pack (no exportVersion) with UUID id + 'general' domain must be rejected"
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

    // ── Video section-keying tests (post-12 fix) ─────────────────────────────
    //
    // The frontend fetches videos with sectionId = block.id (ModuleView.tsx),
    // so the export map must be keyed by section_id (as the schema describes)
    // and import must write section_id = the NAMESPACED BLOCK ID so the cache
    // lookup (module_id, section_id) hits after import. Module-keyed entries
    // (legacy exports) still import with section_id = module_id as fallback.

    #[test]
    fn export_keys_videos_by_section_id() {
        let conn = fresh_conn();
        let modules_json = r#"[{"id":"mod-1","title":"Mod 1","description":"d","objectives":["o"]}]"#;
        let (track_id, path_id) = seed_track(&conn, "claude-3-5-sonnet", modules_json, "[]");
        seed_module(&conn, "mod-1", &path_id);
        insert_block(&conn, "blk-1", "mod-1", "ready", 0);
        // video attached to a specific section (block), not the module
        insert_video_for_section_io(&conn, "mod-1", "blk-1", "vidsec1");

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let save_path = tmp.path().to_str().unwrap().to_string();
        export_course_impl(&conn, &track_id, &save_path).expect("export must succeed");

        let payload: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&save_path).unwrap()).unwrap();
        assert!(
            payload["videos"]["blk-1"].is_array(),
            "videos map must be keyed by section_id (blk-1); got keys: {:?}",
            payload["videos"].as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
        assert_eq!(payload["videos"]["blk-1"][0]["videoId"], "vidsec1");
    }

    /// Insert a ready lesson_video row with an explicit section_id.
    fn insert_video_for_section_io(conn: &Connection, module_id: &str, section_id: &str, video_id: &str) {
        conn.execute(
            "INSERT INTO lesson_videos \
             (id, module_id, section_id, video_id, title, channel_title, relevance_score, status) \
             VALUES (?1, ?2, ?3, ?4, 'SV', 'SC', 0.9, 'ready')",
            rusqlite::params![format!("lv-{}", video_id), module_id, section_id, video_id],
        ).unwrap();
    }

    #[test]
    fn import_section_keyed_videos_land_on_namespaced_block_id() {
        let conn = fresh_conn_with_learner();
        // payload with one module, one ready section block "blk-101", video keyed by that block id
        let json = serde_json::json!({
            "id": "sec-video-pack",
            "title": "Sec Video Course",
            "description": "d",
            "domain_module": "devops",
            "modules": [{"id":"m1","title":"M1","description":"d","objectives":["o"]}],
            "edges": [],
            "exportVersion": "1.0.0",
            "exportedAt": "2026-07-03T00:00:00Z",
            "exportedFrom": "imported:src",
            "blocks": {"m1": [{
                "id":"blk-101","moduleId":"m1","ordering":0,"blockType":"section",
                "status":"ready","paramsJson":"{}","payloadJson":"{\"markdown\":\"hi\"}",
                "sourceAnchorsJson":"[]","metadataJson":"{}","retryCount":0,
                "createdAt":"2026-07-03T00:00:00Z","updatedAt":"2026-07-03T00:00:00Z"
            }]},
            "videos": {"blk-101": [{
                "videoId":"ytabc","title":"T","channelTitle":"C","relevanceScore":1.0
            }]}
        }).to_string();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &json).unwrap();

        let result = import_course_impl(&conn, tmp.path().to_str().unwrap())
            .expect("import must succeed");

        // the imported block id is {ns_module}_{blk-101}; the video row's section_id must equal it
        let (blk_id, blk_module): (String, String) = conn.query_row(
            "SELECT id, module_id FROM module_blocks WHERE id LIKE '%blk-101'",
            [], |r| Ok((r.get(0)?, r.get(1)?)),
        ).expect("imported block exists");
        let (sec_id, vid_module): (String, String) = conn.query_row(
            "SELECT section_id, module_id FROM lesson_videos WHERE video_id='ytabc'",
            [], |r| Ok((r.get(0)?, r.get(1)?)),
        ).expect("imported video exists");
        assert_eq!(sec_id, blk_id, "video section_id must equal the namespaced block id so the frontend cache lookup hits");
        assert_eq!(vid_module, blk_module);
        assert_eq!(result.module_count, 1);
    }

    #[test]
    fn import_module_keyed_videos_still_work_as_fallback() {
        let conn = fresh_conn_with_learner();
        let json = serde_json::json!({
            "id": "legacy-video-pack",
            "title": "Legacy Video Course",
            "description": "d",
            "domain_module": "devops",
            "modules": [{"id":"m1","title":"M1","description":"d","objectives":["o"]}],
            "edges": [],
            "exportVersion": "1.0.0",
            "exportedAt": "2026-07-03T00:00:00Z",
            "exportedFrom": "imported:src",
            "blocks": {"m1": [{
                "id":"blk-1","moduleId":"m1","ordering":0,"blockType":"section",
                "status":"ready","paramsJson":"{}","payloadJson":"{\"markdown\":\"hi\"}",
                "sourceAnchorsJson":"[]","metadataJson":"{}","retryCount":0,
                "createdAt":"2026-07-03T00:00:00Z","updatedAt":"2026-07-03T00:00:00Z"
            }]},
            "videos": {"m1": [{
                "videoId":"ytlegacy","title":"T","channelTitle":"C","relevanceScore":0.8
            }]}
        }).to_string();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &json).unwrap();

        import_course_impl(&conn, tmp.path().to_str().unwrap()).expect("import must succeed");

        let sec_id: String = conn.query_row(
            "SELECT section_id FROM lesson_videos WHERE video_id='ytlegacy'",
            [], |r| r.get(0),
        ).expect("legacy video imported");
        // legacy fallback: section_id = namespaced module id
        assert!(sec_id.ends_with("__m1"), "legacy module-keyed video keeps section_id = ns module id; got {}", sec_id);
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

    /// D-11 (preserve-not-reject): non-exportable source is PRESERVED verbatim, not rejected.
    ///
    /// Phase 14 supersedes the old "unsigned licensed: import succeeds" premise:
    /// D-09 now requires a valid signature for Reserved-tier (licensed:/curated:)
    /// packs (see `unsigned_licensed_pack_rejected` below), so this test uses the
    /// SIGNED `valid-signed.json` fixture (`exportedFrom: "licensed:test-pack|Test
    /// Publisher"`) to exercise the still-valid D-11 preservation invariant: once a
    /// signed licensed pack imports, its non-exportable class is preserved verbatim
    /// (not laundered into an exportable one), and re-export is still blocked.
    ///
    /// Test 1: import of the signed licensed pack succeeds; generated_by_model ==
    /// the source class verbatim.
    /// Test 2: export of that imported track returns CourseNotExportable (no file written).
    /// Invariant: is_course_exportable(stamped) == is_course_exportable(source) — both false.
    #[test]
    fn d11_non_exportable_source_is_preserved_not_rejected() {
        let conn = fresh_conn_with_learner();
        let source_class = "licensed:test-pack|Test Publisher";
        let (_tmp, path) = write_tmp_json(FIXTURE_VALID_SIGNED);

        // Test 1: import SUCCEEDS (signed Reserved-tier pack passes the Step 3.5 gate;
        // no ProvenanceLaundering rejection either)
        let result = import_course_impl(&conn, &path);
        assert!(
            result.is_ok(),
            "D-11: signed licensed: source must be PRESERVED (import must succeed, not reject); got: {:?}",
            result
        );
        let r = result.unwrap();
        assert!(r.verified, "TRUST-01: signed licensed import must be verified=true");

        // Test 1 cont.: generated_by_model must be the source class verbatim (preserved, not laundered)
        let stamped_prov: String = conn.query_row(
            "SELECT generated_by_model FROM learning_paths WHERE track_id = ?1",
            rusqlite::params![r.track_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(
            stamped_prov, source_class,
            "D-11: stamped provenance must be preserved verbatim as '{}'; got: {}",
            source_class, stamped_prov
        );

        // Test 2: export of the imported track returns CourseNotExportable, no file written
        let tmp_export = tempfile::NamedTempFile::new().unwrap();
        let save_path = tmp_export.path().to_str().unwrap().to_string();
        drop(tmp_export); // remove file so we can assert it was NOT written
        assert!(!std::path::Path::new(&save_path).exists(), "precondition: export file must not exist");

        let export_result = export_course_impl(&conn, &r.track_id, &save_path);
        assert!(
            export_result.is_err(),
            "D-11: export of a licensed: track must return Err(CourseNotExportable)"
        );
        let export_err = export_result.unwrap_err();
        assert!(
            matches!(export_err, ExportCourseError::CourseNotExportable(_)),
            "error variant must be CourseNotExportable; got: {:?}", export_err
        );
        assert!(
            !std::path::Path::new(&save_path).exists(),
            "D-11: NO export file must be written for a licensed track"
        );

        // Invariant: is_course_exportable(stamped) == is_course_exportable(source) — both false
        let source_exportable = is_course_exportable(source_class);
        let stamped_exportable = is_course_exportable(&stamped_prov);
        assert!(!source_exportable, "source '{}' must be non-exportable", source_class);
        assert!(!stamped_exportable, "stamped '{}' must also be non-exportable", stamped_prov);
        assert_eq!(
            source_exportable, stamped_exportable,
            "D-11 invariant: exportability class must be preserved (both false — no laundering)"
        );
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

    /// GREEN (Phase 14 Plan 04, TRUST-03) — a pack whose body was edited AFTER
    /// signing is rejected at import with ZERO DB writes by the Step 3.5
    /// pack_trust verification gate. Mirrors the T-12-10 atomicity test pattern
    /// above. This hand-built pack has a garbage `rootSig`/`sig` (not a real
    /// signature over anything) so it fails chain verification regardless of
    /// which root PEM is used — exercising the gate without depending on the
    /// 14-03 fixture keys.
    #[test]
    fn tampered_pack_import_writes_nothing() {
        let conn = fresh_conn_with_learner();

        // A licensed pack carrying a signature block whose `sig` no longer
        // matches the body: the body (title) was edited after signing, which
        // is indistinguishable from a signature computed over different bytes.
        let mut pack: serde_json::Value = serde_json::from_str(&minimal_export_json(
            "tampered-pack",
            "licensed:tampered-pack|Test Licensor",
        ))
        .unwrap();
        pack["signature"] = serde_json::json!({
            "alg": "ed25519",
            "issuerCert": {
                "issuerId": "issuer-001",
                "name": "Test Issuer",
                "publicKeyPem": "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAtOJv2B75vSb1v0PxrEpQe1rrJDPUKSFF12my3AeBOI4=\n-----END PUBLIC KEY-----\n",
                // Hex-shaped but not a valid root signature over this cert.
                "rootSig": "00".repeat(64)
            },
            "keyFingerprint": "deadbeef",
            // Signature over the PRE-tamper body — no longer matches below.
            "sig": "00".repeat(64)
        });
        // Body edited AFTER signing (any byte, including provenance, counts).
        pack["title"] = serde_json::json!("Tampered Title (edited after signing)");

        let (_tmp, path) = write_tmp_json(&pack.to_string());
        let result = import_course_impl(&conn, &path);
        assert!(
            result.is_err(),
            "TRUST-03: tampered signed pack must be rejected (verification gate lands in 14-04); \
             import unexpectedly succeeded"
        );

        // TRUST-03: ZERO rows written — gate must reject BEFORE BEGIN IMMEDIATE.
        let track_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM learning_tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            track_count, 0,
            "TRUST-03: tampered pack import must write ZERO learning_tracks rows"
        );
    }

    // ── 14-04 Step 3.5 gate tests (TRUST-01/02/03, D-09, D-10, D-11) ─────────
    //
    // Fixtures below are the four committed pack_trust fixtures from 14-03
    // (learnforge-core/tests/fixtures/pack_trust/), embedded at compile time.
    // The fixture root PEM is IDENTICAL to `pack_trust::BUNDLED_ROOT_PUBLIC_PEM`
    // (verified: `diff learnforge-core/keys/root_public.pem
    // learnforge-core/tests/fixtures/pack_trust/root-public.pem` — no diff), so
    // `import_course_impl` (which always verifies against BUNDLED_ROOT_PUBLIC_PEM)
    // can exercise these fixtures directly with no test-only root injection needed.

    const FIXTURE_VALID_SIGNED: &str =
        include_str!("../../../learnforge-core/tests/fixtures/pack_trust/valid-signed.json");
    const FIXTURE_TAMPERED_BODY: &str =
        include_str!("../../../learnforge-core/tests/fixtures/pack_trust/tampered-body.json");
    const FIXTURE_STRIPPED_SIGNATURE: &str =
        include_str!("../../../learnforge-core/tests/fixtures/pack_trust/stripped-signature.json");
    const FIXTURE_FORGED_CERT: &str =
        include_str!("../../../learnforge-core/tests/fixtures/pack_trust/forged-cert.json");

    /// TRUST-01 — a valid signed `licensed:` pack imports successfully and the
    /// result surfaces `verified=true` + the issuer name for the frontend badge (D-14).
    #[test]
    fn valid_signed_licensed_pack_imports() {
        let conn = fresh_conn_with_learner();
        let (_tmp, path) = write_tmp_json(FIXTURE_VALID_SIGNED);

        let result = import_course_impl(&conn, &path);
        assert!(result.is_ok(), "valid signed pack must import; got: {:?}", result);
        let r = result.unwrap();
        assert!(r.verified, "TRUST-01: verified must be true for a valid signed import");
        assert_eq!(
            r.issuer_name.as_deref(),
            Some("Test Publisher"),
            "TRUST-01: issuer_name must be surfaced from the verified issuer cert"
        );
    }

    /// CR-01 (14-06) — verified/issuer_name must be PERSISTED to learning_paths,
    /// not just returned once over the import IPC response. This is a real,
    /// non-mocked DB SELECT after import — proves the badge's production data
    /// path (get_path reads this same column) actually has a value to read.
    #[test]
    fn valid_signed_import_persists_verified_and_issuer_name_to_db() {
        let conn = fresh_conn_with_learner();
        let (_tmp, path) = write_tmp_json(FIXTURE_VALID_SIGNED);

        let result = import_course_impl(&conn, &path);
        assert!(result.is_ok(), "valid signed pack must import; got: {:?}", result);
        let r = result.unwrap();

        let (verified, issuer_name): (i64, Option<String>) = conn
            .query_row(
                "SELECT verified, issuer_name FROM learning_paths WHERE track_id = ?1",
                rusqlite::params![r.track_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("learning_paths row must exist for imported track");

        assert_eq!(
            verified, 1,
            "CR-01: learning_paths.verified must be persisted as 1 for a valid signed import (real DB row, not a mock)"
        );
        assert_eq!(
            issuer_name.as_deref(),
            Some("Test Publisher"),
            "CR-01: learning_paths.issuer_name must be persisted from the verified issuer cert"
        );
    }

    /// CR-01 (14-06) — an unsigned/free (Open-tier) import must leave
    /// verified=0 / issuer_name=NULL in the persisted row (backward-compat /
    /// fail-closed default — no badge without proof).
    #[test]
    fn unsigned_import_persists_verified_zero_and_null_issuer_name() {
        let conn = fresh_conn_with_learner();
        let json = minimal_export_json("pack-open-tier", "generated");
        let (_tmp, path) = write_tmp_json(&json);

        let result = import_course_impl(&conn, &path);
        assert!(result.is_ok(), "unsigned open-tier pack must import; got: {:?}", result);
        let r = result.unwrap();
        assert!(!r.verified, "unsigned import must have verified=false in the IPC result");

        let (verified, issuer_name): (i64, Option<String>) = conn
            .query_row(
                "SELECT verified, issuer_name FROM learning_paths WHERE track_id = ?1",
                rusqlite::params![r.track_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("learning_paths row must exist for imported track");

        assert_eq!(
            verified, 0,
            "unsigned import must persist verified=0 (fail-closed default)"
        );
        assert_eq!(
            issuer_name, None,
            "unsigned import must persist issuer_name=NULL"
        );
    }

    /// TRUST-03 — a pack whose body was edited after signing (fixture
    /// tampered-body.json) is rejected via the typed `SignatureTampered`
    /// variant with ZERO DB writes.
    #[test]
    fn tampered_fixture_pack_rejected_with_zero_writes() {
        let conn = fresh_conn_with_learner();
        let (_tmp, path) = write_tmp_json(FIXTURE_TAMPERED_BODY);

        let result = import_course_impl(&conn, &path);
        assert!(
            matches!(result, Err(ImportCourseError::SignatureTampered)),
            "TRUST-03: tampered fixture must reject with SignatureTampered; got {:?}",
            result
        );

        let track_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM learning_tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(track_count, 0, "TRUST-03: tampered pack import must write ZERO rows");
    }

    /// D-09 — an unsigned `licensed:` pack (fixture stripped-signature.json,
    /// signature key removed but `licensed:` provenance retained) is rejected
    /// with the plain-language "missing required signature" taxonomy variant.
    #[test]
    fn unsigned_licensed_pack_rejected() {
        let conn = fresh_conn_with_learner();
        let (_tmp, path) = write_tmp_json(FIXTURE_STRIPPED_SIGNATURE);

        let result = import_course_impl(&conn, &path);
        assert!(
            matches!(result, Err(ImportCourseError::SignatureRequired)),
            "D-09: unsigned licensed: pack must reject with SignatureRequired; got {:?}",
            result
        );

        let track_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM learning_tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(track_count, 0, "unsigned licensed: pack must write ZERO rows");
    }

    /// D-09 — an unsigned `imported:`-provenance (free/Open-tier) pack imports
    /// exactly as today: zero friction, no signature required, verified=false.
    #[test]
    fn unsigned_free_pack_imports_unchanged() {
        let conn = fresh_conn_with_learner();
        let json = minimal_export_json("free-pack-001", "imported:free-pack-001");
        let (_tmp, path) = write_tmp_json(&json);

        let result = import_course_impl(&conn, &path);
        assert!(result.is_ok(), "unsigned free pack must import unchanged; got: {:?}", result);
        let r = result.unwrap();
        assert!(!r.verified, "unsigned free import must have verified=false");
        assert!(r.issuer_name.is_none(), "unsigned free import must have issuer_name=None");
    }

    /// D-10 — verify-if-present: a free-provenance pack that DOES carry a
    /// signature still gets FULL chain verification; a bad signature on it is
    /// still rejected even though its tier would otherwise allow unsigned import.
    #[test]
    fn verify_if_present_any_provenance() {
        let conn = fresh_conn_with_learner();

        // Build a free-provenance ("imported:") pack, then attach a garbage
        // signature block (same shape as the tampered_pack_import_writes_nothing
        // hand-built fixture above) — bad sig must still reject despite Open tier.
        let mut pack: serde_json::Value =
            serde_json::from_str(&minimal_export_json("free-pack-002", "imported:free-pack-002"))
                .unwrap();
        pack["signature"] = serde_json::json!({
            "alg": "ed25519",
            "issuerCert": {
                "issuerId": "issuer-001",
                "name": "Test Issuer",
                "publicKeyPem": "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAtOJv2B75vSb1v0PxrEpQe1rrJDPUKSFF12my3AeBOI4=\n-----END PUBLIC KEY-----\n",
                "rootSig": "00".repeat(64)
            },
            "keyFingerprint": "deadbeef",
            "sig": "00".repeat(64)
        });

        let (_tmp, path) = write_tmp_json(&pack.to_string());
        let result = import_course_impl(&conn, &path);
        assert!(
            result.is_err(),
            "D-10: a free-provenance pack carrying a signature must still be fully \
             verified; a bad signature must reject even though the tier is Open"
        );

        let track_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM learning_tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(track_count, 0, "D-10: rejected verify-if-present import must write ZERO rows");
    }

    /// TRUST-02 negative — a pack whose embedded issuer cert is signed by a
    /// key OTHER than the trusted root (fixture forged-cert.json) is rejected
    /// with the typed `UntrustedPublisher` variant.
    #[test]
    fn forged_cert_pack_rejected() {
        let conn = fresh_conn_with_learner();
        let (_tmp, path) = write_tmp_json(FIXTURE_FORGED_CERT);

        let result = import_course_impl(&conn, &path);
        assert!(
            matches!(result, Err(ImportCourseError::UntrustedPublisher)),
            "TRUST-02: forged-cert pack must reject with UntrustedPublisher; got {:?}",
            result
        );

        let track_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM learning_tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(track_count, 0, "forged-cert pack import must write ZERO rows");
    }

    /// D-11 — the three PackTrustError chain-failure variants map to three
    /// DISTINCT typed ImportCourseError variants (never collapsed to one
    /// generic error, never string-matched).
    #[test]
    fn pack_trust_errors_map_to_distinct_import_errors() {
        assert!(matches!(
            ImportCourseError::from(PackTrustError::TamperedPack),
            ImportCourseError::SignatureTampered
        ));
        assert!(matches!(
            ImportCourseError::from(PackTrustError::UntrustedIssuer),
            ImportCourseError::UntrustedPublisher
        ));
        assert!(matches!(
            ImportCourseError::from(PackTrustError::MissingSignature),
            ImportCourseError::SignatureRequired
        ));

        // Distinct plain-language messages (D-11) — no shared wording, no
        // `.to_string().contains(..)` matching required by callers.
        let tampered_msg = ImportCourseError::from(PackTrustError::TamperedPack).to_string();
        let untrusted_msg = ImportCourseError::from(PackTrustError::UntrustedIssuer).to_string();
        let missing_msg = ImportCourseError::from(PackTrustError::MissingSignature).to_string();
        assert_ne!(tampered_msg, untrusted_msg);
        assert_ne!(tampered_msg, missing_msg);
        assert_ne!(untrusted_msg, missing_msg);
        assert!(tampered_msg.contains("modified after it was signed"));
        assert!(untrusted_msg.contains("publisher isn't recognized"));
        assert!(missing_msg.contains("needs a publisher signature"));
    }

    /// D-14 — a successful signed import carries verified=true + issuer_name;
    /// an unsigned free import carries verified=false + issuer_name=None.
    /// (Behavioral coverage complements `valid_signed_licensed_pack_imports`
    /// and `unsigned_free_pack_imports_unchanged` above.)
    #[test]
    fn import_result_carries_verified_and_issuer() {
        let conn = fresh_conn_with_learner();

        let (_tmp1, signed_path) = write_tmp_json(FIXTURE_VALID_SIGNED);
        let signed_result = import_course_impl(&conn, &signed_path).expect("signed import must succeed");
        assert!(signed_result.verified);
        assert!(signed_result.issuer_name.is_some());

        let unsigned_json = minimal_export_json("free-pack-003", "imported:free-pack-003");
        let (_tmp2, unsigned_path) = write_tmp_json(&unsigned_json);
        let unsigned_result =
            import_course_impl(&conn, &unsigned_path).expect("unsigned free import must succeed");
        assert!(!unsigned_result.verified);
        assert!(unsigned_result.issuer_name.is_none());
    }

    // ── Starter pack tests (Phase 16, Plan 01, Task 2 — T-16-01/T-16-02) ─────

    /// Create a temp dir acting as the bundled starter-packs dir with one
    /// valid export-shaped pack file named `{pack_id}.json`.
    fn starter_packs_fixture_dir(pack_id: &str) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        let json = minimal_export_json(pack_id, format!("topic-pack:{}", pack_id).as_str());
        std::fs::write(dir.path().join(format!("{}.json", pack_id)), json).unwrap();
        dir
    }

    /// T-16-01 traversal guard — `..`, absolute, nested, backslash and empty
    /// ids are ALL rejected before any path is built.
    #[test]
    fn starter_pack_traversal_guard_rejects_escaping_ids() {
        let conn = fresh_conn_with_learner();
        let dir = starter_packs_fixture_dir("guard-pack");

        for bad_id in [
            "../etc/passwd",
            "..",
            "/abs",
            "/etc/passwd",
            "sub/dir",
            "a\\b",
            ".hidden",
            "",
        ] {
            let result = start_starter_pack_impl(&conn, dir.path(), bad_id);
            assert!(
                result.is_err(),
                "traversal guard must reject id {:?}",
                bad_id
            );
        }
    }

    /// Unknown-but-well-formed id returns Err (not a panic).
    #[test]
    fn starter_pack_unknown_id_returns_err() {
        let conn = fresh_conn_with_learner();
        let dir = starter_packs_fixture_dir("known-pack");

        let result = start_starter_pack_impl(&conn, dir.path(), "no-such-pack");
        assert!(result.is_err(), "unknown pack id must be an Err, not a panic");
    }

    /// Happy path — a valid bundled pack id routes through the UNCHANGED
    /// import_course_impl gate and returns an ImportCourseResult (D-13).
    #[test]
    fn starter_pack_happy_path_imports_via_gate() {
        let conn = fresh_conn_with_learner();
        let dir = starter_packs_fixture_dir("starter-happy");

        let result = start_starter_pack_impl(&conn, dir.path(), "starter-happy")
            .expect("valid bundled starter pack must import");

        assert!(!result.track_id.is_empty(), "track_id must be non-empty");
        assert_eq!(result.module_count, 1, "fixture has exactly 1 module");

        // D-13: the gate stamped imported:* provenance exactly as a normal
        // file import would — proof the same code path ran.
        let prov: String = conn
            .query_row(
                "SELECT generated_by_model FROM learning_paths WHERE track_id = ?1",
                rusqlite::params![result.track_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            prov.starts_with("imported:"),
            "starter pack import must carry imported:* provenance; got {}",
            prov
        );
    }

    /// list_starter_packs_impl returns metadata for every valid pack and
    /// SKIPS malformed files without failing the batch.
    #[test]
    fn starter_pack_list_skips_malformed_files() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("good-pack.json"),
            minimal_export_json("good-pack", "topic-pack:good-pack"),
        )
        .unwrap();
        std::fs::write(dir.path().join("broken.json"), "{ not valid json").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "ignored — not json").unwrap();

        let packs = list_starter_packs_impl(dir.path()).expect("listing must not fail the batch");

        assert_eq!(packs.len(), 1, "only the valid pack is listed");
        assert_eq!(packs[0].id, "good-pack");
        assert_eq!(packs[0].title, "Test Course");
        assert_eq!(packs[0].module_count, 1);
    }
}
