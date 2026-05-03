// Full pipeline integration test — Plan 01-03 (LOOP-01..04) makes this fully green.
//
// This test exercises the full data pipeline against a temp-file SQLite DB:
//   profile -> track -> path (template) -> exercise -> mastery -> unlock -> SR -> review
//
// Uses the new helpers from Plan 01-03:
//   apply_mastery_update() — LOOP-01
//   check_unlock_modules() — LOOP-02
//   generate_sr_cards_for_module() — LOOP-03

use learnforge_lib::commands::learning::{apply_mastery_update, check_unlock_modules, generate_sr_cards_for_module};
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

    // ── 6. LOOP-01: apply_mastery_update — ASSERT mastery_level > 0.0 ──
    // Simulate exercise completion: 10 correct answers to push mastery above 0.7 threshold
    let mut prior = 0.0;
    let mut became_completed = false;
    for _ in 0..20 {
        let transition = apply_mastery_update(&db.conn, &profile_id, &mod_a, prior, true)
            .expect("apply_mastery_update must succeed");
        if transition.became_completed {
            became_completed = true;
        }
        prior = transition.new_mastery;
        if prior >= 0.7 {
            break;
        }
    }

    let stored_mastery: f64 = db.conn
        .query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
            params![mod_a, profile_id],
            |row| row.get(0),
        )
        .expect("Module progress must exist");

    assert!(
        stored_mastery > 0.0,
        "mastery_level must be > 0.0 after correct exercise attempts (LOOP-01)"
    );
    assert!(
        became_completed,
        "became_completed must flip true when mastery crosses 0.7 threshold"
    );

    // ── 7. LOOP-02: check_unlock_modules — ASSERT mod_b status = 'available' ──
    let unlocked = check_unlock_modules(&db.conn, &profile_id, &path_id, &mod_a)
        .expect("check_unlock_modules must succeed");

    assert!(
        unlocked.contains(&mod_b),
        "mod_b must be in the unlocked list after mod_a crosses mastery threshold (LOOP-02)"
    );

    let mod_b_status: String = db.conn
        .query_row(
            "SELECT status FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
            params![mod_b, profile_id],
            |row| row.get(0),
        )
        .expect("mod_b progress must exist");

    assert_eq!(
        mod_b_status, "available",
        "mod_b must be 'available' after unlock (LOOP-02)"
    );

    // ── 8. LOOP-03: generate_sr_cards_for_module — ASSERT sr_cards COUNT >= 1 ──
    // Module A has objectives: ["Understand ownership"]
    let objectives = vec!["Understand ownership".to_string()];
    let cards_created = generate_sr_cards_for_module(&db.conn, &mod_a, &objectives)
        .expect("generate_sr_cards_for_module must succeed");

    assert!(cards_created >= 1, "At least 1 SR card must be created for mod_a (LOOP-03)");

    let card_count: i64 = db.conn
        .query_row(
            "SELECT COUNT(*) FROM sr_cards WHERE module_id = ?1",
            [&mod_a],
            |row| row.get(0),
        )
        .unwrap_or(0);

    assert!(
        card_count >= 1,
        "sr_cards table must have at least 1 card for mod_a (LOOP-03)"
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
    // Use repetitions=1 so new_interval = 6.0 (second review, quality=4)
    // Note: SM-2 with repetitions=0 gives interval=1.0 (first review, always 1 day)
    let sm2 = learnforge_lib::learning::spaced_repetition::sm2_calculate(4, 1, 2.5, 1.0);
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

    // SM-2 is wired — quality=4, repetitions=1 produces interval=6.0 (second review)
    assert!(
        interval_after > 1.0,
        "next_review must be updated by SM-2 (interval > 1.0 for quality=4 second review)"
    );

    drop(dir); // Keep TempDir alive until end
}
