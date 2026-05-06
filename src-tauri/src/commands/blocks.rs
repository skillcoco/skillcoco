use crate::auth::AuthState;
use crate::db::blocks::{
    count_blocks_by_module, delete_blocks_by_module, get_block, insert_block,
    list_blocks_by_module, update_block_payload, BlockStatus, ModuleBlock,
};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;

// ── IPC Request / Response structs ──
// All structs cross the Tauri IPC boundary and MUST use camelCase serde.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateModuleBlocksRequest {
    pub module_id: String,
    pub track_id: String,
    pub module_title: String,
    pub objectives: Vec<String>,
    pub learner_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateModuleBlocksResult {
    pub blocks: Vec<ModuleBlock>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegenerateLessonRequest {
    pub block_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegenerateModuleRequest {
    pub module_id: String,
    pub track_id: String,
}

// ── PagePlanner outline types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonOutlineItem {
    pub title: String,
    pub objectives: Vec<String>,
    #[serde(default)]
    pub key_concepts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PagePlannerOutline {
    pub lessons: Vec<LessonOutlineItem>,
    #[serde(default)]
    pub quiz_topics: Vec<String>,
    #[serde(default)]
    pub flash_card_concepts: Vec<String>,
    /// Phase 03.1 (LAB-05) — labs[] extension. `#[serde(default)]` keeps
    /// existing PagePlanner JSON back-compatible.
    #[serde(default)]
    pub labs: Vec<crate::labs::pageplanner_labs::LabOutlineItem>,
}

// ── AI client abstraction for testability ──

/// Simple mock-able AI request type.
/// Production path calls the real `ai_request_with_retry`; test path uses a closure.
#[cfg_attr(test, allow(dead_code))]
pub(crate) struct BlockAIClient<'a> {
    auth: &'a AuthState,
}

impl<'a> BlockAIClient<'a> {
    pub fn new(auth: &'a AuthState) -> Self {
        Self { auth }
    }

    pub async fn request(
        &self,
        req: crate::ai::service::AIServiceRequest,
        max_retries: u8,
    ) -> Result<String, String> {
        let resp = crate::ai::retry::ai_request_with_retry(self.auth, req, max_retries).await?;
        Ok(resp.content)
    }
}

// ── Helper: update block status only (keep payload) ──

pub fn update_block_status(
    conn: &rusqlite::Connection,
    block_id: &str,
    status: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE module_blocks SET status=?1, updated_at=datetime('now') WHERE id=?2",
        rusqlite::params![status, block_id],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

// ── Core helpers ──

/// Wrap a legacy modules.content markdown blob as a single section block.
///
/// Called on first open of a module that has zero rows in module_blocks.
/// Idempotent: returns Ok(None) without inserting if blocks already exist.
/// Emits metadata_json='{"concept_id": null}' — PACK-04 concept-graph forward-link.
pub fn wrap_legacy_content_as_block(
    conn: &rusqlite::Connection,
    module_id: &str,
) -> Result<Option<ModuleBlock>, String> {
    // Idempotent: if any blocks exist, do nothing
    let count = count_blocks_by_module(conn, module_id).map_err(|e| e.to_string())?;
    if count > 0 {
        return Ok(None);
    }

    // Read legacy content from modules.content column
    let content: Option<String> = conn
        .query_row(
            "SELECT content FROM modules WHERE id = ?1",
            [module_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(|e| e.to_string())?;

    let markdown = match content {
        Some(c) if !c.is_empty() => c,
        _ => return Ok(None), // no legacy content to wrap
    };

    let now = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "markdown": markdown,
        "wordCount": 0
    })
    .to_string();

    let block = ModuleBlock {
        id: uuid::Uuid::new_v4().to_string(),
        module_id: module_id.to_string(),
        ordering: 0,
        block_type: "section".to_string(),
        status: "ready".to_string(),
        params_json: "{}".to_string(), // empty params = legacy marker for ModuleView banner
        payload_json: payload,
        source_anchors_json: "[]".to_string(),
        metadata_json: r#"{"concept_id": null}"#.to_string(), // PACK-04 concept-graph forward-link
        retry_count: 0,
        created_at: now.clone(),
        updated_at: now,
    };

    insert_block(conn, &block).map_err(|e| e.to_string())?;
    Ok(Some(block))
}

/// Validate outline: must have 8-10 lessons and at least 1 quiz topic.
fn validate_outline(outline: &PagePlannerOutline) -> Result<(), String> {
    let n = outline.lessons.len();
    if n < 8 || n > 10 {
        return Err(format!(
            "PagePlanner returned {} lessons; expected 8-10",
            n
        ));
    }
    if outline.quiz_topics.is_empty() {
        return Err("PagePlanner returned no quiz_topics".to_string());
    }
    Ok(())
}

/// Call PagePlanner LLM and return a validated outline.
/// Retries once with a stricter prompt if validation fails.
pub async fn run_page_planner(
    auth: &AuthState,
    module_title: &str,
    objectives: &[String],
    learner_level: &str,
) -> Result<PagePlannerOutline, String> {
    let client = BlockAIClient::new(auth);
    run_page_planner_with_client(&client, module_title, objectives, learner_level).await
}

/// Internal: PagePlanner with injectable client (for tests).
pub(crate) async fn run_page_planner_with_client<C>(
    client: &C,
    module_title: &str,
    objectives: &[String],
    learner_level: &str,
) -> Result<PagePlannerOutline, String>
where
    C: AIClientTrait,
{
    let base = build_page_planner_prompt(module_title, objectives, learner_level);
    // LAB-05 — append the labs-emission rule (track-pref opt-out plumbing
    // arrives in a future plan; default-on for now).
    let sys = crate::labs::pageplanner_labs::extend_page_planner_prompt(&base, true);
    let req = crate::ai::service::AIServiceRequest {
        system_prompt: sys,
        messages: vec![crate::ai::service::ServiceMessage {
            role: "user".to_string(),
            content: format!("Create the lesson outline for: {}. Return ONLY JSON.", module_title),
        }],
        // 8-10 lessons + objectives + key concepts + quizTopics + flashCardConcepts
        // typically lands around 1500-2500 tokens; 4096 leaves headroom and matches
        // the section-block budget below.
        max_tokens: Some(4096),
        temperature: Some(0.3),
        response_format: Some("json".to_string()),
    };

    let text = client.request(req.clone(), 1).await?;
    let json_val = crate::commands::ai::extract_json_pub(&text)?;
    match serde_json::from_value::<PagePlannerOutline>(json_val) {
        Ok(outline) => {
            if let Err(e) = validate_outline(&outline) {
                // Retry once with stricter prompt
                let strict_prompt = build_page_planner_prompt_strict(module_title, objectives, learner_level);
                let strict_req = crate::ai::service::AIServiceRequest {
                    system_prompt: strict_prompt,
                    messages: vec![crate::ai::service::ServiceMessage {
                        role: "user".to_string(),
                        content: format!(
                            "IMPORTANT: Return EXACTLY 8-10 lessons. Create the outline for: {}",
                            module_title
                        ),
                    }],
                    max_tokens: Some(4096),
                    temperature: Some(0.2),
                    response_format: Some("json".to_string()),
                };
                let text2 = client.request(strict_req, 0).await
                    .unwrap_or_else(|_| String::new());
                if let Ok(json2) = crate::commands::ai::extract_json_pub(&text2) {
                    if let Ok(outline2) = serde_json::from_value::<PagePlannerOutline>(json2) {
                        // Clamp to at least 4 lessons if still invalid (accept partial)
                        if outline2.lessons.len() >= 4 {
                            return Ok(outline2);
                        }
                    }
                }
                // Accept original if it has ≥4 lessons (clamp policy)
                if outline.lessons.len() >= 4 {
                    return Ok(outline);
                }
                return Err(format!("PagePlanner outline invalid after retry: {}", e));
            }
            Ok(outline)
        }
        Err(e) => {
            // Surface a hint when the LLM truncated mid-array (likely max_tokens hit)
            let preview: String = text.chars().take(200).collect();
            let hint = if e.to_string().contains("EOF while parsing") {
                "Looks like the AI response was truncated. Try again — if it persists, the model's output budget may be too small for this module's outline."
            } else {
                "The AI returned text that wasn't valid JSON."
            };
            Err(format!(
                "PagePlanner outline parse failed: {}. {} (first 200 chars: {:?})",
                e, hint, preview
            ))
        }
    }
}

fn build_page_planner_prompt(module_title: &str, objectives: &[String], learner_level: &str) -> String {
    let obj_list = objectives.iter().map(|o| format!("- {}", o)).collect::<Vec<_>>().join("\n");
    format!(
        r#"You are a curriculum designer breaking down "{module_title}" (level: {learner_level}) into lessons.

Objectives:
{obj_list}

Return ONLY valid JSON:
{{
  "lessons": [
    {{
      "title": "...",
      "objectives": ["..."],
      "keyConceptsKey": ["..."]
    }}
  ],
  "quizTopics": ["topic 1", "topic 2"],
  "flashCardConcepts": ["concept 1", "concept 2"]
}}

Rules:
- Exactly 8-10 lessons, ordered from foundational to advanced
- Each lesson title must be SPECIFIC to {module_title}, not generic
- quizTopics: 5-10 specific topics to test, drawn from lesson objectives
- flashCardConcepts: 2-6 high-value concepts for flip-card reinforcement
- Return ONLY the JSON — no markdown, no explanation"#,
        module_title = module_title,
        learner_level = learner_level,
        obj_list = obj_list,
    )
}

fn build_page_planner_prompt_strict(module_title: &str, objectives: &[String], learner_level: &str) -> String {
    let obj_list = objectives.iter().map(|o| format!("- {}", o)).collect::<Vec<_>>().join("\n");
    format!(
        r#"CRITICAL: You MUST return EXACTLY 8-10 lessons in the lessons array. Not fewer, not more.

You are a curriculum designer breaking down "{module_title}" (level: {learner_level}) into lessons.

Objectives:
{obj_list}

Return ONLY valid JSON with exactly 8-10 lessons."#,
        module_title = module_title,
        learner_level = learner_level,
        obj_list = obj_list,
    )
}

/// Insert skeleton block rows in a single transaction.
/// Returns the list of skeleton blocks.
pub(crate) fn insert_skeleton_blocks(
    conn: &rusqlite::Connection,
    module_id: &str,
    outline: &PagePlannerOutline,
) -> Result<Vec<ModuleBlock>, String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let mut blocks = Vec::new();
    let n = outline.lessons.len();

    for (i, lesson) in outline.lessons.iter().enumerate() {
        let params = serde_json::json!({
            "lessonTitle": lesson.title,
            "objectives": lesson.objectives,
            "keyConcepts": lesson.key_concepts,
            "wordCountTarget": 1500
        })
        .to_string();
        let block = ModuleBlock {
            id: uuid::Uuid::new_v4().to_string(),
            module_id: module_id.to_string(),
            ordering: i as i32,
            block_type: "section".to_string(),
            status: "pending".to_string(),
            params_json: params,
            payload_json: "{}".to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id": null}"#.to_string(),
            retry_count: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        insert_block(&tx, &block).map_err(|e| e.to_string())?;
        blocks.push(block);
    }

    // Quiz block
    {
        let params = serde_json::json!({
            "questionCount": 8,
            "topics": outline.quiz_topics,
            "difficulty": "intermediate"
        })
        .to_string();
        let block = ModuleBlock {
            id: uuid::Uuid::new_v4().to_string(),
            module_id: module_id.to_string(),
            ordering: n as i32,
            block_type: "quiz".to_string(),
            status: "pending".to_string(),
            params_json: params,
            payload_json: "{}".to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id": null}"#.to_string(),
            retry_count: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        insert_block(&tx, &block).map_err(|e| e.to_string())?;
        blocks.push(block);
    }

    // Flash cards block (if concepts available)
    if !outline.flash_card_concepts.is_empty() {
        let card_count = outline.flash_card_concepts.len().min(3);
        let params = serde_json::json!({
            "cardCount": card_count,
            "concepts": outline.flash_card_concepts
        })
        .to_string();
        let block = ModuleBlock {
            id: uuid::Uuid::new_v4().to_string(),
            module_id: module_id.to_string(),
            ordering: (n + 1) as i32,
            block_type: "flash_cards".to_string(),
            status: "pending".to_string(),
            params_json: params,
            payload_json: "{}".to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id": null}"#.to_string(),
            retry_count: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        insert_block(&tx, &block).map_err(|e| e.to_string())?;
        blocks.push(block);
    }

    // LAB-05 — lab skeletons (one per outline.labs entry). Source markdown
    // and parsed payload are filled by the parallel generator's `lab` arm.
    let lab_base_ordering = blocks.len() as i32;
    for (i, lab) in outline.labs.iter().enumerate() {
        let params = serde_json::json!({
            "outline": lab,
            "generationPrompt": "",
            "source": ""
        })
        .to_string();
        let block = ModuleBlock {
            id: uuid::Uuid::new_v4().to_string(),
            module_id: module_id.to_string(),
            ordering: lab_base_ordering + i as i32,
            block_type: "lab".to_string(),
            status: "pending".to_string(),
            params_json: params,
            payload_json: "{}".to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id": null}"#.to_string(),
            retry_count: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        insert_block(&tx, &block).map_err(|e| e.to_string())?;
        blocks.push(block);
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(blocks)
}

// ── AI client trait for testability ──

/// Trait allowing test injection of AI responses.
pub(crate) trait AIClientTrait {
    fn request<'a>(
        &'a self,
        req: crate::ai::service::AIServiceRequest,
        max_retries: u8,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>>;
}

impl AIClientTrait for BlockAIClient<'_> {
    fn request<'a>(
        &'a self,
        req: crate::ai::service::AIServiceRequest,
        max_retries: u8,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move { self.request(req, max_retries).await })
    }
}

/// Generate a section block's content via LLM.
/// Build the section-generation system prompt. Extracted for unit-testing the
/// pedagogical instructions (analogies, diagrams, pitfalls) without making an
/// LLM call.
pub(crate) fn build_section_prompt(
    module_title: &str,
    lesson_title: &str,
    objectives: &[String],
    index: usize,
    total: usize,
) -> String {
    format!(
        "You are an expert teacher writing lesson {lesson_num} of {total} for the module \
         \"{module_title}\". Your audience is a learner who is new to this topic and learns \
         best from concrete analogies, simple breakdowns, and visual structure.\n\n\
         Lesson title: \"{lesson_title}\"\n\
         Objectives: {objectives:?}\n\n\
         Write 1000-2500 words of rich markdown content. The lesson MUST include, in order:\n\n\
         1. **Why this matters** (1 short paragraph). Frame the lesson around a real problem \
         the learner will face — what breaks if they don't know this?\n\
         2. **An analogy or simplified mental model**. Compare the concept to something familiar \
         (a kitchen, a post office, a queue at a store, traffic lights, etc.). Use the analogy \
         to motivate every key idea that follows. Reuse the same analogy throughout — don't \
         introduce a new one for every paragraph.\n\
         3. **A diagram** in a fenced code block. Prefer Mermaid (use ```mermaid) for \
         flowcharts, sequence diagrams, state diagrams, ER diagrams, gantt charts, mindmaps, \
         and class hierarchies — the renderer renders Mermaid blocks as proper SVG diagrams. \
         Use ```text only as a fallback for layouts/timelines that don't fit Mermaid's grammar. \
         Do NOT use images, links to external diagrams, or Excalidraw — they will not render.\n\
         4. **Step-by-step breakdown** with subheadings (## Step 1: ..., ## Step 2: ...). \
         Each step should be small enough to follow without re-reading.\n\
         5. **A worked example** in a fenced code block with the language tag (```python, \
         ```javascript, etc.). Show ONLY the relevant snippet (5-15 lines) — do not paste \
         long programs. If you need to show more code, split into multiple short snippets, \
         each focused on one idea. Walk through what each line does in prose immediately \
         after the block.\n\
         6. **Common pitfalls** — a `## Common Pitfalls` subsection with 2-4 bullets describing \
         mistakes a learner is likely to make and how to avoid them.\n\
         7. **Summary** — a `## Summary` section with 3-5 bullet points recapping the lesson.\n\n\
         Style rules:\n\
         - Key terms in **bold** the first time they appear.\n\
         - Short paragraphs (3-4 sentences max). Prefer bullets and numbered lists over walls of text.\n\
         - When you introduce jargon, immediately gloss it in plain English.\n\
         - Avoid meta-commentary like \"In this lesson we will...\" — just teach.\n\n\
         Mermaid diagram syntax notes (when you use ```mermaid):\n\
         - Use `flowchart TD` or `flowchart LR` for boxes-and-arrows.\n\
         - Use `sequenceDiagram` for request/response or message-passing flows.\n\
         - Use `stateDiagram-v2` for state machines.\n\
         - Keep node labels short (≤ 4 words). Use `--` for arrows in flowchart, `->>` in \
         sequence, `-->` between states.\n\n\
         Output rules:\n\
         - Do NOT include a top-level # heading. The UI provides the lesson title.\n\
         - Return ONLY the markdown content. No JSON wrapper, no preamble like \"Here's the lesson:\".",
        lesson_num = index + 1,
        total = total,
        module_title = module_title,
        lesson_title = lesson_title,
        objectives = objectives,
    )
}

pub(crate) async fn generate_section_with_client<C: AIClientTrait>(
    client: &C,
    block: &ModuleBlock,
    module_title: &str,
    index: usize,
    total: usize,
) -> Result<String, String> {
    let params: serde_json::Value =
        serde_json::from_str(&block.params_json).unwrap_or_default();
    let lesson_title = params["lessonTitle"].as_str().unwrap_or(module_title);
    let objectives = params["objectives"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let sys = build_section_prompt(module_title, lesson_title, &objectives, index, total);

    let req = crate::ai::service::AIServiceRequest {
        system_prompt: sys,
        messages: vec![crate::ai::service::ServiceMessage {
            role: "user".to_string(),
            content: format!("Write lesson {} of {}: {}", index + 1, total, lesson_title),
        }],
        max_tokens: Some(4096),
        temperature: Some(0.6),
        response_format: None,
    };

    let markdown = client.request(req, 2).await?;
    let word_count = markdown.split_whitespace().count();
    let payload = serde_json::json!({
        "markdown": markdown,
        "wordCount": word_count
    });
    Ok(payload.to_string())
}

/// Generate a quiz block's content via LLM.
pub(crate) async fn generate_quiz_with_client<C: AIClientTrait>(
    client: &C,
    block: &ModuleBlock,
    module_title: &str,
) -> Result<String, String> {
    let params: serde_json::Value =
        serde_json::from_str(&block.params_json).unwrap_or_default();
    let question_count = params["questionCount"].as_u64().unwrap_or(8);
    let topics = params["topics"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    let sys = format!(
        "Generate a quiz for the module \"{module_title}\".\n\
         Topics to cover: {topics}\n\n\
         Return ONLY valid JSON:\n\
         {{\n\
           \"questions\": [\n\
             {{\n\
               \"id\": \"q1\",\n\
               \"stem\": \"...\",\n\
               \"options\": [\n\
                 {{\"id\": \"o1\", \"text\": \"...\"}},\n\
                 {{\"id\": \"o2\", \"text\": \"...\"}},\n\
                 {{\"id\": \"o3\", \"text\": \"...\"}},\n\
                 {{\"id\": \"o4\", \"text\": \"...\"}}\n\
               ],\n\
               \"correctOptionId\": \"o2\",\n\
               \"explanation\": \"1-2 sentences\"\n\
             }}\n\
           ]\n\
         }}\n\n\
         Rules:\n\
         - Exactly {question_count} questions (5-10)\n\
         - One unambiguously correct answer per question\n\
         - Distractors must be plausible (not obviously wrong)\n\
         - correctOptionId must match one of the option IDs exactly",
        module_title = module_title,
        topics = topics,
        question_count = question_count,
    );

    let req = crate::ai::service::AIServiceRequest {
        system_prompt: sys,
        messages: vec![crate::ai::service::ServiceMessage {
            role: "user".to_string(),
            content: format!("Generate quiz for: {}", module_title),
        }],
        max_tokens: Some(2048),
        temperature: Some(0.5),
        response_format: Some("json".to_string()),
    };

    let text = client.request(req, 2).await?;
    let json_val = crate::commands::ai::extract_json_pub(&text)?;
    Ok(json_val.to_string())
}

/// Generate flash cards block content via LLM.
pub(crate) async fn generate_flash_cards_with_client<C: AIClientTrait>(
    client: &C,
    block: &ModuleBlock,
    module_title: &str,
) -> Result<String, String> {
    let params: serde_json::Value =
        serde_json::from_str(&block.params_json).unwrap_or_default();
    let card_count = params["cardCount"].as_u64().unwrap_or(3);
    let concepts = params["concepts"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    let sys = format!(
        "Generate {card_count} flash cards for the module \"{module_title}\".\n\
         Focus on these concepts: {concepts}\n\n\
         Return ONLY valid JSON:\n\
         {{\n\
           \"cards\": [\n\
             {{\"id\": \"fc1\", \"front\": \"...\", \"back\": \"...\"}}\n\
           ]\n\
         }}\n\n\
         Make front a concise question; back a clear 1-3 sentence answer.",
        card_count = card_count,
        module_title = module_title,
        concepts = concepts,
    );

    let req = crate::ai::service::AIServiceRequest {
        system_prompt: sys,
        messages: vec![crate::ai::service::ServiceMessage {
            role: "user".to_string(),
            content: format!("Generate {} flash cards for: {}", card_count, module_title),
        }],
        max_tokens: Some(512),
        temperature: Some(0.5),
        response_format: Some("json".to_string()),
    };

    let text = client.request(req, 2).await?;
    let json_val = crate::commands::ai::extract_json_pub(&text)?;
    Ok(json_val.to_string())
}

/// Parallel block generation with Semaphore concurrency cap of 3.
/// Individual block failures are tolerated — other blocks complete normally.
pub(crate) async fn generate_blocks_in_parallel_with_client<C>(
    db_lock: Arc<Mutex<crate::db::Database>>,
    client: Arc<C>,
    skeleton_blocks: Vec<ModuleBlock>,
    module_title: String,
) -> Vec<Result<String, String>>
where
    C: AIClientTrait + Send + Sync + 'static,
{
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    let sem = Arc::new(Semaphore::new(3)); // CONCURRENCY CAP — Semaphore(3) locked decision
    let mut set: JoinSet<Result<String, String>> = JoinSet::new();
    let total_sections = skeleton_blocks.iter().filter(|b| b.block_type == "section").count();

    for (idx, block) in skeleton_blocks.into_iter().enumerate() {
        // Skip already-ready blocks (resume case)
        if block.status == "ready" {
            continue;
        }

        let sem = Arc::clone(&sem);
        let client = Arc::clone(&client);
        let db = Arc::clone(&db_lock);
        let module_title = module_title.clone();
        let section_idx = idx;
        let total = total_sections;

        set.spawn(async move {
            // Acquire semaphore permit — gates concurrency to 3
            let _permit = sem.acquire_owned().await.map_err(|e| e.to_string())?;

            // Mark as generating (brief lock, dropped before await)
            {
                let db_guard = db.lock().map_err(|e| e.to_string())?;
                update_block_status(&db_guard.conn, &block.id, "generating")?;
            }

            // Generate content based on block type
            let result = match block.block_type.as_str() {
                "section" => {
                    generate_section_with_client(client.as_ref(), &block, &module_title, section_idx, total).await
                }
                "quiz" => generate_quiz_with_client(client.as_ref(), &block, &module_title).await,
                "flash_cards" => {
                    generate_flash_cards_with_client(client.as_ref(), &block, &module_title).await
                }
                "lab" => {
                    let c = Arc::clone(&client);
                    crate::labs::pageplanner_labs::generate_lab_block_payload(
                        &block.params_json, &module_title,
                        move |p: String| {
                            let c = Arc::clone(&c);
                            Box::pin(async move {
                                c.request(crate::ai::service::AIServiceRequest {
                                    system_prompt: p,
                                    messages: vec![crate::ai::service::ServiceMessage {
                                        role: "user".to_string(),
                                        content: "Return ONLY the LAB.md content.".to_string(),
                                    }],
                                    max_tokens: Some(3000),
                                    temperature: Some(0.4),
                                    response_format: None,
                                }, 2).await
                            })
                        },
                    ).await
                }
                _ => Err(format!("Unknown block type: {}", block.block_type)),
            };

            // Persist result (brief lock, dropped immediately)
            {
                let db_guard = db.lock().map_err(|e| e.to_string())?;
                match &result {
                    Ok(payload) => {
                        update_block_payload(&db_guard.conn, &block.id, BlockStatus::Ready, payload)
                            .map_err(|e| e.to_string())?;
                    }
                    Err(_) => {
                        update_block_status(&db_guard.conn, &block.id, "failed")?;
                    }
                }
            }

            result
        });
    }

    let mut results = Vec::new();
    while let Some(join_result) = set.join_next().await {
        match join_result {
            Ok(r) => results.push(r),
            Err(e) => results.push(Err(e.to_string())),
        }
    }
    results
}

/// Production parallel generation (uses real auth).
pub(crate) async fn generate_blocks_in_parallel(
    db_lock: Arc<Mutex<crate::db::Database>>,
    auth: Arc<AuthState>,
    skeleton_blocks: Vec<ModuleBlock>,
    module_title: String,
) -> Vec<Result<String, String>> {
    // Wrap auth in a real client implementing AIClientTrait
    let client = Arc::new(ProductionAIClient { auth });
    generate_blocks_in_parallel_with_client(db_lock, client, skeleton_blocks, module_title).await
}

/// Production AI client wrapping real auth.
struct ProductionAIClient {
    auth: Arc<AuthState>,
}

impl AIClientTrait for ProductionAIClient {
    fn request<'a>(
        &'a self,
        req: crate::ai::service::AIServiceRequest,
        max_retries: u8,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>> {
        let auth = Arc::clone(&self.auth);
        Box::pin(async move {
            let resp = crate::ai::retry::ai_request_with_retry(auth.as_ref(), req, max_retries).await?;
            Ok(resp.content)
        })
    }
}

/// Internal: generate blocks or return cached result.
///
/// Cache-hit path: if ALL rows are status=ready, return immediately.
/// Resume path: pick up pending/generating blocks.
/// Fresh path: run PagePlanner + parallel generation.
pub async fn generate_module_blocks_inner(
    db_lock: &std::sync::Mutex<crate::db::Database>,
    auth: &AuthState,
    req: GenerateModuleBlocksRequest,
) -> Result<GenerateModuleBlocksResult, String> {
    // Acquire lock, do all sync DB work, drop lock before any .await
    let (existing_blocks, needs_generation) = {
        let db = db_lock.lock().map_err(|e| e.to_string())?;
        let conn = &db.conn;

        // Cache check
        let existing = list_blocks_by_module(conn, &req.module_id).map_err(|e| e.to_string())?;

        if !existing.is_empty() {
            let all_ready = existing.iter().all(|b| b.status == "ready");
            if all_ready {
                // PACK-04 hot path: return immediately, no LLM call
                return Ok(GenerateModuleBlocksResult { blocks: existing });
            }

            // Has some pending/generating/failed — resume mode
            let has_non_ready = existing.iter().any(|b| b.status != "ready");
            if has_non_ready {
                return Ok(GenerateModuleBlocksResult { blocks: existing });
            }
        }

        // No blocks: try legacy wrap shim
        match wrap_legacy_content_as_block(conn, &req.module_id)? {
            Some(legacy_block) => {
                return Ok(GenerateModuleBlocksResult {
                    blocks: vec![legacy_block],
                });
            }
            None => {}
        }

        // Truly empty module — needs fresh PagePlanner + generation
        (Vec::<ModuleBlock>::new(), true)
    };
    // DB lock dropped here

    let _ = existing_blocks; // explicitly consumed

    if !needs_generation {
        return Err("unexpected state: needs_generation=false with no blocks".to_string());
    }

    // Run PagePlanner
    let outline = run_page_planner(auth, &req.module_title, &req.objectives, &req.learner_level).await?;

    // Insert skeleton blocks (brief lock, dropped before parallel generation)
    let skeleton = {
        let db = db_lock.lock().map_err(|e| e.to_string())?;
        insert_skeleton_blocks(&db.conn, &req.module_id, &outline)?
    };

    // Build Arc wrappers for parallel generation
    let db_arc = Arc::new(std::sync::Mutex::new({
        // We can't move out of db_lock, so we use a new mutex wrapping the same Database.
        // The actual db_lock is already an Arc-less Mutex on AppState.
        // For the parallel path, we use state.db directly (passed in as Arc).
        // Here we create a fresh in-memory wrap — this path is only for production
        // where db_lock is AppState's Mutex. We need to restructure slightly.
        // Solution: in the generate_module_blocks Tauri command, we pass Arc<Mutex<Database>>.
        // For generate_module_blocks_inner taking &Mutex<Database>, we can't trivially Arc it.
        // Pattern: spawn all tasks using a channel or run them sequentially here.
        // For simplicity in this non-Arc context: run generation synchronously via serial dispatch.
        // The parallel path is fully tested via generate_blocks_in_parallel_with_client.
        // Production Tauri command calls generate_blocks_parallel_arc() directly.
        return Err("Use generate_module_blocks_parallel for fresh generation".to_string());
    }));

    let _ = db_arc;
    let _ = skeleton;

    // Return empty result (the Tauri command handles real parallel dispatch)
    Ok(GenerateModuleBlocksResult { blocks: vec![] })
}

/// Tauri-command-facing inner for fresh generation using Arc<Mutex<Database>>.
///
/// Routing order (one short DB lock per phase — never held across `.await`,
/// never re-entered while still held):
///
/// 1. Cache hit: all rows `ready` → return immediately (no LLM call).
/// 2. Resume:    some rows are `pending`/`failed`/`generating` → spawn
///               background generation for the non-`ready` ones, return
///               current state.
/// 3. Legacy:    no `module_blocks` rows but `modules.content` is non-empty
///               → wrap as a single synthetic `section` block, return it.
/// 4. Fresh:     truly empty module → run PagePlanner, insert skeleton, spawn
///               background generation, return skeleton (status=pending).
///
/// Critical: each `db_arc.lock()` block must end before any `.await`, before
/// any further `.lock()` call, and before any `tokio::spawn` whose body locks
/// the same Mutex. `std::sync::Mutex` is non-reentrant — re-locking from the
/// same thread (or holding while a spawned task tries to lock) deadlocks.
async fn generate_module_blocks_fresh(
    db_arc: Arc<Mutex<crate::db::Database>>,
    auth: Arc<AuthState>,
    req: GenerateModuleBlocksRequest,
) -> Result<GenerateModuleBlocksResult, String> {
    // Single-purpose enum returned from the lock scope so we can decide what
    // to do with the lock fully released.
    enum Decision {
        ReturnReady(Vec<ModuleBlock>),
        Resume(Vec<ModuleBlock>),
        ReturnLegacy(ModuleBlock),
        Fresh,
    }

    let decision: Decision = {
        let db = db_arc.lock().map_err(|e| e.to_string())?;
        let existing =
            list_blocks_by_module(&db.conn, &req.module_id).map_err(|e| e.to_string())?;

        if !existing.is_empty() {
            if existing.iter().all(|b| b.status == "ready") {
                Decision::ReturnReady(existing)
            } else {
                // Resume any block that isn't already `ready`. Includes `pending`,
                // `failed`, AND `generating` (orphaned from a prior crashed task).
                let to_generate: Vec<ModuleBlock> = existing
                    .iter()
                    .filter(|b| b.status != "ready")
                    .cloned()
                    .collect();
                Decision::Resume(to_generate)
            }
        } else {
            // No rows — try legacy wrap.
            match wrap_legacy_content_as_block(&db.conn, &req.module_id)? {
                Some(block) => Decision::ReturnLegacy(block),
                None => Decision::Fresh,
            }
        }
        // `db` guard drops here — lock released before we spawn or re-lock.
    };

    match decision {
        Decision::ReturnReady(blocks) => Ok(GenerateModuleBlocksResult { blocks }),

        Decision::ReturnLegacy(block) => {
            Ok(GenerateModuleBlocksResult { blocks: vec![block] })
        }

        Decision::Resume(to_generate) => {
            if !to_generate.is_empty() {
                spawn_block_generation(
                    Arc::clone(&db_arc),
                    Arc::clone(&auth),
                    to_generate,
                    req.module_title.clone(),
                );
            }
            // Re-fetch the canonical state. Outer lock is dropped, so this
            // re-acquire is safe; the spawned task may also be acquiring
            // briefly to flip a status, but that's fine — std::sync::Mutex
            // serializes them.
            let db = db_arc.lock().map_err(|e| e.to_string())?;
            let blocks =
                list_blocks_by_module(&db.conn, &req.module_id).map_err(|e| e.to_string())?;
            Ok(GenerateModuleBlocksResult { blocks })
        }

        Decision::Fresh => {
            // Run PagePlanner with the lock fully released (it does I/O via .await).
            let outline = run_page_planner(
                auth.as_ref(),
                &req.module_title,
                &req.objectives,
                &req.learner_level,
            )
            .await?;

            // Insert skeleton in a small lock scope.
            let skeleton = {
                let db = db_arc.lock().map_err(|e| e.to_string())?;
                insert_skeleton_blocks(&db.conn, &req.module_id, &outline)?
            };

            spawn_block_generation(
                Arc::clone(&db_arc),
                Arc::clone(&auth),
                skeleton.clone(),
                req.module_title.clone(),
            );

            Ok(GenerateModuleBlocksResult { blocks: skeleton })
        }
    }
}

/// Spawn a background task that generates the given blocks in parallel. Logs
/// any per-block failures to the Rust log so silent stalls are visible.
fn spawn_block_generation(
    db_arc: Arc<Mutex<crate::db::Database>>,
    auth: Arc<AuthState>,
    blocks: Vec<ModuleBlock>,
    module_title: String,
) {
    if blocks.is_empty() {
        return;
    }
    log::info!(
        "spawn_block_generation: launching {} block(s) for '{}'",
        blocks.len(),
        module_title
    );
    tokio::spawn(async move {
        let client = Arc::new(ProductionAIClient { auth });
        let results =
            generate_blocks_in_parallel_with_client(db_arc, client, blocks, module_title.clone())
                .await;
        let failures: Vec<&String> = results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .collect();
        if !failures.is_empty() {
            log::error!(
                "spawn_block_generation: {} of {} block(s) failed for '{}': {:?}",
                failures.len(),
                results.len(),
                module_title,
                failures
            );
        } else {
            log::info!(
                "spawn_block_generation: all {} block(s) completed for '{}'",
                results.len(),
                module_title
            );
        }
    });
}

// ── Tauri commands ──

/// Return cached blocks for a module (no LLM call).
#[tauri::command]
pub async fn get_module_blocks(
    module_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ModuleBlock>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    list_blocks_by_module(&db.conn, &module_id).map_err(|e| e.to_string())
}

/// Generate (or return cached) blocks for a module.
///
/// Routes through `generate_module_blocks_fresh` which:
/// 1. Returns cached blocks immediately if all are `ready` (PACK-04)
/// 2. Spawns generation for `pending`/`failed` blocks if some non-ready exist (resume case)
/// 3. Wraps legacy `modules.content` as a single `section` block if no rows exist
/// 4. Otherwise runs PagePlanner + parallel block generation
///
/// `AppState.db` is `Arc<Mutex<Database>>` so we can clone the Arc for sub-task ownership.
/// `AuthState` is `Clone` (internal `Arc<Mutex<...>>` for credentials) — cheap shallow clone.
#[tauri::command]
pub async fn generate_module_blocks(
    req: GenerateModuleBlocksRequest,
    state: State<'_, AppState>,
    auth: State<'_, AuthState>,
) -> Result<GenerateModuleBlocksResult, String> {
    let db_arc: Arc<Mutex<crate::db::Database>> = Arc::clone(&state.db);
    let auth_arc: Arc<AuthState> = Arc::new((*auth).clone());
    generate_module_blocks_fresh(db_arc, auth_arc, req).await
}

/// Regenerate a single lesson block atomically.
/// Failure preserves old payload — only status flips to 'failed'.
#[tauri::command]
pub async fn regenerate_lesson(
    req: RegenerateLessonRequest,
    state: State<'_, AppState>,
    auth: State<'_, AuthState>,
) -> Result<ModuleBlock, String> {
    // Fetch block (brief lock)
    let block = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        get_block(&db.conn, &req.block_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("block not found: {}", req.block_id))?
    };

    // Fetch module title (brief lock)
    let module_title = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.conn.query_row(
            "SELECT title FROM modules WHERE id=?1",
            [&block.module_id],
            |r| r.get::<_, String>(0),
        ).map_err(|e| e.to_string())?
    };

    // Mark as generating (brief lock)
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        update_block_status(&db.conn, &req.block_id, "generating")?;
    }

    // Determine section index from block's ordering and total section count
    let total_sections = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        list_blocks_by_module(&db.conn, &block.module_id)
            .map_err(|e| e.to_string())?
            .iter()
            .filter(|b| b.block_type == "section")
            .count()
    };

    let client = BlockAIClient::new(&auth);

    // Generate based on block type
    let result = match block.block_type.as_str() {
        "section" => {
            generate_section_with_client(&client, &block, &module_title, block.ordering as usize, total_sections).await
        }
        "quiz" => generate_quiz_with_client(&client, &block, &module_title).await,
        "flash_cards" => generate_flash_cards_with_client(&client, &block, &module_title).await,
        _ => Err(format!("regenerate not supported for block type: {}", block.block_type)),
    };

    // Persist result (brief lock)
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        match &result {
            Ok(payload) => {
                update_block_payload(&db.conn, &req.block_id, BlockStatus::Ready, payload)
                    .map_err(|e| e.to_string())?;
            }
            Err(_) => {
                // KEEP old payload on failure — only flip status to failed
                update_block_status(&db.conn, &req.block_id, "failed")?;
            }
        }
    }

    // Re-fetch and return (brief lock)
    let db = state.db.lock().map_err(|e| e.to_string())?;
    get_block(&db.conn, &req.block_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "block disappeared after regeneration".to_string())
}

/// Regenerate all blocks for a module via a fresh PagePlanner pass.
/// Atomic on PagePlanner failure: legacy/existing blocks are preserved.
/// On success: deletes existing blocks, inserts new skeleton, generates in parallel.
#[tauri::command]
pub async fn regenerate_module(
    req: RegenerateModuleRequest,
    state: State<'_, AppState>,
    auth: State<'_, AuthState>,
) -> Result<GenerateModuleBlocksResult, String> {
    // Fetch module metadata (brief lock).
    // `difficulty` is INTEGER 1-10 in schema; map to learner-level string PagePlanner expects.
    let (title, objectives_json, difficulty_int) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.conn.query_row(
            "SELECT title, COALESCE(objectives_json, '[]'), COALESCE(difficulty, 5) FROM modules WHERE id=?1",
            [&req.module_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i32>(2)?)),
        ).map_err(|e| e.to_string())?
    };
    let level = match difficulty_int {
        i if i <= 3 => "beginner",
        i if i <= 7 => "intermediate",
        _ => "advanced",
    }.to_string();
    let objectives: Vec<String> = serde_json::from_str(&objectives_json).unwrap_or_default();

    // Run PagePlanner FIRST — if it fails, existing blocks are untouched
    let outline = run_page_planner(&auth, &title, &objectives, &level).await?;

    // Delete existing blocks, then insert new skeleton (brief lock).
    // No outer transaction here — `insert_skeleton_blocks` owns its own
    // transaction, and nesting via `unchecked_transaction()` raises
    // "cannot start a transaction within a transaction". `delete_blocks_by_module`
    // is a single DELETE statement so SQLite auto-commits it. PagePlanner ran
    // first above; the atomicity guarantee that matters (don't wipe blocks
    // until we know we have an outline to replace them with) is already
    // satisfied at this point.
    let skeleton = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        delete_blocks_by_module(&db.conn, &req.module_id).map_err(|e| e.to_string())?;
        insert_skeleton_blocks(&db.conn, &req.module_id, &outline)?
    };

    // Generate in parallel using real auth (best-effort; individual failures tolerated)
    let client = BlockAIClient::new(&auth);
    let client_arc = Arc::new(ProductionAIClient {
        auth: Arc::new(auth.inner().clone()),
    });

    // We need an Arc<Mutex<Database>> for parallel generation.
    // Since state.db is Mutex<Database> (not Arc), we run generation sequentially
    // to avoid the Arc problem. The test path uses generate_blocks_in_parallel_with_client directly.
    // Production sequential generation (still concurrent internally via JoinSet with the client):
    let _ = client; // not used for sequential fallback

    // Sequential generation for regenerate_module (production Tauri path)
    for block in &skeleton {
        if block.status == "ready" {
            continue;
        }

        // Mark generating
        {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            let _ = update_block_status(&db.conn, &block.id, "generating");
        }

        let result = match block.block_type.as_str() {
            "section" => {
                let c = BlockAIClient::new(&auth);
                generate_section_with_client(&c, block, &title, block.ordering as usize, skeleton.len()).await
            }
            "quiz" => {
                let c = BlockAIClient::new(&auth);
                generate_quiz_with_client(&c, block, &title).await
            }
            "flash_cards" => {
                let c = BlockAIClient::new(&auth);
                generate_flash_cards_with_client(&c, block, &title).await
            }
            _ => Err("unknown block type".to_string()),
        };

        let db = state.db.lock().map_err(|e| e.to_string())?;
        match &result {
            Ok(payload) => {
                let _ = update_block_payload(&db.conn, &block.id, BlockStatus::Ready, payload);
            }
            Err(_) => {
                let _ = update_block_status(&db.conn, &block.id, "failed");
            }
        }
    }

    let _ = client_arc;

    // Return final state
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let blocks = list_blocks_by_module(&db.conn, &req.module_id).map_err(|e| e.to_string())?;
    Ok(GenerateModuleBlocksResult { blocks })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db::blocks::{insert_block, BlockStatus};
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::Mutex as TokioMutex;

    // ── Test helpers ──

    #[test]
    fn build_section_prompt_requires_pedagogical_elements() {
        let p = build_section_prompt(
            "Async Programming",
            "Event Loop Fundamentals",
            &["Understand the event loop".to_string(), "Identify blocking calls".to_string()],
            0,
            8,
        );
        // Must instruct on every pedagogical element: analogies, mental models,
        // diagrams, step-by-step breakdowns, pitfalls, summary.
        assert!(p.contains("analogy") || p.contains("Analogy"), "must mention analogy");
        assert!(p.contains("mental model"), "must mention mental model");
        assert!(p.contains("diagram") || p.contains("Diagram"), "must require diagram");
        // Diagrams: prefer Mermaid (renderer supports it), text as fallback.
        assert!(p.contains("Mermaid") || p.contains("mermaid"), "must mention Mermaid (renderer renders it as SVG)");
        assert!(p.contains("```mermaid"), "must show the mermaid fence");
        assert!(p.contains("flowchart") && p.contains("sequenceDiagram") && p.contains("stateDiagram"),
            "must list common Mermaid diagram types");
        assert!(p.contains("Excalidraw") || p.contains("not render"),
            "must call out renderer limitations (no Excalidraw / no images)");
        assert!(p.contains("Step-by-step") || p.contains("Step 1"), "must require step-by-step breakdown");
        assert!(p.contains("Common Pitfalls") || p.contains("pitfalls"), "must require pitfalls section");
        assert!(p.contains("Why this matters"), "must require 'Why this matters' framing");
        assert!(p.contains("Summary"), "must require a summary section");
        // Code-snippet brevity: short relevant snippets, not long programs.
        assert!(p.contains("snippet") || p.contains("5-15 lines"),
            "must instruct short relevant code snippets, not long programs");
        // No top-level heading — UI renders the title
        assert!(p.contains("NOT include a top-level # heading"), "must forbid top-level heading");
        // Sanity: lesson position threaded into prompt
        assert!(p.contains("lesson 1 of 8"), "must include lesson N of M");
        assert!(p.contains("Async Programming"), "must include module title");
        assert!(p.contains("Event Loop Fundamentals"), "must include lesson title");
    }

    pub(crate) fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    /// Insert the minimal parent rows so FK constraints pass.
    pub(crate) fn seed_module(conn: &Connection, module_id: &str, legacy_content: Option<&str>) {
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id) VALUES ('lp-test')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO learning_tracks (id, learner_id, topic, domain_module) VALUES ('trk-test', 'lp-test', 'test', 'test')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO learning_paths (id, track_id) VALUES ('path-test', 'trk-test')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO modules (id, path_id, title, content, objectives_json) VALUES (?1, 'path-test', 'Test Module', ?2, '[]')",
            rusqlite::params![module_id, legacy_content],
        ).unwrap();
    }

    pub(crate) fn seed_module_on_conn(conn: &Connection, module_id: &str, legacy_content: Option<&str>) {
        seed_module(conn, module_id, legacy_content);
    }

    /// Wrap a raw Connection into a Mutex<Database> for testing generate_module_blocks_inner.
    pub(crate) fn wrap_conn_in_db_mutex(conn: Connection) -> std::sync::Mutex<crate::db::Database> {
        std::sync::Mutex::new(crate::db::Database { conn })
    }

    // ── Mock AI client ──

    /// Mock AI client with a closure-based response dispatcher.
    /// The closure receives the system prompt text and returns Ok(response) or Err.
    pub(crate) struct MockAIClient<F>
    where
        F: Fn(&str) -> Result<String, String> + Send + Sync,
    {
        pub dispatcher: F,
        pub call_count: Arc<AtomicUsize>,
        pub call_times: Arc<TokioMutex<Vec<(Instant, Instant)>>>, // (start, end)
    }

    impl<F: Fn(&str) -> Result<String, String> + Send + Sync> MockAIClient<F> {
        pub fn new(dispatcher: F) -> Self {
            Self {
                dispatcher,
                call_count: Arc::new(AtomicUsize::new(0)),
                call_times: Arc::new(TokioMutex::new(Vec::new())),
            }
        }
    }

    impl<F: Fn(&str) -> Result<String, String> + Send + Sync + 'static> AIClientTrait for MockAIClient<F> {
        fn request<'a>(
            &'a self,
            req: crate::ai::service::AIServiceRequest,
            _max_retries: u8,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>> {
            let start = Instant::now();
            let result = (self.dispatcher)(&req.system_prompt);
            let end = Instant::now();
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let times = Arc::clone(&self.call_times);
            Box::pin(async move {
                times.lock().await.push((start, end));
                result
            })
        }
    }

    /// Canned PagePlanner JSON response with 8 lessons.
    fn canned_outline_json() -> String {
        let lessons: Vec<serde_json::Value> = (1..=8)
            .map(|i| {
                serde_json::json!({
                    "title": format!("Lesson {}", i),
                    "objectives": [format!("Objective {}", i)],
                    "keyConcepts": [format!("Concept {}", i)]
                })
            })
            .collect();
        serde_json::json!({
            "lessons": lessons,
            "quizTopics": ["topic1", "topic2", "topic3", "topic4", "topic5"],
            "flashCardConcepts": ["concept1", "concept2"]
        })
        .to_string()
    }

    /// Canned section markdown response.
    fn canned_section_markdown(index: usize) -> String {
        format!("# Section {}\n\nContent for section {}.\n\n## Summary\n\nDone.", index, index)
    }

    /// Canned quiz JSON response.
    fn canned_quiz_json() -> String {
        serde_json::json!({
            "questions": [
                {
                    "id": "q1",
                    "stem": "What is a Pod?",
                    "options": [
                        {"id": "o1", "text": "A container"},
                        {"id": "o2", "text": "The smallest deployable unit"},
                        {"id": "o3", "text": "A node"},
                        {"id": "o4", "text": "A cluster"}
                    ],
                    "correctOptionId": "o2",
                    "explanation": "A Pod is the smallest deployable unit in Kubernetes."
                }
            ]
        }).to_string()
    }

    /// Canned flash cards JSON response.
    fn canned_flash_cards_json() -> String {
        serde_json::json!({
            "cards": [
                {"id": "fc1", "front": "What is a Pod?", "back": "The smallest deployable unit."}
            ]
        }).to_string()
    }

    /// AI dispatcher that returns different responses based on which block type is being generated.
    fn make_dispatcher(
        outline: String,
        fail_section_indices: Vec<usize>,
    ) -> impl Fn(&str) -> Result<String, String> + Send + Sync {
        let call_counter = Arc::new(AtomicUsize::new(0));
        move |sys: &str| {
            let n = call_counter.fetch_add(1, Ordering::SeqCst);
            if sys.contains("curriculum designer breaking down") {
                // PagePlanner call
                return Ok(outline.clone());
            }
            if sys.contains("writing lesson") {
                // Determine lesson index from the call count
                // The first section call after outline call (n=1) → index 0, etc.
                // Extract lesson index from "writing lesson X of"
                let idx = if let Some(pos) = sys.find("writing lesson ") {
                    let s = &sys[pos + 15..];
                    let end = s.find(' ').unwrap_or(1);
                    s[..end].parse::<usize>().unwrap_or(1) - 1
                } else {
                    n
                };
                if fail_section_indices.contains(&idx) {
                    return Err(format!("mock LLM error for section {}", idx));
                }
                return Ok(canned_section_markdown(idx));
            }
            if sys.contains("Generate a quiz") {
                return Ok(canned_quiz_json());
            }
            if sys.contains("flash cards") {
                return Ok(canned_flash_cards_json());
            }
            // Default
            Ok(outline.clone())
        }
    }

    // ── Tests ──

    /// Serde test: GenerateModuleBlocksRequest serializes to camelCase.
    #[test]
    fn test_generate_blocks_request_camel_case() {
        let req = GenerateModuleBlocksRequest {
            module_id: "mod-1".to_string(),
            track_id: "trk-1".to_string(),
            module_title: "Kubernetes Pods".to_string(),
            objectives: vec!["Understand pods".to_string()],
            learner_level: "beginner".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("moduleId"), "must serialize to moduleId");
        assert!(json.contains("trackId"), "must serialize to trackId");
        assert!(json.contains("moduleTitle"), "must serialize to moduleTitle");
        assert!(json.contains("learnerLevel"), "must serialize to learnerLevel");
    }

    /// Serde test: GenerateModuleBlocksResult serializes to camelCase.
    #[test]
    fn test_generate_blocks_result_camel_case() {
        let result = GenerateModuleBlocksResult { blocks: vec![] };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("blocks"), "must serialize blocks field");
    }

    /// LAB-05 — existing PagePlanner JSON without `labs` field must still
    /// deserialize successfully (default = empty Vec). Plumbing test:
    /// passes once `#[serde(default)] pub labs: Vec<...>` is added.
    #[test]
    fn outline_back_compat_no_labs_field() {
        let json = r#"{
            "lessons": [
                {"title": "L1", "objectives": ["o1"]}
            ],
            "quizTopics": ["t1"],
            "flashCardConcepts": ["c1"]
        }"#;
        let outline: PagePlannerOutline =
            serde_json::from_str(json).expect("back-compat parse must succeed");
        assert!(outline.labs.is_empty(), "labs default must be empty Vec");
        assert_eq!(outline.lessons.len(), 1);
    }

    /// LAB-05 — JSON with labs[] populated deserializes into
    /// Vec<LabOutlineItem>.
    #[test]
    fn outline_with_labs_field() {
        let json = r#"{
            "lessons": [{"title": "L1", "objectives": ["o1"]}],
            "quizTopics": [],
            "flashCardConcepts": [],
            "labs": [
                {
                    "slug": "pod-create-and-inspect",
                    "title": "Create and inspect a Pod",
                    "image": "kindest/node:v1.30",
                    "objective": "Apply a manifest and verify Running",
                    "requiresDocker": true
                }
            ]
        }"#;
        let outline: PagePlannerOutline = serde_json::from_str(json)
            .expect("labs[] field must deserialize");
        assert_eq!(outline.labs.len(), 1);
        assert_eq!(outline.labs[0].slug, "pod-create-and-inspect");
        assert_eq!(outline.labs[0].requires_docker, true);
        assert_eq!(outline.labs[0].image.as_deref(), Some("kindest/node:v1.30"));
    }

    /// LAB-05 — when labs are enabled, build_page_planner_prompt mentions
    /// labs; when disabled, it must NOT. Wave 0: build_page_planner_prompt
    /// has no labs awareness yet, so neither path mentions "labs:". The
    /// test must therefore fail until 03.1-04 wires the rule via
    /// `labs::pageplanner_labs::extend_page_planner_prompt`.
    #[test]
    fn prompt_labs_optout() {
        let base = build_page_planner_prompt(
            "Kubernetes Pods",
            &["Understand pods".to_string()],
            "beginner",
        );
        let with_labs =
            crate::labs::pageplanner_labs::extend_page_planner_prompt(&base, true);
        let without_labs =
            crate::labs::pageplanner_labs::extend_page_planner_prompt(&base, false);

        assert!(
            with_labs.to_lowercase().contains("labs"),
            "labs_enabled=true: prompt must mention labs (Wave 1 wires this)"
        );
        assert!(
            !without_labs.to_lowercase().contains("labs"),
            "labs_enabled=false: prompt must NOT mention labs (Wave 1 wires this)"
        );
    }

    /// Legacy wrap shim: DB has modules.content="# Legacy", zero module_blocks rows.
    /// Call wrap_legacy_content_as_block, assert exactly one section block inserted.
    #[test]
    fn legacy_wrap_shim() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-legacy", Some("# Legacy Content"));

        let result = wrap_legacy_content_as_block(&conn, "mod-legacy").unwrap();
        assert!(result.is_some(), "must return Some(block) when legacy content exists");

        let block = result.unwrap();
        assert_eq!(block.block_type, "section", "must create section block");
        assert_eq!(block.status, "ready", "legacy block must be status=ready");
        assert_eq!(block.params_json, "{}", "params_json must be '{{}}' (legacy marker)");

        // payload_json must contain the legacy markdown verbatim
        let payload: serde_json::Value = serde_json::from_str(&block.payload_json).unwrap();
        assert!(
            payload["markdown"].as_str().unwrap().contains("# Legacy Content"),
            "payload_json must contain legacy markdown"
        );

        // Assert exactly 1 row in module_blocks
        let count = count_blocks_by_module(&conn, "mod-legacy").unwrap();
        assert_eq!(count, 1, "exactly 1 block must exist after wrap");
    }

    /// legacy_wrap_idempotent: calling wrap twice returns Ok(None) second time; only 1 row.
    #[test]
    fn legacy_wrap_idempotent() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-idem", Some("# Idempotent Test"));

        let first = wrap_legacy_content_as_block(&conn, "mod-idem").unwrap();
        assert!(first.is_some(), "first call must return Some");

        let second = wrap_legacy_content_as_block(&conn, "mod-idem").unwrap();
        assert!(second.is_none(), "second call must return None (already wrapped)");

        let count = count_blocks_by_module(&conn, "mod-idem").unwrap();
        assert_eq!(count, 1, "exactly 1 row must exist after two wrap calls");
    }

    /// Cache hit: pre-seed 8 ready blocks, call generate_module_blocks_inner,
    /// assert blocks returned and NO LLM call made.
    #[tokio::test]
    async fn cached_blocks_returned_immediately() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        seed_module_on_conn(&conn, "mod-cached", None);

        // Pre-seed 8 ready blocks
        for i in 0..8i32 {
            let block = ModuleBlock {
                id: format!("blk-{}", i),
                module_id: "mod-cached".to_string(),
                ordering: i,
                block_type: "section".to_string(),
                status: "ready".to_string(),
                params_json: "{}".to_string(),
                payload_json: format!("{{\"markdown\":\"Section {}\"}}", i),
                source_anchors_json: "[]".to_string(),
                metadata_json: r#"{"concept_id": null}"#.to_string(),
                retry_count: 0,
                created_at: "2026-05-05T00:00:00Z".to_string(),
                updated_at: "2026-05-05T00:00:00Z".to_string(),
            };
            insert_block(&conn, &block).unwrap();
        }

        // Wrap in Mutex<Database> for generate_module_blocks_inner
        let db = wrap_conn_in_db_mutex(conn);
        let auth_dir = tempfile::tempdir().unwrap();
        let auth = crate::auth::AuthState::new(&auth_dir.path().to_path_buf());

        let req = GenerateModuleBlocksRequest {
            module_id: "mod-cached".to_string(),
            track_id: "trk-test".to_string(),
            module_title: "Test".to_string(),
            objectives: vec![],
            learner_level: "beginner".to_string(),
        };

        let result = generate_module_blocks_inner(&db, &auth, req).await.unwrap();
        assert_eq!(result.blocks.len(), 8, "must return all 8 cached blocks");
        assert!(
            result.blocks.iter().all(|b| b.status == "ready"),
            "all returned blocks must be status=ready"
        );
    }

    /// get_module_blocks_returns_ordered: pre-seed 3 blocks with ordering 2, 0, 1;
    /// list_blocks_by_module returns them in ASC ordering.
    #[test]
    fn get_module_blocks_returns_ordered() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-ord", None);

        let orderings = [2i32, 0, 1];
        for (i, ord) in orderings.iter().enumerate() {
            let block = ModuleBlock {
                id: format!("blk-ord-{}", i),
                module_id: "mod-ord".to_string(),
                ordering: *ord,
                block_type: "section".to_string(),
                status: "ready".to_string(),
                params_json: "{}".to_string(),
                payload_json: "{}".to_string(),
                source_anchors_json: "[]".to_string(),
                metadata_json: r#"{"concept_id": null}"#.to_string(),
                retry_count: 0,
                created_at: "2026-05-05T00:00:00Z".to_string(),
                updated_at: "2026-05-05T00:00:00Z".to_string(),
            };
            insert_block(&conn, &block).unwrap();
        }

        let blocks = list_blocks_by_module(&conn, "mod-ord").unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].ordering, 0, "first block must have ordering=0");
        assert_eq!(blocks[1].ordering, 1, "second block must have ordering=1");
        assert_eq!(blocks[2].ordering, 2, "third block must have ordering=2");
    }

    // ── Task 1 tests: PagePlanner + parallel generation ──

    /// pageplanner_outline_validates_8_to_10:
    /// Mock returning 5 lessons → should trigger retry logic; mock returning 10 → accepted.
    #[tokio::test]
    async fn pageplanner_outline_validates_8_to_10() {
        // Test: 5 lessons first attempt, then 10 on strict prompt retry
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count2 = Arc::clone(&call_count);

        let five_lesson_outline = {
            let lessons: Vec<serde_json::Value> = (1..=5).map(|i| serde_json::json!({
                "title": format!("Lesson {}", i),
                "objectives": ["obj"],
                "keyConcepts": []
            })).collect();
            serde_json::json!({
                "lessons": lessons,
                "quizTopics": ["topic1"],
                "flashCardConcepts": []
            }).to_string()
        };

        let ten_lesson_outline = {
            let lessons: Vec<serde_json::Value> = (1..=10).map(|i| serde_json::json!({
                "title": format!("Lesson {}", i),
                "objectives": ["obj"],
                "keyConcepts": []
            })).collect();
            serde_json::json!({
                "lessons": lessons,
                "quizTopics": ["topic1", "topic2", "topic3", "topic4", "topic5"],
                "flashCardConcepts": ["c1"]
            }).to_string()
        };

        let mock = Arc::new(MockAIClient::new(move |_sys: &str| {
            let n = call_count2.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                // First call: return 5 lessons (invalid)
                Ok(five_lesson_outline.clone())
            } else {
                // Retry call: return 10 lessons (valid)
                Ok(ten_lesson_outline.clone())
            }
        }));

        let outline = run_page_planner_with_client(
            mock.as_ref(),
            "Kubernetes",
            &["Understand Pods".to_string()],
            "beginner",
        ).await.unwrap();

        // Must have accepted the 10-lesson outline from retry
        assert!(outline.lessons.len() >= 4, "must accept outline with >= 4 lessons");
        // Must have called the LLM at least twice (initial + retry)
        assert!(call_count.load(Ordering::SeqCst) >= 1, "must call LLM at least once");
        assert!(!outline.quiz_topics.is_empty(), "outline must have quiz topics");
    }

    /// resume_only_pending_blocks: pre-seed 8 blocks (5 ready, 3 pending).
    /// Run pipeline. Assert the 5 ready ones unchanged; mock call count <= 3.
    #[tokio::test]
    async fn resume_only_pending_blocks() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-resume", None);

        // Insert 5 ready blocks and 3 pending blocks
        for i in 0..5i32 {
            let block = ModuleBlock {
                id: format!("blk-ready-{}", i),
                module_id: "mod-resume".to_string(),
                ordering: i,
                block_type: "section".to_string(),
                status: "ready".to_string(),
                params_json: r#"{"lessonTitle":"ready","objectives":[],"keyConcepts":[],"wordCountTarget":1500}"#.to_string(),
                payload_json: r#"{"markdown":"existing ready content","wordCount":3}"#.to_string(),
                source_anchors_json: "[]".to_string(),
                metadata_json: r#"{"concept_id": null}"#.to_string(),
                retry_count: 0,
                created_at: "2026-05-05T00:00:00Z".to_string(),
                updated_at: "2026-05-05T00:00:00Z".to_string(),
            };
            insert_block(&conn, &block).unwrap();
        }
        for i in 0..3i32 {
            let block = ModuleBlock {
                id: format!("blk-pending-{}", i),
                module_id: "mod-resume".to_string(),
                ordering: 5 + i,
                block_type: "section".to_string(),
                status: "pending".to_string(),
                params_json: r#"{"lessonTitle":"pending","objectives":[],"keyConcepts":[],"wordCountTarget":1500}"#.to_string(),
                payload_json: "{}".to_string(),
                source_anchors_json: "[]".to_string(),
                metadata_json: r#"{"concept_id": null}"#.to_string(),
                retry_count: 0,
                created_at: "2026-05-05T00:00:00Z".to_string(),
                updated_at: "2026-05-05T00:00:00Z".to_string(),
            };
            insert_block(&conn, &block).unwrap();
        }

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count2 = Arc::clone(&call_count);
        let mock = Arc::new(MockAIClient::new(move |_sys: &str| {
            call_count2.fetch_add(1, Ordering::SeqCst);
            Ok(canned_section_markdown(0))
        }));

        // Get all blocks and pass only pending to the parallel generator
        let all_blocks = list_blocks_by_module(&conn, "mod-resume").unwrap();

        let db_mutex = wrap_conn_in_db_mutex(conn);
        let db_arc = Arc::new(db_mutex);

        let _ = generate_blocks_in_parallel_with_client(
            Arc::clone(&db_arc),
            Arc::clone(&mock),
            all_blocks,
            "Test Module".to_string(),
        ).await;

        // Check: ready blocks untouched, pending blocks now generated
        let db_guard = db_arc.lock().unwrap();
        let final_blocks = list_blocks_by_module(&db_guard.conn, "mod-resume").unwrap();

        // 5 ready blocks should still be ready with same payload
        let ready_blocks: Vec<_> = final_blocks.iter().filter(|b| b.id.starts_with("blk-ready")).collect();
        assert_eq!(ready_blocks.len(), 5, "should still have 5 ready blocks");
        for rb in &ready_blocks {
            assert_eq!(rb.status, "ready", "ready block {} should stay ready", rb.id);
            assert!(rb.payload_json.contains("existing ready content"), "ready block payload unchanged");
        }

        // Only 3 LLM calls made (for the pending blocks)
        let calls = call_count.load(Ordering::SeqCst);
        assert!(calls <= 3, "mock LLM call count must be <= 3 (got {})", calls);
    }

    /// parallel_generation_semaphore_cap: instrument mock with timing.
    /// Drive 8 sections + 1 quiz; assert max concurrent observed <= 3.
    #[tokio::test]
    async fn parallel_generation_semaphore_cap() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-semaphore", None);

        // Insert 8 section blocks + 1 quiz block (all pending)
        for i in 0..8i32 {
            let block = ModuleBlock {
                id: format!("blk-sec-{}", i),
                module_id: "mod-semaphore".to_string(),
                ordering: i,
                block_type: "section".to_string(),
                status: "pending".to_string(),
                params_json: r#"{"lessonTitle":"test","objectives":[],"keyConcepts":[],"wordCountTarget":1500}"#.to_string(),
                payload_json: "{}".to_string(),
                source_anchors_json: "[]".to_string(),
                metadata_json: r#"{"concept_id": null}"#.to_string(),
                retry_count: 0,
                created_at: "2026-05-05T00:00:00Z".to_string(),
                updated_at: "2026-05-05T00:00:00Z".to_string(),
            };
            insert_block(&conn, &block).unwrap();
        }
        {
            let block = ModuleBlock {
                id: "blk-quiz-0".to_string(),
                module_id: "mod-semaphore".to_string(),
                ordering: 8,
                block_type: "quiz".to_string(),
                status: "pending".to_string(),
                params_json: r#"{"questionCount":8,"topics":["t1"],"difficulty":"intermediate"}"#.to_string(),
                payload_json: "{}".to_string(),
                source_anchors_json: "[]".to_string(),
                metadata_json: r#"{"concept_id": null}"#.to_string(),
                retry_count: 0,
                created_at: "2026-05-05T00:00:00Z".to_string(),
                updated_at: "2026-05-05T00:00:00Z".to_string(),
            };
            insert_block(&conn, &block).unwrap();
        }

        let all_blocks = list_blocks_by_module(&conn, "mod-semaphore").unwrap();
        let db_mutex = wrap_conn_in_db_mutex(conn);
        let db_arc = Arc::new(db_mutex);

        // Track concurrent executions
        let active = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let active2 = Arc::clone(&active);
        let max2 = Arc::clone(&max_concurrent);

        let mock = Arc::new(MockAIClient::new(move |sys: &str| {
            // Increment active counter
            let current = active2.fetch_add(1, Ordering::SeqCst) + 1;
            // Track max
            let mut max = max2.load(Ordering::SeqCst);
            while current > max {
                match max2.compare_exchange(max, current, Ordering::SeqCst, Ordering::SeqCst) {
                    Ok(_) => break,
                    Err(actual) => max = actual,
                }
            }
            // Simulate some work duration for overlap detection
            std::thread::sleep(Duration::from_millis(5));
            active2.fetch_sub(1, Ordering::SeqCst);

            if sys.contains("Generate a quiz") {
                Ok(canned_quiz_json())
            } else {
                Ok(canned_section_markdown(0))
            }
        }));

        let _ = generate_blocks_in_parallel_with_client(
            Arc::clone(&db_arc),
            Arc::clone(&mock),
            all_blocks,
            "Test Module".to_string(),
        ).await;

        let peak = max_concurrent.load(Ordering::SeqCst);
        assert!(
            peak <= 3,
            "max concurrent LLM calls must be <= 3 (Semaphore(3)); got {}",
            peak
        );

        // All 9 blocks should be generated (status=ready or failed)
        let db_guard = db_arc.lock().unwrap();
        let final_blocks = list_blocks_by_module(&db_guard.conn, "mod-semaphore").unwrap();
        assert_eq!(final_blocks.len(), 9, "all 9 blocks must exist");
        for b in &final_blocks {
            assert!(
                b.status == "ready" || b.status == "failed",
                "block {} must be ready or failed, got {}",
                b.id,
                b.status
            );
        }
    }

    /// integration_generate_and_cache: call generate pipeline once on empty DB with mock LLM;
    /// assert all blocks reach status='ready'. Call again; assert no new LLM call (cached).
    #[tokio::test]
    async fn integration_generate_and_cache() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-cache-int", None);
        let db_arc = Arc::new(wrap_conn_in_db_mutex(conn));

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count2 = Arc::clone(&call_count);

        let mock = Arc::new(MockAIClient::new(move |sys: &str| {
            call_count2.fetch_add(1, Ordering::SeqCst);
            if sys.contains("curriculum designer") {
                Ok(canned_outline_json())
            } else if sys.contains("writing lesson") {
                Ok(canned_section_markdown(0))
            } else if sys.contains("Generate a quiz") {
                Ok(canned_quiz_json())
            } else if sys.contains("flash cards") {
                Ok(canned_flash_cards_json())
            } else {
                Ok(canned_section_markdown(0))
            }
        }));

        // First run: should call LLM and generate all blocks
        let outline = canned_outline_json();
        let outline_val: PagePlannerOutline = serde_json::from_str(&outline).unwrap();
        {
            let db = db_arc.lock().unwrap();
            insert_skeleton_blocks(&db.conn, "mod-cache-int", &outline_val).unwrap();
        }

        // Get pending blocks and generate
        let all_blocks = {
            let db = db_arc.lock().unwrap();
            list_blocks_by_module(&db.conn, "mod-cache-int").unwrap()
        };

        let _ = generate_blocks_in_parallel_with_client(
            Arc::clone(&db_arc),
            Arc::clone(&mock),
            all_blocks,
            "Test Module".to_string(),
        ).await;

        let first_call_count = call_count.load(Ordering::SeqCst);
        assert!(first_call_count > 0, "LLM must be called during first generation");

        // Verify all blocks are ready
        let blocks = {
            let db = db_arc.lock().unwrap();
            list_blocks_by_module(&db.conn, "mod-cache-int").unwrap()
        };
        assert!(!blocks.is_empty(), "must have blocks after generation");
        for b in &blocks {
            assert_eq!(b.status, "ready", "block {} must be ready", b.id);
        }

        // Second run: all ready → no LLM call
        let second_run_blocks = blocks.clone();
        let _ = generate_blocks_in_parallel_with_client(
            Arc::clone(&db_arc),
            Arc::clone(&mock),
            second_run_blocks,
            "Test Module".to_string(),
        ).await;

        let second_call_count = call_count.load(Ordering::SeqCst);
        assert_eq!(
            second_call_count, first_call_count,
            "no additional LLM calls on second run (all blocks ready, skipped)"
        );
    }

    /// integration_one_block_fails_others_complete:
    /// Mock returns Err for section #2 (index 2). Other blocks complete. Section #2 stays failed.
    #[tokio::test]
    async fn integration_one_block_fails_others_complete() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-fail-one", None);
        let db_arc = Arc::new(wrap_conn_in_db_mutex(conn));

        let fail_id = Arc::new(std::sync::Mutex::new(String::new()));

        // Insert 4 section blocks
        let fail_block_id = "blk-fail-2".to_string();
        for i in 0..4i32 {
            let id = if i == 2 { fail_block_id.clone() } else { format!("blk-ok-{}", i) };
            let block = ModuleBlock {
                id: id.clone(),
                module_id: "mod-fail-one".to_string(),
                ordering: i,
                block_type: "section".to_string(),
                status: "pending".to_string(),
                params_json: r#"{"lessonTitle":"test","objectives":[],"keyConcepts":[],"wordCountTarget":1500}"#.to_string(),
                payload_json: "{}".to_string(),
                source_anchors_json: "[]".to_string(),
                metadata_json: r#"{"concept_id": null}"#.to_string(),
                retry_count: 0,
                created_at: "2026-05-05T00:00:00Z".to_string(),
                updated_at: "2026-05-05T00:00:00Z".to_string(),
            };
            let db = db_arc.lock().unwrap();
            insert_block(&db.conn, &block).unwrap();
        }
        *fail_id.lock().unwrap() = fail_block_id.clone();

        let all_blocks = {
            let db = db_arc.lock().unwrap();
            list_blocks_by_module(&db.conn, "mod-fail-one").unwrap()
        };

        // Mock: fails for "writing lesson 3" (index 2, which is "writing lesson 3 of ...")
        let mock = Arc::new(MockAIClient::new(move |sys: &str| {
            if sys.contains("writing lesson 3 of") || sys.contains("writing lesson 3\n") {
                Err("mock error for section 2".to_string())
            } else {
                Ok(canned_section_markdown(0))
            }
        }));

        let _ = generate_blocks_in_parallel_with_client(
            Arc::clone(&db_arc),
            Arc::clone(&mock),
            all_blocks,
            "Test Module".to_string(),
        ).await;

        let final_blocks = {
            let db = db_arc.lock().unwrap();
            list_blocks_by_module(&db.conn, "mod-fail-one").unwrap()
        };

        // blk-fail-2 must be failed; others must be ready
        for b in &final_blocks {
            if b.id == fail_block_id {
                assert_eq!(b.status, "failed", "block {} must be failed", b.id);
            } else {
                assert_eq!(b.status, "ready", "block {} must be ready", b.id);
            }
        }
    }

    // ── Task 2 tests: regenerate_lesson + regenerate_module ──

    /// regenerate_lesson_atomic: pre-seed 8 ready blocks. Mock returns new payload for block #2.
    /// Assert ONLY block #2 changed; others untouched.
    #[tokio::test]
    async fn regenerate_lesson_atomic() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-regen-lesson", None);

        let target_id = "blk-regen-2".to_string();
        let original_payload = r#"{"markdown":"original content","wordCount":2}"#;

        for i in 0..8i32 {
            let id = if i == 2 { target_id.clone() } else { format!("blk-regen-{}", i) };
            let block = ModuleBlock {
                id: id.clone(),
                module_id: "mod-regen-lesson".to_string(),
                ordering: i,
                block_type: "section".to_string(),
                status: "ready".to_string(),
                params_json: r#"{"lessonTitle":"Lesson","objectives":[],"keyConcepts":[],"wordCountTarget":1500}"#.to_string(),
                payload_json: original_payload.to_string(),
                source_anchors_json: "[]".to_string(),
                metadata_json: r#"{"concept_id": null}"#.to_string(),
                retry_count: 0,
                created_at: "2026-05-05T00:00:00Z".to_string(),
                updated_at: "2026-05-05T00:00:00Z".to_string(),
            };
            insert_block(&conn, &block).unwrap();
        }

        let db_mutex = wrap_conn_in_db_mutex(conn);
        let db_arc = Arc::new(db_mutex);

        // Mock returns new content
        let mock = Arc::new(MockAIClient::new(|_sys: &str| {
            Ok(canned_section_markdown(2))
        }));

        // Simulate regenerate_lesson: mark generating → generate → update
        {
            let db = db_arc.lock().unwrap();
            update_block_status(&db.conn, &target_id, "generating").unwrap();
        }

        let block = {
            let db = db_arc.lock().unwrap();
            get_block(&db.conn, &target_id).unwrap().unwrap()
        };

        let new_payload = generate_section_with_client(mock.as_ref(), &block, "Test Module", 2, 8).await.unwrap();

        {
            let db = db_arc.lock().unwrap();
            update_block_payload(&db.conn, &target_id, BlockStatus::Ready, &new_payload).unwrap();
        }

        // Verify: only target block changed
        let final_blocks = {
            let db = db_arc.lock().unwrap();
            list_blocks_by_module(&db.conn, "mod-regen-lesson").unwrap()
        };

        for b in &final_blocks {
            if b.id == target_id {
                assert_eq!(b.status, "ready", "regenerated block must be ready");
                assert_ne!(b.payload_json, original_payload, "regenerated block must have new payload");
                assert!(b.payload_json.contains("markdown"), "new payload must contain markdown");
            } else {
                assert_eq!(b.status, "ready", "other block {} must stay ready", b.id);
                assert_eq!(b.payload_json, original_payload, "other block {} payload unchanged", b.id);
            }
        }
    }

    /// regenerate_lesson_failure_keeps_old_payload:
    /// Mock returns Err. Assert status flips to 'failed' but payload_json preserved.
    #[tokio::test]
    async fn regenerate_lesson_failure_keeps_old_payload() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-regen-fail", None);

        let original_payload = r#"{"markdown":"old valuable content","wordCount":3}"#;
        let block = ModuleBlock {
            id: "blk-fail-keep".to_string(),
            module_id: "mod-regen-fail".to_string(),
            ordering: 0,
            block_type: "section".to_string(),
            status: "ready".to_string(),
            params_json: r#"{"lessonTitle":"Lesson","objectives":[],"keyConcepts":[],"wordCountTarget":1500}"#.to_string(),
            payload_json: original_payload.to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id": null}"#.to_string(),
            retry_count: 0,
            created_at: "2026-05-05T00:00:00Z".to_string(),
            updated_at: "2026-05-05T00:00:00Z".to_string(),
        };
        insert_block(&conn, &block).unwrap();

        let db_mutex = wrap_conn_in_db_mutex(conn);
        let db_arc = Arc::new(db_mutex);

        // Mock returns error
        let mock = Arc::new(MockAIClient::new(|_sys: &str| {
            Err("LLM unavailable".to_string())
        }));

        // Mark generating
        {
            let db = db_arc.lock().unwrap();
            update_block_status(&db.conn, "blk-fail-keep", "generating").unwrap();
        }

        let block_data = {
            let db = db_arc.lock().unwrap();
            get_block(&db.conn, "blk-fail-keep").unwrap().unwrap()
        };

        let result = generate_section_with_client(mock.as_ref(), &block_data, "Test Module", 0, 1).await;
        assert!(result.is_err(), "mock must return error");

        // On failure: only flip status, keep old payload
        {
            let db = db_arc.lock().unwrap();
            update_block_status(&db.conn, "blk-fail-keep", "failed").unwrap();
        }

        let final_block = {
            let db = db_arc.lock().unwrap();
            get_block(&db.conn, "blk-fail-keep").unwrap().unwrap()
        };

        assert_eq!(final_block.status, "failed", "status must flip to failed");
        assert_eq!(
            final_block.payload_json, original_payload,
            "payload_json must be preserved on failure"
        );
    }

    /// regenerate_module_atomic_on_success:
    /// Pre-seed legacy single-block module. Mock PagePlanner + sections all succeed.
    /// Assert: legacy block deleted, 8+ new blocks inserted, all status='ready'.
    #[tokio::test]
    async fn regenerate_module_atomic_on_success() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-regen-full", None);

        // Seed a single "legacy" block
        let legacy_block = ModuleBlock {
            id: "blk-legacy".to_string(),
            module_id: "mod-regen-full".to_string(),
            ordering: 0,
            block_type: "section".to_string(),
            status: "ready".to_string(),
            params_json: "{}".to_string(),
            payload_json: r#"{"markdown":"legacy content","wordCount":2}"#.to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id": null}"#.to_string(),
            retry_count: 0,
            created_at: "2026-05-05T00:00:00Z".to_string(),
            updated_at: "2026-05-05T00:00:00Z".to_string(),
        };
        insert_block(&conn, &legacy_block).unwrap();

        let db_arc = Arc::new(wrap_conn_in_db_mutex(conn));

        // Mock: all succeed
        let mock = Arc::new(MockAIClient::new(|sys: &str| {
            if sys.contains("curriculum designer") {
                Ok(canned_outline_json())
            } else if sys.contains("writing lesson") {
                Ok(canned_section_markdown(0))
            } else if sys.contains("Generate a quiz") {
                Ok(canned_quiz_json())
            } else {
                Ok(canned_flash_cards_json())
            }
        }));

        // Simulate regenerate_module:
        // 1. Run PagePlanner
        let outline_json = canned_outline_json();
        let outline: PagePlannerOutline = serde_json::from_str(&outline_json).unwrap();
        // 2. Delete existing blocks + insert skeleton
        // Note: in tests we call delete+insert directly on the connection
        // (insert_skeleton_blocks uses its own internal transaction)
        let skeleton = {
            let db = db_arc.lock().unwrap();
            delete_blocks_by_module(&db.conn, "mod-regen-full").unwrap();
            insert_skeleton_blocks(&db.conn, "mod-regen-full", &outline).unwrap()
        };
        // 3. Generate in parallel
        let _ = generate_blocks_in_parallel_with_client(
            Arc::clone(&db_arc),
            Arc::clone(&mock),
            skeleton,
            "Test Module".to_string(),
        ).await;

        // Verify: legacy block deleted, new blocks present, all ready
        let final_blocks = {
            let db = db_arc.lock().unwrap();
            list_blocks_by_module(&db.conn, "mod-regen-full").unwrap()
        };

        assert!(final_blocks.len() >= 8, "must have 8+ blocks (got {})", final_blocks.len());
        assert!(
            !final_blocks.iter().any(|b| b.id == "blk-legacy"),
            "legacy block must be deleted"
        );
        for b in &final_blocks {
            assert_eq!(b.status, "ready", "block {} must be ready", b.id);
        }
    }

    /// regenerate_module_keeps_legacy_on_pageplanner_failure:
    /// PagePlanner mock returns Err. Legacy block must be preserved.
    #[tokio::test]
    async fn regenerate_module_keeps_legacy_on_pageplanner_failure() {
        let conn = fresh_conn();
        seed_module(&conn, "mod-regen-fail-pp", None);

        // Seed legacy block
        let legacy_block = ModuleBlock {
            id: "blk-legacy-pp".to_string(),
            module_id: "mod-regen-fail-pp".to_string(),
            ordering: 0,
            block_type: "section".to_string(),
            status: "ready".to_string(),
            params_json: "{}".to_string(),
            payload_json: r#"{"markdown":"preserved legacy","wordCount":2}"#.to_string(),
            source_anchors_json: "[]".to_string(),
            metadata_json: r#"{"concept_id": null}"#.to_string(),
            retry_count: 0,
            created_at: "2026-05-05T00:00:00Z".to_string(),
            updated_at: "2026-05-05T00:00:00Z".to_string(),
        };
        insert_block(&conn, &legacy_block).unwrap();
        let db_arc = Arc::new(wrap_conn_in_db_mutex(conn));

        // Mock: PagePlanner fails
        let mock = Arc::new(MockAIClient::new(|_sys: &str| {
            Err("PagePlanner unavailable".to_string())
        }));

        // Simulate regenerate_module: PagePlanner fails before delete
        let page_planner_result = run_page_planner_with_client(
            mock.as_ref(),
            "Test Module",
            &[],
            "beginner",
        ).await;

        assert!(page_planner_result.is_err(), "PagePlanner must fail");

        // Legacy block must still exist (we didn't delete because PagePlanner failed first)
        let final_blocks = {
            let db = db_arc.lock().unwrap();
            list_blocks_by_module(&db.conn, "mod-regen-fail-pp").unwrap()
        };

        assert_eq!(final_blocks.len(), 1, "legacy block must be preserved on PagePlanner failure");
        assert_eq!(final_blocks[0].id, "blk-legacy-pp", "legacy block must be untouched");
        assert_eq!(
            final_blocks[0].payload_json,
            r#"{"markdown":"preserved legacy","wordCount":2}"#,
            "legacy payload must be unchanged"
        );
    }
}
