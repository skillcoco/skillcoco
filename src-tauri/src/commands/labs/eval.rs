//! # commands::labs::eval — step evaluator + hint IPC handlers
//!
//! Wires `labs::evaluator::evaluate_step` into the IPC surface, persists Pass
//! outcomes to `lab_progress`, recomputes `module_progress.practical_mastery`,
//! and surfaces hint tiers from the spec.

use super::state::{read_lab_progress, recompute_practical_mastery};
use super::{
    LabCheckStepRequest, LabCheckStepResult, LabShowHintRequest, LabShowHintResult,
    LabValidateMilestoneRequest, LabValidateMilestoneResult,
};
use crate::auth::AuthState;
use crate::labs::evaluator::{
    evaluate_step, evaluate_step_milestone, milestone_reason, AiJudgeRunner, EvalContext,
    EvalOutcome,
};
use crate::labs::spec::{effective_step_grain, Grain, LabSpec, StepCheck};
use crate::{push_command_record, AppState, CommandRecord};
use tauri::State;

/// Plan 03.1-09 GAP-03 — compile-time marker referenced by the RED
/// integration test in `eval_tests.rs::lab_check_step_passes_authenticated_state_to_ai_judge`.
/// Existence of this symbol in the production module proves Task 4
/// landed the AuthState plumbing seam (`lab_check_step_with`).
#[cfg(test)]
pub(crate) const LAB_CHECK_STEP_WITH_SEAM_MARKER: () = ();

/// LAB-06 — evaluate a single step against the live session's terminal
/// buffer. On `Pass`, atomically updates `lab_progress.completed_step_ids`
/// and recomputes `module_progress.practical_mastery`.
///
/// Plan 03.1-09 GAP-03 — wires `tauri::State<AuthState>` into the
/// EvalContext so `ai_authenticated` reflects production reality. The
/// inner helper `lab_check_step_with` accepts the bool directly so unit
/// tests can drive both branches without standing up Tauri State.
#[tauri::command]
pub async fn lab_check_step(
    request: LabCheckStepRequest,
    state: State<'_, AppState>,
    auth_state: State<'_, AuthState>,
) -> Result<LabCheckStepResult, String> {
    // Resolve auth status once; map any failure (poisoned lock, missing
    // store) to false so a problem on the auth side never errors out a
    // lab session — the evaluator will fall back to Manual gracefully.
    let ai_authenticated = auth_state
        .get_active_credential()
        .map(|opt| opt.is_some())
        .unwrap_or(false);

    lab_check_step_with(request, state.inner(), ai_authenticated).await
}

/// Plan 03.1-09 GAP-03 — inner helper. Accepts `ai_authenticated`
/// directly so unit tests can exercise the authed / no-auth branches
/// without constructing Tauri `State<AuthState>`.
///
/// Phase 19.3 — takes `&AppState` (not `tauri::State`) so unit tests can
/// drive the FULL handler flow (append + grain dispatch + persist) against
/// a synthetic AppState; mirrors exam.rs's `..._conn` test-seam convention.
pub(crate) async fn lab_check_step_with(
    request: LabCheckStepRequest,
    state: &AppState,
    ai_authenticated: bool,
) -> Result<LabCheckStepResult, String> {
    // 1. Look up the session sidecar metadata AND append the command record
    //    (D-01: unconditional, all grains, BEFORE any verdict logic) while
    //    holding the lab_sessions lock.
    let (block_id, learner_id, module_id, workspace, ai_budget) = {
        let mut map = state.lab_sessions.lock().await;
        let entry = map
            .get_mut(&request.session_id)
            .ok_or_else(|| format!("session not found: {}", request.session_id))?;
        push_command_record(
            &mut entry.command_history,
            CommandRecord {
                command: request.last_command.clone(),
                output: request.last_output.clone(),
                exit_code: request.last_exit_code,
            },
        );
        (
            entry.block_id.clone(),
            entry.learner_id.clone(),
            entry.module_id.clone(),
            entry.workspace.clone(),
            entry.ai_budget_remaining,
        )
    };

    // 2. Read the spec from the block.
    let spec = read_lab_spec_from_db(state, &block_id)?;
    let step = spec
        .steps
        .get(request.step_index)
        .ok_or_else(|| format!("step_index {} out of range", request.step_index))?;

    // 3. D-04 — milestone-grain steps are append-only at prompt boundaries:
    //    no verdict evaluation, no persistence, no progress advance. The
    //    learner validates explicitly via `lab_validate_milestone`.
    if effective_step_grain(spec.grain, step.grain) == Grain::Milestone {
        return Ok(LabCheckStepResult {
            step_index: request.step_index,
            passed: false,
            reason: "milestone step — press Validate to check".to_string(),
            check_kind: check_kind_str(&step.check),
            mastery_delta: 0.0,
            // WR-03 — advisory, NOT a Fail: the UI must not render this
            // prompt-boundary skip as a failed check.
            outcome: "milestone_pending".to_string(),
        });
    }

    // 4. Step grain — existing path, byte-identical to pre-19.3 behavior.
    let ctx = EvalContext {
        last_command: &request.last_command,
        last_output: &request.last_output,
        last_exit_code: request.last_exit_code,
        workspace: &workspace,
        ai_authenticated,
        ai_budget_remaining: ai_budget,
        history: None,
    };
    let outcome = evaluate_step(&step.check, &ctx)
        .await
        .map_err(|e| format!("evaluate_step: {}", e))?;

    let check_kind = check_kind_str(&step.check);
    let passed = matches!(outcome, EvalOutcome::Pass);
    let reason = outcome_reason(&outcome);

    // 5. Persist outcome.
    let mastery_delta = persist_outcome(
        state,
        &learner_id,
        &module_id,
        &block_id,
        &step.id,
        spec.steps.len(),
        &outcome,
        &check_kind,
        request.step_index,
        &reason,
    )?;

    Ok(LabCheckStepResult {
        step_index: request.step_index,
        passed,
        reason,
        check_kind,
        mastery_delta,
        outcome: outcome_str(&outcome).to_string(),
    })
}

/// Phase 19.3 (D-04) — explicit milestone validation IPC. Evaluates the
/// current milestone-grain step against the session's cumulative command
/// history + workspace tree and routes the outcome through the SAME
/// `persist_outcome` path as `lab_check_step` (Pass advances
/// completed_step_ids identically).
#[tauri::command]
pub async fn lab_validate_milestone(
    request: LabValidateMilestoneRequest,
    state: State<'_, AppState>,
    auth_state: State<'_, AuthState>,
) -> Result<LabValidateMilestoneResult, String> {
    let ai_authenticated = auth_state
        .get_active_credential()
        .map(|opt| opt.is_some())
        .unwrap_or(false);

    lab_validate_milestone_with(request, state.inner(), ai_authenticated).await
}

/// Inner helper (test seam — mirrors `lab_check_step_with`).
///
/// T-19.3-02: session-scoped — learner/module/block/workspace resolve from
/// the server-held `LabSessionEntry` sidecar, never from the request.
pub(crate) async fn lab_validate_milestone_with(
    request: LabValidateMilestoneRequest,
    state: &AppState,
    ai_authenticated: bool,
) -> Result<LabValidateMilestoneResult, String> {
    // 1. Session sidecar + history snapshot.
    let (block_id, learner_id, module_id, workspace, ai_budget, history) = {
        let map = state.lab_sessions.lock().await;
        let entry = map
            .get(&request.session_id)
            .ok_or_else(|| format!("session not found: {}", request.session_id))?;
        (
            entry.block_id.clone(),
            entry.learner_id.clone(),
            entry.module_id.clone(),
            entry.workspace.clone(),
            entry.ai_budget_remaining,
            entry.command_history.clone(),
        )
    };

    // 2. Spec + step + grain guard (fail-safe: the Validate button only
    //    shows for milestone steps, but the handler must not trust the
    //    frontend).
    let spec = read_lab_spec_from_db(state, &block_id)?;
    let step = spec
        .steps
        .get(request.step_index)
        .ok_or_else(|| format!("step_index {} out of range", request.step_index))?;
    if effective_step_grain(spec.grain, step.grain) != Grain::Milestone {
        return Err(format!(
            "step_index {} is not milestone-grain; lab_validate_milestone only validates \
             milestone steps (D-04)",
            request.step_index
        ));
    }

    // 3. Evaluate against history + workspace.
    let ctx = EvalContext {
        last_command: "",
        last_output: "",
        last_exit_code: None,
        workspace: &workspace,
        ai_authenticated,
        ai_budget_remaining: ai_budget,
        history: Some(&history),
    };
    // 19.3-REVIEW WR-02 — `None` runner: DELIBERATE parity with the
    // step-grain production path (`evaluate_step` internally passes
    // `None::<&NoJudgeRunner>`). No production `AiJudgeRunner` impl exists
    // anywhere in the crate (pre-existing, predates 19.3), so ai_judge
    // with auth+budget degrades to Manual at BOTH grains identically.
    // Wiring a real runner (over ai::retry::ai_request_with_retry, with
    // ai_budget_remaining decrement) is a cross-grain change deferred to
    // its own phase — when it lands, thread it through this seam and
    // `lab_check_step_with` together.
    let outcome = evaluate_step_milestone(&step.check, &ctx, None::<&dyn AiJudgeRunner>)
        .await
        .map_err(|e| format!("evaluate_step_milestone: {}", e))?;

    let check_kind = check_kind_str(&step.check);
    let passed = matches!(outcome, EvalOutcome::Pass);
    // D-02 — surface the anti-vacuous "no commands recorded" reason where
    // applicable; otherwise fall back to the generic outcome reason.
    let reason =
        milestone_reason(&step.check, &ctx, &outcome).unwrap_or_else(|| outcome_reason(&outcome));

    // 4. SAME persist path as lab_check_step (D-04 key link).
    let mastery_delta = persist_outcome(
        state,
        &learner_id,
        &module_id,
        &block_id,
        &step.id,
        spec.steps.len(),
        &outcome,
        &check_kind,
        request.step_index,
        &reason,
    )?;

    Ok(LabValidateMilestoneResult {
        step_index: request.step_index,
        passed,
        reason,
        check_kind,
        mastery_delta,
        outcome: outcome_str(&outcome).to_string(),
    })
}

/// LAB-06 — return the requested hint tier text. Hint reveal state lives in
/// the frontend (RESEARCH § Open Question #7); this handler is a pure spec
/// lookup and validates the tier index.
#[tauri::command]
pub async fn lab_show_hint(
    request: LabShowHintRequest,
    state: State<'_, AppState>,
) -> Result<LabShowHintResult, String> {
    // Resolve block_id from the session.
    let block_id = {
        let map = state.lab_sessions.lock().await;
        let entry = map
            .get(&request.session_id)
            .ok_or_else(|| format!("session not found: {}", request.session_id))?;
        entry.block_id.clone()
    };
    let spec = read_lab_spec_from_db(state.inner(), &block_id)?;
    let step = spec
        .steps
        .get(request.step_index)
        .ok_or_else(|| format!("step_index {} out of range", request.step_index))?;
    resolve_hint(&step.hints, request.current_tier)
}

/// Pure helper: given a hint list and the tier the frontend is currently
/// showing, return the next tier's content + whether it's the final tier.
pub(crate) fn resolve_hint(
    hints: &[String],
    current_tier: u8,
) -> Result<LabShowHintResult, String> {
    if hints.is_empty() {
        return Err("no hints declared on this step".to_string());
    }
    let next_index = current_tier as usize;
    if next_index >= hints.len() {
        return Err(format!(
            "current_tier {} out of range; max hint index is {}",
            current_tier,
            hints.len() - 1
        ));
    }
    let text = hints[next_index].clone();
    let tier = (next_index as u8) + 1;
    let final_tier = next_index + 1 == hints.len();
    Ok(LabShowHintResult { tier, text, final_tier })
}

fn check_kind_str(check: &StepCheck) -> String {
    match check {
        StepCheck::CommandRegex { .. } => "commandRegex".to_string(),
        StepCheck::ExitCode { .. } => "exitCode".to_string(),
        StepCheck::FileState { .. } => "fileState".to_string(),
        StepCheck::AiJudge { .. } => "aiJudge".to_string(),
        // Phase 19.2 (D-07) — deterministic check, same class as
        // CommandRegex: "commandAbsent" != "aiJudge" so it falls through
        // the `check_kind == "aiJudge"` branch below with no ai_judge
        // verdict persistence; Pass/Fail flows straight to
        // completed_step_ids via matches!(outcome, EvalOutcome::Pass).
        StepCheck::CommandAbsent { .. } => "commandAbsent".to_string(),
    }
}

/// 19.3-REVIEW WR-03 — stable snake_case outcome string for the wire (and
/// the persisted ai_judge verdict). The frontend consumes this structurally
/// instead of substring-sniffing the human-readable `reason`.
fn outcome_str(outcome: &EvalOutcome) -> &'static str {
    match outcome {
        EvalOutcome::Pass => "pass",
        EvalOutcome::Fail => "fail",
        EvalOutcome::Indeterminate => "indeterminate",
        EvalOutcome::Manual => "manual",
    }
}

fn outcome_reason(outcome: &EvalOutcome) -> String {
    match outcome {
        EvalOutcome::Pass => "step passed".to_string(),
        EvalOutcome::Fail => "check failed — review the step and try again".to_string(),
        EvalOutcome::Indeterminate => {
            "no exit code observed yet — run the command and try again".to_string()
        }
        EvalOutcome::Manual => {
            "manual recheck required (AI-judge budget exhausted or no auth)".to_string()
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn persist_outcome(
    state: &AppState,
    learner_id: &str,
    module_id: &str,
    block_id: &str,
    step_id: &str,
    total_steps: usize,
    outcome: &EvalOutcome,
    check_kind: &str,
    step_index: usize,
    reason: &str,
) -> Result<f64, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("db lock: {}", e))?;
    let conn = &db.conn;

    // Ensure the row exists.
    conn.execute(
        "INSERT OR IGNORE INTO lab_progress
            (learner_id, module_id, block_id, current_step, completed_step_ids,
             total_steps, metadata_json, last_updated)
         VALUES (?1, ?2, ?3, 0, '[]', ?4, '{}', datetime('now'))",
        rusqlite::params![learner_id, module_id, block_id, total_steps as i64],
    )
    .map_err(|e| format!("ensure lab_progress: {}", e))?;

    // For ai_judge outcomes (Pass/Fail/Manual), persist the verdict —
    // keyed per step (WR-06) plus the legacy last_ai_judge slot.
    if check_kind == "aiJudge" {
        let verdict_outcome = outcome_str(outcome);
        let verdict = serde_json::json!({
            "step_index": step_index,
            "outcome": verdict_outcome,
            "reason": reason,
            "at": chrono::Utc::now().to_rfc3339(),
        })
        .to_string();
        persist_ai_judge_verdict(conn, learner_id, module_id, block_id, step_index, &verdict)?;
    }

    // On Pass: append to completed_step_ids + bump current_step.
    //
    // 19.3-REVIEW CR-02 — IDEMPOTENT: at milestone grain the passing
    // evidence is the persistent session history, so a repeated
    // `lab_validate_milestone` on an already-passed step Passes again. The
    // json_each guard skips the update when step_id is already present,
    // preventing duplicate completed_step_ids entries (which inflate
    // practical_mastery past 1.0 — it feeds achievements) and current_step
    // overrun past total_steps.
    if matches!(outcome, EvalOutcome::Pass) {
        conn.execute(
            "UPDATE lab_progress
             SET current_step = current_step + 1,
                 completed_step_ids = json_insert(completed_step_ids, '$[#]', ?1),
                 last_updated = datetime('now')
             WHERE learner_id = ?2 AND module_id = ?3 AND block_id = ?4
               AND NOT EXISTS (
                 SELECT 1 FROM json_each(lab_progress.completed_step_ids)
                 WHERE json_each.value = ?1
               )",
            rusqlite::params![step_id, learner_id, module_id, block_id],
        )
        .map_err(|e| format!("Pass update: {}", e))?;
    }

    let mastery = recompute_practical_mastery(conn, module_id, learner_id)?;
    let _ = read_lab_progress(conn, learner_id, module_id, block_id)?;
    Ok(mastery)
}

/// Phase 19 (WR-06) — persist an ai_judge verdict into BOTH
/// `metadata_json.$.ai_judge_verdicts."<step_index>"` (one slot per step,
/// so multi-ai_judge exams keep every step's latest verdict for
/// `exam::derive_step_verdicts`) AND the legacy single-slot
/// `$.last_ai_judge` (backward compat — older rows/readers still work).
/// The CASE seeds `$.ai_judge_verdicts` with `{}` when absent, since
/// `json_set` cannot create intermediate objects.
pub(crate) fn persist_ai_judge_verdict(
    conn: &rusqlite::Connection,
    learner_id: &str,
    module_id: &str,
    block_id: &str,
    step_index: usize,
    verdict_json: &str,
) -> Result<(), String> {
    let keyed_path = format!("$.ai_judge_verdicts.\"{}\"", step_index);
    conn.execute(
        "UPDATE lab_progress
         SET metadata_json = json_set(
                 CASE WHEN json_extract(metadata_json, '$.ai_judge_verdicts') IS NULL
                      THEN json_set(metadata_json, '$.ai_judge_verdicts', json('{}'))
                      ELSE metadata_json END,
                 ?1, json(?2),
                 '$.last_ai_judge', json(?2)),
             last_updated = datetime('now')
         WHERE learner_id = ?3 AND module_id = ?4 AND block_id = ?5",
        rusqlite::params![keyed_path, verdict_json, learner_id, module_id, block_id],
    )
    .map_err(|e| format!("metadata_json update: {}", e))?;
    Ok(())
}

/// 19.3-REVIEW WR-01 — delegates to the single promoted
/// `read_lab_spec_conn` helper (commands/labs/mod.rs) so DB-stored specs
/// are re-validated via `validate_spec` on the EVALUATION path too: the
/// D-05 exam x milestone exclusion, duplicate step ids, and weight checks
/// all hold before `lab_check_step` / `lab_validate_milestone` /
/// `lab_show_hint` score anything. Validation is cheap (in-memory walks,
/// per-call) — no caching needed. Previously this was a drifted bespoke
/// copy that skipped validate_spec entirely.
fn read_lab_spec_from_db(
    state: &AppState,
    block_id: &str,
) -> Result<LabSpec, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("db lock: {}", e))?;
    super::read_lab_spec_conn(&db.conn, block_id).map(|(spec, _)| spec)
}

#[cfg(test)]
#[path = "eval_tests.rs"]
mod tests;

