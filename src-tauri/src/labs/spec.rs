//! # labs::spec — LAB.md parser (Phase 03.1, Wave 0 stub)
//!
//! `parse_lab_md` is the entry point. Wave 1 (03.1-02) wires `gray_matter
//! 0.2.x` MIT for YAML frontmatter parsing and `pulldown-cmark` (already
//! transitively available) for the body. The parser must:
//! - reject malformed YAML
//! - reject specs that set BOTH `image` and `dockerfile` (XOR)
//! - reject `creates: []` entries that are absolute or contain `..`
//! - require `ai_judge` criteria with at least one non-trivial sentence
//!
//! Frontmatter schema (RESEARCH.md § LAB.md Schema):
//!
//! ```yaml
//! slug: pod-create-and-inspect
//! title: Create and inspect a Pod
//! image: kindest/node:v1.30   # XOR with dockerfile
//! requires_docker: true
//! creates: [manifests/pod.yaml, notes/run-output.txt]
//! steps:
//!   - id: write-manifest
//!     title: ...
//!     check:
//!       kind: file_state
//!       path: manifests/pod.yaml
//!       contains: "kind: Pod"
//!     hints: [..., ..., ...]
//! ```

use super::LabError;
use serde::{Deserialize, Serialize};

/// Parsed LAB.md spec. Wave 1 fills in the impl; Wave 0 keeps the
/// shape stable so downstream files can reference it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabSpec {
    pub slug: String,
    pub title: String,
    /// Public registry tag — XOR with `dockerfile`.
    pub image: Option<String>,
    /// Inline Dockerfile string — XOR with `image`.
    pub dockerfile: Option<String>,
    /// Whether the lab needs an actual container (kindest, k3d, ...).
    pub requires_docker: bool,
    /// Files this lab produces — used by the surgical reset.
    pub creates: Vec<String>,
    pub steps: Vec<LabStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabStep {
    pub id: String,
    pub title: String,
    pub check: StepCheck,
    pub hints: Vec<String>,
}

/// One of four check kinds. Tagged enum: serde uses
/// `#[serde(tag = "kind", rename_all = "snake_case")]` on the wire so
/// LAB.md frontmatter parses naturally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StepCheck {
    /// Last command output matches the regex pattern.
    CommandRegex {
        pattern: String,
        /// When true, also match against stderr; default false.
        #[serde(default)]
        match_stderr: bool,
    },
    /// Last command exited with this code (typically 0).
    ExitCode { expected: i32 },
    /// Workspace-relative file exists and contains the substring.
    FileState {
        path: String,
        #[serde(default)]
        contains: Option<String>,
    },
    /// Last-resort LLM grader. `criteria` is the rubric; `threshold` is the
    /// pass/fail score in [0.0, 1.0].
    AiJudge {
        criteria: String,
        #[serde(default = "default_threshold")]
        threshold: f64,
    },
}

fn default_threshold() -> f64 {
    0.7
}

/// Parse a LAB.md document (frontmatter + body). Wave 1 implements via
/// `gray_matter`. Wave 0 stub returns Err so all spec tests fail.
pub fn parse_lab_md(_text: &str) -> Result<LabSpec, LabError> {
    Err(LabError::Spec(
        "parse_lab_md: implemented in 03.1-02 (gray_matter)".to_string(),
    ))
}

/// Validate the parsed spec. Separate from parse_lab_md so 03.1-02 can
/// unit-test the rules without re-parsing.
pub fn validate_spec(_spec: &LabSpec) -> Result<(), LabError> {
    Err(LabError::Spec(
        "validate_spec: implemented in 03.1-02".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_LAB_MD: &str =
        include_str!("../../tests/fixtures/labs/specs/valid-pod-create.lab.md");
    const MALFORMED_LAB_MD: &str =
        include_str!("../../tests/fixtures/labs/specs/malformed-frontmatter.lab.md");
    const BOTH_IMAGE_DOCKERFILE: &str =
        include_str!("../../tests/fixtures/labs/specs/image-and-dockerfile-both.lab.md");
    const TRAVERSAL_LAB_MD: &str =
        include_str!("../../tests/fixtures/labs/specs/creates-traversal.lab.md");

    /// LAB-04 — valid LAB.md parses to a 4-step LabSpec with the expected
    /// slug + title.
    #[test]
    fn parse_valid_lab_md() {
        let spec = parse_lab_md(VALID_LAB_MD)
            .expect("valid LAB.md must parse once 03.1-02 lands");
        assert_eq!(spec.slug, "pod-create-and-inspect");
        assert_eq!(spec.title, "Create and inspect a Pod");
        assert_eq!(spec.steps.len(), 4, "valid LAB.md has 4 steps");
        assert_eq!(spec.requires_docker, true);
        assert!(spec.creates.iter().any(|p| p == "manifests/pod.yaml"));
    }

    /// LAB-04 — malformed YAML frontmatter surfaces LabError::Spec.
    #[test]
    fn parse_malformed_frontmatter_fails() {
        let result = parse_lab_md(MALFORMED_LAB_MD);
        match result {
            Err(LabError::Spec(_)) => {}
            other => panic!(
                "malformed frontmatter must produce LabError::Spec, got {:?}",
                other
            ),
        }
    }

    /// LAB-04 — image and dockerfile are mutually exclusive.
    #[test]
    fn image_xor_dockerfile() {
        let result = parse_lab_md(BOTH_IMAGE_DOCKERFILE);
        assert!(
            result.is_err(),
            "spec setting both image and dockerfile must be rejected"
        );
    }

    /// LAB-04 / LAB-07 — creates: [absolute, ..traversal] is rejected so
    /// the surgical reset cannot escape the workspace.
    #[test]
    fn creates_path_traversal_rejected() {
        let result = parse_lab_md(TRAVERSAL_LAB_MD);
        assert!(
            result.is_err(),
            "creates entries with absolute or traversal paths must be rejected"
        );
        match result {
            Err(LabError::Spec(msg)) => {
                let lower = msg.to_lowercase();
                assert!(
                    lower.contains("creates")
                        || lower.contains("path")
                        || lower.contains("traversal"),
                    "error message should explain the rule, got: {}",
                    msg
                );
            }
            Err(other) => panic!("expected LabError::Spec, got {:?}", other),
            Ok(_) => unreachable!(),
        }
    }

    /// LAB-04 / LAB-06 — ai_judge criteria string must be substantive
    /// (e.g. > 16 chars). One-word criteria is too lax to grade.
    #[test]
    fn ai_judge_criteria_minimum_length() {
        let too_short = StepCheck::AiJudge {
            criteria: "ok".to_string(),
            threshold: 0.7,
        };
        // Wrap in a minimal LabSpec so we can call validate_spec.
        let spec = LabSpec {
            slug: "x".to_string(),
            title: "x".to_string(),
            image: Some("alpine".to_string()),
            dockerfile: None,
            requires_docker: false,
            creates: vec![],
            steps: vec![LabStep {
                id: "s1".to_string(),
                title: "s1".to_string(),
                check: too_short,
                hints: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            }],
        };
        let result = validate_spec(&spec);
        assert!(
            result.is_err(),
            "ai_judge criteria of <= 2 chars must fail validation"
        );
    }

    /// LAB-04 — StepCheck round-trips through serde with `kind: snake_case`
    /// tag. Wave 0 sanity check (this should pass — flag in SUMMARY).
    #[test]
    fn step_check_serializes_with_snake_case_kind() {
        let regex = StepCheck::CommandRegex {
            pattern: "pod/web (created|configured)".to_string(),
            match_stderr: false,
        };
        let json = serde_json::to_string(&regex).unwrap();
        assert!(
            json.contains("\"kind\":\"command_regex\""),
            "command_regex tag must be snake_case, got {}",
            json
        );

        let exit = StepCheck::ExitCode { expected: 0 };
        let json = serde_json::to_string(&exit).unwrap();
        assert!(
            json.contains("\"kind\":\"exit_code\""),
            "exit_code tag must be snake_case, got {}",
            json
        );

        let file = StepCheck::FileState {
            path: "manifests/pod.yaml".to_string(),
            contains: Some("kind: Pod".to_string()),
        };
        let json = serde_json::to_string(&file).unwrap();
        assert!(
            json.contains("\"kind\":\"file_state\""),
            "file_state tag must be snake_case, got {}",
            json
        );

        let ai = StepCheck::AiJudge {
            criteria: "explain what the output shows about Pod scheduling".to_string(),
            threshold: 0.7,
        };
        let json = serde_json::to_string(&ai).unwrap();
        assert!(
            json.contains("\"kind\":\"ai_judge\""),
            "ai_judge tag must be snake_case, got {}",
            json
        );
    }
}
