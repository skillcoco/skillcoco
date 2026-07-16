//! Topic-pack subsystem — bundled (compile-time) loader, JSON Schema
//! validator, `PackRegistry`, `PackSource` trait (runtime FS abstraction),
//! and `PackStore` trait (persistence abstraction).
//!
//! Phase 7 Wave 7 (07-07) — moved from `src-tauri/src/topic_packs/`.
//!
//! ## Module layout
//!
//! - [`error`] — `PackError` enum.
//! - [`model`] — `Pack`, `PackModule`, `PackEdge`, `LoadedPack`,
//!   `PackSource` (enum — origin: bundled vs. skill), `ValidationStatus`.
//! - [`schema`] — JSON Schema (Draft 2020-12) validator, embedded via
//!   `include_str!` with the Wave-7 corrected path (4 levels up — R2 fix).
//! - [`registry`] — `PackRegistry` in-memory map.
//! - [`loader`] — `BUNDLED_PACKS` static (`include_dir!`-embedded) +
//!   `parse_and_validate` + `classify_errors` + `sentinel_pack` +
//!   `PackSource` trait (R3 mitigation — FS scanning delegated to the
//!   src-tauri binary).
//! - [`persistence`] — `PackStore` trait + `source_str` / `status_str`
//!   helpers. Rusqlite-backed impl lives in
//!   `src-tauri/src/storage_impl/packs.rs` (orphan-rule recipe, 7th
//!   application).
//!
//! ## Naming overlap
//!
//! The **enum** [`model::PackSource`] and the **trait**
//! [`loader::PackSource`] share the identifier. They live in distinct
//! sub-modules and never conflict at the use site — callers reference the
//! enum via `skillcoco_core::packs::PackSource` (the top-level re-export
//! below) and the trait via the fully-qualified
//! `skillcoco_core::packs::loader::PackSource`.
//!
//! ## What stayed in src-tauri (Pitfall 7)
//!
//! `commands.rs` (Tauri IPC handlers) lives at
//! `src-tauri/src/topic_packs/commands.rs` because each handler uses
//! `tauri::AppState`. Moving it would have leaked `tauri` into core,
//! violating the D-02 anti-leakage invariant.

pub mod error;
pub mod export;
pub mod loader;
pub mod model;
pub mod persistence;
pub mod registry;
pub mod schema;

pub use error::PackError;
pub use model::{LoadedPack, Pack, PackEdge, PackModule, PackSource, ValidationStatus};
pub use persistence::PackStore;
pub use registry::PackRegistry;

// NB: the trait `PackSource` is intentionally NOT re-exported at this
// level — it would shadow the enum `PackSource` (re-exported just above).
// Callers reference the trait via its fully-qualified path
// `skillcoco_core::packs::loader::PackSource`.
