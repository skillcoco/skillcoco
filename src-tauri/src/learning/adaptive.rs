//! Transitional re-export — Phase 7 Wave 2 (Plan 07-02) moved the BKT body
//! to [`learnforge_core::bkt`]. Wave 10 (`07-10-PLAN.md`) deletes this file
//! after grep-and-rewriting every `use crate::learning::adaptive::*` import
//! in `src-tauri` to `use learnforge_core::bkt::*`.
//!
//! `#[deprecated]` is intentionally NOT used on these re-exports — rustc may
//! silently ignore the attribute on `pub use` items (R5 / Pitfall 6). The
//! reliable cleanup mechanism is the Wave 10 grep-and-rewrite.

pub use learnforge_core::bkt::{
    BKTParams, BktError, BktStore, MASTERY_THRESHOLD, should_adapt, update_mastery,
};
