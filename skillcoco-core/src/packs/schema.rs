//! JSON Schema (Draft 2020-12) validator for pack.json files.
//!
//! ## Wave 9 (07-09) — packaged-asset path
//!
//! `topic-packs/` lives **inside** the crate at `skillcoco-core/topic-packs/`
//! so that `cargo publish` ships the bundled-pack assets in the crate
//! tarball. The `include_str!` path is therefore relative to this file's
//! directory:
//!
//! ```text
//! skillcoco-core/src/packs/schema.rs  →  ../../topic-packs/pack-schema.json
//! ```
//!
//! Two `..` segments hop from `skillcoco-core/src/packs/` →
//! `skillcoco-core/src/` → `skillcoco-core/`, then `topic-packs/...`
//! addresses the in-crate directory.
//!
//! Wave 9 relocation rationale: pre-Wave-9 the path was
//! `../../../topic-packs/...`, which reached one level above the crate
//! root. That worked for `cargo build` (Cargo lets `include_str!` chase
//! any filesystem path) but **broke `cargo publish --dry-run`** — the
//! verified package tarball cannot contain files outside the crate
//! directory. Moving `topic-packs/` into the crate is the canonical fix.
//!
//! The schema source is embedded at compile time so the binary is
//! self-contained — no runtime FS read needed for validation.

use serde_json::Value;
use std::sync::OnceLock;

/// Raw schema text, embedded at compile time.
///
/// Path is relative to **this file's directory**
/// (`skillcoco-core/src/packs/`), resolving to
/// `skillcoco-core/topic-packs/pack-schema.json` via two `..` segments
/// (`packs/ → src/ → skillcoco-core/`). See the module-level
/// doc-comment for the full derivation + Wave 9 publish-tarball
/// constraint.
pub const SCHEMA_SOURCE: &str =
    include_str!("../../topic-packs/pack-schema.json");

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

    /// Wave 7 / R2 — proves the corrected include_str! path resolves to a
    /// non-empty schema source string. If the path drift was wrong the
    /// build would have failed at compile time, but this serves as a
    /// runtime sanity check too.
    #[test]
    fn schema_source_non_empty() {
        assert!(
            !SCHEMA_SOURCE.is_empty(),
            "SCHEMA_SOURCE must embed pack-schema.json contents (R2 path fix verification)"
        );
        // Pack schemas declare a `$schema` URI — verifies we embedded a real schema
        assert!(
            SCHEMA_SOURCE.contains("$schema") || SCHEMA_SOURCE.contains("type"),
            "SCHEMA_SOURCE looks malformed; first 80 chars: {}",
            &SCHEMA_SOURCE.chars().take(80).collect::<String>()
        );
    }

    /// A minimal valid pack body shared by the signature-block tests below.
    fn minimal_pack() -> serde_json::Value {
        json!({
            "id": "sig-schema-test",
            "title": "Signature Schema Test",
            "description": "d",
            "domain_module": "devops",
            "modules": [
                { "id": "mod-one", "title": "M1", "description": "d", "objectives": ["o"] }
            ]
        })
    }

    fn valid_signature_block() -> serde_json::Value {
        json!({
            "alg": "ed25519",
            "issuerCert": {
                "issuerId": "issuer-001",
                "name": "Test Issuer",
                "publicKeyPem": "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEA\n-----END PUBLIC KEY-----",
                "rootSig": "aabbccdd"
            },
            "keyFingerprint": "a1b2c3d4",
            "sig": "deadbeef"
        })
    }

    /// D-09 — a pack JSON with NO `signature` key at all must stay schema
    /// valid; unsigned free packs are not required to carry a signature.
    #[test]
    fn unsigned_free_pack_still_schema_valid() {
        let value = minimal_pack();
        let errors = validate(&value);
        assert!(
            errors.is_empty(),
            "unsigned pack (no signature key) must be schema-valid; got: {:?}",
            errors
        );
    }

    /// A pack with a well-formed `signature` block passes the schema.
    #[test]
    fn signed_pack_schema_valid() {
        let mut value = minimal_pack();
        value["signature"] = valid_signature_block();
        let errors = validate(&value);
        assert!(
            errors.is_empty(),
            "well-formed signature block must be schema-valid; got: {:?}",
            errors
        );
    }

    /// additionalProperties:false on SignatureBlock — an unknown key inside
    /// `signature` must fail validation.
    #[test]
    fn signature_block_extra_field_rejected() {
        let mut value = minimal_pack();
        let mut sig = valid_signature_block();
        sig["unexpectedField"] = json!("should not be allowed");
        value["signature"] = sig;
        let errors = validate(&value);
        assert!(
            !errors.is_empty(),
            "signature block with an unknown key must fail schema validation"
        );
    }

    /// Pitfall 3 — no signature-adjacent field in the compiled schema source
    /// is typed `"type": "number"` inside the SignatureBlock/IssuerCert defs
    /// (all identifiers/hashes/sigs must be strings to avoid JCS
    /// float-precision loss).
    #[test]
    fn signature_block_fields_never_typed_number() {
        let sig_defs_start = SCHEMA_SOURCE
            .find("\"SignatureBlock\"")
            .expect("SignatureBlock def must exist in schema source");
        let issuer_defs_start = SCHEMA_SOURCE
            .find("\"IssuerCert\"")
            .expect("IssuerCert def must exist in schema source");
        let region_start = sig_defs_start.min(issuer_defs_start);
        // Slice from the earlier def to the end of the Module def (bounds the
        // scan to the signature-related $defs, not the whole file).
        let module_start = SCHEMA_SOURCE
            .find("\"Module\":")
            .expect("Module def must exist");
        let region = &SCHEMA_SOURCE[region_start..module_start];
        assert!(
            !region.contains("\"type\": \"number\""),
            "no signature-adjacent field may be typed number (Pitfall 3)"
        );
    }
}
