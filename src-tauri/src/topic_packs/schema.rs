//! JSON Schema (Draft 2020-12) validator for pack.json files.
//!
//! ## Wave 0 status
//!
//! [`compile`] and [`validate`] are the ONLY GREEN surface in Plan 05-01 —
//! the schema asset itself is a deliverable, not a stub. The wave's RED
//! contract is owned by [`loader`](super::loader) and the v008 migration.
//!
//! The schema source is embedded at compile time via `include_str!` from
//! `topic-packs/pack-schema.json` (relative to the crate root) so the
//! binary is self-contained — no runtime FS read needed for validation.

use serde_json::Value;
use std::sync::OnceLock;

/// Raw schema text, embedded at compile time.
///
/// Path is relative to this file (`src-tauri/src/topic_packs/schema.rs`),
/// resolving to `<repo>/topic-packs/pack-schema.json`.
pub const SCHEMA_SOURCE: &str =
    include_str!("../../../topic-packs/pack-schema.json");

/// Lazily-compiled Draft 2020-12 validator. Compiled once per process.
fn compiled() -> &'static jsonschema::Validator {
    static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let schema: Value =
            serde_json::from_str(SCHEMA_SOURCE).expect("pack-schema.json must parse as JSON");
        jsonschema::draft202012::new(&schema).expect("pack-schema.json must compile as Draft 2020-12")
    })
}

/// Compile-only entry point used by [`validate`] and by the `compiles` test.
///
/// Returns a reference to the cached validator. Panics only if the embedded
/// schema is malformed — that is a build-time invariant, not a runtime error.
pub fn compile() -> &'static jsonschema::Validator {
    compiled()
}

/// Validate a `pack.json` value against the compiled schema, returning
/// human-readable error strings.
///
/// Q4 lock: messages are plain strings; structured records are deferred so
/// the SQLite `validation_messages_json` column can stay TEXT.
pub fn validate(value: &Value) -> Vec<String> {
    compiled()
        .iter_errors(value)
        .map(|e| format!("{}: {}", e.instance_path(), e))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// GREEN — the schema MUST compile as a valid Draft 2020-12 document.
    /// This is the wave's GREEN deliverable; the rest of the topic_packs
    /// tests are RED until Waves 1+ implement the loader and migration body.
    #[test]
    fn compiles() {
        // `compile()` lazy-inits the validator. If the embedded schema is
        // malformed, this would panic in `expect` — the assert below is a
        // defense-in-depth check that we reached this point successfully.
        let v = compile();
        // Validator should accept the trivial empty object as having errors
        // (missing required top-level fields) — proves the validator is wired.
        let empty = json!({});
        let errors: Vec<_> = v.iter_errors(&empty).collect();
        assert!(
            !errors.is_empty(),
            "validator must surface 'required' errors for an empty object"
        );
        // Anchor on existence — the count is allowed to drift as we tighten the schema.
        let _ = errors.len();
    }

    /// GREEN — regression guard that missing `id` produces an error mentioning
    /// `id` or `required`. Already passes today; kept so future schema edits
    /// can't silently drop the required-field contract.
    #[test]
    fn rejects_missing_id() {
        let value = json!({
            "title": "X",
            "description": "Y",
            "domain_module": "devops",
            "modules": [
                { "id": "m1", "title": "M1", "description": "d", "objectives": ["o"] }
            ]
        });
        let errors = validate(&value);
        assert!(
            errors.iter().any(|e| e.contains("id") || e.contains("required")),
            "missing top-level id must produce a 'required'/id error; got: {:?}",
            errors
        );
    }
}
