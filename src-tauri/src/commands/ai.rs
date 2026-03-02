use crate::ai::{ai_request, AIServiceRequest, ServiceMessage};
use crate::auth::AuthState;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── AI Config (legacy, kept for backward compat) ──

#[tauri::command]
pub fn get_ai_config(state: State<AppState>) -> Result<crate::db::models::AIConfig, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .query_row(
            "SELECT provider_type, api_key, model, base_url, max_tokens, temperature FROM ai_config WHERE id = 1",
            [],
            |row| {
                Ok(crate::db::models::AIConfig {
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
pub fn update_ai_config(state: State<AppState>, config: crate::db::models::AIConfig) -> Result<(), String> {
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

// ── Assessment ──

#[derive(Debug, Deserialize)]
pub struct AssessmentRequest {
    pub topic: String,
    pub domain: String,
    pub messages: Vec<AssessmentTurn>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssessmentTurn {
    pub role: String,
    pub content: String,
}

#[tauri::command]
pub async fn assess_knowledge(
    auth: State<'_, AuthState>,
    request: AssessmentRequest,
) -> Result<String, String> {
    let system_prompt = format!(
        "You are an expert tutor assessing a learner's knowledge of {}. \
         Conduct a conversational assessment through 3-5 questions. \
         Use the Socratic method: ask probing questions based on their responses. \
         Gauge depth of understanding, not just surface knowledge. \
         After sufficient assessment, end your response with a JSON block:\n\
         ```json\n\
         {{\"assessment_complete\": true, \"level\": \"beginner|intermediate|advanced\", \
         \"gaps\": [...], \"strengths\": [...], \"recommended_start\": \"...\"}}\n\
         ```\n\
         Until assessment is complete, just ask your next question naturally.",
        request.topic
    );

    let messages: Vec<ServiceMessage> = request
        .messages
        .iter()
        .map(|m| ServiceMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    let response = ai_request(
        auth.inner(),
        AIServiceRequest {
            system_prompt,
            messages,
            max_tokens: Some(1024),
            temperature: Some(0.7),
            response_format: None,
        },
    )
    .await?;

    Ok(response.content)
}

// ── Learning Path Generation ──

#[derive(Debug, Deserialize)]
pub struct GeneratePathRequest {
    pub track_id: String,
    pub topic: String,
    pub domain: String,
    pub goal: String,
    pub assessment_level: String,
    pub assessment_gaps: Vec<String>,
    pub assessment_strengths: Vec<String>,
}

#[tauri::command]
pub async fn generate_learning_path(
    auth: State<'_, AuthState>,
    state: State<'_, AppState>,
    request: GeneratePathRequest,
) -> Result<serde_json::Value, String> {
    let system_prompt = format!(
        "You are a curriculum designer creating a personalized learning path for {}. \
         The learner's level: {}. Their goal: {}. \
         Gaps: {:?}. Strengths: {:?}. \
         Generate a learning path as a DAG of 8-15 modules. \
         Return ONLY valid JSON in this format: \
         {{\"modules\": [{{\"id\": \"m1\", \"title\": \"...\", \"description\": \"...\", \
         \"difficulty\": 1, \"estimated_minutes\": 30, \"objectives\": [\"...\"]}}], \
         \"edges\": [{{\"from\": \"m1\", \"to\": \"m2\"}}]}} \
         Order modules from foundational to advanced. \
         Skip topics the learner already knows based on their strengths. \
         Add extra depth for identified knowledge gaps.",
        request.topic,
        request.assessment_level,
        request.goal,
        request.assessment_gaps,
        request.assessment_strengths,
    );

    let response = ai_request(
        auth.inner(),
        AIServiceRequest {
            system_prompt,
            messages: vec![ServiceMessage {
                role: "user".to_string(),
                content: format!(
                    "Create my personalized learning path for {}. My goal: {}",
                    request.topic, request.goal
                ),
            }],
            max_tokens: Some(4096),
            temperature: Some(0.5),
            response_format: Some("json".to_string()),
        },
    )
    .await?;

    // Parse the AI response as JSON
    let path_data: serde_json::Value = serde_json::from_str(&response.content)
        .map_err(|e| format!("Failed to parse AI response as JSON: {}", e))?;

    // Persist to database
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path_id = uuid::Uuid::new_v4().to_string();

    db.conn
        .execute(
            "INSERT INTO learning_paths (id, track_id, dag_json, version, generated_by_model) VALUES (?1, ?2, ?3, 1, ?4)",
            rusqlite::params![path_id, request.track_id, response.content, response.model],
        )
        .map_err(|e| e.to_string())?;

    // Insert modules into the modules table
    if let Some(modules) = path_data["modules"].as_array() {
        for (i, module) in modules.iter().enumerate() {
            let module_id = module["id"]
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            db.conn
                .execute(
                    "INSERT INTO modules (id, path_id, title, description, difficulty, estimated_minutes, objectives_json, ordering) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        module_id,
                        path_id,
                        module["title"].as_str().unwrap_or("Untitled"),
                        module["description"].as_str().unwrap_or(""),
                        module["difficulty"].as_i64().unwrap_or(1),
                        module["estimated_minutes"].as_i64().unwrap_or(30),
                        serde_json::to_string(&module["objectives"]).unwrap_or_default(),
                        i as i32,
                    ],
                )
                .map_err(|e| e.to_string())?;

            // First module is available, rest are locked
            let status = if i == 0 { "available" } else { "locked" };
            db.conn
                .execute(
                    "INSERT INTO module_progress (id, module_id, learner_id, status) \
                     VALUES (?1, ?2, (SELECT id FROM learner_profiles LIMIT 1), ?3)",
                    rusqlite::params![uuid::Uuid::new_v4().to_string(), module_id, status],
                )
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(serde_json::json!({
        "id": path_id,
        "trackId": request.track_id,
        "modules": path_data["modules"],
        "edges": path_data["edges"],
    }))
}

// ── AI Tutor ──

#[tauri::command]
pub async fn send_tutor_message(
    auth: State<'_, AuthState>,
    message: serde_json::Value,
) -> Result<String, String> {
    let module_context = message["moduleContext"].as_str().unwrap_or("");
    let user_message = message["content"]
        .as_str()
        .ok_or("Missing message content")?;

    let history: Vec<ServiceMessage> = message["history"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    Some(ServiceMessage {
                        role: m["role"].as_str()?.to_string(),
                        content: m["content"].as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let system_prompt = format!(
        "You are an AI tutor helping a learner. Use the Socratic method. \
         Guide the learner to understanding through questions rather than giving direct answers. \
         Current module context: {}\n\
         Be concise, encouraging, and adapt your explanations to the learner's level.",
        module_context
    );

    let mut messages = history;
    messages.push(ServiceMessage {
        role: "user".to_string(),
        content: user_message.to_string(),
    });

    let response = ai_request(
        auth.inner(),
        AIServiceRequest {
            system_prompt,
            messages,
            max_tokens: Some(1024),
            temperature: Some(0.7),
            response_format: None,
        },
    )
    .await?;

    Ok(response.content)
}

// ── Module Content Generation ──

#[derive(Debug, Deserialize)]
pub struct GenerateContentRequest {
    pub module_id: String,
    pub track_id: String,
    pub module_title: String,
    pub objectives: Vec<String>,
    pub learner_level: String,
    pub previous_performance: Option<String>,
}

#[tauri::command]
pub async fn generate_module_content(
    auth: State<'_, AuthState>,
    state: State<'_, AppState>,
    request: GenerateContentRequest,
) -> Result<String, String> {
    // Check cache first
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let cached: Option<String> = db
            .conn
            .query_row(
                "SELECT content FROM modules WHERE id = ?1 AND content IS NOT NULL",
                [&request.module_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(content) = cached {
            return Ok(content);
        }
    }

    let perf_context = request
        .previous_performance
        .as_ref()
        .map(|p| format!("Previous module performance: {}. Adjust difficulty accordingly. ", p))
        .unwrap_or_default();

    let system_prompt = format!(
        "You are creating a lesson for the module: '{}'. \
         Learning objectives: {:?}. Learner level: {}. \
         {}\
         Write the lesson in Markdown. Include: \
         - Clear explanations with real-world analogies \
         - Code examples with syntax highlighting (use fenced code blocks) \
         - Key concepts highlighted in bold \
         - A brief summary at the end \
         Keep it focused and practical. Target 10-15 minutes of reading time.",
        request.module_title, request.objectives, request.learner_level, perf_context,
    );

    let response = ai_request(
        auth.inner(),
        AIServiceRequest {
            system_prompt,
            messages: vec![ServiceMessage {
                role: "user".to_string(),
                content: format!("Generate the lesson content for: {}", request.module_title),
            }],
            max_tokens: Some(4096),
            temperature: Some(0.6),
            response_format: None,
        },
    )
    .await?;

    // Cache the generated content
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "UPDATE modules SET content = ?1, content_generated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![response.content, request.module_id],
        )
        .map_err(|e| e.to_string())?;

    Ok(response.content)
}
