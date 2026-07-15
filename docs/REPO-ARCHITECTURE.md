# Repository Architecture

**Status:** Source of truth for repo structure, branching, and worktrees.
**Last updated:** 2026-07-08

## Top-level model

**One public GitHub repository — one MIT product.**

```
skillcoco/skillcoco   ← single public repo (MIT)
├── LICENSE              ← MIT
├── LICENSING.md         ← reader-facing license summary
├── README.md            ← public-facing project intro
├── CONTRIBUTING.md      ← contribution rules
├── SECURITY.md
├── CHANGELOG.md
├── docs/                ← project documentation
├── learnforge-core/     ← MIT — Rust crate, published to crates.io
├── src/                 ← MIT — OSS desktop React frontend
├── src-tauri/           ← MIT — OSS desktop Tauri 2 backend
├── topic-packs/         ← MIT — bundled pack content + JSON schema
├── scripts/
└── .github/
```

## License boundary

Single license file at the repo root:

- **`LICENSE`** — MIT. Governs every file in the repository.

Each source file SHOULD carry an SPDX license header:

```rust
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)
```

`LICENSING.md` at the repo root summarizes this for casual readers.

## Branch model

Single long-lived branch: **`main`**.

Feature work happens on short-lived `feature/<name>` branches that
merge to `main` via PR. No develop/release/staging branches.

```
main ────────────────────────────────────●─────●─────●─────●
                                          ↑     ↑     ↑     ↑
                                     PR merges from feature/*
```

Tag releases on `main`:
- `core-v{major}.{minor}.{patch}` — triggers crates.io publish
  (`.github/workflows/core-publish.yml`)
- `v{major}.{minor}.{patch}` — triggers desktop release
  (`.github/workflows/release.yml`)
- See `docs/VERSIONING.md` for tag ordering rules.

## Worktrees for parallel feature work

Git worktrees let you check out multiple branches simultaneously
without re-cloning. Use them for:

- **Parallel Claude Code sessions** working on independent branches
- **Long-lived feature exploration** that can't share working state
  with mainline
- **Side-by-side comparison** between an old and a new branch

### Current worktree layout

```
/Users/gshah/work/apps/learnforge/         ← main worktree (branch: main)
   └── .git/                               ← actual .git directory
       └── worktrees/
           └── agent-* (GSD executor worktrees, pruned after each wave)
```

### Adding a new worktree (parallel feature)

```bash
# From the main repo directory:
cd /Users/gshah/work/apps/learnforge

# Create a worktree on a new branch:
git worktree add -b feature/new-thing ../gsd-workspaces/new-thing

# cd into it and work:
cd ../gsd-workspaces/new-thing
pnpm install         # may need separate node_modules per worktree
pnpm tauri dev       # run the app
```

### Conventions

- Worktree directories live under `/Users/gshah/gsd-workspaces/<name>/`
- One Claude Code session per worktree (sessions cannot span worktrees)
- Same `.git` shared across all worktrees; same remotes
- Branch checked out in one worktree CANNOT be checked out in another
- Remove finished worktrees with `git worktree remove <path>`

### Cleaning stale worktrees

```bash
git worktree list                  # see all
git worktree prune                 # remove dead entries
git worktree remove <path>         # remove specific
git branch -D <stale-branch>       # delete the orphan branch
```

## Build

| Command | Binary | Description |
|---------|--------|-------------|
| `pnpm tauri dev` | `learnforge` | Dev build — hot-reload frontend + Rust recompile |
| `pnpm build` | OSS bundle | Production frontend static output |
| `cargo tauri build` | macOS .dmg / Win .msi / Linux .AppImage | Signed release binary |

## Releasing

Two artifacts ship from this repo:

1. **`learnforge-core` Rust crate** to crates.io
   - Tag: `core-v{major}.{minor}.{patch}`
   - Triggers: `.github/workflows/core-publish.yml`
   - Distribution: crates.io + docs.rs

2. **Desktop binaries** to GitHub Releases
   - Tag: `v{major}.{minor}.{patch}`
   - Triggers: `.github/workflows/release.yml`
   - Distribution: GitHub Releases (signed macOS .dmg, Windows .msi,
     Linux .AppImage/.deb)

Tag ordering rule (locked, Phase 8 O-8): when a desktop release
includes a core API change, `core-v*` MUST be pushed + crates.io
publish succeed BEFORE `v*`. See `docs/VERSIONING.md`.

---

*See also: `docs/DEVELOPMENT.md` (Claude Code workflow),
`docs/VERSIONING.md` (semver + tag ordering rules).*
