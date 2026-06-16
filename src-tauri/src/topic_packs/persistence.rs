//! Transitional shim — Wave 7 (07-07) moved the `PackStore` trait + helper
//! pure mappers (`source_str`, `status_str`) to
//! `learnforge_core::packs::persistence`. The rusqlite-backed impl lives
//! in [`crate::storage_impl::packs::SqlitePackStore`].
//!
//! This shim preserves the four legacy free-fn names used by
//! `topic_packs::commands` and `topic_packs::loader` so call sites compile
//! unchanged. Each facade is a 1-line forward to
//! `SqlitePackStore(conn).{method}(…)`.
//!
//! Wave 10 grep-and-rewrites every call site to drive the trait directly
//! and deletes this shim.

pub use learnforge_core::packs::persistence::*;

use crate::storage_impl::packs::SqlitePackStore;
use learnforge_core::packs::{LoadedPack, PackError};
use rusqlite::Connection;

/// UPSERT a pack by id. `enabled` is preserved on conflict (D-09).
/// Legacy facade — delegates to [`SqlitePackStore::upsert_pack`].
pub fn upsert_pack(conn: &Connection, p: &LoadedPack) -> Result<(), PackError> {
    SqlitePackStore(conn).upsert_pack(p)
}

/// Read the persisted `enabled` flag for a single pack id.
/// Returns `Ok(None)` if no row exists. Legacy facade — delegates to
/// [`SqlitePackStore::read_enabled`].
pub fn read_enabled(conn: &Connection, pack_id: &str) -> Result<Option<bool>, PackError> {
    SqlitePackStore(conn).read_enabled(pack_id)
}

/// Update the `enabled` flag for an existing pack row. Caller must have
/// previously UPSERTed the row (this function does NOT insert). Legacy
/// facade — delegates to [`SqlitePackStore::write_enabled`].
pub fn write_enabled(conn: &Connection, pack_id: &str, enabled: bool) -> Result<(), PackError> {
    SqlitePackStore(conn).write_enabled(pack_id, enabled)
}

/// Delete all rows where `source = 'skill'`. Used by the skills-only
/// reload path (Q6 lock). Legacy facade — delegates to
/// [`SqlitePackStore::delete_skill_rows`].
pub fn delete_skill_rows(conn: &Connection) -> Result<usize, PackError> {
    SqlitePackStore(conn).delete_skill_rows()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use learnforge_core::packs::{Pack, PackSource, ValidationStatus};
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
            last_loaded_at: "2026-06-15T00:00:00Z".to_string(),
        }
    }

    /// Smoke test: the legacy free fns continue to round-trip through the
    /// trait implementation. The deep correctness tests live in
    /// `crate::storage_impl::packs::tests` (the trait impl) and
    /// `learnforge_core::packs::persistence::tests` (the trait surface).
    #[test]
    fn legacy_facades_round_trip() {
        let conn = fresh_db();
        upsert_pack(&conn, &sample_pack("alpha", "Alpha")).unwrap();
        assert_eq!(read_enabled(&conn, "alpha").unwrap(), Some(true));

        write_enabled(&conn, "alpha", false).unwrap();
        assert_eq!(read_enabled(&conn, "alpha").unwrap(), Some(false));

        let removed = delete_skill_rows(&conn).unwrap();
        assert_eq!(removed, 0, "alpha is bundled; skill-only delete keeps it");
    }
}
