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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TutorMessageRequest {
    pub content: String,
    /// When set, backend fetches authoritative module + track context from DB.
    #[serde(default)]
    pub module_id: Option<String>,
    /// Free-text fallback (legacy frontend); used only when `module_id` lookup fails.
    #[serde(default)]
    pub module_context: Option<String>,
    #[serde(default)]
    pub history: Vec<TutorHistoryMessage>,
}

#[derive(Debug, Deserialize)]
pub struct TutorHistoryMessage {
    pub role: String,
    pub content: String,
}

const TUTOR_CONTENT_EXCERPT_MAX: usize = 1500;

fn build_tutor_system_prompt(ctx: Option<&ExerciseContext>, fallback: &str) -> String {
    let base = "You are an AI tutor helping a learner. \
        Answer their questions clearly and accurately, using examples and analogies \
        appropriate to their level. Use Socratic prompts SPARINGLY — only when the \
        learner is close to the answer and a small nudge will help them get there. \
        Default to direct, concise explanations. \
        Stay focused on the current module topic; redirect off-topic questions back \
        to the module's learning objectives.";

    match ctx {
        Some(ctx) => {
            let objectives_block = if ctx.objectives.is_empty() {
                "  (none specified)".to_string()
            } else {
                ctx.objectives
                    .iter()
                    .map(|o| format!("  - {}", o))
                    .collect::<Vec<_>>()
                    .join("\n")
            };

            let content_section = if ctx.content_excerpt.trim().is_empty() {
                String::new()
            } else {
                let excerpt = if ctx.content_excerpt.len() > TUTOR_CONTENT_EXCERPT_MAX {
                    let start = ctx.content_excerpt.len() - TUTOR_CONTENT_EXCERPT_MAX;
                    let safe_start = ctx
                        .content_excerpt
                        .char_indices()
                        .map(|(i, _)| i)
                        .find(|&i| i >= start)
                        .unwrap_or(start);
                    &ctx.content_excerpt[safe_start..]
                } else {
                    ctx.content_excerpt.as_str()
                };
                format!("\n\nMODULE CONTENT (recent excerpt):\n{}", excerpt)
            };

            format!(
                "{base}\n\n\
                LEARNING TRACK:\n\
                - Topic: {topic}\n\
                - Goal: {goal}\n\n\
                CURRENT MODULE:\n\
                - Title: {module_title}\n\
                - Description: {module_description}\n\
                - Learning Objectives:\n{objectives_block}{content_section}",
                base = base,
                topic = ctx.topic,
                goal = ctx.goal,
                module_title = ctx.module_title,
                module_description = ctx.module_description,
                objectives_block = objectives_block,
                content_section = content_section,
            )
        }
        None => {
            if fallback.trim().is_empty() {
                base.to_string()
            } else {
                format!("{}\n\nContext (provided by frontend): {}", base, fallback)
            }
        }
    }
}

#[tauri::command]
pub async fn send_tutor_message(
    auth: State<'_, AuthState>,
    state: State<'_, AppState>,
    message: TutorMessageRequest,
) -> Result<String, String> {
    if message.content.trim().is_empty() {
        return Err("Message content is empty".to_string());
    }

    // Fetch authoritative context if moduleId provided. Failures fall back to
    // the optional moduleContext string (backwards compat for any caller that
    // hasn't been updated yet).
    let ctx = match &message.module_id {
        Some(id) if !id.is_empty() => load_exercise_context(state.inner(), id).ok(),
        _ => None,
    };
    let fallback = message.module_context.as_deref().unwrap_or("");

    let system_prompt = build_tutor_system_prompt(ctx.as_ref(), fallback);

    let mut messages: Vec<ServiceMessage> = message
        .history
        .into_iter()
        .map(|h| ServiceMessage {
            role: h.role,
            content: h.content,
        })
        .collect();
    messages.push(ServiceMessage {
        role: "user".to_string(),
        content: message.content,
    });

    let response = ai_request(
        auth.inner(),
        AIServiceRequest {
            system_prompt,
            messages,
            max_tokens: Some(1024),
            temperature: Some(0.6),
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
    /// Optional learner-supplied context (e.g. "focus on networking"). The
    /// authoritative module/track context is fetched from the DB regardless.
    #[serde(default)]
    pub context: String,
}

/// Resolved learner + module context used to build a grounded exercise prompt.
/// Populated from the DB inside `generate_exercise` so the frontend cannot
/// accidentally pass an empty/wrong context (the original bug — see
/// `.planning/phases/01-stabilize-adaptive-loop/01-04-SUMMARY.md`).
#[derive(Debug, Clone)]
struct ExerciseContext {
    topic: String,
    domain: String,
    goal: String,
    module_title: String,
    module_description: String,
    objectives: Vec<String>,
    content_excerpt: String,
}

const EXERCISE_CONTENT_EXCERPT_MAX: usize = 2000;

/// Build the system prompt for exercise generation. Pure function — no IO.
/// Tested in `mod tests`. The prompt MUST anchor the LLM to the specific
/// module so it cannot drift into unrelated subjects.
fn build_exercise_system_prompt(
    exercise_type: &str,
    difficulty: i32,
    ctx: &ExerciseContext,
) -> String {
    let objectives_block = if ctx.objectives.is_empty() {
        "  (none specified)".to_string()
    } else {
        ctx.objectives
            .iter()
            .map(|o| format!("  - {}", o))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let content_section = if ctx.content_excerpt.trim().is_empty() {
        String::new()
    } else {
        let excerpt = if ctx.content_excerpt.len() > EXERCISE_CONTENT_EXCERPT_MAX {
            // Take the LAST N chars — most recent reading is most relevant.
            let start = ctx.content_excerpt.len() - EXERCISE_CONTENT_EXCERPT_MAX;
            // Find a char boundary at or after `start` so we don't slice mid-utf8.
            let safe_start = ctx
                .content_excerpt
                .char_indices()
                .map(|(i, _)| i)
                .find(|&i| i >= start)
                .unwrap_or(start);
            &ctx.content_excerpt[safe_start..]
        } else {
            ctx.content_excerpt.as_str()
        };
        format!("\n\nMODULE CONTENT (recent excerpt):\n{}", excerpt)
    };

    format!(
        "You are creating a {exercise_type} exercise at difficulty {difficulty}/10 \
for a learner studying {topic}.\n\n\
LEARNING TRACK:\n\
- Topic: {topic}\n\
- Domain: {domain}\n\
- Goal: {goal}\n\n\
CURRENT MODULE:\n\
- Title: {module_title}\n\
- Description: {module_description}\n\
- Learning Objectives:\n{objectives_block}{content_section}\n\n\
The exercise MUST be specifically about \"{module_title}\" within {topic}. \
Do NOT generate exercises about unrelated subjects.\n\n\
Return ONLY valid JSON in this format: \
{{\"prompt\": \"...\", \"hints\": [\"...\"], \"metadata\": {{}}}} \
For code_challenge, include starterCode and testCases in metadata. \
For multiple_choice, include options and correctIndices in metadata. \
For fill_in_blank, include template and blanks in metadata.",
        exercise_type = exercise_type,
        difficulty = difficulty,
        topic = ctx.topic,
        domain = ctx.domain,
        goal = ctx.goal,
        module_title = ctx.module_title,
        module_description = ctx.module_description,
        objectives_block = objectives_block,
        content_section = content_section,
    )
}

/// Fetch module + parent track context from the DB. Holds the lock briefly
/// and drops it before any `.await` (mutex-across-await pattern from
/// 01-RESEARCH.md § Pitfall 2).
fn load_exercise_context(
    state: &AppState,
    module_id: &str,
) -> Result<ExerciseContext, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .query_row(
            "SELECT t.topic, t.domain_module, t.goal, \
                    m.title, m.description, m.objectives_json, COALESCE(m.content, '') \
             FROM modules m \
             JOIN learning_paths p ON p.id = m.path_id \
             JOIN learning_tracks t ON t.id = p.track_id \
             WHERE m.id = ?1",
            [module_id],
            |row| {
                let objectives_json: String = row.get(5)?;
                Ok(ExerciseContext {
                    topic: row.get(0)?,
                    domain: row.get(1)?,
                    goal: row.get(2)?,
                    module_title: row.get(3)?,
                    module_description: row.get(4)?,
                    objectives: serde_json::from_str(&objectives_json).unwrap_or_default(),
                    content_excerpt: row.get(6)?,
                })
            },
        )
        .map_err(|e| format!("module not found or context lookup failed: {}", e))
}

#[tauri::command]
pub async fn generate_exercise(
    auth: State<'_, AuthState>,
    state: State<'_, AppState>,
    request: GenerateExerciseRequest,
) -> Result<serde_json::Value, String> {
    let ctx = load_exercise_context(state.inner(), &request.module_id)?;

    let system_prompt =
        build_exercise_system_prompt(&request.exercise_type, request.difficulty, &ctx);

    let user_message = if request.context.trim().is_empty() {
        format!(
            "Generate a {} exercise at difficulty {}/10 about \"{}\" within {}.",
            request.exercise_type, request.difficulty, ctx.module_title, ctx.topic
        )
    } else {
        format!(
            "Generate a {} exercise at difficulty {}/10 about \"{}\" within {}. \
             Additional learner context: {}",
            request.exercise_type,
            request.difficulty,
            ctx.module_title,
            ctx.topic,
            request.context
        )
    };

    let response = ai_request(
        auth.inner(),
        AIServiceRequest {
            system_prompt,
            messages: vec![ServiceMessage {
                role: "user".to_string(),
                content: user_message,
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
    use super::*;
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

    fn k8s_pods_context() -> ExerciseContext {
        ExerciseContext {
            topic: "Kubernetes".to_string(),
            domain: "devops".to_string(),
            goal: "Pass the CKA exam".to_string(),
            module_title: "Pods, Nodes and Clusters".to_string(),
            module_description: "Understand the smallest deployable unit in K8s".to_string(),
            objectives: vec![
                "Define what a Pod is".to_string(),
                "Explain Pod lifecycle".to_string(),
            ],
            content_excerpt: "A Pod is the smallest deployable unit in Kubernetes...".to_string(),
        }
    }

    #[test]
    fn exercise_prompt_includes_module_title() {
        let ctx = k8s_pods_context();
        let prompt = build_exercise_system_prompt("conceptual_qa", 5, &ctx);
        assert!(
            prompt.contains("Pods, Nodes and Clusters"),
            "system prompt must include module title; got: {}",
            prompt
        );
    }

    #[test]
    fn exercise_prompt_includes_track_topic() {
        let ctx = k8s_pods_context();
        let prompt = build_exercise_system_prompt("conceptual_qa", 5, &ctx);
        assert!(
            prompt.contains("Kubernetes"),
            "system prompt must include track topic; got: {}",
            prompt
        );
    }

    #[test]
    fn exercise_prompt_includes_learning_objectives() {
        let ctx = k8s_pods_context();
        let prompt = build_exercise_system_prompt("conceptual_qa", 5, &ctx);
        assert!(
            prompt.contains("Define what a Pod is"),
            "system prompt must include learning objectives; got: {}",
            prompt
        );
    }

    #[test]
    fn exercise_prompt_forbids_unrelated_topics() {
        let ctx = k8s_pods_context();
        let prompt = build_exercise_system_prompt("conceptual_qa", 5, &ctx);
        let lower = prompt.to_lowercase();
        assert!(
            lower.contains("must be specifically about") || lower.contains("do not generate"),
            "prompt should explicitly constrain the model to the module topic; got: {}",
            prompt
        );
    }

    #[test]
    fn exercise_prompt_includes_content_excerpt_when_present() {
        let ctx = k8s_pods_context();
        let prompt = build_exercise_system_prompt("conceptual_qa", 5, &ctx);
        assert!(
            prompt.contains("smallest deployable unit"),
            "system prompt should include the module content excerpt; got: {}",
            prompt
        );
    }

    #[test]
    fn exercise_prompt_handles_empty_content_excerpt() {
        let mut ctx = k8s_pods_context();
        ctx.content_excerpt = String::new();
        let prompt = build_exercise_system_prompt("conceptual_qa", 5, &ctx);
        // No panic, still includes title
        assert!(prompt.contains("Pods, Nodes and Clusters"));
    }

    #[test]
    fn tutor_prompt_uses_db_context_when_present() {
        let ctx = k8s_pods_context();
        let prompt = build_tutor_system_prompt(Some(&ctx), "stale fallback");
        assert!(prompt.contains("Pods, Nodes and Clusters"));
        assert!(prompt.contains("Kubernetes"));
        assert!(prompt.contains("Define what a Pod is"));
        // DB context should win over fallback
        assert!(!prompt.contains("stale fallback"));
    }

    #[test]
    fn tutor_prompt_falls_back_to_string_when_no_context() {
        let prompt = build_tutor_system_prompt(None, "Track: t1, Module: m1 - Pods");
        assert!(prompt.contains("Track: t1, Module: m1 - Pods"));
    }

    #[test]
    fn tutor_prompt_works_with_no_context_and_no_fallback() {
        let prompt = build_tutor_system_prompt(None, "");
        // No panic, returns base prompt
        assert!(prompt.contains("AI tutor"));
    }

    #[test]
    fn tutor_prompt_softens_socratic_directive() {
        let ctx = k8s_pods_context();
        let prompt = build_tutor_system_prompt(Some(&ctx), "");
        // The user feedback was that strict-Socratic frustrates learners.
        // The prompt should explicitly say Socratic is for nudges only.
        let lower = prompt.to_lowercase();
        assert!(
            lower.contains("sparingly") || lower.contains("nudge"),
            "tutor prompt should soften the Socratic-only directive; got: {}",
            prompt
        );
        assert!(
            lower.contains("direct") && lower.contains("concise"),
            "tutor prompt should default to direct, concise answers; got: {}",
            prompt
        );
    }

    #[test]
    fn tutor_prompt_truncates_long_content_excerpt() {
        let mut ctx = k8s_pods_context();
        ctx.content_excerpt = "x".repeat(10_000);
        let prompt = build_tutor_system_prompt(Some(&ctx), "");
        let xs = prompt.matches('x').count();
        assert!(
            xs <= TUTOR_CONTENT_EXCERPT_MAX + 50,
            "tutor content excerpt should be truncated; got {} x's",
            xs
        );
        assert!(xs < 9000, "tutor content excerpt was not truncated");
    }

    #[test]
    fn exercise_prompt_truncates_long_content() {
        let mut ctx = k8s_pods_context();
        ctx.content_excerpt = "x".repeat(10_000);
        let prompt = build_exercise_system_prompt("conceptual_qa", 5, &ctx);
        // Excerpt itself capped at EXERCISE_CONTENT_EXCERPT_MAX (2000); a few
        // boilerplate x's (the word "exercise", "exam" in goal) push total
        // slightly above. Verify we don't blow up to the full 10k input.
        let xs = prompt.matches('x').count();
        assert!(
            xs <= EXERCISE_CONTENT_EXCERPT_MAX + 50,
            "content excerpt should be truncated; got {} x's (cap {})",
            xs, EXERCISE_CONTENT_EXCERPT_MAX
        );
        assert!(
            xs < 9000,
            "content excerpt was not actually truncated; got {} x's",
            xs
        );
    }
}
