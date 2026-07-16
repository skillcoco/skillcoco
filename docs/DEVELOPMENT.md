# Development Workflow

**Status:** Source of truth for day-to-day development with Claude Code.
**Last updated:** 2026-06-19

Audience: maintainers and contributors using Claude Code as the
primary dev tool. See `CONTRIBUTING.md` for the standard contribution flow.

---

## Required tooling

Install before starting:

- **Rust toolchain** via `rustup` (provides `wasm32-unknown-unknown`
  target). Homebrew `rust` lacks the wasm32 sysroot — use rustup.
- **Node 20+** and `pnpm` (project uses pnpm-lock.yaml)
- **Tauri 2 CLI** — installed via `pnpm` automatically
- **`gh`** GitHub CLI (`brew install gh`)
- **`rtk`** Rust Token Killer (for token-efficient Bash through
  Claude — see `~/.claude/RTK.md`)
- **`fd` + `rg` + `jq` + `yq`** (per Claude shell tooling preferences)
- **macOS only:** Xcode Command Line Tools

Optional but recommended:
- `cargo-watch` for auto-recompile during Rust dev
- `wasm-pack` for local WASM testing (Phase 9+)
- `cargo-release` for crate publish ergonomics

---

## Repo bootstrap

```bash
# Clone
gh repo clone skillcoco/skillcoco ~/work/apps/skillcoco
cd ~/work/apps/skillcoco

# Install JS deps
pnpm install

# Verify Rust workspace builds
cargo check --workspace

# Verify desktop dev works (OSS)
pnpm tauri dev
```

---

## Branch + worktree model

See `docs/REPO-ARCHITECTURE.md` for details.

Quick reference:

```bash
# Default workflow — short-lived feature branch on main worktree:
git checkout main
git pull
git checkout -b feature/<name>
# ... work, commit, push, PR, merge ...

# Parallel feature workflow — sibling worktree:
git worktree add -b feature/<name> ../gsd-workspaces/<name>
cd ../gsd-workspaces/<name>
pnpm install     # one-time per worktree
```

Worktree convention: `/Users/gshah/gsd-workspaces/<name>/` for all
sibling worktrees.

One Claude Code session per worktree.

---

## GSD planning workflow

The project uses the GSD (Get Shit Done) framework for phased work.
Every non-trivial feature goes through:

```
/gsd:plan-phase <N>           # discuss → research → plan → verify
   ↓
/gsd:execute-phase <N>        # wave-by-wave subagent execution
   ↓
acceptance walkthrough        # human verifies the feature works
```

### Phase artifacts

For Phase `N`, the planner writes:

```
.planning/phases/<NN>-<slug>/
├── <NN>-CONTEXT.md           # locked implementation decisions
├── <NN>-DISCUSSION-LOG.md    # Q&A audit trail
├── <NN>-RESEARCH.md          # surfaces + risks + open questions
├── <NN>-NN-PLAN.md           # per-wave executable plans
├── <NN>-VERIFICATION-PLAN.md # plan-checker output
└── (after execution:)
├── <NN>-NN-SUMMARY.md        # per-wave execution summary
├── <NN>-VERIFICATION.md      # goal-backward verifier output
├── <NN>-REVIEW.md            # code-reviewer output
└── <NN>-ACCEPTANCE.md        # human walkthrough script
```

### Decision discipline

Every new phase MUST capture key implementation decisions in CONTEXT.md.
Phase 6 taught us: never skip the placement debate. When in doubt: OSS.

### Phase numbering

- Integer phases (`1`, `2`, `3`...) = planned milestone work
- Decimal phases (`03.1`, `03.2`, `06.5`...) = inserted phases
- `gsd:phase` skill manages adding/inserting/removing phases in ROADMAP

---

## Day-to-day Claude Code workflow

### Step 1 — Frame the task

Before opening Claude Code, decide:

- Is this a new phase? → `/gsd:plan-phase <N>` from scratch
- Is this gap-closure on an existing phase? → `/gsd:plan-phase <N> --gaps`
- Is this a one-off fix? → use `/gsd:fast` or work inline
- Is this a docs change? → edit directly, no GSD

### Step 2 — Branch + worktree

```bash
# Short-lived branch on main worktree:
git checkout -b feature/<descriptive-name>

# OR a parallel worktree if you'll run independent Claude sessions:
git worktree add -b feature/<name> ../gsd-workspaces/<name>
cd ../gsd-workspaces/<name>
```

### Step 3 — Run the GSD workflow

For substantial features:

```bash
# Within Claude Code session:
/gsd:plan-phase <N>           # discuss → research → plan → verify
/gsd:execute-phase <N>        # autonomous wave execution
```

For one-off fixes:

```bash
/gsd:fast <description>       # plans + executes inline, no subagents
```

### Step 4 — Verify locally

Before pushing:

```bash
cargo check --workspace
cargo test --workspace
pnpm exec tsc --noEmit
pnpm test --run
pnpm tauri dev               # smoke-test the binary
```

### Step 5 — Commit hygiene

- One logical change per commit (atomic commits)
- Use Conventional Commits style: `feat(scope): description`
- Phase work: `feat(NN-MM): description` where `NN-MM` = plan id
- License header in every new source file
- No emojis in source files (per `CLAUDE.md` no-emoji rule)
- No promotional language in CHANGELOG entries (slow-burn launch)
- Never `git commit --no-verify` unless explicitly asked

### Step 6 — PR + merge

```bash
git push -u origin feature/<name>
gh pr create --title "..." --body "..."
# After CI green + maintainer review:
gh pr merge --squash
```

---

## Contribution mechanics

Open to anyone. Standard CONTRIBUTING.md applies. CLA required (Phase
03.2 wired CLA Assistant Lite v2.6.1 via `.github/workflows/cla.yml`).

All contributions follow the same Conventional Commits style + PR
review process. License boundary is MIT for the entire repository.

---

## Local testing

```bash
# Dev app:
pnpm tauri dev

# Production build:
pnpm build && cargo tauri build
```

---

## When to spawn agents vs work inline

Claude Code workflow has two execution modes:

### Inline (no subagents)
Use when:
- Task is small (single file, single concern)
- You want to see the work as it happens
- Token budget is tight
- Iterating on a specific surface

Triggered by: working directly with Edit/Read/Bash tools in the main
conversation, or `/gsd:fast`, or `/gsd:execute-phase --interactive`.

### Subagent dispatch
Use when:
- Task spans many files / many waves
- Independent work can parallelize
- Context budget for the orchestrator matters
- Want a fresh context window for the actual work

Triggered by: `/gsd:execute-phase` default mode, `/gsd:plan-phase`
(spawns planner + plan-checker), `Agent` tool with specific subagent
type.

Cost trade-off: subagents are more token-efficient overall but each
spawn has a fixed startup cost. For trivial tasks, inline wins.

---

## RTK proxy usage (token efficiency)

The `rtk` CLI proxies common dev commands to save 60-90% tokens
on shell output. Use it for noisy commands:

```bash
# Standard usage (transparent):
git status                    # automatically rewritten by Claude Code hook
ls -la                        # same

# Explicit usage:
rtk proxy <command>           # bypass token filtering
rtk gain                      # show savings analytics
rtk discover                  # find missed opportunities
```

See `~/.claude/RTK.md` for the full reference. Bash commands in this
project should default to rtk-wrapped tools (`fd`, `rg`, `jq`, `yq`)
rather than `find`, `grep`, `cat`, `head`, `tail`.

---

## Test discipline

Per `CLAUDE.md`:
- TDD London School for new code (mock-first; write failing test first)
- Retroactive tests for existing code that gets touched
- 500-line file cap (split before hitting it; docs exempt)
- Run tests after EVERY code change before claiming "done"

Standard test commands:

```bash
# Rust workspace:
cargo test --workspace

# Specific crate:
cargo test -p skillcoco-core
cargo test -p skillcoco

# Specific module:
cargo test -p skillcoco --lib achievements

# Frontend (Vitest):
pnpm test --run
pnpm test --run src/path/to/specific.test.ts

# TypeScript check:
pnpm exec tsc --noEmit

# Doc tests:
cargo test --doc -p skillcoco-core
```

WASM tests (Phase 9+):

```bash
# Use rustup-managed cargo for wasm32 target:
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" \
  cargo build --target wasm32-unknown-unknown -p skillcoco-core --lib
```

Scoped to `--lib` since Phase 14 (14-RESEARCH Open Question 2, RESOLVED):
`skillcoco-core` now also declares the `forge-sign` `[[bin]]` (a
pack-signing CLI), which shares the crate's `[dependencies]` table but never
runs on wasm32. `--lib` narrows this gate to exactly the library surface the
app ships to wasm, making it immune to any future bin-only dependency while
still fully covering `pack_trust` and every other wasm-facing module.

---

## Acceptance walkthroughs

Major phases end with a human-only acceptance walkthrough script at
`.planning/phases/<NN>-*/<NN>-ACCEPTANCE.md`. These are NOT automated.

Pattern:
- 8-12 steps
- Manual reproduction of the user journey
- "What you do" / "Expected" / "Rationale" trios
- Sign-off form at the bottom
- Maintainer signs after completing the walkthrough

Outstanding acceptance walkthroughs (as of 2026-06-19):
- Phase 03.1 Labs
- Phase 04 Microlearning
- Phase 05 Topic Packs
- Phase 06 Certification
- Phase 07 Core Extraction
- Phase 08 Publishing (launch checkpoints C1-C6)

Run before final OSS launch.

---

## Common workflow recipes

### Adding a new feature (medium-size)

```bash
git checkout main && git pull
git checkout -b feature/<name>
# In Claude Code:
/gsd:plan-phase <N>          # if it's a roadmap phase
# OR plan inline if it's smaller
/gsd:execute-phase <N>
# Run local verification
cargo test --workspace && pnpm test --run
gh pr create --title "..." --body "..."
```

### Quick fix (small)

```bash
git checkout main && git pull
git checkout -b fix/<thing>
# In Claude Code:
/gsd:fast "<description>"
gh pr create --title "fix: ..." --body "..."
```

### Running an acceptance walkthrough

```bash
# Open the script:
$EDITOR .planning/phases/<NN>-*/<NN>-ACCEPTANCE.md
# Follow steps in a fresh terminal + fresh DB (delete ~/Library/Application Support/com.skillcoco.app/skillcoco.db if needed)
# After all steps pass, sign off in the doc + commit + open PR
```

### Publishing a new `skillcoco-core` release

```bash
# 1. bump version in skillcoco-core/Cargo.toml + CHANGELOG.md
# 2. atomic commit
# 3. tag:
git tag core-v<X.Y.Z>
git push origin core-v<X.Y.Z>
# CI triggers .github/workflows/core-publish.yml → cargo publish
```

### Publishing a new desktop release

```bash
# IF the release includes a skillcoco-core API change:
# 1. publish core-v<X.Y.Z> FIRST (per O-8 tag ordering rule)
# 2. wait for crates.io confirmation
# 3. THEN tag the desktop release:
git tag v<X.Y.Z>
git push origin v<X.Y.Z>
# CI triggers .github/workflows/release.yml → 3-OS matrix → GitHub Releases draft
# 4. maintainer reviews the draft + curates release notes per .github/release-notes-template.md
# 5. publish the draft
```

---

## Anti-patterns

Do NOT:

- Commit secrets, credentials, API keys, or `.env*` files
- Use `git commit --no-verify` to skip hooks (investigate the hook
  failure instead)
- Use `git rebase -i` or `git add -i` (interactive flags don't work
  with Claude Code Bash)
- Commit `target/`, `node_modules/`, `dist/`, or other generated
  output
- Push to `main` directly (always via PR)
- Use destructive git commands (`reset --hard`, `push --force`,
  `branch -D`) without an explicit user request
- Add emojis to source files, commits, PRs, or docs (per CLAUDE.md)
- Create new files at the repo root (use the right subdirectory)
- Create documentation files (`*.md`, `README*.md`) unless explicitly
  requested by the maintainer

---

## Reference cards

- `docs/REPO-ARCHITECTURE.md` — single repo + worktrees
- `docs/VERSIONING.md` — semver + tag ordering
- `docs/MACOS-SIGNING-SETUP.md` — Apple Developer enrollment
- `docs/CERT-PAYLOAD-V1.md` — Phase 6 signed-payload spec
- `CONTRIBUTING.md` — contribution rules
- `LICENSING.md` — license explainer
- `SECURITY.md` — vulnerability disclosure
- `CLAUDE.md` — Claude Code project preferences
- `~/.claude/RTK.md` — RTK token-efficient CLI
- `.planning/PROJECT.md` — project intent + key decisions
- `.planning/ROADMAP.md` — phase-by-phase roadmap
- `.planning/STATE.md` — current execution state

---

*See also: `docs/REPO-ARCHITECTURE.md`, `docs/VERSIONING.md`.*
