# LearnForge Phase 1 Design Document

**Date:** 2026-03-02
**Status:** Approved
**Approach:** Zeroclaw-First, End-to-End Loop

---

## 1. Goal

Build a working MVP where a user can: authenticate with their existing AI subscription (Claude/OpenAI/Gemini), start a learning track, go through an AI-driven assessment, receive a personalized learning path, study AI-generated module content, complete exercises, and track progress. Ship with 4 topic packs: Kubernetes, Rust, Go, Python.

## 2. Architecture

### 2.1 AI Provider Layer (Zeroclaw)

Zeroclaw embedded as a Rust crate dependency. No separate process.

**Authentication methods (priority order):**
1. OAuth for existing subscriptions (ChatGPT Plus/Pro, Claude Pro/Max, Gemini Advanced)
2. BYO API Key (any provider)
3. Ollama (local, free)
4. Any OpenAI-compatible endpoint

**OAuth flow:**
- Tauri opens user's browser for provider OAuth
- Zeroclaw's localhost TCP listener captures callback
- Tokens encrypted at rest (chacha20poly1305)
- Auto-refresh before expiry

**Claude OAuth disclaimer:** Users shown a ToS notice before authenticating via Claude subscription OAuth, per Anthropic's Feb 2026 ToS update.

**Integration pattern:**
```rust
// src-tauri/Cargo.toml
zeroclaw = { path = "../../agentix/upstream/zeroclaw", features = ["..."] }
```

Tauri commands call zeroclaw's `AuthService` + `Provider` trait directly.

### 2.2 Intelligence Layer (RuVector)

RuVector embedded as Rust crates alongside SQLite.

**Phase 1 scope:**
- `ruvector-core`: Store module/concept embeddings for cross-topic similarity
- `ruvector-graph`: Learning path DAG as a proper graph (replaces JSON-based DAG)
- `AgenticDB`: Learner trajectory tracking (state, action, reward)

**Phase 2 scope:**
- `sona`: Self-learning content optimization via trajectory analysis
- `ruvector-gnn`: GNN-based prerequisite detection and content re-ranking
- Cross-track knowledge transfer via semantic similarity

**Embeddings strategy:**
- Primary: Use zeroclaw's authenticated provider for embeddings
- Fallback: Local ONNX model (all-MiniLM-L6-v2, 384-dim) for offline/free usage
- Swappable via `EmbeddingProvider` trait

**SQLite + RuVector split:**
| SQLite | RuVector |
|--------|----------|
| Profiles, tracks, config | Concept embeddings |
| Module progress, attempts | Learning path graph (DAG) |
| SR cards, AI conversations | Cross-topic similarity |
| Exercise data, adaptation log | Learner trajectories |

### 2.3 Data Flow

```
SQLite (structured CRUD) ←→ Rust Backend ←→ RuVector (semantic intelligence)
                                ↕
                           Zeroclaw (AI providers)
                                ↕
                     Claude / OpenAI / Gemini / Ollama
```

## 3. End-to-End Learning Loop

### 3.1 Onboarding Flow

1. **Topic selection** -- User types what they want to learn
2. **Goals** -- Skill level target, time commitment, motivation
3. **Assessment** -- Conversational (3-5 AI turns), Socratic, gauges existing knowledge
4. **Path generation** -- AI creates module DAG (titles, prerequisites, difficulty, objectives only -- no content yet)
5. **Persist** -- Track + path + modules saved to SQLite, DAG stored in RuVector GraphDB

### 3.2 Progressive Content Generation

Content is NOT generated at path creation time. Each module's content is generated on-demand when the learner opens it.

**Why:** Enables true adaptation. Content for Module 5 is generated knowing exactly how the learner performed on Modules 1-4.

**Generation inputs:**
- Module title, objectives, difficulty (from DAG)
- Learner's current BKT mastery state
- Performance on previous modules (scores, time, struggles)
- Learning style preferences
- Topic pack hints (from pack.json)

**After exercises:**
- BKT mastery updates
- Engine decides next steps:
  - Mastery high: unlock next module(s) in DAG
  - Mastery low: generate reinforcement content
  - Gap detected: insert bridging module into DAG

**The DAG is a living plan** -- modules can be inserted, skipped, or reordered based on demonstrated understanding.

### 3.3 Exercise Types (Phase 1)

- **Conceptual Q&A** -- Open-ended, AI-evaluated for depth and accuracy
- **Code challenges** -- Write code, AI verifies correctness and quality
- **Fill-in-the-blank** -- Complete partial configs, commands, code snippets

3-5 exercises per module, mixed types based on domain. AI generates exercises and evaluates responses with structured feedback.

### 3.4 AI Prompting Strategy

All AI calls go through a single `ai_request()` function in Rust:
- Handles provider selection, token refresh, retry logic
- Validates response structure (JSON schema for structured outputs)
- Tracks token usage per conversation

**System prompts are domain-aware:**
- Assessment: "You are assessing a learner on {topic}. Use Socratic dialogue..."
- Content generation: "Generate a lesson for {module}. Learner level: {level}. Previous struggles: {gaps}..."
- Exercise evaluation: "Evaluate this response against the rubric. Return structured JSON..."

## 4. UI/UX Design

### 4.1 Visual Style

Modern minimal + glassmorphism. Dark mode default, light mode supported.

**Color palette:**
- Dark bg: `#1a1a2e` to `#16213e` gradient
- Light bg: `#f8fafc` to `#f1f5f9`
- Cards (dark): `rgba(255,255,255,0.05)` + `backdrop-blur-xl`
- Cards (light): `rgba(255,255,255,0.60)` + `backdrop-blur-xl`
- Primary accent: Warm orange/coral (`#f97316` to `#ef4444`)
- Track accents: Blue (K8s), Red (Rust), Cyan (Go), Yellow (Python)
- Text (dark): white primary, `#94a3b8` secondary
- Text (light): `#0f172a` primary, `#64748b` secondary

**Theme system:** CSS variables, toggle in sidebar, persisted in useAppStore.

### 4.2 Layout

- **Left sidebar** (fixed, collapsible): Navigation, track list with progress bars, theme toggle, settings
- **Main content area**: Dashboard, onboarding, track view, module view, exercises
- **Bottom bar**: Streak, due cards count, time today

### 4.3 Key Screens (reference: prototype screenshot)

**Dashboard:**
- Greeting with learner name
- Smart Session recommendation card (due reviews + next module suggestion)
- Stats row: Reviews Due, Modules Done, Best Streak, Active Tracks
- Track cards with colored top border, progress bar, 4 metrics (progress, reviews due, streak, ETA), next module preview
- "+ New Track" button

**Onboarding:**
- 4-step wizard: Topic, Goals, Assessment (chat interface), Generating (progress animation)

**Track View:**
- Visual DAG of modules with prerequisite lines
- Module states: locked, unlocked, in-progress, completed
- Click module to open

**Module View:**
- Markdown content (react-markdown + syntax highlighting)
- Code blocks with copy button
- AI tutor sidebar for questions
- "Continue to Exercises" at bottom

**Settings:**
- Provider selection with OAuth login buttons
- BYOK fields as alternative
- Ollama configuration
- Theme toggle
- Connected provider status indicators

## 5. Topic Packs

4 packs ship with Phase 1, each as a `pack.json` skeleton:

| Pack | Modules (est.) | Domain |
|------|---------------|--------|
| Kubernetes Fundamentals | 12-15 | DevOps |
| Rust from Zero | 15-20 | Programming |
| Go Essentials | 12-15 | Programming |
| Python for DevOps | 10-12 | DevOps + Programming |

**Pack format:**
```json
{
  "id": "kubernetes-fundamentals",
  "title": "Kubernetes Fundamentals",
  "domain_module": "devops",
  "modules": [
    { "id": "m1", "title": "...", "objectives": [...], "difficulty": 1 }
  ],
  "edges": [
    { "from": "m1", "to": "m2" }
  ]
}
```

AI generates all content at runtime, personalized per learner. Packs provide structure only.

## 6. Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| AI auth | Zeroclaw OAuth + BYOK + Ollama | No API key friction for subscription users |
| Vector DB | RuVector (embedded Rust crate) | Same language, no separate process, self-learning |
| Structured DB | SQLite (existing) | Already built, works for CRUD |
| Content timing | Generated on-demand per module | True adaptation to learner state |
| DAG storage | RuVector GraphDB | Native graph queries, Cypher support, semantic search |
| Theme | CSS variables, dark + light | Glassmorphism works in both modes |
| Exercises Phase 1 | Q&A, code, fill-in-blank | Covers theory + practice, AI-evaluable |

## 7. What's NOT in Phase 1

- Embedded terminal / labs (Phase 2)
- Spaced repetition review UI (Phase 2 -- SR algorithm is implemented, UI is not)
- SONA self-learning optimization (Phase 2)
- GNN prerequisite detection (Phase 2)
- Cloud sync (Phase 4)
- Marketplace (Phase 4)
- Analytics dashboard (Phase 3)

## 8. Dependencies

### Rust (new additions to Cargo.toml)
- `zeroclaw` (path dep) -- AI provider auth + API calls
- `async-trait` -- missing, needed for provider trait
- `ruvector-core` (path dep) -- vector storage + embeddings
- `ruvector-graph` (path dep) -- graph DB for DAG

### Frontend (already installed, unused)
- `react-markdown` + `remark-gfm` + `rehype-raw` -- module content rendering
- `react-syntax-highlighter` -- code blocks
- All shadcn/ui primitives -- UI components
