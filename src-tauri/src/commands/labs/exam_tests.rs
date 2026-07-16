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
            "image": "alpine",
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

/// Sets `lab_progress` for an attempt that already reset the row (i.e. a
/// SECOND-or-later `exam_attempt_start_conn` call, which UPDATEs the
/// existing `(learner_id, module_id, block_id)` row rather than inserting a
/// new one — `seed_lab_progress`'s INSERT would violate the PK on a retake).
fn update_lab_progress(
    conn: &rusqlite::Connection,
    learner: &str,
    module: &str,
    block: &str,
    completed_step_ids: &str,
    metadata_json: &str,
) {
    conn.execute(
        "UPDATE lab_progress
         SET completed_step_ids = ?4, metadata_json = ?5, current_step = 1,
             last_updated = datetime('now')
         WHERE learner_id = ?1 AND module_id = ?2 AND block_id = ?3",
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

/// WR-01 — DB-stored specs (`payload_json.spec`) are re-validated via
/// `validate_spec` at read time (T-19-02): a stored spec with
/// `passThresholdPct: 150` (or any other out-of-range/structural
/// violation) must be rejected, never used directly at scoring time.
#[test]
fn read_lab_spec_conn_rejects_invalid_stored_spec() {
    let conn = fresh_conn();
    let (_learner, module, _block) = seed_exam_module(&conn);

    let mut spec = exam_spec_json();
    spec["spec"]["exam"]["passThresholdPct"] = serde_json::json!(150.0);
    let bad_block = "blk-invalid-spec-1".to_string();
    conn.execute(
        "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
            params_json, payload_json, source_anchors_json, metadata_json, retry_count,
            created_at, updated_at)
         VALUES (?1, ?2, 2, 'lab', 'ready', '{}', ?3, '[]', '{}', 0,
            datetime('now'), datetime('now'))",
        rusqlite::params![bad_block, module, spec.to_string()],
    )
    .unwrap();

    let result = super::super::read_lab_spec_conn(&conn, &bad_block);
    assert!(
        result.is_err(),
        "a stored spec failing validate_spec must be rejected (WR-01), got {:?}",
        result.map(|(s, _)| s.slug)
    );
}

/// CR-03 — the D-02 gate must live at the trust boundary: a block whose
/// spec has NO `exam:` frontmatter must be rejected by
/// `exam_attempt_start`, not just filtered by the frontend. Otherwise a
/// devtools IPC call against an easy regular lab mints a fake 100% Exam
/// row in the exam-attempt ledger.
#[test]
fn exam_attempt_start_rejects_non_exam_block() {
    let conn = fresh_conn();
    let (learner, module, _block) = seed_exam_module(&conn);

    // A regular (non-exam) lab block in the same module: exam == null.
    let mut spec = exam_spec_json();
    spec["spec"]["exam"] = serde_json::Value::Null;
    let regular_block = "blk-regular-1".to_string();
    conn.execute(
        "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
            params_json, payload_json, source_anchors_json, metadata_json, retry_count,
            created_at, updated_at)
         VALUES (?1, ?2, 1, 'lab', 'ready', '{}', ?3, '[]', '{}', 0,
            datetime('now'), datetime('now'))",
        rusqlite::params![regular_block, module, spec.to_string()],
    )
    .unwrap();

    let clock = FakeClock { now: chrono::Utc::now() };
    let request = ExamAttemptStartRequest {
        block_id: regular_block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };
    let result = exam_attempt_start_conn(&conn, &request, &clock);
    assert!(
        result.is_err(),
        "a non-exam-flagged block must be rejected at the IPC boundary (D-02/CR-03)"
    );

    // No attempt row may have been persisted.
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM exam_attempts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0, "rejected start must not persist an exam_attempts row");
}

/// WR-05 — `module_id` is resolved server-side from `module_blocks`; a
/// bogus client-supplied value is ignored, so the attempt row (and the
/// lab_progress reset/lookup key) always uses the block's real parent
/// module — evidence attribution can't be spoofed or drift.
#[test]
fn exam_attempt_start_resolves_module_id_server_side() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let clock = FakeClock { now: chrono::Utc::now() };

    let request = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: "mod-spoofed-does-not-exist".to_string(),
        learner_id: learner.clone(),
    };
    let started = exam_attempt_start_conn(&conn, &request, &clock)
        .expect("start must succeed — client module_id is ignored");

    let stored_module: String = conn
        .query_row(
            "SELECT module_id FROM exam_attempts WHERE id = ?1",
            rusqlite::params![started.attempt_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        stored_module, module,
        "attempt row must carry the block's REAL parent module_id (WR-05)"
    );

    // Finalize keys lab_progress by the resolved module — progress written
    // under the real (learner, module, block) triple counts.
    seed_lab_progress(&conn, &learner, &module, &block, "[\"write-manifest\"]", "{}");
    let result = finalize_attempt_conn(&conn, &started.attempt_id, &clock).unwrap();
    assert!(
        (result.score_percent - 50.0).abs() < 1e-9,
        "finalize must score against the resolved module key, got {}",
        result.score_percent
    );
}

/// CR-02 — scoring is attempt-scoped: progress earned BEFORE
/// `exam_attempt_start` (e.g. in regular lab mode, with hints and the
/// tutor) must NOT count toward the exam score. `exam_attempt_start`
/// resets the `lab_progress` row for (learner, module, block).
#[test]
fn exam_attempt_start_resets_pre_attempt_lab_progress() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let clock = FakeClock { now: chrono::Utc::now() };

    // Learner completed BOTH steps in ordinary (non-exam) lab mode, with
    // stale judge verdicts lying around (both the legacy single slot and
    // the WR-06 keyed map).
    seed_lab_progress(
        &conn,
        &learner,
        &module,
        &block,
        "[\"write-manifest\",\"explain-scheduling\"]",
        "{\"last_ai_judge\":{\"step_index\":1,\"outcome\":\"pass\",\"reason\":\"good\"},\
          \"ai_judge_verdicts\":{\"1\":{\"step_index\":1,\"outcome\":\"pass\",\"reason\":\"good\"}}}",
    );

    let start_req = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };
    let started = exam_attempt_start_conn(&conn, &start_req, &clock).unwrap();

    // Submit immediately — no work done DURING the attempt window.
    let result = finalize_attempt_conn(&conn, &started.attempt_id, &clock).unwrap();
    assert!(
        result.score_percent.abs() < 1e-9,
        "pre-attempt progress must not count (CR-02) — expected 0.0, got {}",
        result.score_percent
    );
    assert!(!result.passed);
    assert!(
        result.step_verdicts.iter().all(|v| !v.passed_toward_score),
        "no step may score from pre-attempt progress"
    );
    assert!(
        result.step_verdicts.iter().all(|v| v.outcome == "fail"),
        "start must clear stale judge verdicts (keyed + legacy) too, got {:?}",
        result.step_verdicts.iter().map(|v| v.outcome.clone()).collect::<Vec<_>>()
    );
}

/// CR-02 — a retake starts from 0: the second `exam_attempt_start` must
/// not inherit the first attempt's completed steps (D-05 retakes are each
/// their own history row, not N copies of the same inflated score).
#[test]
fn exam_attempt_retake_starts_from_zero_progress() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let clock = FakeClock { now: chrono::Utc::now() };
    let start_req = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };

    // Attempt 1: complete both steps during the window → 100%.
    let first = exam_attempt_start_conn(&conn, &start_req, &clock).unwrap();
    seed_lab_progress(
        &conn,
        &learner,
        &module,
        &block,
        "[\"write-manifest\",\"explain-scheduling\"]",
        "{}",
    );
    let first_result = finalize_attempt_conn(&conn, &first.attempt_id, &clock).unwrap();
    assert!((first_result.score_percent - 100.0).abs() < 1e-9);

    // Attempt 2 (retake): no work done — must score 0, not inherit 100.
    let second = exam_attempt_start_conn(&conn, &start_req, &clock).unwrap();
    let second_result = finalize_attempt_conn(&conn, &second.attempt_id, &clock).unwrap();
    assert!(
        second_result.score_percent.abs() < 1e-9,
        "retake must start from 0 (CR-02) — got {}",
        second_result.score_percent
    );

    // Attempt 1's history row is untouched (D-05).
    let first_again = exam_attempt_get_conn(&conn, &first.attempt_id, &clock).unwrap();
    assert!((first_again.score_percent - 100.0).abs() < 1e-9);
}

/// WR-06 — an exam with MULTIPLE ai_judge steps must keep every step's
/// latest verdict, not just the single most-recent one. Verdicts persist
/// keyed by step index in `metadata_json.$.ai_judge_verdicts`; the legacy
/// single-slot `$.last_ai_judge` remains for backward compat. A step the
/// judge marked Manual must show as Manual with its reason — never
/// degrade to a bare "fail" just because a later step was judged after it.
#[test]
fn exam_attempt_multi_ai_judge_steps_keep_per_step_verdicts() {
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

    // Step 0 was judged Manual (budget exhausted), then step 1 was judged
    // Fail — last_ai_judge only remembers step 1, but the keyed map keeps
    // both.
    seed_lab_progress(
        &conn,
        &learner,
        &module,
        &block,
        "[]",
        "{\"ai_judge_verdicts\":{\
            \"0\":{\"step_index\":0,\"outcome\":\"manual\",\"reason\":\"budget exhausted\"},\
            \"1\":{\"step_index\":1,\"outcome\":\"fail\",\"reason\":\"insufficient detail\"}},\
          \"last_ai_judge\":{\"step_index\":1,\"outcome\":\"fail\",\"reason\":\"insufficient detail\"}}",
    );

    let result = finalize_attempt_conn(&conn, &started.attempt_id, &clock).unwrap();
    assert_eq!(
        result.step_verdicts[0].outcome, "manual",
        "step 0's Manual verdict must survive a later judge call (WR-06)"
    );
    assert_eq!(
        result.step_verdicts[0].check_reason.as_deref(),
        Some("budget exhausted"),
        "the Manual reason must be preserved"
    );
    assert_eq!(result.step_verdicts[1].outcome, "fail");
    assert_eq!(
        result.step_verdicts[1].check_reason.as_deref(),
        Some("insufficient detail")
    );
    assert!(result.score_percent.abs() < 1e-9);
}

/// WR-04 — a judge "pass" NOT backed by `completed_step_ids` (e.g. a
/// stale verdict left behind after lab_reset cleared progress) must never
/// render as a green "Passed" row while contributing zero score. The
/// outcome is sanitized to "indeterminate" with the reason preserved.
#[test]
fn exam_attempt_stale_judge_pass_is_sanitized_to_indeterminate() {
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

    // Judge says "pass" for step 1 but completed_step_ids is empty — the
    // step did NOT count toward the score.
    seed_lab_progress(
        &conn,
        &learner,
        &module,
        &block,
        "[]",
        "{\"last_ai_judge\":{\"step_index\":1,\"outcome\":\"pass\",\"reason\":\"stale judge pass\"}}",
    );

    let result = finalize_attempt_conn(&conn, &started.attempt_id, &clock).unwrap();
    let verdict = &result.step_verdicts[1];
    assert_ne!(
        verdict.outcome, "pass",
        "an unbacked judge pass must never display as Passed (WR-04)"
    );
    assert_eq!(verdict.outcome, "indeterminate");
    assert!(!verdict.passed_toward_score);
    assert_eq!(
        verdict.check_reason.as_deref(),
        Some("stale judge pass"),
        "the judge's reason must be preserved"
    );
    assert!(result.score_percent.abs() < 1e-9);
}

/// CR-01 — an attempt whose lab session never opened (no `lab_progress`
/// row at all: Docker/runtime start failure, or submit racing the async
/// session open) must still finalize: status="completed", score 0.0 —
/// never a hard error that would strand the attempt permanently
/// `in_progress` and make `exam_attempt_get` fail forever after the
/// deadline (D-04 lazy-reconcile also routes through finalize).
#[test]
fn exam_attempt_submit_with_no_lab_progress_row_scores_zero() {
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

    // Deliberately NO seed_lab_progress — the session never opened.
    let result = finalize_attempt_conn(&conn, &started.attempt_id, &clock)
        .expect("finalize must tolerate a missing lab_progress row (CR-01)");

    assert_eq!(result.status, "completed");
    assert!(
        result.score_percent.abs() < 1e-9,
        "no progress row means zero progress — expected 0.0, got {}",
        result.score_percent
    );
    assert!(!result.passed);
    assert_eq!(result.step_verdicts.len(), 2);
    assert!(
        result.step_verdicts.iter().all(|v| v.outcome == "fail"),
        "every step defaults to Fail when no progress exists"
    );
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

// ── 19-07 gap closure: exam_attempt_history (D-06 learner-facing
// best-attempt history note — the counterpart of evidence_ledger's
// best-of-N aggregation in storage_impl/reports.rs) ──

/// Attempt A (50%) then attempt B (100%) for the same learner/block.
/// History for attempt B: attempt_number 2, total_attempts 2, best 100%
/// (attempt B's own finished_at). History for attempt A: attempt_number 1,
/// total_attempts 2, best 100% (the BEST across attempts, not attempt A's
/// own score).
#[test]
fn exam_attempt_history_reports_best_score_and_attempt_count() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);

    let start_req = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };

    // Attempt A: t0, one completed step -> 50%.
    let t0 = chrono::Utc::now() - chrono::Duration::hours(2);
    let clock_a = FakeClock { now: t0 };
    let attempt_a = exam_attempt_start_conn(&conn, &start_req, &clock_a).unwrap();
    seed_lab_progress(&conn, &learner, &module, &block, "[\"write-manifest\"]", "{}");
    let result_a = finalize_attempt_conn(&conn, &attempt_a.attempt_id, &clock_a).unwrap();
    assert!((result_a.score_percent - 50.0).abs() < 1e-9);

    // Attempt B: t1 (safely past attempt A's deadline), both steps -> 100%.
    let t1 = t0 + chrono::Duration::hours(1);
    let clock_b = FakeClock { now: t1 };
    let attempt_b = exam_attempt_start_conn(&conn, &start_req, &clock_b).unwrap();
    update_lab_progress(
        &conn,
        &learner,
        &module,
        &block,
        "[\"write-manifest\",\"explain-scheduling\"]",
        "{}",
    );
    let result_b = finalize_attempt_conn(&conn, &attempt_b.attempt_id, &clock_b).unwrap();
    assert!((result_b.score_percent - 100.0).abs() < 1e-9);

    // History for attempt B.
    let history_b = exam_attempt_history_conn(&conn, &attempt_b.attempt_id)
        .expect("exam_attempt_history_conn for attempt B");
    assert_eq!(history_b.attempt_number, 2);
    assert_eq!(history_b.total_attempts, 2);
    assert!((history_b.best_score_percent - 100.0).abs() < 1e-9);
    assert_eq!(history_b.best_attempt_date, result_b.finished_at.unwrap());

    // History for attempt A — best score is 100% (across attempts), not A's own 50%.
    let history_a = exam_attempt_history_conn(&conn, &attempt_a.attempt_id)
        .expect("exam_attempt_history_conn for attempt A");
    assert_eq!(history_a.attempt_number, 1);
    assert_eq!(history_a.total_attempts, 2);
    assert!(
        (history_a.best_score_percent - 100.0).abs() < 1e-9,
        "best score must be the BEST across all attempts, got {}",
        history_a.best_score_percent
    );
}

/// An `in_progress` (never-finalized) second attempt must never count
/// toward total_attempts or attempt_number — mirrors the evidence_ledger
/// status filter (`completed`/`timed_out_partial` only).
#[test]
fn exam_attempt_history_excludes_in_progress_attempts() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let clock = FakeClock { now: chrono::Utc::now() };

    let start_req = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };

    // Finalize the first attempt.
    let first = exam_attempt_start_conn(&conn, &start_req, &clock).unwrap();
    seed_lab_progress(&conn, &learner, &module, &block, "[\"write-manifest\"]", "{}");
    finalize_attempt_conn(&conn, &first.attempt_id, &clock).unwrap();

    // Start a second attempt but never finalize it.
    let _second = exam_attempt_start_conn(&conn, &start_req, &clock).unwrap();

    let history = exam_attempt_history_conn(&conn, &first.attempt_id)
        .expect("exam_attempt_history_conn");
    assert_eq!(
        history.total_attempts, 1,
        "in_progress attempts must never count toward total_attempts"
    );
    assert_eq!(history.attempt_number, 1);
}

/// `exam_attempt_history_conn` with a nonexistent attempt id returns Err —
/// `read_attempt_row` already produces this.
#[test]
fn exam_attempt_history_errors_on_unknown_attempt_id() {
    let conn = fresh_conn();
    let (_learner, _module, _block) = seed_exam_module(&conn);

    let result = exam_attempt_history_conn(&conn, "exam-does-not-exist");
    assert!(
        result.is_err(),
        "an unknown attempt_id must error, not silently default"
    );
}

/// Two finalized attempts with EQUAL best score_percent must break the tie
/// deterministically toward the EARLIER finished_at (closes the IN-06-style
/// nondeterminism for this IPC).
#[test]
fn exam_attempt_history_ties_break_to_earliest_finished_at() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);

    let start_req = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };

    // Attempt A: t0, identical progress -> 50%.
    let t0 = chrono::Utc::now() - chrono::Duration::hours(2);
    let clock_a = FakeClock { now: t0 };
    let attempt_a = exam_attempt_start_conn(&conn, &start_req, &clock_a).unwrap();
    seed_lab_progress(&conn, &learner, &module, &block, "[\"write-manifest\"]", "{}");
    let result_a = finalize_attempt_conn(&conn, &attempt_a.attempt_id, &clock_a).unwrap();

    // Attempt B: t1 (later), identical progress -> 50% (tie).
    let t1 = t0 + chrono::Duration::hours(1);
    let clock_b = FakeClock { now: t1 };
    let attempt_b = exam_attempt_start_conn(&conn, &start_req, &clock_b).unwrap();
    update_lab_progress(&conn, &learner, &module, &block, "[\"write-manifest\"]", "{}");
    let result_b = finalize_attempt_conn(&conn, &attempt_b.attempt_id, &clock_b).unwrap();

    assert!((result_a.score_percent - result_b.score_percent).abs() < 1e-9, "tie setup");
    assert_ne!(
        result_a.finished_at, result_b.finished_at,
        "test setup: finished_at must differ between the two attempts"
    );

    let history = exam_attempt_history_conn(&conn, &attempt_b.attempt_id)
        .expect("exam_attempt_history_conn");
    assert_eq!(
        history.best_attempt_date,
        result_a.finished_at.unwrap(),
        "tied best score must break to the EARLIER finished_at (deterministic)"
    );
}

/// CR-01 — two attempts started with the IDENTICAL `FakeClock` value (the
/// exact pattern already used by
/// `exam_attempt_start_twice_creates_distinct_history_rows`) must still
/// report DISTINCT, correctly-ordered `attempt_number`s once both are
/// finalized. Before the fix, `attempt_number` was computed via
/// `started_at <= ?3` with no tie-break on insertion order, so both
/// attempts (sharing the same `started_at`) counted each other as "at or
/// before" themselves and BOTH reported `attempt_number = 2` — the first
/// (earlier, lower-rowid) attempt should report `1`.
#[test]
fn exam_attempt_history_attempt_number_breaks_ties_on_identical_started_at() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    let clock = FakeClock { now: chrono::Utc::now() };
    let start_req = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };

    // Both attempts share the exact same started_at (identical FakeClock).
    let attempt_a = exam_attempt_start_conn(&conn, &start_req, &clock).unwrap();
    seed_lab_progress(&conn, &learner, &module, &block, "[\"write-manifest\"]", "{}");
    let result_a = finalize_attempt_conn(&conn, &attempt_a.attempt_id, &clock).unwrap();
    assert_eq!(result_a.status, "completed");

    let attempt_b = exam_attempt_start_conn(&conn, &start_req, &clock).unwrap();
    update_lab_progress(&conn, &learner, &module, &block, "[\"write-manifest\"]", "{}");
    let result_b = finalize_attempt_conn(&conn, &attempt_b.attempt_id, &clock).unwrap();
    assert_eq!(result_b.status, "completed");

    assert_eq!(
        attempt_a.started_at, attempt_b.started_at,
        "test setup: both attempts must share the identical started_at"
    );

    let history_a = exam_attempt_history_conn(&conn, &attempt_a.attempt_id)
        .expect("exam_attempt_history_conn for attempt A");
    let history_b = exam_attempt_history_conn(&conn, &attempt_b.attempt_id)
        .expect("exam_attempt_history_conn for attempt B");

    assert_eq!(
        history_a.attempt_number, 1,
        "the first (earlier-inserted) attempt must report attempt_number 1, got {}",
        history_a.attempt_number
    );
    assert_eq!(
        history_b.attempt_number, 2,
        "the second attempt must report attempt_number 2, got {}",
        history_b.attempt_number
    );
}

// ── Phase 19.3 (D-05) — fail-closed exam rejection of milestone grain ──
//
// `reject_milestone_exam_spec` does not exist yet — RED until Task 3 lands
// the gate in exam_attempt_start_conn (after the D-02 gate, before any
// exam_attempts row is minted).

/// Minimal exam-flagged LabSpec with configurable grains for gate tests.
fn milestone_gate_spec(
    lab_grain: crate::labs::spec::Grain,
    step_grain: crate::labs::spec::Grain,
) -> crate::labs::spec::LabSpec {
    use crate::labs::spec::{ExamMeta, LabSpec, LabStep, StepCheck};
    LabSpec {
        slug: "milestone-gate".to_string(),
        title: "Milestone gate".to_string(),
        image: Some("alpine".to_string()),
        dockerfile: None,
        requires_docker: false,
        creates: vec![],
        exam: Some(ExamMeta {
            time_limit_minutes: Some(45),
            pass_threshold_pct: Some(70.0),
        }),
        grain: lab_grain,
        steps: vec![LabStep {
            id: "s1".to_string(),
            title: "S1".to_string(),
            prompt: "do the thing".to_string(),
            check: StepCheck::ExitCode { expected: 0 },
            hints: vec![],
            weight: 1.0,
            grain: step_grain,
        }],
    }
}

/// D-05 — a spec declaring lab-level milestone grain is rejected with a
/// typed error naming D-05 before any exam_attempts row can be minted.
#[test]
fn exam_start_rejects_milestone_lab() {
    use crate::labs::spec::Grain;
    let spec = milestone_gate_spec(Grain::Milestone, Grain::Step);
    let err = reject_milestone_exam_spec(&spec, "blk-ms").expect_err("must reject");
    assert!(err.contains("D-05"), "error must cite D-05, got: {}", err);
    assert!(
        err.to_lowercase().contains("milestone"),
        "error must name milestone grain, got: {}",
        err
    );
}

/// D-05 — the rejection also fires when only a STEP declares milestone.
#[test]
fn exam_start_rejects_milestone_step() {
    use crate::labs::spec::Grain;
    let spec = milestone_gate_spec(Grain::Step, Grain::Milestone);
    let err = reject_milestone_exam_spec(&spec, "blk-ms").expect_err("must reject");
    assert!(err.contains("D-05"), "error must cite D-05, got: {}", err);
    // Step-grain-only spec passes the gate (no false positive).
    let ok_spec = milestone_gate_spec(Grain::Step, Grain::Step);
    assert!(reject_milestone_exam_spec(&ok_spec, "blk-ok").is_ok());
}

/// D-05 end-to-end — a stored milestone+exam spec cannot start an attempt
/// through `exam_attempt_start_conn` (either via the author-time validator
/// in read_lab_spec_conn or the runtime gate) and NO exam_attempts row is
/// minted (fail-closed proof).
#[test]
fn exam_start_milestone_spec_mints_no_attempt_row() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);
    // Overwrite the stored spec with a milestone+exam variant.
    let mut payload = exam_spec_json();
    payload["spec"]["grain"] = serde_json::json!("milestone");
    conn.execute(
        "UPDATE module_blocks SET payload_json = ?1 WHERE id = ?2",
        rusqlite::params![payload.to_string(), block],
    )
    .unwrap();

    let clock = FakeClock { now: chrono::Utc::now() };
    let request = ExamAttemptStartRequest {
        block_id: block.clone(),
        track_id: "trk-exam-1".to_string(),
        module_id: module.clone(),
        learner_id: learner.clone(),
    };
    let result = exam_attempt_start_conn(&conn, &request, &clock);
    assert!(result.is_err(), "milestone exam spec must not start an attempt");

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM exam_attempts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0, "no exam_attempts row may be minted (fail-closed)");
}
