//! Tests for `commands::labs::eval`. Lives in a sibling file to keep
//! `eval.rs` under the 500-line CLAUDE.md cap. Included via
//! `#[path = "eval_tests.rs"] #[cfg(test)] mod tests;` from `eval.rs`.

use super::*;
use crate::labs::evaluator::EvalOutcome;

/// Apply the persist-side effects directly (Pass / Fail / Indeterminate /
/// Manual) without spinning up the IPC handler — covers the SQL paths the
/// handler delegates to.
fn fresh_conn() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(crate::db::schema::CREATE_TABLES).unwrap();
    crate::db::migrations::apply_migrations(&conn).unwrap();
    conn
}

fn seed(conn: &rusqlite::Connection) -> (String, String, String) {
    let learner = "lp-1".to_string();
    let track = "track-1".to_string();
    let path = "path-1".to_string();
    let module = "mod-1".to_string();
    let block = "blk-1".to_string();
    conn.execute(
        "INSERT INTO learner_profiles (id, display_name) VALUES (?1, 'L')",
        rusqlite::params![learner],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO learning_tracks (id, learner_id, topic, domain_module)
         VALUES (?1, ?2, 'k8s', 'kubernetes')",
        rusqlite::params![track, learner],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO learning_paths (id, track_id) VALUES (?1, ?2)",
        rusqlite::params![path, track],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO modules (id, path_id, title, ordering)
         VALUES (?1, ?2, 'M1', 0)",
        rusqlite::params![module, path],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
            params_json, payload_json, source_anchors_json, metadata_json, retry_count,
            created_at, updated_at)
         VALUES (?1, ?2, 0, 'lab', 'ready', '{}', '{}', '[]', '{}', 0,
            datetime('now'), datetime('now'))",
        rusqlite::params![block, module],
    )
    .unwrap();
    (learner, module, block)
}

/// Direct exercise of the SQL paths in `persist_outcome`. Replicates the
/// handler's SQL (without the lock dance) so we can assert on the row
/// state for each outcome variant.
fn apply_outcome_to_db(
    conn: &rusqlite::Connection,
    learner: &str,
    module: &str,
    block: &str,
    step_id: &str,
    total_steps: usize,
    outcome: &EvalOutcome,
    check_kind: &str,
    step_index: usize,
    reason: &str,
) {
    conn.execute(
        "INSERT OR IGNORE INTO lab_progress
            (learner_id, module_id, block_id, current_step, completed_step_ids,
             total_steps, metadata_json, last_updated)
         VALUES (?1, ?2, ?3, 0, '[]', ?4, '{}', datetime('now'))",
        rusqlite::params![learner, module, block, total_steps as i64],
    )
    .unwrap();
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
            "at": "2026-05-06T00:00:00Z",
        })
        .to_string();
        // WR-06 — exercise the PRODUCTION persistence helper (keyed
        // per-step slot + legacy last_ai_judge) instead of replicating
        // its SQL here.
        persist_ai_judge_verdict(conn, learner, module, block, step_index, &verdict).unwrap();
    }
    // CR-02 — mirrors persist_outcome's idempotent Pass update (json_each
    // guard against duplicate completed_step_ids entries).
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
            rusqlite::params![step_id, learner, module, block],
        )
        .unwrap();
    }
    crate::commands::labs::state::recompute_practical_mastery(conn, module, learner)
        .unwrap();
}

/// LAB-06 — Pass updates lab_progress + recomputes practical_mastery.
#[test]
fn lab_check_step_pass_updates_progress_and_mastery() {
    let conn = fresh_conn();
    let (learner, module, block) = seed(&conn);
    // Seed module_progress so recompute can persist mastery.
    conn.execute(
        "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level,
            attempts, started_at, practical_mastery)
         VALUES ('mp-1', ?1, ?2, 'in_progress', 0.4, 1, datetime('now'), 0.0)",
        rusqlite::params![module, learner],
    )
    .unwrap();
    apply_outcome_to_db(
        &conn, &learner, &module, &block, "write-manifest",
        4, &EvalOutcome::Pass, "fileState", 0, "step passed",
    );
    let (current_step, completed): (i64, String) = conn
        .query_row(
            "SELECT current_step, completed_step_ids FROM lab_progress
             WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
            rusqlite::params![learner, module, block],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .unwrap();
    assert_eq!(current_step, 1);
    let v: serde_json::Value = serde_json::from_str(&completed).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0].as_str(), Some("write-manifest"));

    let mastery: f64 = conn
        .query_row(
            "SELECT practical_mastery FROM module_progress
             WHERE module_id = ?1 AND learner_id = ?2",
            rusqlite::params![module, learner],
            |r| r.get::<_, f64>(0),
        )
        .unwrap();
    assert!((mastery - 0.25).abs() < 1e-9, "1/4 = 0.25, got {}", mastery);
}

/// LAB-06 — Fail does NOT advance current_step or completed_step_ids.
#[test]
fn lab_check_step_fail_does_not_update() {
    let conn = fresh_conn();
    let (learner, module, block) = seed(&conn);
    apply_outcome_to_db(
        &conn, &learner, &module, &block, "step1",
        4, &EvalOutcome::Fail, "commandRegex", 0, "no match",
    );
    let (current_step, completed): (i64, String) = conn
        .query_row(
            "SELECT current_step, completed_step_ids FROM lab_progress
             WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
            rusqlite::params![learner, module, block],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .unwrap();
    assert_eq!(current_step, 0);
    assert_eq!(completed, "[]");
}

/// LAB-06 — Indeterminate (no exit code yet) keeps lab_progress at 0/N.
#[test]
fn lab_check_step_indeterminate_returns_indeterminate() {
    let conn = fresh_conn();
    let (learner, module, block) = seed(&conn);
    apply_outcome_to_db(
        &conn, &learner, &module, &block, "step1",
        4, &EvalOutcome::Indeterminate, "exitCode", 0, "no exit code observed yet",
    );
    let current_step: i64 = conn
        .query_row(
            "SELECT current_step FROM lab_progress
             WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
            rusqlite::params![learner, module, block],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(current_step, 0);
}

/// LAB-06 — Manual (ai_judge budget exhausted / no auth) keeps progress
/// intact AND persists the verdict in metadata_json.
#[test]
fn lab_check_step_manual_returns_manual() {
    let conn = fresh_conn();
    let (learner, module, block) = seed(&conn);
    apply_outcome_to_db(
        &conn, &learner, &module, &block, "step1",
        4, &EvalOutcome::Manual, "aiJudge", 0,
        "manual recheck required (AI-judge budget exhausted or no auth)",
    );
    let (current_step, metadata): (i64, String) = conn
        .query_row(
            "SELECT current_step, metadata_json FROM lab_progress
             WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
            rusqlite::params![learner, module, block],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .unwrap();
    assert_eq!(current_step, 0);
    let v: serde_json::Value = serde_json::from_str(&metadata).unwrap();
    assert_eq!(v["last_ai_judge"]["outcome"].as_str(), Some("manual"));
}

/// LAB-06 — ai_judge Pass persists `$.last_ai_judge.outcome = "pass"`
/// and the reason. Verifies the json_set / json() round-trip works
/// against SQLite's JSON1 functions.
#[test]
fn lab_check_step_ai_judge_persists_verdict_in_metadata_json() {
    let conn = fresh_conn();
    let (learner, module, block) = seed(&conn);
    conn.execute(
        "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level,
            attempts, started_at, practical_mastery)
         VALUES ('mp-1', ?1, ?2, 'in_progress', 0.4, 1, datetime('now'), 0.0)",
        rusqlite::params![module, learner],
    )
    .unwrap();
    apply_outcome_to_db(
        &conn, &learner, &module, &block, "explain-output",
        4, &EvalOutcome::Pass, "aiJudge", 2,
        "explanation matches expected criteria",
    );
    let metadata: String = conn
        .query_row(
            "SELECT metadata_json FROM lab_progress
             WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
            rusqlite::params![learner, module, block],
            |r| r.get::<_, String>(0),
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&metadata).unwrap();
    assert_eq!(v["last_ai_judge"]["outcome"].as_str(), Some("pass"));
    assert_eq!(v["last_ai_judge"]["step_index"].as_u64(), Some(2));
    assert!(v["last_ai_judge"]["reason"]
        .as_str()
        .unwrap_or("")
        .contains("matches"));
}

/// WR-06 — `persist_ai_judge_verdict` keeps one verdict slot PER STEP
/// (`metadata_json.$.ai_judge_verdicts."<idx>"`) so a later judge call on
/// another step never erases an earlier step's Manual/Indeterminate
/// verdict. The legacy `$.last_ai_judge` single slot is still written for
/// backward compat.
#[test]
fn persist_ai_judge_verdict_writes_keyed_per_step_slots() {
    let conn = fresh_conn();
    let (learner, module, block) = seed(&conn);
    conn.execute(
        "INSERT INTO lab_progress
            (learner_id, module_id, block_id, current_step, completed_step_ids,
             total_steps, metadata_json, last_updated)
         VALUES (?1, ?2, ?3, 0, '[]', 4, '{}', datetime('now'))",
        rusqlite::params![learner, module, block],
    )
    .unwrap();

    let manual = serde_json::json!({
        "step_index": 1, "outcome": "manual", "reason": "budget exhausted",
        "at": "2026-05-06T00:00:00Z",
    })
    .to_string();
    let fail = serde_json::json!({
        "step_index": 2, "outcome": "fail", "reason": "wrong explanation",
        "at": "2026-05-06T00:01:00Z",
    })
    .to_string();
    persist_ai_judge_verdict(&conn, &learner, &module, &block, 1, &manual).unwrap();
    persist_ai_judge_verdict(&conn, &learner, &module, &block, 2, &fail).unwrap();

    let metadata: String = conn
        .query_row(
            "SELECT metadata_json FROM lab_progress
             WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
            rusqlite::params![learner, module, block],
            |r| r.get::<_, String>(0),
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&metadata).unwrap();
    assert_eq!(
        v["ai_judge_verdicts"]["1"]["outcome"].as_str(),
        Some("manual"),
        "step 1's Manual verdict must survive step 2's later judge call, got {}",
        metadata
    );
    assert_eq!(
        v["ai_judge_verdicts"]["1"]["reason"].as_str(),
        Some("budget exhausted")
    );
    assert_eq!(v["ai_judge_verdicts"]["2"]["outcome"].as_str(), Some("fail"));
    // Legacy single slot still tracks the most recent verdict.
    assert_eq!(v["last_ai_judge"]["step_index"].as_u64(), Some(2));
    assert_eq!(v["last_ai_judge"]["outcome"].as_str(), Some("fail"));
}

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
    assert_eq!(
        check_kind_str(&StepCheck::CommandAbsent {
            pattern: "x".to_string(),
            match_stderr: false,
        }),
        "commandAbsent"
    );
}

// ── GAP-03 (Plan 03.1-09): AuthState plumbing into lab_check_step ──

/// GAP-03 — `lab_check_step` must read `AuthState` and pass
/// `ai_authenticated = auth_state.has_active_credential()` into the
/// `EvalContext` instead of hardcoded `false`. Closure: Task 4 introduces
/// an inner `lab_check_step_with(state, ai_authenticated, ...)` helper
/// that accepts the boolean directly; the Tauri handler resolves it from
/// `State<AuthState>`.
///
/// FAILS today (compile error: `lab_check_step_with` does not exist) until
/// Task 4 lands the inner helper.
#[tokio::test]
async fn lab_check_step_passes_authenticated_state_to_ai_judge() {
    use crate::labs::evaluator::{evaluate_step_with_judge, AiJudgeRunner, EvalContext, EvalOutcome};
    use crate::labs::spec::StepCheck;
    use std::pin::Pin;

    // Mock judge runner — returns a canned pass verdict so we can
    // observe whether the AI-judge branch was actually exercised.
    struct MockJudgeRunner {
        called: std::sync::atomic::AtomicBool,
    }
    impl AiJudgeRunner for MockJudgeRunner {
        fn run<'a>(
            &'a self,
            _prompt: &'a str,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>> {
            self.called.store(true, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async {
                Ok(r#"{"pass": true, "reason": "matches criteria"}"#.to_string())
            })
        }
    }

    let workspace = tempfile::tempdir().unwrap();
    let workspace_path = workspace.path().to_path_buf();
    let runner = MockJudgeRunner {
        called: std::sync::atomic::AtomicBool::new(false),
    };

    // Behavior 1: ai_authenticated=false (current production behavior) —
    // must short-circuit to Manual; runner NOT invoked.
    let ctx_no_auth = EvalContext {
        last_command: "kubectl get pods",
        last_output: "pod-1   Running",
        last_exit_code: Some(0),
        workspace: &workspace_path,
        ai_authenticated: false,
        ai_budget_remaining: 5,
        history: None,
    };
    let check = StepCheck::AiJudge {
        criteria: "Output shows pods".to_string(),
        threshold: 0.7,
    };
    let no_auth_outcome = evaluate_step_with_judge(&check, &ctx_no_auth, Some(&runner))
        .await
        .unwrap();
    assert!(
        matches!(no_auth_outcome, EvalOutcome::Manual),
        "no auth → Manual short-circuit (preserves existing behavior)"
    );
    assert!(
        !runner.called.load(std::sync::atomic::Ordering::SeqCst),
        "runner must NOT be invoked when ai_authenticated=false"
    );

    // Behavior 2: ai_authenticated=true — AI-judge path is exercised
    // and the runner IS invoked. Outcome is Pass (mock returns pass=true).
    let ctx_authed = EvalContext {
        last_command: "kubectl get pods",
        last_output: "pod-1   Running",
        last_exit_code: Some(0),
        workspace: &workspace_path,
        ai_authenticated: true,
        ai_budget_remaining: 5,
        history: None,
    };
    let authed_outcome = evaluate_step_with_judge(&check, &ctx_authed, Some(&runner))
        .await
        .unwrap();
    assert!(
        runner.called.load(std::sync::atomic::Ordering::SeqCst),
        "runner MUST be invoked when ai_authenticated=true — proves the auth bool flowed through"
    );
    assert!(
        matches!(authed_outcome, EvalOutcome::Pass),
        "with auth + mock returning pass, outcome must be Pass; got {:?}",
        authed_outcome
    );

    // Compile-time seam check: Task 4 introduces an inner
    // `lab_check_step_with(...)` helper that accepts `ai_authenticated`
    // directly so callers can exercise both branches without
    // standing up Tauri State. Referencing the symbol forces a compile
    // error until Task 4 lands (intended RED state).
    let _seam = super::LAB_CHECK_STEP_WITH_SEAM_MARKER;
}

// ── Phase 19.3 (D-01/D-04) — append-only lab_check_step + lab_validate_milestone ──
//
// These tests drive the full handler flow through `lab_check_step_with` /
// `lab_validate_milestone_with` taking `&AppState` (the test seam — Tauri
// `State` cannot be constructed in unit tests; mirrors the `..._conn`
// convention in exam.rs). RED until Task 3 lands the &AppState signature,
// the unconditional history append, the milestone skip, and the new
// `lab_validate_milestone_with` + `LabValidateMilestoneRequest` structs.

use crate::labs::LabSession;
use crate::labs::test_support::MockLabSession;
use crate::{AppState, LabSessionEntry};
use std::sync::Arc;

const STEP_GRAIN_LAB_MD: &str = r#"---
slug: step-grain-lab
title: Step grain lab
image: alpine
steps:
  - id: create-pod
    title: Create pod
    prompt: Create the pod and observe the output.
    check:
      kind: command_regex
      pattern: "pod/web created"
  - id: second-step
    title: Second step
    prompt: Run another command successfully.
    check:
      kind: exit_code
      expected: 0
---
Body.
"#;

const MILESTONE_LAB_MD: &str = r#"---
slug: milestone-lab
title: Milestone lab
image: alpine
grain: milestone
steps:
  - id: create-pod
    title: Create pod
    prompt: Create the pod and observe the output.
    check:
      kind: command_regex
      pattern: "pod/web created"
---
Body.
"#;

const MILESTONE_ABSENT_LAB_MD: &str = r#"---
slug: milestone-absent-lab
title: Milestone absent lab
image: alpine
grain: milestone
steps:
  - id: no-crash
    title: No crash loop
    prompt: Ensure nothing crash-looped during the session.
    check:
      kind: command_absent
      pattern: "CrashLoopBackOff"
---
Body.
"#;

/// Synthetic AppState with in-memory DB + empty lab_sessions registry
/// (mirrors session_tests::test_app_state — module-private there).
fn eval_test_app_state() -> Arc<AppState> {
    let conn = rusqlite::Connection::open_in_memory().expect("open_in_memory");
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(crate::db::schema::CREATE_TABLES).unwrap();
    crate::db::migrations::apply_migrations(&conn).unwrap();
    let db = crate::db::Database { conn };
    Arc::new(AppState {
        db: Arc::new(std::sync::Mutex::new(db)),
        lab_sessions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        topic_packs: Arc::new(std::sync::Mutex::new(
            learnforge_core::packs::PackRegistry::default(),
        )),
        signing_key: Arc::new(std::sync::Mutex::new(None)),
        signing_key_path: std::path::PathBuf::from("/tmp/learnforge-eval-tests-keys"),
    })
}

/// Seed learner/track/path/module + a lab block whose params_json.labMd is
/// `lab_md`. Returns (learner_id, module_id, block_id).
fn insert_lab_fixture_md(state: &AppState, lab_md: &str) -> (String, String, String) {
    let params_json = serde_json::json!({ "labMd": lab_md }).to_string();
    insert_lab_fixture(state, &params_json, "{}")
}

/// WR-01 — like `insert_lab_fixture_md` but stores a pre-serialized spec in
/// payload_json.spec (the PagePlanner-emitted shape) with NO labMd fallback,
/// so tests can drive the stored-spec validation path.
fn insert_lab_fixture_payload(
    state: &AppState,
    spec_json: serde_json::Value,
) -> (String, String, String) {
    let payload_json = serde_json::json!({ "spec": spec_json }).to_string();
    insert_lab_fixture(state, "{}", &payload_json)
}

fn insert_lab_fixture(
    state: &AppState,
    params_json: &str,
    payload_json: &str,
) -> (String, String, String) {
    let db = state.db.lock().unwrap();
    let conn = &db.conn;
    let (learner, track, path, module, block) =
        ("lp-1", "trk-1", "path-1", "mod-1", "blk-1");
    conn.execute(
        "INSERT INTO learner_profiles (id, display_name) VALUES (?1, 'L')",
        rusqlite::params![learner],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO learning_tracks (id, learner_id, topic, domain_module)
         VALUES (?1, ?2, 'k8s', 'kubernetes')",
        rusqlite::params![track, learner],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO learning_paths (id, track_id) VALUES (?1, ?2)",
        rusqlite::params![path, track],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO modules (id, path_id, title, ordering) VALUES (?1, ?2, 'M1', 0)",
        rusqlite::params![module, path],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level,
            attempts, started_at, practical_mastery)
         VALUES ('mp-1', ?1, ?2, 'in_progress', 0.4, 1, datetime('now'), 0.0)",
        rusqlite::params![module, learner],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
            params_json, payload_json, source_anchors_json, metadata_json, retry_count,
            created_at, updated_at)
         VALUES (?1, ?2, 0, 'lab', 'ready', ?3, ?4, '[]', '{}', 0,
            datetime('now'), datetime('now'))",
        rusqlite::params![block, module, params_json, payload_json],
    )
    .unwrap();
    (learner.to_string(), module.to_string(), block.to_string())
}

/// Insert a live session entry (MockLabSession) into the registry.
async fn insert_session(
    state: &AppState,
    session_id: &str,
    learner: &str,
    module: &str,
    block: &str,
    workspace: std::path::PathBuf,
    total_steps: usize,
) {
    let session: Box<dyn LabSession + Send> = Box::new(MockLabSession::default());
    let entry = LabSessionEntry {
        session,
        block_id: block.to_string(),
        learner_id: learner.to_string(),
        module_id: module.to_string(),
        workspace,
        total_steps,
        ai_budget_remaining: 5,
        command_history: Vec::new(),
    };
    let mut map = state.lab_sessions.lock().await;
    map.insert(session_id.to_string(), entry);
}

fn check_req(session_id: &str, step_index: usize, cmd: &str, output: &str, code: Option<i32>) -> LabCheckStepRequest {
    LabCheckStepRequest {
        session_id: session_id.to_string(),
        step_index,
        last_command: cmd.to_string(),
        last_output: output.to_string(),
        last_exit_code: code,
    }
}

fn progress_row(state: &AppState, learner: &str, module: &str, block: &str) -> Option<(i64, String)> {
    let db = state.db.lock().unwrap();
    use rusqlite::OptionalExtension;
    db.conn
        .query_row(
            "SELECT current_step, completed_step_ids FROM lab_progress
             WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
            rusqlite::params![learner, module, block],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()
        .unwrap()
}

/// D-01 — lab_check_step appends a CommandRecord to the session's
/// command_history on EVERY call (all grains), before any verdict logic.
#[tokio::test]
async fn lab_check_step_appends_history_every_call() {
    let state = eval_test_app_state();
    let (learner, module, block) = insert_lab_fixture_md(&state, STEP_GRAIN_LAB_MD);
    let ws = tempfile::tempdir().unwrap();
    insert_session(&state, "sess-hist", &learner, &module, &block, ws.path().to_path_buf(), 2).await;

    for i in 0..2 {
        let req = check_req("sess-hist", 0, &format!("cmd-{}", i), "no match here", Some(1));
        lab_check_step_with(req, &state, false)
            .await
            .expect("lab_check_step must succeed");
    }

    let map = state.lab_sessions.lock().await;
    let entry = map.get("sess-hist").unwrap();
    assert_eq!(entry.command_history.len(), 2, "every call must append");
    assert_eq!(entry.command_history[0].command, "cmd-0");
    assert_eq!(entry.command_history[1].command, "cmd-1");
    assert_eq!(entry.command_history[1].exit_code, Some(1));
}

/// D-04 — milestone-grain step: lab_check_step appends then returns WITHOUT
/// evaluating or persisting a verdict; completed_step_ids unchanged.
#[tokio::test]
async fn lab_check_step_milestone_skips_verdict() {
    let state = eval_test_app_state();
    let (learner, module, block) = insert_lab_fixture_md(&state, MILESTONE_LAB_MD);
    let ws = tempfile::tempdir().unwrap();
    insert_session(&state, "sess-ms", &learner, &module, &block, ws.path().to_path_buf(), 1).await;

    // Output that WOULD Pass at step grain — must still not evaluate.
    let req = check_req("sess-ms", 0, "kubectl apply -f pod.yaml", "pod/web created", Some(0));
    let result = lab_check_step_with(req, &state, false)
        .await
        .expect("lab_check_step must succeed");

    assert!(!result.passed, "milestone step must not report a Pass verdict on prompt-boundary calls");
    assert!(
        result.reason.to_lowercase().contains("validate"),
        "reason must direct the learner to Validate, got: {}",
        result.reason
    );
    // WR-03 — structural outcome, not prose-sniffing: the D-04 advisory
    // must NOT surface as "fail" in the UI.
    assert_eq!(result.outcome, "milestone_pending");

    // No progress advance persisted.
    match progress_row(&state, &learner, &module, &block) {
        None => {} // no row minted at all — fine
        Some((current_step, completed)) => {
            assert_eq!(current_step, 0, "milestone skip must not advance current_step");
            assert_eq!(completed, "[]", "milestone skip must not append completed_step_ids");
        }
    }

    // But history WAS appended.
    let map = state.lab_sessions.lock().await;
    assert_eq!(map.get("sess-ms").unwrap().command_history.len(), 1);
}

/// D-03 back-compat — step-grain step behaves byte-identically to today:
/// matching output evaluates Pass and persists the advance.
#[tokio::test]
async fn lab_check_step_step_grain_unchanged() {
    let state = eval_test_app_state();
    let (learner, module, block) = insert_lab_fixture_md(&state, STEP_GRAIN_LAB_MD);
    let ws = tempfile::tempdir().unwrap();
    insert_session(&state, "sess-step", &learner, &module, &block, ws.path().to_path_buf(), 2).await;

    let req = check_req("sess-step", 0, "kubectl apply -f pod.yaml", "pod/web created", Some(0));
    let result = lab_check_step_with(req, &state, false)
        .await
        .expect("lab_check_step must succeed");

    assert!(result.passed, "step-grain Pass path must be unchanged");
    let (current_step, completed) =
        progress_row(&state, &learner, &module, &block).expect("row must exist");
    assert_eq!(current_step, 1);
    assert!(completed.contains("create-pod"), "completed_step_ids must advance, got {}", completed);
}

/// D-04 — lab_validate_milestone evaluates the milestone step against the
/// session history and routes Pass through the SAME persist_outcome
/// (completed_step_ids advances identically).
#[tokio::test]
async fn lab_validate_milestone_routes_through_persist_outcome() {
    let state = eval_test_app_state();
    let (learner, module, block) = insert_lab_fixture_md(&state, MILESTONE_LAB_MD);
    let ws = tempfile::tempdir().unwrap();
    insert_session(&state, "sess-val", &learner, &module, &block, ws.path().to_path_buf(), 1).await;

    // Seed history through the append-only prompt-boundary path.
    let req = check_req("sess-val", 0, "kubectl apply -f pod.yaml", "pod/web created", Some(0));
    lab_check_step_with(req, &state, false).await.unwrap();

    let vreq = LabValidateMilestoneRequest {
        session_id: "sess-val".to_string(),
        step_index: 0,
    };
    let result = lab_validate_milestone_with(vreq, &state, false)
        .await
        .expect("lab_validate_milestone must succeed");

    assert!(result.passed, "history contains a matching record — must Pass");
    assert_eq!(result.check_kind, "commandRegex");
    // WR-03 — the outcome enum crosses the wire structurally.
    assert_eq!(result.outcome, "pass");
    let (current_step, completed) =
        progress_row(&state, &learner, &module, &block).expect("row must exist after persist");
    assert_eq!(current_step, 1, "Pass must advance current_step via persist_outcome");
    assert!(completed.contains("create-pod"), "got {}", completed);
}

/// 19.3-REVIEW WR-01 — the evaluation path's spec reader must run
/// `validate_spec` on DB-stored payload specs (delegating to
/// `read_lab_spec_conn`), so a stored spec violating the D-05 exam x
/// milestone exclusion never reaches milestone scoring. With no labMd
/// fallback, the invalid stored spec must surface as an error.
#[tokio::test]
async fn lab_validate_milestone_rejects_invalid_stored_payload_spec() {
    let state = eval_test_app_state();
    // exam: + milestone grain coexisting — validate_spec (D-05
    // validate_milestone_exam_exclusion) must reject this spec.
    let invalid_spec = serde_json::json!({
        "slug": "exam-milestone-lab",
        "title": "Exam x milestone (invalid)",
        "image": "alpine",
        "dockerfile": null,
        "requiresDocker": true,
        "creates": [],
        "exam": { "timeLimitMinutes": 10, "passThresholdPct": 70.0 },
        "grain": "milestone",
        "steps": [{
            "id": "s1",
            "title": "T",
            "prompt": "p",
            "check": { "kind": "command_regex", "pattern": "x" },
            "hints": [],
            "weight": 1.0,
            "grain": "step"
        }]
    });
    let (learner, module, block) = insert_lab_fixture_payload(&state, invalid_spec);
    let ws = tempfile::tempdir().unwrap();
    insert_session(&state, "sess-inv", &learner, &module, &block, ws.path().to_path_buf(), 1).await;

    let vreq = LabValidateMilestoneRequest {
        session_id: "sess-inv".to_string(),
        step_index: 0,
    };
    let err = lab_validate_milestone_with(vreq, &state, false)
        .await
        .expect_err("stored spec failing validate_spec must never reach milestone scoring");
    assert!(
        err.contains("no readable lab spec"),
        "invalid stored spec with no labMd fallback must error, got: {}",
        err
    );
}

/// 19.3-REVIEW CR-02 — `persist_outcome` must be idempotent on Pass: two
/// consecutive `lab_validate_milestone_with` Passes on the SAME step (the
/// milestone evidence is persistent session history, so a double-click
/// fires two passing calls) must leave current_step = 1, exactly ONE entry
/// in completed_step_ids, and practical_mastery <= 1.0.
#[tokio::test]
async fn lab_validate_milestone_repeated_pass_is_idempotent() {
    let state = eval_test_app_state();
    let (learner, module, block) = insert_lab_fixture_md(&state, MILESTONE_LAB_MD);
    let ws = tempfile::tempdir().unwrap();
    insert_session(&state, "sess-idem", &learner, &module, &block, ws.path().to_path_buf(), 1).await;

    // Seed passing evidence into the session history once.
    let req = check_req("sess-idem", 0, "kubectl apply -f pod.yaml", "pod/web created", Some(0));
    lab_check_step_with(req, &state, false).await.unwrap();

    for _ in 0..2 {
        let vreq = LabValidateMilestoneRequest {
            session_id: "sess-idem".to_string(),
            step_index: 0,
        };
        let result = lab_validate_milestone_with(vreq, &state, false)
            .await
            .expect("lab_validate_milestone must succeed");
        assert!(result.passed, "history still matches — both calls Pass");
    }

    let (current_step, completed) =
        progress_row(&state, &learner, &module, &block).expect("row must exist");
    assert_eq!(
        current_step, 1,
        "repeated Pass on the same step must not overrun current_step"
    );
    let v: serde_json::Value = serde_json::from_str(&completed).unwrap();
    assert_eq!(
        v.as_array().unwrap().len(),
        1,
        "completed_step_ids must contain the step id exactly ONCE, got {}",
        completed
    );

    let mastery: f64 = {
        let db = state.db.lock().unwrap();
        db.conn
            .query_row(
                "SELECT practical_mastery FROM module_progress
                 WHERE module_id = ?1 AND learner_id = ?2",
                rusqlite::params![module, learner],
                |r| r.get::<_, f64>(0),
            )
            .unwrap()
    };
    assert!(
        mastery <= 1.0,
        "practical_mastery must never exceed 1.0, got {}",
        mastery
    );
}

/// D-04 fail-safe guard — lab_validate_milestone on a step-grain step
/// returns an error (the button only shows for milestone steps, but the
/// handler must not trust the frontend).
#[tokio::test]
async fn lab_validate_milestone_rejects_step_grain() {
    let state = eval_test_app_state();
    let (learner, module, block) = insert_lab_fixture_md(&state, STEP_GRAIN_LAB_MD);
    let ws = tempfile::tempdir().unwrap();
    insert_session(&state, "sess-rej", &learner, &module, &block, ws.path().to_path_buf(), 2).await;

    let vreq = LabValidateMilestoneRequest {
        session_id: "sess-rej".to_string(),
        step_index: 0,
    };
    let err = lab_validate_milestone_with(vreq, &state, false)
        .await
        .expect_err("step-grain step must be rejected");
    assert!(
        err.to_lowercase().contains("milestone"),
        "error must name the milestone guard, got: {}",
        err
    );
}

/// D-02 anti-vacuous — lab_validate_milestone on a command_absent milestone
/// step with EMPTY history Fails with the "no commands recorded" reason.
#[tokio::test]
async fn lab_validate_milestone_empty_history_command_absent_reason() {
    let state = eval_test_app_state();
    let (learner, module, block) = insert_lab_fixture_md(&state, MILESTONE_ABSENT_LAB_MD);
    let ws = tempfile::tempdir().unwrap();
    insert_session(&state, "sess-empty", &learner, &module, &block, ws.path().to_path_buf(), 1).await;

    let vreq = LabValidateMilestoneRequest {
        session_id: "sess-empty".to_string(),
        step_index: 0,
    };
    let result = lab_validate_milestone_with(vreq, &state, false)
        .await
        .expect("handler must succeed (Fail is an outcome, not an error)");
    assert!(!result.passed);
    assert!(
        result.reason.contains("no commands recorded"),
        "reason must contain 'no commands recorded', got: {}",
        result.reason
    );
}
