# Repository Architecture

**Status:** Source of truth for repo structure, branching, and worktrees.
**Last updated:** 2026-06-19

## Top-level model

**One public GitHub repository hosts both products.**

```
agentixgarage/learnforge   ← single public repo
├── LICENSE              ← MIT (governs top-level files except pro/)
├── LICENSE-STUDIO       ← Proprietary (governs pro/ subtree)
├── LICENSING.md         ← reader-facing summary of which file = which license
├── README.md            ← public-facing project intro
├── CONTRIBUTING.md      ← OSS contribution rules
├── CONTRIBUTING-STUDIO.md ← Studio-side contribution rules
├── SECURITY.md
├── CHANGELOG.md
├── docs/                ← MIT (except docs/studio-* → LICENSE-STUDIO)
├── learnforge-core/     ← MIT — Rust crate, published to crates.io
├── src/                 ← MIT — OSS desktop React frontend
├── src-tauri/           ← MIT — OSS desktop Tauri 2 backend
├── topic-packs/         ← MIT — bundled pack content + JSON schema
├── pro/                 ← LICENSE-STUDIO — Studio overlay
│   ├── src-tauri-pro/   ← Studio Tauri 2 binary + licensing crate
│   └── src/             ← Studio-side React components (@pro alias)
├── scripts/
└── .github/
```

## Why one repo (not two)

Previous design split this into two repos with a leak-guard CI. We
consolidated in 2026-06-19 for the following reasons:

- **Industry pattern:** GitLab CE/EE, PostHog, Cal.com, Supabase,
  Mattermost all use single-repo + mixed-license. Their viral OSS
  adoption proves the model works.
- **Simpler mental model:** one git remote, one branch, no sync
  workflow, no leak-guard CI.
- **Audit-friendly:** Pro source is visible. Companies trust source
  they can read.
- **Contributor-friendly:** OSS contributors see the full system.
- **Licensing is enforced by file headers + RUNTIME license check**
  (Studio binary requires a signed license key — Phase 14), NOT by
  source visibility.

If you came from the two-repo era, mental model upgrades:

| Old (2-repo)                          | New (1-repo)                |
|---------------------------------------|-----------------------------|
| `learnforge` + `learnforge-studio`    | `learnforge` only           |
| OSS commits cherry-picked to upstream | No sync                     |
| `upstream` remote points to OSS       | Single `origin`, no upstream|
| `check-no-pro-leak.sh` CI             | Removed                     |
| Worktree carries both                 | Same; just one origin       |
| LICENSE-STUDIO in Studio repo only    | LICENSE-STUDIO at OSS root  |

The `agentixgarage/learnforge-studio` repo has been archived
(2026-06-19) with a README redirect to `agentixgarage/learnforge`.

## License boundary

Two LICENSE files at the repo root:

- **`LICENSE`** — MIT. Governs every file at the repo root and every
  file under `learnforge-core/`, `src/`, `src-tauri/`, `topic-packs/`,
  `scripts/`, `.github/`, and `docs/` (except `docs/studio-*`).
- **`LICENSE-STUDIO`** — Proprietary. Governs every file under `pro/`
  AND files matching `docs/studio-*` AND `build.config.ts`.

Each source file SHOULD carry a license header identifying which
applies. Files lacking headers default to the LICENSE that governs
their directory.

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

`workspace/studio-split` was the transitional branch during Phase 8
launch prep. After repo consolidation, work moves to `main`.

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
           ├── studio-split                ← worktree state for workspace/studio-split
           └── agent-* (× 3, locked)       ← stale Phase 03.2 agent worktrees, to prune

/Users/gshah/gsd-workspaces/studio-split/  ← your worktree (branch: workspace/studio-split)
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
pnpm tauri dev       # run OSS app
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

The 3 `worktree-agent-*` entries from Phase 03.2 agent work are
stale-and-locked; clean them with:

```bash
# Unlock + remove + delete branch (one-time cleanup):
for w in agent-a0d18881e5c313938 agent-a2c0f3ae8fe78a9d5 agent-a4388690e8c0f23f4; do
  git worktree unlock /Users/gshah/work/apps/learnforge/.claude/worktrees/$w 2>/dev/null
  git worktree remove --force /Users/gshah/work/apps/learnforge/.claude/worktrees/$w
  git branch -D worktree-$w
done
git worktree prune
```

## Build flag matrix

| Command | Binary | Frontend | Backend modules |
|---------|--------|----------|-----------------|
| `pnpm tauri dev` | `learnforge` (OSS) | `src/` only | `src-tauri/` only; `@pro` resolves to no-op stubs |
| `LEARNFORGE_PRO=1 pnpm tauri dev --config pro/src-tauri-pro/tauri.conf.json` | `learnforge-studio` | `src/` + `pro/src/` via `@pro` alias | `src-tauri/` + `pro/src-tauri-pro/`; Studio binary's `StudioPlugin impl LearnForgePlugin` registers additional handlers |
| `pnpm build` | OSS bundle | dist/ | static OSS frontend |
| `LEARNFORGE_PRO=1 pnpm build` | Studio bundle | dist/ with pro/ overrides | static Studio frontend |

`build.config.ts` (proprietary, in LICENSE-STUDIO subtree) carries the
Studio overlay path constants. `vite.config.ts` (MIT) reads
`LEARNFORGE_PRO` env var and swaps the `@pro` alias accordingly.

## OSS app vs Studio app — what's different at runtime

Phase 03.2 + the open-core architecture mean both apps share 95% of
code. Differences exist only in:

1. **Binary identity** — productName, identifier, signing identity
2. **Plugin registration** — `LearnForgePlugin` trait lets Studio
   register additional IPC commands without modifying OSS code
3. **License gating** — Studio binary checks a signed license key at
   startup (Phase 14 implements; current scaffold lets it boot)
4. **Pro-side React components** — `@pro` alias swaps in
   Studio-specific UI (e.g. Settings Verify Certificate panel)
5. **Pro IPC handlers** — additional `#[tauri::command]` functions
   registered only when Studio binary boots

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
   - **OSS binary only** in current pipeline. Studio binary ships
     separately via direct sales in Phase 14+ (no public Studio
     release artifacts).

Tag ordering rule (locked, Phase 8 O-8): when a desktop release
includes a core API change, `core-v*` MUST be pushed + crates.io
publish succeed BEFORE `v*`. See `docs/VERSIONING.md`.

## How this differs from prior phases

Phase 03.2 originally created two GitHub repos with `check-no-pro-leak.sh`
guarding accidental commits across the boundary. The two-repo model
worked but added cognitive overhead:

- "Which repo am I committing to?"
- "Did this OSS commit propagate to upstream?"
- "Why is CI yelling about a mixed commit?"

The 2026-06-19 simplification to one repo eliminated all three.
LICENSE boundary is now per-DIRECTORY (everything outside `pro/` is
MIT) and the leak guard is gone.

If you find references to:
- `scripts/check-no-pro-leak.sh` — stale; deleted in repo consolidation
- "upstream remote" — stale; only `origin` exists now
- "OSS repo at agentixgarage/learnforge" — same repo as ever; no
  longer "the OSS subset"
- "Studio repo at agentixgarage/learnforge-studio" — archived; redirect
  to `agentixgarage/learnforge`

…please update them and submit a PR.

---

*See also: `docs/OSS-VS-STUDIO.md` (feature placement), `docs/DEVELOPMENT.md`
(Claude Code workflow), `docs/CONTRIBUTING-STUDIO.md` (pro/ contribution rules).*
