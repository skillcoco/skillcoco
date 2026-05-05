use crate::db::models::{LearningPath, ModuleProgress, SRCard};
use crate::learning::adaptive::{update_mastery, BKTParams, MASTERY_THRESHOLD};
use crate::learning::path::{all_prerequisites_mastered, parse_edges_json};
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── Phase 3 IPC Structs (Wave 0 stubs — camelCase serde required per FIX-02) ──

/// Answer to a single MCQ question. Matches by option ID, not index (shuffle-safe).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizAnswer {
    pub question_id: String,
    pub selected_option_id: String,
}

/// Request to submit a completed quiz attempt.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitQuizRequest {
    pub module_id: String,
    pub track_id: String,
    pub block_id: String,
    pub answers: Vec<QuizAnswer>,
}

/// Per-question review entry returned after quiz submit.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizQuestionReview {
    pub question_id: String,
    pub stem: String,
    pub learner_option_id: String,
    pub correct_option_id: String,
    pub is_correct: bool,
    pub explanation: String,
}

/// Result returned from submit_quiz.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitQuizResult {
    pub score_percent: f64,
    pub passed: bool,
    pub mastery_level: f64,
    pub module_completed: bool,
    pub newly_unlocked_module_ids: Vec<String>,
    pub cards_created: usize,
    pub review: Vec<QuizQuestionReview>,
}

/// Request to rate a flash card (SM-2 quality signal + optional BKT reinforcement).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateFlashCardRequest {
    pub block_id: String,
    pub card_id: String,
    pub module_id: String,
    pub quality: u8, // 1-5; >= 4 = "good/easy"
}

/// Mark-lesson-complete stub. Fields finalized in 03-05 Task 1.
/// Declared as marker struct here so the camelCase serde test FAILS in Wave 0
/// (no camelCase fields to assert) and turns GREEN when 03-05 adds module_id + block_id.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkLessonCompleteRequest;

// ── Phase 3 stub function signatures ──

/// Submit a completed quiz attempt. Wave 2 (03-04 Task 2) implements.
pub async fn submit_quiz_stub(_req: SubmitQuizRequest) -> Result<SubmitQuizResult, String> {
    Err("Wave 2 (03-04) implements submit_quiz".to_string())
}

/// Rate a flash card. Wave 2 (03-04 Task 3) implements.
pub async fn rate_flash_card_stub(
    _req: RateFlashCardRequest,
) -> Result<serde_json::Value, String> {
    Err("Wave 2 (03-04) implements rate_flash_card".to_string())
}

/// Generate SR cards from a module's flash_cards blocks.
/// Wave 2 (03-04 Task 2) implements; stub returns Err so tests FAIL.
pub fn generate_sr_cards_from_flash_blocks(
    _conn: &rusqlite::Connection,
    _module_id: &str,
) -> Result<usize, String> {
    Err("Wave 2 (03-04) implements generate_sr_cards_from_flash_blocks".to_string())
}

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

// ── FIX-04: Streak Update Helper ──

/// Update the streak on `learning_tracks` for the given track.
///
/// Logic:
/// - If `last_activity_date` IS NULL: first activity ever — set `streak_days = 1`.
/// - If `date(last_activity_date) = date('now')`: same calendar day — no-op (don't double-count).
/// - If `last_activity_date >= datetime('now', '-1 day')` AND different calendar day (yesterday):
///   increment `streak_days += 1`.
/// - If `last_activity_date < datetime('now', '-1 day')` (gap > 24h, missed day):
///   reset `streak_days = 1`.
///
/// In all non-no-op cases, also sets `last_activity_date = datetime('now')`.
///
/// Returns the resulting `streak_days` value after the update.
pub fn update_streak(conn: &rusqlite::Connection, track_id: &str) -> Result<i32, String> {
    // Read current streak state
    let (current_streak, last_activity): (i32, Option<String>) = conn
        .query_row(
            "SELECT COALESCE(streak_days, 0), last_activity_date FROM learning_tracks WHERE id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("update_streak: failed to read track {}: {}", track_id, e))?;

    match last_activity {
        None => {
            // First activity ever
            conn.execute(
                "UPDATE learning_tracks SET streak_days = 1, last_activity_date = datetime('now') WHERE id = ?1",
                [track_id],
            ).map_err(|e| format!("update_streak: {}", e))?;
            Ok(1)
        }
        Some(_) => {
            // Check if last_activity_date is today (same calendar day — no-op)
            let is_today: bool = conn
                .query_row(
                    "SELECT date(last_activity_date) = date('now') FROM learning_tracks WHERE id = ?1",
                    [track_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if is_today {
                // Same calendar day — idempotent, don't double count
                return Ok(current_streak);
            }

            // Check if last_activity_date was within the past 24h (yesterday's streak continues)
            let within_24h: bool = conn
                .query_row(
                    "SELECT last_activity_date >= datetime('now', '-1 day') FROM learning_tracks WHERE id = ?1",
                    [track_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            let new_streak = if within_24h {
                current_streak + 1
            } else {
                1 // Gap > 24h — reset
            };

            conn.execute(
                "UPDATE learning_tracks SET streak_days = ?1, last_activity_date = datetime('now') WHERE id = ?2",
                rusqlite::params![new_streak, track_id],
            ).map_err(|e| format!("update_streak: {}", e))?;

            Ok(new_streak)
        }
    }
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

// ── LOOP-01..03: Complete Module Exercises (relocated from commands/ai.rs) ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteExercisesRequest {
    pub module_id: String,
    pub track_id: String,
    pub scores: Vec<f64>,
}

/// Result returned after completing module exercises.
///
/// Extended in Plan 01-03 with `newly_unlocked_module_ids` and `mastery_level` so the
/// frontend can update state without a full reload.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteExercisesResult {
    pub mastery_level: f64,
    pub module_completed: bool,
    pub newly_unlocked_module_ids: Vec<String>,
    pub cards_created: usize,
}

/// Complete module exercises: update BKT mastery (LOOP-01), unlock dependents (LOOP-02),
/// and auto-generate SR cards on first mastery crossing (LOOP-03).
///
/// Mutex lock semantics (from RESEARCH.md Pitfall 2):
/// - Lock acquired ONCE before step 1.
/// - No `.await` in this function (it is `fn`, not `async fn`), so no deadlock risk.
/// - Lock is held for the full synchronous operation (all DB reads/writes in one lock).
///
/// Note: Phase 1 does not call AI for scoring — scores are passed in from the frontend
/// (which receives them from the evaluate_response IPC call). This function processes
/// the aggregated scores.
#[tauri::command]
pub fn complete_module_exercises(
    state: State<AppState>,
    request: CompleteExercisesRequest,
) -> Result<CompleteExercisesResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // 1. Get learner profile ID
    let learner_id: String = db.conn
        .query_row("SELECT id FROM learner_profiles LIMIT 1", [], |row| row.get(0))
        .map_err(|e| format!("No profile found: {}", e))?;

    // 2. Read prior mastery from DB
    let prior_mastery: f64 = db.conn
        .query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
            rusqlite::params![request.module_id, learner_id],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    // 3. Compute aggregate score and is_correct from passed-in scores
    let avg_score = if request.scores.is_empty() {
        0.0
    } else {
        request.scores.iter().sum::<f64>() / request.scores.len() as f64
    };
    // Scores come in 0-100 range from evaluate_response
    let is_correct = avg_score >= 70.0;

    // 4. Ensure module_progress row exists (upsert)
    let progress_id = uuid::Uuid::new_v4().to_string();
    db.conn.execute(
        "INSERT INTO module_progress (id, module_id, learner_id, status, score, started_at)
         VALUES (?1, ?2, ?3, 'in_progress', ?4, datetime('now'))
         ON CONFLICT(module_id, learner_id) DO UPDATE SET
           score = ?4",
        rusqlite::params![progress_id, request.module_id, learner_id, avg_score],
    ).map_err(|e| e.to_string())?;

    // 5. Apply BKT mastery update (LOOP-01)
    let transition = apply_mastery_update(&db.conn, &learner_id, &request.module_id, prior_mastery, is_correct)?;

    let mut newly_unlocked = Vec::new();
    let mut cards_created = 0;

    if transition.became_completed {
        // 6. Find the path_id for the track so check_unlock_modules can load edges_json
        let path_id: Option<String> = db.conn.query_row(
            "SELECT id FROM learning_paths WHERE track_id = ?1 ORDER BY version DESC LIMIT 1",
            [&request.track_id],
            |row| row.get(0),
        ).ok();

        if let Some(path_id) = path_id {
            // 7. Unlock dependent modules (LOOP-02)
            newly_unlocked = check_unlock_modules(
                &db.conn,
                &learner_id,
                &path_id,
                &request.module_id,
            )?;
        }

        // 8. Auto-generate SR cards from module objectives (LOOP-03)
        // Load objectives_json from the modules table
        let objectives_json: String = db.conn
            .query_row(
                "SELECT objectives_json FROM modules WHERE id = ?1",
                [&request.module_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "[]".to_string());

        let objectives: Vec<String> = serde_json::from_str(&objectives_json)
            .unwrap_or_default();

        cards_created = generate_sr_cards_for_module(&db.conn, &request.module_id, &objectives)?;
        log::info!(
            "complete_module_exercises: module {} mastered, {} SR cards generated",
            request.module_id,
            cards_created
        );

        // 9. Update streak (FIX-04) — called when module mastered for first time
        if let Err(e) = update_streak(&db.conn, &request.track_id) {
            log::warn!("update_streak failed for track {}: {}", request.track_id, e);
        }

        // 10. Update track progress_percent
        let total_modules: i64 = db.conn
            .query_row(
                "SELECT COUNT(*) FROM modules m JOIN learning_paths lp ON m.path_id = lp.id WHERE lp.track_id = ?1",
                [&request.track_id],
                |row| row.get(0),
            )
            .unwrap_or(1);
        let completed_modules: i64 = db.conn
            .query_row(
                "SELECT COUNT(*) FROM module_progress mp
                 JOIN modules m ON mp.module_id = m.id
                 JOIN learning_paths lp ON m.path_id = lp.id
                 WHERE lp.track_id = ?1 AND mp.status = 'completed'",
                [&request.track_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let pct = if total_modules > 0 {
            (completed_modules as f64 / total_modules as f64) * 100.0
        } else {
            0.0
        };
        db.conn.execute(
            "UPDATE learning_tracks SET progress_percent = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![pct, request.track_id],
        ).ok();
    }

    Ok(CompleteExercisesResult {
        mastery_level: transition.new_mastery,
        module_completed: transition.became_completed || transition.new_mastery >= MASTERY_THRESHOLD,
        newly_unlocked_module_ids: newly_unlocked,
        cards_created,
    })
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

    // ── FIX-04: update_streak tests ──

    fn setup_streak_db() -> Connection {
        let conn = setup_test_db();
        // Apply migrations so learning_tracks has streak_days + last_activity_date
        crate::db::migrations::apply_migrations(&conn).expect("migrations must succeed for streak tests");
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Tester')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('tk1', 'lp1', 'Rust', 'programming', 'Learn Rust')",
            [],
        ).unwrap();
        conn
    }

    #[test]
    fn update_streak_first_activity() {
        // NULL last_activity_date → streak=1, last_activity_date set to now
        let conn = setup_streak_db();

        let new_streak = update_streak(&conn, "tk1").expect("update_streak should succeed");
        assert_eq!(new_streak, 1, "first activity must set streak_days=1");

        let (streak, last_date): (i32, Option<String>) = conn.query_row(
            "SELECT streak_days, last_activity_date FROM learning_tracks WHERE id = 'tk1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(streak, 1);
        assert!(last_date.is_some(), "last_activity_date must be set after first activity");
    }

    #[test]
    fn update_streak_within_24h() {
        // last_activity_date 12h ago + different calendar day → increment
        // Seed with yesterday's date and streak=3
        let conn = setup_streak_db();
        conn.execute(
            "UPDATE learning_tracks SET streak_days = 3, last_activity_date = datetime('now', '-12 hours') WHERE id = 'tk1'",
            [],
        ).unwrap();

        // Only run this test if 12h ago was actually a different calendar day
        let is_diff_day: bool = conn.query_row(
            "SELECT date(datetime('now', '-12 hours')) != date('now')",
            [],
            |row| row.get(0),
        ).unwrap_or(false);

        if is_diff_day {
            let new_streak = update_streak(&conn, "tk1").expect("update_streak should succeed");
            assert_eq!(new_streak, 4, "streak within 24h on different calendar day should increment to 4");
        } else {
            // Same calendar day — no-op path, streak stays at 3
            let new_streak = update_streak(&conn, "tk1").expect("update_streak should succeed");
            assert_eq!(new_streak, 3, "same-day no-op: streak stays at 3");
        }
    }

    #[test]
    fn update_streak_after_24h() {
        // last_activity_date 30h ago → reset to 1
        let conn = setup_streak_db();
        conn.execute(
            "UPDATE learning_tracks SET streak_days = 5, last_activity_date = datetime('now', '-30 hours') WHERE id = 'tk1'",
            [],
        ).unwrap();

        let new_streak = update_streak(&conn, "tk1").expect("update_streak should succeed");
        assert_eq!(new_streak, 1, "gap > 24h must reset streak to 1");

        let stored: i32 = conn.query_row(
            "SELECT streak_days FROM learning_tracks WHERE id = 'tk1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(stored, 1);
    }

    #[test]
    fn update_streak_same_day_idempotent() {
        // last_activity_date 5 minutes ago → same calendar day → no-op, streak stays at 2
        let conn = setup_streak_db();
        conn.execute(
            "UPDATE learning_tracks SET streak_days = 2, last_activity_date = datetime('now', '-5 minutes') WHERE id = 'tk1'",
            [],
        ).unwrap();

        let new_streak = update_streak(&conn, "tk1").expect("update_streak should succeed");
        assert_eq!(new_streak, 2, "same-day completion must not increment streak (no double-count)");
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

// ── Phase 3 TDD scaffolds (Wave 0 — all tests must FAIL until Wave 2 implements) ──

#[cfg(test)]
mod phase3_tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    // ── camelCase serde tests for Phase 3 IPC structs ──
    // These PASS in Wave 0 for structs with real fields (FIX-02 contract must hold from day one).

    #[test]
    fn test_submit_quiz_request_camel_case() {
        let req = SubmitQuizRequest {
            module_id: "mod-1".to_string(),
            track_id: "trk-1".to_string(),
            block_id: "blk-1".to_string(),
            answers: vec![],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("moduleId"), "must serialize to moduleId");
        assert!(json.contains("trackId"), "must serialize to trackId");
        assert!(json.contains("blockId"), "must serialize to blockId");
    }

    #[test]
    fn test_submit_quiz_result_camel_case() {
        let result = SubmitQuizResult {
            score_percent: 75.0,
            passed: true,
            mastery_level: 0.8,
            module_completed: true,
            newly_unlocked_module_ids: vec![],
            cards_created: 0,
            review: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("scorePercent"), "must serialize to scorePercent");
        assert!(json.contains("masteryLevel"), "must serialize to masteryLevel");
        assert!(json.contains("moduleCompleted"), "must serialize to moduleCompleted");
        assert!(json.contains("newlyUnlockedModuleIds"), "must serialize to newlyUnlockedModuleIds");
        assert!(json.contains("cardsCreated"), "must serialize to cardsCreated");
    }

    #[test]
    fn test_quiz_answer_camel_case() {
        let answer = QuizAnswer {
            question_id: "q1".to_string(),
            selected_option_id: "o2".to_string(),
        };
        let json = serde_json::to_string(&answer).unwrap();
        assert!(json.contains("questionId"), "must serialize to questionId");
        assert!(json.contains("selectedOptionId"), "must serialize to selectedOptionId");
    }

    #[test]
    fn test_rate_flash_card_camel_case() {
        let req = RateFlashCardRequest {
            block_id: "blk-1".to_string(),
            card_id: "fc-1".to_string(),
            module_id: "mod-1".to_string(),
            quality: 4,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("blockId"), "must serialize to blockId");
        assert!(json.contains("cardId"), "must serialize to cardId");
        assert!(json.contains("moduleId"), "must serialize to moduleId");
    }

    /// FAILS in Wave 0: MarkLessonCompleteRequest is a marker struct with no camelCase
    /// fields to assert. GREEN in 03-05 Task 1 when module_id + block_id are finalized.
    #[test]
    fn test_mark_lesson_complete_request_camel_case() {
        let req = MarkLessonCompleteRequest;
        let json = serde_json::to_string(&req).unwrap();
        // These fields don't exist on the marker stub — assertions FAIL in Wave 0.
        assert!(json.contains("moduleId"), "must serialize to moduleId (fails until 03-05 adds fields)");
        assert!(json.contains("blockId"), "must serialize to blockId (fails until 03-05 adds fields)");
    }

    // ── BKT removal / quiz scoring tests ──
    // These FAIL in Wave 0 because implementation hasn't landed yet.

    /// FAILS: complete_module_exercises still calls apply_mastery_update (Phase 1 wiring).
    /// GREEN in 03-04 Task 1 when BKT is removed from complete_module_exercises.
    #[test]
    fn complete_exercises_does_not_update_mastery() {
        panic!("WAVE 2 STUB — implement BKT removal from complete_module_exercises (03-04 Task 1)");
    }

    /// FAILS: score_quiz stub doesn't exist yet.
    /// GREEN in 03-04 Task 2 when quiz scoring by option_id is implemented.
    #[test]
    fn quiz_scoring_option_id_based() {
        panic!("WAVE 2 STUB — implement quiz scoring by option_id (03-04 Task 2)");
    }

    /// FAILS: 70% threshold logic not implemented yet.
    /// GREEN in 03-04 Task 2.
    #[test]
    fn quiz_70_percent_pass_threshold() {
        panic!("WAVE 2 STUB — implement 70% pass threshold in submit_quiz (03-04 Task 2)");
    }

    /// FAILS: submit_quiz_stub returns Err; BKT is not yet called from quiz path.
    /// GREEN in 03-04 Task 2.
    #[test]
    fn submit_quiz_calls_bkt() {
        panic!("WAVE 2 STUB — implement BKT call from submit_quiz (03-04 Task 2)");
    }

    /// FAILS: submit_quiz_stub returns Err; check_unlock_modules not yet called.
    /// GREEN in 03-04 Task 2.
    #[test]
    fn submit_quiz_unlocks_module() {
        panic!("WAVE 2 STUB — implement module unlock from submit_quiz (03-04 Task 2)");
    }

    // ── Integration stubs (full-pipeline tests) ──
    // These three represent the negative-coverage and idempotency contracts.

    /// Full pipeline: pass quiz -> BKT update -> module unlock -> SR cards -> streak.
    /// FAILS in Wave 0 stub; GREEN in 03-04 Task 2.
    #[test]
    fn integration_submit_quiz_full_pipeline() {
        panic!("WAVE 2 STUB — implement submit_quiz pipeline then assert BKT+unlock+SR+streak");
    }

    /// Failing quiz: no unlock, mastery reflects negative BKT update.
    /// FAILS in Wave 0 stub; GREEN in 03-04 Task 2.
    #[test]
    fn integration_quiz_fail_no_unlock() {
        panic!("WAVE 2 STUB — implement submit_quiz pipeline then assert no unlock on fail");
    }

    /// Concurrent submit / double-click negative coverage.
    /// Second submit must be idempotent (no double-BKT, UNIQUE constraint respected).
    /// FAILS in Wave 0 stub; GREEN in 03-04 Task 2.
    #[test]
    fn test_quiz_submit_idempotent() {
        panic!("WAVE 2 STUB — implement submit_quiz idempotency then assert double-click no-ops");
    }

    // ── SR cards from flash blocks ──

    /// FAILS: generate_sr_cards_from_flash_blocks returns Err in Wave 0.
    /// GREEN in 03-04 Task 2.
    #[test]
    fn sr_from_flash_cards_inserts() {
        panic!("WAVE 2 STUB — implement generate_sr_cards_from_flash_blocks inserts");
    }

    /// FAILS: generate_sr_cards_from_flash_blocks returns Err in Wave 0.
    /// GREEN in 03-04 Task 2 when idempotency guard implemented.
    #[test]
    fn sr_from_flash_cards_idempotent() {
        panic!("WAVE 2 STUB — implement generate_sr_cards_from_flash_blocks idempotency");
    }

    /// FAILS: rate_flash_card_stub returns Err; above-threshold guard not implemented.
    /// GREEN in 03-04 Task 3.
    #[test]
    fn flash_card_no_bkt_above_threshold() {
        panic!("WAVE 2 STUB — implement rate_flash_card above-threshold no-op guard");
    }
}
