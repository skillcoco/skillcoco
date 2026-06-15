//! # Topic Packs (Phase 5)
//!
//! Pack format, loader, validator, persistence, and IPC handlers for the
//! Topic Packs + Skills system.
//!
//! ## Wave 0 status (Plan 05-01)
//!
//! This module is in RED scaffold state. Every function body is `unimplemented!()`
//! or returns an `Err(PackError::Loader(...))` with a message naming the downstream
//! wave/plan that will turn it GREEN. The ONLY GREEN test in Wave 0 is
//! [`schema::compiles`] — the schema itself is a deliverable, not a stub.
//!
//! ## Module map
//!
//! - [`model`] — `Pack`, `PackModule`, `PackEdge`, `LoadedPack`, `PackSource`,
//!   `ValidationStatus`. All IPC structs use `#[serde(rename_all = "camelCase")]`.
//! - [`error`] — `PackError` enum (Io / Json / Schema / Loader).
//! - [`schema`] — compiled Draft 2020-12 validator wrapping
//!   `topic-packs/pack-schema.json` (embedded via `include_str!`).
//! - [`loader`] — bundled+skills discovery, parsing, and collision resolution.
//!   Bundled packs win on id collision (D-03).
//! - [`commands`] — Tauri IPC handler signatures (`list_topic_packs`,
//!   `list_topic_packs_admin`, `set_topic_pack_enabled`, `reload_skills`).
//!
//! ## Wave handoff
//!
//! - Wave 1 (Plan 05-02): implement loader + migration v008 `up()`.
//! - Wave 2 (Plan 05-03): implement IPC handlers + persistence.
//! - Wave 3 (Plan 05-04): Settings UI consuming the IPC surface.
//! - Wave 4 (Plan 05-05): Onboarding picker consuming the IPC surface.
//! - Wave 5 (Plan 05-06): format-upgrade the 4 existing packs.

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
