//! Daily-challenge IPC handlers (Phase 4 Wave 2 GREEN).
//!
//! Four `#[tauri::command]` handlers wired into `tauri::generate_handler!`
//! in `lib.rs`. Each handler holds `state.db.lock()` exactly once for its
//! full body — no `.await` inside the lock guard.
//!
//! IPC contract (Q9 lock + FIX-02 + Phase 03.1-06 precedent):
//! - Every command takes a single `request: T` parameter (NOT `req`).
//! - Every request/result struct carries `#[serde(rename_all = "camelCase")]`.
//! - Request structs intentionally exist (even when empty) so forward-compat
//!   additions don't break the JS wire format.
//!
//! Server-side resolution (T-04-07 + T-04-09 mitigation):
//! - `learner_id` is resolved from `learner_profiles ORDER BY created_at ASC
//!   LIMIT 1` — Phase 4 is single-learner desktop. Never trust a client-
//!   supplied learner_id. Multi-learner support is Phase 10+.
//! - `challenge_date` is resolved via SQL `date('now')` — the same UTC clock
//!   that `update_global_streak` uses for its `date(last_activity_date) =
//!   date('now')` comparison (Pitfall 7). Client cannot inject a date.
//!
//! Testability — every handler delegates to a `*_inner(&Connection)` helper.
//! The inner fns are exercised by `#[cfg(test)]` integration tests below
//! that build the same schema the production DB uses (via
//! `db::migrations::apply_migrations`). The `#[tauri::command]` async fns
//! are just thin wrappers that hold the lock and call the inner fn.

use crate::db::microlearning::{
    get_daily_challenge_for_date, get_learner_streak, insert_daily_challenge,
    mark_daily_challenge_completed, mark_daily_challenge_started, update_global_streak,
    DailyChallengeRow,
};
use crate::storage_impl::microlearning::SqliteMicrolearningStore;
use crate::AppState;
use chrono::Utc;
use learnforge_core::microlearning::select_daily_challenge as core_select_daily_challenge;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

// ── get_daily_challenge ──

/// Empty request envelope — kept as a struct (not `()`) so the IPC layer can
/// grow optional fields (e.g., timezone hint) without breaking the JS call
/// site.
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

/// `get_daily_challenge` IPC handler.
///
/// Body (cache-first, idempotent within a day per Pitfall 3):
/// 1. Resolve learner_id + challenge_date.
/// 2. Look up the daily_challenges row for (learner, today).
/// 3. If present — derive status from (started_at, completed_at), return.
/// 4. If absent — run `select_daily_challenge`. If `None`, return
///    `{ challenge: None }` (empty-zone). If `Some`, INSERT a new row and
///    return with status="pending".
#[tauri::command]
pub async fn get_daily_challenge(
    request: GetDailyChallengeRequest,
    state: State<'_, AppState>,
) -> Result<GetDailyChallengeResult, String> {
    // request is unit-like; bind to _ to silence unused-variable.
    let _ = request;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    get_daily_challenge_inner(&db.conn)
}

fn get_daily_challenge_inner(conn: &Connection) -> Result<GetDailyChallengeResult, String> {
    let learner_id = resolve_learner_id(conn)?;
    let challenge_date = resolve_today(conn)?;

    if let Some(row) = get_daily_challenge_for_date(conn, &learner_id, &challenge_date)? {
        let status = status_for_row(row.started_at.as_deref(), row.completed_at.as_deref());
        return Ok(GetDailyChallengeResult {
            challenge: Some(DailyChallengePayload {
                est_minutes: est_minutes_for(&row.block_type),
                block_id: row.block_id,
                block_type: row.block_type,
                module_id: row.module_id,
                track_id: row.track_id,
                status: status.to_string(),
            }),
        });
    }

    // No row yet — run selection algorithm.
    let candidate = {
        let store = SqliteMicrolearningStore(conn);
        match core_select_daily_challenge(&store, &learner_id, Utc::now())
            .map_err(|e| e.to_string())?
        {
            Some(c) => c,
            None => return Ok(GetDailyChallengeResult { challenge: None }),
        }
    };

    let row = DailyChallengeRow {
        learner_id: learner_id.clone(),
        challenge_date: challenge_date.clone(),
        block_id: candidate.block_id.clone(),
        module_id: candidate.module_id.clone(),
        track_id: candidate.track_id.clone(),
        block_type: candidate.block_type.clone(),
        started_at: None,
        completed_at: None,
    };
    insert_daily_challenge(conn, &row)?;

    Ok(GetDailyChallengeResult {
        challenge: Some(DailyChallengePayload {
            est_minutes: est_minutes_for(&candidate.block_type),
            block_id: candidate.block_id,
            block_type: candidate.block_type,
            module_id: candidate.module_id,
            track_id: candidate.track_id,
            status: "pending".to_string(),
        }),
    })
}

// ── start_daily_challenge ──

/// Empty request envelope — date is derived server-side per Pitfall 7.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDailyChallengeRequest {}

/// `start_daily_challenge` IPC handler. Idempotent per COALESCE — second
/// call preserves the FIRST timestamp. Errors if there is no row for today
/// (caller must have called `get_daily_challenge` first so the selection
/// step ran).
#[tauri::command]
pub async fn start_daily_challenge(
    request: StartDailyChallengeRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _ = request;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    start_daily_challenge_inner(&db.conn)
}

fn start_daily_challenge_inner(conn: &Connection) -> Result<(), String> {
    let learner_id = resolve_learner_id(conn)?;
    let challenge_date = resolve_today(conn)?;

    // Verify a row exists for today (T-04-08 — frontend cannot bypass selection).
    let exists = get_daily_challenge_for_date(conn, &learner_id, &challenge_date)?.is_some();
    if !exists {
        return Err(
            "start_daily_challenge: no challenge selected for today — call get_daily_challenge first"
                .to_string(),
        );
    }

    mark_daily_challenge_started(conn, &learner_id, &challenge_date)?;
    Ok(())
}

// ── complete_daily_challenge ──

/// Empty request envelope — date is derived server-side per Pitfall 7.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteDailyChallengeRequest {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteDailyChallengeResult {
    pub new_streak_days: i32,
    pub completed_at: String,
}

/// `complete_daily_challenge` IPC handler. Marks today's challenge complete
/// AND bumps the global streak in one locked operation. Idempotent within a
/// 24h window — Pitfall 2 (same-day completion returns same `completed_at`
/// and same `new_streak_days`).
#[tauri::command]
pub async fn complete_daily_challenge(
    request: CompleteDailyChallengeRequest,
    state: State<'_, AppState>,
) -> Result<CompleteDailyChallengeResult, String> {
    let _ = request;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    complete_daily_challenge_inner(&db.conn)
}

fn complete_daily_challenge_inner(
    conn: &Connection,
) -> Result<CompleteDailyChallengeResult, String> {
    let learner_id = resolve_learner_id(conn)?;
    let challenge_date = resolve_today(conn)?;

    // Verify a row exists for today (T-04-08 — frontend cannot bypass selection).
    let exists = get_daily_challenge_for_date(conn, &learner_id, &challenge_date)?.is_some();
    if !exists {
        return Err(
            "complete_daily_challenge: no challenge selected for today — call get_daily_challenge first"
                .to_string(),
        );
    }

    let completed_at = mark_daily_challenge_completed(conn, &learner_id, &challenge_date)?;
    let new_streak_days = update_global_streak(conn, &learner_id)?;

    Ok(CompleteDailyChallengeResult {
        new_streak_days,
        completed_at,
    })
}

// ── is_daily_challenge_enabled ──

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsDailyChallengeEnabledRequest {}

/// Returns BOTH the enabled flag AND the global streak in one IPC so the
/// Dashboard mount only needs two round-trips total (this +
/// `get_daily_challenge`) — Pitfall 6.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsDailyChallengeEnabledResult {
    pub enabled: bool,
    pub global_streak_days: i32,
}

/// `is_daily_challenge_enabled` IPC handler. Auto-enable gate (D-12) +
/// user opt-out (A5):
/// - `enabled = gate_satisfied && !user_optout`
/// - `gate_satisfied = EXISTS(mp WHERE mastery_level >= 0.7)`
/// - `user_optout = preferences_json.dailyChallengeEnabled === false`
#[tauri::command]
pub async fn is_daily_challenge_enabled(
    request: IsDailyChallengeEnabledRequest,
    state: State<'_, AppState>,
) -> Result<IsDailyChallengeEnabledResult, String> {
    let _ = request;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    is_daily_challenge_enabled_inner(&db.conn)
}

fn is_daily_challenge_enabled_inner(
    conn: &Connection,
) -> Result<IsDailyChallengeEnabledResult, String> {
    let learner_id = resolve_learner_id(conn)?;

    // D-12 — auto-enable gate: any module with mastery >= 0.7.
    let gate_satisfied: bool = conn
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM module_progress
                  WHERE learner_id = ?1 AND mastery_level >= 0.7
             )",
            params![&learner_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n != 0)
        .map_err(|e| format!("is_daily_challenge_enabled: gate check: {}", e))?;

    // User opt-out — preferences_json.dailyChallengeEnabled === false explicitly.
    // WR-02 — evaluate the opt-out structurally via SQLite's json_extract
    // in the same query that reads the row. COALESCE(..., 1) maps the
    // missing-key case (NULL) to "enabled" (D-12 default = opted-in once
    // the gate fires). The final `= 0` selects the explicit false case.
    // Folding this into one query also removes the previous two-step
    // read-then-parse where a substring scan could be fooled by bytes
    // inside a sibling string value.
    let user_optout: bool = conn
        .query_row(
            "SELECT COALESCE(json_extract(preferences_json, '$.dailyChallengeEnabled'), 1) = 0
               FROM learner_profiles WHERE id = ?1",
            params![&learner_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| format!("is_daily_challenge_enabled: read prefs: {}", e))?
        .map(|n| n != 0)
        .unwrap_or(false);

    let enabled = gate_satisfied && !user_optout;

    let streak = get_learner_streak(conn, &learner_id)?;

    Ok(IsDailyChallengeEnabledResult {
        enabled,
        global_streak_days: streak.streak_days,
    })
}

// ── set_daily_challenge_enabled (Wave 5 — Settings opt-out, D-13) ──

/// Request envelope — the boolean the toggle resolves to. Note the camelCase
/// serde: the JS wrapper sends `{ enabled: bool }`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDailyChallengeEnabledRequest {
    pub enabled: bool,
}

/// `set_daily_challenge_enabled` IPC handler — writes
/// `learner_profiles.preferences_json.dailyChallengeEnabled = <bool>` so the
/// existing `is_daily_challenge_enabled_inner` reader (Wave 2) sees the
/// updated opt-out state on the next Dashboard mount. The streak is NOT
/// touched — toggling off and back on preserves `learner_streaks.streak_days`
/// (T-04-17 / R4 must_have: "Existing streak preserved when re-enabled").
#[tauri::command]
pub async fn set_daily_challenge_enabled(
    request: SetDailyChallengeEnabledRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    set_daily_challenge_enabled_inner(&db.conn, request.enabled)
}

fn set_daily_challenge_enabled_inner(conn: &Connection, enabled: bool) -> Result<(), String> {
    let learner_id = resolve_learner_id(conn)?;

    // Use SQLite json_set to upsert the key in place. `json(?1)` wraps the
    // literal "true"/"false" token so SQLite stores a JSON boolean (not a
    // string) — that's what `json_extract($.dailyChallengeEnabled)` will
    // compare against in is_daily_challenge_enabled_inner's reader (and the
    // stringy parser at line 372 which looks for the literal `false` token).
    //
    // The literal is hard-coded to "true"/"false" — never interpolated from
    // untrusted input — so T-04-16 (prefs_json injection) is mitigated by
    // construction. The only parameter is learner_id which is server-resolved.
    let token = if enabled { "true" } else { "false" };
    conn.execute(
        "UPDATE learner_profiles
            SET preferences_json = json_set(
                    COALESCE(preferences_json, '{}'),
                    '$.dailyChallengeEnabled',
                    json(?1)
                ),
                updated_at = datetime('now')
          WHERE id = ?2",
        params![token, &learner_id],
    )
    .map_err(|e| format!("set_daily_challenge_enabled: {}", e))?;

    Ok(())
}

// ── Internal helpers ──

/// Resolve the single learner_id for the desktop app (T-04-09 mitigation —
/// learner_id NEVER comes from the request).
fn resolve_learner_id(conn: &Connection) -> Result<String, String> {
    conn.query_row(
        "SELECT id FROM learner_profiles ORDER BY created_at ASC LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .map_err(|e| format!("microlearning: resolve learner: {}", e))
}

/// Resolve today's date server-side via SQL (T-04-07 + Pitfall 7 — same
/// UTC clock used by `update_global_streak`'s `date('now')` calls).
fn resolve_today(conn: &Connection) -> Result<String, String> {
    conn.query_row("SELECT date('now')", [], |row| row.get::<_, String>(0))
        .map_err(|e| format!("microlearning: resolve date: {}", e))
}

/// Engagement-state machine — derived from (started_at, completed_at).
/// NOT the `BlockStatus` enum (R1).
fn status_for_row(started_at: Option<&str>, completed_at: Option<&str>) -> &'static str {
    match (started_at, completed_at) {
        (_, Some(_)) => "done",
        (Some(_), None) => "in_progress",
        (None, None) => "pending",
    }
}

/// D-01 sizing — same block, same minutes. Defensive default = 5.
fn est_minutes_for(block_type: &str) -> i32 {
    match block_type {
        "flash_cards" => 3,
        "quiz" => 5,
        "section" => 5,
        _ => 5,
    }
}

/// Parse `learner_profiles.preferences_json` and return `true` when the
/// learner has EXPLICITLY opted out
/// (`{ "dailyChallengeEnabled": false }`). Any other value (missing key,
/// `true`, non-bool, malformed JSON) returns `false` (default = opted-in
/// once the gate fires).
///
/// WR-02 — uses SQLite's `json_extract` (structural JSON parser) to
/// evaluate `$.dailyChallengeEnabled` rather than a hand-rolled byte-level
/// substring scan. The substring scan could be tricked by malformed or
/// hand-edited prefs_json whose byte content contained the literal token
/// `"dailyChallengeEnabled":false` inside a string value at a different
/// path. `json_extract` only returns the value at the top-level key, so
/// no string-value content can ever spoof it.
///
/// Production code paths fold this check into the same query that reads
/// the row (see `is_daily_challenge_enabled_inner`). This standalone
/// helper preserves the unit-testable `&str -> bool` surface that the
/// `prefs_parser_*` tests exercise and uses identical SQL semantics, so
/// the contract between the two is locked.
fn prefs_dailychallenge_disabled(prefs_json: &str) -> bool {
    // Evaluate `json_extract(?, '$.dailyChallengeEnabled')` against an
    // ephemeral in-memory connection. `json_extract` returns:
    //   * 0  if the value is JSON `false`
    //   * 1  if the value is JSON `true`
    //   * other integer/text for other types
    //   * NULL for missing key OR malformed input
    // Default = opted-in (NULL/missing/non-zero -> not disabled).
    let conn = match Connection::open_in_memory() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let v: Option<i64> = conn
        .query_row(
            "SELECT json_extract(?1, '$.dailyChallengeEnabled')",
            params![prefs_json],
            |row| row.get(0),
        )
        .optional()
        .unwrap_or(None);
    matches!(v, Some(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(crate::db::schema::CREATE_TABLES).unwrap();
        crate::db::migrations::apply_migrations(&conn).unwrap();
        conn
    }

    /// Seed `learner_profiles -> learning_tracks (active) -> learning_paths
    /// -> modules -> module_blocks (ready/section)` with deterministic IDs.
    /// Returns `(learner_id, track_id, module_id, block_id)`.
    fn seed_full_stack(conn: &Connection) -> (String, String, String, String) {
        let learner_id = "learner-1".to_string();
        let track_id = "trk-1".to_string();
        let path_id = "pth-1".to_string();
        let module_id = "mod-1".to_string();
        let block_id = "blk-1".to_string();

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES (?1, 'Tester')",
            params![&learner_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal, status)
             VALUES (?1, ?2, 'Rust', 'programming', 'Learn Rust', 'active')",
            params![&track_id, &learner_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id) VALUES (?1, ?2)",
            params![&path_id, &track_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES (?1, ?2, 'Module 1')",
            params![&module_id, &path_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status)
             VALUES (?1, ?2, 0, 'section', 'ready')",
            params![&block_id, &module_id],
        )
        .unwrap();

        (learner_id, track_id, module_id, block_id)
    }

    fn insert_mp(conn: &Connection, learner_id: &str, module_id: &str, mastery: f64) {
        conn.execute(
            "INSERT INTO module_progress
                (id, module_id, learner_id, status, mastery_level)
             VALUES (?1, ?2, ?3, 'in_progress', ?4)",
            params![
                uuid::Uuid::new_v4().to_string(),
                module_id,
                learner_id,
                mastery,
            ],
        )
        .unwrap();
    }

    // ── get_daily_challenge ──

    /// Empty-zone — no module_progress row in [0.3, 0.7). Result.challenge = None.
    #[test]
    fn get_daily_challenge_empty_zone_returns_null() {
        let conn = fresh_conn();
        let (_learner, _track, _module, _block) = seed_full_stack(&conn);
        // NO module_progress row — fetch_candidate_modules sees nothing.

        let r = get_daily_challenge_inner(&conn).expect("ok");
        assert!(
            r.challenge.is_none(),
            "empty 0.3-0.7 zone must yield challenge: None"
        );
    }

    /// Pitfall 3 — first call selects + INSERTs; second call returns SAME row.
    #[test]
    fn get_daily_challenge_persists_selection() {
        let conn = fresh_conn();
        let (learner_id, _track, module_id, block_id) = seed_full_stack(&conn);
        insert_mp(&conn, &learner_id, &module_id, 0.5);

        // First call: selection runs, row inserted
        let r1 = get_daily_challenge_inner(&conn).expect("ok");
        let c1 = r1.challenge.expect("must select a candidate");
        assert_eq!(c1.block_id, block_id);
        assert_eq!(c1.status, "pending");
        assert_eq!(c1.est_minutes, 5); // section → 5

        // Verify a row exists in daily_challenges
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM daily_challenges WHERE learner_id = ?1",
                params![&learner_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "INSERT must have happened on first call");

        // Second call: cache hit — SAME row, no re-selection
        let r2 = get_daily_challenge_inner(&conn).expect("ok");
        let c2 = r2.challenge.expect("must return persisted row");
        assert_eq!(c2.block_id, block_id);
        assert_eq!(c2.status, "pending");

        let count2: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM daily_challenges WHERE learner_id = ?1",
                params![&learner_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count2, 1, "second call must NOT insert a second row");
    }

    /// Engagement-state machine: pending → in_progress → done across three IPCs.
    #[test]
    fn get_daily_challenge_status_transitions() {
        let conn = fresh_conn();
        let (learner_id, _track, module_id, _block) = seed_full_stack(&conn);
        insert_mp(&conn, &learner_id, &module_id, 0.5);

        // 1. After get → pending
        let r = get_daily_challenge_inner(&conn).expect("ok");
        assert_eq!(r.challenge.expect("ok").status, "pending");

        // 2. After start → in_progress
        start_daily_challenge_inner(&conn).expect("ok");
        let r = get_daily_challenge_inner(&conn).expect("ok");
        assert_eq!(r.challenge.expect("ok").status, "in_progress");

        // 3. After complete → done
        complete_daily_challenge_inner(&conn).expect("ok");
        let r = get_daily_challenge_inner(&conn).expect("ok");
        assert_eq!(r.challenge.expect("ok").status, "done");
    }

    // ── start_daily_challenge ──

    /// Cannot start without a selected row (T-04-08 mitigation).
    #[test]
    fn start_daily_challenge_requires_existing_row() {
        let conn = fresh_conn();
        let (_learner, _track, _module, _block) = seed_full_stack(&conn);
        // NO get_daily_challenge call — no row exists.

        let err = start_daily_challenge_inner(&conn).expect_err("must error");
        assert!(
            err.contains("no challenge selected for today"),
            "error must mention missing selection (got: {})",
            err
        );
    }

    /// Idempotent — second start preserves the FIRST started_at (COALESCE).
    #[test]
    fn start_daily_challenge_idempotent() {
        let conn = fresh_conn();
        let (learner_id, _track, module_id, _block) = seed_full_stack(&conn);
        insert_mp(&conn, &learner_id, &module_id, 0.5);

        // Bootstrap a row
        get_daily_challenge_inner(&conn).expect("ok");

        start_daily_challenge_inner(&conn).expect("first start ok");
        let first_ts: String = conn
            .query_row(
                "SELECT started_at FROM daily_challenges WHERE learner_id = ?1",
                params![&learner_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!first_ts.is_empty(), "started_at must be set");

        start_daily_challenge_inner(&conn).expect("second start ok");
        let second_ts: String = conn
            .query_row(
                "SELECT started_at FROM daily_challenges WHERE learner_id = ?1",
                params![&learner_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            first_ts, second_ts,
            "second start_daily_challenge must preserve the FIRST started_at (COALESCE idempotency)"
        );
    }

    // ── complete_daily_challenge ──

    /// First completion sets streak=1; completion the next day increments to 2.
    #[test]
    fn complete_daily_challenge_increments_streak() {
        let conn = fresh_conn();
        let (learner_id, _track, module_id, _block) = seed_full_stack(&conn);
        insert_mp(&conn, &learner_id, &module_id, 0.5);

        // Bootstrap and complete today
        get_daily_challenge_inner(&conn).expect("ok");
        let r1 = complete_daily_challenge_inner(&conn).expect("first complete ok");
        assert_eq!(r1.new_streak_days, 1, "first activity must yield streak=1");
        assert!(!r1.completed_at.is_empty());

        // Backdate last_activity_date to yesterday-ish so a fresh complete
        // hits Branch 3 (within 24h, different calendar day).
        conn.execute(
            "UPDATE learner_streaks
                SET last_activity_date = datetime('now', '-23 hours')
              WHERE learner_id = ?1",
            params![&learner_id],
        )
        .unwrap();
        // Backdate today's daily_challenges so resolve_today() returns a row
        // for a "previous day" — but resolve_today always returns date('now'),
        // so to simulate the second day we need to NULL completed_at and use
        // a fresh selection. Easier path: directly call update_global_streak
        // (which Branch-3 increments) via the IPC handler by clearing
        // completed_at for today's row so mark_daily_challenge_completed
        // sets a NEW timestamp (and update_global_streak runs again).
        conn.execute(
            "UPDATE daily_challenges SET completed_at = NULL WHERE learner_id = ?1",
            params![&learner_id],
        )
        .unwrap();
        // Whether Branch 3 vs Branch 2 fires depends on whether
        // datetime('now', '-23 hours') is on a different calendar day than
        // date('now'). Mirror the v003 within_24h test's guard.
        let is_diff_day: bool = conn
            .query_row(
                "SELECT date(datetime('now', '-23 hours')) != date('now')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        let r2 = complete_daily_challenge_inner(&conn).expect("second complete ok");
        if is_diff_day {
            assert_eq!(
                r2.new_streak_days, 2,
                "within-24h different-day completion must increment streak 1→2"
            );
        } else {
            assert_eq!(
                r2.new_streak_days, 1,
                "same calendar day completion must keep streak at 1 (no-op)"
            );
        }
    }

    /// Pitfall 2 — same-day complete twice: same completed_at, same streak.
    #[test]
    fn complete_daily_challenge_same_day_idempotent() {
        let conn = fresh_conn();
        let (learner_id, _track, module_id, _block) = seed_full_stack(&conn);
        insert_mp(&conn, &learner_id, &module_id, 0.5);

        get_daily_challenge_inner(&conn).expect("ok");

        let r1 = complete_daily_challenge_inner(&conn).expect("first complete ok");
        let r2 = complete_daily_challenge_inner(&conn).expect("second complete ok");
        assert_eq!(
            r1.completed_at, r2.completed_at,
            "second complete must return SAME completed_at (no double-bump)"
        );
        assert_eq!(
            r1.new_streak_days, r2.new_streak_days,
            "second complete must return SAME streak count (same-day no-op)"
        );
    }

    // ── is_daily_challenge_enabled ──

    /// Auto-enable gate not satisfied — no module with mastery >= 0.7.
    #[test]
    fn is_daily_challenge_enabled_false_until_gate() {
        let conn = fresh_conn();
        let (_learner, _track, _module, _block) = seed_full_stack(&conn);
        // NO module_progress row at all → gate not satisfied

        let r = is_daily_challenge_enabled_inner(&conn).expect("ok");
        assert!(!r.enabled, "fresh learner has no mastered modules → disabled");
        assert_eq!(r.global_streak_days, 0, "fresh learner has streak=0");
    }

    /// Gate fires once a single module has mastery >= 0.7.
    #[test]
    fn is_daily_challenge_enabled_true_after_mastery() {
        let conn = fresh_conn();
        let (learner_id, _track, module_id, _block) = seed_full_stack(&conn);
        insert_mp(&conn, &learner_id, &module_id, 0.8);

        let r = is_daily_challenge_enabled_inner(&conn).expect("ok");
        assert!(r.enabled, "mastered module must trip the auto-enable gate");
        assert_eq!(r.global_streak_days, 0);
    }

    /// User opt-out wins even when the gate is satisfied (A5).
    #[test]
    fn is_daily_challenge_enabled_respects_opt_out() {
        let conn = fresh_conn();
        let (learner_id, _track, module_id, _block) = seed_full_stack(&conn);
        insert_mp(&conn, &learner_id, &module_id, 0.8);

        conn.execute(
            "UPDATE learner_profiles
                SET preferences_json = '{\"dailyChallengeEnabled\":false}'
              WHERE id = ?1",
            params![&learner_id],
        )
        .unwrap();

        let r = is_daily_challenge_enabled_inner(&conn).expect("ok");
        assert!(
            !r.enabled,
            "explicit opt-out (dailyChallengeEnabled=false) overrides the mastery gate"
        );
    }

    // ── unit tests for the prefs parser ──

    #[test]
    fn prefs_parser_treats_missing_key_as_opted_in() {
        assert!(!prefs_dailychallenge_disabled("{}"));
        assert!(!prefs_dailychallenge_disabled("{\"otherKey\": true}"));
    }

    #[test]
    fn prefs_parser_recognises_explicit_false() {
        assert!(prefs_dailychallenge_disabled(
            "{\"dailyChallengeEnabled\":false}"
        ));
        assert!(prefs_dailychallenge_disabled(
            "{\"dailyChallengeEnabled\": false}"
        ));
        assert!(prefs_dailychallenge_disabled(
            "{\"foo\":1, \"dailyChallengeEnabled\":false}"
        ));
    }

    #[test]
    fn prefs_parser_true_is_opted_in() {
        assert!(!prefs_dailychallenge_disabled(
            "{\"dailyChallengeEnabled\":true}"
        ));
    }

    /// WR-02 regression — substring parser false-positive on bytes that
    /// look like the key-value pair but aren't structurally at the
    /// top level.
    ///
    /// The hand-rolled parser scans raw bytes for the substring
    /// `"dailyChallengeEnabled"` followed by `:` + `false`. JSON allows
    /// arbitrary content inside string values, including (with proper
    /// escaping) sequences that the byte-level scan can be tricked by:
    ///
    /// 1. A nested JSON-string preference whose content embeds a literal
    ///    `"dailyChallengeEnabled":false` pair (e.g. the `notes` field
    ///    captured pasted JSON during onboarding). The structural parser
    ///    sees `notes = "...":false..."` — a single string value — but
    ///    the substring scan sees the literal token.
    /// 2. A second top-level key whose top-level value is `true` AND
    ///    appears BEFORE a sibling string field that embeds the same
    ///    substring. The substring scan walks past the legitimate
    ///    `dailyChallengeEnabled":true` and re-matches inside the string,
    ///    incorrectly flagging opt-out.
    ///
    /// The fix uses SQLite's `json_extract` — a structural JSON parser —
    /// to evaluate `$.dailyChallengeEnabled` directly, so only the
    /// top-level key counts.
    #[test]
    fn prefs_disabled_check_ignores_substring_in_other_fields() {
        // Scenario 1: notes contains the literal substring WITHOUT a
        // backslash before the quote — i.e. the JSON is technically
        // malformed (unescaped quote inside a string), which is exactly
        // what a hand-edited DB or a future preference writer could land.
        // The structural parser would reject the whole document; the
        // substring scanner would match and falsely report opt-out.
        let prefs = "{\"otherKey\":\"see \"dailyChallengeEnabled\":false in the value\"}";
        assert!(
            !prefs_dailychallenge_disabled(prefs),
            "byte-level substring match inside a malformed/embedded string must not count as opt-out (got disabled=true for: {})",
            prefs
        );

        // Scenario 2: top-level dailyChallengeEnabled is explicitly true,
        // but a later field embeds the false-substring in its value
        // (again, technically malformed but byte-scannable). The fix
        // must honor the top-level true.
        let prefs2 = "{\"dailyChallengeEnabled\":true,\"notes\":\"legacy doc said \"dailyChallengeEnabled\":false here\"}";
        assert!(
            !prefs_dailychallenge_disabled(prefs2),
            "explicit top-level true must win over a substring-in-string match (got disabled=true for: {})",
            prefs2
        );
    }

    // ── set_daily_challenge_enabled (Wave 5 — Settings opt-out) ──

    /// Writing `enabled=false` persists `{"dailyChallengeEnabled":false}` into
    /// preferences_json so that `is_daily_challenge_enabled_inner` honors the
    /// opt-out (A5 — the same key that the parser reads in Wave 2).
    #[test]
    fn set_daily_challenge_enabled_writes_false() {
        let conn = fresh_conn();
        let (_learner, _track, _module, _block) = seed_full_stack(&conn);

        set_daily_challenge_enabled_inner(&conn, false).expect("ok");

        let prefs: String = conn
            .query_row(
                "SELECT preferences_json FROM learner_profiles WHERE id = 'learner-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        // The key must serialize as the JSON boolean `false`. We read back
        // via json_extract -> integer (SQLite represents JSON bool as 0/1 in
        // the SQL column-type projection) and assert == 0.
        let extracted: i64 = conn
            .query_row(
                "SELECT json_extract(?1, '$.dailyChallengeEnabled')",
                params![&prefs],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(extracted, 0, "dailyChallengeEnabled must be 0 / false (got: {})", prefs);
        // The Wave-2 parser must agree this is opted-out.
        assert!(prefs_dailychallenge_disabled(&prefs));
    }

    /// Writing `enabled=true` persists `{"dailyChallengeEnabled":true}` so the
    /// stringy parser (which only flags explicit `false`) yields opted-in.
    #[test]
    fn set_daily_challenge_enabled_writes_true() {
        let conn = fresh_conn();
        let (_learner, _track, _module, _block) = seed_full_stack(&conn);

        set_daily_challenge_enabled_inner(&conn, true).expect("ok");

        let prefs: String = conn
            .query_row(
                "SELECT preferences_json FROM learner_profiles WHERE id = 'learner-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let extracted: i64 = conn
            .query_row(
                "SELECT json_extract(?1, '$.dailyChallengeEnabled')",
                params![&prefs],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(extracted, 1, "dailyChallengeEnabled must be 1 / true (got: {})", prefs);
        // And critically, the Wave-2 parser must NOT flag this as opted-out.
        assert!(!prefs_dailychallenge_disabled(&prefs));
    }

    /// End-to-end: master a module (gate satisfied) → opt out via set IPC →
    /// `is_daily_challenge_enabled_inner` returns enabled=false. Locks the
    /// contract between the new IPC and Wave 2's existing reader.
    #[test]
    fn set_daily_challenge_enabled_then_is_daily_challenge_enabled_returns_false() {
        let conn = fresh_conn();
        let (learner_id, _track, module_id, _block) = seed_full_stack(&conn);
        insert_mp(&conn, &learner_id, &module_id, 0.8);

        // Pre-condition: gate is satisfied.
        let r = is_daily_challenge_enabled_inner(&conn).expect("ok");
        assert!(r.enabled, "gate should fire after mastery (pre-condition)");

        // Opt out via the new IPC.
        set_daily_challenge_enabled_inner(&conn, false).expect("ok");

        let r = is_daily_challenge_enabled_inner(&conn).expect("ok");
        assert!(
            !r.enabled,
            "opt-out via set_daily_challenge_enabled must disable even when gate fires"
        );
    }
}
