# Versioning Policy

**Status:** Active as of 2026-06-17 (Phase 8 publish-prep)
**Authoritative source:** Phase 8 D-03 / D-03b / D-03c, D-08 / D-08b / D-08c, locked O-8

This document describes how LearnForge version numbers are assigned,
how releases are cut, and the strict ordering rule between the
`learnforge-core` crate publish and the desktop application release.
For per-release change history, see the `CHANGELOG.md` files
referenced at the bottom of this document.

## Scope: two independent version streams

LearnForge ships TWO artifacts that version independently:

| Artifact                | Where it lives          | Tag namespace             | Bumped when                                                   |
| ----------------------- | ----------------------- | ------------------------- | ------------------------------------------------------------- |
| `learnforge-core` crate | crates.io               | `core-v{major}.{minor}.{patch}` | Any change to crate source, public API, or crate metadata |
| LearnForge desktop app  | GitHub Releases         | `v{major}.{minor}.{patch}`      | Any change to desktop app behaviour, UX, or bundled config |

The desktop app *depends on* the crate. Per D-03c the desktop app's
**minor** number tracks the crate's minor number (so v0.2.x desktop
consumes core-v0.2.x). Patch numbers may diverge — the desktop can
ship a UX-only fix as 0.2.1 while core stays at 0.2.0.

## Semver discipline (D-03)

LearnForge follows [Semantic Versioning 2.0.0](https://semver.org/)
strictly. The version triplet is `MAJOR.MINOR.PATCH`:

- **PATCH** (`0.x.Y` bump) — bug fixes, performance improvements,
  documentation-only changes. No new public API. No behaviour
  changes visible to dependent code.
- **MINOR** (`0.X.y` bump) — backward-compatible feature additions.
  Until 1.0.0 (see "Pre-1.0 rules" below), MINOR bumps may also
  contain breaking changes — this is explicitly permitted by the
  semver spec for the 0.x.y range.
- **MAJOR** (`X.y.z` bump) — reserved for 1.0.0 and any subsequent
  breaking-change release. Major bumps after 1.0.0 require the
  full deprecation cycle described below.

### Pre-1.0 rules (D-08c)

Until LearnForge ships 1.0.0, breaking changes that would otherwise
require a MAJOR bump go in `0.X.0` MINOR bumps. This is per the
semver spec ("Anything MAY change at any time. The public API
SHOULD NOT be considered stable") and matches how the broader
Rust ecosystem treats 0.x crates.

Concretely:

- Removing a public function from `learnforge-core` → `0.X.0` minor.
- Renaming a Tauri command consumed by the frontend → `0.X.0` minor.
- Changing a default config value → `0.X.0` minor.
- Fixing a typo in a doc comment → `0.x.Y` patch.

We will *prefer* to be conservative — e.g. deprecate before removing —
but the version-number rule above is the policy.

### 1.0.0 commitment criteria (D-08b)

We will not tag 1.0.0 until ALL of the following are true:

1. Two consecutive `0.x.y` minor releases have shipped without
   external breaking-change pressure (i.e. no consumer reports that
   would have forced a MAJOR bump under post-1.0 rules).
2. At least three external consumers exist (corporate, hobbyist, or
   academic) — verified via crates.io download stats + GitHub
   Discussions / issues / direct feedback.
3. `learnforge-core` test coverage is ≥80% (line coverage as reported
   by `cargo-tarpaulin`).
4. The `wasm32-unknown-unknown` target compiles green on every CI
   matrix run (not just spot-checked).
5. The Phase 9 web platform stress-tests (when they exist) pass.

Earliest realistic target: **December 2026**. Per D-08 the commitment
is deferred at least six months from the Phase 8 ship date to leave
room for soak.

## Cadence (D-03b)

LearnForge releases when ready. There is no fixed cadence — no
quarterly, no monthly. A release is cut when the maintainer judges
that the accumulated changes deserve one (typically: a notable bug
fix has landed, OR a small batch of features is ready, OR a security
patch needs to ship).

All releases use the [Keep a Changelog](https://keepachangelog.com/)
format. The `Unreleased` section of each `CHANGELOG.md` accumulates
changes between releases.

## Tag-ordering rule (locked O-8)

When a release includes a change to `learnforge-core` (the crate
published to crates.io), the tags MUST be pushed in this order:

1. Push `core-vX.Y.Z` first.
2. Wait for `.github/workflows/core-publish.yml` to complete
   successfully (the GitHub Actions run finishes green, and the
   new version is visible on crates.io — verify with
   `cargo search learnforge-core` or by visiting
   `https://crates.io/crates/learnforge-core`).
3. Only then push `vX.Y.Z` for the desktop release.

**Rationale:** the desktop build defined by `.github/workflows/release.yml`
depends on the published `learnforge-core` crate. If the desktop tag is
pushed first, the matrix builds may complete and even ship a draft
release artifact that references a crate version that does not yet
exist on crates.io — a state we then have to clean up manually.

**Desktop-only releases:** if a release contains UX/asset fixes only
(no change to the `learnforge-core` crate, no change to crate
dependencies), only the `vX.Y.Z` tag is needed. The `core-vX.Y.Z`
tag is skipped for that cycle.

**How the workflows enforce this:**

- `core-publish.yml` triggers ONLY on `core-v*.*.*` tags.
- `release.yml` triggers ONLY on `v*.*.*` tags (note: this glob does
  NOT match `core-v*` — tag filters are anchored to the full ref name).
- Neither workflow checks the *other's* state — the ordering is a
  human-process rule documented here. The cost of violating it is
  one to two hours of cleanup (yank the bad core publish, fix the
  desktop draft release).

## Yank policy

Released crate versions may be **yanked** (made un-installable via
`cargo` resolver) under any of these conditions:

- A security vulnerability is discovered (see `SECURITY.md` for the
  90-day disclosure flow — yanking happens at fix-publish time).
- The release contains a packaging error (broken `Cargo.toml`
  metadata, missing files, invalid signature on a desktop artifact).
- The wrong version number was published (e.g. tag-version mismatch
  slipped past the workflow guard — should not happen but if it does).

Yanking is permanent on crates.io. We will NOT delete crate versions
(crates.io does not support deletion by policy).

Desktop releases on GitHub Releases may be deleted entirely if needed
(this is a GitHub-specific affordance not available on crates.io).

## Emergency patch flow

For a critical security fix or a release-blocker bug discovered
post-release:

1. Branch from the released tag (`vX.Y.Z` or `core-vX.Y.Z`).
2. Cherry-pick or author the minimal fix.
3. Bump PATCH (`X.Y.(Z+1)`) in the relevant `Cargo.toml` / desktop
   `package.json`.
4. Update the `CHANGELOG.md` `[Unreleased]` section with a `## Fixed`
   entry, then promote to a dated release header.
5. Tag — `core-v` first if the crate changed (per tag-ordering rule
   above), then `v`. Push both.
6. Yank the broken predecessor if the impact warrants it (see
   `SECURITY.md`).

## Changelog locations

- Desktop application: [`CHANGELOG.md`](../CHANGELOG.md) at repo root.
- learnforge-core crate: [`learnforge-core/CHANGELOG.md`](../learnforge-core/CHANGELOG.md).

Per locked O-6, each crate maintains its own per-crate changelog.
The top-level `CHANGELOG.md` covers the desktop app only.

## References

- Phase 8 CONTEXT D-03 / D-03b / D-03c — semver discipline + cadence
- Phase 8 CONTEXT D-08 / D-08b / D-08c — 1.0.0 commitment criteria
- Phase 8 locked O-8 — `core-v*` before `v*` tag-ordering rule
- [Semantic Versioning 2.0.0](https://semver.org/)
- [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/)
- [crates.io: Yanking versions](https://doc.rust-lang.org/cargo/reference/publishing.html#cargo-yank)
