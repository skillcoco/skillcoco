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
