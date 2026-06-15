//! Daily-challenge IPC handlers (Phase 4 Wave 0 RED shell).
//!
//! Wave 0 lands the request/result struct surface — the IPC contract Plan 03
//! will wire into `tauri::generate_handler!`. No `#[tauri::command]` attributes
//! yet; that's Plan 03's job. Bodies are `unimplemented!()` so the typed
//! surface compiles and downstream waves get a stable target.
//!
//! Every struct carries `#[serde(rename_all = "camelCase")]` per the
//! mandatory FIX-02 / Q9 IPC contract. The "envelope" shape (single `request:
//! T` argument per command) is set by Plan 03 — request structs intentionally
//! exist (even when empty) so the JS layer always invokes with
//! `{ request: T }` and forward-compat additions don't break the wire format.

use serde::{Deserialize, Serialize};

// ── get_daily_challenge ──

/// Empty request envelope — kept as a struct (not `()`) so Plan 03 can grow
/// optional fields (e.g., timezone hint) without breaking the JS call site.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDailyChallengeRequest {}

/// The block payload the daily view renders. `status` is the
/// engagement-state machine ("pending" | "in_progress" | "done"), NOT the
/// `BlockStatus` enum (R1 — that enum is untouched).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyChallengePayload {
    pub block_id: String,
    pub block_type: String,
    pub module_id: String,
    pub track_id: String,
    pub est_minutes: i32,
    pub status: String, // "pending" | "in_progress" | "done"
}

/// Result of `get_daily_challenge`. `challenge` is `None` when the learner
/// has no candidate today (empty 0.3–0.7 BKT zone, or every candidate was
/// excluded by the recency penalty).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDailyChallengeResult {
    pub challenge: Option<DailyChallengePayload>,
}

/// Wave 0 stub. Plan 03 wires this into `tauri::generate_handler!` and
/// implements the body (cache-first via `get_daily_challenge_for_date`,
/// fall through to `select_daily_challenge` + `insert_daily_challenge`).
pub async fn get_daily_challenge(
    _request: GetDailyChallengeRequest,
) -> Result<GetDailyChallengeResult, String> {
    unimplemented!("Plan 03 registers + implements this command")
}

// ── start_daily_challenge ──

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDailyChallengeRequest {
    pub challenge_date: String,
}

/// Idempotent — re-mounting `/daily/today` must not reset `started_at`.
/// Plan 03 calls `mark_daily_challenge_started`.
pub async fn start_daily_challenge(
    _request: StartDailyChallengeRequest,
) -> Result<(), String> {
    unimplemented!("Plan 03 registers + implements this command")
}

// ── complete_daily_challenge ──

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteDailyChallengeRequest {
    pub challenge_date: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteDailyChallengeResult {
    pub new_streak_days: i32,
    pub completed_at: String,
}

/// Marks today's challenge complete AND bumps the global streak in one
/// transaction. Plan 03 chains `mark_daily_challenge_completed` →
/// `update_global_streak` so the frontend's optimistic update has a
/// canonical streak count to reconcile against (Pattern 3 rollback path).
pub async fn complete_daily_challenge(
    _request: CompleteDailyChallengeRequest,
) -> Result<CompleteDailyChallengeResult, String> {
    unimplemented!("Plan 03 registers + implements this command")
}

// ── is_daily_challenge_enabled ──

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsDailyChallengeEnabledRequest {}

/// Returns BOTH the enabled flag AND the global streak in one IPC so the
/// Dashboard mount only needs two round-trips total (this + the eventual
/// `get_daily_challenge`), satisfying Pitfall 6.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsDailyChallengeEnabledResult {
    pub enabled: bool,
    pub global_streak_days: i32,
}

/// Auto-enable gate (D-12): at least one `module_progress.mastery_level >= 0.7`
/// AND `learner_profiles.preferences_json.dailyChallengeEnabled != false`.
pub async fn is_daily_challenge_enabled(
    _request: IsDailyChallengeEnabledRequest,
) -> Result<IsDailyChallengeEnabledResult, String> {
    unimplemented!("Plan 03 registers + implements this command")
}
