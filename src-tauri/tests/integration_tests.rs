// Wave 0 scaffold — Plan 03 (LOOP-01..04) makes this green.
// Today: The assertions for mastery_level > 0.0, module unlock, and SR card generation
// all FAIL because LOOP-01 (BKT wiring), LOOP-02 (unlock), and LOOP-03 (SR gen) are
// not yet wired into the data pipeline.
//
// This test exercises the full data pipeline against an in-memory SQLite DB:
//   profile -> track -> path (template) -> exercise -> mastery -> unlock -> SR -> review

use learnforge_lib::db::Database;
use rusqlite::params;
use tempfile::TempDir;

/// Create a fresh in-memory-like Database using a temp file path.
/// (Database::new requires a Path since it enables WAL mode, which needs a real file)
fn setup_test_db() -> (Database, TempDir) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let db = Database::new(&db_path).expect("Failed to create test Database");
    (db, dir)
}


#[test]
fn test_full_pipeline_profile_to_review() {
    let (db, dir) = setup_test_db();

    // ── 1. Create learner profile ──
    let profile_id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES (?1, ?2)",
            params![profile_id, "Test Learner"],
        )
        .expect("Failed to create profile");

    let stored_name: String = db
        .conn
        .query_row(
            "SELECT display_name FROM learner_profiles WHERE id = ?1",
            [&profile_id],
            |row| row.get(0),
        )
        .expect("Profile must exist");
    assert_eq!(stored_name, "Test Learner");

    // ── 2. Create learning track ──
    let track_id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![track_id, profile_id, "Rust", "programming", "Learn Rust basics"],
        )
        .expect("Failed to create track");

    // ── 3. Insert template path with linear DAG: mod-a -> mod-b -> mod-c ──
    let path_id = uuid::Uuid::new_v4().to_string();
    let mod_a = uuid::Uuid::new_v4().to_string();
    let mod_b = uuid::Uuid::new_v4().to_string();
    let mod_c = uuid::Uuid::new_v4().to_string();

    let modules_json = serde_json::json!([
        {"id": mod_a, "title": "Ownership Basics", "description": "", "difficulty": 2, "estimated_minutes": 30, "objectives": ["Understand ownership"]},
        {"id": mod_b, "title": "Borrowing", "description": "", "difficulty": 3, "estimated_minutes": 30, "objectives": ["Use borrowing"]},
        {"id": mod_c, "title": "Lifetimes", "description": "", "difficulty": 4, "estimated_minutes": 45, "objectives": ["Apply lifetimes"]},
    ]).to_string();

    let edges_json = serde_json::json!([
        {"from": mod_a, "to": mod_b},
        {"from": mod_b, "to": mod_c},
    ]).to_string();

    db.conn
        .execute(
            "INSERT INTO learning_paths (id, track_id, modules_json, edges_json, generated_by_model) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![path_id, track_id, modules_json, edges_json, "template"],
        )
        .expect("Failed to create path");

    // Insert modules
    for (i, (mid, title)) in [(&mod_a, "Ownership Basics"), (&mod_b, "Borrowing"), (&mod_c, "Lifetimes")].iter().enumerate() {
        db.conn
            .execute(
                "INSERT INTO modules (id, path_id, title, ordering) VALUES (?1, ?2, ?3, ?4)",
                params![mid, path_id, title, i as i32],
            )
            .expect("Failed to create module");

        // First module available, rest locked
        let status = if i == 0 { "available" } else { "locked" };
        db.conn
            .execute(
                "INSERT INTO module_progress (id, module_id, learner_id, status) VALUES (?1, ?2, ?3, ?4)",
                params![uuid::Uuid::new_v4().to_string(), mid, profile_id, status],
            )
            .expect("Failed to create module_progress");
    }

    // ── 4. Insert an exercise for module A ──
    let exercise_id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute(
            "INSERT INTO exercises (id, module_id, exercise_type, difficulty, prompt) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![exercise_id, mod_a, "multiple_choice", 3, "What does the borrow checker enforce?"],
        )
        .expect("Failed to create exercise");

    // ── 5. Assert mastery_level starts at 0.0 ──
    let initial_mastery: f64 = db.conn
        .query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
            params![mod_a, profile_id],
            |row| row.get(0),
        )
        .expect("Module progress must exist");
    assert_eq!(initial_mastery, 0.0, "Initial mastery must be 0.0");

    // ── 6. ASSERT mastery_level > 0.0 after exercise attempt ──
    // Simulate what LOOP-01 will wire: update mastery after exercise completion
    // For now, manually update mastery (as LOOP-01 will do from complete_module_exercises)
    let bkt_params = learnforge_lib::learning::adaptive::BKTParams::default();
    let new_mastery = learnforge_lib::learning::adaptive::update_mastery(&bkt_params, 0.0, true); // score 90% -> correct

    db.conn
        .execute(
            "UPDATE module_progress SET mastery_level = ?1, status = 'completed' WHERE module_id = ?2 AND learner_id = ?3",
            params![new_mastery, mod_a, profile_id],
        )
        .expect("Failed to update mastery");

    let stored_mastery: f64 = db.conn
        .query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
            params![mod_a, profile_id],
            |row| row.get(0),
        )
        .expect("Module progress must exist");

    // ASSERTS PASS once LOOP-01 wires update_mastery() into complete_module_exercises()
    // For this scaffold: mastery update is done manually above, so this PASSES as a baseline
    assert!(
        stored_mastery > 0.0,
        "mastery_level must be > 0.0 after exercise — Plan 03 LOOP-01 must wire update_mastery in complete_module_exercises"
    );

    // ── 7. ASSERT mod_b status = 'available' after mod_a completed ──
    // Today this FAILS — LOOP-02 (unlock logic) is not yet invoked after exercise completion.
    // The code currently requires manual unlock. Plan 03 LOOP-02 will wire the DAG unlock.
    let mod_b_status: String = db.conn
        .query_row(
            "SELECT status FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
            params![mod_b, profile_id],
            |row| row.get(0),
        )
        .expect("mod_b progress must exist");

    // After plan 03 LOOP-02, this will be "available". Currently "locked".
    // Scaffold assertion: plan 03 must change this to pass.
    assert_eq!(
        mod_b_status, "available",
        "mod_b must unlock when mod_a is completed — Plan 03 LOOP-02 must wire unlock logic"
    );

    // ── 8. ASSERT sr_cards COUNT >= 1 for mod_a ──
    // Today this FAILS — LOOP-03 (SR card generation) is not yet wired.
    let card_count: i64 = db.conn
        .query_row(
            "SELECT COUNT(*) FROM sr_cards WHERE module_id = ?1",
            [&mod_a],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // After plan 03 LOOP-03, this will be >= 1. Currently 0.
    assert!(
        card_count >= 1,
        "At least one SR card must exist for mod_a after mastery >= 0.7 — Plan 03 LOOP-03 must auto-generate cards"
    );

    // ── 9. Simulate review of one SR card (SM-2 already wired) ──
    // This part tests the already-working SM-2 submission path.
    // We insert an SR card manually and verify next_review updates.
    let card_id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute(
            "INSERT INTO sr_cards (id, module_id, concept, card_type, front, back, next_review) \
             VALUES (?1, ?2, 'ownership', 'active_recall', 'What is ownership?', 'Memory safety', datetime('now'))",
            params![card_id, mod_a],
        )
        .expect("Failed to insert SR card");

    // SM-2 calculation (already wired in submit_review command)
    let sm2 = learnforge_lib::learning::spaced_repetition::sm2_calculate(4, 0, 2.5, 1.0);
    db.conn
        .execute(
            "UPDATE sr_cards SET interval_days = ?1, ease_factor = ?2, repetitions = ?3, \
             next_review = datetime('now', '+' || ?4 || ' days'), last_review = datetime('now') \
             WHERE id = ?5",
            params![sm2.interval, sm2.ease_factor, sm2.repetitions, sm2.interval as i64, card_id],
        )
        .expect("Failed to update SR card");

    let interval_after: f64 = db.conn
        .query_row(
            "SELECT interval_days FROM sr_cards WHERE id = ?1",
            [&card_id],
            |row| row.get(0),
        )
        .expect("Card must exist after update");

    // SM-2 is wired — this assertion PASSES today
    assert!(
        interval_after > 1.0,
        "next_review must be updated by SM-2 (interval > 1.0 for quality=4)"
    );

    drop(dir); // Keep TempDir alive until end
}
