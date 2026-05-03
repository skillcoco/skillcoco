use crate::db::models::{LearnerProfile, LearningTrack};
use crate::AppState;
use serde::Deserialize;
use tauri::State;

/// Typed request for updating the learner profile.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
}

#[tauri::command]
pub fn get_or_create_profile(state: State<AppState>) -> Result<LearnerProfile, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Check if profile exists
    let maybe_profile: Option<LearnerProfile> = db
        .conn
        .query_row(
            "SELECT id, display_name, learning_style, experience_level, preferences_json, created_at, updated_at FROM learner_profiles LIMIT 1",
            [],
            |row| {
                Ok(LearnerProfile {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    learning_style: row.get(2)?,
                    experience_level: row.get(3)?,
                    preferences_json: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .ok();

    match maybe_profile {
        Some(profile) => Ok(profile),
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            db.conn
                .execute(
                    "INSERT INTO learner_profiles (id) VALUES (?1)",
                    [&id],
                )
                .map_err(|e| e.to_string())?;

            db.conn
                .query_row(
                    "SELECT id, display_name, learning_style, experience_level, preferences_json, created_at, updated_at FROM learner_profiles WHERE id = ?1",
                    [&id],
                    |row| {
                        Ok(LearnerProfile {
                            id: row.get(0)?,
                            display_name: row.get(1)?,
                            learning_style: row.get(2)?,
                            experience_level: row.get(3)?,
                            preferences_json: row.get(4)?,
                            created_at: row.get(5)?,
                            updated_at: row.get(6)?,
                        })
                    },
                )
                .map_err(|e| e.to_string())
        }
    }
}

#[tauri::command]
pub fn update_profile(
    state: State<AppState>,
    profile: UpdateProfileRequest,
) -> Result<LearnerProfile, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Get the current profile's ID to scope the UPDATE
    let profile_id: String = db
        .conn
        .query_row(
            "SELECT id FROM learner_profiles LIMIT 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("No profile found: {}", e))?;

    if let Some(name) = profile.display_name {
        db.conn
            .execute(
                "UPDATE learner_profiles SET display_name = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![name, profile_id],
            )
            .map_err(|e| e.to_string())?;
    }

    get_or_create_profile_inner(&db)
}

fn get_or_create_profile_inner(db: &crate::db::Database) -> Result<LearnerProfile, String> {
    db.conn
        .query_row(
            "SELECT id, display_name, learning_style, experience_level, preferences_json, created_at, updated_at FROM learner_profiles LIMIT 1",
            [],
            |row| {
                Ok(LearnerProfile {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    learning_style: row.get(2)?,
                    experience_level: row.get(3)?,
                    preferences_json: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_tracks(state: State<AppState>) -> Result<Vec<LearningTrack>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = db
        .conn
        .prepare(
            "SELECT id, learner_id, topic, domain_module, status, goal, current_module_id, progress_percent, total_time_spent, created_at, updated_at, COALESCE(streak_days, 0), last_activity_date FROM learning_tracks ORDER BY updated_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let tracks = stmt
        .query_map([], |row| {
            Ok(LearningTrack {
                id: row.get(0)?,
                learner_id: row.get(1)?,
                topic: row.get(2)?,
                domain_module: row.get(3)?,
                status: row.get(4)?,
                goal: row.get(5)?,
                current_module_id: row.get(6)?,
                progress_percent: row.get(7)?,
                total_time_spent: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                streak_days: row.get(11)?,
                last_activity_date: row.get(12)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(tracks)
}

#[tauri::command]
pub fn create_track(
    state: State<AppState>,
    topic: String,
    domain_module: String,
    goal: String,
) -> Result<LearningTrack, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Ensure profile exists
    let profile = get_or_create_profile_inner(&db)?;

    let id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, profile.id, topic, domain_module, goal],
        )
        .map_err(|e| e.to_string())?;

    get_track_inner(&db, &id)
}

#[tauri::command]
pub fn get_track(state: State<AppState>, track_id: String) -> Result<LearningTrack, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    get_track_inner(&db, &track_id)
}

fn get_track_inner(db: &crate::db::Database, track_id: &str) -> Result<LearningTrack, String> {
    db.conn
        .query_row(
            "SELECT id, learner_id, topic, domain_module, status, goal, current_module_id, progress_percent, total_time_spent, created_at, updated_at, COALESCE(streak_days, 0), last_activity_date FROM learning_tracks WHERE id = ?1",
            [track_id],
            |row| {
                Ok(LearningTrack {
                    id: row.get(0)?,
                    learner_id: row.get(1)?,
                    topic: row.get(2)?,
                    domain_module: row.get(3)?,
                    status: row.get(4)?,
                    goal: row.get(5)?,
                    current_module_id: row.get(6)?,
                    progress_percent: row.get(7)?,
                    total_time_spent: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                    streak_days: row.get(11)?,
                    last_activity_date: row.get(12)?,
                })
            },
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_track_status(
    state: State<AppState>,
    track_id: String,
    status: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "UPDATE learning_tracks SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![status, track_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}
