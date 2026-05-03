use crate::ai::{ai_request, AIServiceRequest, ServiceMessage};
use crate::auth::AuthState;
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::State;

/// Extract JSON from AI response that may be wrapped in markdown code fences.
fn extract_json(text: &str) -> Result<serde_json::Value, String> {
    let trimmed = text.trim();

    // Try direct parse first
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Ok(v);
    }

    // Strip markdown code fences: ```json\n...\n``` or ```\n...\n```
    let stripped = if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            &trimmed[start..=end]
        } else {
            trimmed
        }
    } else {
        trimmed
    };

    serde_json::from_str(stripped)
        .map_err(|e| format!("{} (first 200 chars: {:?})", e, &trimmed[..trimmed.len().min(200)]))
}

// get_ai_config and update_ai_config removed in FIX-03.
// Auth flows through AuthState (auth/mod.rs), not the ai_config table.

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
    let topic = &request.topic;
    let system_prompt = format!(
        "You are a curriculum designer. Create a learning path for {topic} ({domain}). \
         Learner level: {level}. Goal: {goal}. \
         Gaps: {gaps:?}. Strengths: {strengths:?}. \
         \
         Generate 6-10 modules with REAL topic-specific titles and descriptions. \
         Each module ID MUST be a UUID (use format like \"mod-01\", \"mod-02\" etc). \
         \
         Return ONLY raw JSON, no markdown: \
         {{\"modules\": [{{\"id\": \"mod-01\", \"title\": \"...\", \"description\": \"...\", \
         \"difficulty\": 1, \"estimated_minutes\": 30, \"objectives\": [\"...\"]}}], \
         \"edges\": [{{\"from\": \"mod-01\", \"to\": \"mod-02\"}}]}} \
         \
         Make titles SPECIFIC to {topic}, not generic. \
         Example for Kubernetes beginner: \"Pods, Nodes, and Clusters\", not \"Introduction to Kubernetes\". \
         Example for Rust intermediate: \"Ownership and Borrowing Patterns\", not \"Rust Patterns\". \
         Order from foundational to advanced. Skip what the learner already knows.",
        topic = topic,
        domain = request.domain,
        level = request.assessment_level,
        goal = request.goal,
        gaps = request.assessment_gaps,
        strengths = request.assessment_strengths,
    );

    let response = ai_request(
        auth.inner(),
        AIServiceRequest {
            system_prompt,
            messages: vec![ServiceMessage {
                role: "user".to_string(),
                content: format!("Create my learning path for {}. Return ONLY JSON.", topic),
            }],
            max_tokens: Some(4096),
            temperature: Some(0.5),
            response_format: Some("json".to_string()),
        },
    )
    .await?;

    let mut path_data: serde_json::Value = extract_json(&response.content)
        .map_err(|e| format!("Failed to parse AI response: {}", e))?;

    // Replace AI-generated IDs with UUIDs to avoid UNIQUE constraint collisions
    if let Some(modules) = path_data["modules"].as_array_mut() {
        let mut id_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for module in modules.iter_mut() {
            let old_id = module["id"].as_str().unwrap_or("").to_string();
            let new_id = uuid::Uuid::new_v4().to_string();
            id_map.insert(old_id, new_id.clone());
            module["id"] = json!(new_id);
        }
        // Remap edge IDs
        if let Some(edges) = path_data["edges"].as_array_mut() {
            for edge in edges.iter_mut() {
                if let Some(from) = edge["from"].as_str().map(String::from) {
                    if let Some(new_from) = id_map.get(&from) {
                        edge["from"] = json!(new_from);
                    }
                }
                if let Some(to) = edge["to"].as_str().map(String::from) {
                    if let Some(new_to) = id_map.get(&to) {
                        edge["to"] = json!(new_to);
                    }
                }
            }
        }
    }

    let empty_vec = vec![];
    let modules_arr = path_data["modules"].as_array().ok_or("AI response missing 'modules' array")?;
    let edges_arr = path_data["edges"].as_array().unwrap_or(&empty_vec);

    // Validate DAG structure
    {
        use crate::learning::path::{PathNode, PathEdge};
        let nodes: Vec<PathNode> = modules_arr
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
        let path_edges: Vec<PathEdge> = edges_arr
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

    let modules_json = serde_json::to_string(modules_arr).unwrap_or_else(|_| "[]".to_string());
    let edges_json = serde_json::to_string(edges_arr).unwrap_or_else(|_| "[]".to_string());

    db.conn
        .execute(
            "INSERT INTO learning_paths (id, track_id, modules_json, edges_json, version, generated_by_model) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            rusqlite::params![path_id, request.track_id, modules_json, edges_json, response.model],
        )
        .map_err(|e| e.to_string())?;

    for (i, module) in modules_arr.iter().enumerate() {
        let module_id = module["id"].as_str().unwrap_or("").to_string();

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

        let status = if i == 0 { "available" } else { "locked" };
        db.conn
            .execute(
                "INSERT INTO module_progress (id, module_id, learner_id, status) \
                 VALUES (?1, ?2, (SELECT id FROM learner_profiles LIMIT 1), ?3)",
                rusqlite::params![uuid::Uuid::new_v4().to_string(), module_id, status],
            )
            .map_err(|e| e.to_string())?;
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

    let exercise_data: serde_json::Value = extract_json(&response.content)
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

    let result: serde_json::Value = extract_json(&response.content)
        .map_err(|e| format!("Failed to parse evaluation JSON: {}", e))?;

    Ok(result)
}

// complete_module_exercises has been relocated to commands/learning.rs in Plan 01-03.
// That file now owns all Phase 1 learning-flow logic.
// ai.rs retains: AI-only concerns (path/content/exercise generation, ai_request, tutor).

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
