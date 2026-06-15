//! In-memory pack registry — keyed by `pack.id`.
//!
//! Populated at boot by [`super::loader::load_all`] and held in
//! `AppState.topic_packs` for the lifetime of the process. The Tauri IPC
//! handlers (Wave 2) borrow the lock for read; the `reload_skills` handler
//! borrows it for write.
//!
//! Wave 1 ships the data structure + small ergonomic accessors. Heavier
//! lookups (e.g. by domain_module, by source) belong in `loader` or the
//! IPC handler that needs them.

use std::collections::BTreeMap;

use super::model::{LoadedPack, PackSource};

#[derive(Debug, Default)]
pub struct PackRegistry {
    /// All loaded packs keyed by their canonical id. `BTreeMap` keeps the
    /// iteration order stable across runs — handy for both deterministic
    /// tests and a stable-looking Settings UI.
    pub packs: BTreeMap<String, LoadedPack>,
}

impl PackRegistry {
    /// Iterate every enabled pack in id order.
    pub fn enabled_iter(&self) -> impl Iterator<Item = &LoadedPack> {
        self.packs.values().filter(|p| p.enabled)
    }

    /// Iterate packs filtered by source (Bundled vs Skill).
    pub fn iter_by_source(&self, source: PackSource) -> impl Iterator<Item = &LoadedPack> {
        self.packs.values().filter(move |p| p.source == source)
    }

    /// Update the `enabled` flag in memory. Returns `true` if the id existed.
    pub fn set_enabled(&mut self, pack_id: &str, enabled: bool) -> bool {
        if let Some(p) = self.packs.get_mut(pack_id) {
            p.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Look up a pack by id.
    pub fn get(&self, pack_id: &str) -> Option<&LoadedPack> {
        self.packs.get(pack_id)
    }

    /// Number of loaded packs.
    pub fn len(&self) -> usize {
        self.packs.len()
    }

    /// `true` when no packs are loaded.
    pub fn is_empty(&self) -> bool {
        self.packs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topic_packs::model::{Pack, ValidationStatus};

    fn lp(id: &str, source: PackSource, enabled: bool) -> LoadedPack {
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
            last_loaded_at: "2026-06-15T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn enabled_iter_filters_disabled() {
        let mut r = PackRegistry::default();
        r.packs.insert("a".to_string(), lp("a", PackSource::Bundled, true));
        r.packs.insert("b".to_string(), lp("b", PackSource::Bundled, false));
        r.packs.insert("c".to_string(), lp("c", PackSource::Skill, true));

        let enabled_ids: Vec<&str> = r.enabled_iter().map(|p| p.pack.id.as_str()).collect();
        assert_eq!(enabled_ids, vec!["a", "c"]);
    }

    #[test]
    fn iter_by_source_splits() {
        let mut r = PackRegistry::default();
        r.packs.insert("a".to_string(), lp("a", PackSource::Bundled, true));
        r.packs.insert("b".to_string(), lp("b", PackSource::Skill, true));

        let bundled: Vec<&str> = r
            .iter_by_source(PackSource::Bundled)
            .map(|p| p.pack.id.as_str())
            .collect();
        let skill: Vec<&str> = r
            .iter_by_source(PackSource::Skill)
            .map(|p| p.pack.id.as_str())
            .collect();
        assert_eq!(bundled, vec!["a"]);
        assert_eq!(skill, vec!["b"]);
    }

    #[test]
    fn set_enabled_toggles_existing_only() {
        let mut r = PackRegistry::default();
        r.packs.insert("a".to_string(), lp("a", PackSource::Bundled, true));

        assert!(r.set_enabled("a", false));
        assert!(!r.packs["a"].enabled);
        assert!(!r.set_enabled("missing", true));
    }
}
