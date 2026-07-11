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
    /// Phase 19 (EXAM-01/D-01/D-02) — Wave 0 scaffold field. `parse_lab_md`
    /// does NOT yet read the `exam:` frontmatter block into this field
    /// (always `None` today); 19-02 wires the real parsing + defaults
    /// (D-03 30min / D-08 70%) + range validation (T-19-02). Only authored
    /// exam specs populate this — a regular lab must stay `None` (D-02).
    #[serde(default)]
    pub exam: Option<ExamMeta>,
    pub steps: Vec<LabStep>,
}

/// Phase 19 (EXAM-01) — pack-authored exam calibration. Wave 0 scaffold
/// type only; 19-02 wires parsing/defaults/validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExamMeta {
    /// Minutes allotted for the attempt. `None` -> default 30 (D-03).
    pub time_limit_minutes: Option<u32>,
    /// Percentage required to pass. `None` -> default 70.0 (D-08).
    pub pass_threshold_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabStep {
    pub id: String,
    pub title: String,
    /// Markdown body for the step — what the learner reads in the
    /// instructions panel. Required: without prose, the lab reduces to
    /// "guess what to type" (LabInstructions renders this via Markdown).
    pub prompt: String,
    pub check: StepCheck,
    pub hints: Vec<String>,
    /// Phase 19 (D-07) — Wave 0 scaffold field. `parse_lab_md` does NOT yet
    /// read the per-step `weight:` frontmatter value into this field
    /// (always hardcoded to `1.0` today, never the authored value); 19-02
    /// wires the real serde default (1.0) + frontmatter round-trip.
    #[serde(default = "default_step_weight")]
    pub weight: f64,
}

fn default_step_weight() -> f64 {
    1.0
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
    #[serde(default)]
    prompt: String,
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
        if s.prompt.trim().is_empty() {
            return Err(LabError::Spec(format!(
                "step {:?} is missing `prompt:` (the markdown body that tells \
                 the learner what to do — without it the lab is just titles)",
                s.id
            )));
        }
    }
    validate_creates(&raw.creates)?;

    let spec = LabSpec {
        slug: raw.slug,
        title: raw.title,
        image: raw.image,
        dockerfile: raw.dockerfile,
        requires_docker: raw.requires_docker,
        creates: raw.creates,
        // Wave 0 scaffold — always None until 19-02 reads the `exam:`
        // frontmatter block (see LabSpec.exam doc comment).
        exam: None,
        steps: raw
            .steps
            .into_iter()
            .map(|s| LabStep {
                id: s.id,
                title: s.title,
                prompt: s.prompt,
                check: s.check,
                hints: s.hints,
                // Wave 0 scaffold — always 1.0 until 19-02 reads the
                // per-step `weight:` frontmatter value (see LabStep.weight
                // doc comment).
                weight: 1.0,
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
        if s.prompt.trim().is_empty() {
            return Err(LabError::Spec(format!(
                "step {:?} is missing `prompt:` (the markdown body that tells \
                 the learner what to do)",
                s.id
            )));
        }
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
#[path = "spec_tests.rs"]
mod tests;
