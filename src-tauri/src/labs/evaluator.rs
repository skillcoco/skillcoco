//! # labs::evaluator — step evaluator (Phase 03.1, Wave 2)
//!
//! Inline evaluator for the five StepCheck variants. Determinism is the
//! load-bearing contract here:
//!
//! - `command_regex` uses `regex::Regex` against `last_output` (or the
//!   stderr buffer when `match_stderr=true`).
//! - `command_absent` (Phase 19.2, D-04) is the exact inverse of
//!   `command_regex` — Pass when the pattern does NOT match, Fail when it
//!   does. Same `Regex::new` compile path, same `match_stderr` no-op shape.
//! - `exit_code` is `Indeterminate` when no OSC 133 D sequence has arrived
//!   (`ctx.last_exit_code == None`).
//! - `file_state` reads the file under `workspace.join(path)` with `tempfile`
//!   roots in tests.
//! - `ai_judge` is the last-resort grader. It short-circuits to `Manual`
//!   when budget is exhausted OR no auth is available — both are graceful
//!   degrades, NOT errors. When invoked, it goes through `AiJudgeRunner`
//!   so tests inject a closure-based mock instead of touching a real LLM.

use super::spec::StepCheck;
use super::LabError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Per-step evaluation context built by the IPC handler from the
/// prompt-boundary buffer.
#[derive(Debug, Clone)]
pub struct EvalContext<'a> {
    pub last_command: &'a str,
    /// Stdout (and combined output when stderr isn't separated).
    pub last_output: &'a str,
    pub last_exit_code: Option<i32>,
    pub workspace: &'a std::path::Path,
    /// Whether the LLM judge has any auth context. When false, ai_judge
    /// falls back to Manual.
    pub ai_authenticated: bool,
    /// Remaining ai_judge calls in this lab session's budget. When 0,
    /// ai_judge falls back to Manual to keep LLM cost predictable.
    pub ai_budget_remaining: u32,
    /// Phase 19.3 (D-06) — per-session command history view for
    /// milestone-grain evaluation (`evaluate_step_milestone`). `None` for
    /// every step-grain call (back-compat: existing eval fns never read
    /// this field, so their behavior is byte-identical regardless of value).
    pub history: Option<&'a [crate::CommandRecord]>,
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

/// Test seam for ai_judge — production wires this to
/// `crate::ai::retry::ai_request_with_retry`; tests inject a closure that
/// returns a canned `{"pass": bool, "reason": string}` string.
pub trait AiJudgeRunner: Send + Sync {
    fn run<'a>(
        &'a self,
        prompt: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>>;
}

/// Evaluate a single step. Picks the deterministic dispatch path for
/// command_regex / exit_code / file_state. ai_judge with budget>0 + auth
/// is documented but routed via [`evaluate_step_with_judge`] for testability.
pub async fn evaluate_step(
    check: &StepCheck,
    ctx: &EvalContext<'_>,
) -> Result<EvalOutcome, LabError> {
    evaluate_step_with_judge(check, ctx, None::<&NoJudgeRunner>).await
}

/// Sentinel runner that always errs — used when no runner is supplied and the
/// check is an ai_judge with budget+auth. Production code should always pass
/// a real runner; in tests for the budget/no-auth paths the runner is never
/// invoked so this is unreachable.
struct NoJudgeRunner;

impl AiJudgeRunner for NoJudgeRunner {
    fn run<'a>(
        &'a self,
        _prompt: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            Err("ai_judge runner not configured (production callers must pass one)".to_string())
        })
    }
}

/// Variant that accepts an injected runner — call this from production code
/// that has built a real `AiJudgeRunner` over `ai_request_with_retry`, and
/// from tests that want to drive the LLM-success / LLM-failure branches.
pub async fn evaluate_step_with_judge<R>(
    check: &StepCheck,
    ctx: &EvalContext<'_>,
    runner: Option<&R>,
) -> Result<EvalOutcome, LabError>
where
    R: AiJudgeRunner + ?Sized,
{
    match check {
        StepCheck::CommandRegex {
            pattern,
            match_stderr,
        } => eval_command_regex(pattern, *match_stderr, ctx),
        StepCheck::ExitCode { expected } => Ok(eval_exit_code(*expected, ctx)),
        StepCheck::FileState { path, contains } => eval_file_state(path, contains.as_deref(), ctx),
        StepCheck::AiJudge { criteria, threshold } => {
            eval_ai_judge(criteria, *threshold, ctx, runner).await
        }
        StepCheck::CommandAbsent {
            pattern,
            match_stderr,
        } => eval_command_absent(pattern, *match_stderr, ctx),
    }
}

/// Phase 19.3 (D-02/D-06) — milestone-grain dispatch, evaluated against the
/// cumulative session history (`ctx.history`, `None` treated as empty)
/// plus the workspace tree, instead of only `last_output`/`last_exit_code`:
///
/// - `command_regex`: Pass iff the regex matches ANY record's output
///   (match_stderr keeps its step-grain no-op parity). Empty history → Fail.
/// - `command_absent`: Pass iff the regex matches NO record's output;
///   EMPTY history → Fail (anti-vacuous rule — see `milestone_reason` for
///   the "no commands recorded" reason surface).
/// - `exit_code`: Pass iff ANY record's exit_code equals expected;
///   empty/no-match → Fail (never Indeterminate at milestone grain —
///   validation is explicit, so "nothing reached" is a definitive Fail).
/// - `file_state`: delegates to `eval_file_state` unchanged (end-state by
///   construction — reads the workspace, history-independent).
/// - `ai_judge`: judges over the concatenated history tail
///   (`scrollback_tail(_, 100)`), reusing `eval_ai_judge`'s runner path.
pub async fn evaluate_step_milestone<R>(
    check: &StepCheck,
    ctx: &EvalContext<'_>,
    runner: Option<&R>,
) -> Result<EvalOutcome, LabError>
where
    R: AiJudgeRunner + ?Sized,
{
    let history: &[crate::CommandRecord] = ctx.history.unwrap_or(&[]);
    match check {
        StepCheck::CommandRegex { pattern, match_stderr: _ } => {
            // Reuse the exact Regex::new error shape from eval_command_regex.
            let re = Regex::new(pattern)
                .map_err(|e| LabError::Eval(format!("invalid regex {:?}: {}", pattern, e)))?;
            if history.iter().any(|r| re.is_match(&r.output)) {
                Ok(EvalOutcome::Pass)
            } else {
                Ok(EvalOutcome::Fail)
            }
        }
        StepCheck::CommandAbsent { pattern, match_stderr: _ } => {
            let re = Regex::new(pattern)
                .map_err(|e| LabError::Eval(format!("invalid regex {:?}: {}", pattern, e)))?;
            if history.is_empty() {
                // D-02 anti-vacuous rule: zero records is Fail, never a
                // vacuous Pass (deliberate difference from step grain).
                return Ok(EvalOutcome::Fail);
            }
            if history.iter().any(|r| re.is_match(&r.output)) {
                Ok(EvalOutcome::Fail)
            } else {
                Ok(EvalOutcome::Pass)
            }
        }
        StepCheck::ExitCode { expected } => {
            if history.iter().any(|r| r.exit_code == Some(*expected)) {
                Ok(EvalOutcome::Pass)
            } else {
                Ok(EvalOutcome::Fail)
            }
        }
        StepCheck::FileState { path, contains } => {
            eval_file_state(path, contains.as_deref(), ctx)
        }
        StepCheck::AiJudge { criteria, threshold } => {
            // Concatenate record outputs into a scrollback-shaped buffer,
            // cap via scrollback_tail(_, 100) inside build_ai_judge_prompt's
            // path, and reuse eval_ai_judge's auth/budget/runner behavior by
            // rebuilding a ctx whose last_output IS the history tail.
            let concatenated: String = history
                .iter()
                .map(|r| r.output.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let tail = scrollback_tail(&concatenated, 100);
            let judge_ctx = EvalContext {
                last_command: ctx.last_command,
                last_output: &tail,
                last_exit_code: ctx.last_exit_code,
                workspace: ctx.workspace,
                ai_authenticated: ctx.ai_authenticated,
                ai_budget_remaining: ctx.ai_budget_remaining,
                history: None,
            };
            eval_ai_judge(criteria, *threshold, &judge_ctx, runner).await
        }
    }
}

/// Phase 19.3 (D-02) — milestone-specific reason surface. Returns
/// `Some("no commands recorded ...")` for the anti-vacuous empty-history
/// `command_absent` Fail; `None` for every other combination (callers fall
/// back to the generic outcome reason). Reused by `lab_validate_milestone`.
pub fn milestone_reason(
    check: &StepCheck,
    ctx: &EvalContext<'_>,
    outcome: &EvalOutcome,
) -> Option<String> {
    let history_empty = ctx.history.map_or(true, |h| h.is_empty());
    match (check, outcome) {
        (StepCheck::CommandAbsent { .. }, EvalOutcome::Fail) if history_empty => Some(
            "no commands recorded — a milestone command_absent check cannot pass over an \
             empty session history (D-02 anti-vacuous rule)"
                .to_string(),
        ),
        _ => None,
    }
}

fn eval_command_regex(
    pattern: &str,
    _match_stderr: bool,
    ctx: &EvalContext<'_>,
) -> Result<EvalOutcome, LabError> {
    // Note: stdout-only buffer in v1. When the IPC handler separates stderr
    // (RESEARCH § Open Question), `match_stderr=true` will swap to the
    // stderr buffer. In v1, `match_stderr=true` is a stricter assertion: it
    // requires the pattern to live in the trailing error-style chunk. The
    // detector merges stdout+stderr into `last_output` today so we test the
    // stderr-flag path with an explicit "no match" fixture (existing Wave 0
    // test asserts Fail when stderr is empty AND match_stderr=true).
    let re = Regex::new(pattern)
        .map_err(|e| LabError::Eval(format!("invalid regex {:?}: {}", pattern, e)))?;
    if re.is_match(ctx.last_output) {
        Ok(EvalOutcome::Pass)
    } else {
        Ok(EvalOutcome::Fail)
    }
}

/// Phase 19.2 (D-04/D-05/D-06) — exact inverse of `eval_command_regex`: Pass
/// when the pattern does NOT match, Fail when it does. Reuses the identical
/// `Regex::new` compile path/error shape (D-05); `_match_stderr` stays a
/// no-op with the same v1 stdout-only buffer caveat as `eval_command_regex`
/// (D-06) — real stderr-buffer switching is out of scope for this phase.
fn eval_command_absent(
    pattern: &str,
    _match_stderr: bool,
    ctx: &EvalContext<'_>,
) -> Result<EvalOutcome, LabError> {
    let re = Regex::new(pattern)
        .map_err(|e| LabError::Eval(format!("invalid regex {:?}: {}", pattern, e)))?;
    if re.is_match(ctx.last_output) {
        Ok(EvalOutcome::Fail)
    } else {
        Ok(EvalOutcome::Pass)
    }
}

fn eval_exit_code(expected: i32, ctx: &EvalContext<'_>) -> EvalOutcome {
    match ctx.last_exit_code {
        None => EvalOutcome::Indeterminate,
        Some(code) if code == expected => EvalOutcome::Pass,
        Some(_) => EvalOutcome::Fail,
    }
}

fn eval_file_state(
    path: &str,
    contains: Option<&str>,
    ctx: &EvalContext<'_>,
) -> Result<EvalOutcome, LabError> {
    let resolved = ctx.workspace.join(path);
    if !resolved.exists() {
        return Ok(EvalOutcome::Fail);
    }
    if let Some(needle) = contains {
        let body = std::fs::read_to_string(&resolved)?;
        if body.contains(needle) {
            Ok(EvalOutcome::Pass)
        } else {
            Ok(EvalOutcome::Fail)
        }
    } else {
        // Existence-only check.
        Ok(EvalOutcome::Pass)
    }
}

async fn eval_ai_judge<R>(
    criteria: &str,
    _threshold: f64,
    ctx: &EvalContext<'_>,
    runner: Option<&R>,
) -> Result<EvalOutcome, LabError>
where
    R: AiJudgeRunner + ?Sized,
{
    if !ctx.ai_authenticated {
        return Ok(EvalOutcome::Manual);
    }
    if ctx.ai_budget_remaining == 0 {
        return Ok(EvalOutcome::Manual);
    }
    let Some(runner) = runner else {
        // No runner supplied but auth + budget would have invoked one.
        // Production callers must pass one; in this defensive branch we
        // surface Manual rather than fail the whole step.
        return Ok(EvalOutcome::Manual);
    };

    let prompt = build_ai_judge_prompt(criteria, ctx.last_command, ctx.last_output);
    match runner.run(&prompt).await {
        Ok(content) => Ok(parse_judge_verdict(&content)),
        // LLM call failed — degrade gracefully to Manual rather than
        // failing the whole step (RESEARCH § ai_judge degradation policy).
        Err(_) => Ok(EvalOutcome::Manual),
    }
}

/// Build the ai_judge prompt. Embeds the criteria, the last-command line,
/// and a tail of the scrollback (max 100 lines) so the LLM has the
/// terminal context to grade against.
pub fn build_ai_judge_prompt(criteria: &str, last_command: &str, scrollback: &str) -> String {
    let tail = scrollback_tail(scrollback, 100);
    format!(
        "You are grading a hands-on lab step. Reply ONLY with JSON: \
{{\"pass\": <bool>, \"reason\": <string>}}.\n\n\
Criteria:\n{}\n\n\
Last command:\n{}\n\n\
Terminal output (last lines):\n{}\n",
        criteria, last_command, tail
    )
}

fn scrollback_tail(scrollback: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = scrollback.lines().collect();
    if lines.len() <= max_lines {
        return scrollback.to_string();
    }
    lines[lines.len() - max_lines..].join("\n")
}

/// Parse the LLM's `{"pass": bool, "reason": string}` JSON. Any parse error
/// is treated as Manual rather than Fail — the LLM is fallible and the
/// learner shouldn't be penalised for our parser.
pub fn parse_judge_verdict(content: &str) -> EvalOutcome {
    let trimmed = content.trim();
    let json_str = extract_json_object(trimmed).unwrap_or(trimmed.to_string());
    match serde_json::from_str::<serde_json::Value>(&json_str) {
        Ok(v) => match v.get("pass").and_then(|x| x.as_bool()) {
            Some(true) => EvalOutcome::Pass,
            Some(false) => EvalOutcome::Fail,
            None => EvalOutcome::Manual,
        },
        Err(_) => EvalOutcome::Manual,
    }
}

/// LLMs sometimes wrap JSON in code fences. Pull the first `{...}` block out.
fn extract_json_object(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end > start {
        Some(s[start..=end].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    const CRITERIA: &str = "explain what kubectl get pods reveals about scheduling";

    fn ctx_with(last_output: &'static str, exit: Option<i32>) -> EvalContext<'static> {
        EvalContext {
            last_command: "kubectl apply -f pod.yaml",
            last_output,
            last_exit_code: exit,
            workspace: Path::new("/tmp/learnforge-eval-test"),
            ai_authenticated: true,
            ai_budget_remaining: 5,
            history: None,
        }
    }

    fn ctx_in_dir<'a>(
        dir: &'a std::path::Path,
        auth: bool,
        budget: u32,
    ) -> EvalContext<'a> {
        EvalContext {
            last_command: "",
            last_output: "",
            last_exit_code: None,
            workspace: dir,
            ai_authenticated: auth,
            ai_budget_remaining: budget,
            history: None,
        }
    }

    /// Closure-driven mock for AiJudgeRunner.
    struct MockRunner {
        response: Result<String, String>,
        called: Arc<Mutex<u32>>,
    }
    impl MockRunner {
        fn ok(p: &str) -> Self {
            Self { response: Ok(p.to_string()), called: Arc::new(Mutex::new(0)) }
        }
        fn err(p: &str) -> Self {
            Self { response: Err(p.to_string()), called: Arc::new(Mutex::new(0)) }
        }
        fn call_count(&self) -> u32 { *self.called.lock().unwrap() }
    }
    impl AiJudgeRunner for MockRunner {
        fn run<'a>(
            &'a self,
            _prompt: &'a str,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>> {
            *self.called.lock().unwrap() += 1;
            let r = self.response.clone();
            Box::pin(async move { r })
        }
    }

    fn ai_judge() -> StepCheck {
        StepCheck::AiJudge { criteria: CRITERIA.to_string(), threshold: 0.7 }
    }

    /// LAB-06 — command_regex matches stdout substring.
    #[tokio::test]
    async fn command_regex_match() {
        let check = StepCheck::CommandRegex {
            pattern: "pod/web (created|configured)".to_string(),
            match_stderr: false,
        };
        let ctx = ctx_with("pod/web created", Some(0));
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Pass);
    }

    /// LAB-06 — command_regex Fail when stdout doesn't match.
    #[tokio::test]
    async fn command_regex_no_match() {
        let check = StepCheck::CommandRegex {
            pattern: "pod/web (created|configured)".to_string(),
            match_stderr: false,
        };
        let ctx = ctx_with("pod/api created", Some(0));
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Fail);
    }

    /// LAB-06 — command_regex does NOT match stderr unless match_stderr=true.
    #[tokio::test]
    async fn command_regex_no_match_against_stderr_unless_flag() {
        let check = StepCheck::CommandRegex {
            pattern: "command not found".to_string(),
            match_stderr: false,
        };
        let ctx = ctx_with("", Some(127));
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Fail);
    }

    /// D-04/D-09 — command_absent Pass when the pattern does NOT match
    /// last_output (exact inverse of command_regex_match).
    #[tokio::test]
    async fn command_absent_pass_when_no_match() {
        let check = StepCheck::CommandAbsent {
            pattern: "CrashLoopBackOff".to_string(),
            match_stderr: false,
        };
        let ctx = ctx_with("pod/web created", Some(0));
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Pass);
    }

    /// D-04/D-09 — command_absent Fail when the pattern DOES match
    /// last_output (exact inverse of command_regex_no_match).
    #[tokio::test]
    async fn command_absent_fail_when_match() {
        let check = StepCheck::CommandAbsent {
            pattern: "CrashLoopBackOff".to_string(),
            match_stderr: false,
        };
        let ctx = ctx_with("pod/web CrashLoopBackOff", Some(0));
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Fail);
    }

    /// D-06/D-09 — command_absent honors match_stderr=false the same
    /// no-op way command_regex does (v1 stdout-only buffer): Pass when
    /// the (merged) last_output buffer has no match.
    #[tokio::test]
    async fn command_absent_match_stderr_false_pass_when_no_match() {
        let check = StepCheck::CommandAbsent {
            pattern: "command not found".to_string(),
            match_stderr: false,
        };
        let ctx = ctx_with("", Some(127));
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Pass);
    }

    /// D-06/D-09 — command_absent honors match_stderr=true identically
    /// (no real stderr-buffer switching in v1); Fail when the pattern
    /// matches the merged last_output buffer regardless of the flag.
    #[tokio::test]
    async fn command_absent_match_stderr_true_fail_when_match() {
        let check = StepCheck::CommandAbsent {
            pattern: "CrashLoopBackOff".to_string(),
            match_stderr: true,
        };
        let ctx = ctx_with("pod/web CrashLoopBackOff", Some(0));
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Fail);
    }

    /// D-05 — invalid regex returns the SAME LabError::Eval shape as
    /// eval_command_regex.
    #[tokio::test]
    async fn command_absent_invalid_regex_returns_eval_error() {
        let check = StepCheck::CommandAbsent {
            pattern: "(unclosed".to_string(),
            match_stderr: false,
        };
        let ctx = ctx_with("anything", Some(0));
        let err = evaluate_step(&check, &ctx).await.unwrap_err();
        match err {
            LabError::Eval(msg) => assert!(
                msg.contains("invalid regex"),
                "expected 'invalid regex' in message, got: {}",
                msg
            ),
            other => panic!("expected LabError::Eval, got {:?}", other),
        }
    }

    /// LAB-06 — exit_code Pass when OSC 133 D ;0 captured.
    #[tokio::test]
    async fn exit_code_pass() {
        let check = StepCheck::ExitCode { expected: 0 };
        let ctx = ctx_with("ok", Some(0));
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Pass);
    }

    /// LAB-06 — exit_code Indeterminate when no OSC 133 D arrived.
    #[tokio::test]
    async fn exit_code_indeterminate_without_osc133() {
        let check = StepCheck::ExitCode { expected: 0 };
        let ctx = ctx_with("anything", None);
        assert_eq!(
            evaluate_step(&check, &ctx).await.unwrap(),
            EvalOutcome::Indeterminate
        );
    }

    /// LAB-06 — exit_code Fail when code doesn't match expected.
    #[tokio::test]
    async fn exit_code_fail_when_mismatched() {
        let check = StepCheck::ExitCode { expected: 0 };
        let ctx = ctx_with("oops", Some(127));
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Fail);
    }

    /// LAB-06 — file_state contains the expected substring.
    #[tokio::test]
    async fn file_state_contains() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pod.yaml"),
            "kind: Pod\nmetadata:\n  name: web\n",
        )
        .unwrap();
        let check = StepCheck::FileState {
            path: "pod.yaml".to_string(),
            contains: Some("kind: Pod".to_string()),
        };
        let ctx = ctx_in_dir(dir.path(), true, 5);
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Pass);
    }

    /// LAB-06 — file_state Fail when the file is absent.
    #[tokio::test]
    async fn file_state_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let check = StepCheck::FileState {
            path: "missing.yaml".to_string(),
            contains: Some("anything".to_string()),
        };
        let ctx = ctx_in_dir(dir.path(), true, 5);
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Fail);
    }

    /// LAB-06 — ai_judge -> Manual when budget=0; LLM not invoked.
    #[tokio::test]
    async fn ai_judge_budget_exhausted() {
        let runner = MockRunner::ok("{\"pass\":true,\"reason\":\"ok\"}");
        let mut ctx = ctx_with("ok", Some(0));
        ctx.ai_budget_remaining = 0;
        let outcome = evaluate_step_with_judge(&ai_judge(), &ctx, Some(&runner))
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Manual);
        assert_eq!(runner.call_count(), 0);
    }

    /// LAB-06 — ai_judge -> Manual when no auth; LLM not invoked.
    #[tokio::test]
    async fn ai_judge_no_auth_falls_back() {
        let runner = MockRunner::ok("{\"pass\":true,\"reason\":\"ok\"}");
        let mut ctx = ctx_with("ok", Some(0));
        ctx.ai_authenticated = false;
        let outcome = evaluate_step_with_judge(&ai_judge(), &ctx, Some(&runner))
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Manual);
        assert_eq!(runner.call_count(), 0);
    }

    /// LAB-06 — ai_judge Pass when mocked LLM returns {pass: true}.
    #[tokio::test]
    async fn ai_judge_pass_when_llm_returns_pass_true() {
        let runner = MockRunner::ok("{\"pass\": true, \"reason\": \"ok\"}");
        let ctx = ctx_with("NAME READY STATUS\nweb 1/1 Running", Some(0));
        let outcome = evaluate_step_with_judge(&ai_judge(), &ctx, Some(&runner))
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Pass);
        assert_eq!(runner.call_count(), 1);
    }

    /// LAB-06 — ai_judge Fail when mocked LLM returns {pass: false}.
    #[tokio::test]
    async fn ai_judge_fail_when_llm_returns_pass_false() {
        let runner = MockRunner::ok("{\"pass\": false, \"reason\": \"miss\"}");
        let ctx = ctx_with("(empty)", Some(0));
        assert_eq!(
            evaluate_step_with_judge(&ai_judge(), &ctx, Some(&runner))
                .await
                .unwrap(),
            EvalOutcome::Fail
        );
    }

    /// LAB-06 — ai_judge degrades to Manual on LLM transport error.
    #[tokio::test]
    async fn ai_judge_manual_on_llm_error() {
        let runner = MockRunner::err("network unreachable");
        let ctx = ctx_with("anything", Some(0));
        assert_eq!(
            evaluate_step_with_judge(&ai_judge(), &ctx, Some(&runner))
                .await
                .unwrap(),
            EvalOutcome::Manual
        );
    }

    /// LAB-06 — parse_judge_verdict tolerates fenced JSON / preamble.
    #[test]
    fn parse_judge_verdict_handles_fenced_json() {
        let s = "```json\n{\"pass\": true, \"reason\": \"ok\"}\n```";
        assert_eq!(parse_judge_verdict(s), EvalOutcome::Pass);
        let s = "Sure: {\"pass\": false, \"reason\": \"x\"}";
        assert_eq!(parse_judge_verdict(s), EvalOutcome::Fail);
        assert_eq!(parse_judge_verdict("not json"), EvalOutcome::Manual);
    }

    /// LAB-06 — build_ai_judge_prompt embeds criteria + command + scrollback.
    #[test]
    fn build_ai_judge_prompt_embeds_inputs() {
        let p = build_ai_judge_prompt(
            "criteria-X",
            "kubectl get pods",
            "NAME READY STATUS\nweb 1/1 Running",
        );
        assert!(p.contains("criteria-X"));
        assert!(p.contains("kubectl get pods"));
        assert!(p.contains("web 1/1 Running"));
        assert!(p.contains("\"pass\""), "must specify the JSON shape");
    }

    // ── Phase 19.3 (D-02/D-06) — evaluate_step_milestone ──
    //
    // `evaluate_step_milestone` does not exist yet. These tests fail to
    // COMPILE until Task 2 lands it — the correct RED signal.

    use crate::CommandRecord;

    fn rec(output: &str, exit_code: Option<i32>) -> CommandRecord {
        CommandRecord {
            command: "cmd".to_string(),
            output: output.to_string(),
            exit_code,
        }
    }

    fn milestone_ctx<'a>(history: &'a [CommandRecord], workspace: &'a std::path::Path) -> EvalContext<'a> {
        EvalContext {
            last_command: "",
            last_output: "",
            last_exit_code: None,
            workspace,
            ai_authenticated: true,
            ai_budget_remaining: 5,
            history: Some(history),
        }
    }

    /// D-02 — command_regex Passes iff the regex matches ANY record's output.
    #[tokio::test]
    async fn milestone_command_regex_matches_any_record() {
        let history = vec![
            rec("pod/api created", None),
            rec("pod/web created", Some(0)),
        ];
        let dir = tempfile::tempdir().unwrap();
        let ctx = milestone_ctx(&history, dir.path());
        let check = StepCheck::CommandRegex {
            pattern: "pod/web (created|configured)".to_string(),
            match_stderr: false,
        };
        let outcome = evaluate_step_milestone(&check, &ctx, None::<&NoJudgeRunner>)
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Pass);
    }

    /// D-02 — command_regex over EMPTY history Fails (nothing reached; no
    /// record output can match). Explicitly named per plan-checker warning.
    #[tokio::test]
    async fn milestone_command_regex_empty_history_fails() {
        let history: Vec<CommandRecord> = vec![];
        let dir = tempfile::tempdir().unwrap();
        let ctx = milestone_ctx(&history, dir.path());
        let check = StepCheck::CommandRegex {
            pattern: "pod/web (created|configured)".to_string(),
            match_stderr: false,
        };
        let outcome = evaluate_step_milestone(&check, &ctx, None::<&NoJudgeRunner>)
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Fail, "empty history must Fail for command_regex");
    }

    /// D-02 anti-vacuous rule — command_absent over EMPTY history Fails
    /// with reason "no commands recorded" (explicit difference from
    /// step-grain's locked inverse semantics).
    #[tokio::test]
    async fn milestone_command_absent_empty_history_fails() {
        let history: Vec<CommandRecord> = vec![];
        let dir = tempfile::tempdir().unwrap();
        let ctx = milestone_ctx(&history, dir.path());
        let check = StepCheck::CommandAbsent {
            pattern: "CrashLoopBackOff".to_string(),
            match_stderr: false,
        };
        let outcome = evaluate_step_milestone(&check, &ctx, None::<&NoJudgeRunner>)
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Fail, "empty history must Fail (anti-vacuous)");
        let reason = milestone_reason(&check, &ctx, &outcome)
            .expect("empty-history command_absent Fail must carry an explicit reason");
        assert!(
            reason.contains("no commands recorded"),
            "reason must contain 'no commands recorded', got: {}",
            reason
        );
    }

    /// D-02 — command_absent Passes over non-empty history iff NO record
    /// matches the pattern.
    #[tokio::test]
    async fn milestone_command_absent_passes_when_no_record_matches() {
        let history = vec![rec("pod/web created", Some(0)), rec("pod/web Running", Some(0))];
        let dir = tempfile::tempdir().unwrap();
        let ctx = milestone_ctx(&history, dir.path());
        let check = StepCheck::CommandAbsent {
            pattern: "CrashLoopBackOff".to_string(),
            match_stderr: false,
        };
        let outcome = evaluate_step_milestone(&check, &ctx, None::<&NoJudgeRunner>)
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Pass);
    }

    /// D-02 — command_absent Fails when ANY record's output matches.
    #[tokio::test]
    async fn milestone_command_absent_fails_when_any_record_matches() {
        let history = vec![
            rec("pod/web created", Some(0)),
            rec("pod/web CrashLoopBackOff", Some(1)),
        ];
        let dir = tempfile::tempdir().unwrap();
        let ctx = milestone_ctx(&history, dir.path());
        let check = StepCheck::CommandAbsent {
            pattern: "CrashLoopBackOff".to_string(),
            match_stderr: false,
        };
        let outcome = evaluate_step_milestone(&check, &ctx, None::<&NoJudgeRunner>)
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Fail);
    }

    /// D-02 — exit_code Passes iff ANY record's exit_code equals expected.
    #[tokio::test]
    async fn milestone_exit_code_matches_any_record() {
        let history = vec![rec("oops", Some(1)), rec("ok", Some(0))];
        let dir = tempfile::tempdir().unwrap();
        let ctx = milestone_ctx(&history, dir.path());
        let check = StepCheck::ExitCode { expected: 0 };
        let outcome = evaluate_step_milestone(&check, &ctx, None::<&NoJudgeRunner>)
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Pass);
    }

    /// D-02 — exit_code Fails over empty history (nothing reached).
    #[tokio::test]
    async fn milestone_exit_code_empty_history_fails() {
        let history: Vec<CommandRecord> = vec![];
        let dir = tempfile::tempdir().unwrap();
        let ctx = milestone_ctx(&history, dir.path());
        let check = StepCheck::ExitCode { expected: 0 };
        let outcome = evaluate_step_milestone(&check, &ctx, None::<&NoJudgeRunner>)
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Fail);
    }

    /// D-02 — file_state is byte-identical to step grain (history-independent,
    /// reads workspace).
    #[tokio::test]
    async fn milestone_file_state_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pod.yaml"),
            "kind: Pod\nmetadata:\n  name: web\n",
        )
        .unwrap();
        let history = vec![rec("irrelevant", Some(0))];
        let ctx = milestone_ctx(&history, dir.path());
        let check = StepCheck::FileState {
            path: "pod.yaml".to_string(),
            contains: Some("kind: Pod".to_string()),
        };
        let outcome = evaluate_step_milestone(&check, &ctx, None::<&NoJudgeRunner>)
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Pass);
    }

    /// D-02/D-06 — ai_judge judges over the concatenated history tail via
    /// scrollback_tail(_, 100), reusing eval_ai_judge's runner path.
    #[tokio::test]
    async fn milestone_ai_judge_uses_concatenated_history() {
        let runner = MockRunner::ok("{\"pass\": true, \"reason\": \"ok\"}");
        let history = vec![rec("NAME READY STATUS", Some(0)), rec("web 1/1 Running", Some(0))];
        let dir = tempfile::tempdir().unwrap();
        let ctx = milestone_ctx(&history, dir.path());
        let outcome = evaluate_step_milestone(&ai_judge(), &ctx, Some(&runner))
            .await
            .unwrap();
        assert_eq!(outcome, EvalOutcome::Pass);
        assert_eq!(runner.call_count(), 1);
    }

    /// EvalContext.history == None must reproduce every existing step-grain
    /// verdict byte-identically (back-compat, D-06) — spot-check via
    /// evaluate_step (the step-grain entrypoint) still passing with
    /// history: None on the shared ctx_with helper.
    #[tokio::test]
    async fn step_grain_context_history_none_back_compat() {
        let check = StepCheck::CommandRegex {
            pattern: "pod/web (created|configured)".to_string(),
            match_stderr: false,
        };
        let ctx = ctx_with("pod/web created", Some(0));
        assert!(ctx.history.is_none());
        assert_eq!(evaluate_step(&check, &ctx).await.unwrap(), EvalOutcome::Pass);
    }
}
