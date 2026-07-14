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
    /// Phase 19 (EXAM-01/D-01/D-02) — pack-authored exam calibration.
    /// Populated from the `exam:` frontmatter block when present; a regular
    /// lab has no `exam:` block and stays `None` (D-02). Defaults for
    /// absent sub-fields (D-03 30min / D-08 70%) are resolved at scoring
    /// time (19-03), not baked in here — see `ExamMeta` doc comment.
    #[serde(default)]
    pub exam: Option<ExamMeta>,
    /// Phase 19.3 (D-03) — whole-lab default validation grain. Absent
    /// `grain:` frontmatter defaults to `Grain::Step` (`default_grain`) —
    /// this is the back-compat HARD GATE: every pre-19.3 LAB.md has no
    /// `grain:` key and must evaluate byte-identically to today.
    #[serde(default = "default_grain")]
    pub grain: Grain,
    pub steps: Vec<LabStep>,
}

/// Phase 19.3 (D-03) — per-step / per-lab validation grain.
///
/// `Step` (default): checks evaluate against the single most recent command
/// (`EvalContext.last_output`/`last_exit_code`) — today's behavior.
/// `Milestone`: checks evaluate against the cumulative per-session command
/// history + workspace tree, only on an explicit "Validate" action (D-04).
///
/// A step's *effective* grain is resolved via `effective_step_grain` below,
/// not by reading this field directly — see that function's doc comment for
/// the lab-level/step-level inheritance rule (D-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grain {
    Step,
    Milestone,
}

fn default_grain() -> Grain {
    Grain::Step
}

/// Phase 19.3 (D-03) — resolve a step's effective validation grain from the
/// lab-level default and the step's own (possibly-default) grain value.
///
/// Step-level `Milestone` always wins (a milestone step in an otherwise
/// step-grain lab validates as milestone). Otherwise the lab-level grain
/// applies — so a step in a milestone-grain lab inherits milestone even if
/// its own field reads `Grain::Step` (accepted simplification: an *explicit*
/// `grain: step` on a step inside a milestone-grain lab is NOT distinguishable
/// from "no grain authored" at this layer, since both parse to `Grain::Step`
/// on `LabStep.grain`. Both still collapse to the lab's milestone grain here
/// — D-04's UI only shows the Validate button driven by the lab's overall
/// milestone-ness, so this simplification doesn't lose author-visible
/// behavior in this phase).
pub fn effective_step_grain(lab_grain: Grain, step_grain: Grain) -> Grain {
    if step_grain == Grain::Milestone {
        Grain::Milestone
    } else {
        lab_grain
    }
}

/// Phase 19 (EXAM-01) — pack-authored exam calibration.
///
/// Both fields stay `Option` on the parsed struct (never defaulted here) so
/// "the pack author didn't specify a value" stays distinguishable from "the
/// pack author explicitly wants the default" — 19-03's scoring path resolves
/// `None` -> 30 min (D-03) / 70.0% (D-08).
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
    /// Phase 19 (D-07) — per-step scoring weight, authored via the
    /// `weight:` frontmatter value on the step. Defaults to 1.0 (equal
    /// weighting) when absent — `default_weight`.
    #[serde(default = "default_weight")]
    pub weight: f64,
    /// Phase 19.3 (D-03) — per-step validation grain override. Defaults to
    /// `Grain::Step` when absent (`default_grain`) — resolve the EFFECTIVE
    /// grain via `effective_step_grain(lab_grain, step.grain)`, not by
    /// reading this field alone.
    #[serde(default = "default_grain")]
    pub grain: Grain,
}

fn default_weight() -> f64 {
    1.0
}

/// One of five check kinds. Tagged enum so LAB.md frontmatter parses naturally:
/// `kind: command_regex | exit_code | file_state | ai_judge | command_absent`.
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
    /// Phase 19.2 (D-01) — deterministic "output must NOT match" check; the
    /// exact inverse of `CommandRegex`. Same field shape, same
    /// `#[serde(default)]` on `match_stderr`.
    CommandAbsent {
        pattern: String,
        #[serde(default)]
        match_stderr: bool,
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
    #[serde(default)]
    exam: Option<ExamMeta>,
    #[serde(default = "default_grain")]
    grain: Grain,
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
    #[serde(default = "default_weight")]
    weight: f64,
    #[serde(default = "default_grain")]
    grain: Grain,
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
        validate_step_weight(&s.id, s.weight)?;
        if s.prompt.trim().is_empty() {
            return Err(LabError::Spec(format!(
                "step {:?} is missing `prompt:` (the markdown body that tells \
                 the learner what to do — without it the lab is just titles)",
                s.id
            )));
        }
    }
    validate_creates(&raw.creates)?;
    if let Some(exam) = &raw.exam {
        validate_exam_meta(exam)?;
    }

    let spec = LabSpec {
        slug: raw.slug,
        title: raw.title,
        image: raw.image,
        dockerfile: raw.dockerfile,
        requires_docker: raw.requires_docker,
        creates: raw.creates,
        exam: raw.exam,
        grain: raw.grain,
        steps: raw
            .steps
            .into_iter()
            .map(|s| LabStep {
                id: s.id,
                title: s.title,
                prompt: s.prompt,
                check: s.check,
                hints: s.hints,
                weight: s.weight,
                grain: s.grain,
            })
            .collect(),
    };
    validate_milestone_exam_exclusion(&spec)?;
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
        validate_step_weight(&s.id, s.weight)?;
        if s.prompt.trim().is_empty() {
            return Err(LabError::Spec(format!(
                "step {:?} is missing `prompt:` (the markdown body that tells \
                 the learner what to do)",
                s.id
            )));
        }
    }
    validate_creates(&spec.creates)?;
    if let Some(exam) = &spec.exam {
        validate_exam_meta(exam)?;
    }
    validate_milestone_exam_exclusion(spec)?;
    Ok(())
}

/// Phase 19.3 (D-05) — author-time fail-closed gate: a spec cannot declare
/// BOTH `exam:` metadata AND any milestone grain (lab-level or any step).
/// Exams remain per-step grain only — see `exam_attempt_start_conn`'s
/// mirror-image runtime gate (D-05, T-19-10/12 exam-integrity posture) for
/// the belt-and-suspenders check at attempt-start.
fn validate_milestone_exam_exclusion(spec: &LabSpec) -> Result<(), LabError> {
    if spec.exam.is_none() {
        return Ok(());
    }
    let has_milestone =
        spec.grain == Grain::Milestone || spec.steps.iter().any(|s| s.grain == Grain::Milestone);
    if has_milestone {
        return Err(LabError::Spec(
            "D-05: `exam:` metadata and milestone grain cannot coexist in one spec — \
             exams remain per-step grain only"
                .to_string(),
        ));
    }
    Ok(())
}

/// Phase 19 (T-19-02) — range-bound exam calibration fields at the same
/// trust boundary as `validate_slug`/`validate_image_xor` (pack-author ->
/// LAB.md parser). Regular labs (`exam: None`) skip this entirely — D-13
/// behavior for non-exam specs is unaffected by construction.
fn validate_exam_meta(exam: &ExamMeta) -> Result<(), LabError> {
    if let Some(pct) = exam.pass_threshold_pct {
        if !(0.0..=100.0).contains(&pct) {
            return Err(LabError::Spec(format!(
                "exam.passThresholdPct must be within 0..=100, got {}",
                pct
            )));
        }
    }
    if let Some(minutes) = exam.time_limit_minutes {
        if !(1..=480).contains(&minutes) {
            return Err(LabError::Spec(format!(
                "exam.timeLimitMinutes must be within 1..=480, got {}",
                minutes
            )));
        }
    }
    Ok(())
}

/// Phase 19 (WR-02, D-07/T-19-02) — step weights feed directly into the
/// weighted-score numerator/denominator. Negative weights make mixed-sign
/// totals exceed 100% (trivially-true `passed`); NaN poisons the whole
/// score into a silent always-fail. Only positive finite weights are valid.
fn validate_step_weight(step_id: &str, weight: f64) -> Result<(), LabError> {
    if !weight.is_finite() || weight <= 0.0 {
        return Err(LabError::Spec(format!(
            "step {:?}: weight must be a positive finite number, got {}",
            step_id, weight
        )));
    }
    Ok(())
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
        StepCheck::CommandAbsent { pattern, .. } => {
            if pattern.trim().is_empty() {
                return Err(LabError::Spec(
                    "command_absent pattern must not be empty".to_string(),
                ));
            }
        }
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
