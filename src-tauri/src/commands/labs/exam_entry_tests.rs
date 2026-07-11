//! GREEN tests for `commands::labs::exam_entry` (Phase 19, 19-04).
//! Exercises `exam_blocks_for_track_conn` directly (mirrors 19-03's
//! `exam_tests.rs` seam pattern) — no `tauri::State` needed.

use super::*;

fn fresh_conn() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(crate::db::schema::CREATE_TABLES).unwrap();
    crate::db::migrations::apply_migrations(&conn).unwrap();
    conn
}

/// `payload_json.spec` shape `read_lab_spec_conn` tries first (mirrors
/// exam_tests.rs::exam_spec_json). `exam_flagged` controls whether the
/// `exam:` block is present (`Some(_)`) or absent (`None`, D-02).
fn lab_spec_json(exam_flagged: bool) -> serde_json::Value {
    let exam = if exam_flagged {
        serde_json::json!({ "timeLimitMinutes": 45, "passThresholdPct": 70.0 })
    } else {
        serde_json::Value::Null
    };
    serde_json::json!({
        "spec": {
            "slug": "fixture-lab",
            "title": "Fixture Lab",
            "image": "alpine",
            "dockerfile": null,
            "requiresDocker": false,
            "creates": [],
            "exam": exam,
            "steps": [
                {
                    "id": "step-1",
                    "title": "Step 1",
                    "prompt": "Do the thing.",
                    "check": { "kind": "file_state", "path": "out.txt" },
                    "hints": [],
                    "weight": 1.0
                }
            ]
        }
    })
}

fn seed_track_with_module(conn: &rusqlite::Connection, track: &str, path: &str) {
    conn.execute(
        "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Tester')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO learning_tracks (id, learner_id, topic, domain_module)
         VALUES (?1, 'lp1', 'k8s', 'kubernetes')",
        rusqlite::params![track],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO learning_paths (id, track_id) VALUES (?1, ?2)",
        rusqlite::params![path, track],
    )
    .unwrap();
}

fn seed_module(conn: &rusqlite::Connection, module_id: &str, path: &str, ordering: i64) {
    conn.execute(
        "INSERT INTO modules (id, path_id, title, ordering) VALUES (?1, ?2, 'Module', ?3)",
        rusqlite::params![module_id, path, ordering],
    )
    .unwrap();
}

fn seed_lab_block(
    conn: &rusqlite::Connection,
    block_id: &str,
    module_id: &str,
    ordering: i64,
    exam_flagged: bool,
) {
    conn.execute(
        "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
            params_json, payload_json, source_anchors_json, metadata_json, retry_count,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, 'lab', 'ready', '{}', ?4, '[]', '{}', 0,
            datetime('now'), datetime('now'))",
        rusqlite::params![block_id, module_id, ordering, lab_spec_json(exam_flagged).to_string()],
    )
    .unwrap();
}

/// One module owns an exam-flagged lab block, a sibling module owns a
/// non-exam lab block — `exam_blocks_for_track` returns exactly the
/// exam-flagged module's block_id, omitting the non-exam module entirely
/// (fail-closed).
#[test]
fn exam_blocks_for_track_returns_exam_flagged_module_only() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk1", "path1");
    seed_module(&conn, "mod-exam", "path1", 0);
    seed_module(&conn, "mod-plain", "path1", 1);
    seed_lab_block(&conn, "blk-exam", "mod-exam", 0, true);
    seed_lab_block(&conn, "blk-plain", "mod-plain", 0, false);

    let refs = exam_blocks_for_track_conn(&conn, "trk1").expect("query must succeed");

    assert_eq!(refs.len(), 1, "only the exam-flagged module must appear, got {:?}", refs);
    assert_eq!(
        refs[0],
        ExamBlockRef { module_id: "mod-exam".to_string(), block_id: "blk-exam".to_string() }
    );
}

/// A track with zero exam-flagged blocks (or zero modules) returns an
/// empty Vec — TrackView renders no Start Exam buttons.
#[test]
fn exam_blocks_for_track_returns_empty_vec_when_no_exam_blocks() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk-empty", "path-empty");
    seed_module(&conn, "mod-plain-only", "path-empty", 0);
    seed_lab_block(&conn, "blk-plain-only", "mod-plain-only", 0, false);

    let refs = exam_blocks_for_track_conn(&conn, "trk-empty").expect("query must succeed");
    assert!(refs.is_empty(), "no exam blocks in track must yield [], got {:?}", refs);

    // A completely unknown track_id also yields [].
    let refs_missing = exam_blocks_for_track_conn(&conn, "trk-does-not-exist").expect("query must succeed");
    assert!(refs_missing.is_empty());
}

/// block_type != 'lab' rows (e.g. quiz/section) are never considered, even
/// if their payload happens to contain an `exam` key.
#[test]
fn exam_blocks_for_track_ignores_non_lab_block_types() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk-quiz", "path-quiz");
    seed_module(&conn, "mod-quiz", "path-quiz", 0);
    conn.execute(
        "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
            params_json, payload_json, source_anchors_json, metadata_json, retry_count,
            created_at, updated_at)
         VALUES ('blk-quiz', 'mod-quiz', 0, 'quiz', 'ready', '{}', ?1, '[]', '{}', 0,
            datetime('now'), datetime('now'))",
        rusqlite::params![lab_spec_json(true).to_string()],
    )
    .unwrap();

    let refs = exam_blocks_for_track_conn(&conn, "trk-quiz").expect("query must succeed");
    assert!(refs.is_empty(), "quiz block_type must never surface as an exam entry point");
}

/// A malformed/unparseable lab payload is skipped, not fatal — a single
/// bad block must not crash the whole-track query (T-19-11).
#[test]
fn exam_blocks_for_track_skips_malformed_lab_payload_without_error() {
    let conn = fresh_conn();
    seed_track_with_module(&conn, "trk-bad", "path-bad");
    seed_module(&conn, "mod-bad", "path-bad", 0);
    seed_module(&conn, "mod-good", "path-bad", 1);
    conn.execute(
        "INSERT INTO module_blocks (id, module_id, ordering, block_type, status,
            params_json, payload_json, source_anchors_json, metadata_json, retry_count,
            created_at, updated_at)
         VALUES ('blk-bad', 'mod-bad', 0, 'lab', 'ready', '{}', 'not valid json', '[]', '{}', 0,
            datetime('now'), datetime('now'))",
        [],
    )
    .unwrap();
    seed_lab_block(&conn, "blk-good", "mod-good", 0, true);

    let refs = exam_blocks_for_track_conn(&conn, "trk-bad")
        .expect("a malformed lab payload must not error the whole query");
    assert_eq!(refs.len(), 1, "the good module's exam block must still surface, got {:?}", refs);
    assert_eq!(refs[0].module_id, "mod-good");
}
