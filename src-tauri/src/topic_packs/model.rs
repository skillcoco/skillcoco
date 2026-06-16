//! Transitional shim — Wave 7 (07-07) moved the pack data model to
//! `learnforge_core::packs::model`.
//!
//! Wave 10 grep-and-rewrites every `use crate::topic_packs::model::*` call
//! site to import directly from `learnforge_core::packs` and deletes this
//! shim.

pub use learnforge_core::packs::model::*;
