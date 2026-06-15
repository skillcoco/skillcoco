//! Pack loader — bundled (compile-time) + skills (runtime) discovery.
//!
//! Wave 1 (Plan 05-02). Walks the compile-time-embedded [`BUNDLED_PACKS`]
//! tree, scans `~/.learnforge/skills/<id>/pack.json` at runtime, validates
//! every pack against the JSON Schema, classifies errors strict-vs-soft
//! (D-07), and persists the result into the `topic_packs` SQLite table
//! via [`super::persistence`].
//!
//! Decisions honored:
//! - D-03: bundled wins on id collision. Skill packs whose id matches an
//!   already-loaded bundled pack are dropped + warning-logged.
//! - D-07: required-field violations reject the pack; optional-field
//!   violations let it load with `validation_status: warnings`.
//! - D-09: user toggle (`enabled` column) survives reloads.
//! - Q6: `reload_skills_into` rescans skills only — bundled is frozen
//!   at compile time so a "reload" never re-walks include_dir.
//!
//! Security mitigations:
//! - T-05-05: symlink escape rejected. Each skill `pack.json` candidate is
//!   canonicalized and required to live under the canonical skills root.
//! - T-05-06: 5 MB pack-size cap before reading.
//! - T-05-07: every SQLite write uses parameterized rusqlite queries via
//!   the [`super::persistence`] layer.

use include_dir::{include_dir, Dir};
use serde_json::Value;
use std::path::{Path, PathBuf};

use super::error::PackError;
use super::model::{LoadedPack, Pack, PackSource, ValidationStatus};
use super::persistence;
use super::registry::PackRegistry;
use super::schema;

/// Compile-time-embedded `topic-packs/` directory.
///
/// Path is relative to `$CARGO_MANIFEST_DIR` (= `src-tauri/`). Resolves to
/// `<repo>/topic-packs/`, picking up every pack directory automatically —
/// no per-pack code edit needed.
pub static BUNDLED_PACKS: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/../topic-packs");

/// Env var that tests use to redirect the skills root to a tempdir.
/// Production code never sets this — only `#[cfg(test)]` paths do.
pub const SKILLS_DIR_OVERRIDE_ENV: &str = "LEARNFORGE_SKILLS_DIR_OVERRIDE";

/// T-05-06: hard cap on individual pack.json size (5 MB) before reading.
const MAX_PACK_BYTES: u64 = 5 * 1024 * 1024;

/// Resolve the user-skills directory (`~/.learnforge/skills/`).
///
/// Returns `None` only when the home directory cannot be determined AND
/// the test-override env var is unset. Honors [`SKILLS_DIR_OVERRIDE_ENV`]
/// first so unit tests can redirect to a `tempfile::TempDir`.
pub fn skills_dir() -> Option<PathBuf> {
    if let Ok(override_dir) = std::env::var(SKILLS_DIR_OVERRIDE_ENV) {
        return Some(PathBuf::from(override_dir));
    }
    dirs::home_dir().map(|h| h.join(".learnforge").join("skills"))
}

/// Idempotently create the skills directory. Returns `Some(path)` on
/// success, `None` if [`skills_dir`] could not resolve a path.
pub fn ensure_skills_dir() -> Option<PathBuf> {
    let p = skills_dir()?;
    if let Err(e) = std::fs::create_dir_all(&p) {
        log::warn!(
            "Failed to create skills dir at {:?}: {} — proceeding without skills.",
            p,
            e
        );
        return None;
    }
    Some(p)
}

/// Full boot-time load: bundled + skills, bundled-wins-on-collision,
/// persists each accepted pack to SQLite.
pub fn load_all(conn: &rusqlite::Connection) -> Result<PackRegistry, PackError> {
    let mut registry = PackRegistry::default();

    // ── Bundled (compile-time) ──
    for dir in BUNDLED_PACKS.dirs() {
        let pack_path = dir.path().join("pack.json");
        let Some(file) = BUNDLED_PACKS.get_file(&pack_path) else {
            log::warn!(
                "Bundled pack dir {:?} has no pack.json — skipping",
                dir.path()
            );
            continue;
        };
        let Some(text) = file.contents_utf8() else {
            log::warn!(
                "Bundled pack {:?}: pack.json is not valid UTF-8 — skipping",
                dir.path()
            );
            continue;
        };
        match parse_and_validate(text) {
            Ok((pack, soft_warnings)) => {
                let id = pack.id.clone();
                let enabled = persistence::read_enabled(conn, &id)
                    .ok()
                    .flatten()
                    .unwrap_or(true);
                let status = if soft_warnings.is_empty() {
                    ValidationStatus::Ok
                } else {
                    ValidationStatus::Warnings
                };
                let lp = LoadedPack {
                    pack,
                    source: PackSource::Bundled,
                    enabled,
                    validation_status: status,
                    validation_messages: soft_warnings,
                    last_loaded_at: now_rfc3339(),
                };
                let _ = persistence::upsert_pack(conn, &lp);
                registry.packs.insert(id, lp);
            }
            Err(e) => {
                // Surface sentinel row so Settings UI can see the failure.
                let id_guess = dir
                    .path()
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown-bundled-pack")
                    .to_string();
                log::error!(
                    "Bundled pack at {:?} failed validation: {} — sentinel row written",
                    dir.path(),
                    e
                );
                let _ = persistence::upsert_pack(
                    conn,
                    &sentinel_pack(&id_guess, PackSource::Bundled, &e.to_string()),
                );
            }
        }
    }

    // ── Skills (runtime) ──
    if let Some(skills_root) = ensure_skills_dir() {
        load_skills(&mut registry, conn, &skills_root);
    }

    Ok(registry)
}

/// Re-scan skills only. Q6 lock: bundled is compile-time-frozen, so
/// "reload" never re-walks include_dir.
///
/// Wipes all `source='skill'` rows from SQLite first so a removed-from-
/// disk skill disappears from the Settings UI. Then re-runs the
/// skills portion of [`load_all`].
pub fn reload_skills_into(
    registry: &mut PackRegistry,
    conn: &rusqlite::Connection,
) -> Result<(), PackError> {
    // Drop existing skill entries from memory.
    registry
        .packs
        .retain(|_, p| p.source != PackSource::Skill);

    // Drop persisted skill rows. Bundled rows are NOT touched.
    if let Err(e) = persistence::delete_skill_rows(conn) {
        log::warn!("Failed to delete skill rows on reload: {}", e);
    }

    if let Some(skills_root) = ensure_skills_dir() {
        load_skills(registry, conn, &skills_root);
    }
    Ok(())
}

/// Walk the skills root, parse + validate every `<id>/pack.json`, merge
/// with bundled-wins-on-collision, and UPSERT every accepted pack.
fn load_skills(
    registry: &mut PackRegistry,
    conn: &rusqlite::Connection,
    skills_root: &Path,
) {
    // T-05-05: canonicalize the root once so we can verify every candidate
    // file resolves underneath it (rejects `pack.json` symlinks that escape).
    let canon_root = match std::fs::canonicalize(skills_root) {
        Ok(p) => p,
        Err(e) => {
            log::warn!(
                "Failed to canonicalize skills root {:?}: {} — skipping skills scan",
                skills_root,
                e
            );
            return;
        }
    };

    let read = match std::fs::read_dir(skills_root) {
        Ok(it) => it,
        Err(e) => {
            log::warn!(
                "Failed to read skills dir {:?}: {} — skipping skills scan",
                skills_root,
                e
            );
            return;
        }
    };

    for entry in read.flatten() {
        let dir_path = entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        let pack_path = dir_path.join("pack.json");
        if !pack_path.exists() {
            continue;
        }

        // T-05-05: canonicalize candidate and assert it lives under canon_root.
        let canon_candidate = match std::fs::canonicalize(&pack_path) {
            Ok(p) => p,
            Err(e) => {
                log::warn!(
                    "Failed to canonicalize {:?}: {} — skipping",
                    pack_path,
                    e
                );
                continue;
            }
        };
        if !canon_candidate.starts_with(&canon_root) {
            log::error!(
                "T-05-05: skill pack at {:?} escapes skills root via symlink — rejected",
                pack_path
            );
            continue;
        }

        // T-05-06: enforce 5 MB cap BEFORE read.
        match std::fs::metadata(&canon_candidate) {
            Ok(md) if md.len() > MAX_PACK_BYTES => {
                log::error!(
                    "T-05-06: skill pack {:?} exceeds {} bytes ({} actual) — rejected",
                    pack_path,
                    MAX_PACK_BYTES,
                    md.len()
                );
                let id_guess = dir_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("oversized")
                    .to_string();
                let _ = persistence::upsert_pack(
                    conn,
                    &sentinel_pack(
                        &id_guess,
                        PackSource::Skill,
                        &format!("pack.json exceeds {} bytes", MAX_PACK_BYTES),
                    ),
                );
                continue;
            }
            Ok(_) => {}
            Err(e) => {
                log::warn!(
                    "Failed to stat {:?}: {} — skipping",
                    canon_candidate,
                    e
                );
                continue;
            }
        }

        let text = match std::fs::read_to_string(&canon_candidate) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to read {:?}: {}", canon_candidate, e);
                continue;
            }
        };

        match parse_and_validate(&text) {
            Ok((pack, soft_warnings)) => {
                let id = pack.id.clone();
                if registry.packs.contains_key(&id) {
                    // D-03: bundled wins on collision.
                    log::warn!(
                        "Skill pack id {:?} collides with bundled — skill dropped (D-03 bundled wins)",
                        id
                    );
                    continue;
                }
                let enabled = persistence::read_enabled(conn, &id)
                    .ok()
                    .flatten()
                    .unwrap_or(true);
                let status = if soft_warnings.is_empty() {
                    ValidationStatus::Ok
                } else {
                    ValidationStatus::Warnings
                };
                let lp = LoadedPack {
                    pack,
                    source: PackSource::Skill,
                    enabled,
                    validation_status: status,
                    validation_messages: soft_warnings,
                    last_loaded_at: now_rfc3339(),
                };
                let _ = persistence::upsert_pack(conn, &lp);
                registry.packs.insert(id, lp);
            }
            Err(e) => {
                // Persist sentinel error row so Settings UI surfaces it.
                let id_guess = dir_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown-skill")
                    .to_string();
                log::error!(
                    "Skill pack at {:?} failed validation: {} — sentinel row written",
                    pack_path,
                    e
                );
                let _ = persistence::upsert_pack(
                    conn,
                    &sentinel_pack(&id_guess, PackSource::Skill, &e.to_string()),
                );
            }
        }
    }
}

/// Parse + schema-validate. Returns `(pack, soft-warning messages)` on
/// success, or `Err(PackError)` when either parsing or any required-field
/// (D-07 strict) check fails.
fn parse_and_validate(json_text: &str) -> Result<(Pack, Vec<String>), PackError> {
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
fn classify_errors(
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
fn sentinel_pack(id: &str, source: PackSource, error_msg: &str) -> LoadedPack {
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

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema as db_schema;
    use rusqlite::Connection;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Serialize tests that mutate the process-wide env var. Without this,
    /// parallel test threads race and set conflicting `LEARNFORGE_SKILLS_DIR_OVERRIDE`
    /// values.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn fresh_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(db_schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    fn write_skill_pack(skills_root: &Path, id: &str, body: &str) {
        let dir = skills_root.join(id);
        std::fs::create_dir_all(&dir).expect("create skill dir");
        std::fs::write(dir.join("pack.json"), body).expect("write pack.json");
    }

    fn valid_pack_json(id: &str, title: &str) -> String {
        serde_json::json!({
            "id": id,
            "title": title,
            "description": "A test pack.",
            "domain_module": "devops",
            "modules": [
                {
                    "id": "mod-one",
                    "title": "Module One",
                    "description": "First module.",
                    "objectives": ["learn the basics"]
                }
            ]
        })
        .to_string()
    }

    /// GREEN — `BUNDLED_PACKS` walks all 6 packs at compile time AND
    /// `load_all()` loads them.
    #[test]
    fn bundled_loads_all() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // Make sure no skills bleed in.
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

        let dir_count = BUNDLED_PACKS.dirs().count();
        assert!(
            dir_count >= 6,
            "BUNDLED_PACKS must contain at least 6 pack dirs (got {})",
            dir_count
        );

        let conn = fresh_db();
        let registry = load_all(&conn).expect("load_all must succeed");
        let bundled_ids: Vec<&str> = registry
            .iter_by_source(PackSource::Bundled)
            .map(|p| p.pack.id.as_str())
            .collect();
        assert!(
            bundled_ids.len() >= 6,
            "registry must contain >=6 bundled packs (got {:?})",
            bundled_ids
        );
        for required in [
            "kubernetes-fundamentals",
            "rust-from-zero",
            "go-essentials",
            "python-for-devops",
            "agentic-devops",
            "ai-engineering",
        ] {
            assert!(
                bundled_ids.contains(&required),
                "registry missing bundled pack `{}` (got {:?})",
                required,
                bundled_ids
            );
        }

        std::env::remove_var(SKILLS_DIR_OVERRIDE_ENV);
    }

    /// GREEN — a valid skill pack dropped into the skills root is picked
    /// up with `source = Skill` and `enabled = true`.
    #[test]
    fn skills_picked_up() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

        write_skill_pack(tmp.path(), "my-test-skill", &valid_pack_json("my-test-skill", "Test Skill"));

        let conn = fresh_db();
        let registry = load_all(&conn).expect("load_all must succeed");
        let skill = registry.get("my-test-skill").expect("skill must be loaded");
        assert_eq!(skill.source, PackSource::Skill);
        assert!(skill.enabled, "freshly loaded skill must default to enabled");
        assert_eq!(skill.validation_status, ValidationStatus::Ok);

        std::env::remove_var(SKILLS_DIR_OVERRIDE_ENV);
    }

    /// GREEN — D-03: bundled-wins-on-id-collision. A skill claiming the id
    /// `kubernetes-fundamentals` is dropped and the registry keeps the
    /// bundled entry.
    #[test]
    fn collision_bundled_wins() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

        // Skill claims the same id as a bundled pack
        write_skill_pack(
            tmp.path(),
            "kubernetes-fundamentals",
            &valid_pack_json("kubernetes-fundamentals", "Pretender K8s"),
        );

        let conn = fresh_db();
        let registry = load_all(&conn).expect("load_all must succeed");
        let entry = registry
            .get("kubernetes-fundamentals")
            .expect("bundled k8s pack must still be present");
        assert_eq!(
            entry.source,
            PackSource::Bundled,
            "D-03: bundled must win over skill with same id"
        );

        std::env::remove_var(SKILLS_DIR_OVERRIDE_ENV);
    }

    /// GREEN — every bundled pack validates as `Ok` (no required-field
    /// violations). Wave 5 (Plan 05-06) will format-upgrade so even soft
    /// warnings disappear; this test just guards against required-field
    /// regressions.
    #[test]
    fn existing_packs_valid() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

        let conn = fresh_db();
        let registry = load_all(&conn).expect("load_all must succeed");
        for (id, lp) in &registry.packs {
            assert_ne!(
                lp.validation_status,
                ValidationStatus::Errors,
                "{} must not be in Errors state — got messages: {:?}",
                id,
                lp.validation_messages
            );
        }

        std::env::remove_var(SKILLS_DIR_OVERRIDE_ENV);
    }

    /// GREEN — D-07 strict path: a skill missing the required `title`
    /// field is REJECTED. The pack is absent from the in-memory registry
    /// but a sentinel error row is persisted to SQLite so Settings can
    /// show the failure.
    #[test]
    fn strict_rejects_required() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

        let body = serde_json::json!({
            "id": "missing-title",
            // "title" intentionally omitted
            "description": "no title here",
            "domain_module": "devops",
            "modules": [
                {"id": "mod-one", "title": "M", "description": "d", "objectives": ["o"]}
            ]
        })
        .to_string();
        write_skill_pack(tmp.path(), "missing-title", &body);

        let conn = fresh_db();
        let registry = load_all(&conn).expect("load_all must succeed");
        assert!(
            registry.get("missing-title").is_none(),
            "strict reject: missing-required-field pack must NOT be in registry"
        );

        // Sentinel error row exists in SQLite
        let (status, messages_json): (String, String) = conn
            .query_row(
                "SELECT validation_status, validation_messages_json FROM topic_packs WHERE id='missing-title'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("sentinel row must exist");
        assert_eq!(status, "errors");
        assert!(
            messages_json.to_lowercase().contains("title")
                || messages_json.to_lowercase().contains("required"),
            "validation message must mention `title` or `required` (got {})",
            messages_json
        );

        std::env::remove_var(SKILLS_DIR_OVERRIDE_ENV);
    }

    /// GREEN — D-07 soft path: a pack with `difficulty: 99` (out of
    /// schema range 1..=5) on one module LOADS with
    /// `validation_status: Warnings` and a message naming the offending
    /// instance path.
    #[test]
    fn soft_warns_optional() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

        let body = serde_json::json!({
            "id": "soft-warn",
            "title": "Soft Warn Skill",
            "description": "out-of-range difficulty",
            "domain_module": "devops",
            "modules": [
                {
                    "id": "mod-one",
                    "title": "M",
                    "description": "d",
                    "objectives": ["o"],
                    "difficulty": 99
                }
            ]
        })
        .to_string();
        write_skill_pack(tmp.path(), "soft-warn", &body);

        let conn = fresh_db();
        let registry = load_all(&conn).expect("load_all must succeed");
        let lp = registry.get("soft-warn").expect("soft-warn pack must load");
        assert_eq!(lp.validation_status, ValidationStatus::Warnings);
        let messages = lp.validation_messages.join("; ");
        assert!(
            messages.contains("/modules/0/difficulty"),
            "warning must name /modules/0/difficulty (got {})",
            messages
        );

        std::env::remove_var(SKILLS_DIR_OVERRIDE_ENV);
    }

    /// GREEN — Q6: reload_skills_into rescans skills only. A new skill
    /// dropped post-boot appears after reload; a removed skill disappears.
    /// Bundled packs are untouched.
    #[test]
    fn reload_skills_picks_up_changes() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

        let conn = fresh_db();
        let mut registry = load_all(&conn).expect("initial load");
        let bundled_before = registry.iter_by_source(PackSource::Bundled).count();
        assert!(registry.get("reload-target").is_none());

        // Drop a new skill, then reload.
        write_skill_pack(tmp.path(), "reload-target", &valid_pack_json("reload-target", "Reload Target"));
        reload_skills_into(&mut registry, &conn).expect("reload");
        assert!(
            registry.get("reload-target").is_some(),
            "newly-added skill must appear after reload"
        );

        // Bundled count must NOT change.
        let bundled_after = registry.iter_by_source(PackSource::Bundled).count();
        assert_eq!(bundled_before, bundled_after, "Q6: bundled untouched by reload");

        // Remove the skill from disk, reload again — should disappear.
        std::fs::remove_dir_all(tmp.path().join("reload-target")).unwrap();
        reload_skills_into(&mut registry, &conn).expect("reload after delete");
        assert!(
            registry.get("reload-target").is_none(),
            "deleted skill must disappear after reload"
        );

        std::env::remove_var(SKILLS_DIR_OVERRIDE_ENV);
    }

    /// GREEN — malformed JSON is rejected with a sentinel error row.
    #[test]
    fn malformed_json_rejected() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

        write_skill_pack(tmp.path(), "bad-json", "{ this is not json ");

        let conn = fresh_db();
        let registry = load_all(&conn).expect("load_all");
        assert!(registry.get("bad-json").is_none());

        let status: String = conn
            .query_row(
                "SELECT validation_status FROM topic_packs WHERE id='bad-json'",
                [],
                |r| r.get(0),
            )
            .expect("sentinel row must exist for malformed JSON");
        assert_eq!(status, "errors");

        std::env::remove_var(SKILLS_DIR_OVERRIDE_ENV);
    }

    /// Classifier unit test — top-level missing-required field is strict.
    #[test]
    fn classify_top_level_required_is_strict() {
        let v = serde_json::json!({
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

    /// Classifier unit test — module-level optional out-of-range is soft.
    #[test]
    fn classify_module_optional_is_soft() {
        let v = serde_json::json!({
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

    /// T-05-05 symlink-escape mitigation (unix-only). A symlink inside
    /// the skills root pointing OUTSIDE it must be rejected.
    #[cfg(unix)]
    #[test]
    fn symlink_escape_rejected() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // Two tempdirs: `skills_root` holds the symlink; `outside` holds the real target.
        let skills_root = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();

        // Write a valid pack file OUTSIDE the skills root.
        let target_dir = outside.path().join("evil-pack");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(
            target_dir.join("pack.json"),
            valid_pack_json("evil-escape", "Evil Escape"),
        )
        .unwrap();

        // Make a symlink inside skills_root pointing to it.
        let link_dir = skills_root.path().join("evil-escape");
        std::os::unix::fs::symlink(&target_dir, &link_dir).unwrap();

        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, skills_root.path());
        let conn = fresh_db();
        let registry = load_all(&conn).expect("load_all");
        assert!(
            registry.get("evil-escape").is_none(),
            "T-05-05: symlink escape must be rejected"
        );
        std::env::remove_var(SKILLS_DIR_OVERRIDE_ENV);
    }
}
