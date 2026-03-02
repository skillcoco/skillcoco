# LearnForge - Claude Code Project Instructions

## Project Overview

LearnForge is an AI-powered adaptive learning desktop application built with **Tauri 2 (Rust) + React + TypeScript**. It creates personalized learning paths for technical topics, provides hands-on lab environments, and uses spaced repetition for long-term retention.

**Read the full product spec:** `docs/learnforge-product-spec.docx`

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop Runtime | Tauri 2.x (Rust) |
| Frontend | React 18 + TypeScript + Vite |
| UI | shadcn/ui + Tailwind CSS |
| State Management | Zustand |
| Backend Logic | Rust (Tauri IPC commands) |
| Local Database | SQLite via `rusqlite` (Rust-side) |
| AI Integration | Multi-provider: Claude API, OpenAI API, Ollama (local) |
| Lab Environments | Docker containers (managed via Rust backend) |
| Package Manager | pnpm |

## Architecture

### Three-Layer Learning Architecture

1. **Core Engine** (domain-agnostic): Adaptive algorithm, progress tracking, SM-2 spaced repetition, assessment engine, AI orchestration
2. **Domain Modules**: Specialized content types — "Programming" (code execution), "DevOps" (terminal + containers), "Concepts" (theory)
3. **Topic Packs**: JSON-based curricula that plug into domain modules (e.g., Kubernetes Fundamentals, Rust from Zero)

### Code Architecture

```
src-tauri/src/           # Rust backend
├── main.rs              # Tauri app entry point
├── lib.rs               # Module registration + app setup
├── db/                  # SQLite database layer
│   ├── mod.rs           # DB connection pool, migrations
│   ├── schema.rs        # Table definitions, queries
│   └── models.rs        # Rust structs for DB entities
├── ai/                  # AI provider abstraction
│   ├── mod.rs           # Provider trait + factory
│   ├── provider.rs      # AIProvider trait definition
│   ├── claude.rs        # Anthropic Claude implementation
│   ├── openai.rs        # OpenAI implementation
│   └── ollama.rs        # Ollama (local) implementation
├── learning/            # Core learning engine
│   ├── mod.rs
│   ├── adaptive.rs      # BKT adaptive algorithm
│   ├── path.rs          # Learning path DAG generation
│   └── spaced_repetition.rs  # SM-2 algorithm
├── labs/                # Lab environment management
│   └── docker.rs        # Docker container lifecycle
└── commands/            # Tauri IPC command handlers
    ├── mod.rs
    ├── tracks.rs        # Learning track CRUD
    ├── learning.rs      # Module progress, exercises
    └── ai.rs            # AI tutor interactions

src/                     # React frontend
├── main.tsx             # React entry point
├── App.tsx              # Root component + routing
├── types/               # TypeScript type definitions
├── stores/              # Zustand state stores
├── components/          # React components
│   ├── layout/          # App shell, sidebar, nav
│   ├── dashboard/       # Main dashboard
│   ├── learning/        # Module content rendering
│   ├── exercises/       # Exercise components
│   ├── labs/            # Terminal, editor, lab UI
│   ├── review/          # Spaced repetition review
│   ├── onboarding/      # New track onboarding flow
│   └── common/          # Shared UI components
├── hooks/               # Custom React hooks
├── lib/                 # Utilities + Tauri command wrappers
└── pages/               # Top-level page components
```

## Key Design Decisions

- **Local-first**: All data stored in SQLite on the user's machine. Cloud sync is optional (Phase 4).
- **AI provider abstraction**: The `AIProvider` trait in Rust defines the interface. All AI calls go through this abstraction so providers can be swapped transparently.
- **Adaptive engine**: Uses Bayesian Knowledge Tracing (BKT) with SM-2 spaced repetition. The engine runs in Rust for performance.
- **Learning paths are DAGs**: Modules have prerequisites forming a directed acyclic graph. The frontend renders this as a visual path.
- **Topic packs are data**: JSON-based packages that define curricula. They're loaded at runtime and can be AI-generated or hand-authored.

## Development Commands

```bash
# Install dependencies
pnpm install

# Run in development (Vite + Tauri)
pnpm tauri dev

# Build for production
pnpm tauri build

# Rust tests
cd src-tauri && cargo test

# Frontend tests
pnpm test

# Lint
pnpm lint
cargo clippy
```

## Database

SQLite stored at Tauri app data dir. Schema defined in `src-tauri/src/db/schema.rs`. Migrations run on startup.

**Core tables**: learner_profiles, learning_tracks, learning_paths, modules, module_progress, exercises, exercise_attempts, sr_cards, ai_conversations, adaptation_events

## AI Provider Configuration

Users configure AI in Settings. Config stored in SQLite:

```rust
enum AIProviderType { Claude, OpenAI, Ollama, Custom }
```

## Coding Conventions

- **Rust**: `thiserror` for errors, `serde` for serialization, `tokio` for async. Tauri commands return `Result<T, String>`.
- **TypeScript**: Strict mode. `interface` over `type` for objects. Named exports.
- **Components**: Functional + hooks. shadcn/ui primitives. Tailwind only — no CSS files.
- **State**: Zustand stores. Tauri IPC calls live in hooks, not components.
- **Naming**: Rust snake_case. TS camelCase for vars, PascalCase for components/types.
- **Errors**: Always handle Tauri invoke errors. Show user-friendly toasts.

## Current Phase: Phase 1 (Foundation)

**Build now:**
- [x] Project scaffold and architecture
- [ ] Database schema + auto-migrations
- [ ] AI provider abstraction (start with Claude)
- [ ] Onboarding flow (topic → assessment → path generation)
- [ ] Learning path generation + DAG visualization
- [ ] Module content rendering (markdown + code blocks)
- [ ] Basic exercises (Q&A, code challenges, fill-in-blank)
- [ ] Progress tracking
- [ ] Initial topic pack: Kubernetes Fundamentals

**Do NOT build yet:**
- Embedded terminal / labs (Phase 2)
- Spaced repetition system (Phase 2)
- Ollama integration (Phase 3)
- Cloud sync (Phase 4)
- Marketplace (Phase 4)

## Environment Variables

`.env` in project root (gitignored):

```env
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...
OLLAMA_HOST=http://localhost:11434
```

## Testing Strategy

- Rust: unit tests for learning algorithm, DB ops, AI abstraction
- React: Vitest + React Testing Library
- Integration: Tauri IPC round-trips
- E2E: Phase 2+

## File Naming

- Rust: `snake_case.rs`
- Components: `PascalCase.tsx`
- Hooks: `useCamelCase.ts`
- Types: `camelCase.ts`
- Stores: `useCamelCaseStore.ts`
