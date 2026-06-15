//! Persistence layer for `topic_packs` SQLite rows.
//!
//! Wave 1 (Plan 05-02). Provides UPSERT semantics on `id` that intentionally
//! does NOT overwrite the `enabled` column on conflict — D-09 toggle persistence:
//! the user's enable/disable choice must survive pack reloads.
//!
//! Storage details:
//! - `validation_messages_json` is the `serde_json::to_string` of the
//!   `LoadedPack.validation_messages: Vec<String>` field (Q4 lock — plain strings).
//! - `last_loaded_at` is written from the in-memory `LoadedPack.last_loaded_at`
//!   string (RFC3339; the loader sets it via `chrono::Utc::now().to_rfc3339()`).

use rusqlite::{params, Connection, Result};

use super::model::{LoadedPack, PackSource, ValidationStatus};

fn source_str(s: PackSource) -> &'static str {
    match s {
        PackSource::Bundled => "bundled",
        PackSource::Skill => "skill",
    }
}

fn status_str(s: ValidationStatus) -> &'static str {
    match s {
        ValidationStatus::Ok => "ok",
        ValidationStatus::Warnings => "warnings",
        ValidationStatus::Errors => "errors",
    }
}

/// UPSERT a pack by id. `enabled` is preserved on conflict (D-09).
///
/// On conflict (id already exists), updates `title, source, pack_version,
/// last_loaded_at, validation_status, validation_messages_json`. The `enabled`
/// column is left untouched so a user toggle survives subsequent reloads.
pub fn upsert_pack(conn: &Connection, p: &LoadedPack) -> Result<()> {
    let messages_json = serde_json::to_string(&p.validation_messages)
        .unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO topic_packs \
            (id, title, source, enabled, pack_version, last_loaded_at, validation_status, validation_messages_json) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(id) DO UPDATE SET \
            title=excluded.title, \
            source=excluded.source, \
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
    )?;
    Ok(())
}

/// Read the persisted `enabled` flag for a single pack id.
/// Returns `Ok(None)` if no row exists for that id yet.
pub fn read_enabled(conn: &Connection, pack_id: &str) -> Result<Option<bool>> {
    let mut stmt = conn.prepare("SELECT enabled FROM topic_packs WHERE id = ?1")?;
    let mut rows = stmt.query(params![pack_id])?;
    if let Some(row) = rows.next()? {
        let v: i64 = row.get(0)?;
        Ok(Some(v != 0))
    } else {
        Ok(None)
    }
}

/// Update the `enabled` flag for an existing pack row. Caller must have
/// previously UPSERTed the row (this function does NOT insert).
pub fn write_enabled(conn: &Connection, pack_id: &str, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE topic_packs SET enabled = ?1 WHERE id = ?2",
        params![enabled as i64, pack_id],
    )?;
    Ok(())
}

/// Delete all rows where `source = 'skill'`. Used by the skills-only reload
/// path (Q6 lock) so a removed-from-disk skill disappears from the Settings
/// UI without the bundled rows being touched.
pub fn delete_skill_rows(conn: &Connection) -> Result<usize> {
    let n = conn.execute("DELETE FROM topic_packs WHERE source = 'skill'", [])?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use crate::topic_packs::model::{LoadedPack, Pack, PackSource, ValidationStatus};
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

    #[test]
    fn upsert_inserts_new_row() {
        let conn = fresh_db();
        let p = sample_pack("alpha", "Alpha");
        upsert_pack(&conn, &p).expect("insert");

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

    /// D-09 contract: a user toggle (write_enabled false) survives a
    /// subsequent upsert_pack call (even though the in-memory pack carries
    /// `enabled: true`).
    #[test]
    fn upsert_preserves_enabled_on_conflict() {
        let conn = fresh_db();
        let p = sample_pack("beta", "Beta");
        upsert_pack(&conn, &p).unwrap();

        // User disables the pack
        write_enabled(&conn, "beta", false).unwrap();
        let after_toggle = read_enabled(&conn, "beta").unwrap();
        assert_eq!(after_toggle, Some(false));

        // Reload — upsert_pack runs again with enabled: true
        let mut p2 = sample_pack("beta", "Beta v2 — title updated");
        p2.enabled = true;
        upsert_pack(&conn, &p2).unwrap();

        // enabled must STILL be false (preserved on conflict, D-09)
        let enabled_after_reload = read_enabled(&conn, "beta").unwrap();
        assert_eq!(
            enabled_after_reload,
            Some(false),
            "D-09: user toggle must survive upsert_pack on conflict"
        );

        // But the title WAS updated (excluded.title is in the ON CONFLICT clause)
        let title: String = conn
            .query_row(
                "SELECT title FROM topic_packs WHERE id='beta'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title, "Beta v2 — title updated");
    }

    #[test]
    fn read_enabled_returns_none_for_missing() {
        let conn = fresh_db();
        let v = read_enabled(&conn, "does-not-exist").unwrap();
        assert_eq!(v, None);
    }

    #[test]
    fn validation_messages_round_trip() {
        let conn = fresh_db();
        let mut p = sample_pack("gamma", "Gamma");
        p.validation_status = ValidationStatus::Warnings;
        p.validation_messages = vec![
            "/modules/0/difficulty: out of range".to_string(),
            "/modules/1/estimated_minutes: missing".to_string(),
        ];
        upsert_pack(&conn, &p).unwrap();

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
        let p = sample_pack("delta", "Delta");
        upsert_pack(&conn, &p).unwrap();
        assert_eq!(read_enabled(&conn, "delta").unwrap(), Some(true));

        write_enabled(&conn, "delta", false).unwrap();
        assert_eq!(read_enabled(&conn, "delta").unwrap(), Some(false));

        write_enabled(&conn, "delta", true).unwrap();
        assert_eq!(read_enabled(&conn, "delta").unwrap(), Some(true));
    }

    /// Source-filtered delete used by the skills-only reload path.
    #[test]
    fn delete_skill_rows_removes_only_skills() {
        let conn = fresh_db();

        // Insert one bundled, two skill rows
        let mut b = sample_pack("bundled-one", "Bundled One");
        b.source = PackSource::Bundled;
        upsert_pack(&conn, &b).unwrap();

        let mut s1 = sample_pack("skill-one", "Skill One");
        s1.source = PackSource::Skill;
        upsert_pack(&conn, &s1).unwrap();

        let mut s2 = sample_pack("skill-two", "Skill Two");
        s2.source = PackSource::Skill;
        upsert_pack(&conn, &s2).unwrap();

        let deleted = delete_skill_rows(&conn).unwrap();
        assert_eq!(deleted, 2, "must delete both skill rows");

        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM topic_packs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 1, "bundled row must remain");

        let kind: String = conn
            .query_row("SELECT source FROM topic_packs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(kind, "bundled");
    }
}
