//! Pack data model — `Pack`, `PackModule`, `PackEdge`, `LoadedPack`, etc.
//!
//! Every IPC-bound struct uses `#[serde(rename_all = "camelCase")]` per the
//! project-wide convention (CONVENTIONS.md "Tauri IPC Serialization").
//!
//! Phase 7 Wave 7 (07-07) — note the naming overlap with the **trait**
//! [`crate::packs::loader::PackSource`]: the enum and the trait share the
//! identifier `PackSource` but live in distinct modules. Callers
//! disambiguate via fully-qualified paths
//! (`skillcoco_core::packs::PackSource` for the enum,
//! `skillcoco_core::packs::loader::PackSource` for the trait).

use serde::{Deserialize, Serialize};

/// On-disk pack shape (matches `topic-packs/<id>/pack.json`).
///
/// `pack_version` and `requires_docker` are D-01 extensions; both optional.
/// `additionalProperties: true` at the schema level (Q7 lock) means unknown
/// fields are preserved at parse time without erroring — Wave 1's loader
/// may choose to surface them as soft warnings.
///
/// **No `rename_all` here**: the on-disk schema is snake_case
/// (`domain_module`, `pack_version`, `estimated_hours`, ...) so the struct
/// fields match 1:1. The IPC-facing wrapper [`LoadedPack`] applies
/// `rename_all = "camelCase"` on its OWN fields (`validationStatus`,
/// `lastLoadedAt`); the nested `pack` blob keeps its snake_case shape
/// because that's how learners (and the future pack editor) author it.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pack {
    /// Canonical id (matches the directory name on disk).
    pub id: String,
    /// Human-readable display title.
    pub title: String,
    /// Free-form description shown in Settings.
    pub description: String,
    /// Coarse-grained domain bucket (e.g. `"devops"`, `"ai"`, …).
    pub domain_module: String,
    /// Optional estimated hours-to-complete; informational only.
    #[serde(default)]
    pub estimated_hours: Option<i64>,
    /// D-01 extension — required-with-default, enforced by schema default.
    #[serde(default = "default_pack_version")]
    pub pack_version: String,
    /// D-01 extension — informational only (Q5 lock); per-lab `requires_docker`
    /// is the authoritative runtime gate (Phase 03.1).
    #[serde(default)]
    pub requires_docker: bool,
    /// Ordered list of pack modules (sections of learning content).
    pub modules: Vec<PackModule>,
    /// Optional prerequisite edges (`from` → `to`, both `PackModule.id`).
    #[serde(default)]
    pub edges: Vec<PackEdge>,
}

fn default_pack_version() -> String {
    "1.0".to_string()
}

/// Per-module shape inside a `pack.json`. snake_case to match the on-disk
/// format (see [`Pack`] doc comment).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackModule {
    /// Module id — unique within the pack.
    pub id: String,
    /// Human-readable module title.
    pub title: String,
    /// Free-form description.
    pub description: String,
    /// Optional 1..=5 difficulty rating (schema-validated; out-of-range = soft warning).
    #[serde(default)]
    pub difficulty: Option<i64>,
    /// Optional estimated minutes-to-complete.
    #[serde(default)]
    pub estimated_minutes: Option<i64>,
    /// Learning objectives — at least one required.
    pub objectives: Vec<String>,
    /// Allowed exercise types (e.g. `["quiz", "lab"]`).
    #[serde(default)]
    pub exercise_types: Vec<String>,
}

/// Per-edge shape inside a `pack.json`. The fields `from`/`to` already
/// have no underscores so naming choice is moot.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackEdge {
    /// Source `PackModule.id`.
    pub from: String,
    /// Target `PackModule.id`.
    pub to: String,
}

/// Where this pack came from at load time. Derived by the loader,
/// NOT authored — schema lists it for completeness only.
///
/// Note: this enum is named `PackSource` for historical / call-site
/// compatibility. The trait that abstracts FS-backed pack discovery
/// (Wave 7 / R3 mitigation) is also named [`crate::packs::loader::PackSource`];
/// they live in different modules and never collide at use sites.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackSource {
    /// Compile-time-embedded bundled pack (lives in `topic-packs/<id>/`).
    Bundled,
    /// Runtime-discovered skill pack (lives in `~/.learnforge/skills/<id>/`).
    Skill,
}

/// Validation outcome surfaced in Settings + diagnostics.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    /// No schema violations.
    Ok,
    /// Optional-field violations only (D-07 soft path).
    Warnings,
    /// Required-field violations (D-07 strict path) — pack is sentinel-only.
    Errors,
}

/// In-memory pack record — the unit returned by `list_topic_packs` IPC and
/// persisted (without `pack.modules`) in the `topic_packs` SQLite table.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LoadedPack {
    /// The parsed pack body (or a sentinel skeleton when validation failed).
    pub pack: Pack,
    /// Origin (bundled vs. skill).
    pub source: PackSource,
    /// User-toggled enable/disable flag.
    pub enabled: bool,
    /// Schema-validation outcome.
    pub validation_status: ValidationStatus,
    /// Q4 lock — plain strings. Structured records can come later without a
    /// migration since this is JSON-serialized into a TEXT column.
    pub validation_messages: Vec<String>,
    /// RFC3339 timestamp of the most recent load attempt.
    pub last_loaded_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_source_serializes_snake_case() {
        let b = serde_json::to_string(&PackSource::Bundled).unwrap();
        let s = serde_json::to_string(&PackSource::Skill).unwrap();
        assert_eq!(b, "\"bundled\"");
        assert_eq!(s, "\"skill\"");
    }

    #[test]
    fn validation_status_serializes_snake_case() {
        let o = serde_json::to_string(&ValidationStatus::Ok).unwrap();
        let w = serde_json::to_string(&ValidationStatus::Warnings).unwrap();
        let e = serde_json::to_string(&ValidationStatus::Errors).unwrap();
        assert_eq!(o, "\"ok\"");
        assert_eq!(w, "\"warnings\"");
        assert_eq!(e, "\"errors\"");
    }

    #[test]
    fn loaded_pack_serializes_camel_case() {
        let lp = LoadedPack {
            pack: Pack {
                id: "x".to_string(),
                title: "t".to_string(),
                description: "d".to_string(),
                domain_module: "devops".to_string(),
                estimated_hours: None,
                pack_version: "1.0".to_string(),
                requires_docker: false,
                modules: vec![],
                edges: vec![],
            },
            source: PackSource::Bundled,
            enabled: true,
            validation_status: ValidationStatus::Ok,
            validation_messages: vec![],
            last_loaded_at: "2026-06-16T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&lp).unwrap();
        // IPC contract — camelCase keys
        assert!(json.contains("\"validationStatus\""), "missing validationStatus in {}", json);
        assert!(json.contains("\"validationMessages\""), "missing validationMessages in {}", json);
        assert!(json.contains("\"lastLoadedAt\""), "missing lastLoadedAt in {}", json);
    }

    #[test]
    fn pack_default_version_applied() {
        let json = r#"{
            "id": "x", "title": "t", "description": "d", "domain_module": "devops",
            "modules": []
        }"#;
        let p: Pack = serde_json::from_str(json).unwrap();
        assert_eq!(p.pack_version, "1.0");
        assert!(!p.requires_docker);
    }
}
