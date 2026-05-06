//! # labs::evaluator — step evaluator (Phase 03.1, Wave 0 stub)
//!
//! Inline evaluator for the four StepCheck variants. Wave 1 (03.1-02 +
//! 03.1-04) wires the real `regex`, `std::fs`, and AIClientTrait paths.

use super::spec::StepCheck;
use super::LabError;
use serde::{Deserialize, Serialize};

/// Per-step evaluation context built by the IPC handler from the
/// prompt-boundary buffer.
#[derive(Debug, Clone)]
pub struct EvalContext<'a> {
    pub last_command: &'a str,
    pub last_output: &'a str,
    pub last_exit_code: Option<i32>,
    pub workspace: &'a std::path::Path,
    /// Whether the LLM judge has any auth context (Claude/OpenAI/Gemini OAuth
    /// or BYOK API key). When false, ai_judge falls back to Manual.
    pub ai_authenticated: bool,
    /// Remaining ai_judge calls in this lab session's budget. When 0,
    /// ai_judge falls back to Manual to keep LLM cost predictable.
    pub ai_budget_remaining: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalOutcome {
    Pass,
    Fail,
    /// Cannot tell yet — e.g. ExitCode check before OSC 133 D arrived.
    Indeterminate,
    /// Manual recheck required (ai_judge + no auth, ai_judge + budget=0,
    /// or LLM error).
    Manual,
}

/// Evaluate a single step. Wave 1 dispatches to the per-kind handler.
/// Wave 0 stub returns Err so every test fails.
pub async fn evaluate_step(
    _check: &StepCheck,
    _ctx: &EvalContext<'_>,
) -> Result<EvalOutcome, LabError> {
    Err(LabError::Eval(
        "evaluate_step: implemented in 03.1-02 / 03.1-04".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn ctx_with(last_output: &'static str, exit: Option<i32>) -> EvalContext<'static> {
        EvalContext {
            last_command: "kubectl apply -f pod.yaml",
            last_output,
            last_exit_code: exit,
            workspace: Path::new("/tmp/learnforge-eval-test"),
            ai_authenticated: true,
            ai_budget_remaining: 5,
        }
    }

    /// LAB-06 — command_regex matches stdout substring.
    #[tokio::test]
    async fn command_regex_match() {
        let check = StepCheck::CommandRegex {
            pattern: "pod/web (created|configured)".to_string(),
            match_stderr: false,
        };
        let ctx = ctx_with("pod/web created", Some(0));
        let outcome = evaluate_step(&check, &ctx)
            .await
            .expect("evaluate_step must succeed once 03.1-02 lands");
        assert_eq!(outcome, EvalOutcome::Pass);
    }

    /// LAB-06 — command_regex does NOT match stderr unless match_stderr=true.
    #[tokio::test]
    async fn command_regex_no_match_against_stderr_unless_flag() {
        let check = StepCheck::CommandRegex {
            pattern: "command not found".to_string(),
            match_stderr: false,
        };
        // last_output here is the stdout-only buffer; the impl will need
        // to track stderr separately. Wave 1 splits these.
        let ctx = ctx_with("", Some(127));
        let outcome = evaluate_step(&check, &ctx)
            .await
            .expect("evaluate_step must succeed once 03.1-02 lands");
        assert_eq!(
            outcome,
            EvalOutcome::Fail,
            "match_stderr=false must NOT match against stderr"
        );
    }

    /// LAB-06 — exit_code is Indeterminate when no OSC 133 D arrived.
    #[tokio::test]
    async fn exit_code_indeterminate_without_osc133() {
        let check = StepCheck::ExitCode { expected: 0 };
        let ctx = ctx_with("anything", None); // no exit code captured
        let outcome = evaluate_step(&check, &ctx)
            .await
            .expect("evaluate_step must succeed once 03.1-02 lands");
        assert_eq!(outcome, EvalOutcome::Indeterminate);
    }

    /// LAB-06 — exit_code Pass when OSC 133 D ;0 captured.
    #[tokio::test]
    async fn exit_code_pass() {
        let check = StepCheck::ExitCode { expected: 0 };
        let ctx = ctx_with("ok", Some(0));
        let outcome = evaluate_step(&check, &ctx)
            .await
            .expect("evaluate_step must succeed once 03.1-02 lands");
        assert_eq!(outcome, EvalOutcome::Pass);
    }

    /// LAB-06 — file_state contains the expected substring.
    #[tokio::test]
    async fn file_state_contains() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("pod.yaml");
        std::fs::write(&path, "kind: Pod\nmetadata:\n  name: web\n").expect("write");
        let check = StepCheck::FileState {
            path: "pod.yaml".to_string(),
            contains: Some("kind: Pod".to_string()),
        };
        let ctx = EvalContext {
            last_command: "",
            last_output: "",
            last_exit_code: None,
            workspace: dir.path(),
            ai_authenticated: true,
            ai_budget_remaining: 5,
        };
        let outcome = evaluate_step(&check, &ctx)
            .await
            .expect("evaluate_step must succeed once 03.1-02 lands");
        assert_eq!(outcome, EvalOutcome::Pass);
    }

    /// LAB-06 — file_state Fail when the file is absent.
    #[tokio::test]
    async fn file_state_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let check = StepCheck::FileState {
            path: "missing.yaml".to_string(),
            contains: Some("anything".to_string()),
        };
        let ctx = EvalContext {
            last_command: "",
            last_output: "",
            last_exit_code: None,
            workspace: dir.path(),
            ai_authenticated: true,
            ai_budget_remaining: 5,
        };
        let outcome = evaluate_step(&check, &ctx)
            .await
            .expect("evaluate_step must succeed once 03.1-02 lands");
        assert_eq!(outcome, EvalOutcome::Fail);
    }

    /// LAB-06 — ai_judge falls back to Manual when budget is exhausted.
    #[tokio::test]
    async fn ai_judge_budget_exhausted() {
        let check = StepCheck::AiJudge {
            criteria: "explain what kubectl get pods reveals about scheduling".to_string(),
            threshold: 0.7,
        };
        let ctx = EvalContext {
            last_command: "kubectl get pods",
            last_output: "NAME READY STATUS\nweb 1/1 Running",
            last_exit_code: Some(0),
            workspace: Path::new("/tmp/learnforge-budget-test"),
            ai_authenticated: true,
            ai_budget_remaining: 0,
        };
        let outcome = evaluate_step(&check, &ctx)
            .await
            .expect("evaluate_step must succeed once 03.1-02 lands");
        assert_eq!(
            outcome,
            EvalOutcome::Manual,
            "budget=0 must short-circuit to Manual without LLM call"
        );
    }

    /// LAB-06 — ai_judge falls back to Manual when there is no auth.
    #[tokio::test]
    async fn ai_judge_no_auth_falls_back() {
        let check = StepCheck::AiJudge {
            criteria: "explain what kubectl get pods reveals about scheduling".to_string(),
            threshold: 0.7,
        };
        let ctx = EvalContext {
            last_command: "kubectl get pods",
            last_output: "NAME READY STATUS",
            last_exit_code: Some(0),
            workspace: Path::new("/tmp/learnforge-noauth-test"),
            ai_authenticated: false,
            ai_budget_remaining: 5,
        };
        let outcome = evaluate_step(&check, &ctx)
            .await
            .expect("evaluate_step must succeed once 03.1-02 lands");
        assert_eq!(
            outcome,
            EvalOutcome::Manual,
            "no auth must short-circuit to Manual without LLM call"
        );
    }
}
