//! # commands::labs::exam — timed/scored exam-run IPC lifecycle
//! (Phase 19, Wave 0 RED scaffold — implemented by 19-03)
//!
//! Three IPC entry points: `exam_attempt_start`, `exam_attempt_submit`,
//! `exam_attempt_get`. Wave 0 ships stub bodies that panic on call so
//! `cargo test --lib commands::labs::exam` fails RED for the right reason
//! (missing implementation, not a compile error). See the three handler
//! bodies below for the exact stub macro used.
//!
//! ## D-15 — server-authoritative scoring (T-19-10 mitigation)
//!
//! `ExamAttemptSubmitRequest` carries `attempt_id` and `current_step` ONLY.
//! It does NOT admit a client-supplied verdicts field. 19-03's real
//! implementation DERIVES every step verdict server-side from
//! `lab_progress.completed_step_ids` (Pass) and
//! `lab_progress.metadata_json.$.last_ai_judge` (fail/manual/indeterminate)
//! — see `commands::labs::state::read_lab_progress` (state.rs:123) and
//! `commands::labs::eval::persist_outcome` (eval.rs:187-255), the same
//! rails regular labs already write into. A step absent from
//! `completed_step_ids` scores Fail regardless of what a caller might
//! wish to claim — this is the blocker-fix contract this scaffold locks
//! in before any implementation exists.
//!
//! ## D-04 — stale in_progress reconciliation
//!
//! `exam_attempt_get` on an `in_progress` row whose `deadline_at` has
//! passed must lazily reconcile it to `timed_out_partial` on read.

use serde::{Deserialize, Serialize};

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
pub struct StepVerdictResult {
    pub step_id: String,
    pub verdict: String, // "pass" | "fail" | "manual" | "indeterminate"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamAttemptResult {
    pub attempt_id: String,
    pub status: String, // "in_progress" | "submitted" | "timed_out_partial"
    pub score_percent: f64,
    pub passed: bool,
    pub step_verdicts: Vec<StepVerdictResult>,
    pub started_at: String,
    pub deadline_at: String,
    pub finished_at: Option<String>,
}

/// Deterministic-clock seam (mirrors `labs::prompt_detect`'s tick-driven
/// heuristic timeout pattern) so exam_tests can assert timeout/deadline
/// logic without depending on wall-clock `Utc::now()`. 19-03's real
/// implementation injects `RealClock` in production and `FakeClock` in
/// tests.
pub trait Clock {
    fn now(&self) -> chrono::DateTime<chrono::Utc>;
}

pub struct RealClock;

impl Clock for RealClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
}

// ── IPC handlers (stubs — 19-03 implements) ──

#[tauri::command]
pub async fn exam_attempt_start(
    _request: ExamAttemptStartRequest,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<ExamAttemptStartResult, String> {
    unimplemented!("19-03: exam IPC lifecycle — exam_attempt_start")
}

#[tauri::command]
pub async fn exam_attempt_submit(
    _request: ExamAttemptSubmitRequest,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<ExamAttemptResult, String> {
    unimplemented!("19-03: exam IPC lifecycle — exam_attempt_submit")
}

#[tauri::command]
pub async fn exam_attempt_get(
    _request: ExamAttemptGetRequest,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<ExamAttemptResult, String> {
    unimplemented!("19-03: exam IPC lifecycle — exam_attempt_get")
}

#[cfg(test)]
#[path = "exam_tests.rs"]
mod tests;
