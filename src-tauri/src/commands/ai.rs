use crate::db::models::AIConfig;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_ai_config(state: State<AppState>) -> Result<AIConfig, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.conn
        .query_row(
            "SELECT provider_type, api_key, model, base_url, max_tokens, temperature FROM ai_config WHERE id = 1",
            [],
            |row| {
                Ok(AIConfig {
                    provider_type: row.get(0)?,
                    api_key: row.get(1)?,
                    model: row.get(2)?,
                    base_url: row.get(3)?,
                    max_tokens: row.get(4)?,
                    temperature: row.get(5)?,
                })
            },
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_ai_config(state: State<AppState>, config: AIConfig) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.conn
        .execute(
            "UPDATE ai_config SET provider_type = ?1, api_key = ?2, model = ?3, base_url = ?4, max_tokens = ?5, temperature = ?6 WHERE id = 1",
            rusqlite::params![
                config.provider_type,
                config.api_key,
                config.model,
                config.base_url,
                config.max_tokens,
                config.temperature,
            ],
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn assess_knowledge(
    _state: State<'_, AppState>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    // TODO: Implement actual AI call
    // For now, return a mock assessment
    Ok(serde_json::json!({
        "skillLevel": {},
        "knowledgeGaps": [],
        "recommendedStartingPoint": "fundamentals",
        "overallLevel": "beginner"
    }))
}

#[tauri::command]
pub async fn generate_learning_path(
    _state: State<'_, AppState>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    // TODO: Implement actual AI-powered path generation
    // This will call the configured AI provider to generate a learning path DAG
    Ok(serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "modules": [],
        "edges": [],
        "estimatedHours": 0
    }))
}

#[tauri::command]
pub async fn send_tutor_message(
    _state: State<'_, AppState>,
    message: serde_json::Value,
) -> Result<String, String> {
    // TODO: Implement actual AI tutor conversation
    // This will maintain conversation context and call the AI provider
    Ok("AI tutor response will be implemented here. This is a placeholder.".to_string())
}
