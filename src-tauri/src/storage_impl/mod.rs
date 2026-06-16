//! Rusqlite-backed implementations of the per-module storage traits declared
//! in `learnforge_core`.
//!
//! Each algorithm crate module owns its own trait (A3 lock — `07-RESEARCH.md`),
//! so the impls land in matching files here as the migration proceeds:
//!
//! - [`bkt`] — `SqliteBktStore<'a>(&'a Connection)` (Phase 7 Wave 2).
//! - [`sr`] — `SqliteSrStore<'a>(&'a Connection)` (Phase 7 Wave 3).
//! - [`threshold`] — parked `track_mastery_aggregate` free fn (Phase 7
//!   Wave 4); Wave 8 will promote it to an `AchievementStore` trait method.
//!
//! Later waves add `microlearning`, `packs`, `achievements`, etc.
//!
//! Both adapters use the local-newtype pattern to satisfy Rust's orphan rule
//! (E0117) — see each module's "Orphan-rule note" for details.

pub mod bkt;
pub mod sr;
pub mod threshold;
