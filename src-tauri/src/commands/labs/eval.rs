//! # commands::labs::eval — step evaluator + hint IPC handlers
//!
//! Wires `labs::evaluator::evaluate_step` into the IPC surface, persists Pass
//! outcomes to `lab_progress`, recomputes `module_progress.practical_mastery`,
//! and surfaces hint tiers from the spec.

use super::state::{read_lab_progress, recompute_practical_mastery};
use super::{LabCheckStepRequest, LabCheckStepResult, LabShowHintRequest, LabShowHintResult};
use crate::auth::AuthState;
use crate::labs::evaluator::{evaluate_step, EvalContext, EvalOutcome};
use crate::labs::spec::{LabSpec, StepCheck};
use crate::AppState;
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

    lab_check_step_with(request, state, ai_authenticated).await
}

/// Plan 03.1-09 GAP-03 — inner helper. Accepts `ai_authenticated`
/// directly so unit tests can exercise the authed / no-auth branches
/// without constructing Tauri `State<AuthState>`.
pub(crate) async fn lab_check_step_with(
    request: LabCheckStepRequest,
    state: State<'_, AppState>,
    ai_authenticated: bool,
) -> Result<LabCheckStepResult, String> {
    // 1. Look up the session sidecar metadata.
    let (block_id, learner_id, module_id, workspace, ai_budget) = {
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
        )
    };

    // 2. Read the spec from the block.
    let spec = read_lab_spec_from_db(&state, &block_id)?;
    let step = spec
        .steps
        .get(request.step_index)
        .ok_or_else(|| format!("step_index {} out of range", request.step_index))?;

    // 3. Build EvalContext + dispatch.
    let ctx = EvalContext {
        last_command: &request.last_command,
        last_output: &request.last_output,
        last_exit_code: request.last_exit_code,
        workspace: &workspace,
        ai_authenticated,
        ai_budget_remaining: ai_budget,
    };
    let outcome = evaluate_step(&step.check, &ctx)
        .await
        .map_err(|e| format!("evaluate_step: {}", e))?;

    let check_kind = check_kind_str(&step.check);
    let passed = matches!(outcome, EvalOutcome::Pass);
    let reason = outcome_reason(&outcome);

    // 4. Persist outcome.
    let mastery_delta = persist_outcome(
        &state,
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
    let spec = read_lab_spec_from_db(&state, &block_id)?;
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
    state: &State<'_, AppState>,
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

    // For ai_judge outcomes (Pass/Fail/Manual), persist the verdict to
    // metadata_json.$.last_ai_judge for diagnostics + future review.
    if check_kind == "aiJudge" {
        let verdict_outcome = match outcome {
            EvalOutcome::Pass => "pass",
            EvalOutcome::Fail => "fail",
            EvalOutcome::Indeterminate => "indeterminate",
            EvalOutcome::Manual => "manual",
        };
        let verdict = serde_json::json!({
            "step_index": step_index,
            "outcome": verdict_outcome,
            "reason": reason,
            "at": chrono::Utc::now().to_rfc3339(),
        })
        .to_string();
        conn.execute(
            "UPDATE lab_progress
             SET metadata_json = json_set(metadata_json, '$.last_ai_judge', json(?1)),
                 last_updated = datetime('now')
             WHERE learner_id = ?2 AND module_id = ?3 AND block_id = ?4",
            rusqlite::params![verdict, learner_id, module_id, block_id],
        )
        .map_err(|e| format!("metadata_json update: {}", e))?;
    }

    // On Pass: append to completed_step_ids + bump current_step.
    if matches!(outcome, EvalOutcome::Pass) {
        conn.execute(
            "UPDATE lab_progress
             SET current_step = current_step + 1,
                 completed_step_ids = json_insert(completed_step_ids, '$[#]', ?1),
                 last_updated = datetime('now')
             WHERE learner_id = ?2 AND module_id = ?3 AND block_id = ?4",
            rusqlite::params![step_id, learner_id, module_id, block_id],
        )
        .map_err(|e| format!("Pass update: {}", e))?;
    }

    let mastery = recompute_practical_mastery(conn, module_id, learner_id)?;
    let _ = read_lab_progress(conn, learner_id, module_id, block_id)?;
    Ok(mastery)
}

fn read_lab_spec_from_db(
    state: &State<'_, AppState>,
    block_id: &str,
) -> Result<LabSpec, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("db lock: {}", e))?;
    let conn = &db.conn;
    let block = crate::db::blocks::get_block(conn, block_id)
        .map_err(|e| format!("get_block: {}", e))?
        .ok_or_else(|| format!("block not found: {}", block_id))?;

    if !block.payload_json.trim().is_empty() && block.payload_json != "{}" {
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&block.payload_json) {
            if let Some(spec_val) = payload.get("spec") {
                if let Ok(spec) =
                    serde_json::from_value::<LabSpec>(spec_val.clone())
                {
                    return Ok(spec);
                }
            }
        }
    }
    if let Ok(params) = serde_json::from_str::<serde_json::Value>(&block.params_json) {
        if let Some(md) = params.get("labMd").and_then(|v| v.as_str()) {
            return crate::labs::spec::parse_lab_md(md)
                .map(|(s, _)| s)
                .map_err(|e| format!("parse_lab_md: {}", e));
        }
    }
    Err(format!("block {} has no readable lab spec", block_id))
}

#[cfg(test)]
#[path = "eval_tests.rs"]
mod tests;

