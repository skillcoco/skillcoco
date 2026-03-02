# LearnForge

AI-powered adaptive learning desktop application. Built with Tauri 2 (Rust) + React + TypeScript.

## Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [pnpm](https://pnpm.io/) 8+
- [Rust](https://rustup.rs/) (latest stable)
- [Tauri CLI prerequisites](https://v2.tauri.app/start/prerequisites/)

## Setup

```bash
# Clone the repo
git clone <repo-url>
cd learnforge

# Install frontend dependencies
pnpm install

# Run in development mode
pnpm tauri dev
```

## Project Structure

See `CLAUDE.md` for detailed architecture documentation.

```
learnforge/
├── CLAUDE.md              # Claude Code project instructions
├── docs/                  # Product spec and documentation
├── src/                   # React frontend
│   ├── components/        # UI components
│   ├── pages/             # Page-level components
│   ├── stores/            # Zustand state stores
│   ├── types/             # TypeScript type definitions
│   ├── hooks/             # Custom React hooks
│   └── lib/               # Utilities + Tauri command wrappers
├── src-tauri/             # Rust backend
│   └── src/
│       ├── ai/            # AI provider abstraction
│       ├── commands/       # Tauri IPC handlers
│       ├── db/            # SQLite database layer
│       ├── labs/          # Docker lab management
│       └── learning/      # Adaptive engine + spaced repetition
└── topic-packs/           # Learning content packages
```

## Development with Claude Code

This project includes a `CLAUDE.md` file optimized for Claude Code. Open the project in Claude Code and it will understand the full architecture, conventions, and current development phase.

```bash
# Open with Claude Code
claude code .
```

## License

Proprietary - School of DevOps & AI
