use crate::ai::{ai_request, AIServiceRequest, ServiceMessage};
use crate::auth::AuthState;
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
    pub level: String, // "beginner" | "intermediate" | "advanced"
}

#[tauri::command]
pub fn assess_knowledge(request: AssessmentRequest) -> Result<String, String> {
    let (gaps, strengths) = match request.level.as_str() {
        "intermediate" => (
            vec!["advanced patterns", "performance optimization"],
            vec!["core concepts", "basic usage"],
        ),
        "advanced" => (
            vec!["niche edge cases"],
            vec!["core concepts", "patterns", "best practices", "performance"],
        ),
        _ => (
            // beginner
            vec!["core concepts", "basic syntax", "fundamental patterns"],
            vec![],
        ),
    };
    let result = serde_json::json!({
        "assessment_complete": true,
        "level": request.level,
        "gaps": gaps,
        "strengths": strengths,
    });
    Ok(result.to_string())
}

// ── Learning Path Generation ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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

    // Validate DAG structure before persisting
    if let (Some(modules), Some(edges)) = (
        path_data["modules"].as_array(),
        path_data["edges"].as_array(),
    ) {
        use crate::learning::path::{PathNode, PathEdge};
        let nodes: Vec<PathNode> = modules
            .iter()
            .map(|m| PathNode {
                id: m["id"].as_str().unwrap_or("").to_string(),
                title: m["title"].as_str().unwrap_or("").to_string(),
                description: m["description"].as_str().unwrap_or("").to_string(),
                module_type: "lesson".to_string(),
                difficulty: m["difficulty"].as_i64().unwrap_or(1) as i32,
                estimated_minutes: m["estimated_minutes"].as_i64().unwrap_or(30) as i32,
                objectives: vec![],
                prerequisites: vec![],
            })
            .collect();
        let path_edges: Vec<PathEdge> = edges
            .iter()
            .filter_map(|e| {
                Some(PathEdge {
                    from: e["from"].as_str()?.to_string(),
                    to: e["to"].as_str()?.to_string(),
                    edge_type: "prerequisite".to_string(),
                })
            })
            .collect();
        crate::learning::path::validate_dag(&nodes, &path_edges)
            .map_err(|e| format!("AI generated invalid learning path: {}", e))?;
    }

    // Persist to database
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path_id = uuid::Uuid::new_v4().to_string();

    let modules_json = serde_json::to_string(&path_data["modules"]).unwrap_or_else(|_| "[]".to_string());
    let edges_json = serde_json::to_string(&path_data["edges"]).unwrap_or_else(|_| "[]".to_string());

    db.conn
        .execute(
            "INSERT INTO learning_paths (id, track_id, modules_json, edges_json, version, generated_by_model) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            rusqlite::params![path_id, request.track_id, modules_json, edges_json, response.model],
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
#[serde(rename_all = "camelCase")]
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

// ── Exercise Commands ──

#[tauri::command]
pub fn get_exercises(
    state: State<AppState>,
    module_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = db
        .conn
        .prepare(
            "SELECT id, module_id, exercise_type, difficulty, prompt, hints_json, metadata_json \
             FROM exercises WHERE module_id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let exercises = stmt
        .query_map([&module_id], |row| {
            let hints_str: String = row.get(5)?;
            let metadata_str: String = row.get(6)?;
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "moduleId": row.get::<_, String>(1)?,
                "type": row.get::<_, String>(2)?,
                "difficulty": row.get::<_, i32>(3)?,
                "prompt": row.get::<_, String>(4)?,
                "hints": serde_json::from_str::<serde_json::Value>(&hints_str).unwrap_or(json!([])),
                "metadata": serde_json::from_str::<serde_json::Value>(&metadata_str).unwrap_or(json!({})),
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(exercises)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateExerciseRequest {
    pub module_id: String,
    pub difficulty: i32,
    #[serde(rename = "type")]
    pub exercise_type: String,
    pub context: String,
}

#[tauri::command]
pub async fn generate_exercise(
    auth: State<'_, AuthState>,
    state: State<'_, AppState>,
    request: GenerateExerciseRequest,
) -> Result<serde_json::Value, String> {
    let system_prompt = format!(
        "You are creating a {} exercise at difficulty level {}/10. \
         Context: {}. \
         Return ONLY valid JSON in this format: \
         {{\"prompt\": \"...\", \"hints\": [\"...\"], \"metadata\": {{}}}} \
         For code_challenge, include starterCode and testCases in metadata. \
         For multiple_choice, include options and correctIndices in metadata. \
         For fill_in_blank, include template and blanks in metadata.",
        request.exercise_type, request.difficulty, request.context,
    );

    let response = ai_request(
        auth.inner(),
        AIServiceRequest {
            system_prompt,
            messages: vec![ServiceMessage {
                role: "user".to_string(),
                content: format!(
                    "Generate a {} exercise at difficulty {}/10",
                    request.exercise_type, request.difficulty
                ),
            }],
            max_tokens: Some(2048),
            temperature: Some(0.7),
            response_format: Some("json".to_string()),
        },
    )
    .await?;

    let exercise_data: serde_json::Value = serde_json::from_str(&response.content)
        .map_err(|e| format!("Failed to parse exercise JSON: {}", e))?;

    // Persist to database
    let exercise_id = uuid::Uuid::new_v4().to_string();
    let hints_json = serde_json::to_string(&exercise_data["hints"]).unwrap_or_else(|_| "[]".to_string());
    let metadata_json = serde_json::to_string(&exercise_data["metadata"]).unwrap_or_else(|_| "{}".to_string());

    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "INSERT INTO exercises (id, module_id, exercise_type, difficulty, prompt, hints_json, metadata_json) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                exercise_id,
                request.module_id,
                request.exercise_type,
                request.difficulty,
                exercise_data["prompt"].as_str().unwrap_or(""),
                hints_json,
                metadata_json,
            ],
        )
        .map_err(|e| e.to_string())?;

    Ok(json!({
        "id": exercise_id,
        "moduleId": request.module_id,
        "type": request.exercise_type,
        "difficulty": request.difficulty,
        "prompt": exercise_data["prompt"],
        "hints": exercise_data["hints"],
        "metadata": exercise_data["metadata"],
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateResponseRequest {
    pub exercise_prompt: String,
    pub learner_response: String,
    pub rubric: String,
    pub expected_answer: Option<String>,
}

#[tauri::command]
pub async fn evaluate_response(
    auth: State<'_, AuthState>,
    request: EvaluateResponseRequest,
) -> Result<serde_json::Value, String> {
    let expected = request
        .expected_answer
        .as_ref()
        .map(|a| format!("Expected answer: {}. ", a))
        .unwrap_or_default();

    let system_prompt = format!(
        "You are evaluating a learner's response to an exercise. \
         {}Rubric: {}. \
         Return ONLY valid JSON: \
         {{\"score\": 0-100, \"feedback\": \"...\", \"misconceptions\": [...], \
         \"hints\": [...], \"isCorrect\": true/false}}",
        expected, request.rubric,
    );

    let response = ai_request(
        auth.inner(),
        AIServiceRequest {
            system_prompt,
            messages: vec![ServiceMessage {
                role: "user".to_string(),
                content: format!(
                    "Exercise: {}\n\nLearner's response: {}",
                    request.exercise_prompt, request.learner_response
                ),
            }],
            max_tokens: Some(1024),
            temperature: Some(0.3),
            response_format: Some("json".to_string()),
        },
    )
    .await?;

    let result: serde_json::Value = serde_json::from_str(&response.content)
        .map_err(|e| format!("Failed to parse evaluation JSON: {}", e))?;

    Ok(result)
}

// ── Complete Module Exercises (adaptive loop closure) ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteExercisesRequest {
    pub module_id: String,
    pub track_id: String,
    pub scores: Vec<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteExercisesResult {
    pub mastery_level: f64,
    pub module_completed: bool,
    pub unlocked_modules: Vec<String>,
    pub cards_created: i32,
}

#[tauri::command]
pub fn complete_module_exercises(
    state: State<AppState>,
    request: CompleteExercisesRequest,
) -> Result<CompleteExercisesResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let params = crate::learning::adaptive::BKTParams::default();

    // 1. Get current mastery
    let current_mastery: f64 = db.conn
        .query_row(
            "SELECT mastery_level FROM module_progress WHERE module_id = ?1 LIMIT 1",
            [&request.module_id],
            |row| row.get(0),
        )
        .unwrap_or(0.3);

    // 2. Run BKT for each score
    let mut mastery = current_mastery;
    for score in &request.scores {
        let is_correct = *score >= 50.0;
        mastery = crate::learning::adaptive::update_mastery(&params, mastery, is_correct);
    }

    // 3. Compute average score
    let avg_score = if request.scores.is_empty() {
        0.0
    } else {
        request.scores.iter().sum::<f64>() / request.scores.len() as f64
    };

    let module_completed = mastery >= 0.7;
    let new_status = if module_completed { "completed" } else { "in_progress" };

    // 4. Upsert module_progress
    let profile_id: String = db.conn
        .query_row("SELECT id FROM learner_profiles LIMIT 1", [], |row| row.get(0))
        .map_err(|e| format!("No profile: {}", e))?;

    let progress_id = uuid::Uuid::new_v4().to_string();
    db.conn
        .execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status, score, mastery_level, attempts, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, datetime('now'))
             ON CONFLICT(module_id, learner_id) DO UPDATE SET
               status = ?4, score = ?5, mastery_level = ?6,
               attempts = attempts + 1,
               completed_at = CASE WHEN ?4 = 'completed' THEN datetime('now') ELSE completed_at END",
            rusqlite::params![progress_id, request.module_id, profile_id, new_status, avg_score, mastery],
        )
        .map_err(|e| e.to_string())?;

    // 5. If completed, unlock dependent modules in DAG
    let mut unlocked = Vec::new();
    if module_completed {
        let path_row: Option<(String, String)> = db.conn
            .query_row(
                "SELECT modules_json, edges_json FROM learning_paths WHERE track_id = ?1 ORDER BY version DESC LIMIT 1",
                [&request.track_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        if let Some((_modules_json, edges_json)) = path_row {
            let edges: Vec<serde_json::Value> = serde_json::from_str(&edges_json).unwrap_or_default();

            let dependents: Vec<String> = edges.iter()
                .filter(|e| e["from"].as_str() == Some(&request.module_id))
                .filter_map(|e| e["to"].as_str().map(String::from))
                .collect();

            for dep_id in &dependents {
                let prereqs: Vec<String> = edges.iter()
                    .filter(|e| e["to"].as_str() == Some(dep_id.as_str()))
                    .filter_map(|e| e["from"].as_str().map(String::from))
                    .collect();

                let all_prereqs_done = prereqs.iter().all(|prereq_id| {
                    db.conn.query_row(
                        "SELECT status FROM module_progress WHERE module_id = ?1 AND learner_id = ?2",
                        rusqlite::params![prereq_id, profile_id],
                        |row| row.get::<_, String>(0),
                    )
                    .map(|s| s == "completed")
                    .unwrap_or(false)
                });

                if all_prereqs_done {
                    let unlock_id = uuid::Uuid::new_v4().to_string();
                    db.conn.execute(
                        "INSERT INTO module_progress (id, module_id, learner_id, status)
                         VALUES (?1, ?2, ?3, 'available')
                         ON CONFLICT(module_id, learner_id) DO UPDATE SET
                           status = CASE WHEN status = 'locked' THEN 'available' ELSE status END",
                        rusqlite::params![unlock_id, dep_id, profile_id],
                    ).ok();
                    unlocked.push(dep_id.clone());
                }
            }
        }

        // Update track progress_percent
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
        let pct = if total_modules > 0 { (completed_modules as f64 / total_modules as f64) * 100.0 } else { 0.0 };
        db.conn.execute(
            "UPDATE learning_tracks SET progress_percent = ?1, updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![pct, request.track_id],
        ).ok();
    }

    // 6. Generate SR cards for exercises
    let mut cards_created = 0i32;
    let mut exercise_stmt = db.conn
        .prepare("SELECT id, prompt, exercise_type FROM exercises WHERE module_id = ?1")
        .map_err(|e| e.to_string())?;
    let exercises: Vec<(String, String, String)> = exercise_stmt
        .query_map([&request.module_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    for (_ex_id, prompt, ex_type) in &exercises {
        let card_id = uuid::Uuid::new_v4().to_string();
        let concept = prompt.chars().take(80).collect::<String>();
        let front = format!("Recall: {}", prompt.chars().take(200).collect::<String>());
        let back = format!("Review your {} exercise answer.", ex_type.replace('_', " "));
        db.conn.execute(
            "INSERT OR IGNORE INTO sr_cards (id, module_id, concept, card_type, front, back)
             VALUES (?1, ?2, ?3, 'active_recall', ?4, ?5)",
            rusqlite::params![card_id, request.module_id, concept, front, back],
        ).ok();
        cards_created += 1;
    }

    Ok(CompleteExercisesResult {
        mastery_level: mastery,
        module_completed,
        unlocked_modules: unlocked,
        cards_created,
    })
}

#[cfg(test)]
mod tests {
    use crate::learning::adaptive::{BKTParams, update_mastery};

    #[test]
    fn test_bkt_mastery_update_logic() {
        let params = BKTParams::default();
        let scores = vec![80.0, 30.0, 90.0];
        let mut mastery = 0.3;
        for score in &scores {
            let is_correct = *score >= 50.0;
            mastery = update_mastery(&params, mastery, is_correct);
        }
        assert!(mastery > 0.3, "Mastery should increase with 2/3 correct");
        assert!(mastery < 1.0);
    }
}
