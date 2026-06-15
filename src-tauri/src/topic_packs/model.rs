//! Pack data model — `Pack`, `PackModule`, `PackEdge`, `LoadedPack`, etc.
//!
//! Every IPC-bound struct uses `#[serde(rename_all = "camelCase")]` per the
//! project-wide convention (CONVENTIONS.md "Tauri IPC Serialization").

use serde::{Deserialize, Serialize};

/// On-disk pack shape (matches `topic-packs/<id>/pack.json`).
///
/// `pack_version` and `requires_docker` are D-01 extensions; both optional.
/// `additionalProperties: true` at the schema level (Q7 lock) means unknown
/// fields are preserved at parse time without erroring — Wave 1's loader
/// may choose to surface them as soft warnings.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub id: String,
    pub title: String,
    pub description: String,
    pub domain_module: String,
    #[serde(default)]
    pub estimated_hours: Option<i64>,
    /// D-01 extension — required-with-default, enforced by schema default.
    #[serde(default = "default_pack_version")]
    pub pack_version: String,
    /// D-01 extension — informational only (Q5 lock); per-lab `requires_docker`
    /// is the authoritative runtime gate (Phase 03.1).
    #[serde(default)]
    pub requires_docker: bool,
    pub modules: Vec<PackModule>,
    #[serde(default)]
    pub edges: Vec<PackEdge>,
}

fn default_pack_version() -> String {
    "1.0".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackModule {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub difficulty: Option<i64>,
    #[serde(default)]
    pub estimated_minutes: Option<i64>,
    pub objectives: Vec<String>,
    #[serde(default)]
    pub exercise_types: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackEdge {
    pub from: String,
    pub to: String,
}

/// Where this pack came from at load time. Derived by the loader,
/// NOT authored — schema lists it for completeness only.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackSource {
    Bundled,
    Skill,
}

/// Validation outcome surfaced in Settings + diagnostics.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Ok,
    Warnings,
    Errors,
}

/// In-memory pack record — the unit returned by `list_topic_packs` IPC and
/// persisted (without `pack.modules`) in the `topic_packs` SQLite table.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LoadedPack {
    pub pack: Pack,
    pub source: PackSource,
    pub enabled: bool,
    pub validation_status: ValidationStatus,
    /// Q4 lock — plain strings. Structured records can come later without a
    /// migration since this is JSON-serialized into a TEXT column.
    pub validation_messages: Vec<String>,
    pub last_loaded_at: String,
}
