# Contributing to SkillCoco

Thank you for your interest in contributing to SkillCoco! This document covers
everything you need to get started: the Contributor License Agreement, dev
environment setup, coding standards, and the pull-request workflow.

---

## Contributor License Agreement (CLA)

SkillCoco is developed under an open-core model. To protect both contributors
and the project, **all contributors must sign the CLA before their first pull
request can be merged.**

Read the full [Contributor License Agreement](./CLA.md) before signing. Key
points:

- You grant the maintainers a perpetual, worldwide copyright and patent license
  over your contributions.
- Section 5 grants the maintainers the right to incorporate your contributions
  into commercial products (SkillCoco Pro, SkillCoco Hub, SkillCoco Studio,
  or successors) under any license. The public MIT-licensed copy is unaffected.

### How to sign

When you open your first pull request, CLA Assistant Lite will post a comment
asking you to sign. Reply with **exactly** this phrase as a new comment:

```
I have read the CLA Document and I hereby sign the CLA
```

Signing is recorded in `signatures/version1/cla.json` and applies to all
future contributions. You only sign once.

---

## Development environment setup

### Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Node.js | 18+ | LTS recommended |
| pnpm | 8+ | `npm install -g pnpm` |
| Rust | stable | `rustup default stable` |
| Tauri CLI | 2.x | installed via `pnpm tauri` |
| Docker | any | optional; labs fall back to host shell |

### Clone and install

```bash
git clone https://github.com/skillcoco/skillcoco.git
cd skillcoco
pnpm install
```

### Run in development mode

```bash
pnpm tauri dev
```

Tauri starts the Vite dev server and the Rust backend simultaneously.
Hot reload is active for the frontend; Rust changes trigger a full recompile.

### Build for production

```bash
pnpm tauri build
```

---

## Running tests

### Frontend (TypeScript / React)

```bash
pnpm test
```

Uses Vitest. All tests run without a real browser.

### Backend (Rust)

```bash
cargo test -p skillcoco
```

Or from the `src-tauri` subdirectory:

```bash
cd src-tauri && cargo test
```

Unit and integration tests use trait-based mocks (`LabRuntime`, `DockerProbe`,
`AIClientTrait`) — **zero real Docker, zero real PTY, zero real LLM** required
to run the test suite.

### Lint

```bash
pnpm run lint
```

---

## Test-driven development (TDD)

SkillCoco uses **TDD London School** (mock-first):

1. **RED** — write one failing test that describes the desired behaviour. Run it,
   verify it fails for the right reason (assertion failure, not compile error).
2. **GREEN** — write the minimum code required to make it pass.
3. **REFACTOR** — clean up without adding behaviour. Keep all tests green.

Production code written before a failing test exists **will be asked to be
deleted and re-implemented**. This is not negotiable.

---

## Code style

### TypeScript / React

- Use TypeScript strict mode (`strict: true` in `tsconfig.json`).
- Prefer functional components and hooks over class components.
- All public APIs must have typed interfaces — no `any` without justification.
- Run `pnpm run lint` before committing; CI will catch violations.

### Rust

- Run `cargo fmt` and `cargo clippy -- -D warnings` before committing.
- Use `thiserror` for library errors, `anyhow` for application-level errors.
- Keep files under 500 lines; extract modules when they grow larger.

---

## SPDX license headers

Add an SPDX identifier comment at the top of every **new source file** you
create:

**TypeScript / JavaScript:**
```typescript
// SPDX-License-Identifier: MIT
```

**Rust:**
```rust
// SPDX-License-Identifier: MIT
```

Do **not** add SPDX headers to Markdown documentation, YAML config files,
JSON files, or legal documents (like `CLA.md` or `LICENSE`).

---

## Commit messages

Follow the Conventional Commits format:

```
<type>(<scope>): <short summary>
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`, `ci`

Examples:
```
feat(labs): add ai_judge step evaluator with budget guard
fix(bkt): correct posterior update when slip > guess
docs(contributing): add SPDX header instructions
```

Keep the summary line under 72 characters. Add a body paragraph for any
non-obvious reasoning.

---

## Pull request workflow

1. Fork the repository and create a branch from `main`.
2. Make your changes following the TDD and code-style rules above.
3. Run `pnpm test` and `cargo test -p skillcoco` — both must pass before you
   open a PR.
4. Open a pull request with a clear description of *why* the change is needed,
   not just what it does.
5. **Sign the CLA** if this is your first contribution (see above).
6. Address review feedback. Maintainers may ask for test coverage, refactoring,
   or documentation.

### Review expectations

- Maintainers aim to review PRs within 5 business days.
- "Request changes" feedback must be addressed before merge.
- Breaking API changes require discussion in an issue before a PR is opened.
- Large features should be discussed in an issue first to avoid wasted effort.

---

## Open core — OSS vs the commercial platform

SkillCoco follows an **open-core model**:

| What | Where | License |
|------|-------|---------|
| Adaptive engine (BKT, SM-2, microlearning), open pack format + import, lessons/video/quizzes, gamification, AI tutor (BYOK/local), local self-signed completion badge, and terminal labs (use + build) | **This repo** | MIT |
| Richer integrated learning environment (IDE, interactive simulators, exam simulator) and other paid-tier features | SkillCoco Pro (private) | Commercial |
| Course licensing, verifiable certificates, progress sync, cohort/manager reporting, multi-tenant web | SkillCoco Hub (private) | Commercial |
| Course authoring + AI enrichment for educators | SkillCoco Studio (private) | Commercial |

The "OSS-lite" open-core split is complete (v2.0.0): the commercial-tier
features — skill reports, exam simulator, entitlements/redeem, and the
client-side certificate trust chain — were removed from this repository and
continue in the private SkillCoco Pro fork. Terminal labs stay here as a
first-class OSS feature. Contributions to this repository improve the
open-source desktop product; commercial-product features are built in their
own repositories by the core team.

If you are unsure whether your contribution is in scope, open an issue and ask
before investing time in implementation.

---

## Getting help

- **Bug reports / feature requests:** [GitHub Issues](https://github.com/skillcoco/skillcoco/issues)
- **Questions:** Start a [GitHub Discussion](https://github.com/skillcoco/skillcoco/discussions)
- **Security vulnerabilities:** Email `bean@initcron.org` — do not open a public issue

---

*An Agentix Garage and School of DevOps / School of AI project. Built in the open.*
