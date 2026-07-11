//! # commands::labs::exam — timed/scored exam-run IPC lifecycle
//! (Phase 19, implemented 19-03)
//!
//! Three IPC entry points: `exam_attempt_start`, `exam_attempt_submit`,
//! `exam_attempt_get`. Each `#[tauri::command]` handler delegates to a
//! `pub(crate)` `Connection`-based inner helper (`..._conn`) so
//! `exam_tests.rs` can exercise the full derive/finalize/timeout logic
//! against an in-memory SQLite connection without needing a
//! `tauri::State<AppState>` (which cannot be constructed outside the
//! Tauri runtime).
//!
//! ## D-15 — server-authoritative scoring (T-19-10 mitigation)
//!
//! `ExamAttemptSubmitRequest` carries `attempt_id` and `current_step` ONLY.
//! It does NOT admit a client-supplied verdicts field. Every step verdict
//! is DERIVED server-side from `lab_progress.completed_step_ids` (Pass) and
//! `lab_progress.metadata_json.$.last_ai_judge` (fail/manual/indeterminate)
//! — see `commands::labs::state::read_lab_progress` (state.rs:123) and
//! `commands::labs::eval::persist_outcome` (eval.rs:187-255), the same
//! rails regular labs already write into. A step absent from
//! `completed_step_ids` scores Fail regardless of what a caller might
//! wish to claim.
//!
//! ## D-04 — stale in_progress reconciliation
//!
//! `exam_attempt_get` on an `in_progress` row whose `deadline_at` has
//! passed lazily reconciles it to `timed_out_partial` on read.

use crate::labs::spec::LabStep;
use crate::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── IPC structs (camelCase, FIX-02) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamAttemptStartRequest {
    pub block_id: String,
    pub track_id: String,
    pub module_id: String,
    pub learner_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamAttemptStartResult {
    pub attempt_id: String,
    pub started_at: String,
    pub deadline_at: String,
    pub time_limit_minutes: u32,
    pub pass_threshold_pct: f64,
    pub total_steps: usize,
}

/// D-15 — NO verdicts field. Submit carries only the attempt id and
/// (optionally) which step the learner had reached; every verdict is
/// server-derived from `lab_progress`, never accepted from the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamAttemptSubmitRequest {
    pub attempt_id: String,
    pub current_step: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamAttemptGetRequest {
    pub attempt_id: String,
}

/// Per-step verdict in the RESULT (server-derived — never accepted as
/// client input; this struct is a response shape only).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StepVerdict {
    pub step_id: String,
    pub title: String,
    pub outcome: String, // "pass" | "fail" | "manual" | "indeterminate"
    pub passed_toward_score: bool,
    pub check_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamAttemptResult {
    pub attempt_id: String,
    pub status: String, // "in_progress" | "completed" | "timed_out_partial"
    pub score_percent: f64,
    pub passed: bool,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub deadline_at: String,
    pub total_steps: usize,
    pub step_verdicts: Vec<StepVerdict>,
}

/// Deterministic-clock seam (mirrors `labs::prompt_detect`'s tick-driven
/// heuristic timeout pattern) so exam_tests can assert timeout/deadline
/// logic without depending on wall-clock `Utc::now()`. Production wires
/// `RealClock`; tests inject `FakeClock`.
pub trait Clock {
    fn now(&self) -> chrono::DateTime<chrono::Utc>;
}

pub struct RealClock;

impl Clock for RealClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
}

const DEFAULT_TIME_LIMIT_MINUTES: u32 = 30; // D-03
const DEFAULT_PASS_THRESHOLD_PCT: f64 = 70.0; // D-08

/// Row snapshot of an `exam_attempts` record used internally by the
/// submit/get/finalize paths.
struct AttemptRow {
    learner_id: String,
    module_id: String,
    block_id: String,
    started_at: String,
    deadline_at: String,
    status: String,
    score_percent: f64,
    passed: bool,
    step_verdicts_json: String,
    total_steps: usize,
    finished_at: Option<String>,
}

fn read_attempt_row(conn: &Connection, attempt_id: &str) -> Result<AttemptRow, String> {
    conn.query_row(
        "SELECT learner_id, module_id, block_id, started_at, deadline_at, status,
                score_percent, passed, step_verdicts_json, total_steps, finished_at
         FROM exam_attempts WHERE id = ?1",
        rusqlite::params![attempt_id],
        |r| {
            Ok(AttemptRow {
                learner_id: r.get(0)?,
                module_id: r.get(1)?,
                block_id: r.get(2)?,
                started_at: r.get(3)?,
                deadline_at: r.get(4)?,
                status: r.get(5)?,
                score_percent: r.get::<_, Option<f64>>(6)?.unwrap_or(0.0),
                passed: r.get::<_, Option<bool>>(7)?.unwrap_or(false),
                step_verdicts_json: r.get(8)?,
                total_steps: r.get::<_, i64>(9)? as usize,
                finished_at: r.get(10)?,
            })
        },
    )
    .map_err(|e| format!("read exam_attempts row {}: {}", attempt_id, e))
}

fn attempt_row_to_result(row: &AttemptRow, attempt_id: &str) -> Result<ExamAttemptResult, String> {
    let step_verdicts: Vec<StepVerdict> = serde_json::from_str(&row.step_verdicts_json)
        .map_err(|e| format!("step_verdicts_json parse: {}", e))?;
    Ok(ExamAttemptResult {
        attempt_id: attempt_id.to_string(),
        status: row.status.clone(),
        score_percent: row.score_percent,
        passed: row.passed,
        started_at: row.started_at.clone(),
        finished_at: row.finished_at.clone(),
        deadline_at: row.deadline_at.clone(),
        total_steps: row.total_steps,
        step_verdicts,
    })
}

/// D-15 — server-authoritative per-step verdict derivation. For each
/// `LabStep` in the block's parsed spec: Pass when `step.id` is present in
/// `completed_step_ids`; otherwise consult
/// `lab_progress.metadata_json.$.last_ai_judge` (matched by step_index) for
/// fail/manual/indeterminate; default to Fail when absent. Manual and
/// Indeterminate never count toward the score (UI-SPEC lock).
fn derive_step_verdicts(
    steps: &[LabStep],
    completed_step_ids: &[String],
    last_ai_judge: Option<&serde_json::Value>,
) -> Vec<StepVerdict> {
    steps
        .iter()
        .enumerate()
        .map(|(idx, step)| {
            if completed_step_ids.iter().any(|id| id == &step.id) {
                return StepVerdict {
                    step_id: step.id.clone(),
                    title: step.title.clone(),
                    outcome: "pass".to_string(),
                    passed_toward_score: true,
                    check_reason: Some("step passed".to_string()),
                };
            }

            // Consult the last persisted AI-judge verdict when it targets
            // this step index; otherwise default to Fail (D-15).
            if let Some(judge) = last_ai_judge {
                let judge_step_index = judge.get("step_index").and_then(|v| v.as_u64());
                if judge_step_index == Some(idx as u64) {
                    let outcome = judge
                        .get("outcome")
                        .and_then(|v| v.as_str())
                        .unwrap_or("fail")
                        .to_string();
                    let reason =
                        judge.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());
                    // Only Pass counts toward the score; manual/indeterminate/fail
                    // all score as not-passed (UI-SPEC lock).
                    return StepVerdict {
                        step_id: step.id.clone(),
                        title: step.title.clone(),
                        outcome,
                        passed_toward_score: false,
                        check_reason: reason,
                    };
                }
            }

            StepVerdict {
                step_id: step.id.clone(),
                title: step.title.clone(),
                outcome: "fail".to_string(),
                passed_toward_score: false,
                check_reason: None,
            }
        })
        .collect()
}

/// Weighted score: `100 * sum(weight where passed_toward_score) / sum(weight)`.
/// Manual/Indeterminate/Fail all count in the denominator but never the
/// numerator (UI-SPEC lock).
fn weighted_score_percent(steps: &[LabStep], verdicts: &[StepVerdict]) -> f64 {
    let total_weight: f64 = steps.iter().map(|s| s.weight).sum();
    if total_weight <= 0.0 {
        return 0.0;
    }
    let passed_weight: f64 = steps
        .iter()
        .zip(verdicts.iter())
        .filter(|(_, v)| v.passed_toward_score)
        .map(|(s, _)| s.weight)
        .sum();
    100.0 * passed_weight / total_weight
}

/// Shared derive-and-finalize logic used by both `exam_attempt_submit` and
/// the lazy-reconcile path in `exam_attempt_get`. Recomputes `timed_out`
/// server-side from the persisted `deadline_at` — never trusts a client
/// flag (T-19-01). Idempotent: if the attempt is already finalized
/// (`status != 'in_progress'`), returns the existing result unchanged
/// without re-scoring (T-19-05). Takes a bare `&Connection` so tests can
/// drive it directly (mirrors `read_lab_spec_conn`'s testability seam).
pub(crate) fn finalize_attempt_conn(
    conn: &Connection,
    attempt_id: &str,
    clock: &dyn Clock,
) -> Result<ExamAttemptResult, String> {
    // Step 1: read the attempt row + guard idempotency (T-19-05).
    let row = read_attempt_row(conn, attempt_id)?;
    if row.status != "in_progress" {
        return attempt_row_to_result(&row, attempt_id);
    }

    // Step 2: read the block's parsed spec (promoted helper, no third copy).
    let (spec, _body) = super::read_lab_spec_conn(conn, &row.block_id)?;

    // Step 3: read lab_progress (D-15 server-authoritative source) +
    // the raw metadata_json for the last_ai_judge verdict.
    let progress =
        super::state::read_lab_progress(conn, &row.learner_id, &row.module_id, &row.block_id)?;
    let metadata_json: String = conn
        .query_row(
            "SELECT metadata_json FROM lab_progress
             WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
            rusqlite::params![row.learner_id, row.module_id, row.block_id],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "{}".to_string());
    let metadata: serde_json::Value =
        serde_json::from_str(&metadata_json).unwrap_or(serde_json::json!({}));
    let last_ai_judge = metadata.get("last_ai_judge").cloned();

    // Step 4: recompute timeout server-side from the persisted deadline —
    // never trust a client flag (T-19-01). String-lexicographic RFC-3339
    // comparison matches the datetime-TEXT precedent (STATE.md 07-04).
    let now = clock.now().to_rfc3339();
    let timed_out = now.as_str() >= row.deadline_at.as_str();

    // Step 5: derive verdicts + weighted score (D-15).
    let step_verdicts =
        derive_step_verdicts(&spec.steps, &progress.completed_step_ids, last_ai_judge.as_ref());
    let score_percent = weighted_score_percent(&spec.steps, &step_verdicts);

    let pass_threshold_pct = spec
        .exam
        .as_ref()
        .and_then(|e| e.pass_threshold_pct)
        .unwrap_or(DEFAULT_PASS_THRESHOLD_PCT);
    let passed = score_percent >= pass_threshold_pct;

    let status = if timed_out { "timed_out_partial" } else { "completed" };
    let finished_at = clock.now().to_rfc3339();
    let step_verdicts_json =
        serde_json::to_string(&step_verdicts).map_err(|e| format!("serialize verdicts: {}", e))?;

    // Step 6: persist — bound params only (T-19-03), no string-formatted SQL.
    // The `status = 'in_progress'` guard makes this UPDATE itself idempotent
    // against a concurrent finalize racing in between steps 1 and 6.
    conn.execute(
        "UPDATE exam_attempts
         SET status = ?1, score_percent = ?2, passed = ?3,
             step_verdicts_json = ?4, finished_at = ?5
         WHERE id = ?6 AND status = 'in_progress'",
        rusqlite::params![
            status,
            score_percent,
            passed,
            step_verdicts_json,
            finished_at,
            attempt_id
        ],
    )
    .map_err(|e| format!("finalize UPDATE: {}", e))?;

    Ok(ExamAttemptResult {
        attempt_id: attempt_id.to_string(),
        status: status.to_string(),
        score_percent,
        passed,
        started_at: row.started_at,
        finished_at: Some(finished_at),
        deadline_at: row.deadline_at,
        total_steps: row.total_steps,
        step_verdicts,
    })
}

/// `Connection`-based inner helper for `exam_attempt_start` (test seam).
pub(crate) fn exam_attempt_start_conn(
    conn: &Connection,
    request: &ExamAttemptStartRequest,
    clock: &dyn Clock,
) -> Result<ExamAttemptStartResult, String> {
    // 1. Read the block's parsed spec for total_steps + ExamMeta defaults.
    let (spec, _body) = super::read_lab_spec_conn(conn, &request.block_id)?;
    let time_limit_minutes = spec
        .exam
        .as_ref()
        .and_then(|e| e.time_limit_minutes)
        .unwrap_or(DEFAULT_TIME_LIMIT_MINUTES);
    let pass_threshold_pct = spec
        .exam
        .as_ref()
        .and_then(|e| e.pass_threshold_pct)
        .unwrap_or(DEFAULT_PASS_THRESHOLD_PCT);
    let total_steps = spec.steps.len();

    // 2. Compute started_at / deadline_at from the injected clock.
    let started_at = clock.now();
    let deadline_at = started_at + chrono::Duration::minutes(time_limit_minutes as i64);
    let started_at_str = started_at.to_rfc3339();
    let deadline_at_str = deadline_at.to_rfc3339();
    let attempt_id = format!("exam-{}", uuid::Uuid::new_v4());

    // 3. INSERT (not INSERT OR REPLACE) — each attempt is distinct history (D-05).
    conn.execute(
        "INSERT INTO exam_attempts
            (id, learner_id, module_id, block_id, started_at, deadline_at,
             status, score_percent, passed, step_verdicts_json, total_steps)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'in_progress', 0.0, 0, '[]', ?7)",
        rusqlite::params![
            attempt_id,
            request.learner_id,
            request.module_id,
            request.block_id,
            started_at_str,
            deadline_at_str,
            total_steps as i64,
        ],
    )
    .map_err(|e| format!("exam_attempts INSERT: {}", e))?;

    Ok(ExamAttemptStartResult {
        attempt_id,
        started_at: started_at_str,
        deadline_at: deadline_at_str,
        time_limit_minutes,
        pass_threshold_pct,
        total_steps,
    })
}

/// `Connection`-based inner helper for `exam_attempt_get` (test seam).
/// D-04 — lazily reconciles a stale in_progress attempt past its deadline.
pub(crate) fn exam_attempt_get_conn(
    conn: &Connection,
    attempt_id: &str,
    clock: &dyn Clock,
) -> Result<ExamAttemptResult, String> {
    let row = read_attempt_row(conn, attempt_id)?;

    if row.status == "in_progress" {
        let now = clock.now().to_rfc3339();
        if now.as_str() >= row.deadline_at.as_str() {
            return finalize_attempt_conn(conn, attempt_id, clock);
        }
    }

    attempt_row_to_result(&row, attempt_id)
}

// ── IPC handlers ──

#[tauri::command]
pub async fn exam_attempt_start(
    request: ExamAttemptStartRequest,
    state: State<'_, AppState>,
) -> Result<ExamAttemptStartResult, String> {
    let db = state.db.lock().map_err(|e| format!("db lock: {}", e))?;
    exam_attempt_start_conn(&db.conn, &request, &RealClock)
}

#[tauri::command]
pub async fn exam_attempt_submit(
    request: ExamAttemptSubmitRequest,
    state: State<'_, AppState>,
) -> Result<ExamAttemptResult, String> {
    // current_step is display-only telemetry (never trusted for scoring);
    // it is intentionally not persisted or read here.
    let _ = request.current_step;
    let db = state.db.lock().map_err(|e| format!("db lock: {}", e))?;
    finalize_attempt_conn(&db.conn, &request.attempt_id, &RealClock)
}

#[tauri::command]
pub async fn exam_attempt_get(
    request: ExamAttemptGetRequest,
    state: State<'_, AppState>,
) -> Result<ExamAttemptResult, String> {
    let db = state.db.lock().map_err(|e| format!("db lock: {}", e))?;
    exam_attempt_get_conn(&db.conn, &request.attempt_id, &RealClock)
}

#[cfg(test)]
#[path = "exam_tests.rs"]
mod tests;
