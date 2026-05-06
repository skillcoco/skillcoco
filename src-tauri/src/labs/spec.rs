//! # labs::spec — LAB.md parser (Phase 03.1, Wave 2)
//!
//! `parse_lab_md` parses LAB.md frontmatter+body via `gray_matter` 0.2.
//! Validation rules (RESEARCH § LAB.md Schema):
//! - reject malformed YAML
//! - exactly one of `image` / `dockerfile` (XOR)
//! - `creates: []` entries must be workspace-relative (no leading `/`, no `..`)
//! - slug `^[a-z0-9-]+$`; steps non-empty + unique ids
//! - ai_judge criteria must be at least 16 chars

use super::LabError;
use gray_matter::engine::YAML;
use gray_matter::Matter;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const AI_JUDGE_CRITERIA_MIN_LEN: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabSpec {
    pub slug: String,
    pub title: String,
    pub image: Option<String>,
    pub dockerfile: Option<String>,
    pub requires_docker: bool,
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

/// One of four check kinds. Tagged enum so LAB.md frontmatter parses naturally:
/// `kind: command_regex | exit_code | file_state | ai_judge`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StepCheck {
    CommandRegex {
        pattern: String,
        #[serde(default)]
        match_stderr: bool,
    },
    ExitCode {
        expected: i32,
    },
    FileState {
        path: String,
        #[serde(default)]
        contains: Option<String>,
    },
    AiJudge {
        criteria: String,
        #[serde(default = "default_threshold")]
        threshold: f64,
    },
}

fn default_threshold() -> f64 {
    0.7
}

#[derive(Debug, Deserialize)]
struct LabSpecRaw {
    slug: String,
    title: String,
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    dockerfile: Option<String>,
    #[serde(default)]
    requires_docker: bool,
    #[serde(default)]
    creates: Vec<String>,
    steps: Vec<LabStepRaw>,
}

#[derive(Debug, Deserialize)]
struct LabStepRaw {
    id: String,
    title: String,
    check: StepCheck,
    #[serde(default)]
    hints: Vec<String>,
}

/// Parse a LAB.md document. Returns the typed spec alongside the original
/// markdown body so callers can store it in `paramsJson` for regen / display.
pub fn parse_lab_md(text: &str) -> Result<(LabSpec, String), LabError> {
    let parsed = Matter::<YAML>::new().parse(text);
    let pod = parsed.data.ok_or_else(|| {
        LabError::Spec("missing or empty YAML frontmatter (delimited by `---`)".to_string())
    })?;
    let raw: LabSpecRaw = pod
        .deserialize()
        .map_err(|e| LabError::Spec(format!("invalid YAML frontmatter: {}", e)))?;

    validate_image_xor(&raw.image, &raw.dockerfile)?;
    validate_slug(&raw.slug)?;
    if raw.steps.is_empty() {
        return Err(LabError::Spec("spec must declare at least one step".to_string()));
    }
    let mut seen = HashSet::new();
    for s in &raw.steps {
        if !seen.insert(s.id.as_str()) {
            return Err(LabError::Spec(format!("duplicate step id: {:?}", s.id)));
        }
        validate_step_check(&s.check)?;
    }
    validate_creates(&raw.creates)?;

    let spec = LabSpec {
        slug: raw.slug,
        title: raw.title,
        image: raw.image,
        dockerfile: raw.dockerfile,
        requires_docker: raw.requires_docker,
        creates: raw.creates,
        steps: raw
            .steps
            .into_iter()
            .map(|s| LabStep {
                id: s.id,
                title: s.title,
                check: s.check,
                hints: s.hints,
            })
            .collect(),
    };
    Ok((spec, parsed.content))
}

/// Re-validate a parsed spec (e.g. round-tripped from DB JSON).
pub fn validate_spec(spec: &LabSpec) -> Result<(), LabError> {
    validate_image_xor(&spec.image, &spec.dockerfile)?;
    validate_slug(&spec.slug)?;
    if spec.steps.is_empty() {
        return Err(LabError::Spec("spec must declare at least one step".to_string()));
    }
    let mut seen = HashSet::new();
    for s in &spec.steps {
        if !seen.insert(s.id.as_str()) {
            return Err(LabError::Spec(format!("duplicate step id: {:?}", s.id)));
        }
        validate_step_check(&s.check)?;
    }
    validate_creates(&spec.creates)
}

fn validate_image_xor(image: &Option<String>, dockerfile: &Option<String>) -> Result<(), LabError> {
    match (image.is_some(), dockerfile.is_some()) {
        (true, true) => Err(LabError::Spec(
            "image and dockerfile are mutually exclusive — declare one, not both".to_string(),
        )),
        (false, false) => Err(LabError::Spec(
            "spec must declare either `image` or `dockerfile`".to_string(),
        )),
        _ => Ok(()),
    }
}

fn validate_slug(slug: &str) -> Result<(), LabError> {
    if slug.trim().is_empty() {
        return Err(LabError::Spec("spec `slug` must not be empty".to_string()));
    }
    if !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(LabError::Spec(format!(
            "spec `slug` must match ^[a-z0-9-]+$, got {:?}",
            slug
        )));
    }
    Ok(())
}

/// Workspace-relative path validation. Used at parse time AND defense-in-depth
/// at reset time. Rejects empty, null-byte, absolute (`/`, `\`, `C:`), and
/// any segment equal to `..`.
pub fn validate_creates(creates: &[String]) -> Result<(), LabError> {
    for entry in creates {
        if entry.is_empty() {
            return Err(LabError::Spec("creates: empty path entry rejected".to_string()));
        }
        if entry.contains('\0') {
            return Err(LabError::Spec(format!("creates: null byte in path: {:?}", entry)));
        }
        if entry.starts_with('/') || entry.starts_with('\\') {
            return Err(LabError::Spec(format!(
                "creates: absolute paths rejected, got {:?}",
                entry
            )));
        }
        if entry.len() >= 2 && entry.chars().nth(1) == Some(':') {
            return Err(LabError::Spec(format!(
                "creates: absolute paths rejected, got {:?}",
                entry
            )));
        }
        for component in entry.split(|c| c == '/' || c == '\\') {
            if component == ".." {
                return Err(LabError::Spec(format!(
                    "creates: path traversal (..) rejected in {:?}",
                    entry
                )));
            }
        }
    }
    Ok(())
}

fn validate_step_check(check: &StepCheck) -> Result<(), LabError> {
    match check {
        StepCheck::AiJudge { criteria, .. } => {
            if criteria.trim().chars().count() < AI_JUDGE_CRITERIA_MIN_LEN {
                return Err(LabError::Spec(format!(
                    "ai_judge criteria must be at least {} chars; got {:?}",
                    AI_JUDGE_CRITERIA_MIN_LEN, criteria
                )));
            }
        }
        StepCheck::CommandRegex { pattern, .. } => {
            if pattern.trim().is_empty() {
                return Err(LabError::Spec(
                    "command_regex pattern must not be empty".to_string(),
                ));
            }
        }
        StepCheck::FileState { path, .. } => {
            if path.trim().is_empty() {
                return Err(LabError::Spec(
                    "file_state path must not be empty".to_string(),
                ));
            }
        }
        StepCheck::ExitCode { .. } => {}
    }
    Ok(())
}

/// Surgical reset — delete only the files declared in `creates`, joined under
/// `workspace`. Returns paths actually removed. Validates `creates` defensively
/// so a spec mutated post-parse cannot escape the workspace.
pub fn reset_lab(workspace: &Path, creates: &[String]) -> Result<Vec<PathBuf>, LabError> {
    validate_creates(creates)?;
    let mut removed = Vec::new();
    for entry in creates {
        let resolved = workspace.join(entry);
        if !resolved.exists() {
            continue;
        }
        if resolved.is_dir() {
            std::fs::remove_dir_all(&resolved)?;
        } else {
            std::fs::remove_file(&resolved)?;
        }
        removed.push(resolved);
    }
    Ok(removed)
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

    fn assert_spec_err_msg(result: Result<(LabSpec, String), LabError>, needles: &[&str]) {
        match result {
            Err(LabError::Spec(msg)) => {
                let lower = msg.to_lowercase();
                assert!(
                    needles.iter().any(|n| lower.contains(n)),
                    "expected one of {:?} in error, got: {}",
                    needles,
                    msg
                );
            }
            other => panic!("expected LabError::Spec, got {:?}", other),
        }
    }

    /// LAB-04 — valid LAB.md parses to a 4-step LabSpec.
    #[test]
    fn parse_valid_lab_md() {
        let (spec, body) = parse_lab_md(VALID_LAB_MD).expect("valid LAB.md must parse");
        assert_eq!(spec.slug, "pod-create-and-inspect");
        assert_eq!(spec.title, "Create and inspect a Pod");
        assert_eq!(spec.steps.len(), 4);
        assert_eq!(spec.image.as_deref(), Some("kindest/node:v1.30"));
        assert!(spec.dockerfile.is_none());
        assert!(spec.requires_docker);
        assert!(spec.creates.iter().any(|p| p == "manifests/pod.yaml"));
        assert!(spec.creates.iter().any(|p| p == "notes/run-output.txt"));
        assert!(!body.trim().is_empty(), "markdown body must be preserved");
        assert!(
            body.contains("Step 1") || body.contains("# Create and inspect a Pod"),
            "body must include markdown headings, got {:?}",
            body
        );
    }

    /// LAB-04 — malformed YAML surfaces LabError::Spec with a useful message.
    #[test]
    fn parse_malformed_frontmatter_fails() {
        assert_spec_err_msg(
            parse_lab_md(MALFORMED_LAB_MD),
            &["frontmatter", "yaml", "missing", "invalid"],
        );
    }

    /// LAB-04 — image and dockerfile are mutually exclusive.
    #[test]
    fn image_xor_dockerfile() {
        match parse_lab_md(BOTH_IMAGE_DOCKERFILE) {
            Err(LabError::Spec(msg)) => {
                let lower = msg.to_lowercase();
                assert!(
                    lower.contains("image") && lower.contains("dockerfile"),
                    "error should mention both, got: {}",
                    msg
                );
            }
            other => panic!("expected LabError::Spec, got {:?}", other),
        }
    }

    /// LAB-04 / LAB-07 — creates with absolute or `..` paths is rejected.
    #[test]
    fn creates_path_traversal_rejected() {
        assert_spec_err_msg(
            parse_lab_md(TRAVERSAL_LAB_MD),
            &["creates", "absolute", "..", "traversal"],
        );
    }

    /// LAB-04 / LAB-06 — ai_judge criteria must be substantive.
    #[test]
    fn ai_judge_criteria_minimum_length() {
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
                check: StepCheck::AiJudge {
                    criteria: "ok".to_string(),
                    threshold: 0.7,
                },
                hints: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            }],
        };
        // Slug "x" passes (single char alphanumeric); ai_judge fails.
        assert!(validate_spec(&spec).is_err());
    }

    /// LAB-04 — StepCheck round-trips with snake_case `kind:` tag.
    #[test]
    fn step_check_serializes_with_snake_case_kind() {
        for (check, needle) in [
            (
                StepCheck::CommandRegex {
                    pattern: "x".to_string(),
                    match_stderr: false,
                },
                "\"kind\":\"command_regex\"",
            ),
            (StepCheck::ExitCode { expected: 0 }, "\"kind\":\"exit_code\""),
            (
                StepCheck::FileState {
                    path: "p".to_string(),
                    contains: None,
                },
                "\"kind\":\"file_state\"",
            ),
            (
                StepCheck::AiJudge {
                    criteria: "explain what the output shows about scheduling".to_string(),
                    threshold: 0.7,
                },
                "\"kind\":\"ai_judge\"",
            ),
        ] {
            let json = serde_json::to_string(&check).unwrap();
            assert!(json.contains(needle), "missing {} in {}", needle, json);
        }
    }

    /// LAB-07 — reset_lab is surgical: deletes ONLY declared creates,
    /// leaves sibling files untouched.
    #[test]
    fn reset_lab_surgical() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::write(root.join("foo.txt"), "foo").unwrap();
        std::fs::write(root.join("bar.txt"), "bar").unwrap();
        std::fs::create_dir_all(root.join("manifests")).unwrap();
        std::fs::write(root.join("manifests/pod.yaml"), "kind: Pod\n").unwrap();
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::write(root.join("notes/run-output.txt"), "Running\n").unwrap();

        let creates = vec![
            "manifests/pod.yaml".to_string(),
            "notes/run-output.txt".to_string(),
        ];
        let removed = reset_lab(root, &creates).expect("reset_lab must succeed");
        assert_eq!(removed.len(), 2);
        assert!(root.join("foo.txt").exists());
        assert!(root.join("bar.txt").exists());
        assert!(!root.join("manifests/pod.yaml").exists());
        assert!(!root.join("notes/run-output.txt").exists());
    }

    /// LAB-07 — reset_lab refuses absolute / traversal paths.
    #[test]
    fn reset_lab_rejects_unsafe_paths() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(reset_lab(tmp.path(), &["../etc/passwd".to_string()]).is_err());
        assert!(reset_lab(tmp.path(), &["/etc/passwd".to_string()]).is_err());
    }
}
