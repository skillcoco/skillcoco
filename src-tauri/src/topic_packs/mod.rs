//! # Topic Packs (Phase 5) — Phase 7 Wave 7 (07-07) transitional shim
//!
//! Pack format, loader, validator, persistence, and IPC handlers for the
//! Topic Packs + Skills system.
//!
//! Phase 7 Wave 7 moved the pure parts of this subsystem
//! (`error`, `model`, `schema`, `registry`, the bundled-pack loader, the
//! `PackSource` + `PackStore` traits) to `learnforge_core::packs`. The
//! files in this module are now thin re-export shims so existing call
//! sites (`commands/ai.rs`, `commands/labs/session_tests.rs`,
//! `topic_packs::commands::*`) compile unchanged.
//!
//! ## What stays in src-tauri
//!
//! - [`commands`] — Tauri IPC handlers (`list_topic_packs`,
//!   `list_topic_packs_admin`, `set_topic_pack_enabled`, `reload_skills`,
//!   `get_topic_pack_modules`). **UNCHANGED** by Wave 7 — these use
//!   `tauri::AppState`, which cannot move into core (Pitfall 7).
//! - [`loader`] — FS-backed `FsPackSource` impl + the orchestration free
//!   fns `load_all(conn)` and `reload_skills_into(reg, conn)` that bind
//!   the pure bundled loader to rusqlite. Wave 10 cleanup will rewrite
//!   the two call sites and delete the free fns.
//! - [`persistence`] — re-exports `learnforge_core::packs::PackStore` +
//!   `SqlitePackStore` from `crate::storage_impl::packs`, and exposes
//!   legacy free fns (`upsert_pack`, `read_enabled`, `write_enabled`,
//!   `delete_skill_rows`) so `commands.rs` and the existing unit tests
//!   compile unchanged.

pub mod commands;
pub mod error;
pub mod loader;
pub mod model;
pub mod persistence;
pub mod registry;
pub mod schema;

pub use error::PackError;
pub use model::{LoadedPack, Pack, PackEdge, PackModule, PackSource, ValidationStatus};
pub use registry::PackRegistry;
