# Changelog

All notable changes to the **LearnForge desktop application** are
documented in this file.

This changelog tracks the LearnForge desktop application (Tauri 2,
distributed via GitHub Releases). For the `learnforge-core` Rust
crate (published to crates.io), see
[learnforge-core/CHANGELOG.md](learnforge-core/CHANGELOG.md), which
remains the per-crate source of truth per Phase 8 O-6.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
per Phase 8 D-03 (strict semver, on-demand cadence; pre-1.0 minor
bumps may include backwards-incompatible changes).

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

[Unreleased]: https://github.com/agentixgarage/learnforge/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/agentixgarage/learnforge/releases/tag/v0.1.0
