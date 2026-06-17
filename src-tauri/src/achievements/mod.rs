//! Achievement artifacts (PDF cert + PNG badge + QR + share text).
//!
//! Phase 7 Wave 10 cleanup: the pre-Wave-10 module shipped a fat shim that
//! re-exported `learnforge_core::achievements::*` and held the Phase-6
//! legacy `maybe_issue(&Mutex<…>, &Path)` wrapper alongside a
//! `MutexCachedKeyStore` newtype. Wave 10 (`07-10-PLAN.md`) grep-and-
//! rewrote every src-tauri call site to drive `learnforge_core` directly,
//! lifted the `MutexCachedKeyStore` to `crate::storage_impl::signing`, and
//! deleted the rest of this module.
//!
//! ## What stays here (D-03 amendment + R-7 mitigation)
//!
//! - [`artifacts`] — PDF certificate + PNG badge + QR renderer + share
//!   text. **Stays in src-tauri** because `printpdf` / `image` / `qrcode`
//!   are not WASM-portable. The `CertificatePdfInput` / `BadgePngInput`
//!   renderer input shapes also live here (next to the renderers); only
//!   `share_text()` (pure string fn) moved into core
//!   (`learnforge_core::signing::share_text`). WR-01 (Phase 7 review)
//!   removed an earlier duplicate copy of the input shapes from
//!   `learnforge_core::achievements` — they had no external callers and
//!   were freezing 0.1.0 public API.
//!
//! Everything else (algorithm, types, trait, errors) lives in
//! `learnforge_core::achievements` after Wave 8. The rusqlite-backed
//! `AchievementStore` impl lives in `crate::storage_impl::achievements`.

pub mod artifacts;

// `AchievementError` is referenced by `artifacts.rs` via `super::AchievementError`.
// Re-export from core so the existing artifacts module body compiles
// without an internal-path change.
pub use learnforge_core::achievements::AchievementError;
