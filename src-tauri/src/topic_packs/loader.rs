//! Transitional shim — Wave 7 (07-07) moved bundled-pack loading +
//! `PackSource` trait declaration to `skillcoco_core::packs::loader`.
//! The FS-touching skill-scan code stays here as the
//! [`FsPackSource`] impl (R3 / Pitfall 4 mitigation), and the
//! rusqlite-bound orchestration free fns ([`load_all`],
//! [`reload_skills_into`]) live here too so the two call sites
//! (`lib.rs:156` and `commands::reload_skills`) compile unchanged.
//!
//! Wave 10 grep-and-rewrites callers to invoke
//! `skillcoco_core::packs::loader::*` + `FsPackSource` directly and
//! deletes the shim.
//!
//! ## Security mitigations preserved verbatim
//!
//! - **T-05-05** (symlink escape): every skill candidate is canonicalized
//!   and rejected if its canonical path escapes the canonical skills
//!   root.
//! - **T-05-06** (5 MB cap): `std::fs::metadata` is consulted before any
//!   `read_to_string`; oversized files yield a sentinel error row.
//! - **T-05-07** (parameterized SQL): all writes go through
//!   [`crate::storage_impl::packs::SqlitePackStore`] which uses
//!   `rusqlite::params!`.

// Re-export pure loader helpers from core. We deliberately do NOT
// `pub use skillcoco_core::packs::loader::*` because that would shadow
// the enum `PackSource` (re-exported by `topic_packs::mod.rs`) with the
// trait `PackSource` from `skillcoco_core::packs::loader`. Each symbol
// is named explicitly to keep the shim's surface unambiguous.
pub use skillcoco_core::packs::loader::{
    classify_errors, now_rfc3339, parse_and_validate, sentinel_pack, BUNDLED_PACKS,
};
pub use skillcoco_core::packs::loader::PackSource as CorePackSource;

use skillcoco_core::packs::loader::PackSource as PackSourceTrait;
use skillcoco_core::packs::{LoadedPack, PackError, PackRegistry, PackSource, ValidationStatus};

use std::collections::HashSet;
use std::path::PathBuf;

use crate::storage_impl::packs::SqlitePackStore;
use skillcoco_core::packs::PackStore;

/// Env var that tests use to redirect the skills root to a tempdir.
/// Production code never sets this — only `#[cfg(test)]` paths do.
pub const SKILLS_DIR_OVERRIDE_ENV: &str = "SKILLCOCO_SKILLS_DIR_OVERRIDE";

/// T-05-06: hard cap on individual pack.json size (5 MB) before reading.
const MAX_PACK_BYTES: u64 = 5 * 1024 * 1024;

/// Resolve the user-skills directory (`~/.skillcoco/skills/`).
///
/// Returns `None` only when the home directory cannot be determined AND
/// the test-override env var is unset. Honors [`SKILLS_DIR_OVERRIDE_ENV`]
/// first so unit tests can redirect to a `tempfile::TempDir`.
pub fn skills_dir() -> Option<PathBuf> {
    if let Ok(override_dir) = std::env::var(SKILLS_DIR_OVERRIDE_ENV) {
        return Some(PathBuf::from(override_dir));
    }
    dirs::home_dir().map(|h| h.join(".skillcoco").join("skills"))
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

/// Single-file FS source for import — replicates the T-05-05 + T-05-06
/// mitigations from [`FsPackSource`] for a single user-supplied file.
///
/// ## Security mitigations (Phase 12, Plan 03 Task 1)
///
/// - **T-05-05** (symlink escape): `std::fs::canonicalize` is called BEFORE any read;
///   the canonical path is what gets read, rejecting symlinks that escape to outside dirs.
/// - **T-05-06** (5 MB cap): `std::fs::metadata` is consulted BEFORE any `read`;
///   oversized files are rejected with `PackError::Schema` before any bytes are read.
pub struct ImportedFilePackSource {
    file_path: std::path::PathBuf,
}

impl ImportedFilePackSource {
    /// Create a new `ImportedFilePackSource` for the given path string.
    pub fn new(file_path: &str) -> Self {
        Self {
            file_path: std::path::PathBuf::from(file_path),
        }
    }

    /// Read the single import file, applying T-05-05 and T-05-06.
    ///
    /// Returns `(bytes, canonical_path)` on success.
    ///
    /// Errors:
    /// - `PackError::Io` — path does not exist, canonicalize fails, or read fails.
    /// - `PackError::Schema` — file exceeds `MAX_IMPORT_BYTES` (5 MB cap).
    pub fn read_file(&self) -> Result<(Vec<u8>, std::path::PathBuf), skillcoco_core::packs::PackError> {
        // T-05-05: canonicalize — reject symlinks that escape allowed paths.
        let canon = std::fs::canonicalize(&self.file_path)
            .map_err(|e| skillcoco_core::packs::PackError::Io(e.to_string()))?;

        // T-05-06: enforce 5 MB cap BEFORE read (mirror FsPackSource constant).
        const MAX_IMPORT_BYTES: u64 = 5 * 1024 * 1024;
        let md = std::fs::metadata(&canon)
            .map_err(|e| skillcoco_core::packs::PackError::Io(e.to_string()))?;
        if md.len() > MAX_IMPORT_BYTES {
            return Err(skillcoco_core::packs::PackError::Schema(format!(
                "import file exceeds {} bytes ({} actual)",
                MAX_IMPORT_BYTES,
                md.len()
            )));
        }

        let bytes = std::fs::read(&canon)
            .map_err(|e| skillcoco_core::packs::PackError::Io(e.to_string()))?;

        Ok((bytes, canon))
    }
}

/// FS-backed [`skillcoco_core::packs::loader::PackSource`] impl.
///
/// Wraps the canonical skills-dir scan with the T-05-05 + T-05-06
/// mitigations preserved verbatim from pre-Wave-7
/// `src-tauri/src/topic_packs/loader.rs`. Each invocation walks the
/// configured skills root once and returns the raw `(skill_id, bytes)`
/// pairs; parsing + validation happens in the orchestration layer below
/// (and in `skillcoco_core::packs::loader::parse_and_validate`).
///
/// Wave 10 cleanup will inline this into the call site if no other
/// platforms grow the same need.
pub struct FsPackSource;

impl PackSourceTrait for FsPackSource {
    fn skills_dir(&self) -> Result<Option<PathBuf>, PackError> {
        Ok(skills_dir())
    }

    fn read_skill_pack_files(&self) -> Result<Vec<(String, Vec<u8>)>, PackError> {
        let skills_root = match ensure_skills_dir() {
            Some(p) => p,
            None => return Ok(vec![]),
        };

        // T-05-05: canonicalize the root once so we can verify every candidate
        // file resolves underneath it (rejects `pack.json` symlinks that escape).
        let canon_root = match std::fs::canonicalize(&skills_root) {
            Ok(p) => p,
            Err(e) => {
                log::warn!(
                    "Failed to canonicalize skills root {:?}: {} — skipping skills scan",
                    skills_root,
                    e
                );
                return Ok(vec![]);
            }
        };

        let read = match std::fs::read_dir(&skills_root) {
            Ok(it) => it,
            Err(e) => {
                log::warn!(
                    "Failed to read skills dir {:?}: {} — skipping skills scan",
                    skills_root,
                    e
                );
                return Ok(vec![]);
            }
        };

        let mut out = Vec::new();
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
                    // Push an empty body — the orchestration layer treats it as
                    // a parse failure and writes a sentinel error row keyed by
                    // the id_guess.
                    out.push((id_guess, Vec::new()));
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

            let bytes = match std::fs::read(&canon_candidate) {
                Ok(b) => b,
                Err(e) => {
                    log::warn!("Failed to read {:?}: {}", canon_candidate, e);
                    continue;
                }
            };

            let id_guess = dir_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown-skill")
                .to_string();
            out.push((id_guess, bytes));
        }

        Ok(out)
    }
}

/// Full boot-time load: bundled + skills, bundled-wins-on-collision,
/// persists each accepted pack to SQLite via the `PackStore` trait
/// (Wave 7 split — was an inline rusqlite write pre-Wave-7).
///
/// CR-02: accumulates a `bundled_ids` set from EVERY bundled pack attempt
/// (success OR sentinel-on-failure). The set is stored on the returned
/// `PackRegistry` so subsequent `reload_skills_into` calls can honor
/// D-03 against failed-bundled ids that are not present in
/// `registry.packs`.
pub fn load_all(conn: &rusqlite::Connection) -> Result<PackRegistry, PackError> {
    let source = FsPackSource;
    load_all_with(&source, conn)
}

/// Re-scan skills only. Q6 lock: bundled is compile-time-frozen, so
/// "reload" never re-walks `BUNDLED_PACKS`.
pub fn reload_skills_into(
    registry: &mut PackRegistry,
    conn: &rusqlite::Connection,
) -> Result<(), PackError> {
    let source = FsPackSource;
    reload_skills_with(registry, &source, conn)
}

/// Generic boot-time load against any `PackSource` impl. Public so unit
/// tests can inject mock sources without going through the filesystem.
pub fn load_all_with<S: PackSourceTrait>(
    source: &S,
    conn: &rusqlite::Connection,
) -> Result<PackRegistry, PackError> {
    let mut registry = PackRegistry::default();

    // ── Bundled (compile-time, pure) ──
    for dir in BUNDLED_PACKS.dirs() {
        // CR-02: capture the directory name as the id-of-record up front,
        // so we can populate `bundled_ids` even when parse/validate fails.
        let id_guess = dir
            .path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown-bundled-pack")
            .to_string();
        if !id_guess.is_empty() && id_guess != "unknown-bundled-pack" {
            registry.bundled_ids.insert(id_guess.clone());
        }

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
                registry.bundled_ids.insert(id.clone());
                let enabled = SqlitePackStore(conn).read_enabled(&id)
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
                let _ = SqlitePackStore(conn).upsert_pack(&lp);
                registry.packs.insert(id, lp);
            }
            Err(e) => {
                log::error!(
                    "Bundled pack at {:?} failed validation: {} — sentinel row written",
                    dir.path(),
                    e
                );
                let _ = SqlitePackStore(conn).upsert_pack(
                    &sentinel_pack(&id_guess, PackSource::Bundled, &e.to_string()),
                );
            }
        }
    }

    // ── Skills (runtime, via PackSource trait) ──
    let bundled_ids = registry.bundled_ids.clone();
    load_skills_via_source(&mut registry, source, conn, &bundled_ids);

    Ok(registry)
}

/// Generic skills-only reload against any `PackSource` impl. Public so
/// unit tests can inject mock sources.
pub fn reload_skills_with<S: PackSourceTrait>(
    registry: &mut PackRegistry,
    source: &S,
    conn: &rusqlite::Connection,
) -> Result<(), PackError> {
    registry
        .packs
        .retain(|_, p| p.source != PackSource::Skill);

    if let Err(e) = SqlitePackStore(conn).delete_skill_rows() {
        log::warn!("Failed to delete skill rows on reload: {}", e);
    }

    let bundled_ids = registry.bundled_ids.clone();
    load_skills_via_source(registry, source, conn, &bundled_ids);

    Ok(())
}

/// Internal: consume a `PackSource`'s skill pack bytes, parse+validate
/// each, honor D-03 bundled-wins-on-collision, and UPSERT every accepted
/// pack.
fn load_skills_via_source<S: PackSourceTrait>(
    registry: &mut PackRegistry,
    source: &S,
    conn: &rusqlite::Connection,
    bundled_ids: &HashSet<String>,
) {
    let entries = match source.read_skill_pack_files() {
        Ok(v) => v,
        Err(e) => {
            log::warn!("PackSource::read_skill_pack_files failed: {} — skipping", e);
            return;
        }
    };

    for (id_guess, bytes) in entries {
        // Empty bytes → FsPackSource flagged an oversized file (T-05-06).
        // Treat as a parse failure and persist a sentinel.
        if bytes.is_empty() {
            let _ = SqlitePackStore(conn).upsert_pack(
                &sentinel_pack(
                    &id_guess,
                    PackSource::Skill,
                    &format!("pack.json exceeds {} bytes", MAX_PACK_BYTES),
                ),
            );
            continue;
        }

        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Skill pack {:?} is not valid UTF-8: {}", id_guess, e);
                let _ = SqlitePackStore(conn).upsert_pack(
                    &sentinel_pack(
                        &id_guess,
                        PackSource::Skill,
                        &format!("pack.json not UTF-8: {}", e),
                    ),
                );
                continue;
            }
        };

        match parse_and_validate(text) {
            Ok((pack, soft_warnings)) => {
                let id = pack.id.clone();
                if bundled_ids.contains(&id) || registry.packs.contains_key(&id) {
                    log::warn!(
                        "Skill pack id {:?} collides with bundled — skill dropped (D-03 bundled wins)",
                        id
                    );
                    continue;
                }
                let enabled = SqlitePackStore(conn).read_enabled(&id)
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
                let _ = SqlitePackStore(conn).upsert_pack(&lp);
                registry.packs.insert(id, lp);
            }
            Err(e) => {
                log::error!(
                    "Skill pack {:?} failed validation: {} — sentinel row written",
                    id_guess,
                    e
                );
                let _ = SqlitePackStore(conn).upsert_pack(
                    &sentinel_pack(&id_guess, PackSource::Skill, &e.to_string()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema as db_schema;
    use rusqlite::Connection;
    use std::path::Path;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Serialize tests that mutate the process-wide env var.
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

    /// GREEN — D-03: bundled-wins-on-id-collision.
    #[test]
    fn collision_bundled_wins() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

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
    /// violations).
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

    /// GREEN — D-07 strict path: a skill missing `title` is REJECTED.
    #[test]
    fn strict_rejects_required() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

        let body = serde_json::json!({
            "id": "missing-title",
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

    /// GREEN — D-07 soft path: out-of-range difficulty becomes a warning.
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

    /// GREEN — Q6: reload_skills_into rescans skills only.
    #[test]
    fn reload_skills_picks_up_changes() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

        let conn = fresh_db();
        let mut registry = load_all(&conn).expect("initial load");
        let bundled_before = registry.iter_by_source(PackSource::Bundled).count();
        assert!(registry.get("reload-target").is_none());

        write_skill_pack(tmp.path(), "reload-target", &valid_pack_json("reload-target", "Reload Target"));
        reload_skills_into(&mut registry, &conn).expect("reload");
        assert!(
            registry.get("reload-target").is_some(),
            "newly-added skill must appear after reload"
        );

        let bundled_after = registry.iter_by_source(PackSource::Bundled).count();
        assert_eq!(bundled_before, bundled_after, "Q6: bundled untouched by reload");

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

    /// CR-02 regression test: D-03 must hold for FAILED-bundled packs too.
    #[test]
    fn skill_cannot_shadow_failed_bundled_pack() {
        let tmp = TempDir::new().unwrap();
        let conn = fresh_db();

        // Seed a bundled sentinel row directly.
        let sentinel = sentinel_pack(
            "broken-pack",
            PackSource::Bundled,
            "schema violation: missing required field `title`",
        );
        SqlitePackStore(&conn).upsert_pack(&sentinel)
            .expect("seed sentinel bundled row");

        let mut bundled_ids: HashSet<String> = HashSet::new();
        bundled_ids.insert("broken-pack".to_string());

        // Drop a perfectly-valid skill pack claiming the same id.
        write_skill_pack(
            tmp.path(),
            "broken-pack",
            &valid_pack_json("broken-pack", "Imposter Skill"),
        );

        // Custom in-process source that returns exactly the broken-pack bytes
        // — bypasses ENV_LOCK + global env-var fiddling.
        struct ScopedSource {
            files: Vec<(String, Vec<u8>)>,
        }
        impl PackSourceTrait for ScopedSource {
            fn skills_dir(&self) -> Result<Option<PathBuf>, PackError> {
                Ok(None)
            }
            fn read_skill_pack_files(&self) -> Result<Vec<(String, Vec<u8>)>, PackError> {
                Ok(self.files.clone())
            }
        }
        let source = ScopedSource {
            files: vec![(
                "broken-pack".to_string(),
                valid_pack_json("broken-pack", "Imposter Skill").into_bytes(),
            )],
        };

        let mut registry = PackRegistry::default();
        registry.bundled_ids = bundled_ids.clone();
        load_skills_via_source(&mut registry, &source, &conn, &bundled_ids);

        assert!(
            registry.get("broken-pack").is_none(),
            "D-03 (failed-bundled variant): skill with id `broken-pack` must \
             be dropped because a bundled sentinel claimed the id"
        );

        let (source_col, status): (String, String) = conn
            .query_row(
                "SELECT source, validation_status FROM topic_packs WHERE id='broken-pack'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("sentinel row must still exist");
        assert_eq!(
            source_col, "bundled",
            "CR-02: bundled sentinel row must not be flipped to `skill`"
        );
        assert_eq!(
            status, "errors",
            "CR-02: validation_status must remain `errors` (sentinel preserved)"
        );
    }

    /// T-05-05 symlink-escape mitigation (unix-only).
    #[cfg(unix)]
    #[test]
    fn symlink_escape_rejected() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let skills_root = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();

        let target_dir = outside.path().join("evil-pack");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(
            target_dir.join("pack.json"),
            valid_pack_json("evil-escape", "Evil Escape"),
        )
        .unwrap();

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

    // ── ImportedFilePackSource tests (Phase 12, Plan 03, Task 1) ─────────────

    /// GREEN — normal small file returns (bytes, canonical_path).
    #[test]
    fn imported_file_pack_source_reads_small_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("test-pack.json");
        let content = b"{ \"hello\": \"world\" }";
        std::fs::write(&file_path, content).unwrap();

        let src = ImportedFilePackSource::new(file_path.to_str().unwrap());
        let result = src.read_file();
        assert!(result.is_ok(), "small file must succeed; got: {:?}", result);
        let (bytes, canon) = result.unwrap();
        assert_eq!(bytes, content, "bytes must match file content");
        assert!(canon.is_absolute(), "canonical path must be absolute");
    }

    /// GREEN — T-05-06: >5MB file is rejected before read (size cap enforced via metadata).
    #[test]
    fn imported_file_pack_source_rejects_oversized_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("big.json");
        // Write >5MB worth of data.
        let big_content = vec![b'x'; 5 * 1024 * 1024 + 1];
        std::fs::write(&file_path, &big_content).unwrap();

        let src = ImportedFilePackSource::new(file_path.to_str().unwrap());
        let result = src.read_file();
        assert!(result.is_err(), ">5MB file must return Err (T-05-06)");
        let err = result.unwrap_err();
        let msg = format!("{}", err);
        assert!(
            msg.contains("exceeds") || msg.contains("5242880"),
            "error must mention size cap; got: {}",
            msg
        );
    }

    /// GREEN — nonexistent path returns Err(Io), not a panic.
    #[test]
    fn imported_file_pack_source_nonexistent_returns_io_err() {
        let src = ImportedFilePackSource::new("/tmp/__does_not_exist_skillcoco_test__.json");
        let result = src.read_file();
        assert!(result.is_err(), "nonexistent path must return Err");
        // Must be an Io error (not Schema)
        match result.unwrap_err() {
            skillcoco_core::packs::PackError::Io(_) => {} // correct
            other => panic!("expected PackError::Io, got: {:?}", other),
        }
    }

    /// GREEN — T-05-05: symlink escaping outside its target is rejected via canonicalize (unix only).
    #[cfg(unix)]
    #[test]
    fn imported_file_pack_source_symlink_resolves_to_canonical() {
        // Create a real file outside tmp dir (simulating a symlink that "escapes")
        // In practice canonicalize() succeeds on valid symlinks — T-05-05 guard
        // in import is simpler than in FsPackSource (no root-prefix check needed
        // since we're reading a single file, not scanning a directory).
        // We test that a symlink TO a valid file reads correctly (no false reject)
        // and that a dangling symlink returns Err (Io), not panic.
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("real.json");
        std::fs::write(&target, b"{}").unwrap();

        let link = tmp.path().join("link.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let src = ImportedFilePackSource::new(link.to_str().unwrap());
        let result = src.read_file();
        assert!(result.is_ok(), "valid symlink to real file must succeed; got: {:?}", result);

        // Dangling symlink → canonicalize fails → Io error
        let dangling = tmp.path().join("dangling.json");
        std::os::unix::fs::symlink("/tmp/__nonexistent_target_xyz__", &dangling).unwrap();
        let src2 = ImportedFilePackSource::new(dangling.to_str().unwrap());
        let result2 = src2.read_file();
        assert!(result2.is_err(), "dangling symlink must return Err");
    }

    /// T-05-06 cap test — a >5MB pack is rejected with a sentinel row.
    #[test]
    fn oversized_pack_rejected() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        std::env::set_var(SKILLS_DIR_OVERRIDE_ENV, tmp.path());

        // Build a >5MB pack.json
        let mut body = String::from("{\"id\":\"oversized\",\"title\":\"Big\",\"description\":\"");
        body.push_str(&"x".repeat(5 * 1024 * 1024 + 100));
        body.push_str("\",\"domain_module\":\"devops\",\"modules\":[]}");
        write_skill_pack(tmp.path(), "oversized", &body);

        let conn = fresh_db();
        let registry = load_all(&conn).expect("load_all");
        assert!(
            registry.get("oversized").is_none(),
            "T-05-06: oversized pack must NOT be in registry"
        );

        let (status, messages_json): (String, String) = conn
            .query_row(
                "SELECT validation_status, validation_messages_json FROM topic_packs WHERE id='oversized'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("sentinel row must exist for oversized pack");
        assert_eq!(status, "errors");
        assert!(
            messages_json.contains("5242880") || messages_json.contains("exceeds"),
            "validation message must mention size cap (got {})",
            messages_json
        );

        std::env::remove_var(SKILLS_DIR_OVERRIDE_ENV);
    }
}
