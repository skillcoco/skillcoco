//! Rusqlite-backed implementations of the per-module storage traits declared
//! in `learnforge_core`.
//!
//! Each algorithm crate module owns its own trait (A3 lock — `07-RESEARCH.md`),
//! so the impls land in matching files here as the migration proceeds:
//!
//! - [`bkt`] — `impl BktStore for &rusqlite::Connection` (Phase 7 Wave 2).
//!
//! Later waves add `sm2`, `microlearning`, `packs`, `achievements`, etc.

pub mod bkt;
