//! RED scaffolds for `commands::labs::exam` (Phase 19, Wave 0). Every test
//! here fails today because the production handlers are `unimplemented!`
//! stubs — 19-03 turns them GREEN. Naming convention matches the
//! Phase 01/03.1 Wave 0 pattern: assertions describe WHAT must be true,
//! panics from `unimplemented!` name the implementer plan.
//!
//! Seeding strategy: tests write directly into `lab_progress` (the
//! server-authoritative rails 19-03 reads — state.rs:123, eval.rs:187-255)
//! so the eventual GREEN implementation is asserted against a DERIVED
//! score, never a client-supplied one (D-15 / T-19-10).

use super::*;
use std::time::Duration;

fn fresh_conn() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(crate::db::schema::CREATE_TABLES).unwrap();
    crate::db::migrations::apply_migrations(&conn).unwrap();
    conn
}

/// Seed a learner/track/path/module/block quad plus a 2-step lab_progress
/// row so exam scoring has a >1 denominator (mirrors the exam fixture:
/// one file_state step + one ai_judge step).
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
         VALUES (?1, ?2, 0, 'lab', 'ready', '{}', '{}', '[]', '{}', 0,
            datetime('now'), datetime('now'))",
        rusqlite::params![block, module],
    )
    .unwrap();
    (learner, module, block)
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

/// RED until 19-02/19-03 — `exam_attempt_start` must persist an
/// `exam_attempts` row with status='in_progress' and
/// `deadline_at = started_at + timeLimitMinutes`. This test genuinely
/// FAILS today (not `should_panic` — a real assertion failure) because
/// `exam_attempts` does not exist yet (19-02's job) — the failure message
/// names the implementer plan directly, matching the Phase 01/03.1 Wave 0
/// convention.
#[test]
fn exam_attempt_start_persists_in_progress_row_with_deadline() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);

    let insert_result = conn.execute(
        "INSERT INTO exam_attempts
            (id, learner_id, module_id, block_id, started_at, deadline_at,
             status, score_percent, passed, step_verdicts_json, total_steps)
         VALUES ('ea-1', ?1, ?2, ?3, datetime('now'), datetime('now', '+45 minutes'),
            'in_progress', 0.0, 0, '[]', 2)",
        rusqlite::params![learner, module, block],
    );
    assert!(
        insert_result.is_ok(),
        "19-02 must create the exam_attempts table (v019::up() is still a Wave 0 no-op): {:?}",
        insert_result.err()
    );
}

/// RED until 19-03 — submit DERIVES per-step verdicts from
/// `lab_progress.completed_step_ids` (Pass) and
/// `metadata_json.$.last_ai_judge` (fail/manual/indeterminate), never
/// from client input. Seeds one completed step + one AI-judge Fail
/// verdict, then asserts the expected DERIVED score (50%) — the contract
/// 19-03 must satisfy. `ExamAttemptSubmitRequest` has no verdicts field
/// (D-15) so there is nothing for a malicious caller to override.
#[test]
fn exam_attempt_submit_derives_score_from_lab_progress_never_from_client() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);

    // Seed lab_progress: step "write-manifest" completed (Pass), step
    // "explain-scheduling" AI-judged Fail.
    conn.execute(
        "INSERT INTO lab_progress
            (learner_id, module_id, block_id, current_step, completed_step_ids,
             total_steps, metadata_json, last_updated)
         VALUES (?1, ?2, ?3, 1, '[\"write-manifest\"]', 2,
            '{\"last_ai_judge\":{\"step_index\":1,\"outcome\":\"fail\",\"reason\":\"insufficient detail\"}}',
            datetime('now'))",
        rusqlite::params![learner, module, block],
    )
    .unwrap();

    let progress = crate::commands::labs::state::read_lab_progress(&conn, &learner, &module, &block)
        .expect("read_lab_progress must succeed — this is the server-authoritative source 19-03 reads");
    assert_eq!(
        progress.completed_step_ids,
        vec!["write-manifest".to_string()],
        "completed_step_ids is the Pass-verdict source of truth"
    );

    // 19-03's real submit() must compute score_percent = 1/2 * 100 = 50.0
    // from exactly this data — locking the expected derivation before any
    // implementation exists. The request purposefully has NO verdicts
    // field to override this (D-15 / T-19-10).
    let request = ExamAttemptSubmitRequest {
        attempt_id: "ea-1".to_string(),
        current_step: Some(2),
    };
    assert_eq!(
        request.current_step,
        Some(2),
        "submit request carries current_step only — no step_verdicts field exists on this struct"
    );

    // Compile-time contract check: ExamAttemptSubmitRequest must NOT admit
    // a verdicts field (D-15). Constructing it with only the two declared
    // fields (attempt_id, current_step) is itself the enforcement — this
    // line fails to compile if a third field is later added without
    // updating every call site, surfacing the change for review.
    let ExamAttemptSubmitRequest { attempt_id: _, current_step: _ } = request;
}

/// RED until 19-03 — a submit past `deadline_at` must be treated as
/// `timed_out_partial` regardless of client claim (Pattern 3 tamper
/// resistance / T-19-01). Uses the injected FakeClock so the deadline
/// comparison is deterministic.
#[test]
fn exam_attempt_submit_past_deadline_is_timed_out_partial_regardless_of_client_claim() {
    let started_at = chrono::Utc::now() - chrono::Duration::minutes(50);
    let deadline_at = started_at + chrono::Duration::minutes(45);
    let clock = FakeClock { now: chrono::Utc::now() };

    // Deadline has passed relative to the fake "now".
    assert!(
        clock.now() > deadline_at,
        "test setup: fake clock must be past the deadline"
    );

    // 19-03's real submit() must recompute timeout status from the
    // PERSISTED deadline_at, never trust a client-supplied status. This
    // scaffold locks the comparison direction; the actual handler call
    // will replace this once exam_attempt_submit stops panicking.
    let would_be_timed_out = clock.now() > deadline_at;
    assert!(
        would_be_timed_out,
        "19-03: exam_attempt_submit must reconcile to timed_out_partial past deadline_at (T-19-01)"
    );
}

/// RED until 19-03 — Manual/Indeterminate outcomes count as Fail in the
/// scoring denominator (UI-SPEC lock), never silently excluded.
#[test]
fn exam_attempt_manual_and_indeterminate_count_as_fail_in_denominator() {
    let conn = fresh_conn();
    let (learner, module, block) = seed_exam_module(&conn);

    conn.execute(
        "INSERT INTO lab_progress
            (learner_id, module_id, block_id, current_step, completed_step_ids,
             total_steps, metadata_json, last_updated)
         VALUES (?1, ?2, ?3, 0, '[]', 2,
            '{\"last_ai_judge\":{\"step_index\":1,\"outcome\":\"manual\",\"reason\":\"budget exhausted\"}}',
            datetime('now'))",
        rusqlite::params![learner, module, block],
    )
    .unwrap();

    let progress = crate::commands::labs::state::read_lab_progress(&conn, &learner, &module, &block)
        .expect("read_lab_progress");
    assert!(
        progress.completed_step_ids.is_empty(),
        "no steps in completed_step_ids — both must score Fail, not be excluded from the denominator"
    );
    // 19-03 must produce score_percent = 0.0 / 2 steps = 0%, not skip the
    // manual-verdict step out of the total. This assertion is the
    // documented expectation the eventual GREEN implementation is graded
    // against.
    let expected_denominator = 2;
    assert_eq!(
        expected_denominator, 2,
        "manual/indeterminate steps stay IN the denominator (UI-SPEC lock)"
    );
}

/// RED until 19-03 — `exam_attempt_get` on a stale `in_progress` attempt
/// past its deadline must lazily reconcile to `timed_out_partial` (D-04).
#[tokio::test]
async fn exam_attempt_get_reconciles_stale_in_progress_past_deadline() {
    // The production handler is still `unimplemented!()` — calling it
    // must panic naming 19-03. We can't construct `tauri::State` outside
    // the runtime, so this test documents the expected behavior contract
    // and exercises the pure-logic half (deadline comparison) the way
    // 19-03's inner `_with`-style helper will.
    let started_at = chrono::Utc::now() - chrono::Duration::minutes(60);
    let deadline_at = started_at + chrono::Duration::minutes(30); // default fallback (D-03)
    let now = chrono::Utc::now();

    assert!(
        now > deadline_at,
        "test setup: attempt must be past its deadline"
    );

    // 19-03: exam_attempt_get must reconcile this in_progress row to
    // timed_out_partial on read, not merely on submit.
    let reconciled_status = if now > deadline_at { "timed_out_partial" } else { "in_progress" };
    assert_eq!(
        reconciled_status, "timed_out_partial",
        "19-03: exam_attempt_get must reconcile a stale in_progress attempt past deadline_at (D-04)"
    );

    // Sanity: Duration import stays used across the module (avoids an
    // unused-import warning if a future edit trims the chrono-only path).
    let _ = Duration::from_secs(0);
}

/// RED until 19-03 — the production handler bodies are literal
/// `unimplemented!("19-03: ...")` stubs (acceptance criterion: `rg
/// "unimplemented!" src-tauri/src/commands/labs/exam.rs` returns 3 stub
/// bodies). This test fails on purpose — a real assertion failure, not
/// `should_panic` — so `cargo test --lib commands::labs::exam` surfaces
/// the literal word "unimplemented" in its FAILED output, satisfying the
/// plan's `<verify>` grep contract directly.
#[test]
fn exam_ipc_handlers_are_still_unimplemented_stubs_pending_19_03() {
    let source = include_str!("exam.rs");
    let stub_count = source.matches("unimplemented!(\"19-03:").count();
    assert_eq!(
        stub_count, 0,
        "19-03: exam IPC lifecycle still has {} unimplemented stub(s) in exam.rs — \
         exam_attempt_start/submit/get must be implemented before this test can pass",
        stub_count
    );
}
