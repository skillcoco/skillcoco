//! # Topic Packs (Phase 5) — Phase 7 Wave 10 cleaned surface
//!
//! Pack format, loader, validator, persistence, and IPC handlers for the
//! Topic Packs + Skills system.
//!
//! Phase 7 Waves 7-10 moved the pure parts of this subsystem (`error`,
//! `model`, `schema`, `registry`, the bundled-pack pure loader helpers,
//! and the `PackSource` + `PackStore` traits) into
//! `learnforge_core::packs`. Wave 10 (`07-10-PLAN.md`) deleted every
//! transitional re-export shim file and rewired every src-tauri call site
//! to import directly from `learnforge_core::packs`.
//!
//! ## What stays in src-tauri (R3 / Pitfall 4 / D-03)
//!
//! - [`commands`] — Tauri IPC handlers (`list_topic_packs`,
//!   `list_topic_packs_admin`, `set_topic_pack_enabled`, `reload_skills`,
//!   `get_topic_pack_modules`). **UNCHANGED** by Wave 10 — these use
//!   `tauri::AppState`, which cannot move into core (Pitfall 7).
//! - [`loader`] — FS-backed `FsPackSource` impl + the orchestration free
//!   fns `load_all(conn)` and `reload_skills_into(reg, conn)` that bind
//!   the pure bundled loader to rusqlite via
//!   `crate::storage_impl::packs::SqlitePackStore`.

pub mod commands;
pub mod loader;
