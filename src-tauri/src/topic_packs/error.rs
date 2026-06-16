//! Transitional shim — Wave 7 (07-07) moved `PackError` to
//! `learnforge_core::packs::error`.
//!
//! Wave 10 grep-and-rewrites every `use crate::topic_packs::error::*` call
//! site to import directly from `learnforge_core::packs` and deletes this
//! shim.

pub use learnforge_core::packs::error::*;
