//! # labs::pageplanner_labs — PagePlanner labs[] extension (Phase 03.1, Wave 2b)
//!
//! Wires `LabOutlineItem`, the topic-pack manifest loader/override, and the
//! per-lab LLM generator into the PagePlanner pipeline. The verbatim labs
//! prompt rule lives in `build_labs_prompt_rule()` so `commands/blocks.rs`
//! stays under its ≤ 60-line net-add budget. Tests inject `LabContentRunner`
//! mocks (mirrors evaluator's `AiJudgeRunner` from 03.1-03) — zero real LLM.

use super::spec::{parse_lab_md, LabSpec};
use super::LabError;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// One entry in `PagePlannerOutline.labs[]`. Stored in the lab block's
/// `params_json`. All fields except `slug` / `title` are `#[serde(default)]`
/// for round-trip robustness against older or pack-supplied entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabOutlineItem {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub dockerfile: Option<String>,
    /// One-line "why this lab teaches X".
    #[serde(default)]
    pub rationale: String,
    /// Legacy alias for `rationale`; kept for Wave 0 fixture back-compat.
    #[serde(default)]
    pub objective: String,
    #[serde(default)]
    pub requires_docker: bool,
    /// 5-15 minutes per RESEARCH.
    #[serde(default)]
    pub estimated_minutes: u32,
    /// 4-8 per RESEARCH; downstream LAB.md generator emits actual steps.
    #[serde(default = "default_step_count_target")]
    pub step_count_target: u32,
    /// Apple-Silicon platform tag (e.g. `linux/amd64` for `kindest/node`).
    /// RESEARCH q9 / 03.1-01 ledger.
    #[serde(default)]
    pub platform: Option<String>,
}

fn default_step_count_target() -> u32 {
    6
}

/// Topic-pack manifest schema (locked in 03.1-01 fixture):
///
/// ```yaml
/// schema_version: 1
/// labs:
///   pods-101:
///     - pod-create-and-inspect
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TopicPackManifest {
    pub schema_version: i32,
    #[serde(default)]
    pub labs: std::collections::BTreeMap<String, Vec<String>>,
}

/// Parse the manifest YAML. Tests pass `include_str!` of the fixture;
/// production callers read `topic-packs/<pack>/manifest.yaml`.
pub fn parse_manifest_yaml(yaml: &str) -> Result<TopicPackManifest, LabError> {
    serde_yaml::from_str(yaml)
        .map_err(|e| LabError::Spec(format!("topic-pack manifest YAML parse failed: {}", e)))
}

/// Replace AI labs with the pack's curated slugs when the manifest covers
/// `module_slug`; otherwise return the AI labs unchanged.
pub fn apply_topic_pack_override(
    ai_labs: &[LabOutlineItem],
    manifest: &TopicPackManifest,
    module_slug: &str,
) -> Vec<LabOutlineItem> {
    match manifest.labs.get(module_slug) {
        Some(pack_slugs) => pack_slugs
            .iter()
            .map(|slug| LabOutlineItem {
                slug: slug.clone(),
                title: slug_to_title(slug),
                image: None,
                dockerfile: None,
                rationale: format!("Topic-pack-curated lab: {}", slug),
                objective: String::new(),
                requires_docker: false,
                estimated_minutes: 0,
                step_count_target: default_step_count_target(),
                platform: None,
            })
            .collect(),
        None => ai_labs.to_vec(),
    }
}

fn slug_to_title(slug: &str) -> String {
    slug.split('-')
        .map(|w| {
            let mut chars = w.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Verbatim labs-emission rule from RESEARCH § PagePlanner Extension. Held
/// here (not inline in `commands/blocks.rs`) to honor blocks.rs ≤ 60-line
/// budget per RESEARCH § risk row 1167.
pub fn build_labs_prompt_rule() -> &'static str {
    r#"
- labs: 0-2 hands-on lab specs PER MODULE. Emit a lab ONLY when:
    * the module is about a CLI tool, runtime, framework, or hands-on system
      (Kubernetes, Docker, Terraform, Linux, kubectl, npm, cargo, Rust ownership,
      Go goroutines, Python virtualenv, etc.)
    * the learning objectives include verbs like "create", "deploy", "configure",
      "debug", "inspect", "run"
  Emit ZERO labs for pure-theory modules ("history of …", "principles of …",
  "comparing X vs Y"). When in doubt, emit zero — the cost of a missing lab is
  small; the cost of a useless lab is high.

- For each lab:
    * slug: kebab-case, 3-6 words.
    * title: 4-8 words, action-verb first ("Create and inspect a Pod").
    * rationale: ONE sentence.
    * image: prefer a public registry tag (kindest/node, alpine, rust:slim,
      python:3.12, golang:1.22). Use `dockerfile` only when no public image fits.
    * dockerfile (if used): minimal, 3-8 lines, no entrypoint (we run a shell).
    * requires_docker: true ONLY for cluster/system labs (kindest/node, etc.);
      false for `python:3.12` / `node:20` / `alpine` style.
    * estimated_minutes: 5-15.
    * step_count_target: 4-8.
"#
}

/// Append the labs-emission rule to a base PagePlanner prompt, or return the
/// base unchanged when labs are disabled (track-level opt-out).
pub fn extend_page_planner_prompt(base: &str, labs_enabled: bool) -> String {
    if labs_enabled {
        let mut out = String::with_capacity(base.len() + 1024);
        out.push_str(base);
        out.push_str(build_labs_prompt_rule());
        out
    } else {
        base.to_string()
    }
}

/// Build the per-lab LAB.md generator prompt — verbatim from RESEARCH § Lab
/// content generator prompt with `{module_title}`, `{lab_title}`,
/// `{step_count_target}`, `{image_or_dockerfile_decision}`, `{requires_docker}`
/// interpolated.
pub fn build_lab_content_prompt(module_title: &str, lab: &LabOutlineItem) -> String {
    let image_or_dockerfile_decision = match (&lab.image, &lab.dockerfile) {
        (Some(img), _) => format!("use the public image `{}`", img),
        (None, Some(_)) => "use the inline Dockerfile from the outline (do NOT regenerate it; \
                            copy the dockerfile field verbatim into the frontmatter)"
            .to_string(),
        (None, None) => {
            "pick a minimal public image appropriate for the lab (alpine, python:3.12, \
             node:20, rust:slim, golang:1.22, kindest/node:v1.30, ...)"
                .to_string()
        }
    };
    format!(
        r#"You are an expert hands-on instructor writing a step-by-step lab for the module
"{module_title}". The lab is "{lab_title}". The learner will type commands in
an integrated terminal; the system evaluates each step automatically.

Write a lab spec as YAML frontmatter + Markdown step bodies that conforms to
LearnForge's LAB.md schema:

  - 4-{step_count_target} steps, ordered foundational → advanced
  - Each step.check uses ONE of: command_regex | exit_code | file_state | ai_judge
  - PREFER deterministic checks (command_regex, exit_code, file_state).
    Use ai_judge ONLY when the step is open-ended (e.g. "explain output").
  - Every step has EXACTLY 3 hints: gentle nudge, partial answer, full solution.
  - Step prompts must be specific to {module_title} — no generic placeholders.
  - creates: list every file the steps tell the learner to create.
  - image: {image_or_dockerfile_decision}
  - requires_docker: {requires_docker}

Output: ONLY the LAB.md content (frontmatter + markdown). No JSON wrapper, no
preamble.
"#,
        module_title = module_title,
        lab_title = lab.title,
        step_count_target = lab.step_count_target.max(4),
        image_or_dockerfile_decision = image_or_dockerfile_decision,
        requires_docker = lab.requires_docker,
    )
}

/// Test seam for `generate_lab_with_client` — production wires this to
/// `crate::ai::retry::ai_request_with_retry`; tests inject closures.
/// Mirrors `AiJudgeRunner` from 03.1-03 (Wave 2a ledger).
pub trait LabContentRunner: Send + Sync {
    fn run<'a>(
        &'a self,
        prompt: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>>;
}

const MAX_LAB_GEN_ATTEMPTS: u32 = 2;

/// Generate a LAB.md for one outline item via the LLM and parse it. Returns
/// the typed spec + raw markdown (caller stores raw in `params_json` per
/// CONTEXT.md "AI-generated `LAB.md` lives in DB only"). On parse failure,
/// retries once with a "previous attempt failed" suffix; gives up after
/// `MAX_LAB_GEN_ATTEMPTS` and returns `LabError::Spec`.
pub async fn generate_lab_with_client<R: LabContentRunner + ?Sized>(
    module_title: &str,
    lab: &LabOutlineItem,
    runner: &R,
) -> Result<(LabSpec, String), LabError> {
    let base_prompt = build_lab_content_prompt(module_title, lab);
    let mut last_err: Option<String> = None;
    for attempt in 0..MAX_LAB_GEN_ATTEMPTS {
        let prompt = match &last_err {
            None => base_prompt.clone(),
            Some(err) => format!(
                "{}\n\nPrevious attempt failed validation with error:\n  {}\n\nFix the \
                 issue and return ONLY the corrected LAB.md content.",
                base_prompt, err
            ),
        };
        let response = runner
            .run(&prompt)
            .await
            .map_err(|e| LabError::Runtime(format!("lab generator LLM call failed: {}", e)))?;
        match parse_lab_md(&response) {
            Ok((spec, source)) => return Ok((spec, source)),
            Err(LabError::Spec(reason)) => {
                last_err = Some(reason);
                if attempt + 1 >= MAX_LAB_GEN_ATTEMPTS {
                    break;
                }
            }
            Err(other) => return Err(other),
        }
    }
    Err(LabError::Spec(format!(
        "lab generator failed after {} attempts: {}",
        MAX_LAB_GEN_ATTEMPTS,
        last_err.unwrap_or_else(|| "(no error captured)".to_string())
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    const KUBERNETES_MANIFEST: &str =
        include_str!("../../tests/fixtures/labs/manifests/kubernetes-manifest.yaml");
    const VALID_LAB_MD: &str =
        include_str!("../../tests/fixtures/labs/specs/valid-pod-create.lab.md");

    /// Closure-backed `LabContentRunner` for tests.
    struct MockRunner<F: Fn(&str) -> Result<String, String> + Send + Sync> {
        dispatcher: F,
        call_count: Arc<AtomicUsize>,
    }
    impl<F: Fn(&str) -> Result<String, String> + Send + Sync> MockRunner<F> {
        fn new(dispatcher: F) -> Self {
            Self {
                dispatcher,
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }
    impl<F: Fn(&str) -> Result<String, String> + Send + Sync> LabContentRunner for MockRunner<F> {
        fn run<'a>(
            &'a self,
            prompt: &'a str,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let result = (self.dispatcher)(prompt);
            Box::pin(async move { result })
        }
    }

    fn ai_lab_outline_item() -> LabOutlineItem {
        LabOutlineItem {
            slug: "ai-fallback".to_string(),
            title: "AI Fallback Lab".to_string(),
            image: Some("alpine".to_string()),
            dockerfile: None,
            rationale: "filler".to_string(),
            objective: String::new(),
            requires_docker: false,
            estimated_minutes: 10,
            step_count_target: 5,
            platform: None,
        }
    }

    #[test]
    fn manifest_yaml_loader_parses_module_to_lab_slug_map() {
        let manifest = parse_manifest_yaml(KUBERNETES_MANIFEST)
            .expect("manifest must parse once 03.1-04 lands");
        assert_eq!(manifest.schema_version, 1);
        let pods = manifest.labs.get("pods-101").expect("pods-101 entry");
        assert_eq!(pods.len(), 2);
        assert_eq!(pods[0], "pod-create-and-inspect");
        assert_eq!(pods[1], "pod-debug-with-kubectl");
    }

    #[test]
    fn topic_pack_overrides_ai() {
        let manifest = parse_manifest_yaml(KUBERNETES_MANIFEST).unwrap();
        let ai_labs = vec![ai_lab_outline_item()];
        let merged = apply_topic_pack_override(&ai_labs, &manifest, "pods-101");
        let slugs: Vec<&str> = merged.iter().map(|l| l.slug.as_str()).collect();
        assert_eq!(
            slugs,
            vec!["pod-create-and-inspect", "pod-debug-with-kubectl"],
            "manifest must replace AI labs for pods-101"
        );
    }

    #[test]
    fn topic_pack_does_not_touch_unrelated_modules() {
        let manifest = parse_manifest_yaml(KUBERNETES_MANIFEST).unwrap();
        let ai_labs = vec![ai_lab_outline_item()];
        let merged = apply_topic_pack_override(&ai_labs, &manifest, "deployments");
        let slugs: Vec<&str> = merged.iter().map(|l| l.slug.as_str()).collect();
        assert_eq!(slugs, vec!["deploy-rolling-update"]);
        assert!(!slugs.contains(&"ai-fallback"));
    }

    #[test]
    fn topic_pack_no_match_keeps_ai() {
        let manifest = parse_manifest_yaml(KUBERNETES_MANIFEST).unwrap();
        let ai_labs = vec![ai_lab_outline_item()];
        let merged = apply_topic_pack_override(&ai_labs, &manifest, "uncovered-module");
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].slug, "ai-fallback");
    }

    #[test]
    fn prompt_labs_optout() {
        let base = "Return JSON with lessons[] etc.".to_string();
        let with_labs = extend_page_planner_prompt(&base, true);
        assert!(with_labs.to_lowercase().contains("labs"));
        let without_labs = extend_page_planner_prompt(&base, false);
        assert!(!without_labs.to_lowercase().contains("labs"));
    }

    #[tokio::test]
    async fn generate_lab_with_client_parses_llm_output() {
        let canned = VALID_LAB_MD.to_string();
        let runner = MockRunner::new(move |_p: &str| Ok(canned.clone()));
        let lab = LabOutlineItem {
            slug: "pod-create-and-inspect".to_string(),
            title: "Create and inspect a Pod".to_string(),
            image: Some("kindest/node:v1.30".to_string()),
            dockerfile: None,
            rationale: "Hands-on Pod creation".to_string(),
            objective: String::new(),
            requires_docker: true,
            estimated_minutes: 12,
            step_count_target: 4,
            platform: None,
        };
        let (spec, source) = generate_lab_with_client("Kubernetes Pods", &lab, &runner)
            .await
            .expect("valid LAB.md must parse");
        assert_eq!(spec.slug, "pod-create-and-inspect");
        assert_eq!(spec.steps.len(), 4);
        assert_eq!(spec.image.as_deref(), Some("kindest/node:v1.30"));
        assert!(spec.requires_docker);
        assert!(!source.is_empty(), "source body must be preserved");
    }

    #[tokio::test]
    async fn generate_lab_with_client_retries_on_parse_failure() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_in = Arc::clone(&attempts);
        let valid_md = VALID_LAB_MD.to_string();
        let runner = MockRunner::new(move |_p: &str| {
            let n = attempts_in.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                Ok("This is not a LAB.md".to_string())
            } else {
                Ok(valid_md.clone())
            }
        });
        let lab = ai_lab_outline_item();
        let (spec, _) = generate_lab_with_client("Kubernetes Pods", &lab, &runner)
            .await
            .expect("second attempt must succeed");
        assert_eq!(spec.slug, "pod-create-and-inspect");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn generate_lab_with_client_fails_after_two_attempts() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_in = Arc::clone(&attempts);
        let runner = MockRunner::new(move |_p: &str| {
            attempts_in.fetch_add(1, Ordering::SeqCst);
            Ok("Still not valid LAB.md".to_string())
        });
        let lab = ai_lab_outline_item();
        let result = generate_lab_with_client("Kubernetes Pods", &lab, &runner).await;
        assert!(matches!(result, Err(LabError::Spec(_))));
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            MAX_LAB_GEN_ATTEMPTS as usize
        );
    }

    #[test]
    fn lab_outline_item_camel_case_roundtrip() {
        let item = LabOutlineItem {
            slug: "demo".to_string(),
            title: "Demo".to_string(),
            image: Some("alpine".to_string()),
            dockerfile: None,
            rationale: "why".to_string(),
            objective: String::new(),
            requires_docker: true,
            estimated_minutes: 10,
            step_count_target: 5,
            platform: Some("linux/amd64".to_string()),
        };
        let json = serde_json::to_string(&item).unwrap();
        for key in [
            "slug",
            "title",
            "image",
            "rationale",
            "requiresDocker",
            "estimatedMinutes",
            "stepCountTarget",
            "platform",
        ] {
            assert!(
                json.contains(&format!("\"{}\"", key)),
                "expected camelCase key {:?} in {}",
                key,
                json
            );
        }
        let round: LabOutlineItem = serde_json::from_str(&json).unwrap();
        assert_eq!(round, item);
    }
}
