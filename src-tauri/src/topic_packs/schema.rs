//! Transitional shim — Wave 7 (07-07) moved the JSON Schema validator to
//! `learnforge_core::packs::schema` with the `include_str!` path fix
//! (R2 / Pitfall 1).
//!
//! Wave 10 grep-and-rewrites every `use crate::topic_packs::schema::*` call
//! site to import directly from `learnforge_core::packs::schema` and
//! deletes this shim.

pub use learnforge_core::packs::schema::*;
