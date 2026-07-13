//! Tests for `labs::spec`. Extracted to a sibling file (19-01 Wave 0) so
//! the Phase 19 ExamMeta RED scaffolds can be appended without growing
//! `spec.rs` past the 500-line CLAUDE.md cap. Included via
//! `#[cfg(test)] #[path = "spec_tests.rs"] mod tests;` from `spec.rs` —
//! same convention as `commands::labs::eval`/`session`/
//! `storage_impl::reports`.

use super::*;

const VALID_LAB_MD: &str =
    include_str!("../../tests/fixtures/labs/specs/valid-pod-create.lab.md");
const MALFORMED_LAB_MD: &str =
    include_str!("../../tests/fixtures/labs/specs/malformed-frontmatter.lab.md");
const BOTH_IMAGE_DOCKERFILE: &str =
    include_str!("../../tests/fixtures/labs/specs/image-and-dockerfile-both.lab.md");
const TRAVERSAL_LAB_MD: &str =
    include_str!("../../tests/fixtures/labs/specs/creates-traversal.lab.md");
const EXAM_LAB_MD: &str =
    include_str!("../../tests/fixtures/labs/specs/exam-pod-create.lab.md");

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

/// LAB-04 — valid LAB.md parses to a 5-step LabSpec (D-10: adds a
/// command_absent step to the fixture in Phase 19.2).
#[test]
fn parse_valid_lab_md() {
    let (spec, body) = parse_lab_md(VALID_LAB_MD).expect("valid LAB.md must parse");
    assert_eq!(spec.slug, "pod-create-and-inspect");
    assert_eq!(spec.title, "Create and inspect a Pod");
    assert_eq!(spec.steps.len(), 5);
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

/// D-10 — the fixture's new step parses to StepCheck::CommandAbsent with
/// the authored pattern.
#[test]
fn parse_valid_lab_md_yields_command_absent_step() {
    let (spec, _body) = parse_lab_md(VALID_LAB_MD).expect("valid LAB.md must parse");
    let step = spec
        .steps
        .iter()
        .find(|s| s.id == "no-crash-loop")
        .expect("no-crash-loop step must exist");
    match &step.check {
        StepCheck::CommandAbsent { pattern, match_stderr } => {
            assert_eq!(pattern, "Error|CrashLoopBackOff");
            assert!(!match_stderr, "match_stderr must default to false");
        }
        other => panic!("expected StepCheck::CommandAbsent, got {:?}", other),
    }
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
        exam: None,
        grain: Grain::Step,
        steps: vec![LabStep {
            id: "s1".to_string(),
            title: "s1".to_string(),
            prompt: "do the thing".to_string(),
            check: StepCheck::AiJudge {
                criteria: "ok".to_string(),
                threshold: 0.7,
            },
            hints: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            weight: 1.0,
            grain: Grain::Step,
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
        (
            StepCheck::CommandAbsent {
                pattern: "x".to_string(),
                match_stderr: false,
            },
            "\"kind\":\"command_absent\"",
        ),
    ] {
        let json = serde_json::to_string(&check).unwrap();
        assert!(json.contains(needle), "missing {} in {}", needle, json);
    }
}

/// D-08 — command_absent round-trips with and without `match_stderr`:
/// omitted defaults to false; explicit true is preserved.
#[test]
fn command_absent_round_trips_match_stderr_default_and_explicit() {
    let json_no_match_stderr = r#"{"kind":"command_absent","pattern":"CrashLoopBackOff"}"#;
    let parsed: StepCheck = serde_json::from_str(json_no_match_stderr).unwrap();
    assert_eq!(
        parsed,
        StepCheck::CommandAbsent {
            pattern: "CrashLoopBackOff".to_string(),
            match_stderr: false,
        }
    );

    let json_match_stderr_true =
        r#"{"kind":"command_absent","pattern":"CrashLoopBackOff","match_stderr":true}"#;
    let parsed: StepCheck = serde_json::from_str(json_match_stderr_true).unwrap();
    assert_eq!(
        parsed,
        StepCheck::CommandAbsent {
            pattern: "CrashLoopBackOff".to_string(),
            match_stderr: true,
        }
    );
}

/// D-03 — validate_step_check rejects an empty/whitespace-only
/// command_absent pattern, mirroring command_regex's error shape.
#[test]
fn validate_step_check_rejects_empty_command_absent_pattern() {
    let check = StepCheck::CommandAbsent {
        pattern: "   ".to_string(),
        match_stderr: false,
    };
    match validate_step_check(&check) {
        Err(LabError::Spec(msg)) => {
            assert!(
                msg.contains("command_absent pattern must not be empty"),
                "got: {}",
                msg
            );
        }
        other => panic!("expected LabError::Spec, got {:?}", other),
    }
}

/// D-03 — a non-empty command_absent pattern passes validation.
#[test]
fn validate_step_check_accepts_non_empty_command_absent_pattern() {
    let check = StepCheck::CommandAbsent {
        pattern: "CrashLoopBackOff".to_string(),
        match_stderr: false,
    };
    assert!(validate_step_check(&check).is_ok());
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

// ── Phase 19 (EXAM-01..04) Wave 0 RED scaffolds — 19-02 implements ──
//
// `parse_lab_md` today has no knowledge of an `exam:` frontmatter block,
// no `ExamMeta` type, and `LabStep` has no `weight` field. Every test
// below fails to COMPILE or fails at ASSERT until 19-02 lands `ExamMeta`
// parsing + defaults + range validation + `LabStep.weight` (serde
// default 1.0, D-07). The compile failure itself is the correct RED
// signal for a type that doesn't exist yet — 19-02's first commit must
// introduce `ExamMeta` before these tests can even build, which is
// intentional: it forces the interface shape to be designed against
// these assertions rather than the other way around.

/// EXAM-01/EXAM-02 — exam frontmatter parses `ExamMeta { time_limit_minutes,
/// pass_threshold_pct }`; `LabStep.weight` defaults to 1.0 for steps that
/// don't declare one, and reads back the authored value when present
/// (D-07/D-08). RED until 19-02 adds `ExamMeta` + wires it onto `LabSpec`.
#[test]
fn exam_frontmatter_parses_exam_meta_and_step_weight() {
    let (spec, _body) = parse_lab_md(EXAM_LAB_MD).expect("19-02 must parse exam: frontmatter");

    let exam_meta = spec
        .exam
        .as_ref()
        .expect("19-02 must populate LabSpec.exam from the `exam:` frontmatter block");
    assert_eq!(
        exam_meta.time_limit_minutes,
        Some(45),
        "19-02: exam.timeLimitMinutes must round-trip from frontmatter"
    );
    assert_eq!(
        exam_meta.pass_threshold_pct,
        Some(80.0),
        "19-02: exam.passThresholdPct must round-trip from frontmatter"
    );

    let weighted_step = spec
        .steps
        .iter()
        .find(|s| s.id == "write-manifest")
        .expect("write-manifest step must exist");
    assert_eq!(
        weighted_step.weight, 2.0,
        "19-02: LabStep.weight must round-trip the authored `weight: 2` (D-07)"
    );

    let default_weight_step = spec
        .steps
        .iter()
        .find(|s| s.id == "explain-scheduling")
        .expect("explain-scheduling step must exist");
    assert_eq!(
        default_weight_step.weight, 1.0,
        "19-02: LabStep.weight must default to 1.0 when absent (D-07 serde default)"
    );
}

/// EXAM-01 — absent exam fields default to 30 minutes / 70.0% (D-03/D-08).
/// RED until 19-02 wires the defaults into `ExamMeta`'s deserialize path.
#[test]
fn exam_meta_defaults_when_fields_absent() {
    // A regular (non-exam) LAB.md has no `exam:` block at all — LabSpec.exam
    // must be None, never a default-populated ExamMeta (only authored exams
    // run as exams — D-02).
    let (spec, _body) = parse_lab_md(VALID_LAB_MD).expect("valid LAB.md must still parse");
    assert!(
        spec.exam.is_none(),
        "19-02: a LAB.md with no `exam:` block must have LabSpec.exam == None (D-02)"
    );
}

/// EXAM-01 (T-19-02) — `passThresholdPct` outside 0..=100 must fail
/// validation. RED until 19-02 adds the range check to `validate_spec`.
#[test]
fn exam_meta_rejects_pass_threshold_out_of_range() {
    let mut spec = exam_spec_fixture();
    spec.exam = Some(ExamMeta {
        time_limit_minutes: Some(45),
        pass_threshold_pct: Some(150.0),
    });
    let result = validate_spec(&spec);
    assert!(
        result.is_err(),
        "19-02: passThresholdPct=150 (outside 0..=100) must be rejected by validate_spec"
    );
}

/// EXAM-01 (T-19-02) — `timeLimitMinutes` outside 1..=480 must fail
/// validation. RED until 19-02 adds the range check to `validate_spec`.
#[test]
fn exam_meta_rejects_time_limit_out_of_range() {
    let mut spec = exam_spec_fixture();
    spec.exam = Some(ExamMeta {
        time_limit_minutes: Some(0),
        pass_threshold_pct: Some(80.0),
    });
    let result = validate_spec(&spec);
    assert!(
        result.is_err(),
        "19-02: timeLimitMinutes=0 (outside 1..=480) must be rejected by validate_spec"
    );
}

/// WR-02 (D-07/T-19-02) — non-positive step weights corrupt scoring
/// (mixed-sign weights can yield >100% and trivially-true `passed`);
/// `validate_spec` must reject them.
#[test]
fn validate_spec_rejects_non_positive_weight() {
    for bad_weight in [0.0, -1.0] {
        let mut spec = exam_spec_fixture();
        spec.steps[0].weight = bad_weight;
        assert!(
            validate_spec(&spec).is_err(),
            "weight {} must be rejected by validate_spec (WR-02)",
            bad_weight
        );
    }
}

/// WR-02 — a NaN/infinite weight makes total_weight NaN and the score
/// silently always-fail; `validate_spec` must reject non-finite weights.
#[test]
fn validate_spec_rejects_non_finite_weight() {
    for bad_weight in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut spec = exam_spec_fixture();
        spec.steps[0].weight = bad_weight;
        assert!(
            validate_spec(&spec).is_err(),
            "weight {} must be rejected by validate_spec (WR-02)",
            bad_weight
        );
    }
}

/// WR-02 — the same guard applies at the LAB.md parse boundary.
#[test]
fn parse_lab_md_rejects_negative_weight() {
    let md = r#"---
slug: weight-test
title: Weight test
image: alpine
steps:
  - id: s1
    title: S1
    prompt: Do the thing
    check:
      kind: exit_code
      expected: 0
    weight: -1
---
Body.
"#;
    assert_spec_err_msg(parse_lab_md(md), &["weight"]);
}

/// Minimal valid (non-exam) LabSpec fixture reused by the range-validation
/// RED tests above.
fn exam_spec_fixture() -> LabSpec {
    LabSpec {
        slug: "exam-fixture".to_string(),
        title: "Exam fixture".to_string(),
        image: Some("alpine".to_string()),
        dockerfile: None,
        requires_docker: false,
        creates: vec![],
        exam: None,
        grain: Grain::Step,
        steps: vec![LabStep {
            id: "s1".to_string(),
            title: "s1".to_string(),
            prompt: "do the thing".to_string(),
            check: StepCheck::ExitCode { expected: 0 },
            hints: vec![],
            weight: 1.0,
            grain: Grain::Step,
        }],
    }
}

// ── Phase 19.3 (D-01..D-08) — milestone validation grain ──
//
// `Grain` does not exist yet on `spec.rs`. These tests fail to COMPILE until
// Task 1 lands `enum Grain`, `default_grain`, `LabSpec.grain` / `LabStep.grain`
// (public + Raw), `effective_step_grain`, and
// `validate_milestone_exam_exclusion`. The compile failure is the correct RED
// signal (same convention as the Phase 19 EXAM-01 scaffolds above).

/// D-03 — a LAB.md with no `grain:` key anywhere parses to `Grain::Step` on
/// both the spec and every step (back-compat default).
#[test]
fn grain_absent_defaults_to_step() {
    let (spec, _body) = parse_lab_md(VALID_LAB_MD).expect("valid LAB.md must parse");
    assert_eq!(spec.grain, Grain::Step, "LabSpec.grain must default to Step");
    for step in &spec.steps {
        assert_eq!(
            step.grain,
            Grain::Step,
            "step {:?} grain must default to Step when absent",
            step.id
        );
    }
}

/// D-03 — `grain: milestone` parses on both lab-level and step-level
/// frontmatter (snake_case enum values).
#[test]
fn grain_milestone_parses() {
    let md = r#"---
slug: grain-test
title: Grain test
image: alpine
grain: milestone
steps:
  - id: s1
    title: S1
    prompt: Do the thing
    check:
      kind: exit_code
      expected: 0
  - id: s2
    title: S2
    prompt: Do another thing
    grain: step
    check:
      kind: exit_code
      expected: 0
---
Body.
"#;
    let (spec, _body) = parse_lab_md(md).expect("grain: milestone must parse");
    assert_eq!(spec.grain, Grain::Milestone, "LabSpec.grain must parse milestone");
    let s1 = spec.steps.iter().find(|s| s.id == "s1").unwrap();
    assert_eq!(
        s1.grain,
        Grain::Step,
        "step s1 has no explicit grain; LabStep.grain itself defaults to Step \
         (effective_step_grain resolves lab-level inheritance separately)"
    );
    let s2 = spec.steps.iter().find(|s| s.id == "s2").unwrap();
    assert_eq!(s2.grain, Grain::Step, "step s2 explicit grain: step must parse");
}

/// D-03 — `effective_step_grain` resolution: step-level Milestone always
/// wins; otherwise the lab-level grain applies (inheritance).
#[test]
fn effective_step_grain_resolution() {
    assert_eq!(
        effective_step_grain(Grain::Step, Grain::Milestone),
        Grain::Milestone,
        "a milestone step in a step-grain lab must validate as milestone"
    );
    assert_eq!(
        effective_step_grain(Grain::Milestone, Grain::Step),
        Grain::Milestone,
        "a step in a milestone-grain lab inherits milestone (explicit `grain: step` \
         collapses to the lab grain per D-03 accepted simplification)"
    );
    assert_eq!(
        effective_step_grain(Grain::Step, Grain::Step),
        Grain::Step
    );
    assert_eq!(
        effective_step_grain(Grain::Milestone, Grain::Milestone),
        Grain::Milestone
    );
}

/// D-05 — a spec with `exam:` metadata AND any milestone grain (lab-level)
/// is rejected by both `parse_lab_md` and `validate_spec` citing D-05.
#[test]
fn exam_milestone_coexistence_rejected_lab_level() {
    let md = r#"---
slug: exam-milestone-test
title: Exam milestone test
image: alpine
grain: milestone
exam:
  timeLimitMinutes: 30
  passThresholdPct: 70
steps:
  - id: s1
    title: S1
    prompt: Do the thing
    check:
      kind: exit_code
      expected: 0
---
Body.
"#;
    assert_spec_err_msg(parse_lab_md(md), &["d-05", "milestone", "exam"]);

    let mut spec = exam_spec_fixture();
    spec.exam = Some(ExamMeta {
        time_limit_minutes: Some(30),
        pass_threshold_pct: Some(70.0),
    });
    spec.grain = Grain::Milestone;
    match validate_spec(&spec) {
        Err(LabError::Spec(_)) => {}
        other => panic!("expected LabError::Spec for exam+milestone coexistence, got {:?}", other),
    }
}

/// D-05 — the exam×milestone rejection also fires when ONLY a step (not the
/// lab) declares milestone grain.
#[test]
fn exam_milestone_coexistence_rejected_step_level() {
    let mut spec = exam_spec_fixture();
    spec.exam = Some(ExamMeta {
        time_limit_minutes: Some(30),
        pass_threshold_pct: Some(70.0),
    });
    spec.steps[0].grain = Grain::Milestone;
    match validate_spec(&spec) {
        Err(LabError::Spec(_)) => {}
        other => panic!(
            "expected LabError::Spec for exam+step-milestone coexistence, got {:?}",
            other
        ),
    }
}

/// D-05 — a spec with `exam:` metadata and all step-grain (default) steps
/// parses fine — no false positive from the coexistence validator.
#[test]
fn exam_step_grain_ok() {
    let spec = exam_spec_fixture();
    let mut spec = spec;
    spec.exam = Some(ExamMeta {
        time_limit_minutes: Some(30),
        pass_threshold_pct: Some(70.0),
    });
    assert!(
        validate_spec(&spec).is_ok(),
        "exam metadata with all step-grain steps must not trigger the D-05 rejection"
    );
}
