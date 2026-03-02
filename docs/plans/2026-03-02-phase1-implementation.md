# LearnForge Phase 1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a working MVP where users authenticate via OAuth (Claude/OpenAI/Gemini subscriptions), create learning tracks, complete AI-driven assessments, receive personalized learning paths, study AI-generated content, and complete exercises -- all with a glassmorphism dark+light UI.

**Architecture:** Tauri 2 (Rust) + React + TypeScript. Zeroclaw embedded as Rust crate for AI provider auth/calls. RuVector embedded for vector search + graph DB. SQLite for structured CRUD. Progressive content generation (on-demand per module).

**Tech Stack:** Tauri 2, React 18, TypeScript, Zustand, shadcn/ui, Tailwind CSS, Zeroclaw (Rust), RuVector (Rust), SQLite, Vite

**Parallelization:** Tasks marked [PARALLEL-GROUP-X] can be dispatched simultaneously via ruflo agents. Tasks within a group have no dependencies on each other.

---

## Task 1: Add Rust Dependencies (Zeroclaw + RuVector + async-trait)

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: Add dependencies to Cargo.toml**

Add after line 26 (`env_logger = "0.11"`):

```toml
async-trait = "0.1"

# AI provider auth + API calls (embedded)
[dependencies.zeroclaw]
path = "../../agentix/upstream/zeroclaw"
default-features = false

# Vector DB for semantic intelligence
[dependencies.ruvector-core]
path = "../../agentix/upstream/ruvector/crates/ruvector-core"
features = ["storage", "hnsw", "simd", "parallel"]

# Graph DB for learning path DAGs
[dependencies.ruvector-graph]
path = "../../agentix/upstream/ruvector/crates/ruvector-graph"
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -20`

Expected: Successful compilation (warnings OK, no errors). If zeroclaw or ruvector have incompatible deps, we may need to pin versions or use feature flags.

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "feat: add zeroclaw, ruvector-core, ruvector-graph dependencies"
```

---

## Task 2: Implement Zeroclaw Auth Integration

**Files:**
- Create: `src-tauri/src/auth/mod.rs`
- Create: `src-tauri/src/auth/commands.rs`
- Modify: `src-tauri/src/lib.rs` (add auth module + commands)
- Modify: `src/lib/tauri-commands.ts` (add auth IPC wrappers)
- Modify: `src/types/ai.ts` (add auth types)

**Step 1: Create auth module that wraps zeroclaw's AuthService**

Create `src-tauri/src/auth/mod.rs`:

```rust
pub mod commands;

use zeroclaw::auth::AuthService;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AuthState {
    pub service: Arc<Mutex<AuthService>>,
}

impl AuthState {
    pub fn new(state_dir: &PathBuf) -> Self {
        let service = AuthService::new(state_dir, true);
        Self {
            service: Arc::new(Mutex::new(service)),
        }
    }
}
```

**Step 2: Create auth Tauri commands**

Create `src-tauri/src/auth/commands.rs`:

```rust
use crate::auth::AuthState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderAuthStatus {
    pub provider: String,
    pub authenticated: bool,
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthLoginRequest {
    pub provider: String, // "openai-codex" | "anthropic" | "gemini"
    pub method: String,   // "oauth" | "api-key" | "setup-token"
    pub credential: Option<String>, // API key or setup token if method != "oauth"
}

/// Get authentication status for all providers
#[tauri::command]
pub async fn get_auth_status(
    auth: State<'_, AuthState>,
) -> Result<Vec<ProviderAuthStatus>, String> {
    let service = auth.service.lock().await;
    let providers = vec!["openai-codex", "anthropic", "gemini"];
    let mut statuses = Vec::new();

    for provider in providers {
        let has_token = service
            .get_valid_token(provider, "default")
            .await
            .is_ok();
        statuses.push(ProviderAuthStatus {
            provider: provider.to_string(),
            authenticated: has_token,
            display_name: None,
        });
    }

    Ok(statuses)
}

/// Initiate OAuth login for a provider (opens browser)
#[tauri::command]
pub async fn login_provider(
    auth: State<'_, AuthState>,
    request: AuthLoginRequest,
) -> Result<ProviderAuthStatus, String> {
    let service = auth.service.lock().await;

    match request.method.as_str() {
        "oauth" => {
            // Zeroclaw handles OAuth flow (opens browser, localhost callback)
            service
                .login_oauth(&request.provider, "default")
                .await
                .map_err(|e| format!("OAuth login failed: {}", e))?;
        }
        "api-key" | "setup-token" => {
            let credential = request
                .credential
                .ok_or("Credential required for API key/token auth")?;
            service
                .store_token(&request.provider, "default", &credential)
                .await
                .map_err(|e| format!("Failed to store credential: {}", e))?;
        }
        _ => return Err(format!("Unknown auth method: {}", request.method)),
    }

    Ok(ProviderAuthStatus {
        provider: request.provider,
        authenticated: true,
        display_name: None,
    })
}

/// Logout from a provider
#[tauri::command]
pub async fn logout_provider(
    auth: State<'_, AuthState>,
    provider: String,
) -> Result<(), String> {
    let service = auth.service.lock().await;
    service
        .remove_token(&provider, "default")
        .await
        .map_err(|e| e.to_string())
}
```

NOTE: The exact zeroclaw API methods (`login_oauth`, `get_valid_token`, `store_token`, `remove_token`) need to be verified against the actual zeroclaw crate's public API. Check `zeroclaw/src/auth/mod.rs` for the real method signatures and adapt accordingly.

**Step 3: Register auth module and commands in lib.rs**

Modify `src-tauri/src/lib.rs`:
- Add `mod auth;` after `mod ai;`
- Add `use auth::AuthState;`
- In `setup()`, create `AuthState` and manage it:
  ```rust
  let auth_dir = app_dir.join("auth");
  std::fs::create_dir_all(&auth_dir).expect("Failed to create auth dir");
  app.manage(AuthState::new(&auth_dir));
  ```
- Add commands to invoke_handler:
  ```rust
  // Auth commands
  auth::commands::get_auth_status,
  auth::commands::login_provider,
  auth::commands::logout_provider,
  ```

**Step 4: Add TypeScript types and IPC wrappers**

Add to `src/types/ai.ts`:
```typescript
export interface ProviderAuthStatus {
  provider: string;
  authenticated: boolean;
  displayName: string | null;
}

export interface AuthLoginRequest {
  provider: string;
  method: "oauth" | "api-key" | "setup-token";
  credential?: string;
}
```

Add to `src/lib/tauri-commands.ts`:
```typescript
import type { ProviderAuthStatus, AuthLoginRequest } from "@/types/ai";

export async function getAuthStatus(): Promise<ProviderAuthStatus[]> {
  return invoke("get_auth_status");
}

export async function loginProvider(req: AuthLoginRequest): Promise<ProviderAuthStatus> {
  return invoke("login_provider", { request: req });
}

export async function logoutProvider(provider: string): Promise<void> {
  return invoke("logout_provider", { provider });
}
```

**Step 5: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: Compiles successfully.

**Step 6: Commit**

```bash
git add src-tauri/src/auth/ src-tauri/src/lib.rs src/types/ai.ts src/lib/tauri-commands.ts
git commit -m "feat: add zeroclaw auth integration with OAuth, BYOK, and setup-token support"
```

---

## Task 3: Implement AI Request Layer via Zeroclaw Providers

**Files:**
- Rewrite: `src-tauri/src/ai/provider.rs` (replace stub with zeroclaw-backed impl)
- Rewrite: `src-tauri/src/ai/claude.rs` (remove, use zeroclaw)
- Rewrite: `src-tauri/src/ai/openai.rs` (remove, use zeroclaw)
- Rewrite: `src-tauri/src/ai/ollama.rs` (remove, use zeroclaw)
- Create: `src-tauri/src/ai/service.rs` (unified AI request function)
- Modify: `src-tauri/src/ai/mod.rs`

**Step 1: Create unified AI service that routes through zeroclaw**

Create `src-tauri/src/ai/service.rs`:

```rust
use crate::auth::AuthState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIRequest {
    pub system_prompt: String,
    pub messages: Vec<AIMessage>,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f64>,
    pub response_format: Option<String>, // "json" for structured responses
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIServiceResponse {
    pub content: String,
    pub model: String,
    pub tokens_used: i32,
}

/// Central AI request function. All AI calls go through here.
/// Routes to the authenticated provider via zeroclaw.
pub async fn ai_request(
    auth: &AuthState,
    provider_name: &str,
    request: AIRequest,
) -> Result<AIServiceResponse, String> {
    let service = auth.service.lock().await;

    // Get valid token for the provider
    let token = service
        .get_valid_token(provider_name, "default")
        .await
        .map_err(|e| format!("Not authenticated with {}: {}", provider_name, e))?;

    // Create zeroclaw provider and call it
    // NOTE: Adapt to actual zeroclaw Provider trait API
    let provider = zeroclaw::providers::create_provider(provider_name, &token)
        .map_err(|e| format!("Failed to create provider: {}", e))?;

    let messages: Vec<zeroclaw::providers::ChatMessage> = request
        .messages
        .iter()
        .map(|m| zeroclaw::providers::ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    let response = provider
        .chat_with_system(
            Some(&request.system_prompt),
            &messages.last().map(|m| m.content.as_str()).unwrap_or(""),
            "auto", // model selection
            request.temperature.unwrap_or(0.7),
        )
        .await
        .map_err(|e| format!("AI request failed: {}", e))?;

    Ok(AIServiceResponse {
        content: response,
        model: provider_name.to_string(),
        tokens_used: 0, // TODO: extract from provider response
    })
}
```

NOTE: The zeroclaw provider API (`create_provider`, `chat_with_system`, `ChatMessage`) must be verified against the actual crate. Check `zeroclaw/src/providers/traits.rs` and `zeroclaw/src/providers/mod.rs` for real signatures.

**Step 2: Update ai/mod.rs**

```rust
pub mod service;
pub use service::{ai_request, AIMessage, AIRequest, AIServiceResponse};
```

Remove the old `claude.rs`, `openai.rs`, `ollama.rs`, `provider.rs` files or keep them as dead code until zeroclaw integration is verified.

**Step 3: Verify compilation**

Run: `cd src-tauri && cargo check`

**Step 4: Commit**

```bash
git add src-tauri/src/ai/
git commit -m "feat: replace AI provider stubs with zeroclaw-backed unified AI service"
```

---

## Task 4: Wire Up AI Commands (Assessment + Path Generation)

**Files:**
- Rewrite: `src-tauri/src/commands/ai.rs`
- Modify: `src-tauri/src/db/schema.rs` (add content caching to modules table if needed)

**Step 1: Implement assess_knowledge with real AI**

Replace the mock in `commands/ai.rs`:

```rust
use crate::ai::{ai_request, AIMessage, AIRequest};
use crate::auth::AuthState;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct AssessmentTurn {
    pub role: String,  // "assistant" or "user"
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssessmentRequest {
    pub topic: String,
    pub domain: String,
    pub messages: Vec<AssessmentTurn>, // conversation so far
}

#[tauri::command]
pub async fn assess_knowledge(
    auth: State<'_, AuthState>,
    state: State<'_, AppState>,
    request: AssessmentRequest,
) -> Result<String, String> {
    let config = get_ai_config_internal(&state)?;

    let system_prompt = format!(
        "You are an expert tutor assessing a learner's knowledge of {}. \
         Conduct a conversational assessment through 3-5 questions. \
         Use the Socratic method -- ask probing questions based on their responses. \
         Gauge their depth of understanding, not just surface knowledge. \
         After sufficient assessment, end your response with a JSON block: \
         ```json\n{{\"assessment_complete\": true, \"level\": \"beginner|intermediate|advanced\", \
         \"gaps\": [...], \"strengths\": [...], \"recommended_start\": \"...\"}}\n``` \
         Until assessment is complete, just ask your next question naturally.",
        request.topic
    );

    let messages: Vec<AIMessage> = request
        .messages
        .iter()
        .map(|m| AIMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    let response = ai_request(
        &auth.inner(),
        &config.provider_type,
        AIRequest {
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
```

**Step 2: Implement generate_learning_path with real AI + DB persistence**

```rust
#[derive(Debug, Serialize, Deserialize)]
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
    let config = get_ai_config_internal(&state)?;

    let system_prompt = format!(
        "You are a curriculum designer creating a personalized learning path for {}. \
         The learner's level: {}. Their goal: {}. \
         Gaps: {:?}. Strengths: {:?}. \
         Generate a learning path as a DAG of 8-15 modules. \
         Return ONLY valid JSON in this format: \
         {{\"modules\": [{{\"id\": \"m1\", \"title\": \"...\", \"description\": \"...\", \
         \"difficulty\": 1-5, \"estimated_minutes\": 30, \"objectives\": [\"...\"]}}], \
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
        &auth.inner(),
        &config.provider_type,
        AIRequest {
            system_prompt,
            messages: vec![AIMessage {
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
            rusqlite::params![path_id, request.track_id, response.content, config.model],
        )
        .map_err(|e| e.to_string())?;

    // Insert modules
    if let Some(modules) = path_data["modules"].as_array() {
        for module in modules {
            let module_id = module["id"].as_str().unwrap_or(&uuid::Uuid::new_v4().to_string());
            db.conn
                .execute(
                    "INSERT INTO modules (id, path_id, title, type, difficulty, estimated_minutes, objectives_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        module_id,
                        path_id,
                        module["title"].as_str().unwrap_or("Untitled"),
                        request.domain,
                        module["difficulty"].as_i64().unwrap_or(1),
                        module["estimated_minutes"].as_i64().unwrap_or(30),
                        serde_json::to_string(&module["objectives"]).unwrap_or_default(),
                    ],
                )
                .map_err(|e| e.to_string())?;

            // Create module_progress entry (first module unlocked, rest locked)
            let status = if module_id == modules[0]["id"].as_str().unwrap_or("") {
                "available"
            } else {
                "locked"
            };
            db.conn
                .execute(
                    "INSERT INTO module_progress (id, module_id, learner_id, status) VALUES (?1, ?2, (SELECT id FROM learner_profiles LIMIT 1), ?3)",
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
```

**Step 3: Implement send_tutor_message with real AI**

```rust
#[tauri::command]
pub async fn send_tutor_message(
    auth: State<'_, AuthState>,
    state: State<'_, AppState>,
    message: serde_json::Value,
) -> Result<String, String> {
    let config = get_ai_config_internal(&state)?;

    let module_context = message["moduleContext"].as_str().unwrap_or("");
    let user_message = message["content"].as_str().ok_or("Missing message content")?;
    let history: Vec<AIMessage> = message["history"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    Some(AIMessage {
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
    messages.push(AIMessage {
        role: "user".to_string(),
        content: user_message.to_string(),
    });

    let response = ai_request(
        &auth.inner(),
        &config.provider_type,
        AIRequest {
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
```

**Step 4: Add helper for reading AI config**

```rust
fn get_ai_config_internal(state: &AppState) -> Result<crate::db::models::AIConfig, String> {
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
```

**Step 5: Add generate_module_content command**

```rust
#[derive(Debug, Serialize, Deserialize)]
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
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let cached: Option<String> = db.conn
        .query_row(
            "SELECT content FROM modules WHERE id = ?1 AND content IS NOT NULL",
            [&request.module_id],
            |row| row.get(0),
        )
        .ok();

    if let Some(content) = cached {
        return Ok(content);
    }
    drop(db); // Release lock before async AI call

    let config = get_ai_config_internal(&state)?;

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
        request.module_title,
        request.objectives,
        request.learner_level,
        request.previous_performance
            .map(|p| format!("Previous module performance: {}. Adjust difficulty accordingly. ", p))
            .unwrap_or_default(),
    );

    let response = ai_request(
        &auth.inner(),
        &config.provider_type,
        AIRequest {
            system_prompt,
            messages: vec![AIMessage {
                role: "user".to_string(),
                content: format!("Generate the lesson content for: {}", request.module_title),
            }],
            max_tokens: Some(4096),
            temperature: Some(0.6),
            response_format: None,
        },
    )
    .await?;

    // Cache in DB
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "UPDATE modules SET content = ?1 WHERE id = ?2",
            rusqlite::params![response.content, request.module_id],
        )
        .map_err(|e| e.to_string())?;

    Ok(response.content)
}
```

**Step 6: Register new commands in lib.rs**

Add to invoke_handler:
```rust
commands::ai::generate_module_content,
```

**Step 7: Add TypeScript wrapper**

Add to `src/lib/tauri-commands.ts`:
```typescript
export async function generateModuleContent(req: {
  moduleId: string;
  trackId: string;
  moduleTitle: string;
  objectives: string[];
  learnerLevel: string;
  previousPerformance?: string;
}): Promise<string> {
  return invoke("generate_module_content", { request: req });
}
```

**Step 8: Verify compilation**

Run: `cd src-tauri && cargo check`

**Step 9: Commit**

```bash
git add src-tauri/src/commands/ai.rs src-tauri/src/lib.rs src/lib/tauri-commands.ts
git commit -m "feat: wire AI commands to zeroclaw - assessment, path gen, tutor, content gen"
```

---

## Task 5: Initialize RuVector in App Setup

**Files:**
- Create: `src-tauri/src/vector/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Create vector store module**

Create `src-tauri/src/vector/mod.rs`:

```rust
use ruvector_core::{VectorDB, VectorEntry, SearchQuery, DbOptions, DistanceMetric};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct VectorStore {
    pub db: Arc<Mutex<VectorDB>>,
}

impl VectorStore {
    pub fn new(storage_path: &str) -> Result<Self, String> {
        let options = DbOptions {
            dimensions: 384, // all-MiniLM-L6-v2 or provider embeddings
            distance_metric: DistanceMetric::Cosine,
            storage_path: storage_path.to_string(),
            ..Default::default()
        };
        let db = VectorDB::new(options).map_err(|e| format!("VectorDB init failed: {}", e))?;
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
        })
    }

    pub async fn index_concept(&self, id: &str, embedding: Vec<f32>, metadata: serde_json::Value) -> Result<(), String> {
        let db = self.db.lock().await;
        db.insert(VectorEntry {
            id: Some(id.to_string()),
            vector: embedding,
            metadata: Some(
                metadata.as_object()
                    .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default()
            ),
        }).map_err(|e| format!("Insert failed: {}", e))
    }

    pub async fn find_similar(&self, embedding: Vec<f32>, k: usize) -> Result<Vec<(String, f32)>, String> {
        let db = self.db.lock().await;
        let results = db.search(&SearchQuery {
            vector: embedding,
            k,
            filter: None,
            ef_search: None,
        }).map_err(|e| format!("Search failed: {}", e))?;

        Ok(results.into_iter().map(|r| (r.id, r.score)).collect())
    }
}
```

NOTE: Verify `VectorDB::new`, `DbOptions`, `VectorEntry`, `SearchQuery` against actual ruvector-core API. Check `ruvector-core/src/types.rs` and `ruvector-core/src/lib.rs`.

**Step 2: Add to AppState and setup in lib.rs**

Modify `src-tauri/src/lib.rs`:
- Add `mod vector;`
- Add `use vector::VectorStore;`
- In `setup()`:
  ```rust
  let vector_path = app_dir.join("vectors");
  std::fs::create_dir_all(&vector_path).expect("Failed to create vector dir");
  let vector_store = VectorStore::new(
      vector_path.to_str().expect("Invalid vector path")
  ).expect("Failed to initialize VectorStore");
  app.manage(vector_store);
  ```

**Step 3: Verify compilation**

Run: `cd src-tauri && cargo check`

**Step 4: Commit**

```bash
git add src-tauri/src/vector/ src-tauri/src/lib.rs
git commit -m "feat: initialize ruvector store for concept embeddings"
```

---

## Task 6: Design System + Theme Setup [PARALLEL-GROUP-A]

**Files:**
- Modify: `src/index.css` (CSS variables for dark + light)
- Create: `src/hooks/useTheme.ts`
- Modify: `src/stores/useAppStore.ts` (theme persistence)
- Modify: `tailwind.config.ts` (extend theme)

**Step 1: Define CSS variables for dark + light themes**

Replace content of `src/index.css` with glassmorphism design system variables:

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  :root {
    /* Light theme */
    --background: 220 14% 96%;
    --foreground: 222 47% 11%;
    --card: 0 0% 100%;
    --card-foreground: 222 47% 11%;
    --popover: 0 0% 100%;
    --popover-foreground: 222 47% 11%;
    --primary: 24 95% 53%;
    --primary-foreground: 0 0% 100%;
    --secondary: 220 14% 92%;
    --secondary-foreground: 222 47% 11%;
    --muted: 220 14% 92%;
    --muted-foreground: 215 16% 47%;
    --accent: 220 14% 92%;
    --accent-foreground: 222 47% 11%;
    --destructive: 0 84% 60%;
    --destructive-foreground: 0 0% 100%;
    --border: 220 13% 87%;
    --input: 220 13% 87%;
    --ring: 24 95% 53%;
    --radius: 0.75rem;

    /* Glass effects - light */
    --glass-bg: rgba(255, 255, 255, 0.6);
    --glass-border: rgba(255, 255, 255, 0.3);
    --glass-blur: 16px;
    --glass-shadow: 0 8px 32px rgba(0, 0, 0, 0.06);

    /* Track accent colors */
    --track-kubernetes: 217 91% 60%;
    --track-rust: 15 75% 55%;
    --track-go: 187 72% 51%;
    --track-python: 45 93% 58%;

    /* Status colors */
    --success: 142 71% 45%;
    --warning: 38 92% 50%;
    --info: 217 91% 60%;
  }

  .dark {
    --background: 235 25% 12%;
    --foreground: 210 40% 98%;
    --card: 235 20% 16%;
    --card-foreground: 210 40% 98%;
    --popover: 235 20% 16%;
    --popover-foreground: 210 40% 98%;
    --primary: 24 95% 53%;
    --primary-foreground: 0 0% 100%;
    --secondary: 235 15% 22%;
    --secondary-foreground: 210 40% 98%;
    --muted: 235 15% 22%;
    --muted-foreground: 215 20% 65%;
    --accent: 235 15% 22%;
    --accent-foreground: 210 40% 98%;
    --destructive: 0 84% 60%;
    --destructive-foreground: 0 0% 100%;
    --border: 235 15% 25%;
    --input: 235 15% 25%;
    --ring: 24 95% 53%;

    /* Glass effects - dark */
    --glass-bg: rgba(255, 255, 255, 0.05);
    --glass-border: rgba(255, 255, 255, 0.08);
    --glass-blur: 20px;
    --glass-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);

    --track-kubernetes: 217 91% 60%;
    --track-rust: 15 75% 55%;
    --track-go: 187 72% 51%;
    --track-python: 45 93% 58%;

    --success: 142 71% 45%;
    --warning: 38 92% 50%;
    --info: 217 91% 60%;
  }
}

@layer base {
  body {
    @apply bg-background text-foreground;
    font-feature-settings: "rlig" 1, "calt" 1;
  }
}

@layer utilities {
  .glass {
    background: var(--glass-bg);
    backdrop-filter: blur(var(--glass-blur));
    -webkit-backdrop-filter: blur(var(--glass-blur));
    border: 1px solid var(--glass-border);
    box-shadow: var(--glass-shadow);
  }

  .glass-strong {
    background: var(--glass-bg);
    backdrop-filter: blur(calc(var(--glass-blur) * 1.5));
    -webkit-backdrop-filter: blur(calc(var(--glass-blur) * 1.5));
    border: 1px solid var(--glass-border);
    box-shadow: var(--glass-shadow);
  }
}
```

**Step 2: Create useTheme hook**

Create `src/hooks/useTheme.ts`:

```typescript
import { useEffect } from "react";
import { useAppStore } from "@/stores/useAppStore";

export function useTheme() {
  const { theme, setTheme } = useAppStore();

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "dark") {
      root.classList.add("dark");
    } else {
      root.classList.remove("dark");
    }
  }, [theme]);

  const toggleTheme = () => {
    setTheme(theme === "dark" ? "light" : "dark");
  };

  return { theme, setTheme, toggleTheme };
}
```

**Step 3: Update useAppStore with theme persistence**

Ensure `useAppStore` has `theme` and `setTheme`:
```typescript
theme: "dark" as "dark" | "light",
setTheme: (theme: "dark" | "light") => set({ theme }),
```

**Step 4: Apply theme in App.tsx**

Add `useTheme()` call in App component so the class is set on mount.

**Step 5: Commit**

```bash
git add src/index.css src/hooks/useTheme.ts src/stores/useAppStore.ts src/App.tsx
git commit -m "feat: add glassmorphism design system with dark + light theme support"
```

---

## Task 7: Dashboard Redesign [PARALLEL-GROUP-A]

**Files:**
- Rewrite: `src/pages/Dashboard.tsx`
- Create: `src/components/dashboard/StatsCard.tsx`
- Create: `src/components/dashboard/TrackCard.tsx`
- Create: `src/components/dashboard/SmartSessionCard.tsx`

Reference: prototype screenshot (dark theme, greeting, stats row, track cards with colored borders).

**Step 1: Create StatsCard component**

```tsx
// src/components/dashboard/StatsCard.tsx
interface StatsCardProps {
  label: string;
  value: string | number;
  subtitle: string;
  accentColor?: string;
}

export function StatsCard({ label, value, subtitle, accentColor }: StatsCardProps) {
  return (
    <div className="glass rounded-xl p-4">
      <div className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
        {label}
      </div>
      <div
        className="mt-1 text-2xl font-bold"
        style={accentColor ? { color: accentColor } : undefined}
      >
        {value}
      </div>
      <div className="text-xs text-muted-foreground">{subtitle}</div>
    </div>
  );
}
```

**Step 2: Create TrackCard component**

```tsx
// src/components/dashboard/TrackCard.tsx
import { Link } from "react-router-dom";
import { ArrowRight } from "lucide-react";
import type { LearningTrack } from "@/types";

const trackColors: Record<string, string> = {
  kubernetes: "hsl(var(--track-kubernetes))",
  rust: "hsl(var(--track-rust))",
  go: "hsl(var(--track-go))",
  python: "hsl(var(--track-python))",
};

function getTrackColor(topic: string): string {
  const key = topic.toLowerCase();
  for (const [name, color] of Object.entries(trackColors)) {
    if (key.includes(name)) return color;
  }
  return "hsl(var(--primary))";
}

interface TrackCardProps {
  track: LearningTrack;
  reviewsDue?: number;
}

export function TrackCard({ track, reviewsDue = 0 }: TrackCardProps) {
  const color = getTrackColor(track.topic);

  return (
    <Link
      to={`/track/${track.id}`}
      className="glass group relative overflow-hidden rounded-xl p-5 transition-all hover:-translate-y-0.5"
    >
      {/* Colored top border */}
      <div
        className="absolute left-0 right-0 top-0 h-1"
        style={{ backgroundColor: color }}
      />

      <div className="flex items-start justify-between">
        <h3 className="text-lg font-semibold text-foreground">{track.topic}</h3>
      </div>

      {/* Progress bar */}
      <div className="mt-3 h-1.5 rounded-full bg-secondary">
        <div
          className="h-1.5 rounded-full transition-all"
          style={{
            width: `${track.progressPercent}%`,
            backgroundColor: color,
          }}
        />
      </div>

      {/* Stats grid */}
      <div className="mt-4 grid grid-cols-2 gap-x-6 gap-y-2 text-sm">
        <div>
          <span className="text-xs uppercase text-muted-foreground">Progress</span>
          <div className="font-medium text-foreground">{track.progressPercent}%</div>
        </div>
        <div>
          <span className="text-xs uppercase text-muted-foreground">Reviews</span>
          <div className={reviewsDue > 0 ? "font-medium text-destructive" : "font-medium text-foreground"}>
            {reviewsDue} due
          </div>
        </div>
      </div>

      {/* Next module hint */}
      <div className="mt-3 flex items-center gap-1 text-xs text-muted-foreground">
        <ArrowRight size={12} />
        <span>Next: {track.goal || "Continue learning"}</span>
      </div>
    </Link>
  );
}
```

**Step 3: Create SmartSessionCard component**

```tsx
// src/components/dashboard/SmartSessionCard.tsx
import { Link } from "react-router-dom";
import { Zap } from "lucide-react";

interface SmartSessionCardProps {
  dueCount: number;
  nextModule?: string;
  estimatedMinutes?: number;
}

export function SmartSessionCard({ dueCount, nextModule, estimatedMinutes = 25 }: SmartSessionCardProps) {
  const parts: string[] = [];
  if (dueCount > 0) parts.push(`${dueCount} review cards`);
  if (nextModule) parts.push(`Continue ${nextModule}`);

  if (parts.length === 0) return null;

  return (
    <div className="glass relative overflow-hidden rounded-xl p-5"
      style={{ borderImage: "linear-gradient(135deg, hsl(24, 95%, 53%), hsl(270, 70%, 55%)) 1" }}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
            <Zap size={20} className="text-primary" />
          </div>
          <div>
            <div className="font-semibold text-foreground">Smart Session Ready</div>
            <div className="text-sm text-muted-foreground">
              Recommended: {parts.join(" → ")}. Estimated {estimatedMinutes} minutes.
            </div>
          </div>
        </div>
        <Link
          to="/review"
          className="shrink-0 rounded-lg bg-primary px-5 py-2.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
        >
          Start Session
        </Link>
      </div>
    </div>
  );
}
```

**Step 4: Rewrite Dashboard page**

Rewrite `src/pages/Dashboard.tsx` to use the new components, include greeting with learner name, stats row, smart session card, and track cards grid.

**Step 5: Commit**

```bash
git add src/pages/Dashboard.tsx src/components/dashboard/
git commit -m "feat: redesign dashboard with glassmorphism, stats, smart session, track cards"
```

---

## Task 8: Settings Page -- OAuth + BYOK + Ollama [PARALLEL-GROUP-A]

**Files:**
- Rewrite: `src/pages/Settings.tsx`
- Create: `src/stores/useAuthStore.ts`

**Step 1: Create auth store**

```typescript
// src/stores/useAuthStore.ts
import { create } from "zustand";
import { getAuthStatus, loginProvider, logoutProvider } from "@/lib/tauri-commands";
import type { ProviderAuthStatus } from "@/types/ai";

interface AuthStore {
  providers: ProviderAuthStatus[];
  isLoading: boolean;
  loadAuthStatus: () => Promise<void>;
  login: (provider: string, method: string, credential?: string) => Promise<void>;
  logout: (provider: string) => Promise<void>;
}

export const useAuthStore = create<AuthStore>((set, get) => ({
  providers: [],
  isLoading: false,

  loadAuthStatus: async () => {
    set({ isLoading: true });
    try {
      const providers = await getAuthStatus();
      set({ providers });
    } finally {
      set({ isLoading: false });
    }
  },

  login: async (provider, method, credential) => {
    await loginProvider({ provider, method, credential });
    await get().loadAuthStatus();
  },

  logout: async (provider) => {
    await logoutProvider(provider);
    await get().loadAuthStatus();
  },
}));
```

**Step 2: Rewrite Settings page**

Rewrite `src/pages/Settings.tsx` with:
- Provider cards (Claude, OpenAI, Gemini) each with OAuth login button + connected status
- BYOK section with API key input field as alternative
- Ollama section with host URL configuration
- Theme toggle (dark/light)
- Learning preferences (daily goal, session duration)
- Save button wired to `updateAIConfig`
- Claude provider shows ToS disclaimer text before OAuth button

**Step 3: Commit**

```bash
git add src/pages/Settings.tsx src/stores/useAuthStore.ts
git commit -m "feat: settings page with OAuth login, BYOK, Ollama config, theme toggle"
```

---

## Task 9: Wire Onboarding Flow End-to-End

**Files:**
- Rewrite: `src/pages/Onboarding.tsx`
- Modify: `src/stores/useLearningStore.ts` (add assessment + path gen actions)
- Modify: `src/types/ai.ts` (update request/response types)

**Step 1: Update types**

Add to `src/types/ai.ts`:
```typescript
export interface AssessmentTurn {
  role: "assistant" | "user";
  content: string;
}
```

**Step 2: Add store actions**

Add to `useLearningStore`:
```typescript
assessKnowledge: async (topic: string, domain: string, messages: AssessmentTurn[]) => {
  const response = await tauriCommands.assessKnowledge({ topic, domain, messages });
  return response;
},
generatePath: async (trackId: string, topic: string, domain: string, goal: string, level: string, gaps: string[], strengths: string[]) => {
  return tauriCommands.generateLearningPath({ trackId, topic, domain, goal, assessmentLevel: level, assessmentGaps: gaps, assessmentStrengths: strengths });
},
```

**Step 3: Rewrite Onboarding.tsx**

4-step flow:
1. **Topic** -- text input + domain selector (existing, keep)
2. **Goals** -- goal textarea (existing, keep)
3. **Assessment** -- chat interface. Shows AI question, user types response, 3-5 turns. Parse final JSON response for assessment results.
4. **Generating** -- calls `createTrack()` then `generatePath()`, shows progress animation, navigates to `/track/:trackId` on completion.

The assessment step is a conversational chat UI:
```tsx
// Chat messages displayed in scrollable area
// User input at bottom
// "Continue to Generate" button appears after AI returns assessment_complete: true
```

**Step 4: Commit**

```bash
git add src/pages/Onboarding.tsx src/stores/useLearningStore.ts src/types/ai.ts
git commit -m "feat: wire onboarding flow end-to-end with AI assessment and path generation"
```

---

## Task 10: Module Content Rendering [PARALLEL-GROUP-B]

**Files:**
- Rewrite: `src/pages/ModuleView.tsx`
- Create: `src/components/learning/MarkdownRenderer.tsx`
- Create: `src/components/learning/CodeBlock.tsx`
- Create: `src/components/learning/TutorSidebar.tsx`

**Step 1: Create MarkdownRenderer**

```tsx
// src/components/learning/MarkdownRenderer.tsx
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import { CodeBlock } from "./CodeBlock";

interface MarkdownRendererProps {
  content: string;
}

export function MarkdownRenderer({ content }: MarkdownRendererProps) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeRaw]}
      components={{
        code({ node, className, children, ...props }) {
          const match = /language-(\w+)/.exec(className || "");
          const isInline = !match;
          if (isInline) {
            return (
              <code className="rounded bg-secondary px-1.5 py-0.5 text-sm font-mono" {...props}>
                {children}
              </code>
            );
          }
          return <CodeBlock language={match[1]} code={String(children).replace(/\n$/, "")} />;
        },
        // Style other elements...
        h1: ({ children }) => <h1 className="mb-4 mt-8 text-2xl font-bold text-foreground">{children}</h1>,
        h2: ({ children }) => <h2 className="mb-3 mt-6 text-xl font-semibold text-foreground">{children}</h2>,
        h3: ({ children }) => <h3 className="mb-2 mt-4 text-lg font-medium text-foreground">{children}</h3>,
        p: ({ children }) => <p className="mb-4 leading-7 text-foreground/90">{children}</p>,
        ul: ({ children }) => <ul className="mb-4 ml-6 list-disc space-y-1">{children}</ul>,
        ol: ({ children }) => <ol className="mb-4 ml-6 list-decimal space-y-1">{children}</ol>,
        li: ({ children }) => <li className="text-foreground/90">{children}</li>,
        blockquote: ({ children }) => (
          <blockquote className="mb-4 border-l-4 border-primary/50 pl-4 italic text-muted-foreground">
            {children}
          </blockquote>
        ),
        strong: ({ children }) => <strong className="font-semibold text-foreground">{children}</strong>,
      }}
    />
  );
}
```

**Step 2: Create CodeBlock with copy button**

```tsx
// src/components/learning/CodeBlock.tsx
import { useState } from "react";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import { Copy, Check } from "lucide-react";

interface CodeBlockProps {
  language: string;
  code: string;
}

export function CodeBlock({ language, code }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="group relative mb-4 rounded-lg overflow-hidden">
      <div className="flex items-center justify-between bg-secondary/50 px-4 py-2 text-xs">
        <span className="font-mono text-muted-foreground">{language}</span>
        <button
          onClick={handleCopy}
          className="flex items-center gap-1 text-muted-foreground hover:text-foreground"
        >
          {copied ? <Check size={14} /> : <Copy size={14} />}
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <SyntaxHighlighter
        language={language}
        style={oneDark}
        customStyle={{ margin: 0, borderRadius: 0 }}
      >
        {code}
      </SyntaxHighlighter>
    </div>
  );
}
```

**Step 3: Create TutorSidebar**

A slide-out panel with chat interface for asking the AI tutor questions about the current module.

**Step 4: Rewrite ModuleView.tsx**

- On mount: call `generateModuleContent()` if no cached content
- Show loading skeleton while generating
- Render content via `MarkdownRenderer`
- Show "Continue to Exercises" button at bottom
- Floating tutor button that opens `TutorSidebar`

**Step 5: Commit**

```bash
git add src/pages/ModuleView.tsx src/components/learning/
git commit -m "feat: module content rendering with markdown, syntax highlighting, AI tutor sidebar"
```

---

## Task 11: Exercise Components [PARALLEL-GROUP-B]

**Files:**
- Create: `src/components/exercises/ExerciseContainer.tsx`
- Create: `src/components/exercises/ConceptualQA.tsx`
- Create: `src/components/exercises/CodeChallenge.tsx`
- Create: `src/components/exercises/FillInBlank.tsx`
- Create: `src/components/exercises/ExerciseFeedback.tsx`
- Modify: `src-tauri/src/commands/ai.rs` (add generate_exercises + evaluate_exercise commands)
- Modify: `src/lib/tauri-commands.ts`

**Step 1: Add Rust commands for exercise generation and evaluation**

Add to `commands/ai.rs`:

```rust
#[tauri::command]
pub async fn generate_exercises(
    auth: State<'_, AuthState>,
    state: State<'_, AppState>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    // Generates 3-5 exercises for a module
    // Returns JSON array of exercises with type, prompt, hints
    // ...
}

#[tauri::command]
pub async fn evaluate_exercise(
    auth: State<'_, AuthState>,
    state: State<'_, AppState>,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    // AI evaluates learner's response
    // Returns score, feedback, misconceptions
    // Updates BKT mastery via learning::adaptive
    // ...
}
```

**Step 2: Create ExerciseContainer**

Routes to correct exercise component based on type. Shows progress (1/5, 2/5...). Shows feedback after submission.

**Step 3: Create ConceptualQA component**

Open-ended text area, submit for AI evaluation, show feedback.

**Step 4: Create CodeChallenge component**

Code editor (textarea with monospace font for Phase 1, Monaco later), language selector, submit for evaluation.

**Step 5: Create FillInBlank component**

Rendered text with inline input fields for blanks. Submit checks answers.

**Step 6: Register commands, add TS wrappers**

**Step 7: Commit**

```bash
git add src/components/exercises/ src-tauri/src/commands/ai.rs src/lib/tauri-commands.ts
git commit -m "feat: exercise components - Q&A, code challenges, fill-in-blank with AI evaluation"
```

---

## Task 12: Sidebar Redesign [PARALLEL-GROUP-A]

**Files:**
- Rewrite: `src/components/layout/Sidebar.tsx`
- Modify: `src/components/layout/AppLayout.tsx`

**Step 1: Redesign sidebar to match prototype**

- LearnForge logo at top
- Navigation section: Dashboard, Review (with due count badge), Analytics
- Learning Tracks section: list with inline progress bars and percentages
- "+ New Track" button at bottom
- Settings link at very bottom
- Theme toggle icon
- Collapsible with glassmorphism styling

**Step 2: Update AppLayout for new sidebar width and glass effects**

**Step 3: Commit**

```bash
git add src/components/layout/
git commit -m "feat: redesign sidebar with track progress, glassmorphism, theme toggle"
```

---

## Task 13: Track View with DAG Visualization [PARALLEL-GROUP-B]

**Files:**
- Rewrite: `src/pages/TrackView.tsx`
- Create: `src/components/learning/PathDAG.tsx`

**Step 1: Create PathDAG component**

A visual DAG of modules. For Phase 1, use a vertical flow layout with connector lines between prerequisites:

```tsx
// Render modules as cards in a vertical flow
// Draw SVG lines between connected modules (edges)
// Color-code by status: locked (gray), available (white), in_progress (blue), completed (green)
// Click available/in_progress modules to navigate to ModuleView
```

Use CSS + SVG for the connector lines. No external graph library needed for a simple vertical DAG.

**Step 2: Rewrite TrackView to use PathDAG**

**Step 3: Commit**

```bash
git add src/pages/TrackView.tsx src/components/learning/
git commit -m "feat: track view with visual DAG, module status indicators, prerequisite lines"
```

---

## Task 14: Topic Pack Skeletons [PARALLEL-GROUP-C]

**Files:**
- Modify: `topic-packs/kubernetes-fundamentals/pack.json` (expand)
- Create: `topic-packs/rust-from-zero/pack.json`
- Create: `topic-packs/go-essentials/pack.json`
- Create: `topic-packs/python-for-devops/pack.json`

**Step 1: Expand Kubernetes pack to 12-15 modules with proper DAG**

**Step 2: Create Rust pack (15-20 modules)**

**Step 3: Create Go pack (12-15 modules)**

**Step 4: Create Python pack (10-12 modules)**

Each pack follows the same JSON schema:
```json
{
  "id": "...",
  "title": "...",
  "description": "...",
  "domain_module": "programming|devops",
  "estimated_hours": 0,
  "modules": [
    {
      "id": "m1",
      "title": "...",
      "description": "...",
      "difficulty": 1,
      "estimated_minutes": 30,
      "objectives": ["..."]
    }
  ],
  "edges": [
    { "from": "m1", "to": "m2" }
  ]
}
```

**Step 5: Commit**

```bash
git add topic-packs/
git commit -m "feat: add 4 topic pack skeletons - Kubernetes, Rust, Go, Python"
```

---

## Task 15: DB Schema Updates for Content Caching

**Files:**
- Modify: `src-tauri/src/db/schema.rs` (add content column to modules, add exercises table columns)

**Step 1: Add content column to modules table**

In the CREATE TABLE statement for modules, add:
```sql
content TEXT,           -- cached AI-generated content (markdown)
content_generated_at TEXT  -- timestamp of generation
```

**Step 2: Verify migration runs cleanly**

Run: `cd src-tauri && cargo test`

**Step 3: Commit**

```bash
git add src-tauri/src/db/schema.rs
git commit -m "feat: add content caching columns to modules table"
```

---

## Task 16: Integration Testing -- Full Loop

**Files:**
- Test manually via `pnpm tauri dev`

**Step 1: Start the app**

Run: `pnpm tauri dev`

**Step 2: Test auth flow**

- Go to Settings
- Click OAuth login for one provider
- Verify browser opens, auth completes, status shows connected

**Step 3: Test onboarding flow**

- Click New Track
- Enter topic, select domain, set goal
- Complete AI assessment (3-5 conversational turns)
- Generate learning path
- Verify path persists to DB and track appears on dashboard

**Step 4: Test module content**

- Click into generated track
- Open first available module
- Verify AI generates content
- Verify markdown renders with code highlighting

**Step 5: Test exercises**

- Complete module, click Continue to Exercises
- Complete 3-5 exercises
- Verify feedback displays
- Verify next module unlocks

**Step 6: Fix any issues found**

**Step 7: Commit all fixes**

```bash
git add -A
git commit -m "fix: integration fixes from full loop testing"
```

---

## Parallel Execution Strategy (via ruflo)

```
PARALLEL-GROUP-A (no backend deps):
  Task 6: Design System + Theme
  Task 7: Dashboard Redesign
  Task 8: Settings Page
  Task 12: Sidebar Redesign

PARALLEL-GROUP-B (after Tasks 1-4 backend):
  Task 10: Module Content Rendering
  Task 11: Exercise Components
  Task 13: Track View with DAG

PARALLEL-GROUP-C (independent):
  Task 14: Topic Pack Skeletons
  Task 15: DB Schema Updates

Sequential dependencies:
  Task 1 → Task 2 → Task 3 → Task 4 (backend chain)
  Task 5 depends on Task 1
  Task 9 depends on Tasks 4 + 7 (needs backend + UI)
  Task 16 depends on everything
```

```
Timeline:
  ┌─ Task 1 ─→ Task 2 ─→ Task 3 ─→ Task 4 ─→ Task 9 ─→ Task 16
  │
  ├─ Task 6 ─┐
  ├─ Task 7 ─┤ (PARALLEL-GROUP-A, start immediately)
  ├─ Task 8 ─┤
  ├─ Task 12 ┘
  │
  ├─ Task 5 ──────────────────────────┐
  │                                    │
  ├─ Task 10 ─┐                       │ (PARALLEL-GROUP-B, after Task 4)
  ├─ Task 11 ─┤                       │
  ├─ Task 13 ─┘                       │
  │                                    │
  ├─ Task 14 ─┐ (PARALLEL-GROUP-C)    │
  └─ Task 15 ─┘                       │
                                       └─→ Task 16
```
