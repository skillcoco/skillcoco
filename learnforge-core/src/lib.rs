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
//! - [`verifier`] — Phase 14 verification contract stub (D-08).
//!
//! Additional algorithm modules (blocks, packs, achievements)
//! land in later Phase 7 waves per decision D-05.
//!
//! ## License
//!
//! MIT — see `LICENSE` at the crate root.

#![warn(missing_docs)]

pub mod bkt;
pub mod canonical_json;
pub mod microlearning;
pub mod path;
pub mod signing;
pub mod sm2;
pub mod threshold;
pub mod verifier;
