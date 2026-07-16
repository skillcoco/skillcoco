//! `PackStore` trait — abstract storage for the `topic_packs` SQLite table.
//!
//! Phase 7 Wave 7 (07-07). Mirrors the pattern established in Waves 2-6:
//! the trait + pure-data helpers live in core; the rusqlite-backed impl
//! lives in `src-tauri/src/storage_impl/packs.rs` behind a local newtype
//! `SqlitePackStore<'a>(&'a Connection)` (orphan-rule recipe, seventh
//! application).
//!
//! Pure-data helpers for the `source`/`status` text columns are exported
//! here so the rusqlite adapter can stay a thin SQL-only layer.

use super::error::PackError;
use super::model::{LoadedPack, PackSource, ValidationStatus};

/// Map [`PackSource`] to its SQLite text-column representation.
pub fn source_str(s: PackSource) -> &'static str {
    match s {
        PackSource::Bundled => "bundled",
        PackSource::Skill => "skill",
    }
}

/// Map [`ValidationStatus`] to its SQLite text-column representation.
pub fn status_str(s: ValidationStatus) -> &'static str {
    match s {
        ValidationStatus::Ok => "ok",
        ValidationStatus::Warnings => "warnings",
        ValidationStatus::Errors => "errors",
    }
}

/// Abstract persistence surface for the `topic_packs` SQLite table.
///
/// Implementations honor the contracts established in Phase 5:
///
/// - **D-09 (user-toggle persistence):** [`Self::upsert_pack`] **MUST NOT**
///   overwrite the `enabled` flag on conflict — the user's toggle survives
///   subsequent pack reloads.
/// - **CR-02 (source-column stickiness):** [`Self::upsert_pack`] **MUST NOT**
///   downgrade a row's `source` column from `"bundled"` to `"skill"` on
///   conflict. Belt-and-suspenders for the loader's `bundled_ids` D-03 check.
/// - **Q6 (skills-only reload):** [`Self::delete_skill_rows`] deletes only
///   rows with `source='skill'`; bundled rows are untouched.
///
/// All methods take `&self` (no interior mutability required) and return
/// the typed [`PackError`] envelope. Trust-boundary rule: rusqlite errors
/// are stringified into `PackError::Loader` or `PackError::Io` at the
/// adapter boundary (matches the established Wave-2/3/4/5/6 pattern).
pub trait PackStore {
    /// UPSERT a pack by id, preserving `enabled` + `source='bundled'` on conflict.
    fn upsert_pack(&self, p: &LoadedPack) -> Result<(), PackError>;

    /// Read the persisted `enabled` flag for a single pack id, returning
    /// `Ok(None)` when no row exists yet.
    fn read_enabled(&self, pack_id: &str) -> Result<Option<bool>, PackError>;

    /// Update the `enabled` flag for an existing pack row. Caller must have
    /// previously UPSERTed the row (this function does NOT insert).
    fn write_enabled(&self, pack_id: &str, enabled: bool) -> Result<(), PackError>;

    /// Delete all rows where `source = 'skill'`. Bundled rows are untouched.
    /// Returns the number of rows removed.
    fn delete_skill_rows(&self) -> Result<usize, PackError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Minimal in-memory `PackStore` impl used to prove the trait is
    /// implementable and exercise the contract end-to-end without
    /// pulling in rusqlite (Wave 7 / WASM-safe).
    struct InMemoryStore {
        rows: RefCell<HashMap<String, LoadedPack>>,
    }

    impl InMemoryStore {
        fn new() -> Self {
            Self { rows: RefCell::new(HashMap::new()) }
        }
    }

    impl PackStore for InMemoryStore {
        fn upsert_pack(&self, p: &LoadedPack) -> Result<(), PackError> {
            let mut rows = self.rows.borrow_mut();
            // D-09: preserve enabled on conflict.
            let preserved_enabled = rows.get(&p.pack.id).map(|existing| existing.enabled);
            // CR-02: preserve bundled source on conflict.
            let preserved_source = rows
                .get(&p.pack.id)
                .map(|existing| existing.source)
                .filter(|s| *s == PackSource::Bundled);
            let mut to_insert = p.clone();
            if let Some(e) = preserved_enabled {
                to_insert.enabled = e;
            }
            if let Some(s) = preserved_source {
                to_insert.source = s;
            }
            rows.insert(p.pack.id.clone(), to_insert);
            Ok(())
        }
        fn read_enabled(&self, pack_id: &str) -> Result<Option<bool>, PackError> {
            Ok(self.rows.borrow().get(pack_id).map(|p| p.enabled))
        }
        fn write_enabled(&self, pack_id: &str, enabled: bool) -> Result<(), PackError> {
            if let Some(p) = self.rows.borrow_mut().get_mut(pack_id) {
                p.enabled = enabled;
            }
            Ok(())
        }
        fn delete_skill_rows(&self) -> Result<usize, PackError> {
            let mut rows = self.rows.borrow_mut();
            let before = rows.len();
            rows.retain(|_, p| p.source != PackSource::Skill);
            Ok(before - rows.len())
        }
    }

    use crate::packs::model::{Pack, PackSource};

    fn sample(id: &str, source: PackSource, enabled: bool) -> LoadedPack {
        LoadedPack {
            pack: Pack {
                id: id.to_string(),
                title: id.to_string(),
                description: "d".to_string(),
                domain_module: "devops".to_string(),
                estimated_hours: None,
                pack_version: "1.0".to_string(),
                requires_docker: false,
                modules: vec![],
                edges: vec![],
            },
            source,
            enabled,
            validation_status: ValidationStatus::Ok,
            validation_messages: vec![],
            last_loaded_at: "2026-06-16T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn pack_store_trait_compiles_and_round_trips() {
        let store = InMemoryStore::new();
        let p = sample("alpha", PackSource::Bundled, true);
        store.upsert_pack(&p).unwrap();
        assert_eq!(store.read_enabled("alpha").unwrap(), Some(true));

        store.write_enabled("alpha", false).unwrap();
        assert_eq!(store.read_enabled("alpha").unwrap(), Some(false));
    }

    #[test]
    fn pack_store_d09_preserves_enabled_on_upsert() {
        let store = InMemoryStore::new();
        let p = sample("beta", PackSource::Bundled, true);
        store.upsert_pack(&p).unwrap();
        store.write_enabled("beta", false).unwrap();

        // Reload — upsert with enabled: true
        let mut p2 = sample("beta", PackSource::Bundled, true);
        p2.pack.title = "Beta v2".to_string();
        store.upsert_pack(&p2).unwrap();

        // D-09: enabled must still be false
        assert_eq!(store.read_enabled("beta").unwrap(), Some(false));
    }

    #[test]
    fn pack_store_cr02_does_not_downgrade_source() {
        let store = InMemoryStore::new();
        // Seed bundled
        store
            .upsert_pack(&sample("gamma", PackSource::Bundled, true))
            .unwrap();
        // Skill collision attempt
        store
            .upsert_pack(&sample("gamma", PackSource::Skill, true))
            .unwrap();

        let rows = store.rows.borrow();
        assert_eq!(
            rows.get("gamma").unwrap().source,
            PackSource::Bundled,
            "CR-02: source must NOT downgrade from bundled to skill"
        );
    }

    #[test]
    fn pack_store_delete_skill_rows_keeps_bundled() {
        let store = InMemoryStore::new();
        store
            .upsert_pack(&sample("bundled-1", PackSource::Bundled, true))
            .unwrap();
        store
            .upsert_pack(&sample("skill-1", PackSource::Skill, true))
            .unwrap();
        store
            .upsert_pack(&sample("skill-2", PackSource::Skill, true))
            .unwrap();

        let removed = store.delete_skill_rows().unwrap();
        assert_eq!(removed, 2);
        assert!(store.read_enabled("bundled-1").unwrap().is_some());
        assert!(store.read_enabled("skill-1").unwrap().is_none());
    }

    #[test]
    fn pack_store_is_object_safe() {
        // Compile-only: `&dyn PackStore` must be constructible.
        fn _use_dyn(_s: &dyn PackStore) {}
        let store = InMemoryStore::new();
        _use_dyn(&store);
    }

    #[test]
    fn source_str_maps_variants() {
        assert_eq!(source_str(PackSource::Bundled), "bundled");
        assert_eq!(source_str(PackSource::Skill), "skill");
    }

    #[test]
    fn status_str_maps_variants() {
        assert_eq!(status_str(ValidationStatus::Ok), "ok");
        assert_eq!(status_str(ValidationStatus::Warnings), "warnings");
        assert_eq!(status_str(ValidationStatus::Errors), "errors");
    }
}
