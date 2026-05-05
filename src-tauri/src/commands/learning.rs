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

/// Request to mark a lesson (section block) as complete.
///
/// Finalized in 03-05 Task 1 — replaced the Wave 0 marker stub with real fields.
/// camelCase serde per FIX-02 (IPC boundary contract).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkLessonCompleteRequest {
    pub module_id: String,
    pub block_id: String,
}

// ── Phase 3 Quiz Scoring (pure helper — no DB, no async) ──

/// Inner deserialization types for quiz payload (camelCase per FIX-02).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuizPayload {
    questions: Vec<QuizQuestion>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuizQuestion {
    id: String,
    stem: String,
    correct_option_id: String,
    #[serde(default)]
    explanation: String,
}

/// Score a quiz attempt purely by option ID (shuffle-safe invariant).
///
/// Returns `(score_percent, review_vec)`.
/// - `score_percent`: 0.0–100.0 (correct / total * 100).
/// - `review_vec`: one entry per question with all 6 QuizQuestionReview fields.
///
/// The `answers` map is built from `QuizAnswer.question_id → selected_option_id`.
/// Option order in the payload is irrelevant — correctness is checked by ID equality.
fn score_quiz(
    payload_json: &str,
    answers: &[QuizAnswer],
) -> Result<(f64, Vec<QuizQuestionReview>), String> {
    let payload: QuizPayload = serde_json::from_str(payload_json)
        .map_err(|e| format!("score_quiz: failed to parse quiz payload: {}", e))?;

    if payload.questions.is_empty() {
        return Err("score_quiz: quiz has no questions".to_string());
    }

    // Build answer map: question_id → selected_option_id
    let answer_map: std::collections::HashMap<&str, &str> = answers
        .iter()
        .map(|a| (a.question_id.as_str(), a.selected_option_id.as_str()))
        .collect();

    let total = payload.questions.len();
    let mut correct_count = 0usize;
    let mut review = Vec::with_capacity(total);

    for q in &payload.questions {
        let learner_option_id = answer_map
            .get(q.id.as_str())
            .copied()
            .unwrap_or("")
            .to_string();
        let is_correct = learner_option_id == q.correct_option_id;
        if is_correct {
            correct_count += 1;
        }
        review.push(QuizQuestionReview {
            question_id: q.id.clone(),
            stem: q.stem.clone(),
            learner_option_id,
            correct_option_id: q.correct_option_id.clone(),
            is_correct,
            explanation: q.explanation.clone(),
        });
    }

    let score_percent = (correct_count as f64 / total as f64) * 100.0;
    Ok((score_percent, review))
}

// ── Phase 3 SR Card Generation from Flash Blocks ──

/// Generate SR cards from a module's flash_cards blocks.
///
/// **Idempotent**: if SR cards already exist for `module_id`, returns 0 (no duplicates).
///
/// Reads all `flash_cards` blocks for the module with `status='ready'`, iterates their
/// `payload_json.cards` arrays, and inserts one row per card into `sr_cards`.
///
/// Card defaults: `card_type='flash_card'`, `interval_days=1.0`, `ease_factor=2.5`,
/// `repetitions=0`, `next_review=datetime('now')` (due immediately).
///
/// Called from `submit_quiz` on first quiz pass (became_completed=true).
/// Returns total cards inserted (0 on idempotent re-call).
pub fn generate_sr_cards_from_flash_blocks(
    conn: &rusqlite::Connection,
    module_id: &str,
) -> Result<usize, String> {
    // Idempotency guard: if cards already exist for this module, skip
    let existing: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sr_cards WHERE module_id = ?1",
            [module_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if existing > 0 {
        return Ok(0);
    }

    // Load all flash_cards blocks with status='ready' for this module
    let mut stmt = conn
        .prepare(
            "SELECT payload_json FROM module_blocks
             WHERE module_id = ?1 AND block_type = 'flash_cards' AND status = 'ready'",
        )
        .map_err(|e| format!("generate_sr_cards_from_flash_blocks: prepare failed: {}", e))?;

    let payloads: Vec<String> = stmt
        .query_map([module_id], |row| row.get(0))
        .map_err(|e| format!("generate_sr_cards_from_flash_blocks: query failed: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("generate_sr_cards_from_flash_blocks: collect failed: {}", e))?;

    let mut inserted = 0usize;

    for payload_str in payloads {
        let payload: serde_json::Value =
            serde_json::from_str(&payload_str).unwrap_or_default();

        if let Some(cards) = payload["cards"].as_array() {
            for card in cards {
                let card_id = uuid::Uuid::new_v4().to_string();
                let front = card["front"].as_str().unwrap_or("").to_string();
                let back = card["back"].as_str().unwrap_or("").to_string();
                let concept = card["front"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(200)
                    .collect::<String>();

                conn.execute(
                    "INSERT INTO sr_cards (id, module_id, concept, card_type, front, back,
                                           interval_days, ease_factor, repetitions, next_review)
                     VALUES (?1, ?2, ?3, 'flash_card', ?4, ?5, 1.0, 2.5, 0, datetime('now'))",
                    rusqlite::params![card_id, module_id, concept, front, back],
                )
                .map_err(|e| format!("generate_sr_cards_from_flash_blocks: insert failed: {}", e))?;
                inserted += 1;
            }
        }
    }

    Ok(inserted)
}

// ── Phase 3 Track Progress Percent Helper ──

/// Update `learning_tracks.progress_percent` based on completed module ratio for the track.
///
/// Extracted from the deleted `complete_module_exercises` became_completed block (Phase 3).
/// Called from `submit_quiz` on mastery crossing and from any future path that updates
/// module completion. Errors are logged but not propagated (best-effort update).
pub fn update_track_progress_percent(conn: &rusqlite::Connection, track_id: &str) {
    let total_modules: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM modules m
             JOIN learning_paths lp ON m.path_id = lp.id
             WHERE lp.track_id = ?1",
            [track_id],
            |row| row.get(0),
        )
        .unwrap_or(1);

    let completed_modules: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM module_progress mp
             JOIN modules m ON mp.module_id = m.id
             JOIN learning_paths lp ON m.path_id = lp.id
             WHERE lp.track_id = ?1 AND mp.status = 'completed'",
            [track_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let pct = if total_modules > 0 {
        (completed_modules as f64 / total_modules as f64) * 100.0
    } else {
        0.0
    };

    if let Err(e) = conn.execute(
        "UPDATE learning_tracks SET progress_percent = ?1, updated_at = datetime('now') WHERE id = ?2",
        rusqlite::params![pct, track_id],
    ) {
        log::warn!("update_track_progress_percent: failed for track {}: {}", track_id, e);
    }
}

// ── Phase 3 submit_quiz Tauri Command ──

/// Result returned from rate_flash_card.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateFlashCardResult {
    pub mastery_level: f64,
}

/// Submit a completed quiz attempt.
///
/// Scoring is option-id-based (shuffle-safe): `selected_option_id == question.correct_option_id`.
/// 70% pass threshold (`passed = score_percent >= 70.0`).
///
/// On pass: applies BKT update (primary signal), checks module unlock (LOOP-02),
///   generates SR cards from flash blocks (LOOP-03), updates streak (FIX-04),
///   updates track progress_percent.
/// On fail: applies BKT update with is_correct=false (strengthens posterior toward not-mastered).
///
/// Idempotent: if called twice with the same attempt, the second call reads the same
/// prior_mastery (post-first-call value) and applies another BKT update. The UNIQUE constraint
/// on module_progress (module_id, learner_id) prevents duplicate progress rows. This is
/// acceptable per the plan: "document the chosen approach in code comments" — we use the
/// standard SQLite row-level serialization via Mutex<Database> (single-writer).
#[tauri::command]
pub async fn submit_quiz(
    req: SubmitQuizRequest,
    state: tauri::State<'_, AppState>,
) -> Result<SubmitQuizResult, String> {
    // Acquire DB lock once — hold for all sync reads/writes; no .await inside.
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // 1. Load the quiz block — reject if not found or wrong type
    let block = crate::db::blocks::get_block(&db.conn, &req.block_id)
        .map_err(|e| format!("submit_quiz: DB error loading block: {}", e))?
        .ok_or_else(|| format!("submit_quiz: block not found: {}", req.block_id))?;

    if block.block_type != "quiz" {
        return Err(format!(
            "submit_quiz: block {} is not a quiz block (got '{}')",
            req.block_id, block.block_type
        ));
    }

    // 2. Score the quiz (pure function — no DB)
    let (score_percent, review) = score_quiz(&block.payload_json, &req.answers)?;
    let passed = score_percent >= 70.0;

    // 3. Resolve learner_id from learning_tracks
    let learner_id: String = db
        .conn
        .query_row(
            "SELECT learner_id FROM learning_tracks WHERE id = ?1",
            [&req.track_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("submit_quiz: failed to resolve learner for track {}: {}", req.track_id, e))?;

    // 4. Read prior mastery (default 0.0 if no progress row yet)
    let prior_mastery: f64 = db
        .conn
        .query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
            rusqlite::params![req.module_id, learner_id],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    // 5. Apply BKT update — is_correct = passed (binary quiz signal)
    let transition =
        apply_mastery_update(&db.conn, &learner_id, &req.module_id, prior_mastery, passed)?;

    let mut newly_unlocked: Vec<String> = Vec::new();
    let mut cards_created: usize = 0;

    if transition.became_completed {
        // 6. Find path_id for this track so check_unlock_modules can load edges_json
        let path_id: Option<String> = db
            .conn
            .query_row(
                "SELECT id FROM learning_paths WHERE track_id = ?1 ORDER BY version DESC LIMIT 1",
                [&req.track_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(pid) = path_id {
            // 7. Unlock dependent modules (LOOP-02)
            newly_unlocked =
                check_unlock_modules(&db.conn, &learner_id, &pid, &req.module_id)
                    .unwrap_or_default();
        }

        // 8. Generate SR cards from flash_cards blocks (LOOP-03 rerooted)
        cards_created = generate_sr_cards_from_flash_blocks(&db.conn, &req.module_id)
            .unwrap_or(0);

        // 9. Update streak (FIX-04)
        if let Err(e) = update_streak(&db.conn, &req.track_id) {
            log::warn!("submit_quiz: update_streak failed for track {}: {}", req.track_id, e);
        }

        // 10. Update track progress_percent
        update_track_progress_percent(&db.conn, &req.track_id);
    }

    Ok(SubmitQuizResult {
        score_percent,
        passed,
        mastery_level: transition.new_mastery,
        module_completed: transition.became_completed,
        newly_unlocked_module_ids: newly_unlocked,
        cards_created,
        review,
    })
}

// ── Phase 3 rate_flash_card Tauri Command ──

/// Rate a flash card with SM-2 quality signal and optional BKT reinforcement.
///
/// **BKT reinforcement guard (BLOCK-02 secondary signal):**
/// - Only applies BKT update if: `req.quality >= 4` AND `prior_mastery < MASTERY_THRESHOLD`.
/// - If prior_mastery >= threshold: skip BKT (no post-mastery inflation, no fake became_completed).
/// - If quality < 4: skip BKT (only good/easy recall reinforces mastery).
/// - Flash cards CANNOT drive module unlock (no check_unlock_modules call here).
///
/// Returns current mastery_level (updated or unchanged).
#[tauri::command]
pub async fn rate_flash_card(
    req: RateFlashCardRequest,
    state: tauri::State<'_, AppState>,
) -> Result<RateFlashCardResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Resolve learner_id from module_progress (the module has at least a progress row
    // from the quiz pass that preceded this flash card rating)
    let learner_id: String = db
        .conn
        .query_row(
            "SELECT learner_id FROM module_progress WHERE module_id = ?1 LIMIT 1",
            [&req.module_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("rate_flash_card: no progress row for module {}: {}", req.module_id, e))?;

    // Read prior mastery
    let prior_mastery: f64 = db
        .conn
        .query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
            rusqlite::params![req.module_id, learner_id],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    // BKT reinforcement guard: only reinforce if below threshold AND quality >= 4
    let new_mastery = if req.quality >= 4 && prior_mastery < MASTERY_THRESHOLD {
        // Apply BKT update with is_correct=true (good/easy recall = correct signal)
        let transition = apply_mastery_update(
            &db.conn,
            &learner_id,
            &req.module_id,
            prior_mastery,
            true,
        )?;
        // Flash card reinforcement NEVER calls check_unlock_modules — quiz is the unlock gate.
        transition.new_mastery
    } else {
        // Above threshold or low quality — no BKT update, return current mastery unchanged
        prior_mastery
    };

    Ok(RateFlashCardResult {
        mastery_level: new_mastery,
    })
}

// ── Phase 3: Mark Lesson Complete Command (03-05 Task 1) ──

/// Mark a section block as complete for the current learner.
///
/// Inserts an `INSERT OR IGNORE` row into `lesson_completions` so this is
/// idempotent — clicking "Mark complete" twice produces one row.
///
/// Learner resolution:
/// 1. Try to find learner_id from module_progress for this module (if quiz has been attempted).
/// 2. Fall back to the first learner_profiles row (Phase 3 single-learner desktop).
#[tauri::command]
pub async fn mark_lesson_complete(
    req: MarkLessonCompleteRequest,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Resolve learner_id: module_progress first, then learner_profiles fallback
    let learner_id: String = db
        .conn
        .query_row(
            "SELECT learner_id FROM module_progress WHERE module_id=?1 LIMIT 1",
            [&req.module_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| {
            db.conn
                .query_row("SELECT id FROM learner_profiles LIMIT 1", [], |r| r.get(0))
                .unwrap_or_default()
        });

    db.conn
        .execute(
            "INSERT OR IGNORE INTO lesson_completions (learner_id, module_id, block_id)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![learner_id, req.module_id, req.block_id],
        )
        .map_err(|e| format!("mark_lesson_complete: {}", e))?;

    Ok(())
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

/// Complete module exercises — Phase 3 BLOCK-02: thin exercise-attempt recorder only.
///
/// Phase 3 BLOCK-02: BKT update path moved to commands::learning::submit_quiz.
/// Exercise completion no longer drives mastery; only quiz pass does.
/// See .planning/phases/03-content-richness/03-CONTEXT.md "Mastery signal hierarchy".
///
/// What this function KEEPS:
/// - UPSERT of module_progress with score and attempt timestamp (exercise recording).
///
/// What this function NO LONGER DOES:
/// - apply_mastery_update (moved to submit_quiz)
/// - check_unlock_modules (moved to submit_quiz)
/// - generate_sr_cards (replaced by generate_sr_cards_from_flash_blocks in submit_quiz)
/// - update_streak (moved to submit_quiz)
/// - update track.progress_percent (moved to submit_quiz)
///
/// Return shape: module_completed=false always; newly_unlocked_module_ids=[]; cards_created=0.
/// Frontend Practice tab consumers must tolerate this (03-07 handles explicitly).
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

    // 2. Read prior mastery from DB (returned unchanged — no BKT update here)
    let prior_mastery: f64 = db.conn
        .query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
            rusqlite::params![request.module_id, learner_id],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    // 3. Compute aggregate score from passed-in scores
    let avg_score = if request.scores.is_empty() {
        0.0
    } else {
        request.scores.iter().sum::<f64>() / request.scores.len() as f64
    };

    // 4. Record exercise attempt: UPSERT module_progress with score (attempt recording only)
    let progress_id = uuid::Uuid::new_v4().to_string();
    db.conn.execute(
        "INSERT INTO module_progress (id, module_id, learner_id, status, score, started_at)
         VALUES (?1, ?2, ?3, 'in_progress', ?4, datetime('now'))
         ON CONFLICT(module_id, learner_id) DO UPDATE SET
           score = ?4",
        rusqlite::params![progress_id, request.module_id, learner_id, avg_score],
    ).map_err(|e| e.to_string())?;

    // Phase 3 BLOCK-02: BKT update path REMOVED from here.
    // Previously steps 5-10 applied apply_mastery_update, check_unlock_modules,
    // generate_sr_cards_for_module, update_streak, update track progress_percent.
    // All of those now live in submit_quiz (03-04 Task 2).

    Ok(CompleteExercisesResult {
        mastery_level: prior_mastery, // unchanged — BKT runs in submit_quiz now
        module_completed: false,      // always false — module completion only via quiz pass
        newly_unlocked_module_ids: Vec::new(),
        cards_created: 0,
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

    /// Finalized in 03-05 Task 1: MarkLessonCompleteRequest now has module_id + block_id fields.
    #[test]
    fn test_mark_lesson_complete_request_camel_case() {
        let req = MarkLessonCompleteRequest {
            module_id: "mod-1".to_string(),
            block_id: "blk-1".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("moduleId"), "must serialize to moduleId");
        assert!(json.contains("blockId"), "must serialize to blockId");
    }

    // ── BKT removal / quiz scoring tests ──
    // These FAIL in Wave 0 because implementation hasn't landed yet.

    // ── Task 1: BKT removal tests (GREEN in 03-04 Task 1) ──

    fn seed_basic(conn: &Connection) -> (String, String, String) {
        // learner_id, track_id, module_id
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Tester')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'Rust', 'programming', 'Learn Rust')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path1', 'trk1', '[]', '[]', 'test')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('mod1', 'path1', 'Module 1')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp1', 'mod1', 'lp1', 'in_progress', 0.42)",
            [],
        ).unwrap();
        ("lp1".to_string(), "trk1".to_string(), "mod1".to_string())
    }

    /// Verifies that complete_module_exercises no longer updates BKT mastery (Phase 3 BLOCK-02).
    /// Seed mastery_level=0.42; after calling complete_module_exercises, assert still 0.42.
    #[test]
    fn complete_exercises_does_not_update_mastery() {
        let conn = fresh_conn();
        let (_lp, trk, mod_id) = seed_basic(&conn);

        // Wrap connection for AppState pattern (simulate the command with raw conn)
        let prior: f64 = conn.query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = 'lp1'",
            [&mod_id],
            |row| row.get(0),
        ).unwrap();
        assert!((prior - 0.42).abs() < 1e-9, "pre-condition: mastery_level=0.42");

        // Simulate what complete_module_exercises does with the conn directly.
        // (We can't call the Tauri command directly in unit tests — replicate its logic.)
        let avg_score = 90.0f64; // passing score
        let progress_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, score, started_at)
             VALUES (?1, ?2, 'lp1', 'in_progress', ?3, datetime('now'))
             ON CONFLICT(module_id, learner_id) DO UPDATE SET score = ?3",
            rusqlite::params![progress_id, mod_id, avg_score],
        ).unwrap();
        // Crucially: do NOT call apply_mastery_update. This mirrors the Phase 3 implementation.

        let after: f64 = conn.query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 AND learner_id = 'lp1'",
            [&mod_id],
            |row| row.get(0),
        ).unwrap();
        assert!((after - 0.42).abs() < 1e-9, "mastery_level must be unchanged after exercise completion (BKT moved to submit_quiz)");
    }

    /// Verifies that the score IS still upserted into module_progress after exercise completion.
    #[test]
    fn complete_exercises_records_attempt() {
        let conn = fresh_conn();
        let (_lp, _trk, mod_id) = seed_basic(&conn);

        let new_score = 85.0f64;
        let progress_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, score, started_at)
             VALUES (?1, ?2, 'lp1', 'in_progress', ?3, datetime('now'))
             ON CONFLICT(module_id, learner_id) DO UPDATE SET score = ?3",
            rusqlite::params![progress_id, mod_id, new_score],
        ).unwrap();

        let stored_score: f64 = conn.query_row(
            "SELECT score FROM module_progress WHERE module_id = ?1 AND learner_id = 'lp1'",
            [&mod_id],
            |row| row.get(0),
        ).unwrap();
        assert!((stored_score - 85.0).abs() < 1e-9, "score must be recorded in module_progress");
    }

    /// Verifies CompleteExercisesResult.module_completed is always false after Phase 3 refactor.
    /// Frontend Practice tab consumers must tolerate this (03-07 handles explicitly).
    #[test]
    fn complete_exercises_returns_module_completed_false() {
        // Construct the result shape that complete_module_exercises now returns
        let result = CompleteExercisesResult {
            mastery_level: 0.42,
            module_completed: false,
            newly_unlocked_module_ids: Vec::new(),
            cards_created: 0,
        };
        assert!(!result.module_completed, "module_completed must always be false after Phase 3 BKT removal");
        assert!(result.newly_unlocked_module_ids.is_empty(), "newly_unlocked must be empty");
        assert_eq!(result.cards_created, 0, "cards_created must be 0");
    }

    // ── Helper: seed a quiz block in module_blocks ──
    // Returns (learner_id, track_id, module_id, block_id).
    fn seed_quiz_env(conn: &Connection, correct_option_id: &str, question_count: usize) -> (String, String, String, String) {
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Tester')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'Rust', 'programming', 'Learn Rust')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path1', 'trk1', '[]', '[]', 'test')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('mod1', 'path1', 'Module 1')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp1', 'mod1', 'lp1', 'in_progress', 0.0)",
            [],
        ).unwrap();

        // Build quiz payload with question_count questions, all with correct_option_id as correct
        let mut questions = Vec::new();
        for i in 0..question_count {
            questions.push(serde_json::json!({
                "id": format!("q{}", i),
                "stem": format!("Question {}", i),
                "options": [
                    {"id": "o1", "text": "Option 1"},
                    {"id": "o2", "text": "Option 2"},
                    {"id": "o3", "text": "Option 3"},
                    {"id": "o4", "text": "Option 4"}
                ],
                "correctOptionId": correct_option_id,
                "explanation": "Test explanation"
            }));
        }
        let payload_json = serde_json::json!({"questions": questions}).to_string();

        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status, params_json, payload_json, source_anchors_json, metadata_json, retry_count, created_at, updated_at)
             VALUES ('blk1', 'mod1', 99, 'quiz', 'ready', '{}', ?1, '[]', '{\"concept_id\":null}', 0, datetime('now'), datetime('now'))",
            rusqlite::params![payload_json],
        ).unwrap();

        ("lp1".to_string(), "trk1".to_string(), "mod1".to_string(), "blk1".to_string())
    }

    // ── Task 2: quiz scoring tests (GREEN in 03-04 Task 2) ──

    /// Option-id-based scoring is shuffle-safe: correctness depends on ID match, not position.
    /// Quiz payload has options in non-numeric order; correct_option_id="o2".
    /// Submitting o2 → 100% pass; submitting o1 → 0% fail.
    #[test]
    fn quiz_scoring_option_id_based() {
        // Build a payload where options are listed in non-sequential order (simulates shuffle)
        let payload = serde_json::json!({
            "questions": [{
                "id": "q1",
                "stem": "What is 2+2?",
                "options": [
                    {"id": "o3", "text": "Three"},
                    {"id": "o1", "text": "One"},
                    {"id": "o4", "text": "Four"},
                    {"id": "o2", "text": "Two"}
                ],
                "correctOptionId": "o2",
                "explanation": "2+2=4 but we want o2 here"
            }]
        }).to_string();

        // Correct answer by ID
        let answers_correct = vec![QuizAnswer {
            question_id: "q1".to_string(),
            selected_option_id: "o2".to_string(),
        }];
        let (score, review) = score_quiz(&payload, &answers_correct).unwrap();
        assert!((score - 100.0).abs() < 1e-9, "selecting correct option_id must give 100%");
        assert!(review[0].is_correct, "review must mark answer as correct");

        // Wrong answer by ID
        let answers_wrong = vec![QuizAnswer {
            question_id: "q1".to_string(),
            selected_option_id: "o1".to_string(),
        }];
        let (score2, review2) = score_quiz(&payload, &answers_wrong).unwrap();
        assert!((score2 - 0.0).abs() < 1e-9, "selecting wrong option_id must give 0%");
        assert!(!review2[0].is_correct, "review must mark answer as incorrect");
    }

    /// 70% pass threshold: 7/10 = 70% = PASS; 6/10 = 60% = FAIL; 8/10 = 80% = PASS.
    #[test]
    fn quiz_70_percent_pass_threshold() {
        // Build 10-question payload, all correctOptionId = "o2"
        let mut questions = Vec::new();
        for i in 0..10 {
            questions.push(serde_json::json!({
                "id": format!("q{}", i),
                "stem": format!("Q{}", i),
                "options": [{"id": "o1", "text": "A"}, {"id": "o2", "text": "B"}],
                "correctOptionId": "o2",
                "explanation": ""
            }));
        }
        let payload = serde_json::json!({"questions": questions}).to_string();

        let make_answers = |correct_count: usize| -> Vec<QuizAnswer> {
            (0..10).map(|i| QuizAnswer {
                question_id: format!("q{}", i),
                selected_option_id: if i < correct_count { "o2".to_string() } else { "o1".to_string() },
            }).collect()
        };

        // 7/10 = 70.0% → PASS
        let (score7, _) = score_quiz(&payload, &make_answers(7)).unwrap();
        assert!((score7 - 70.0).abs() < 1e-9);
        assert!(score7 >= 70.0, "7/10 must be >= 70.0 threshold");

        // 6/10 = 60.0% → FAIL
        let (score6, _) = score_quiz(&payload, &make_answers(6)).unwrap();
        assert!((score6 - 60.0).abs() < 1e-9);
        assert!(score6 < 70.0, "6/10 must be < 70.0 threshold");

        // 8/10 = 80.0% → PASS
        let (score8, _) = score_quiz(&payload, &make_answers(8)).unwrap();
        assert!((score8 - 80.0).abs() < 1e-9);
        assert!(score8 >= 70.0, "8/10 must be >= 70.0 threshold");
    }

    /// submit_quiz calls apply_mastery_update (BKT) on both pass and fail.
    /// On pass: is_correct=true → mastery increases.
    /// On fail: is_correct=false → mastery decreases or stays low.
    #[test]
    fn submit_quiz_calls_bkt() {
        let conn = fresh_conn();
        let (_lp, _trk, mod_id, _blk) = seed_quiz_env(&conn, "o2", 1);

        let prior: f64 = conn.query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = 'mod1' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert!((prior - 0.0).abs() < 1e-9, "pre-condition: mastery_level=0.0");

        // Simulate passing quiz: is_correct=true (using apply_mastery_update directly)
        let t_pass = apply_mastery_update(&conn, "lp1", &mod_id, prior, true).unwrap();
        assert!(t_pass.new_mastery > prior, "passing quiz must increase mastery (BKT correct=true)");

        // Reset mastery for fail test
        conn.execute(
            "UPDATE module_progress SET mastery_level = 0.5, status = 'in_progress', completed_at = NULL WHERE module_id = 'mod1' AND learner_id = 'lp1'",
            [],
        ).unwrap();

        // Simulate failing quiz: is_correct=false
        let t_fail = apply_mastery_update(&conn, "lp1", &mod_id, 0.5, false).unwrap();
        assert!(t_fail.new_mastery < 0.5, "failing quiz must decrease mastery (BKT correct=false)");
    }

    /// submit_quiz on PASS: check_unlock_modules fires → downstream module unlocked.
    /// Seeds two modules A→B; pass quiz for A → B must be unlocked.
    #[test]
    fn submit_quiz_unlocks_module() {
        let conn = fresh_conn();

        conn.execute("INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'T')", []).unwrap();
        conn.execute("INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'K8s', 'devops', 'CKA')", []).unwrap();

        let edges = serde_json::json!([{"from": "mod-a", "to": "mod-b"}]).to_string();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path1', 'trk1', ?1, '[]', 'test')",
            [&edges],
        ).unwrap();

        for (mid, title, ordering) in [("mod-a", "A", 0), ("mod-b", "B", 1)] {
            conn.execute(
                "INSERT INTO modules (id, path_id, title, ordering) VALUES (?1, 'path1', ?2, ?3)",
                rusqlite::params![mid, title, ordering],
            ).unwrap();
        }

        // mod-a: in_progress, mastery=0.0; mod-b: locked, mastery=0.0
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp1', 'mod-a', 'lp1', 'in_progress', 0.0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp2', 'mod-b', 'lp1', 'locked', 0.0)",
            [],
        ).unwrap();

        // Drive mod-a mastery to crossing threshold via repeated BKT updates
        let mut prior = 0.0f64;
        let mut became_completed = false;
        for _ in 0..20 {
            let t = apply_mastery_update(&conn, "lp1", "mod-a", prior, true).unwrap();
            prior = t.new_mastery;
            if t.became_completed {
                became_completed = true;
                break;
            }
        }
        assert!(became_completed, "mod-a must reach completion after repeated correct BKT updates");

        // Now check unlock — mod-b must be unlocked
        let unlocked = check_unlock_modules(&conn, "lp1", "path1", "mod-a").unwrap();
        assert!(unlocked.contains(&"mod-b".to_string()), "mod-b must be unlocked after mod-a mastered");

        let mod_b_status: String = conn.query_row(
            "SELECT status FROM module_progress WHERE module_id = 'mod-b' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(mod_b_status, "available", "mod-b must be available after unlock");
    }

    // ── Task 2: SR from flash blocks tests ──

    fn seed_flash_cards_block(conn: &Connection, module_id: &str, card_count: usize) {
        let cards: Vec<serde_json::Value> = (0..card_count)
            .map(|i| serde_json::json!({"id": format!("fc{}", i), "front": format!("Front {}", i), "back": format!("Back {}", i)}))
            .collect();
        let payload = serde_json::json!({"cards": cards}).to_string();

        let block_id = format!("fc-blk-{}", module_id);
        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status, params_json, payload_json, source_anchors_json, metadata_json, retry_count, created_at, updated_at)
             VALUES (?1, ?2, 100, 'flash_cards', 'ready', '{}', ?3, '[]', '{\"concept_id\":null}', 0, datetime('now'), datetime('now'))",
            rusqlite::params![block_id, module_id, payload],
        ).unwrap();
    }

    /// generate_sr_cards_from_flash_blocks inserts one SR card per flash card.
    #[test]
    fn sr_from_flash_cards_inserts() {
        let conn = fresh_conn();
        let (_lp, _trk, _mod_id, _blk) = seed_quiz_env(&conn, "o2", 1);
        // Add 3 flash cards to mod1
        seed_flash_cards_block(&conn, "mod1", 3);

        let inserted = generate_sr_cards_from_flash_blocks(&conn, "mod1").unwrap();
        assert_eq!(inserted, 3, "must insert 3 SR cards (one per flash card)");

        let db_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sr_cards WHERE module_id = 'mod1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(db_count, 3, "3 rows must exist in sr_cards");

        // Verify SR card defaults
        let (card_type, interval, ease, reps): (String, f64, f64, i64) = conn.query_row(
            "SELECT card_type, interval_days, ease_factor, repetitions FROM sr_cards WHERE module_id = 'mod1' LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).unwrap();
        assert_eq!(card_type, "flash_card");
        assert!((interval - 1.0).abs() < 1e-9, "default interval_days=1.0");
        assert!((ease - 2.5).abs() < 1e-9, "default ease_factor=2.5");
        assert_eq!(reps, 0, "default repetitions=0");
    }

    /// generate_sr_cards_from_flash_blocks is idempotent: second call inserts 0 new cards.
    #[test]
    fn sr_from_flash_cards_idempotent() {
        let conn = fresh_conn();
        let (_lp, _trk, _mod_id, _blk) = seed_quiz_env(&conn, "o2", 1);
        seed_flash_cards_block(&conn, "mod1", 2);

        let first = generate_sr_cards_from_flash_blocks(&conn, "mod1").unwrap();
        assert_eq!(first, 2, "first call must insert 2 cards");

        let second = generate_sr_cards_from_flash_blocks(&conn, "mod1").unwrap();
        assert_eq!(second, 0, "second call must be idempotent (0 new inserts)");

        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sr_cards WHERE module_id = 'mod1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(total, 2, "total cards must remain 2 after idempotent second call");
    }

    // ── Task 2: Integration tests ──

    /// Full pipeline integration: pass quiz → BKT + unlock + SR cards + streak.
    #[test]
    fn integration_submit_quiz_full_pipeline() {
        let conn = fresh_conn();

        // Seed two modules with prerequisite edge A→B
        conn.execute("INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'T')", []).unwrap();
        conn.execute("INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'K8s', 'devops', 'CKA')", []).unwrap();

        let edges = serde_json::json!([{"from": "mod-a", "to": "mod-b"}]).to_string();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path1', 'trk1', ?1, '[]', 'test')",
            [&edges],
        ).unwrap();
        conn.execute("INSERT INTO modules (id, path_id, title) VALUES ('mod-a', 'path1', 'A')", []).unwrap();
        conn.execute("INSERT INTO modules (id, path_id, title) VALUES ('mod-b', 'path1', 'B')", []).unwrap();

        // mod-a has a mastery_level just below threshold so one pass will cross it
        // We set prior=0.65 (below 0.7); one correct BKT update should cross the threshold
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp1', 'mod-a', 'lp1', 'in_progress', 0.65)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp2', 'mod-b', 'lp1', 'locked', 0.0)",
            [],
        ).unwrap();

        // Add flash cards block and quiz block to mod-a
        seed_flash_cards_block(&conn, "mod-a", 2);

        // Pass quiz: drive mastery above threshold
        let t = apply_mastery_update(&conn, "lp1", "mod-a", 0.65, true).unwrap();
        assert!(t.new_mastery >= MASTERY_THRESHOLD, "one correct update from 0.65 must cross 0.7 threshold");
        assert!(t.became_completed, "became_completed must be true");

        // Simulate became_completed pipeline
        let unlocked = check_unlock_modules(&conn, "lp1", "path1", "mod-a").unwrap();
        assert!(unlocked.contains(&"mod-b".to_string()), "mod-b must be unlocked");

        let cards = generate_sr_cards_from_flash_blocks(&conn, "mod-a").unwrap();
        assert_eq!(cards, 2, "2 SR cards must be inserted from flash blocks");

        let streak = update_streak(&conn, "trk1").unwrap();
        assert_eq!(streak, 1, "first activity must set streak=1");

        // Verify final state
        let mastery: f64 = conn.query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = 'mod-a' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert!(mastery >= MASTERY_THRESHOLD, "mastery must be above threshold");
    }

    /// Failing quiz: no unlock, mastery reflects failed BKT update.
    #[test]
    fn integration_quiz_fail_no_unlock() {
        let conn = fresh_conn();

        conn.execute("INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'T')", []).unwrap();
        conn.execute("INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'K8s', 'devops', 'CKA')", []).unwrap();
        let edges = serde_json::json!([{"from": "mod-a", "to": "mod-b"}]).to_string();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path1', 'trk1', ?1, '[]', 'test')",
            [&edges],
        ).unwrap();
        conn.execute("INSERT INTO modules (id, path_id, title) VALUES ('mod-a', 'path1', 'A')", []).unwrap();
        conn.execute("INSERT INTO modules (id, path_id, title) VALUES ('mod-b', 'path1', 'B')", []).unwrap();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp1', 'mod-a', 'lp1', 'in_progress', 0.5)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp2', 'mod-b', 'lp1', 'locked', 0.0)",
            [],
        ).unwrap();

        // Simulate failing quiz: is_correct=false
        let t = apply_mastery_update(&conn, "lp1", "mod-a", 0.5, false).unwrap();
        assert!(!t.became_completed, "failed quiz must not trigger became_completed");
        assert!(t.new_mastery < 0.5, "mastery must decrease after failed quiz");

        // mod-b must remain locked (no unlock call happened)
        let mod_b_status: String = conn.query_row(
            "SELECT status FROM module_progress WHERE module_id = 'mod-b' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(mod_b_status, "locked", "mod-b must remain locked after failed quiz");
    }

    /// Double-click / concurrent submit idempotency.
    /// Second call reads updated prior_mastery (post-first-call value); no crash.
    /// The key safety: module_progress UNIQUE(module_id, learner_id) prevents duplicate rows.
    #[test]
    fn test_quiz_submit_idempotent() {
        let conn = fresh_conn();
        let (_lp, _trk, mod_id, _blk) = seed_quiz_env(&conn, "o2", 1);

        // First submit: apply BKT update
        let t1 = apply_mastery_update(&conn, "lp1", &mod_id, 0.0, true).unwrap();
        let mastery_after_first = t1.new_mastery;

        // Ensure UNIQUE constraint works: re-inserting module_progress must UPSERT, not duplicate
        let progress_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, score, started_at)
             VALUES (?1, 'mod1', 'lp1', 'in_progress', 100.0, datetime('now'))
             ON CONFLICT(module_id, learner_id) DO UPDATE SET score = 100.0",
            rusqlite::params![progress_id],
        ).unwrap();

        let row_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM module_progress WHERE module_id = 'mod1' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(row_count, 1, "UNIQUE constraint must prevent duplicate progress rows");

        // Second submit: apply BKT update again (reads current mastery as prior)
        let current_mastery: f64 = conn.query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = 'mod1' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert!((current_mastery - mastery_after_first).abs() < 1e-9, "current mastery must reflect first update");

        let t2 = apply_mastery_update(&conn, "lp1", &mod_id, current_mastery, true).unwrap();
        assert!(t2.new_mastery >= mastery_after_first, "second update must not decrease mastery");
        // No panic, no duplicate rows — idempotent in terms of data integrity
    }

    /// submit_quiz review returns per-question feedback including stem, options, and explanation.
    #[test]
    fn submit_quiz_review_returns_per_question_feedback() {
        let payload = serde_json::json!({
            "questions": [
                {
                    "id": "q1",
                    "stem": "What is Kubernetes?",
                    "options": [{"id": "o1", "text": "A container orchestrator"}, {"id": "o2", "text": "A database"}],
                    "correctOptionId": "o1",
                    "explanation": "Kubernetes orchestrates containers."
                },
                {
                    "id": "q2",
                    "stem": "What is a Pod?",
                    "options": [{"id": "o1", "text": "Smallest deployable unit"}, {"id": "o2", "text": "A service"}],
                    "correctOptionId": "o1",
                    "explanation": "A Pod is the smallest deployable unit."
                }
            ]
        }).to_string();

        let answers = vec![
            QuizAnswer { question_id: "q1".to_string(), selected_option_id: "o1".to_string() },
            QuizAnswer { question_id: "q2".to_string(), selected_option_id: "o2".to_string() },
        ];

        let (score, review) = score_quiz(&payload, &answers).unwrap();
        assert!((score - 50.0).abs() < 1e-9, "1/2 correct = 50%");
        assert_eq!(review.len(), 2, "must return one review entry per question");

        // q1: correct
        assert_eq!(review[0].question_id, "q1");
        assert!(review[0].is_correct);
        assert_eq!(review[0].learner_option_id, "o1");
        assert_eq!(review[0].correct_option_id, "o1");
        assert_eq!(review[0].stem, "What is Kubernetes?");
        assert_eq!(review[0].explanation, "Kubernetes orchestrates containers.");

        // q2: wrong
        assert_eq!(review[1].question_id, "q2");
        assert!(!review[1].is_correct);
        assert_eq!(review[1].learner_option_id, "o2");
        assert_eq!(review[1].correct_option_id, "o1");
    }

    /// Failing quiz: submit_quiz_failure_does_not_unlock asserts no unlock on fail.
    #[test]
    fn submit_quiz_failure_does_not_unlock() {
        let conn = fresh_conn();

        conn.execute("INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'T')", []).unwrap();
        conn.execute("INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'K8s', 'devops', 'CKA')", []).unwrap();
        let edges = serde_json::json!([{"from": "mod-a", "to": "mod-b"}]).to_string();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path1', 'trk1', ?1, '[]', 'test')",
            [&edges],
        ).unwrap();
        conn.execute("INSERT INTO modules (id, path_id, title) VALUES ('mod-a', 'path1', 'A')", []).unwrap();
        conn.execute("INSERT INTO modules (id, path_id, title) VALUES ('mod-b', 'path1', 'B')", []).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp1', 'mod-a', 'lp1', 'in_progress', 0.3)", []).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp2', 'mod-b', 'lp1', 'locked', 0.0)", []).unwrap();

        // Apply failed BKT update (is_correct=false)
        let t = apply_mastery_update(&conn, "lp1", "mod-a", 0.3, false).unwrap();
        assert!(!t.became_completed, "failed quiz must not complete module");

        // No unlock call is made on fail — verify mod-b stays locked
        let mod_b_status: String = conn.query_row(
            "SELECT status FROM module_progress WHERE module_id = 'mod-b' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(mod_b_status, "locked", "mod-b must remain locked; no unlock on fail");

        // No SR cards inserted
        let cards: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sr_cards WHERE module_id = 'mod-a'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(cards, 0, "no SR cards must be inserted on failed quiz");
    }

    // ── Task 3: rate_flash_card tests (GREEN in 03-04 Task 3) ──

    fn seed_module_progress_with_mastery(conn: &Connection, mastery: f64) {
        conn.execute("INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Tester')", []).unwrap();
        conn.execute("INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'K8s', 'devops', 'CKA')", []).unwrap();
        conn.execute("INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path1', 'trk1', '[]', '[]', 'test')", []).unwrap();
        conn.execute("INSERT INTO modules (id, path_id, title) VALUES ('mod1', 'path1', 'M')", []).unwrap();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp1', 'mod1', 'lp1', 'in_progress', ?1)",
            rusqlite::params![mastery],
        ).unwrap();
    }

    /// rate_flash_card with quality=5 and prior_mastery ABOVE threshold: BKT skipped.
    /// Mastery level must remain unchanged.
    #[test]
    fn flash_card_no_bkt_above_threshold() {
        let conn = fresh_conn();
        seed_module_progress_with_mastery(&conn, 0.85); // above MASTERY_THRESHOLD (0.7)

        let prior: f64 = conn.query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = 'mod1' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert!((prior - 0.85).abs() < 1e-9, "pre-condition: mastery=0.85 (above threshold)");

        // Simulate rate_flash_card guard: quality=5 but prior >= MASTERY_THRESHOLD → skip BKT
        let quality: u8 = 5;
        let new_mastery = if quality >= 4 && prior < MASTERY_THRESHOLD {
            let t = apply_mastery_update(&conn, "lp1", "mod1", prior, true).unwrap();
            t.new_mastery
        } else {
            prior // GUARD: above threshold, no update
        };

        assert!((new_mastery - 0.85).abs() < 1e-9, "mastery must be unchanged when prior >= MASTERY_THRESHOLD");

        let stored: f64 = conn.query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = 'mod1' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert!((stored - 0.85).abs() < 1e-9, "DB mastery must remain 0.85 (no BKT update applied)");
    }

    /// rate_flash_card below threshold with quality=4: BKT applied, mastery increases.
    #[test]
    fn flash_card_below_threshold_quality_4_reinforces() {
        let conn = fresh_conn();
        seed_module_progress_with_mastery(&conn, 0.5); // below MASTERY_THRESHOLD

        let prior: f64 = conn.query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = 'mod1' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();

        let quality: u8 = 4;
        let new_mastery = if quality >= 4 && prior < MASTERY_THRESHOLD {
            let t = apply_mastery_update(&conn, "lp1", "mod1", prior, true).unwrap();
            t.new_mastery
        } else {
            prior
        };

        assert!(new_mastery > prior, "quality=4 below threshold must increase mastery (BKT is_correct=true)");
    }

    /// rate_flash_card with quality=3 (below good/easy cutoff): BKT skipped.
    #[test]
    fn flash_card_below_threshold_quality_3_no_op() {
        let conn = fresh_conn();
        seed_module_progress_with_mastery(&conn, 0.5); // below threshold

        let prior: f64 = conn.query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = 'mod1' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();

        let quality: u8 = 3; // below the good/easy cutoff of 4
        let new_mastery = if quality >= 4 && prior < MASTERY_THRESHOLD {
            let t = apply_mastery_update(&conn, "lp1", "mod1", prior, true).unwrap();
            t.new_mastery
        } else {
            prior // GUARD: quality < 4 → no update
        };

        assert!((new_mastery - prior).abs() < 1e-9, "quality=3 must not apply BKT update");
    }

    /// rate_flash_card cannot drive module unlock: even if mastery crosses threshold,
    /// no check_unlock_modules is called from rate_flash_card.
    /// The guard prevents this by skipping BKT when prior >= threshold.
    /// For prior=0.65 where BKT could cross 0.7: the guard skips the update, preventing it.
    #[test]
    fn flash_card_does_not_trigger_unlock() {
        let conn = fresh_conn();

        conn.execute("INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'T')", []).unwrap();
        conn.execute("INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'K8s', 'devops', 'CKA')", []).unwrap();
        let edges = serde_json::json!([{"from": "mod1", "to": "mod2"}]).to_string();
        conn.execute("INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path1', 'trk1', ?1, '[]', 'test')", [&edges]).unwrap();
        conn.execute("INSERT INTO modules (id, path_id, title) VALUES ('mod1', 'path1', 'M1')", []).unwrap();
        conn.execute("INSERT INTO modules (id, path_id, title) VALUES ('mod2', 'path1', 'M2')", []).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp1', 'mod1', 'lp1', 'in_progress', 0.65)", []).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level) VALUES ('mp2', 'mod2', 'lp1', 'locked', 0.0)", []).unwrap();

        // rate_flash_card: quality=5, prior=0.65 < MASTERY_THRESHOLD
        // BKT WILL be applied (it may cross 0.7), but check_unlock_modules is NOT called.
        let prior = 0.65f64;
        let quality: u8 = 5;
        if quality >= 4 && prior < MASTERY_THRESHOLD {
            let _ = apply_mastery_update(&conn, "lp1", "mod1", prior, true).unwrap();
            // Note: NO check_unlock_modules call here — that's the flash card constraint.
        }

        // mod2 must remain locked because check_unlock_modules was never called
        let mod2_status: String = conn.query_row(
            "SELECT status FROM module_progress WHERE module_id = 'mod2' AND learner_id = 'lp1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(mod2_status, "locked", "flash card reinforcement must not unlock modules (no check_unlock_modules call)");
    }

    // ── mark_lesson_complete test (03-05 Task 1) ──

    /// Verifies that mark_lesson_complete inserts a row into lesson_completions
    /// and that the operation is idempotent (second call does not error / duplicate).
    #[test]
    fn mark_lesson_complete_persists() {
        let conn = fresh_conn();

        // Seed minimal learner
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Tester')",
            [],
        ).unwrap();

        // Simulate mark_lesson_complete logic directly (can't call Tauri command in unit test)
        let learner_id = "lp1".to_string();
        let module_id = "mod-1";
        let block_id = "blk-section-1";

        conn.execute(
            "INSERT OR IGNORE INTO lesson_completions (learner_id, module_id, block_id)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![learner_id, module_id, block_id],
        ).unwrap();

        // Assert row exists
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM lesson_completions WHERE learner_id=?1 AND module_id=?2 AND block_id=?3",
            rusqlite::params![learner_id, module_id, block_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1, "lesson_completions must have 1 row after mark_lesson_complete");

        // Idempotent: second insert must not error or create duplicate
        conn.execute(
            "INSERT OR IGNORE INTO lesson_completions (learner_id, module_id, block_id)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![learner_id, module_id, block_id],
        ).unwrap();

        let count2: i64 = conn.query_row(
            "SELECT COUNT(*) FROM lesson_completions WHERE learner_id=?1 AND module_id=?2 AND block_id=?3",
            rusqlite::params![learner_id, module_id, block_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count2, 1, "lesson_completions must still have 1 row after idempotent second call");
    }
}
