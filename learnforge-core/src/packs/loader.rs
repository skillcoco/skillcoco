//! Pack loader — pure (bundled-only) + `PackSource` trait for runtime FS discovery.
//!
//! Phase 7 Wave 7 (07-07). Splits the pre-Wave-7
//! `src-tauri/src/topic_packs/loader.rs` into:
//!
//! 1. **Core (this file):** `BUNDLED_PACKS` static + parse/validate +
//!    classify_errors + sentinel_pack + the [`PackSource`] trait. Zero
//!    `std::fs`, zero `dirs::home_dir`, zero `PathBuf` instantiation for
//!    skills paths. R3 / Pitfall 4 mitigation — WASM-portable.
//! 2. **src-tauri shim (`src-tauri/src/topic_packs/loader.rs`):**
//!    `FsPackSource` impl carrying the `std::fs::read_dir` + symlink-escape
//!    rejection + 5 MB cap logic. The free-fn `load_all(conn)` orchestrator
//!    stays in src-tauri because it touches rusqlite via the
//!    `topic_packs` table.
//!
//! Decisions honored:
//! - D-03: bundled wins on id collision.
//! - D-07: required-field violations reject (strict); optional-field
//!   violations soft-warn.
//! - Q6: skills-only reload — bundled is compile-time-frozen.
//!
//! Security mitigations (still apply, just enforced in the FS impl):
//! - T-05-05: symlink-escape rejection (`FsPackSource::read_skill_pack_files`).
//! - T-05-06: 5 MB pack-size cap before reading (`FsPackSource::read_skill_pack_files`).

use include_dir::{include_dir, Dir};
use serde_json::Value;
use std::path::PathBuf;

use super::error::PackError;
use super::model::{LoadedPack, Pack, PackSource as PackSourceKind, ValidationStatus};
use super::schema;

/// Compile-time-embedded `topic-packs/` directory.
///
/// Path is relative to `$CARGO_MANIFEST_DIR` (= `learnforge-core/`).
/// Resolves to `<repo>/topic-packs/`, picking up every pack directory
/// automatically — no per-pack code edit needed.
///
/// Phase 7 Wave 7 (07-07): the manifest base changed from `src-tauri/` to
/// `learnforge-core/`; the relative string `$CARGO_MANIFEST_DIR/../topic-packs`
/// is identical because both crates sit at the same depth from the repo root.
pub static BUNDLED_PACKS: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/../topic-packs");

/// Abstract runtime source for skill packs (R3 / Pitfall 4 — Wave 7).
///
/// The pre-Wave-7 `src-tauri/src/topic_packs/loader.rs` mixed compile-time
/// bundled-pack loading with `std::fs::read_dir` skills scanning. Moving
/// that file into core would have leaked `std::fs` / `dirs::home_dir` /
/// `PathBuf::canonicalize` (all of which break wasm32-unknown-unknown).
///
/// `PackSource` is the seam: core declares the trait and consumes it via
/// generic functions; the src-tauri binary implements [`PackSource`] via
/// its `FsPackSource` newtype (in `src-tauri/src/topic_packs/loader.rs`).
/// Future web/IndexedDB consumers can implement it differently.
///
/// **Rejected alternative:** `#[cfg(not(target_arch = "wasm32"))]` gating
/// around the FS code. Rejected because it (a) clutters production source
/// with target-arch conditionals, (b) hides the seam from `cargo doc` /
/// IDE navigation, and (c) makes the rusqlite-vs-IndexedDB split look like
/// "wasm vs not-wasm" when it is actually "FS-backed-store vs
/// browser-backed-store". The trait makes the seam explicit, testable,
/// and identical to the pattern Waves 2-6 already use (`BktStore`,
/// `SrStore`, `BlockStore`, `MicrolearningStore`, `SigningKeyStore`).
///
/// ## Contract
///
/// Implementations MUST preserve the Phase 5 security mitigations:
///
/// - **T-05-05 (symlink-escape):** canonicalize the candidate path and
///   reject any pack.json whose canonical path is not a descendant of
///   the canonical skills root.
/// - **T-05-06 (5 MB cap):** reject any pack.json whose size exceeds 5 MB
///   BEFORE allocating a `Vec<u8>` to hold the body.
pub trait PackSource {
    /// Resolve the runtime skills root (e.g. `~/.learnforge/skills/`).
    ///
    /// Returns `Ok(None)` when the platform has no concept of a skills
    /// directory (WASM / sandboxed contexts). Implementations MAY also
    /// return `Ok(None)` if the configured directory cannot be created.
    fn skills_dir(&self) -> Result<Option<PathBuf>, PackError>;

    /// Enumerate skill packs by reading `pack.json` bytes from the skills
    /// directory.
    ///
    /// Returns a `Vec<(skill_id, pack_json_bytes)>` where `skill_id` is the
    /// candidate directory name (used for sentinel-pack id reconstruction
    /// when validation fails).
    ///
    /// Implementations MUST honor T-05-05 (symlink-escape rejection) and
    /// T-05-06 (5 MB hard cap) — see the trait-level contract above.
    fn read_skill_pack_files(&self) -> Result<Vec<(String, Vec<u8>)>, PackError>;
}

/// Parse + schema-validate. Returns `(pack, soft-warning messages)` on
/// success, or `Err(PackError)` when either parsing or any required-field
/// (D-07 strict) check fails.
///
/// Pure function — no FS, no SQL. WASM-safe.
pub fn parse_and_validate(json_text: &str) -> Result<(Pack, Vec<String>), PackError> {
    // Parse errors are ALWAYS strict (Pitfall 5).
    let value: Value = serde_json::from_str(json_text)
        .map_err(|e| PackError::Json(format!("{}", e)))?;

    let validator = schema::compile();
    let errors: Vec<jsonschema::ValidationError> = validator.iter_errors(&value).collect();
    let (strict, soft) = classify_errors(&errors);

    if !strict.is_empty() {
        return Err(PackError::Schema(strict.join("; ")));
    }

    let pack: Pack = serde_json::from_value(value)
        .map_err(|e| PackError::Json(format!("deserialize Pack: {}", e)))?;

    Ok((pack, soft))
}

/// D-07 classifier. Required-field violations are STRICT (reject the
/// pack); everything else is SOFT (load + warn).
///
/// An error is STRICT when either:
///   - its `schema_path` mentions `/required` (JSON Schema's `required`
///     keyword fired), OR
///   - its `instance_path` lands on a required top-level field
///     (`/id`, `/title`, `/description`, `/domain_module`, `/modules`)
///     or a required per-module field (`/modules/{N}/id`,
///     `/modules/{N}/title`, `/modules/{N}/description`,
///     `/modules/{N}/objectives`).
///
/// Returns `(strict_messages, soft_messages)`. Messages are formatted as
/// `"<instance_path>: <error>"` so Settings can show them as-is.
pub fn classify_errors(
    errors: &[jsonschema::ValidationError<'_>],
) -> (Vec<String>, Vec<String>) {
    let mut strict = Vec::new();
    let mut soft = Vec::new();

    for err in errors {
        let instance = err.instance_path().as_str().to_string();
        let schema = err.schema_path().as_str().to_string();
        let msg = format!("{}: {}", instance, err);

        let is_strict =
            schema.contains("/required") || is_required_instance_path(&instance);

        if is_strict {
            strict.push(msg);
        } else {
            soft.push(msg);
        }
    }
    (strict, soft)
}

/// Returns true when `path` (a JSON-Pointer string from
/// `ValidationError::instance_path`) lands on a required field.
fn is_required_instance_path(path: &str) -> bool {
    // Top-level required fields.
    const TOP_REQUIRED: &[&str] = &[
        "/id",
        "/title",
        "/description",
        "/domain_module",
        "/modules",
    ];
    if TOP_REQUIRED.contains(&path) {
        return true;
    }
    // Per-module required fields: /modules/{N}/{field}
    if let Some(rest) = path.strip_prefix("/modules/") {
        // strip `{N}` then check field
        let mut parts = rest.splitn(2, '/');
        let _idx = parts.next(); // e.g. "0"
        let field = parts.next();
        match field {
            Some("id") | Some("title") | Some("description") | Some("objectives") => {
                return true
            }
            // Whole module missing (e.g. /modules/0 itself is invalid).
            None => return true,
            _ => {}
        }
    }
    false
}

/// Build a "sentinel" LoadedPack used when a pack failed to parse/validate.
/// Carries the error message in `validation_messages` and is persisted to
/// SQLite so the Settings UI can show it as `status='errors'`.
pub fn sentinel_pack(id: &str, source: PackSourceKind, error_msg: &str) -> LoadedPack {
    LoadedPack {
        pack: Pack {
            id: id.to_string(),
            title: id.to_string(),
            description: String::new(),
            domain_module: String::new(),
            estimated_hours: None,
            pack_version: "1.0".to_string(),
            requires_docker: false,
            modules: vec![],
            edges: vec![],
        },
        source,
        enabled: false,
        validation_status: ValidationStatus::Errors,
        validation_messages: vec![error_msg.to_string()],
        last_loaded_at: now_rfc3339(),
    }
}

/// RFC3339 wall-clock timestamp — used by [`sentinel_pack`] + bundled/skill
/// loaders to populate `LoadedPack.last_loaded_at`. WASM-safe because
/// `chrono` is enabled with the `wasmbind` feature (Phase 7 Wave 1).
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// GREEN — `BUNDLED_PACKS` walks all bundled packs at compile time.
    /// Counts directories only — no rusqlite, no FS. WASM-safe.
    #[test]
    fn bundled_packs_dir_present() {
        let dir_count = BUNDLED_PACKS.dirs().count();
        assert!(
            dir_count >= 6,
            "BUNDLED_PACKS must contain at least 6 pack dirs (got {})",
            dir_count
        );
    }

    /// GREEN — every embedded pack.json parses + validates without strict errors.
    /// Soft warnings are allowed (Wave 5 format-upgrade may have closed some).
    #[test]
    fn bundled_packs_parse_and_validate() {
        for dir in BUNDLED_PACKS.dirs() {
            let pack_path = dir.path().join("pack.json");
            let Some(file) = BUNDLED_PACKS.get_file(&pack_path) else {
                continue;
            };
            let Some(text) = file.contents_utf8() else {
                continue;
            };
            let id = dir
                .path()
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?");
            // Must not produce strict errors
            match parse_and_validate(text) {
                Ok(_) => {}
                Err(e) => panic!(
                    "bundled pack `{}` failed parse/validate (strict): {}",
                    id, e
                ),
            }
        }
    }

    #[test]
    fn classify_top_level_required_is_strict() {
        let v = json!({
            "title": "no-id",
            "description": "d",
            "domain_module": "devops",
            "modules": [{"id": "m1", "title": "M", "description": "d", "objectives": ["o"]}]
        });
        let errs: Vec<_> = schema::compile().iter_errors(&v).collect();
        let (strict, _soft) = classify_errors(&errs);
        assert!(
            !strict.is_empty(),
            "missing top-level `id` must produce a strict error"
        );
    }

    #[test]
    fn classify_module_optional_is_soft() {
        let v = json!({
            "id": "valid-id",
            "title": "X",
            "description": "d",
            "domain_module": "devops",
            "modules": [
                {"id": "mod-one", "title": "M", "description": "d", "objectives": ["o"], "difficulty": 99}
            ]
        });
        let errs: Vec<_> = schema::compile().iter_errors(&v).collect();
        let (strict, soft) = classify_errors(&errs);
        assert!(
            strict.is_empty(),
            "out-of-range difficulty must NOT be strict (got strict={:?})",
            strict
        );
        assert!(
            soft.iter().any(|s| s.contains("/modules/0/difficulty")),
            "soft warnings must name /modules/0/difficulty (got {:?})",
            soft
        );
    }

    #[test]
    fn parse_and_validate_returns_pack_on_happy_path() {
        let text = json!({
            "id": "happy",
            "title": "Happy",
            "description": "d",
            "domain_module": "devops",
            "modules": [
                {"id": "mod-one", "title": "M", "description": "d", "objectives": ["o"]}
            ]
        })
        .to_string();
        let (pack, soft) = parse_and_validate(&text).expect("happy path must succeed");
        assert_eq!(pack.id, "happy");
        assert!(soft.is_empty());
    }

    #[test]
    fn parse_and_validate_rejects_required_violation() {
        let text = json!({
            // missing "id"
            "title": "X",
            "description": "d",
            "domain_module": "devops",
            "modules": [
                {"id": "m1", "title": "M", "description": "d", "objectives": ["o"]}
            ]
        })
        .to_string();
        match parse_and_validate(&text) {
            Err(PackError::Schema(_)) => {} // expected
            other => panic!("expected Schema error, got {:?}", other),
        }
    }

    #[test]
    fn parse_and_validate_rejects_malformed_json() {
        let bad = "{ this is not json ";
        match parse_and_validate(bad) {
            Err(PackError::Json(_)) => {} // expected
            other => panic!("expected Json error, got {:?}", other),
        }
    }

    #[test]
    fn sentinel_pack_has_errors_status() {
        let lp = sentinel_pack("broken", PackSourceKind::Bundled, "missing title");
        assert_eq!(lp.validation_status, ValidationStatus::Errors);
        assert_eq!(lp.pack.id, "broken");
        assert_eq!(lp.source, PackSourceKind::Bundled);
        assert!(!lp.enabled);
        assert!(lp.validation_messages[0].contains("missing title"));
    }

    /// `PackSource` trait must be object-safe so consumers can hold a
    /// `&dyn PackSource` or `Box<dyn PackSource>`.
    #[test]
    fn pack_source_trait_is_object_safe() {
        struct Empty;
        impl PackSource for Empty {
            fn skills_dir(&self) -> Result<Option<PathBuf>, PackError> {
                Ok(None)
            }
            fn read_skill_pack_files(&self) -> Result<Vec<(String, Vec<u8>)>, PackError> {
                Ok(vec![])
            }
        }
        fn _use_dyn(_s: &dyn PackSource) {}
        let e = Empty;
        _use_dyn(&e);
    }
}
