use crate::ai::{ai_request, AIServiceRequest, ServiceMessage};
use crate::auth::AuthState;
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::State;

/// Extract JSON from AI response that may be wrapped in markdown code fences.
/// Public alias used by commands/blocks.rs (Phase 3 BLOCK-03 pipeline).
pub fn extract_json_pub(text: &str) -> Result<serde_json::Value, String> {
    extract_json(text)
}

fn extract_json(text: &str) -> Result<serde_json::Value, String> {
    let trimmed = text.trim();

    // Try direct parse first
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Ok(v);
    }

    // Fallback for responses wrapped in markdown fences or prose: extract the
    // outermost JSON container. The container may be an object (`{...}`) OR a
    // top-level array (`[...]`). Choose by whichever opener appears FIRST, then
    // slice to the matching last closer of the SAME kind. This preserves arrays
    // (video-ranking returns a top-level array) instead of stripping their `[` `]`
    // brackets, while keeping objects-with-inner-arrays parsing as objects.
    let obj_start = trimmed.find('{');
    let arr_start = trimmed.find('[');
    let slice = |start: usize, is_array: bool| -> Option<&str> {
        let close = if is_array { trimmed.rfind(']') } else { trimmed.rfind('}') }?;
        if close >= start {
            Some(&trimmed[start..=close])
        } else {
            None
        }
    };
    let candidate = match (obj_start, arr_start) {
        (Some(o), Some(a)) if a < o => slice(a, true),
        (Some(o), Some(_)) => slice(o, false),
        (Some(o), None) => slice(o, false),
        (None, Some(a)) => slice(a, true),
        (None, None) => None,
    };

    let stripped = candidate.unwrap_or(trimmed);
    serde_json::from_str(stripped).map_err(|e| {
        // Build the preview on a CHAR boundary. `trimmed.len()` is a BYTE
        // length; slicing `&trimmed[..200]` panics when byte 200 lands mid
        // multibyte codepoint (common with emoji, smart quotes, accented text,
        // CJK). Taking 200 chars can never split a codepoint. (CR-02)
        let preview: String = trimmed.chars().take(200).collect();
        format!("{} (first 200 chars: {:?})", e, preview)
    })
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
    /// Phase 5 Q3 lock — when `Some`, the AI generation is short-circuited and
    /// the path is built directly from the named pack's modules + edges. When
    /// `None`, the existing AI generation path runs unchanged (back-compat).
    #[serde(default)]
    pub pack_id: Option<String>,
}

#[tauri::command]
pub async fn generate_learning_path(
    auth: State<'_, AuthState>,
    state: State<'_, AppState>,
    request: GeneratePathRequest,
) -> Result<serde_json::Value, String> {
    // Phase 5 Q3 — pack_id short-circuit: build the path directly from the
    // named pack's modules + edges (no AI call) and persist. D-11 immutability
    // is preserved by using the same `learning_paths.modules_json` snapshot
    // column the AI path writes to.
    if let Some(pack_id) = request.pack_id.clone() {
        let registry = state
            .topic_packs
            .lock()
            .map_err(|e| format!("topic_packs lock poisoned: {}", e))?;
        let db = state.db.lock().map_err(|e| e.to_string())?;
        return generate_path_from_pack_impl(&db.conn, &registry, &request, &pack_id);
    }

    let topic = &request.topic;
    let system_prompt = format!(
        "You are a curriculum designer. Create a learning path for {topic} ({domain}). \
         Learner level: {level}. Goal: {goal}. \
         Gaps: {gaps:?}. Strengths: {strengths:?}. \
         \
         Generate 6-10 modules with REAL topic-specific titles and descriptions. \
         Each module ID MUST be a UUID (use format like \"mod-01\", \"mod-02\" etc). \
         \
         For each module, also list 1-3 capability tags in can-do phrasing — \
         short statements of what the learner will be ABLE TO DO after the module \
         (e.g. \"Can configure RBAC policies\", \"Can debug pod networking\"), never \
         topic nouns (NOT \"RBAC\", NOT \"Pod networking\"). If a module doesn't map \
         cleanly to a distinct capability, return an empty skills array for it. \
         \
         Return ONLY raw JSON, no markdown: \
         {{\"modules\": [{{\"id\": \"mod-01\", \"title\": \"...\", \"description\": \"...\", \
         \"difficulty\": 1, \"estimated_minutes\": 30, \"objectives\": [\"...\"], \
         \"skills\": [\"Can configure RBAC policies\"]}}], \
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
        use learnforge_core::path::{validate_dag, PathEdge, PathNode};
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
        validate_dag(&nodes, &path_edges)
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

// ── Pack-sourced path generation (Phase 5 Q3) ──

/// Build a learning path directly from a Topic Pack's modules + edges and
/// persist it, bypassing AI generation entirely. Module ids are namespaced
/// as `{pack_id}__{module_id}` because the `modules` table uses a global
/// `TEXT PRIMARY KEY` (schema.rs:44) — without namespacing, two packs that
/// both declare a module with id="intro" would collide across the DB
/// (T-05-18 mitigation).
///
/// Returns the same `serde_json::Value` shape the AI path returns so the
/// frontend can consume both flows uniformly.
///
/// Errors:
/// - `"Topic pack not found: <id>"` — pack id missing from the registry
/// - `"Topic pack is disabled: <id>"` — pack present but `enabled == false`
/// - DAG validation / SQLite errors — surface verbatim
pub fn generate_path_from_pack_impl(
    conn: &rusqlite::Connection,
    registry: &learnforge_core::packs::PackRegistry,
    request: &GeneratePathRequest,
    pack_id: &str,
) -> Result<serde_json::Value, String> {
    let loaded = registry
        .get(pack_id)
        .ok_or_else(|| format!("Topic pack not found: {}", pack_id))?;
    if !loaded.enabled {
        return Err(format!("Topic pack is disabled: {}", pack_id));
    }

    // Snapshot modules with namespacing.
    let ns = |id: &str| format!("{}__{}", pack_id, id);
    let modules_arr: Vec<serde_json::Value> = loaded
        .pack
        .modules
        .iter()
        .map(|m| {
            json!({
                "id": ns(&m.id),
                "title": m.title,
                "description": m.description,
                "difficulty": m.difficulty.unwrap_or(1),
                "estimated_minutes": m.estimated_minutes.unwrap_or(30),
                "objectives": m.objectives,
            })
        })
        .collect();
    let edges_arr: Vec<serde_json::Value> = loaded
        .pack
        .edges
        .iter()
        .map(|e| json!({ "from": ns(&e.from), "to": ns(&e.to) }))
        .collect();

    // Belt-and-suspenders: validate the pack-authored DAG before persisting
    // (T-05-20 mitigation — packs SHOULD be DAGs by authoring convention,
    // but we never trust on-disk authoring blindly).
    {
        use learnforge_core::path::{validate_dag, PathEdge, PathNode};
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
        validate_dag(&nodes, &path_edges)
            .map_err(|e| format!("Topic pack has invalid DAG: {}", e))?;
    }

    let path_id = uuid::Uuid::new_v4().to_string();
    let modules_json =
        serde_json::to_string(&modules_arr).unwrap_or_else(|_| "[]".to_string());
    let edges_json = serde_json::to_string(&edges_arr).unwrap_or_else(|_| "[]".to_string());
    let generated_by_model = format!("topic-pack:{}", pack_id);

    conn.execute(
        "INSERT INTO learning_paths (id, track_id, modules_json, edges_json, version, generated_by_model) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
        rusqlite::params![path_id, request.track_id, modules_json, edges_json, generated_by_model],
    )
    .map_err(|e| e.to_string())?;

    for (i, module) in modules_arr.iter().enumerate() {
        let module_id = module["id"].as_str().unwrap_or("").to_string();

        conn.execute(
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
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, status) \
             VALUES (?1, ?2, (SELECT id FROM learner_profiles LIMIT 1), ?3)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), module_id, status],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(json!({
        "id": path_id,
        "trackId": request.track_id,
        "modules": modules_arr,
        "edges": edges_arr,
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
    /// Block ID of the active section lesson (Phase 3 BLOCK-04 ABI extension).
    /// Wave 2 (03-03 Task 3) wires this into the tutor system prompt.
    /// Not yet used — declared here so frontend scaffolds can compile against the new field.
    #[serde(default)]
    pub current_section_id: Option<String>,
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

/// Load a section block's markdown payload excerpt for tutor grounding (BLOCK-04).
///
/// Returns Some(excerpt) if the block exists, has type='section', and has a non-empty markdown payload.
/// Returns None on any failure (block not found, not a section, missing markdown) — caller falls back.
/// Truncates to TUTOR_CONTENT_EXCERPT_MAX to avoid oversized system prompts.
fn load_section_excerpt(conn: &rusqlite::Connection, section_id: &str) -> Option<String> {
    use learnforge_core::blocks::BlockStore;
    let block = crate::storage_impl::blocks::SqliteBlockStore(conn)
        .get_by_id(section_id)
        .ok()
        .flatten()?;
    if block.block_type != "section" {
        return None;
    }
    let parsed: serde_json::Value = serde_json::from_str(&block.payload_json).ok()?;
    let md = parsed.get("markdown")?.as_str()?;
    if md.trim().is_empty() {
        return None;
    }
    let truncated = if md.len() > TUTOR_CONTENT_EXCERPT_MAX {
        // Find a safe char boundary at or after TUTOR_CONTENT_EXCERPT_MAX
        let start = md.len() - TUTOR_CONTENT_EXCERPT_MAX;
        let safe_start = md
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= start)
            .unwrap_or(start);
        &md[safe_start..]
    } else {
        md
    };
    Some(truncated.to_string())
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

    // Resolve context with BLOCK-04 grounding priority:
    // 1. currentSectionId → load section payload as primary context
    // 2. module_id → load exercise context (module overview)
    // 3. module_context string → legacy fallback
    let (ctx, section_excerpt) = if let Some(section_id) = &message.current_section_id {
        if !section_id.is_empty() {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            let excerpt = load_section_excerpt(&db.conn, section_id);
            // Also try module context as fallback
            let module_ctx = if excerpt.is_none() {
                message.module_id.as_deref()
                    .and_then(|id| load_exercise_context(state.inner(), id).ok())
            } else {
                None
            };
            (module_ctx, excerpt)
        } else {
            // Empty section_id — fall through to module context
            let ctx = match &message.module_id {
                Some(id) if !id.is_empty() => load_exercise_context(state.inner(), id).ok(),
                _ => None,
            };
            (ctx, None)
        }
    } else {
        // No currentSectionId — use module context path
        let ctx = match &message.module_id {
            Some(id) if !id.is_empty() => load_exercise_context(state.inner(), id).ok(),
            _ => None,
        };
        (ctx, None)
    };

    // Build system prompt with section excerpt taking priority over module context
    let fallback = section_excerpt.as_deref()
        .or(message.module_context.as_deref())
        .unwrap_or("");

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
    use learnforge_core::bkt::{update_mastery, BKTParams};

    // ── extract_json: top-level array handling (Phase 11 video-ranking bug) ──
    // The LLM video-ranking call (commands/videos.rs) is the only caller that
    // expects a TOP-LEVEL JSON array. The markdown-fence fallback was written
    // for objects and corrupted arrays by stripping the enclosing `[` `]`,
    // making every video-discovery call return Err → empty panel. These tests
    // lock the array contract; object cases guard against regression.

    #[test]
    fn extract_json_parses_bare_array() {
        let v = extract_json(r#"[{"videoId":"a","relevanceScore":0.9}]"#).unwrap();
        assert!(v.is_array(), "bare array must parse as array; got {:?}", v);
        assert_eq!(v.as_array().unwrap().len(), 1);
    }

    #[test]
    fn extract_json_parses_fenced_array() {
        // Claude (esp. haiku) routinely wraps JSON in ```json fences.
        let text = "```json\n[{\"videoId\":\"a\",\"relevanceScore\":0.9},{\"videoId\":\"b\",\"relevanceScore\":0.7}]\n```";
        let v = extract_json(text).unwrap();
        assert!(v.is_array(), "fenced array must parse as array; got {:?}", v);
        assert_eq!(v.as_array().unwrap().len(), 2);
    }

    #[test]
    fn extract_json_parses_array_with_preamble() {
        let text = "Here are the scores:\n[{\"videoId\":\"a\",\"relevanceScore\":0.9}]";
        let v = extract_json(text).unwrap();
        assert!(v.is_array(), "array with prose preamble must parse as array; got {:?}", v);
        assert_eq!(v.as_array().unwrap().len(), 1);
    }

    #[test]
    fn extract_json_still_parses_fenced_object() {
        // Regression guard: object callers (block generation) must keep working.
        let v = extract_json("```json\n{\"blocks\":[1,2]}\n```").unwrap();
        assert!(v.is_object(), "fenced object must still parse as object; got {:?}", v);
    }

    #[test]
    fn extract_json_prefers_outer_container_by_first_opener() {
        // An object whose values contain arrays must not be mistaken for an array.
        let v = extract_json("```json\n{\"items\":[{\"x\":1}]}\n```").unwrap();
        assert!(v.is_object(), "outer object with inner arrays must parse as object; got {:?}", v);
    }

    #[test]
    fn extract_json_does_not_panic_on_multibyte_boundary_in_error_path() {
        // CR-02: the parse-failure error preview must be built on a char
        // boundary. A non-JSON multibyte string > 200 chars where byte index
        // 200 falls MID-codepoint would panic if sliced by byte index.
        //
        // "é" is 2 bytes (U+00E9). Prefix with a single ASCII "x" so every "é"
        // starts at an ODD byte offset; byte index 200 then falls MID-codepoint.
        // ("x" + 150×"é" = 1 + 300 = 301 bytes; byte 200 = "x"(1) + 199 → inside
        // the 100th "é".) This is NOT valid JSON, so extract_json errors —
        // exercising the char-boundary preview in the error path.
        let multibyte = format!("x{}", "é".repeat(150));
        assert!(multibyte.len() >= 300, "fixture must exceed 200 bytes");
        assert!(!multibyte.is_char_boundary(200), "byte 200 must be mid-codepoint");

        // Must return Err (not panic) — fail-soft contract (D-09).
        let result = extract_json(&multibyte);
        assert!(result.is_err(), "non-JSON multibyte input must return Err, not panic");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("first 200 chars"),
            "error message must include the char-boundary preview; got: {}",
            msg
        );
    }

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

    // ── Task 3: Tutor section grounding tests (BLOCK-04) ──

    fn fresh_conn_with_schema() -> rusqlite::Connection {
        use crate::db::migrations::apply_migrations;
        use crate::db::schema;
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    fn seed_module_with_section_block(
        conn: &rusqlite::Connection,
        module_id: &str,
        block_id: &str,
        section_markdown: &str,
    ) {
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id) VALUES ('lp-tutor')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO learning_tracks (id, learner_id, topic, domain_module) VALUES ('trk-tutor', 'lp-tutor', 'Kubernetes', 'devops')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO learning_paths (id, track_id) VALUES ('path-tutor', 'trk-tutor')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO modules (id, path_id, title, description, objectives_json) VALUES (?1, 'path-tutor', 'Test Module', 'A test module', '[]')",
            [module_id],
        ).unwrap();

        // Insert section block with markdown payload
        let payload = serde_json::json!({
            "markdown": section_markdown,
            "wordCount": section_markdown.split_whitespace().count()
        }).to_string();
        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status, params_json, payload_json)
             VALUES (?1, ?2, 0, 'section', 'ready', '{}', ?3)",
            rusqlite::params![block_id, module_id, payload],
        ).unwrap();
    }

    /// tutor_uses_section_payload_when_provided:
    /// Mock module_blocks with section block id="b1" payload {"markdown":"<content about Pods>"}.
    /// Build tutor prompt with currentSectionId="b1". Assert system prompt contains section excerpt.
    #[test]
    fn tutor_uses_section_payload_when_provided() {
        let conn = fresh_conn_with_schema();
        let section_content = "A Pod is a group of one or more containers sharing storage and network resources.";
        seed_module_with_section_block(&conn, "mod-tutor-1", "b1", section_content);

        let excerpt = load_section_excerpt(&conn, "b1");
        assert!(excerpt.is_some(), "load_section_excerpt must return Some for existing section block");

        let excerpt = excerpt.unwrap();
        assert!(
            excerpt.contains("Pod is a group"),
            "excerpt must contain section markdown content; got: {}",
            excerpt
        );

        // The system prompt built with this excerpt as fallback must contain the content
        let prompt = build_tutor_system_prompt(None, &excerpt);
        assert!(
            prompt.contains("Pod is a group"),
            "tutor system prompt must contain section excerpt when provided; got: {}",
            prompt
        );

        // Excerpt must be truncated to TUTOR_CONTENT_EXCERPT_MAX
        assert!(
            excerpt.len() <= TUTOR_CONTENT_EXCERPT_MAX,
            "excerpt must be truncated to TUTOR_CONTENT_EXCERPT_MAX ({}); got len {}",
            TUTOR_CONTENT_EXCERPT_MAX,
            excerpt.len()
        );
    }

    /// tutor_falls_back_to_module_overview_when_section_missing:
    /// Pass currentSectionId="nonexistent". load_section_excerpt returns None — no crash.
    #[test]
    fn tutor_falls_back_to_module_overview_when_section_missing() {
        let conn = fresh_conn_with_schema();
        // No blocks seeded — section lookup must gracefully return None
        let excerpt = load_section_excerpt(&conn, "nonexistent-block-id");
        assert!(excerpt.is_none(), "load_section_excerpt must return None for missing block");

        // Tutor prompt falls back gracefully to module overview (empty fallback here)
        let prompt = build_tutor_system_prompt(None, "module overview fallback");
        assert!(
            prompt.contains("module overview fallback"),
            "when section missing, prompt uses fallback; got: {}",
            prompt
        );
        // No crash — test passes by not panicking
    }

    /// tutor_falls_back_when_no_section_id:
    /// currentSectionId=None. Assert prompt uses module overview path (regression guard for Phase 1).
    #[test]
    fn tutor_falls_back_when_no_section_id() {
        // When no currentSectionId, the tutor uses the module context (ExerciseContext path)
        let ctx = k8s_pods_context();
        let prompt = build_tutor_system_prompt(Some(&ctx), "fallback string");

        // Phase 1 behavior: DB context wins over fallback string
        assert!(
            prompt.contains("Pods, Nodes and Clusters"),
            "Phase 1 module context must be used when no currentSectionId; got: {}",
            prompt
        );
        assert!(
            !prompt.contains("fallback string"),
            "DB context should win over fallback; got: {}",
            prompt
        );
    }

    /// integration_tutor_section_grounding:
    /// Stub block in DB, call load_section_excerpt, assert prompt contains section content.
    /// Full integration: verify the grounding chain works end-to-end (without actual LLM call).
    #[test]
    fn integration_tutor_section_grounding() {
        let conn = fresh_conn_with_schema();
        let section_content = "Kubernetes uses a control plane to manage workloads across nodes. \
            The API server is the central management point for all cluster operations. \
            etcd stores the cluster state as a distributed key-value store.";
        seed_module_with_section_block(&conn, "mod-grounded", "section-ctrl", section_content);

        // Step 1: Verify section excerpt is loadable
        let excerpt = load_section_excerpt(&conn, "section-ctrl").unwrap();
        assert!(
            excerpt.contains("control plane"),
            "excerpt must contain section content about control plane; got: {}",
            excerpt
        );
        assert!(
            excerpt.contains("API server"),
            "excerpt must contain section content about API server; got: {}",
            excerpt
        );

        // Step 2: Build tutor prompt using section excerpt as primary context
        let prompt = build_tutor_system_prompt(None, &excerpt);
        assert!(
            prompt.contains("control plane"),
            "tutor system prompt must include section excerpt content; got: {}",
            prompt
        );
        assert!(
            prompt.contains("API server"),
            "tutor system prompt must include API server mention; got: {}",
            prompt
        );

        // Step 3: Verify fallback chain — non-existent section falls back gracefully
        let missing_excerpt = load_section_excerpt(&conn, "does-not-exist");
        assert!(missing_excerpt.is_none(), "missing section must return None");

        // Step 4: Verify non-section block (quiz) returns None
        conn.execute(
            "INSERT INTO module_blocks (id, module_id, ordering, block_type, status, params_json, payload_json)
             VALUES ('blk-quiz', 'mod-grounded', 1, 'quiz', 'ready', '{}', '{\"questions\":[]}')",
            [],
        ).unwrap();
        let quiz_excerpt = load_section_excerpt(&conn, "blk-quiz");
        assert!(
            quiz_excerpt.is_none(),
            "non-section block (quiz) must return None from load_section_excerpt"
        );
    }

    // ── Phase 5 Q3 — pack_id short-circuit tests ──

    use learnforge_core::packs::{
        LoadedPack as TpLoadedPack, Pack as TpPack, PackEdge as TpPackEdge,
        PackModule as TpPackModule, PackRegistry as TpPackRegistry, PackSource as TpPackSource,
        ValidationStatus as TpValidationStatus,
    };

    fn make_pack_fixture(id: &str, enabled: bool) -> TpLoadedPack {
        TpLoadedPack {
            pack: TpPack {
                id: id.to_string(),
                title: format!("{} Pack", id),
                description: format!("desc for {}", id),
                domain_module: "devops".to_string(),
                estimated_hours: Some(8),
                pack_version: "1.0".to_string(),
                requires_docker: false,
                modules: vec![
                    TpPackModule {
                        id: "intro".to_string(),
                        title: "Intro to the topic".to_string(),
                        description: "starter module".to_string(),
                        difficulty: Some(2),
                        estimated_minutes: Some(30),
                        objectives: vec!["learn basics".to_string()],
                        exercise_types: vec!["conceptual_qa".to_string()],
                    },
                    TpPackModule {
                        id: "advanced".to_string(),
                        title: "Advanced patterns".to_string(),
                        description: "deeper module".to_string(),
                        difficulty: Some(6),
                        estimated_minutes: Some(60),
                        objectives: vec!["master patterns".to_string()],
                        exercise_types: vec!["code_challenge".to_string()],
                    },
                    TpPackModule {
                        id: "mastery".to_string(),
                        title: "Mastery checkpoint".to_string(),
                        description: "graduation".to_string(),
                        difficulty: Some(9),
                        estimated_minutes: Some(45),
                        objectives: vec!["demonstrate mastery".to_string()],
                        exercise_types: vec![],
                    },
                ],
                edges: vec![
                    TpPackEdge {
                        from: "intro".to_string(),
                        to: "advanced".to_string(),
                    },
                    TpPackEdge {
                        from: "advanced".to_string(),
                        to: "mastery".to_string(),
                    },
                ],
            },
            source: TpPackSource::Bundled,
            enabled,
            validation_status: TpValidationStatus::Ok,
            validation_messages: vec![],
            last_loaded_at: "2026-06-15T00:00:00Z".to_string(),
        }
    }

    fn registry_with_packs(packs: Vec<TpLoadedPack>) -> TpPackRegistry {
        let mut r = TpPackRegistry::default();
        for p in packs {
            r.packs.insert(p.pack.id.clone(), p);
        }
        r
    }

    fn db_with_track(conn: &rusqlite::Connection, track_id: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id) VALUES ('lp-pack-test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module) VALUES (?1, 'lp-pack-test', 'TestTopic', 'devops')",
            [track_id],
        )
        .unwrap();
    }

    fn make_request(track_id: &str, pack_id: Option<&str>) -> GeneratePathRequest {
        GeneratePathRequest {
            track_id: track_id.to_string(),
            topic: "TestTopic".to_string(),
            domain: "devops".to_string(),
            goal: "learn things".to_string(),
            assessment_level: "beginner".to_string(),
            assessment_gaps: vec![],
            assessment_strengths: vec![],
            pack_id: pack_id.map(String::from),
        }
    }

    /// generate_path_from_pack_uses_pack_modules: registry has a bundled pack
    /// with 3 modules; call helper with pack_id; assert modules_json
    /// deserializes to 3 modules with the exact ids/titles from the pack
    /// (with `{pack_id}__` namespacing applied — T-05-18 mitigation).
    #[test]
    fn generate_path_from_pack_uses_pack_modules() {
        let conn = fresh_conn_with_schema();
        db_with_track(&conn, "trk-pack-1");
        let reg = registry_with_packs(vec![make_pack_fixture("agentic-devops", true)]);
        let request = make_request("trk-pack-1", Some("agentic-devops"));

        let result = generate_path_from_pack_impl(&conn, &reg, &request, "agentic-devops")
            .expect("pack short-circuit must succeed for enabled known pack");

        // Returned shape mirrors AI path shape (id, trackId, modules, edges).
        let modules = result["modules"].as_array().expect("modules array");
        assert_eq!(modules.len(), 3, "pack has 3 modules");
        assert_eq!(modules[0]["id"], "agentic-devops__intro");
        assert_eq!(modules[0]["title"], "Intro to the topic");
        assert_eq!(modules[1]["id"], "agentic-devops__advanced");
        assert_eq!(modules[2]["id"], "agentic-devops__mastery");

        // Edges carry the same namespacing.
        let edges = result["edges"].as_array().expect("edges array");
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0]["from"], "agentic-devops__intro");
        assert_eq!(edges[0]["to"], "agentic-devops__advanced");

        // SQLite reflects the snapshot.
        let modules_json: String = conn
            .query_row(
                "SELECT modules_json FROM learning_paths WHERE track_id = 'trk-pack-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&modules_json).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 3);

        // modules table got 3 rows, each id namespaced.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM modules WHERE id LIKE 'agentic-devops__%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 3, "modules table should have 3 namespaced rows");
    }

    /// generate_path_from_pack_writes_generated_by_model: assert
    /// `generated_by_model` column starts with "topic-pack:".
    #[test]
    fn generate_path_from_pack_writes_generated_by_model() {
        let conn = fresh_conn_with_schema();
        db_with_track(&conn, "trk-pack-2");
        let reg = registry_with_packs(vec![make_pack_fixture("ai-engineering", true)]);
        let request = make_request("trk-pack-2", Some("ai-engineering"));

        generate_path_from_pack_impl(&conn, &reg, &request, "ai-engineering").unwrap();

        let model: String = conn
            .query_row(
                "SELECT generated_by_model FROM learning_paths WHERE track_id = 'trk-pack-2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            model.starts_with("topic-pack:"),
            "generated_by_model must start with 'topic-pack:'; got: {}",
            model
        );
        assert!(
            model.contains("ai-engineering"),
            "generated_by_model must mention pack id; got: {}",
            model
        );
    }

    /// generate_path_from_unknown_pack_errors: registry is empty; assert Err
    /// "Topic pack not found".
    #[test]
    fn generate_path_from_unknown_pack_errors() {
        let conn = fresh_conn_with_schema();
        db_with_track(&conn, "trk-pack-3");
        let reg = registry_with_packs(vec![]);
        let request = make_request("trk-pack-3", Some("ghost-pack"));

        let err = generate_path_from_pack_impl(&conn, &reg, &request, "ghost-pack")
            .expect_err("unknown pack must Err");
        assert!(
            err.contains("Topic pack not found"),
            "error must mention 'Topic pack not found'; got: {}",
            err
        );
        assert!(err.contains("ghost-pack"), "error must mention the id; got: {}", err);

        // No rows leaked into learning_paths.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM learning_paths WHERE track_id = 'trk-pack-3'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "no learning_paths row should be inserted on Err");
    }

    /// generate_path_from_disabled_pack_errors: registry has pack with
    /// enabled=false; assert Err mentions "disabled" (T-05-19 mitigation).
    #[test]
    fn generate_path_from_disabled_pack_errors() {
        let conn = fresh_conn_with_schema();
        db_with_track(&conn, "trk-pack-4");
        let reg = registry_with_packs(vec![make_pack_fixture("kubernetes-fundamentals", false)]);
        let request = make_request("trk-pack-4", Some("kubernetes-fundamentals"));

        let err =
            generate_path_from_pack_impl(&conn, &reg, &request, "kubernetes-fundamentals")
                .expect_err("disabled pack must Err");
        assert!(
            err.to_lowercase().contains("disabled"),
            "error must mention 'disabled'; got: {}",
            err
        );

        // No rows leaked.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM learning_paths WHERE track_id = 'trk-pack-4'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    /// generate_path_from_pack_does_not_call_ai: the helper does not take an
    /// `AuthState` and does not invoke `ai_request` — this is enforced by the
    /// type signature (compile-time guarantee). This test additionally proves
    /// the helper is sync-callable from a non-async context.
    #[test]
    fn generate_path_from_pack_does_not_call_ai() {
        let conn = fresh_conn_with_schema();
        db_with_track(&conn, "trk-pack-5");
        let reg = registry_with_packs(vec![make_pack_fixture("rust-from-zero", true)]);
        let request = make_request("trk-pack-5", Some("rust-from-zero"));

        // Note: This call site is NOT inside `tokio::task::block_on` or any
        // async runtime. If `generate_path_from_pack_impl` ever started calling
        // `.await` on `ai_request` it would either fail to compile (not async)
        // or panic at runtime (no runtime). Calling it here in a sync test
        // proves the AI code path is statically excluded.
        let result = generate_path_from_pack_impl(&conn, &reg, &request, "rust-from-zero")
            .expect("must succeed without AI");
        assert_eq!(result["modules"].as_array().unwrap().len(), 3);
    }

    /// Cross-pack namespacing test: two packs both define module id="intro"
    /// → both can coexist in `modules` table without UNIQUE collision
    /// (T-05-18 mitigation). Belt-and-suspenders alongside the test above.
    #[test]
    fn generate_path_from_two_packs_namespaces_module_ids() {
        let conn = fresh_conn_with_schema();
        db_with_track(&conn, "trk-cross-a");
        db_with_track(&conn, "trk-cross-b");
        let reg = registry_with_packs(vec![
            make_pack_fixture("pack-a", true),
            make_pack_fixture("pack-b", true),
        ]);

        generate_path_from_pack_impl(
            &conn,
            &reg,
            &make_request("trk-cross-a", Some("pack-a")),
            "pack-a",
        )
        .expect("first pack");
        generate_path_from_pack_impl(
            &conn,
            &reg,
            &make_request("trk-cross-b", Some("pack-b")),
            "pack-b",
        )
        .expect("second pack with same module-ids (intro/advanced/mastery)");

        let intro_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM modules WHERE id IN ('pack-a__intro', 'pack-b__intro')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            intro_count, 2,
            "namespacing must let both packs' 'intro' modules coexist"
        );
    }
}
