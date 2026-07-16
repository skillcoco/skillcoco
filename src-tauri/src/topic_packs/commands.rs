//! Tauri IPC handler signatures for the topic_packs module.
//!
//! ## Wave 2 status (Plan 05-03)
//!
//! Five `#[tauri::command]` handlers expose the in-memory [`PackRegistry`]
//! and persistence layer to the frontend. Wave 0's `unimplemented!()`
//! stubs are now real bodies.
//!
//! ### Inner-helper-seam pattern (Phase 03.1 `lab_check_step_with` precedent)
//!
//! Each Tauri command is a thin 3-line shim that locks state and forwards
//! to a pure `*_impl` function. The impl functions take `&PackRegistry`,
//! `&mut PackRegistry`, or `&rusqlite::Connection` directly — no
//! `tauri::State` involvement — so they're trivially unit-testable
//! without spinning up a Tauri runtime.
//!
//! Wave 3 (Settings UI) + Wave 4 (Onboarding picker) consume these via
//! the TS wrappers in `src/lib/tauri-commands.ts`.

use rusqlite::Connection;
use tauri::State;

use super::loader;
use crate::storage_impl::packs::SqlitePackStore;
use crate::AppState;
use skillcoco_core::packs::{LoadedPack, PackEdge, PackModule, PackRegistry, PackStore};

// ── Request / Response types ─────────────────────────────────────────────

/// Payload for [`set_topic_pack_enabled`]. camelCase wire shape (Q9 lock).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetTopicPackEnabledRequest {
    pub pack_id: String,
    pub enabled: bool,
}

/// Payload for [`get_topic_pack_modules`]. camelCase wire shape (Q9 lock).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTopicPackModulesRequest {
    pub pack_id: String,
}

/// Result of [`get_topic_pack_modules`] — a pack's modules + edges arrays
/// (Wave 4's track-creation flow consumes this to seed the new track).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackModulesResult {
    pub modules: Vec<PackModule>,
    pub edges: Vec<PackEdge>,
}

// ── Inner-helper impls (Tauri-free, unit-testable) ───────────────────────

/// Inner impl for [`list_topic_packs`]: only `enabled` packs, in id order.
pub fn list_enabled_impl(reg: &PackRegistry) -> Vec<LoadedPack> {
    reg.enabled_iter().cloned().collect()
}

/// Inner impl for [`list_topic_packs_admin`]: every pack, enabled or not,
/// regardless of source. Iteration order is BTreeMap id order.
pub fn list_admin_impl(reg: &PackRegistry) -> Vec<LoadedPack> {
    reg.packs.values().cloned().collect()
}

/// Inner impl for [`set_topic_pack_enabled`]: updates SQLite first, then
/// mirrors the change into the in-memory registry. Returns `Err` with a
/// "Unknown pack id: …" message if the id is not in the registry —
/// T-05-08 short-circuit so SQL never touches the DB for an unknown id.
///
/// WR-01 (Phase 5 review): order matters. The registry write is
/// performed only AFTER `persistence::write_enabled` returns `Ok`. If
/// the SQL write fails (disk full, lock contention, schema corruption),
/// the in-memory registry stays consistent with persistence rather than
/// drifting "newer-than-DB" — which would silently revert on next
/// process restart and confuse the frontend store's refresh-after-toggle
/// path.
pub fn set_enabled_impl(
    reg: &mut PackRegistry,
    conn: &Connection,
    req: &SetTopicPackEnabledRequest,
) -> Result<(), String> {
    // Existence check (T-05-08) — does NOT mutate either layer.
    if !reg.packs.contains_key(&req.pack_id) {
        return Err(format!("Unknown pack id: {}", req.pack_id));
    }
    // DB first: if this fails, neither persistence nor registry mutates.
    SqlitePackStore(conn)
        .write_enabled(&req.pack_id, req.enabled)
        .map_err(|e| format!("SqlitePackStore::write_enabled failed: {}", e))?;
    // Mirror to the registry; infallible because we just verified the id
    // exists (and `set_enabled` is a no-op for unknown ids anyway).
    reg.set_enabled(&req.pack_id, req.enabled);
    Ok(())
}

/// Inner impl for [`get_topic_pack_modules`]: returns the requested pack's
/// modules + edges arrays. Errors with a descriptive message on unknown id.
pub fn get_modules_impl(
    reg: &PackRegistry,
    req: &GetTopicPackModulesRequest,
) -> Result<PackModulesResult, String> {
    let lp = reg
        .get(&req.pack_id)
        .ok_or_else(|| format!("Unknown pack id: {}", req.pack_id))?;
    Ok(PackModulesResult {
        modules: lp.pack.modules.clone(),
        edges: lp.pack.edges.clone(),
    })
}

// ── Tauri command shims ──────────────────────────────────────────────────

#[tauri::command]
pub fn list_topic_packs(state: State<AppState>) -> Result<Vec<LoadedPack>, String> {
    let reg = state
        .topic_packs
        .lock()
        .map_err(|e| format!("topic_packs lock poisoned: {}", e))?;
    Ok(list_enabled_impl(&reg))
}

#[tauri::command]
pub fn list_topic_packs_admin(state: State<AppState>) -> Result<Vec<LoadedPack>, String> {
    let reg = state
        .topic_packs
        .lock()
        .map_err(|e| format!("topic_packs lock poisoned: {}", e))?;
    Ok(list_admin_impl(&reg))
}

#[tauri::command]
pub fn set_topic_pack_enabled(
    state: State<AppState>,
    request: SetTopicPackEnabledRequest,
) -> Result<(), String> {
    let mut reg = state
        .topic_packs
        .lock()
        .map_err(|e| format!("topic_packs lock poisoned: {}", e))?;
    let db = state
        .db
        .lock()
        .map_err(|e| format!("db lock poisoned: {}", e))?;
    set_enabled_impl(&mut reg, &db.conn, &request)
}

#[tauri::command]
pub fn reload_skills(state: State<AppState>) -> Result<(), String> {
    let mut reg = state
        .topic_packs
        .lock()
        .map_err(|e| format!("topic_packs lock poisoned: {}", e))?;
    let db = state
        .db
        .lock()
        .map_err(|e| format!("db lock poisoned: {}", e))?;
    loader::reload_skills_into(&mut reg, &db.conn)
        .map_err(|e| format!("reload_skills_into failed: {}", e))
}

#[tauri::command]
pub fn get_topic_pack_modules(
    state: State<AppState>,
    request: GetTopicPackModulesRequest,
) -> Result<PackModulesResult, String> {
    let reg = state
        .topic_packs
        .lock()
        .map_err(|e| format!("topic_packs lock poisoned: {}", e))?;
    get_modules_impl(&reg, &request)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema as db_schema;
    use skillcoco_core::packs::{Pack, PackSource, ValidationStatus};

    fn fresh_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(db_schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    fn make_loaded_pack(id: &str, enabled: bool) -> LoadedPack {
        LoadedPack {
            pack: Pack {
                id: id.to_string(),
                title: format!("{} title", id),
                description: format!("{} description", id),
                domain_module: "devops".to_string(),
                estimated_hours: Some(8),
                pack_version: "1.0".to_string(),
                requires_docker: false,
                modules: vec![
                    PackModule {
                        id: format!("{}-m1", id),
                        title: "Module One".to_string(),
                        description: "first module".to_string(),
                        difficulty: Some(2),
                        estimated_minutes: Some(30),
                        objectives: vec!["learn one".to_string()],
                        exercise_types: vec!["conceptual_qa".to_string()],
                    },
                    PackModule {
                        id: format!("{}-m2", id),
                        title: "Module Two".to_string(),
                        description: "second module".to_string(),
                        difficulty: Some(3),
                        estimated_minutes: Some(45),
                        objectives: vec!["learn two".to_string()],
                        exercise_types: vec!["code_challenge".to_string()],
                    },
                ],
                edges: vec![PackEdge {
                    from: format!("{}-m1", id),
                    to: format!("{}-m2", id),
                }],
            },
            source: PackSource::Bundled,
            enabled,
            validation_status: ValidationStatus::Ok,
            validation_messages: vec![],
            last_loaded_at: "2026-06-15T00:00:00Z".to_string(),
        }
    }

    fn registry_with(packs: Vec<LoadedPack>) -> PackRegistry {
        let mut reg = PackRegistry::default();
        for p in packs {
            reg.packs.insert(p.pack.id.clone(), p);
        }
        reg
    }

    /// T-05-08: unknown pack_id MUST short-circuit BEFORE touching SQLite.
    #[test]
    fn set_enabled_unknown_pack_returns_err() {
        let mut reg = registry_with(vec![make_loaded_pack("alpha", true)]);
        let conn = fresh_db();

        let result = set_enabled_impl(
            &mut reg,
            &conn,
            &SetTopicPackEnabledRequest {
                pack_id: "does-not-exist".to_string(),
                enabled: false,
            },
        );

        let err = result.expect_err("must err on unknown pack");
        assert!(
            err.contains("Unknown pack id"),
            "error must mention 'Unknown pack id' (got {})",
            err
        );
        assert!(
            err.contains("does-not-exist"),
            "error must mention the offending id (got {})",
            err
        );

        // Registry is untouched.
        assert!(reg.get("alpha").unwrap().enabled);

        // SQLite is untouched — no rows exist at all (nothing was ever upserted).
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM topic_packs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "SQL must not be touched on unknown-id reject");
    }

    /// Happy path: registry flips AND SQLite row reflects the new value.
    #[test]
    fn set_enabled_updates_registry_and_db() {
        let mut reg = registry_with(vec![make_loaded_pack("alpha", true)]);
        let conn = fresh_db();

        // Seed the DB row so write_enabled has a target.
        SqlitePackStore(&conn).upsert_pack(reg.get("alpha").unwrap()).unwrap();

        // Disable via the impl.
        set_enabled_impl(
            &mut reg,
            &conn,
            &SetTopicPackEnabledRequest {
                pack_id: "alpha".to_string(),
                enabled: false,
            },
        )
        .expect("set_enabled_impl must succeed for known id");

        // In-memory: enabled flipped.
        assert!(!reg.get("alpha").unwrap().enabled, "registry must be updated");

        // SQLite: column reflects the change.
        let persisted = SqlitePackStore(&conn).read_enabled("alpha").unwrap();
        assert_eq!(persisted, Some(false), "SQLite enabled column must be 0");

        // Subsequent list_enabled_impl must EXCLUDE the disabled pack.
        let enabled = list_enabled_impl(&reg);
        assert!(
            enabled.iter().all(|p| p.pack.id != "alpha"),
            "list_enabled_impl must omit disabled pack"
        );
    }

    /// list_enabled_impl filters out disabled packs; list_admin_impl does not.
    #[test]
    fn list_filters_disabled() {
        let reg = registry_with(vec![
            make_loaded_pack("alpha", true),
            make_loaded_pack("beta", false),
            make_loaded_pack("gamma", true),
        ]);

        let enabled = list_enabled_impl(&reg);
        let enabled_ids: Vec<&str> = enabled.iter().map(|p| p.pack.id.as_str()).collect();
        assert_eq!(
            enabled_ids,
            vec!["alpha", "gamma"],
            "enabled filter must omit beta"
        );

        let admin = list_admin_impl(&reg);
        let admin_ids: Vec<&str> = admin.iter().map(|p| p.pack.id.as_str()).collect();
        assert_eq!(
            admin_ids,
            vec!["alpha", "beta", "gamma"],
            "admin listing must include disabled packs"
        );
    }

    /// Unknown pack_id on get_modules_impl returns Err with the offending id.
    #[test]
    fn get_modules_unknown_pack_returns_err() {
        let reg = registry_with(vec![make_loaded_pack("alpha", true)]);

        let err = get_modules_impl(
            &reg,
            &GetTopicPackModulesRequest {
                pack_id: "missing".to_string(),
            },
        )
        .expect_err("must err on unknown pack");
        assert!(err.contains("Unknown pack id"));
        assert!(err.contains("missing"));
    }

    /// WR-01: `set_enabled_impl` must NOT mutate the in-memory registry
    /// when the persistence write fails. Original implementation flipped
    /// `reg.set_enabled` BEFORE calling `persistence::write_enabled`, so a
    /// DB-write failure (disk full, schema corruption, lock contention)
    /// left the registry "newer" than SQLite — and the next process
    /// restart would silently revert the toggle.
    ///
    /// Fix: write to SQLite first; only mirror to memory on success.
    /// The "Unknown pack id" short-circuit (T-05-08) is preserved as a
    /// pre-flight existence check that does NOT mutate either layer.
    #[test]
    fn set_enabled_does_not_mutate_registry_when_db_write_fails() {
        // Build a registry seeded with a known pack (enabled = true).
        let mut reg = registry_with(vec![make_loaded_pack("alpha", true)]);

        // Use a connection that lacks the `topic_packs` table — any
        // UPDATE against it will return a rusqlite::Error and the
        // persistence layer will surface it. No migrations applied.
        let bad_conn = Connection::open_in_memory().unwrap();

        let result = set_enabled_impl(
            &mut reg,
            &bad_conn,
            &SetTopicPackEnabledRequest {
                pack_id: "alpha".to_string(),
                enabled: false,
            },
        );

        assert!(
            result.is_err(),
            "DB write failure must propagate as Err (got Ok)"
        );

        // Critical assertion: the registry's `enabled` is still `true`,
        // i.e. the in-memory state was NOT mutated despite the failure.
        assert!(
            reg.get("alpha").expect("pack must still exist").enabled,
            "WR-01: registry must NOT be flipped when persistence::write_enabled fails"
        );
    }

    /// get_modules_impl returns the modules + edges from the requested pack.
    #[test]
    fn get_modules_returns_pack_data() {
        let reg = registry_with(vec![make_loaded_pack("alpha", true)]);

        let result = get_modules_impl(
            &reg,
            &GetTopicPackModulesRequest {
                pack_id: "alpha".to_string(),
            },
        )
        .expect("must succeed for known id");

        assert_eq!(result.modules.len(), 2, "alpha has 2 modules");
        assert_eq!(result.modules[0].id, "alpha-m1");
        assert_eq!(result.modules[1].id, "alpha-m2");
        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.edges[0].from, "alpha-m1");
        assert_eq!(result.edges[0].to, "alpha-m2");
    }
}
