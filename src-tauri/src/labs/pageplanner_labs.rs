//! # labs::pageplanner_labs — PagePlanner labs[] extension (Phase 03.1, Wave 0 stub)
//!
//! Wave 1 (03.1-04) wires:
//! - `LabOutlineItem` into `PagePlannerOutline.labs[]` (already added in
//!   `commands/blocks.rs` — see plan 03.1-01 step 10)
//! - `build_page_planner_prompt(labs_enabled: bool)` extension that adds the
//!   labs-emission rules when enabled (skips them otherwise — feature flag
//!   for tracks where lab generation is off)
//! - `apply_topic_pack_override` that swaps AI-generated labs for the
//!   pack's curated lab slugs whenever the topic-pack manifest covers
//!   the module
//! - `generate_lab_with_client` — per-lab LLM call producing a LAB.md spec

use super::LabError;
use serde::{Deserialize, Serialize};

/// One entry in PagePlannerOutline.labs[]. Stored in the lab block's
/// params_json and used by `generate_lab_with_client` to produce the
/// LAB.md.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabOutlineItem {
    pub slug: String,
    pub title: String,
    /// Optional public registry tag. XOR with `dockerfile`.
    #[serde(default)]
    pub image: Option<String>,
    /// Optional inline Dockerfile string. XOR with `image`.
    #[serde(default)]
    pub dockerfile: Option<String>,
    /// One-line objective for the LLM lab generator.
    #[serde(default)]
    pub objective: String,
    /// Whether the lab needs Docker (kindest, k3d, ...).
    #[serde(default)]
    pub requires_docker: bool,
}

/// Topic-pack manifest schema (subset). Wave 1 extends as needed.
#[derive(Debug, Clone, Deserialize)]
pub struct TopicPackManifest {
    pub schema_version: i32,
    /// Maps module slug -> ordered list of lab slugs.
    #[serde(default)]
    pub labs: std::collections::BTreeMap<String, Vec<String>>,
}

/// Wave 0 stub — Wave 1 (03.1-04) calls AIClientTrait to generate the
/// LAB.md content for one outline item.
pub async fn generate_lab_with_client(
    _item: &LabOutlineItem,
    _module_title: &str,
) -> Result<String, LabError> {
    Err(LabError::Spec(
        "generate_lab_with_client: implemented in 03.1-04".to_string(),
    ))
}

/// Wave 0 stub — Wave 1 (03.1-04) implements the override: when the
/// pack manifest covers `module_slug`, replace the AI-generated lab list
/// with the pack's curated slugs.
pub fn apply_topic_pack_override(
    _ai_labs: &[LabOutlineItem],
    _manifest: &TopicPackManifest,
    _module_slug: &str,
) -> Vec<LabOutlineItem> {
    // Wave 0 returns an empty vec so the override test fails (expected
    // override to come from the manifest).
    Vec::new()
}

/// Wave 0 stub — Wave 1 (03.1-04) parses the YAML topic-pack manifest.
pub fn parse_manifest_yaml(_yaml: &str) -> Result<TopicPackManifest, LabError> {
    Err(LabError::Spec(
        "parse_manifest_yaml: implemented in 03.1-04".to_string(),
    ))
}

/// Wave 0 stub — Wave 1 (03.1-04) appends labs[]-emission rules to the
/// PagePlanner system prompt when `labs_enabled` is true.
pub fn extend_page_planner_prompt(_base_prompt: &str, _labs_enabled: bool) -> String {
    // Wave 0 returns the base prompt unchanged so the prompt_labs_optout
    // test fails (Wave 1 makes the labs-rule appear/disappear correctly).
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KUBERNETES_MANIFEST: &str =
        include_str!("../../tests/fixtures/labs/manifests/kubernetes-manifest.yaml");

    /// LAB-05 — manifest YAML parses module_slug -> [lab_slug] map.
    #[test]
    fn manifest_yaml_loader_parses_module_to_lab_slug_map() {
        let manifest = parse_manifest_yaml(KUBERNETES_MANIFEST)
            .expect("manifest must parse once 03.1-04 lands");
        assert_eq!(manifest.schema_version, 1);
        let pods = manifest
            .labs
            .get("pods-101")
            .expect("manifest must contain pods-101 entry");
        assert_eq!(pods.len(), 2);
        assert_eq!(pods[0], "pod-create-and-inspect");
        assert_eq!(pods[1], "pod-debug-with-kubectl");
    }

    /// LAB-05 — topic-pack manifest overrides AI-generated labs for
    /// covered modules.
    #[test]
    fn topic_pack_overrides_ai() {
        let manifest = parse_manifest_yaml(KUBERNETES_MANIFEST)
            .expect("manifest must parse once 03.1-04 lands");
        let ai_labs = vec![LabOutlineItem {
            slug: "ai-fallback".to_string(),
            title: "AI Fallback Lab".to_string(),
            image: Some("alpine".to_string()),
            dockerfile: None,
            objective: "filler".to_string(),
            requires_docker: false,
        }];
        let merged = apply_topic_pack_override(&ai_labs, &manifest, "pods-101");
        let slugs: Vec<&str> = merged.iter().map(|l| l.slug.as_str()).collect();
        assert_eq!(
            slugs,
            vec!["pod-create-and-inspect", "pod-debug-with-kubectl"],
            "manifest must replace AI labs for pods-101 module"
        );
    }

    /// LAB-05 — when labs_enabled=false, the system prompt does NOT
    /// contain the labs-emission rule. When true, it does.
    #[test]
    fn prompt_labs_optout() {
        let base = "Return JSON with lessons[] etc.".to_string();

        let with_labs = extend_page_planner_prompt(&base, true);
        assert!(
            with_labs.to_lowercase().contains("labs"),
            "labs_enabled=true must include labs rule in prompt, got:\n{}",
            with_labs
        );

        let without_labs = extend_page_planner_prompt(&base, false);
        assert!(
            !without_labs.to_lowercase().contains("labs"),
            "labs_enabled=false must omit labs rule from prompt, got:\n{}",
            without_labs
        );
    }
}
