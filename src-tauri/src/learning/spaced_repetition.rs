//! Transitional re-export — Phase 7 Wave 3 (Plan 07-03) moved the SM-2 body
//! to [`learnforge_core::sm2`]. Wave 10 (`07-10-PLAN.md`) deletes this file
//! after grep-and-rewriting every `use crate::learning::spaced_repetition::*`
//! import in `src-tauri` to `use learnforge_core::sm2::*`.
//!
//! `#[deprecated]` is intentionally NOT used on these re-exports — rustc may
//! silently ignore the attribute on `pub use` items (R5 / Pitfall 6). The
//! reliable cleanup mechanism is the Wave 10 grep-and-rewrite.
//!
//! The rusqlite-backed [`learnforge_core::sm2::SrStore`] impl lives in
//! [`crate::storage_impl::sr::SqliteSrStore`] (Wave 3 wired alongside the
//! Wave 2 `SqliteBktStore` pattern).

pub use learnforge_core::sm2::{SM2Result, sm2_calculate};
