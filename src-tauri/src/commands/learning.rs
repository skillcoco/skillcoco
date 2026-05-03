use crate::db::models::{LearningPath, ModuleProgress, SRCard};
use crate::learning::adaptive::{update_mastery, BKTParams, MASTERY_THRESHOLD};
use crate::learning::path::{all_prerequisites_mastered, parse_edges_json};
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── LOOP-01: BKT Mastery Update Helper ──

/// Outcome of a single mastery update step.
pub struct MasteryTransition {
    /// BKT posterior after this attempt.
    pub new_mastery: f64,
    /// True only on the first flip from < MASTERY_THRESHOLD to >= MASTERY_THRESHOLD.
    pub became_completed: bool,
    /// The mastery value before this update.
    pub prior_mastery: f64,
}

/// Apply one BKT update to `module_progress` and persist it.
///
/// - Computes `new_mastery = update_mastery(BKTParams::default(), prior_mastery, is_correct)`.
/// - Increments `attempts`.
/// - If mastery crosses `MASTERY_THRESHOLD` for the first time
///   (`became_completed = prior < threshold AND new >= threshold`), also sets
///   `status='completed'` and `completed_at=datetime('now')`.
///
/// The caller is responsible for holding or not holding the DB mutex; this
/// function takes a plain `&rusqlite::Connection` so it can be called inside
/// an already-locked block without a second lock attempt.
pub fn apply_mastery_update(
    conn: &rusqlite::Connection,
    learner_id: &str,
    module_id: &str,
    prior_mastery: f64,
    is_correct: bool,
) -> Result<MasteryTransition, String> {
    let params = BKTParams::default();
    let new_mastery = update_mastery(&params, prior_mastery, is_correct);
    let became_completed = prior_mastery < MASTERY_THRESHOLD && new_mastery >= MASTERY_THRESHOLD;

    if became_completed {
        conn.execute(
            "UPDATE module_progress
             SET mastery_level = ?1, attempts = attempts + 1,
                 status = 'completed', completed_at = datetime('now')
             WHERE module_id = ?2 AND learner_id = ?3",
            rusqlite::params![new_mastery, module_id, learner_id],
        )
        .map_err(|e| format!("apply_mastery_update: {}", e))?;
    } else {
        conn.execute(
            "UPDATE module_progress
             SET mastery_level = ?1, attempts = attempts + 1
             WHERE module_id = ?2 AND learner_id = ?3",
            rusqlite::params![new_mastery, module_id, learner_id],
        )
        .map_err(|e| format!("apply_mastery_update: {}", e))?;
    }

    Ok(MasteryTransition {
        new_mastery,
        became_completed,
        prior_mastery,
    })
}

// ── LOOP-02: Module Unlock Helper ──

/// Check which modules should be unlocked after `just_completed_module_id` was mastered.
///
/// Logic:
/// 1. Load `edges_json` for the learning path.
/// 2. Parse edges.
/// 3. Find candidate modules where `from == just_completed_module_id`.
/// 4. For each candidate `to`, call `all_prerequisites_mastered`.
/// 5. If all prereqs mastered AND current status is 'locked', UPDATE to 'available'.
///
/// Returns the list of newly-unlocked module IDs.
pub fn check_unlock_modules(
    conn: &rusqlite::Connection,
    learner_id: &str,
    path_id: &str,
    just_completed_module_id: &str,
) -> Result<Vec<String>, String> {
    // Load edges_json for this path
    let edges_json: String = conn
        .query_row(
            "SELECT edges_json FROM learning_paths WHERE id = ?1",
            [path_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("check_unlock_modules: failed to load path {}: {}", path_id, e))?;

    let edges = parse_edges_json(&edges_json)?;

    // Find candidate modules that just_completed_module_id points to
    let candidates: Vec<String> = edges
        .iter()
        .filter(|e| e.from == just_completed_module_id)
        .map(|e| e.to.clone())
        .collect();

    let mut unlocked = Vec::new();

    for candidate_id in &candidates {
        // Check all prerequisites of the candidate
        if !all_prerequisites_mastered(conn, learner_id, candidate_id, &edges)? {
            continue;
        }

        // Only unlock if currently locked
        let current_status: Option<String> = conn
            .query_row(
                "SELECT status FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
                rusqlite::params![candidate_id, learner_id],
                |row| row.get(0),
            )
            .ok();

        if current_status.as_deref() == Some("locked") {
            conn.execute(
                "UPDATE module_progress SET status = 'available'
                 WHERE module_id = ?1 AND learner_id = ?2",
                rusqlite::params![candidate_id, learner_id],
            )
            .map_err(|e| format!("check_unlock_modules: unlock update failed: {}", e))?;
            unlocked.push(candidate_id.clone());
        }
    }

    Ok(unlocked)
}

// ── LOOP-03: SR Card Auto-Generation Helper ──

/// Generate spaced repetition cards for a module from its objectives.
///
/// **Idempotent**: if SR cards already exist for `module_id`, returns 0 immediately
/// (no duplicates). Capped at 5 cards per module to avoid spam.
///
/// Cards use a simple Phase 1 template:
/// - front: "What is the key idea of: {objective}?"
/// - back:  "{objective}" (the objective itself)
/// - next_review: datetime('now') (due immediately)
///
/// Returns the number of cards inserted (0 if already existed).
pub fn generate_sr_cards_for_module(
    conn: &rusqlite::Connection,
    module_id: &str,
    objectives: &[String],
) -> Result<usize, String> {
    // Idempotency guard: if cards already exist for this module, skip
    let existing_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sr_cards WHERE module_id = ?1",
            [module_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if existing_count > 0 {
        return Ok(0);
    }

    let cap = objectives.len().min(5);
    let mut inserted = 0;

    for objective in &objectives[..cap] {
        let card_id = uuid::Uuid::new_v4().to_string();
        let concept = objective.chars().take(200).collect::<String>();
        let front = format!("What is the key idea of: {}?", objective);
        let back = objective.clone();

        conn.execute(
            "INSERT INTO sr_cards (id, module_id, concept, card_type, front, back,
                                   interval_days, ease_factor, repetitions,
                                   next_review)
             VALUES (?1, ?2, ?3, 'concept', ?4, ?5, 1.0, 2.5, 0, datetime('now'))",
            rusqlite::params![card_id, module_id, concept, front, back],
        )
        .map_err(|e| format!("generate_sr_cards_for_module: insert failed: {}", e))?;
        inserted += 1;
    }

    Ok(inserted)
}

/// Typed request struct for update_module_progress.
/// Replaces the prior serde_json::Value approach to ensure camelCase IPC contract.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct UpdateProgressRequest {
    pub module_id: String,
    pub status: String,
    pub score: Option<f64>,
    pub time_spent: Option<i64>,
}

#[tauri::command]
pub fn get_path(state: State<AppState>, track_id: String) -> Result<LearningPath, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.conn
        .query_row(
            "SELECT id, track_id, version, generated_by_model, modules_json, edges_json, estimated_hours, created_at FROM learning_paths WHERE track_id = ?1 ORDER BY version DESC LIMIT 1",
            [&track_id],
            |row| {
                Ok(LearningPath {
                    id: row.get(0)?,
                    track_id: row.get(1)?,
                    version: row.get(2)?,
                    generated_by_model: row.get(3)?,
                    modules_json: row.get(4)?,
                    edges_json: row.get(5)?,
                    estimated_hours: row.get(6)?,
                    created_at: row.get(7)?,
                })
            },
        )
        .map_err(|e| format!("No learning path found for track: {}", e))
}

#[tauri::command]
pub fn get_module_progress(
    state: State<AppState>,
    track_id: String,
) -> Result<Vec<ModuleProgress>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = db
        .conn
        .prepare(
            "SELECT mp.id, mp.module_id, mp.learner_id, mp.status, mp.score, mp.time_spent, mp.attempts, mp.mastery_level, mp.started_at, mp.completed_at
             FROM module_progress mp
             JOIN modules m ON mp.module_id = m.id
             JOIN learning_paths lp ON m.path_id = lp.id
             WHERE lp.track_id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let progress = stmt
        .query_map([&track_id], |row| {
            Ok(ModuleProgress {
                id: row.get(0)?,
                module_id: row.get(1)?,
                learner_id: row.get(2)?,
                status: row.get(3)?,
                score: row.get(4)?,
                time_spent: row.get(5)?,
                attempts: row.get(6)?,
                mastery_level: row.get(7)?,
                started_at: row.get(8)?,
                completed_at: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(progress)
}

#[tauri::command]
pub fn update_module_progress(
    state: State<AppState>,
    progress: UpdateProgressRequest,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.conn
        .execute(
            "UPDATE module_progress SET status = ?1, updated_at = datetime('now') WHERE module_id = ?2",
            rusqlite::params![progress.status, progress.module_id],
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn get_due_cards(state: State<AppState>) -> Result<Vec<SRCard>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = db
        .conn
        .prepare(
            "SELECT id, module_id, concept, card_type, front, back, interval_days, ease_factor, repetitions, next_review, last_review
             FROM sr_cards
             WHERE next_review <= datetime('now')
             ORDER BY next_review ASC
             LIMIT 50",
        )
        .map_err(|e| e.to_string())?;

    let cards = stmt
        .query_map([], |row| {
            Ok(SRCard {
                id: row.get(0)?,
                module_id: row.get(1)?,
                concept: row.get(2)?,
                card_type: row.get(3)?,
                front: row.get(4)?,
                back: row.get(5)?,
                interval_days: row.get(6)?,
                ease_factor: row.get(7)?,
                repetitions: row.get(8)?,
                next_review: row.get(9)?,
                last_review: row.get(10)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(cards)
}

/// Result returned by `submit_review` — provides the updated SM-2 scheduling info
/// so the frontend can display "Next review in N days" without a second query.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitReviewResult {
    pub new_interval_days: f64,
    pub next_review: String, // ISO datetime string
    pub ease_factor: f64,
}

/// Typed request for submit_review (replacing serde_json::Value).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitReviewRequest {
    pub card_id: String,
    pub quality: u8,
}

#[tauri::command]
pub fn submit_review(
    state: State<AppState>,
    result: SubmitReviewRequest,
) -> Result<SubmitReviewResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let card_id = &result.card_id;
    let quality = result.quality as i32;

    // Get current card
    let card: SRCard = db
        .conn
        .query_row(
            "SELECT id, module_id, concept, card_type, front, back, interval_days, ease_factor, repetitions, next_review, last_review FROM sr_cards WHERE id = ?1",
            [card_id],
            |row| {
                Ok(SRCard {
                    id: row.get(0)?,
                    module_id: row.get(1)?,
                    concept: row.get(2)?,
                    card_type: row.get(3)?,
                    front: row.get(4)?,
                    back: row.get(5)?,
                    interval_days: row.get(6)?,
                    ease_factor: row.get(7)?,
                    repetitions: row.get(8)?,
                    next_review: row.get(9)?,
                    last_review: row.get(10)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    // Calculate new SM-2 values
    let sm2_result = crate::learning::spaced_repetition::sm2_calculate(
        quality,
        card.repetitions,
        card.ease_factor,
        card.interval_days,
    );

    // Update card
    db.conn
        .execute(
            "UPDATE sr_cards SET interval_days = ?1, ease_factor = ?2, repetitions = ?3, next_review = datetime('now', '+' || ?4 || ' days'), last_review = datetime('now') WHERE id = ?5",
            rusqlite::params![
                sm2_result.interval,
                sm2_result.ease_factor,
                sm2_result.repetitions,
                sm2_result.interval as i64,
                card_id,
            ],
        )
        .map_err(|e| e.to_string())?;

    // Query the updated next_review timestamp and build result
    let next_review: String = db.conn
        .query_row(
            "SELECT next_review FROM sr_cards WHERE id = ?1",
            [card_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    Ok(SubmitReviewResult {
        new_interval_days: sm2_result.interval,
        next_review,
        ease_factor: sm2_result.ease_factor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::CREATE_TABLES;
    use rusqlite::Connection;

    // ── Schema helper for in-memory test DB ──

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_TABLES).unwrap();
        conn
    }

    fn seed_linear_path(conn: &Connection) -> (String, String, String, String, String) {
        // learner, path, mod_a, mod_b, mod_c
        let learner_id = "learner-1".to_string();
        let path_id = "path-1".to_string();
        let mod_a = "mod-a".to_string();
        let mod_b = "mod-b".to_string();
        let mod_c = "mod-c".to_string();

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('learner-1', 'Test')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('track-1', 'learner-1', 'Rust', 'programming', 'Learn Rust')",
            [],
        ).unwrap();

        let edges_json = serde_json::json!([
            {"from": mod_a, "to": mod_b},
            {"from": mod_b, "to": mod_c},
        ]).to_string();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path-1', 'track-1', ?1, '[]', 'test')",
            [&edges_json],
        ).unwrap();

        for (mid, title, i) in [(&mod_a, "A", 0), (&mod_b, "B", 1), (&mod_c, "C", 2)] {
            conn.execute(
                "INSERT INTO modules (id, path_id, title, ordering) VALUES (?1, 'path-1', ?2, ?3)",
                rusqlite::params![mid, title, i],
            ).unwrap();
            let status = if i == 0 { "available" } else { "locked" };
            conn.execute(
                "INSERT INTO module_progress (id, module_id, learner_id, status) VALUES (?1, ?2, 'learner-1', ?3)",
                rusqlite::params![uuid::Uuid::new_v4().to_string(), mid, status],
            ).unwrap();
        }

        (learner_id, path_id, mod_a, mod_b, mod_c)
    }

    // ── Task 1: Mastery update tests ──

    #[test]
    fn mastery_update_persists() {
        let conn = setup_test_db();
        let (_l, _p, mod_a, _, _) = seed_linear_path(&conn);

        let transition = apply_mastery_update(&conn, "learner-1", &mod_a, 0.0, true)
            .expect("apply_mastery_update should succeed");

        assert!(transition.new_mastery > 0.0, "new_mastery must be > 0 after correct answer");

        let stored: f64 = conn.query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = 'learner-1'",
            [&mod_a],
            |row| row.get(0),
        ).unwrap();
        assert!((stored - transition.new_mastery).abs() < 1e-9, "DB must reflect new mastery");
    }

    #[test]
    fn mastery_crosses_threshold() {
        let conn = setup_test_db();
        let (_l, _p, mod_a, _, _) = seed_linear_path(&conn);

        let mut prior = 0.0;
        let mut became_completed = false;

        // Repeatedly apply correct answers until threshold is crossed
        for _ in 0..20 {
            let t = apply_mastery_update(&conn, "learner-1", &mod_a, prior, true).unwrap();
            if t.became_completed {
                became_completed = true;
            }
            prior = t.new_mastery;
            if prior >= crate::learning::adaptive::MASTERY_THRESHOLD {
                break;
            }
        }

        assert!(became_completed, "became_completed must flip true when mastery crosses MASTERY_THRESHOLD");

        let completed_at: Option<String> = conn.query_row(
            "SELECT completed_at FROM module_progress WHERE module_id = ?1 AND learner_id = 'learner-1'",
            [&mod_a],
            |row| row.get(0),
        ).unwrap();
        assert!(completed_at.is_some(), "completed_at must be set when became_completed=true");
    }

    // ── Task 1: Unlock tests ──

    #[test]
    fn unlock_linear() {
        let conn = setup_test_db();
        let (_l, _p, mod_a, mod_b, _) = seed_linear_path(&conn);

        // Mark mod_a as mastered
        conn.execute(
            "UPDATE module_progress SET mastery_level = 0.8, status = 'completed' WHERE module_id = ?1 AND learner_id = 'learner-1'",
            [&mod_a],
        ).unwrap();

        let unlocked = check_unlock_modules(&conn, "learner-1", "path-1", &mod_a)
            .expect("check_unlock_modules should succeed");

        assert!(unlocked.contains(&mod_b), "mod_b must be unlocked after mod_a mastered");

        // mod_c should NOT be unlocked (mod_b prerequisite not mastered)
        let mod_c_status: String = conn.query_row(
            "SELECT status FROM module_progress WHERE module_id = 'mod-c' AND learner_id = 'learner-1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(mod_c_status, "locked", "mod_c must remain locked");
    }

    #[test]
    fn unlock_diamond() {
        // Diamond: a->b, a->c, b->d, c->d
        let conn = setup_test_db();

        conn.execute("INSERT INTO learner_profiles (id, display_name) VALUES ('learner-1', 'T')", []).unwrap();
        conn.execute("INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('track-1', 'learner-1', 'x', 'programming', 'x')", []).unwrap();

        let edges_json = serde_json::json!([
            {"from": "a", "to": "b"},
            {"from": "a", "to": "c"},
            {"from": "b", "to": "d"},
            {"from": "c", "to": "d"},
        ]).to_string();

        conn.execute(
            "INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path-1', 'track-1', ?1, '[]', 'test')",
            [&edges_json],
        ).unwrap();

        for mid in ["a", "b", "c", "d"] {
            conn.execute("INSERT INTO modules (id, path_id, title) VALUES (?1, 'path-1', ?1)", [mid]).unwrap();
        }

        // Initially: a=completed/mastered, b=completed/mastered, c=locked (not mastered), d=locked
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('p1','a','learner-1','completed', 0.8)", []).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('p2','b','learner-1','completed', 0.8)", []).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('p3','c','learner-1','locked', 0.2)", []).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('p4','d','learner-1','locked', 0.0)", []).unwrap();

        // After b completes: d should still be locked (c not mastered)
        let unlocked_after_b = check_unlock_modules(&conn, "learner-1", "path-1", "b").unwrap();
        assert!(!unlocked_after_b.contains(&"d".to_string()), "d must not unlock when c is not mastered");

        // Now master c, then call unlock for c
        conn.execute("UPDATE module_progress SET mastery_level = 0.8, status = 'completed' WHERE module_id = 'c' AND learner_id = 'learner-1'", []).unwrap();
        let unlocked_after_c = check_unlock_modules(&conn, "learner-1", "path-1", "c").unwrap();
        assert!(unlocked_after_c.contains(&"d".to_string()), "d must unlock when both b and c are mastered");
    }

    // ── Task 2: SR card generation tests ──

    fn setup_sr_test_db() -> Connection {
        let conn = setup_test_db();
        // Insert learner and module for SR tests
        conn.execute("INSERT INTO learner_profiles (id, display_name) VALUES ('learner-1', 'T')", []).unwrap();
        conn.execute("INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('track-1', 'learner-1', 'x', 'programming', 'x')", []).unwrap();
        conn.execute("INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path-1', 'track-1', '[]', '[]', 'test')", []).unwrap();
        conn.execute("INSERT INTO modules (id, path_id, title) VALUES ('mod-a', 'path-1', 'A')", []).unwrap();
        conn
    }

    #[test]
    fn sr_card_generation() {
        let conn = setup_sr_test_db();
        let objectives = vec![
            "Understand pods".to_string(),
            "Use kubectl".to_string(),
        ];

        let count = generate_sr_cards_for_module(&conn, "mod-a", &objectives).unwrap();
        assert_eq!(count, 2, "should generate 2 SR cards (one per objective)");

        let db_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sr_cards WHERE module_id = 'mod-a'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(db_count, 2);

        // Verify SM-2 defaults
        let interval: f64 = conn.query_row(
            "SELECT interval_days FROM sr_cards WHERE module_id = 'mod-a' LIMIT 1",
            [],
            |row| row.get(0),
        ).unwrap();
        assert!((interval - 1.0).abs() < 1e-9, "default interval_days must be 1.0");

        let ease: f64 = conn.query_row(
            "SELECT ease_factor FROM sr_cards WHERE module_id = 'mod-a' LIMIT 1",
            [],
            |row| row.get(0),
        ).unwrap();
        assert!((ease - 2.5).abs() < 1e-9, "default ease_factor must be 2.5");
    }

    #[test]
    fn sr_no_duplicate_on_second_master_update() {
        let conn = setup_sr_test_db();
        let objectives = vec!["Understand pods".to_string()];

        // First generation
        let count1 = generate_sr_cards_for_module(&conn, "mod-a", &objectives).unwrap();
        assert_eq!(count1, 1);

        // Second call (idempotency guard)
        let count2 = generate_sr_cards_for_module(&conn, "mod-a", &objectives).unwrap();
        assert_eq!(count2, 0, "second call must be a no-op (idempotency)");

        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sr_cards WHERE module_id = 'mod-a'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(total, 1, "total cards must remain 1 after idempotent second call");
    }

    // ── Prior tests (IPC deserialization) ──

    #[test]
    fn test_update_progress_request_deserializes_camel_case() {
        // Simulates TypeScript sending: { trackId, moduleId, status, score, timeSpent }
        let json = r#"{"moduleId":"m1","status":"completed","score":0.9,"timeSpent":120}"#;
        let req: UpdateProgressRequest = serde_json::from_str(json)
            .expect("UpdateProgressRequest must deserialize from camelCase JSON");
        assert_eq!(req.module_id, "m1");
        assert_eq!(req.status, "completed");
        assert_eq!(req.score, Some(0.9));
        assert_eq!(req.time_spent, Some(120));
    }

    #[test]
    fn test_update_progress_request_optional_fields() {
        // Minimal payload — score and timeSpent are optional
        let json = r#"{"moduleId":"m2","status":"in_progress"}"#;
        let req: UpdateProgressRequest = serde_json::from_str(json)
            .expect("UpdateProgressRequest must accept missing optional fields");
        assert_eq!(req.module_id, "m2");
        assert_eq!(req.score, None);
        assert_eq!(req.time_spent, None);
    }

    // ── Task 3: submit_review returns SubmitReviewResult ──

    #[test]
    fn submit_review_result_shape() {
        // Verify SubmitReviewResult serializes correctly to camelCase
        let result = SubmitReviewResult {
            new_interval_days: 6.0,
            next_review: "2026-05-10T00:00:00Z".to_string(),
            ease_factor: 2.6,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("newIntervalDays"), "must serialize to newIntervalDays (camelCase)");
        assert!(json.contains("nextReview"), "must serialize to nextReview (camelCase)");
        assert!(json.contains("easeFactor"), "must serialize to easeFactor (camelCase)");
    }
}
