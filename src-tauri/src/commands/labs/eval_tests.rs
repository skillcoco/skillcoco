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
    if matches!(outcome, EvalOutcome::Pass) {
        conn.execute(
            "UPDATE lab_progress
             SET current_step = current_step + 1,
                 completed_step_ids = json_insert(completed_step_ids, '$[#]', ?1),
                 last_updated = datetime('now')
             WHERE learner_id = ?2 AND module_id = ?3 AND block_id = ?4",
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
