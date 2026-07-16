//! Course export payload types — Phase 12 Plan 01.
//!
//! WASM-clean boundary: no `std::fs`, no `dirs`, no `rusqlite`.
//! Only `serde`, `serde_json`, `thiserror`, `std::collections::HashMap`.
//!
//! ## Design decisions honored
//!
//! - **D-02**: Only `status = "ready"` blocks are representable in the export
//!   payload. The enum in `pack-schema.json` + the field type here both enforce this.
//! - **D-03**: `ExportedLab.image` is a registry-pullable name reference (`Option<String>`),
//!   never a binary blob.
//! - **D-04**: `ExportedVideo` carries embeddable YouTube IDs, no media data.
//! - **D-05**: `CourseExportPayload` is a superset of `pack.json` — it includes all
//!   required pack fields so the same file validates against `pack-schema.json`.
//! - **D-06**: `exported_from` carries provenance, e.g. `"imported:<pack_id>"`.
//! - **D-08**: Learner state (`module_progress`, `bkt_params`, `sr_cards`, `lab_progress`,
//!   `lesson_completions`) is absent by design — this is a course export, not a
//!   learner-state backup.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single serialized block for inclusion in an exported course file.
///
/// Only `status = "ready"` blocks are exported (D-02). camelCase serde
/// matches the IPC wire format (same convention as `ModuleBlock` in
/// `skillcoco_core::blocks`).
///
/// Field names mirror `ModuleBlock` exactly so that a mapping from
/// `ModuleBlock -> ExportedBlock` in `commands/course_io.rs` is trivial.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedBlock {
    /// UUID v4 string — primary key.
    pub id: String,
    /// FK → `modules.id`.
    pub module_id: String,
    /// Display order within the module (0-based, ASC).
    pub ordering: i32,
    /// Serialized block type (snake_case, e.g. `"section"`, `"quiz"`, `"lab"`).
    pub block_type: String,
    /// Always `"ready"` — export strips non-ready blocks at serialization time.
    pub status: String,
    /// JSON-encoded generation params.
    pub params_json: String,
    /// JSON-encoded rendered content payload.
    pub payload_json: String,
    /// JSON-encoded array of source-document anchors (citations).
    pub source_anchors_json: String,
    /// JSON-encoded metadata blob.
    pub metadata_json: String,
    /// Number of generation retries attempted.
    pub retry_count: i32,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-update timestamp.
    pub updated_at: String,
}

/// Lab exported as definition + image reference, NOT binary artifact (D-03).
///
/// The `image` field holds a Docker registry image name (registry-pullable).
/// Never a binary blob. `None` means the lab runs in the host shell
/// (`requires_docker` should also be `false` in that case).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedLab {
    /// Kebab-case lab identifier matching the lab directory name.
    pub slug: String,
    /// Human-readable lab title.
    pub title: String,
    /// Whether a Docker daemon is required to run this lab.
    pub requires_docker: bool,
    /// Docker image name (registry-pullable). `None` for host-shell labs.
    pub image: Option<String>,
    /// Serialized step definitions (lab-specific JSON objects).
    pub steps: Vec<serde_json::Value>,
}

/// A cached YouTube video reference (embeddable video ID + metadata, D-04).
///
/// No media blob — recipient plays via IFrame with no API key required.
/// camelCase serde matches the `LessonVideo` IPC shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedVideo {
    /// YouTube video ID (e.g. `"dQw4w9WgXcQ"`).
    pub video_id: String,
    /// Video title.
    pub title: String,
    /// YouTube channel title.
    pub channel_title: String,
    /// Relevance score [0.0, 1.0] assigned by the video-enrichment pipeline.
    pub relevance_score: f32,
}

/// Full-fidelity exported course payload — superset of `pack.json` (D-05).
///
/// Blocks carry generated content; labs carry image references; videos
/// carry cached embeddable IDs. Learner state is absent (D-08).
///
/// The struct serializes to JSON that validates against the extended
/// `pack-schema.json` (Task 1 of this plan). The `blocks`/`labs`/`videos`
/// fields are optional at the schema level but present in every export
/// produced by `serialize_export`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseExportPayload {
    // ── pack.json required fields (must satisfy pack-schema.json) ──
    /// Pack identifier — kebab-case, matches the `id` field of `pack.json`.
    pub id: String,
    /// Pack title.
    pub title: String,
    /// Pack description.
    pub description: String,
    /// Domain module (one of `programming | devops | cloud | concepts | data`).
    ///
    /// **Schema-alignment:** pack-schema.json `required` array uses the snake_case key
    /// `"domain_module"`. Although the outer struct has `rename_all = "camelCase"`,
    /// this field uses a field-level `rename` override so the serialized JSON satisfies
    /// `parse_and_validate` (D-05, D-07 forward-compat). All other export-only extension
    /// fields (exportVersion, exportedAt, exportedFrom, blocks, labs, videos) remain
    /// camelCase because they are NOT in the schema's `required` array.
    #[serde(rename = "domain_module")]
    pub domain_module: String,
    /// Ordered module list (pack `Module` objects as JSON values).
    pub modules: Vec<serde_json::Value>,
    /// Dependency edges between modules (pack `Edge` objects as JSON values).
    pub edges: Vec<serde_json::Value>,

    // ── Export-only extensions (schema additionalProperties: true) ──
    /// Exporter version string for forward-compat checks (e.g. `"0.1.0"`).
    pub export_version: String,
    /// RFC3339 export timestamp.
    pub exported_at: String,
    /// Provenance marker — mirrors `"topic-pack:<id>"` convention (D-06).
    ///
    /// Format: `"imported:<original_pack_id>"` or `"imported:custom:<uuid>"`.
    pub exported_from: String,

    /// `module_id -> ready blocks` (D-02). Only `status = "ready"` blocks included.
    pub blocks: HashMap<String, Vec<ExportedBlock>>,
    /// `module_id -> lab specs` with image references only (D-03).
    #[serde(default)]
    pub labs: HashMap<String, Vec<ExportedLab>>,
    /// `section_id -> cached video refs` (D-04).
    #[serde(default)]
    pub videos: HashMap<String, Vec<ExportedVideo>>,
}

/// Typed error envelope for export operations.
///
/// Follows the `BlocksError` / `BktError` pattern from the existing codebase:
/// stringified at the IPC boundary in `src-tauri`, typed here in core.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// JSON serialization failed (e.g. non-finite float in `relevance_score`).
    #[error("export serialization failed: {0}")]
    Serialize(String),
    /// Payload failed a semantic validation check.
    #[error("export validation failed: {0}")]
    Validation(String),
}

/// Serialize a `CourseExportPayload` to pretty-printed JSON text.
///
/// Pure function — no FS, no SQL, no async. WASM-portable.
///
/// # Errors
///
/// Returns `ExportError::Serialize` if `serde_json::to_string_pretty` fails
/// (rare; only for non-finite floats or other JSON-unsafe values).
pub fn serialize_export(payload: &CourseExportPayload) -> Result<String, ExportError> {
    serde_json::to_string_pretty(payload).map_err(|e| ExportError::Serialize(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_block(status: &str) -> ExportedBlock {
        ExportedBlock {
            id: "blk-001".to_string(),
            module_id: "mod-001".to_string(),
            ordering: 0,
            block_type: "section".to_string(),
            status: status.to_string(),
            params_json: "{}".to_string(),
            payload_json: r#"{"content":"hello"}"#.to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id":null}"#.to_string(),
            retry_count: 0,
            created_at: "2026-07-01T00:00:00Z".to_string(),
            updated_at: "2026-07-01T00:00:00Z".to_string(),
        }
    }

    fn sample_payload() -> CourseExportPayload {
        let mut blocks = HashMap::new();
        blocks.insert("mod-001".to_string(), vec![sample_block("ready")]);

        CourseExportPayload {
            id: "test-pack".to_string(),
            title: "Test Pack".to_string(),
            description: "A test pack for unit tests.".to_string(),
            domain_module: "devops".to_string(),
            modules: vec![serde_json::json!({"id": "mod-001", "title": "Mod 1", "description": "desc", "objectives": ["obj1"]})],
            edges: vec![],
            export_version: "0.1.0".to_string(),
            exported_at: "2026-07-01T00:00:00Z".to_string(),
            exported_from: "imported:test-pack".to_string(),
            blocks,
            labs: HashMap::new(),
            videos: HashMap::new(),
        }
    }

    /// serialize_export returns Ok JSON with camelCase keys; round-trip
    /// from_str::<CourseExportPayload> recovers the struct.
    #[test]
    fn serialize_export_camel_case_round_trip() {
        let payload = sample_payload();
        let json = serialize_export(&payload).expect("serialize_export must succeed");

        // camelCase keys
        assert!(
            json.contains("exportVersion"),
            "must contain exportVersion; got: {json}"
        );
        assert!(
            json.contains("exportedFrom"),
            "must contain exportedFrom; got: {json}"
        );
        assert!(
            json.contains("exportedAt"),
            "must contain exportedAt; got: {json}"
        );
        assert!(
            json.contains("domain_module"),
            "must contain domain_module (schema-aligned snake_case key — see field-level rename override); got: {json}"
        );

        // round-trip
        let back: CourseExportPayload =
            serde_json::from_str(&json).expect("round-trip from_str must succeed");
        assert_eq!(back.id, payload.id);
        assert_eq!(back.export_version, payload.export_version);
        assert_eq!(back.exported_from, payload.exported_from);
        assert_eq!(back.blocks.len(), 1);
    }

    /// payload with one ExportedBlock{status:"ready"} serializes/deserializes
    /// with blockType/paramsJson/payloadJson camelCase keys.
    #[test]
    fn exported_block_camel_case_keys() {
        let block = sample_block("ready");
        let json = serde_json::to_string(&block).expect("block serialize must succeed");

        assert!(json.contains("blockType"), "must contain blockType");
        assert!(json.contains("paramsJson"), "must contain paramsJson");
        assert!(json.contains("payloadJson"), "must contain payloadJson");
        assert!(json.contains("moduleId"), "must contain moduleId");
        assert!(json.contains("sourceAnchorsJson"), "must contain sourceAnchorsJson");
        assert!(json.contains("metadataJson"), "must contain metadataJson");
        assert!(json.contains("retryCount"), "must contain retryCount");
        assert!(json.contains("createdAt"), "must contain createdAt");
        assert!(json.contains("updatedAt"), "must contain updatedAt");

        // round-trip
        let back: ExportedBlock = serde_json::from_str(&json).expect("block deserialize must succeed");
        assert_eq!(back.id, block.id);
        assert_eq!(back.block_type, block.block_type);
        assert_eq!(back.status, "ready");
        assert_eq!(back.params_json, block.params_json);
        assert_eq!(back.payload_json, block.payload_json);
    }

    /// ExportedLab.image None serializes as JSON null;
    /// Some("repo/img:tag") as the string verbatim (D-03).
    #[test]
    fn exported_lab_image_none_is_null_some_is_string() {
        let lab_none = ExportedLab {
            slug: "lab-01".to_string(),
            title: "Lab 1".to_string(),
            requires_docker: false,
            image: None,
            steps: vec![],
        };
        let lab_some = ExportedLab {
            slug: "lab-02".to_string(),
            title: "Lab 2".to_string(),
            requires_docker: true,
            image: Some("docker.io/learnforge/lab:latest".to_string()),
            steps: vec![],
        };

        let json_none = serde_json::to_string(&lab_none).expect("serialize None lab");
        assert!(
            json_none.contains("\"image\":null"),
            "image: None must serialize as null; got: {json_none}"
        );

        let json_some = serde_json::to_string(&lab_some).expect("serialize Some lab");
        assert!(
            json_some.contains("\"image\":\"docker.io/learnforge/lab:latest\""),
            "image: Some(str) must serialize as string; got: {json_some}"
        );

        // round-trip None
        let back_none: ExportedLab =
            serde_json::from_str(&json_none).expect("deserialize None lab");
        assert!(back_none.image.is_none());

        // round-trip Some
        let back_some: ExportedLab =
            serde_json::from_str(&json_some).expect("deserialize Some lab");
        assert_eq!(
            back_some.image.as_deref(),
            Some("docker.io/learnforge/lab:latest")
        );
    }

    /// ExportError displays correct messages for both variants.
    #[test]
    fn export_error_display() {
        let e1 = ExportError::Serialize("bad float".to_string());
        assert_eq!(
            format!("{e1}"),
            "export serialization failed: bad float"
        );

        let e2 = ExportError::Validation("status must be ready".to_string());
        assert_eq!(
            format!("{e2}"),
            "export validation failed: status must be ready"
        );
    }
}
