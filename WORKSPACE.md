# Workspace: studio-split

Created: 2026-06-03
Strategy: worktree

## Member Repos

| Repo | Source | Branch | Strategy |
|------|--------|--------|----------|
| learnforge | /Users/gshah/work/apps/learnforge | workspace/studio-split | worktree |

## Purpose

Isolated workspace for **Phase 3.2 — Open Core Split + Studio Foundation**.
Carves the LearnForge codebase into:
- Public `learnforge` repo (MIT, viral OSS core)
- Private `learnforge-studio` repo (proprietary commercial overlay)

Lets the split work proceed without blocking on Phase 03.1 acceptance
walkthrough still pending on `main`.

Strategy doc: `.planning/todos/pending/2026-06-03-open-core-split-and-studio-strategy.md`

## Next steps

```
cd /Users/gshah/gsd-workspaces/studio-split
# (already in workspace branch — no /gsd:new-project needed, .planning/ inherited)
/gsd:phase --insert 3.1 "Open Core Split + Studio Foundation"
/gsd:discuss-phase 3.2
/gsd:plan-phase 3.2
/gsd:execute-phase 3.2
```
