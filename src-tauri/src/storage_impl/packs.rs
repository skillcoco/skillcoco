//! Rusqlite-backed `PackStore` impl + helpers for the `topic_packs`
//! SQLite table.
//!
//! Phase 7 Wave 7 (07-07). Seventh application of the per-module storage
//! trait + orphan-rule newtype recipe established in Waves 2-6.
//!
//! ## Orphan-rule recipe
//!
//! `impl PackStore for &Connection` would violate E0117 because both the
//! trait (`skillcoco_core::packs::persistence::PackStore`) and the
//! target (`rusqlite::Connection`) are foreign to `src-tauri`. The local
//! newtype [`SqlitePackStore`] satisfies the orphan rule with no runtime
//! cost — a single-field tuple struct around `&T` has identical layout
//! to `&T`.
//!
//! ## Honored contracts
//!
//! - **D-09 (user-toggle persistence):** [`SqlitePackStore::upsert_pack`]
//!   uses `ON CONFLICT(id) DO UPDATE` that intentionally does NOT touch
//!   the `enabled` column.
//! - **CR-02 (source-column stickiness):** the same ON CONFLICT clause
//!   keeps `source='bundled'` even when an incoming row has `source='skill'`.
//! - **Q6 (skills-only reload):** [`SqlitePackStore::delete_skill_rows`]
//!   filters by `source='skill'`; bundled rows are untouched.

use skillcoco_core::packs::persistence::{source_str, status_str, PackStore};
use skillcoco_core::packs::{LoadedPack, PackError};
use rusqlite::{params, Connection};

/// Local newtype carrying the rusqlite-backed [`PackStore`] impl.
///
/// See module-level docs for the orphan-rule rationale.
pub struct SqlitePackStore<'a>(pub &'a Connection);

impl<'a> PackStore for SqlitePackStore<'a> {
    fn upsert_pack(&self, p: &LoadedPack) -> Result<(), PackError> {
        let messages_json = serde_json::to_string(&p.validation_messages)
            .unwrap_or_else(|_| "[]".to_string());
        self.0
            .execute(
                "INSERT INTO topic_packs \
                    (id, title, source, enabled, pack_version, last_loaded_at, validation_status, validation_messages_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
                 ON CONFLICT(id) DO UPDATE SET \
                    title=excluded.title, \
                    source=CASE WHEN topic_packs.source='bundled' THEN 'bundled' ELSE excluded.source END, \
                    pack_version=excluded.pack_version, \
                    last_loaded_at=excluded.last_loaded_at, \
                    validation_status=excluded.validation_status, \
                    validation_messages_json=excluded.validation_messages_json",
                params![
                    p.pack.id,
                    p.pack.title,
                    source_str(p.source),
                    p.enabled as i64,
                    p.pack.pack_version,
                    p.last_loaded_at,
                    status_str(p.validation_status),
                    messages_json,
                ],
            )
            .map_err(|e| PackError::Loader(format!("upsert_pack: {}", e)))?;
        Ok(())
    }

    fn read_enabled(&self, pack_id: &str) -> Result<Option<bool>, PackError> {
        let mut stmt = self
            .0
            .prepare("SELECT enabled FROM topic_packs WHERE id = ?1")
            .map_err(|e| PackError::Loader(format!("read_enabled prepare: {}", e)))?;
        let mut rows = stmt
            .query(params![pack_id])
            .map_err(|e| PackError::Loader(format!("read_enabled query: {}", e)))?;
        if let Some(row) = rows
            .next()
            .map_err(|e| PackError::Loader(format!("read_enabled row: {}", e)))?
        {
            let v: i64 = row
                .get(0)
                .map_err(|e| PackError::Loader(format!("read_enabled get: {}", e)))?;
            Ok(Some(v != 0))
        } else {
            Ok(None)
        }
    }

    fn write_enabled(&self, pack_id: &str, enabled: bool) -> Result<(), PackError> {
        self.0
            .execute(
                "UPDATE topic_packs SET enabled = ?1 WHERE id = ?2",
                params![enabled as i64, pack_id],
            )
            .map_err(|e| PackError::Loader(format!("write_enabled: {}", e)))?;
        Ok(())
    }

    fn delete_skill_rows(&self) -> Result<usize, PackError> {
        let n = self
            .0
            .execute("DELETE FROM topic_packs WHERE source = 'skill'", [])
            .map_err(|e| PackError::Loader(format!("delete_skill_rows: {}", e)))?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use skillcoco_core::packs::{Pack, PackSource, ValidationStatus};
    use rusqlite::Connection;

    fn fresh_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    fn sample_pack(id: &str, title: &str) -> LoadedPack {
        LoadedPack {
            pack: Pack {
                id: id.to_string(),
                title: title.to_string(),
                description: "desc".to_string(),
                domain_module: "devops".to_string(),
                estimated_hours: Some(10),
                pack_version: "1.0".to_string(),
                requires_docker: false,
                modules: vec![],
                edges: vec![],
            },
            source: PackSource::Bundled,
            enabled: true,
            validation_status: ValidationStatus::Ok,
            validation_messages: vec![],
            last_loaded_at: "2026-06-16T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn upsert_inserts_new_row() {
        let conn = fresh_db();
        let store = SqlitePackStore(&conn);
        store.upsert_pack(&sample_pack("alpha", "Alpha")).expect("insert");

        let (title, source, enabled, status): (String, String, i64, String) = conn
            .query_row(
                "SELECT title, source, enabled, validation_status FROM topic_packs WHERE id='alpha'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(title, "Alpha");
        assert_eq!(source, "bundled");
        assert_eq!(enabled, 1);
        assert_eq!(status, "ok");
    }

    /// D-09: a user toggle (write_enabled false) survives a subsequent
    /// upsert_pack call (even though the in-memory pack carries enabled: true).
    #[test]
    fn upsert_preserves_enabled_on_conflict() {
        let conn = fresh_db();
        let store = SqlitePackStore(&conn);
        store.upsert_pack(&sample_pack("beta", "Beta")).unwrap();

        store.write_enabled("beta", false).unwrap();
        assert_eq!(store.read_enabled("beta").unwrap(), Some(false));

        let mut p2 = sample_pack("beta", "Beta v2 — title updated");
        p2.enabled = true;
        store.upsert_pack(&p2).unwrap();

        assert_eq!(
            store.read_enabled("beta").unwrap(),
            Some(false),
            "D-09: user toggle must survive upsert_pack on conflict"
        );

        let title: String = conn
            .query_row("SELECT title FROM topic_packs WHERE id='beta'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Beta v2 — title updated");
    }

    #[test]
    fn read_enabled_returns_none_for_missing() {
        let conn = fresh_db();
        let store = SqlitePackStore(&conn);
        assert_eq!(store.read_enabled("does-not-exist").unwrap(), None);
    }

    #[test]
    fn validation_messages_round_trip() {
        let conn = fresh_db();
        let store = SqlitePackStore(&conn);
        let mut p = sample_pack("gamma", "Gamma");
        p.validation_status = ValidationStatus::Warnings;
        p.validation_messages = vec![
            "/modules/0/difficulty: out of range".to_string(),
            "/modules/1/estimated_minutes: missing".to_string(),
        ];
        store.upsert_pack(&p).unwrap();

        let messages_json: String = conn
            .query_row(
                "SELECT validation_messages_json FROM topic_packs WHERE id='gamma'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let decoded: Vec<String> = serde_json::from_str(&messages_json).unwrap();
        assert_eq!(decoded.len(), 2);
        assert!(decoded[0].contains("/modules/0/difficulty"));
        assert!(decoded[1].contains("/modules/1/estimated_minutes"));
    }

    #[test]
    fn write_enabled_toggles() {
        let conn = fresh_db();
        let store = SqlitePackStore(&conn);
        store.upsert_pack(&sample_pack("delta", "Delta")).unwrap();
        assert_eq!(store.read_enabled("delta").unwrap(), Some(true));

        store.write_enabled("delta", false).unwrap();
        assert_eq!(store.read_enabled("delta").unwrap(), Some(false));

        store.write_enabled("delta", true).unwrap();
        assert_eq!(store.read_enabled("delta").unwrap(), Some(true));
    }

    /// CR-02: an incoming skill upsert MUST NOT downgrade a bundled row's
    /// source column. This guards every future caller, not just the loader.
    #[test]
    fn upsert_does_not_downgrade_bundled_to_skill() {
        let conn = fresh_db();
        let store = SqlitePackStore(&conn);

        // Seed a bundled row
        let mut bundled = sample_pack("shared-id", "Bundled");
        bundled.source = PackSource::Bundled;
        store.upsert_pack(&bundled).unwrap();

        // Attempt a skill collision
        let mut skill_clash = sample_pack("shared-id", "Skill Pretender");
        skill_clash.source = PackSource::Skill;
        store.upsert_pack(&skill_clash).unwrap();

        let source: String = conn
            .query_row("SELECT source FROM topic_packs WHERE id='shared-id'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(source, "bundled", "CR-02: source must NOT downgrade");
    }

    #[test]
    fn delete_skill_rows_removes_only_skills() {
        let conn = fresh_db();
        let store = SqlitePackStore(&conn);

        let mut b = sample_pack("bundled-one", "Bundled One");
        b.source = PackSource::Bundled;
        store.upsert_pack(&b).unwrap();

        let mut s1 = sample_pack("skill-one", "Skill One");
        s1.source = PackSource::Skill;
        store.upsert_pack(&s1).unwrap();

        let mut s2 = sample_pack("skill-two", "Skill Two");
        s2.source = PackSource::Skill;
        store.upsert_pack(&s2).unwrap();

        let deleted = store.delete_skill_rows().unwrap();
        assert_eq!(deleted, 2);

        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM topic_packs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 1);

        let kind: String = conn
            .query_row("SELECT source FROM topic_packs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(kind, "bundled");
    }

    #[test]
    fn sqlite_pack_store_is_object_safe() {
        let conn = fresh_db();
        fn _use_dyn(_s: &dyn PackStore) {}
        let store = SqlitePackStore(&conn);
        _use_dyn(&store);
    }
}
