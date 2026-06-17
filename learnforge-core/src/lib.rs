//! # learnforge-core
//!
//! Adaptive learning algorithms — Bayesian Knowledge Tracing (BKT), SuperMemo 2
//! (SM-2), threshold predicates, microlearning selection, canonical JSON,
//! Ed25519 signing, block taxonomy, and topic-pack loading — packaged as a
//! desktop/web/WASM-portable Rust crate.
//!
//! ## ⚠ API UNSTABLE
//!
//! This crate is published at version `0.1.x`. **Breaking changes are
//! permitted in any 0.x release.** The public API stabilizes at `1.0.0`.
//! Pin to a specific minor (`learnforge-core = "0.1"`) and read the
//! `CHANGELOG.md` before upgrading.
//!
//! ## Architecture
//!
//! The crate exposes pure, sync, WASM-portable algorithms. Persistence is
//! abstracted via small per-module `Storage` traits (`BktStore`, `SrStore`,
//! `BlockStore`, `PackStore`, `AchievementStore`, `MicrolearningStore`)
//! that consumers implement against their DB of choice. The reference
//! implementation in `learnforge`'s `src-tauri` uses `rusqlite`; web/WASM
//! consumers can implement against IndexedDB.
//!
//! ## Modules
//!
//! - [`bkt`] — Bayesian Knowledge Tracing (BKT) algorithm + `BktStore` trait (Wave 2).
//! - [`path`] — Learning-path DAG primitives + trait-driven prerequisite check (Wave 2).
//! - [`sm2`] — SuperMemo 2 (SM-2) spaced-repetition algorithm + `SrStore` trait (Wave 3).
//! - [`threshold`] — Skill-tier predicates (Associate/Practitioner/Professional) (Wave 4).
//! - [`microlearning`] — Daily-challenge selection algorithm + `MicrolearningStore` trait + A5 clock injection (Wave 4).
//! - [`canonical_json`] — Byte-stable canonical JSON serializer for signing payloads (Wave 5).
//! - [`signing`] — Pure Ed25519 sign/verify + `SigningKeyStore` trait + `share_text` (Wave 5).
//! - [`blocks`] — Block taxonomy (BlockType, BlockStatus, ModuleBlock) + `BlockStore` trait (Wave 6).
//! - [`packs`] — Topic-pack subsystem: bundled loader, schema validator, `PackSource` + `PackStore` traits (Wave 7).
//! - [`achievements`] — Achievement issuance algorithm + `AchievementStore` trait + `maybe_issue` with A5 clock injection (Wave 8).
//! - [`verifier`] — Phase 14 verification contract stub (D-08).
//!
//! All algorithmic modules have now moved to `learnforge-core`. PDF/PNG
//! rendering (printpdf/qrcode/image) STAYS in `src-tauri` per D-03
//! amendment because those crates are not WASM-portable. The renderer
//! input shapes (`CertificatePdfInput` / `BadgePngInput`) live next to
//! the renderers in `src-tauri/src/achievements/artifacts.rs`; they were
//! removed from core in the Phase 7 review-fix pass (WR-01 — the core
//! copies had zero external callers and froze 0.1.0 public API).
//!
//! ## License
//!
//! MIT — see `LICENSE` at the crate root.

#![deny(missing_docs)]

pub mod achievements;
pub mod bkt;
pub mod blocks;
pub mod canonical_json;
pub mod microlearning;
pub mod packs;
pub mod path;
pub mod signing;
pub mod sm2;
pub mod threshold;
pub mod verifier;
