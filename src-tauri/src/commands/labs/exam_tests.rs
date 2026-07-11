//! GREEN tests for `commands::labs::exam` (Phase 19, 19-03). Exercises the
//! `Connection`-based inner helpers (`exam_attempt_start_conn`,
//! `finalize_attempt_conn`, `exam_attempt_get_conn`) directly — these carry
//! all production logic; the `#[tauri::command]` wrappers only add the
//! `state.db.lock()` step, which isn't constructible in a unit test.
//!
//! Seeding strategy: tests write directly into `lab_progress` (the
//! server-authoritative rails 19-03 reads — state.rs:123, eval.rs:187-255)
//! so every score assertion is against a DERIVED score, never a
//! client-supplied one (D-15 / T-19-10).

use super::*;

fn fresh_conn() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(crate::db::schema::CREATE_TABLES).unwrap();
    crate::db::migrations::apply_migrations(&conn).unwrap();
    conn
}

/// A fake clock pinned to a fixed instant so timeout logic is
/// deterministic — mirrors `labs::prompt_detect`'s tick-driven approach
/// without depending on wall-clock `Utc::now()`.
struct FakeClock {
    now: chrono::DateTime<chrono::Utc>,
}

impl Clock for FakeClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.now
    }
}

/// Two-step exam spec: one file_state-style step ("write-manifest") and one
/// ai_judge step ("explain-scheduling"), each weight 1.0 (equal weighting),
/// with a 45-minute time limit and 70% pass threshold — stored in the
/// block's `payload_json.spec` (the PagePlanner-emitted shape
/// `read_lab_spec_conn` tries first).
fn exam_spec_json() -> serde_json::Value {
    serde_json::json!({
        "spec": {
            "slug": "exam-fixture",
            "title": "Exam Fixture",
            "image": null,
            "dockerfile": null,
            "requiresDocker": false,
            "creates": [],
            "exam": { "timeLimitMinutes": 45, "passThresholdPct": 70.0 },
            "steps": [
                {
                    "id": "write-manifest",
                    "title": "Write the manifest",
                    "prompt": "Write a Pod manifest.",
                    "check": { "kind": "file_state", "path": "pod.yaml" },
                    "hints": [],
                    "weight": 1.0
                },
                {
                    "id": "explain-scheduling",
                    "title": "Explain scheduling",
                    "prompt": "Explain how the scheduler placed the pod.",
                    "check": { "kind": "ai_judge", "criteria": "Explanation covers node selection basics", "threshold": 0.7 },
                    "hints": [],
                    "weight": 1.0
                }
            ]
        }
    })
}

/// Seed a learner/track/path/module/block quad with the two-step exam spec
/// wired into `payload_json.spec` so `read_lab_spec_conn` resolves it.
fn seed_exam_module(conn: &rusqlite::Connection) -> (String, String, String) {
    let learner = "lp-exam-1".to_string();
    let track = "trk-exam-1".to_string();
    let path = "path-exam-1".to_string();
    let module = "mod-exam-1".to_string();
    let block = "blk-exam-1".to_string();
    conn.execute(
        "INSERT INTO learner_profiles (id, display_name) VALUES (?1, 'Exam Taker')",
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
        "INSERT INTO modules (id, path_id, title, ordering) VALUES (?1, ?2, 'Exam Module', 0)",
        rusqlite::params![module, path],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
            params_json, payload_json, source_anchors_json, metadata_json, retry_count,
            created_at, updated_at)
         VALUES (?1, ?2, 0, 'lab', 'ready', '{}', ?3, '[]', '{}', 0,
            datetime('now'), datetime('now'))",
        rusqlite::params![block, module, exam_spec_json().to_string()],
    )
    .unwrap();
    (learner, module, block)
}

fn seed_lab_progress(
    conn: &rusqlite::Connection,
    learner: &str,
    module: &str,
    block: &str,
    completed_step_ids: &str,
    metadata_json: &str,
) {
    conn.execute(
        "INSERT INTO lab_progress
            (learner_id, module_id, block_id, current_step, completed_step_ids,
             total_steps, metadata_json, last_updated)
         VALUES (?1, ?2, ?3, 1, ?4, 2, ?5, datetime('now'))",
        rusqlite::params![learner, module, block, completed_step_ids, metadata_json],
    )
    .unwrap();
}

/// `exam_attempt_start` must persist an `exam_attempts` row with
/// status='in_progress' and `deadline_at = started_at + timeLimitMinutes`.
#[test]
fn exam_attempt_start_persists_in_progress_row_with_deadline() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let clock = FakeClock { now: chrono::Utc::now() };

    let request = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };
    let result = exam_attempt_start_conn(&conn, &request, &clock).expect("exam_attempt_start_conn");

    assert_eq!(result.total_steps, 2);
    assert_eq!(result.time_limit_minutes, 45);
    assert!((result.pass_threshold_pct - 70.0).abs() < 1e-9);
    assert_eq!(result.started_at, clock.now.to_rfc3339());
    let expected_deadline = (clock.now + chrono::Duration::minutes(45)).to_rfc3339();
    assert_eq!(result.deadline_at, expected_deadline);

    // Row persisted with status='in_progress'.
    let status: String = conn
        .query_row(
            "SELECT status FROM exam_attempts WHERE id = ?1",
            rusqlite::params![result.attempt_id],
            |r| r.get(0),
        )
        .expect("row must exist");
    assert_eq!(status, "in_progress");
}

/// A second `exam_attempt_start` call for the same learner/module/block
/// produces a DISTINCT attempt row (D-05 — unlimited retakes, each its own
/// history row via INSERT, never upserted).
#[test]
fn exam_attempt_start_twice_creates_distinct_history_rows() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let clock = FakeClock { now: chrono::Utc::now() };
    let request = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };

    let first = exam_attempt_start_conn(&conn, &request, &clock).unwrap();
    let second = exam_attempt_start_conn(&conn, &request, &clock).unwrap();
    assert_ne!(first.attempt_id, second.attempt_id);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM exam_attempts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2, "each start is a distinct history row (D-05)");
}

/// `exam_attempt_submit` DERIVES per-step verdicts from
/// `lab_progress.completed_step_ids` (Pass) and
/// `metadata_json.$.last_ai_judge` (fail/manual/indeterminate) — never from
/// client input. Seeds one completed step + one AI-judge Fail verdict, then
/// asserts the expected DERIVED score of 50%. `ExamAttemptSubmitRequest`
/// has no verdicts field (D-15) so there is nothing for a caller to
/// override.
#[test]
fn exam_attempt_submit_derives_score_from_lab_progress_never_from_client() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let clock = FakeClock { now: chrono::Utc::now() };

    let start_req = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };
    let started = exam_attempt_start_conn(&conn, &start_req, &clock).unwrap();

    // Seed lab_progress: step "write-manifest" completed (Pass, index 0),
    // step "explain-scheduling" (index 1) AI-judged Fail.
    seed_lab_progress(
        &conn,
        &learner,
        &module,
        &block,
        "[\"write-manifest\"]",
        "{\"last_ai_judge\":{\"step_index\":1,\"outcome\":\"fail\",\"reason\":\"insufficient detail\"}}",
    );

    // Compile-time contract check: ExamAttemptSubmitRequest must NOT admit
    // a verdicts field (D-15). Constructing it with only the two declared
    // fields is itself the enforcement.
    let submit_req = ExamAttemptSubmitRequest { attempt_id: started.attempt_id.clone(), current_step: Some(2) };
    let ExamAttemptSubmitRequest { attempt_id: _, current_step: _ } = submit_req.clone();

    let result = finalize_attempt_conn(&conn, &submit_req.attempt_id, &clock)
        .expect("finalize_attempt_conn must succeed");

    assert_eq!(result.status, "completed");
    assert!(
        (result.score_percent - 50.0).abs() < 1e-9,
        "expected 50.0, got {}",
        result.score_percent
    );
    assert!(!result.passed, "50% < 70% pass threshold");
    assert_eq!(result.step_verdicts.len(), 2);
    assert_eq!(result.step_verdicts[0].outcome, "pass");
    assert!(result.step_verdicts[0].passed_toward_score);
    assert_eq!(result.step_verdicts[1].outcome, "fail");
    assert!(!result.step_verdicts[1].passed_toward_score);
}

/// A submit past `deadline_at` must be treated as `timed_out_partial`
/// regardless of client claim (Pattern 3 tamper resistance / T-19-01).
/// Steps not in `completed_step_ids` count as Fail (D-04).
#[test]
fn exam_attempt_submit_past_deadline_is_timed_out_partial_regardless_of_client_claim() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let start_time = chrono::Utc::now() - chrono::Duration::minutes(50);
    let start_clock = FakeClock { now: start_time };

    let start_req = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };
    let started = exam_attempt_start_conn(&conn, &start_req, &start_clock).unwrap();
    // deadline_at = start_time + 45 minutes; "now" below is start_time itself,
    // which is > deadline only once we advance past 45 minutes.
    let now_clock = FakeClock { now: start_time + chrono::Duration::minutes(46) };
    assert!(
        now_clock.now.to_rfc3339().as_str() > started.deadline_at.as_str(),
        "test setup: fake clock must be past the deadline"
    );

    // Only step 0 completed — step 1 has no verdict at all (never checked).
    seed_lab_progress(&conn, &learner, &module, &block, "[\"write-manifest\"]", "{}");

    let result = finalize_attempt_conn(&conn, &started.attempt_id, &now_clock)
        .expect("finalize_attempt_conn");
    assert_eq!(result.status, "timed_out_partial");
    assert_eq!(result.step_verdicts[1].outcome, "fail");
    assert!(!result.step_verdicts[1].passed_toward_score);
}

/// Manual/Indeterminate outcomes count as Fail in the scoring denominator
/// (UI-SPEC lock), never silently excluded.
#[test]
fn exam_attempt_manual_and_indeterminate_count_as_fail_in_denominator() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let clock = FakeClock { now: chrono::Utc::now() };

    let start_req = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };
    let started = exam_attempt_start_conn(&conn, &start_req, &clock).unwrap();

    // No steps completed; step 1 AI-judged Manual (budget exhausted).
    seed_lab_progress(
        &conn,
        &learner,
        &module,
        &block,
        "[]",
        "{\"last_ai_judge\":{\"step_index\":1,\"outcome\":\"manual\",\"reason\":\"budget exhausted\"}}",
    );

    let result = finalize_attempt_conn(&conn, &started.attempt_id, &clock).unwrap();
    assert!(
        result.score_percent.abs() < 1e-9,
        "manual/indeterminate steps stay IN the denominator, never excluded — expected 0.0, got {}",
        result.score_percent
    );
    assert_eq!(result.step_verdicts.len(), 2, "denominator must include both steps");
    assert_eq!(result.step_verdicts[1].outcome, "manual");
    assert!(!result.step_verdicts[1].passed_toward_score);
}

/// `exam_attempt_get` on an in_progress row past its deadline must lazily
/// reconcile it to `timed_out_partial` (D-04) — a learner who closed the
/// app never sends submit.
#[test]
fn exam_attempt_get_reconciles_stale_in_progress_past_deadline() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let start_time = chrono::Utc::now() - chrono::Duration::minutes(60);
    let start_clock = FakeClock { now: start_time };

    let start_req = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };
    let started = exam_attempt_start_conn(&conn, &start_req, &start_clock).unwrap();

    seed_lab_progress(&conn, &learner, &module, &block, "[\"write-manifest\"]", "{}");

    // "Now" is far past the 45-minute deadline computed at start_time.
    let get_clock = FakeClock { now: chrono::Utc::now() };
    assert!(
        get_clock.now.to_rfc3339().as_str() > started.deadline_at.as_str(),
        "test setup: attempt must be past its deadline"
    );

    let result = exam_attempt_get_conn(&conn, &started.attempt_id, &get_clock)
        .expect("exam_attempt_get_conn");
    assert_eq!(
        result.status, "timed_out_partial",
        "exam_attempt_get must reconcile a stale in_progress attempt past deadline_at (D-04)"
    );

    // Reconciliation persisted — re-reading the row shows the same status.
    let persisted_status: String = conn
        .query_row(
            "SELECT status FROM exam_attempts WHERE id = ?1",
            rusqlite::params![started.attempt_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(persisted_status, "timed_out_partial");
}

/// Two submits for the same attempt_id: second is idempotent — no
/// double-scoring (T-19-05). Asserts unchanged score_percent and
/// finished_at across both calls.
#[test]
fn exam_attempt_second_submit_on_finalized_attempt_is_idempotent() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let clock = FakeClock { now: chrono::Utc::now() };

    let start_req = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };
    let started = exam_attempt_start_conn(&conn, &start_req, &clock).unwrap();
    seed_lab_progress(&conn, &learner, &module, &block, "[\"write-manifest\"]", "{}");

    let first = finalize_attempt_conn(&conn, &started.attempt_id, &clock).unwrap();

    // Advance the clock and mutate lab_progress — if submit re-scored, this
    // would change the result. It must not.
    let later_clock = FakeClock { now: clock.now + chrono::Duration::minutes(5) };
    conn.execute(
        "UPDATE lab_progress SET completed_step_ids = '[\"write-manifest\",\"explain-scheduling\"]'
         WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
        rusqlite::params![learner, module, block],
    )
    .unwrap();

    let second = finalize_attempt_conn(&conn, &started.attempt_id, &later_clock).unwrap();
    assert_eq!(second.score_percent, first.score_percent, "second submit must not re-score");
    assert_eq!(second.finished_at, first.finished_at, "second submit must not overwrite finished_at");
    assert_eq!(second.status, first.status);
}

/// The production handler bodies must no longer be `unimplemented!` stubs.
#[test]
fn exam_ipc_handlers_are_implemented() {
    let source = include_str!("exam.rs");
    let stub_count = source.matches("unimplemented!(").count();
    assert_eq!(
        stub_count, 0,
        "exam.rs still has {} unimplemented!() stub(s) — 19-03 must fully implement \
         exam_attempt_start/submit/get",
        stub_count
    );
}
