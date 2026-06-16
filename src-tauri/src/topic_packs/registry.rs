//! Transitional shim — Wave 7 (07-07) moved `PackRegistry` to
//! `learnforge_core::packs::registry`.
//!
//! Wave 10 grep-and-rewrites every `use crate::topic_packs::registry::*`
//! call site to import directly from `learnforge_core::packs` and deletes
//! this shim.

pub use learnforge_core::packs::registry::*;
