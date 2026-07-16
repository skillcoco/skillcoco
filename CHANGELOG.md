# Changelog

All notable changes to the **SkillCoco desktop application** are
documented in this file.

This changelog tracks the SkillCoco desktop application (Tauri 2,
distributed via GitHub Releases). For the `skillcoco-core` Rust
crate (published to crates.io), see
[skillcoco-core/CHANGELOG.md](skillcoco-core/CHANGELOG.md), which
remains the per-crate source of truth per Phase 8 O-6.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2026-07-16

The **open-core SkillCoco** release. This is the first public release
under the SkillCoco brand: the adaptive-learning engine, terminal labs,
pack format, and completion badge ship as open source (MIT), while the
commercial-tier features are removed (they continue in the private
SkillCoco Pro fork). Major version bump because the product identity,
bundle identifier, and application data directory all changed and
several commercial features (and their IPC commands) were removed.

Entries below `[Unreleased]` are the pre-release planning record and
are retained for provenance; this `[2.0.0]` entry is the authoritative
account of what the public release actually ships.

### Removed (commercial tier — now Pro-only)

- **Skill Reports** — the signed report envelope, report server / cohort
  aggregation, evidence ledger, capability-tags storage, PDF/JSON export,
  and the in-app report-verify panel.
- **Exam Simulator** — timed, hint-free, server-scored exam mode over the
  labs engine, plus best-attempt history and the exam evidence class.
- **Entitlements & License Redeem** — Hub commerce: license-key redeem,
  buyer-stamped pack download + local entitlement cache, and buyer
  attribution on tracks/cards.
- **Client-side Certificate Trust Chain + pack signature gate** — the
  pack-signing chain of trust, the fail-closed import verify gate, the
  cert-verify IPC + Settings panel, the `forge-sign` pack-signing CLI, and
  the bundled root trust anchor. Pack import is now **plain unsigned local
  import** (schema validation still runs; signatures, if present, are
  ignored). The local completion badge remains, still self-signed
  (Ed25519 via the local key store).

### Kept (the open-source feature set)

- Adaptive engine: Bayesian Knowledge Tracing (BKT), SuperMemo-2 (SM-2)
  spaced repetition, and microlearning daily-challenge selection.
- Topic-pack format + import + bundled free starter packs.
- Lessons, video-enriched content, and quizzes.
- Gamification and the **local completion badge** (self-signed).
- AI tutor with bring-your-own-key (Anthropic, OpenAI, Gemini) and local
  models (Ollama).
- **Terminal labs (use AND build) — a first-class OSS feature.** Learners
  run hands-on labs; authors ship pack-supplied `LAB.md` content. The
  Docker + host-shell runtimes, deterministic + AI-judge checks, hints,
  and milestone-grain validation all remain.

### Renamed

- Rust crate `learnforge-core` → `skillcoco-core` (D-11).
- Product name LearnForge → **SkillCoco**.
- Bundle identifier `com.learnforge.app` → `com.skillcoco.app`.
- Application data directory → `skillcoco` (**clean break — no migration**;
  a fresh install directory is created).
- Environment variables `LEARNFORGE_*` → `SKILLCOCO_*`.

### Changed

- New "coco" visual theme: coconut-cream / cocoa-brown / mango palette
  (light + dark), a coconut mascot as the app icon + favicon.
- README now links the SkillCoco family (Pro / Hub / Studio) at
  skillcoco.com; an in-app Settings "Explore the SkillCoco family" link.

### Breaking

- Major version (2.0.0). Existing installs do **not** migrate — the app
  starts against a fresh `skillcoco` data directory.
- Several IPC commands were removed with their features (reports assembly
  + export + submit + verify, exam attempt/entry, entitlement redeem +
  download + attribution + recovery, and `verify_signature` /
  `get_signing_public_key` / `fingerprint_from_public_pem`).

## [Unreleased]

<!-- Planning milestone v1.1 "Course Commerce Pilot" closed 2026-07-14
     (git tag: milestone/v1.1). App remains pre-release; these entries
     ship with the first public release. -->

### Added

- Library view (`/library`): unified home for course packs — owned packs
  with one-click Start/Continue and progress, bundled free starter packs,
  inline license-key redeem, and pack-file import (relocated from
  Settings; sidebar "New track" affordances consolidated into a single
  Library entry). (Phase 16)
- License-key redeem flow with staged confirmation, local entitlement
  cache (SHA-256 key fingerprints), buyer attribution on tracks and
  cards, and offline re-import from the retained pack artifact.
  (Phase 15)
- Cryptographic pack trust: Ed25519-signed course packs verified by a
  fail-closed import gate; issuer badges on verified content; forge-sign
  CLI for pack/report signing and verification. (Phases 13-14)
- Signed skill reports: tamper-evident report envelope with capability
  table, evidence ledger, and mastery bands; PDF + JSON export; in-app
  verify panel; cohort aggregator for workshops. (Phase 18)
- Exam-Sim mode: timed, hint-free exam runs over the labs engine with
  server-authoritative scoring, best-attempt history, and exam results
  as a distinct evidence class in skill reports. (Phase 19)
- Topic packs can ship their own lab content (`labs/<slug>/LAB.md`),
  loaded verbatim with zero LLM calls; LLM generation remains the
  fallback. (Phase 19.1)
- New deterministic lab check kinds: `command_absent` ("output must NOT
  match", zero-LLM) and `grain: milestone` (validate reached state
  against cumulative session history via an explicit Validate action).
  Existing labs are unaffected; exams reject milestone grain by design.
  (Phases 19.2-19.3)

### Removed

- `scripts/` (sheet2pack converter + enrichment pipeline and its test
  suite) and the `enrich-course` skill — creator-side tooling relocated
  to the private LearnForge Creator Studio product (2026-07-10). The
  learner app has no code, CI, or build dependency on these; pack
  import (including licensed packs) is unaffected. See COORDINATION.md.

## [0.1.0] - planned

First public LearnForge desktop release. Open source under MIT
(repo-root `LICENSE`); algorithms and whitepapers under CC BY 4.0.

API stability: pre-1.0; breaking changes permitted in any 0.x.0
minor bump per D-08c.

### Added

- Adaptive practice loop with Bayesian Knowledge Tracing mastery
  estimation (Phase 1-2; whitepaper: `learnforge-core/docs/BKT.md`).
- SM-2 spaced repetition scheduling for review prompts (Phase 7;
  whitepaper: `learnforge-core/docs/SM2.md`).
- Microlearning daily-challenge selection with BKT-decay scoring
  and a 0.3-0.7 mastery zone (Phase 4).
- Skill-pack import and execution: bundled skill packs plus
  user-imported third-party packs (Phase 3, Phase 5).
- Achievement system with calibrated thresholds and Ed25519-signed
  certificates (Phase 6) for portable proof of mastery.
- macOS, Linux, and Windows desktop builds via Tauri 2; macOS
  binaries code-signed and notarized (D-02); Linux + Windows ship
  unsigned with install docs (D-02b).
- `learnforge-core` Rust crate published to crates.io for embedders
  and the future web platform (Phase 8 D-01; Phase 9 consumer).
- Five algorithm whitepapers in `learnforge-core/docs/` (Phase 8
  D-05): BKT, SM2, threshold calibration, microlearning selection,
  signing.
- Repo-root `SECURITY.md` (Phase 8 Wave 1) with 90-day coordinated
  disclosure policy and GitHub Private Vulnerability Reporting
  intake.
- Open Core repository split (Phase 03.2): MIT OSS surface plus a
  closed-source `pro/` overlay for the paid LearnForge Studio tier;
  CI guardrail (`check-pro-leak.yml`) prevents accidental cross-tier
  leakage.

### Notes

- This release is the public floor of the slow-burn launch (D-04).
  No Hacker News, Reddit, or Lobsters coordination accompanies it;
  the wider launch is deferred to approximately v1.0 (target
  December 2026, per D-08).

[2.0.0]: https://github.com/skillcoco/skillcoco/releases/tag/v2.0.0
[Unreleased]: https://github.com/skillcoco/skillcoco/compare/v2.0.0...HEAD
[0.1.0]: https://github.com/skillcoco/skillcoco/releases/tag/v0.1.0
