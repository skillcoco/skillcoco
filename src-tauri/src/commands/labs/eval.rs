//! # commands::labs::eval — step evaluator + hint IPC handlers
//!
//! Wires `labs::evaluator::evaluate_step` into the IPC surface, persists Pass
//! outcomes to `lab_progress`, recomputes `module_progress.practical_mastery`,
//! and surfaces hint tiers from the spec.

use super::state::{read_lab_progress, recompute_practical_mastery};
use super::{LabCheckStepRequest, LabCheckStepResult, LabShowHintRequest, LabShowHintResult};
use crate::labs::evaluator::{evaluate_step, EvalContext, EvalOutcome};
use crate::labs::spec::{LabSpec, StepCheck};
use crate::AppState;
use tauri::State;

/// LAB-06 — evaluate a single step against the live session's terminal
/// buffer. On `Pass`, atomically updates `lab_progress.completed_step_ids`
/// and recomputes `module_progress.practical_mastery`.
#[tauri::command]
pub async fn lab_check_step(
    request: LabCheckStepRequest,
    state: State<'_, AppState>,
) -> Result<LabCheckStepResult, String> {
    // 1. Look up the session sidecar metadata.
    let (block_id, learner_id, module_id, workspace, ai_budget) = {
        let map = state
            .lab_sessions
            .lock()
            .map_err(|e| format!("lab_sessions lock: {}", e))?;
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
        ai_authenticated: false, // wired in 03.1-06 once the auth seam is plumbed
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
        let map = state
            .lab_sessions
            .lock()
            .map_err(|e| format!("lab_sessions lock: {}", e))?;
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
mod tests {
    use super::*;

    /// Build a tier list and verify resolve_hint emits the next tier.
    #[test]
    fn lab_show_hint_returns_correct_tier() {
        let hints = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let r = resolve_hint(&hints, 0).unwrap();
        assert_eq!(r.tier, 1);
        assert_eq!(r.text, "a");
        assert!(!r.final_tier);

        let r = resolve_hint(&hints, 1).unwrap();
        assert_eq!(r.tier, 2);
        assert_eq!(r.text, "b");
        assert!(!r.final_tier);

        let r = resolve_hint(&hints, 2).unwrap();
        assert_eq!(r.tier, 3);
        assert_eq!(r.text, "c");
        assert!(r.final_tier, "third tier is final");
    }

    /// Out-of-range tier indices return Err.
    #[test]
    fn lab_show_hint_rejects_invalid_tier() {
        let hints = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert!(resolve_hint(&hints, 3).is_err());
        assert!(resolve_hint(&hints, 99).is_err());
        assert!(resolve_hint(&[], 0).is_err());
    }

    /// `check_kind_str` round-trips every variant to a stable camelCase
    /// string.
    #[test]
    fn check_kind_strings() {
        assert_eq!(
            check_kind_str(&StepCheck::CommandRegex {
                pattern: "x".to_string(),
                match_stderr: false,
            }),
            "commandRegex"
        );
        assert_eq!(
            check_kind_str(&StepCheck::ExitCode { expected: 0 }),
            "exitCode"
        );
        assert_eq!(
            check_kind_str(&StepCheck::FileState {
                path: "p".to_string(),
                contains: None,
            }),
            "fileState"
        );
        assert_eq!(
            check_kind_str(&StepCheck::AiJudge {
                criteria: "do this thing carefully".to_string(),
                threshold: 0.7,
            }),
            "aiJudge"
        );
    }
}
