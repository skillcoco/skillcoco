//! Migration v017 — skill-reports data model
//!
//! Phase 18 (REP-01/REP-03): capability-tag storage. Adds `modules.skills_json`
//! (D-03.2 path-generation-time tags), the `capability_tags` table (D-01/D-02
//! many-to-many module<->capability mapping), and `pending_evidence_submissions`
//! (D-13 fire-and-forget durable retry queue for org report submission).
//!
//! `evidence_class` is stored as plain TEXT validated at the Rust layer (NOT a
//! CHECK constraint) so the reserved D-07 `exam` evidence class can be
//! inserted without a future migration — per 18-PATTERNS.md §v017 guidance,
//! CHECK-constraint rebuilds are the more expensive/risky pattern and should
//! only be used when genuinely needed.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 17;
pub const NAME: &str = "skill_reports";

/// Apply the v017 migration.
///
/// 1. Guard-ALTER `modules` to add `skills_json TEXT NOT NULL DEFAULT '[]'`.
/// 2. CREATE TABLE IF NOT EXISTS `capability_tags`.
/// 3. CREATE TABLE IF NOT EXISTS `pending_evidence_submissions`.
pub fn up(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "modules", "skills_json")? {
        conn.execute(
            "ALTER TABLE modules ADD COLUMN skills_json TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS capability_tags (
            id             TEXT PRIMARY KEY,
            learner_id     TEXT NOT NULL REFERENCES learner_profiles(id),
            track_id       TEXT NOT NULL,
            module_id      TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
            tag_slug       TEXT NOT NULL,
            tag_label      TEXT NOT NULL,
            evidence_class TEXT NOT NULL DEFAULT 'module'
        );
        CREATE INDEX IF NOT EXISTS idx_capability_tags_learner_track
            ON capability_tags(learner_id, track_id);
        CREATE INDEX IF NOT EXISTS idx_capability_tags_slug
            ON capability_tags(tag_slug);

        CREATE TABLE IF NOT EXISTS pending_evidence_submissions (
            id                TEXT PRIMARY KEY,
            payload_json      TEXT NOT NULL,
            signature_json    TEXT NOT NULL,
            report_server_url TEXT NOT NULL,
            attempts          INTEGER NOT NULL DEFAULT 0,
            created_at        TEXT NOT NULL DEFAULT (datetime('now')),
            last_attempt_at   TEXT
        );
        "#,
    )
}

/// Check whether `column` exists in `table` by querying PRAGMA table_info.
fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for c in cols {
        if c? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    /// REP-01/REP-03 — v017 idempotency. After two applies:
    ///  - modules.skills_json column exists (TEXT NOT NULL DEFAULT '[]')
    ///  - capability_tags table exists with both indexes
    ///  - pending_evidence_submissions table exists
    #[test]
    fn v017_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        // modules.skills_json column exists
        let mut stmt = conn.prepare("PRAGMA table_info(modules)").unwrap();
        let cols: Vec<(String, String, Option<String>)> = stmt
            .query_map([], |r| {
                let name: String = r.get(1)?;
                let ty: String = r.get(2)?;
                let dflt: Option<String> = r.get(4)?;
                Ok((name, ty, dflt))
            })
            .unwrap()
            .filter_map(|c| c.ok())
            .collect();
        let skills_json = cols
            .iter()
            .find(|(n, _, _)| n == "skills_json")
            .expect("modules.skills_json column must exist after v017");
        assert!(
            skills_json.1.to_uppercase().contains("TEXT"),
            "skills_json must be TEXT, got {:?}",
            skills_json
        );
        assert!(
            skills_json
                .2
                .as_deref()
                .map(|d| d.contains("[]"))
                .unwrap_or(false),
            "skills_json default must be '[]', got {:?}",
            skills_json.2
        );

        // capability_tags table exists
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='capability_tags'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "capability_tags table must exist after v017");

        // pending_evidence_submissions table exists
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='pending_evidence_submissions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "pending_evidence_submissions table must exist after v017");

        // Both capability_tags indexes exist
        for idx in ["idx_capability_tags_learner_track", "idx_capability_tags_slug"] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    [idx],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "{} must exist after v017", idx);
        }
    }

    /// Existing modules rows (no skills_json value at ALTER time) default to
    /// '[]' — never NULL.
    #[test]
    fn v017_existing_modules_default_to_empty_array() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Tester')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'Rust', 'programming', 'Learn Rust')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path1', 'trk1', '[]', '[]', 'test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('mod1', 'path1', 'Module 1')",
            [],
        )
        .unwrap();

        apply_migrations(&conn).expect("apply must succeed on pre-existing modules row");

        let skills_json: Option<String> = conn
            .query_row(
                "SELECT skills_json FROM modules WHERE id = 'mod1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            skills_json.as_deref(),
            Some("[]"),
            "pre-existing modules row must default skills_json to '[]', never NULL"
        );
    }

    /// The reserved D-07 `exam` evidence_class value must be insertable
    /// without a future migration — proves the plain-TEXT (no CHECK) choice.
    #[test]
    fn v017_evidence_class_accepts_reserved_exam_value() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).expect("baseline tables");
        apply_migrations(&conn).expect("apply must succeed");

        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'Tester')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'Rust', 'programming', 'Learn Rust')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, edges_json, modules_json, generated_by_model) VALUES ('path1', 'trk1', '[]', '[]', 'test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO modules (id, path_id, title) VALUES ('mod1', 'path1', 'Module 1')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO capability_tags (id, learner_id, track_id, module_id, tag_slug, tag_label, evidence_class) VALUES ('ct1', 'lp1', 'trk1', 'mod1', 'debug-pod-networking', 'can debug pod networking', 'exam')",
            [],
        )
        .expect("evidence_class='exam' must insert without error (reserved D-07 slot, no CHECK constraint)");

        let stored: String = conn
            .query_row(
                "SELECT evidence_class FROM capability_tags WHERE id = 'ct1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, "exam");
    }
}
