# Contributing to LearnForge Studio (`pro/` subtree)

**Status:** Source of truth for contributions to the proprietary
LearnForge Studio code under `pro/`.
**Last updated:** 2026-06-19

This document governs contributions to **LearnForge Studio** —
specifically, any file under `pro/`, files matching `docs/studio-*`,
and `build.config.ts`. These files are NOT MIT-licensed; they are
covered by the proprietary `LICENSE-STUDIO` at the repo root.

If your contribution touches ONLY MIT-licensed files (everything
outside `pro/`), see `CONTRIBUTING.md` instead. This document does
NOT apply to OSS contributions.

---

## Who can contribute to `pro/`

Contributions to the proprietary Studio subtree are restricted:

- **Maintainers** — the founders of LearnForge Studio (Gourav Shah,
  Vivian Aranha)
- **Employees + contractors** of LearnForge Studio (the legal entity
  operating the commercial product)
- **Specifically-invited external contributors** who have signed a
  separate proprietary contributor agreement

**External pull requests touching `pro/` are auto-closed.** Open a PR
against MIT files only, or contact the maintainers privately to
discuss a Studio-side change.

This is enforced by maintainer review, not by automation (current
state). A future Phase may add a GitHub Action that auto-flags
PRs touching `pro/` from non-team members.

---

## Why this exists

Despite the open-source-first stance (see `docs/OSS-VS-STUDIO.md`
Decision #0), Studio source code has commercial value:

- Audit + security review benefit from public visibility
- Customers + procurement teams want to read what they're licensing
- Open-core trust depends on "we're not hiding anything"

Public read access does NOT imply contribution rights. The license
boundary is enforced at TWO levels:

1. **LICENSE-STUDIO** legal terms — you may read + audit, but may
   not use commercially without a license key, nor modify + ship as
   a competing product
2. **Maintainer process** — only authorized contributors land
   `pro/`-side commits

---

## License: LICENSE-STUDIO summary

Full terms at the repo root in `LICENSE-STUDIO`. Highlights:

- **Read + audit:** allowed
- **Personal use:** allowed (with a valid license key)
- **Commercial use:** allowed only with a current commercial license
  key issued by LearnForge Studio
- **Fork + modify for non-commercial use:** allowed
- **Fork + ship a competing commercial product:** PROHIBITED
- **Contribute back:** requires accepting the proprietary contributor
  agreement (CLA-equivalent for the proprietary subtree)
- **License keys:** Ed25519-signed; tamper attempts violate the
  agreement and may constitute computer-fraud under applicable laws

If LICENSE-STUDIO and this CONTRIBUTING-STUDIO disagree, LICENSE-STUDIO
wins.

---

## Studio Contributor Agreement (SCA)

The SCA is the proprietary counterpart to the OSS CLA. It assigns +
licenses contribution copyright to LearnForge Studio, the legal
entity.

External contributors who become authorized to touch `pro/` MUST
sign the SCA before their first merge. The SCA is administered out
of band (email to `hello@learnforge.dev`).

Note: the OSS CLA (signed via `.github/workflows/cla.yml`) does NOT
cover `pro/` contributions. They are separate agreements.

---

## How to contribute to Studio (authorized contributors only)

If you have been granted Studio contributor rights, follow the
process below.

### Branch + worktree

Same as the OSS workflow (see `docs/DEVELOPMENT.md`):

```bash
git checkout main && git pull
git checkout -b feature/pro-<name>
```

Or use a sibling worktree for parallel work:

```bash
git worktree add -b feature/pro-<name> ../gsd-workspaces/pro-<name>
cd ../gsd-workspaces/pro-<name>
```

### What you can modify

- Anything under `pro/`
- Files matching `docs/studio-*`
- `build.config.ts`
- LICENSE-STUDIO itself (requires founder sign-off)

### What you CANNOT modify in a Studio-only PR

- Files governed by MIT LICENSE
- Public-facing docs (README.md, CONTRIBUTING.md, etc.)
- `.github/workflows/` (governance applies to both products)
- `learnforge-core/` (it's MIT and publicly published)

If your Studio change REQUIRES an OSS-side change, raise TWO separate
PRs — one OSS, one Studio — and link them.

### Commit hygiene

Same as OSS standard (see `docs/DEVELOPMENT.md`):

- Conventional Commits style: `feat(pro): description`
- Phase work: `feat(pro-NN-MM): description` where `NN-MM` = plan id
- One logical change per commit
- LICENSE-STUDIO header in every new file under `pro/` (see template
  below)
- No emojis
- No `git commit --no-verify`
- No promotional language

### License header template

Every new source file under `pro/` MUST include this header (adjust
syntax per language):

```rust
// LICENSE-STUDIO — Proprietary
// Copyright (c) 2026 LearnForge Studio
// All rights reserved. See LICENSE-STUDIO at repository root.
```

```ts
// LICENSE-STUDIO — Proprietary
// Copyright (c) 2026 LearnForge Studio
// All rights reserved. See LICENSE-STUDIO at repository root.
```

```toml
# LICENSE-STUDIO — Proprietary
# Copyright (c) 2026 LearnForge Studio
# All rights reserved. See LICENSE-STUDIO at repository root.
```

### PR review

Pro-side PRs go through a maintainer-only review channel:

- Tag the PR with the `studio` label
- Request review from a maintainer (Gourav Shah, Vivian Aranha)
- Public visibility on GitHub is acceptable (the source is already
  public); proprietary review happens via private comments where
  needed

### Merge

After approval:

```bash
gh pr merge --squash
```

CI gates same as OSS PRs. No special leak-guard required (removed in
the 2026-06-19 repo consolidation).

---

## Pro-side architecture recap

See `docs/REPO-ARCHITECTURE.md` for the full picture. Quick summary
for Studio contributors:

```
pro/
├── src-tauri-pro/        ← Studio Tauri 2 binary (separate from OSS src-tauri/)
│   ├── Cargo.toml        ← package = "learnforge-studio"
│   ├── tauri.conf.json   ← "LearnForge Studio" product name
│   ├── build.rs
│   ├── Entitlements.plist (future — Phase 14)
│   ├── icons/
│   ├── src/
│   │   └── main.rs       ← Studio binary entry; StudioPlugin impl LearnForgePlugin
│   └── licensing/        ← License-key validation crate (Phase 14 implements)
│       ├── Cargo.toml
│       └── src/lib.rs
└── src/                  ← Studio-side React components (@pro Vite alias)
```

The OSS desktop binary at `src-tauri/` consumes `learnforge-core` and
implements `LearnForgePlugin` via a no-op `NoopPlugin`. The Studio
binary at `pro/src-tauri-pro/` consumes `learnforge-core` AND
implements `LearnForgePlugin` via `StudioPlugin`, which can register
additional `#[tauri::command]` handlers.

The frontend uses a Vite `@pro` alias:
- When `LEARNFORGE_PRO=1`, `@pro` resolves to `pro/src/`
- When unset (OSS mode), `@pro` resolves to `src/features/_pro_placeholder/`
  (which provides no-op stubs for any component the OSS app references
  via the alias)

This means OSS code can reference `@pro/SettingsVerifyCertSection`
freely without breaking — the OSS build wires it to a no-op stub.

---

## What CAN'T be Studio-side (architectural rules)

To preserve the open-core boundary, the following must remain OSS:

1. **`learnforge-core`** — always MIT, always crates.io-publishable
2. **`LearnForgePlugin` trait** — defined in OSS; Studio implements
   the trait but does NOT modify it
3. **`src-tauri/` core** — OSS desktop binary stays MIT
4. **`src/` core React** — OSS frontend stays MIT
5. **Pack format + schema + loader** — MIT primitives; commercial
   PACKS may be license-gated via a `requires_license` flag, but the
   FORMAT cannot be proprietary
6. **Algorithm whitepapers** — CC BY 4.0; community-readable
7. **Build + CI infrastructure** — `.github/workflows/` stays MIT

If a Studio feature requires modifying any of the above, raise the
question with the maintainers BEFORE writing the code. Crossing the
boundary inappropriately undermines the open-core trust model.

---

## Pro-side review rubric (for maintainers)

Before merging a Studio PR, verify:

- [ ] All new files have LICENSE-STUDIO headers
- [ ] No MIT-licensed file was modified (or it's split into a separate
      OSS PR)
- [ ] No emojis in source / commits / docs
- [ ] Studio Contributor Agreement signed by the author (first PR only)
- [ ] Code reuses MIT primitives from `learnforge-core` where possible
- [ ] No secrets / credentials committed
- [ ] Tests added (TDD discipline)
- [ ] Documentation updated (`pro/`-specific docs)
- [ ] Changelog entry added (per-component CHANGELOG if applicable)
- [ ] `docs/OSS-VS-STUDIO.md` updated if this feature reshapes the
      placement matrix
- [ ] Build passes:
  - `cargo check -p learnforge-studio`
  - `LEARNFORGE_PRO=1 pnpm build`
  - `LEARNFORGE_PRO=1 pnpm test --run`

---

## Inquiries

For:
- Studio contributor onboarding (SCA, access)
- Cross-boundary architectural questions
- License-related questions
- Anything sensitive

Email: `hello@learnforge.dev`

For:
- OSS contributions (anything outside `pro/`)
- Public-facing security disclosures

Use the public channels: GitHub Issues, Discussions, and
`security@learnforge.dev` (per `SECURITY.md`).

---

## What's intentionally NOT here

- A formal contributor list — kept private for now
- The Studio Contributor Agreement text — administered out of band
- Pro-side roadmap details — see `docs/OSS-VS-STUDIO.md` for the
  public-facing feature placement matrix; deeper roadmap is internal

---

*See also: `LICENSE-STUDIO`, `docs/OSS-VS-STUDIO.md`,
`docs/REPO-ARCHITECTURE.md`, `docs/DEVELOPMENT.md`, `CONTRIBUTING.md`.*
