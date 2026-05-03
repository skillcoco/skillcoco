use crate::db::models::{LearningPath, ModuleProgress, SRCard};
use crate::AppState;
use serde::Deserialize;
use tauri::State;

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

#[tauri::command]
pub fn submit_review(state: State<AppState>, result: serde_json::Value) -> Result<SRCard, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let card_id = result["cardId"].as_str().ok_or("Missing cardId")?;
    let quality = result["quality"].as_i64().ok_or("Missing quality")? as i32;

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

    // Return updated card
    db.conn
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
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
