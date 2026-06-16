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
//! - [`microlearning`] — `SqliteMicrolearningStore<'a>(&'a Connection)`
//!   (Phase 7 Wave 4).
//! - [`signing`] — `FsKeyStore { key_dir: PathBuf }` (Phase 7 Wave 5);
//!   filesystem-backed `SigningKeyStore` impl preserving the 0o600 file
//!   mode invariant (R3 / Pitfall 4 / V6 ASVS).
//! - [`blocks`] — `SqliteBlockStore<'a>(&'a Connection)` (Phase 7 Wave 6);
//!   per-block-row CRUD against the `module_blocks` table.
//! - [`packs`] — `SqlitePackStore<'a>(&'a Connection)` (Phase 7 Wave 7);
//!   `topic_packs` table CRUD honoring D-09 enabled-on-conflict +
//!   CR-02 source-column stickiness.
//!
//! Later waves add `achievements`.
//!
//! Both adapters use the local-newtype pattern to satisfy Rust's orphan rule
//! (E0117) — see each module's "Orphan-rule note" for details.

pub mod bkt;
pub mod blocks;
pub mod microlearning;
pub mod packs;
pub mod signing;
pub mod sr;
pub mod threshold;
