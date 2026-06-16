//! Migration v009 — achievements (Phase 6 Wave 1).
//!
//! Schema per D-12 + R4 (Pitfall 5): `learner_id` cascades on learner
//! delete; `track_id` is plain TEXT (NO foreign-key, NO cascade) so the
//! historical record survives track deletion. `track_topic` snapshots the
//! display value. `UNIQUE (learner_id, track_id, level)` is the D-04
//! immutability gate (maybe_issue uses INSERT OR IGNORE).

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 9;
pub const NAME: &str = "achievements";

/// Apply the v009 migration. Idempotent.
pub fn up(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS achievements (
            id              TEXT PRIMARY KEY,
            learner_id      TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE CASCADE,
            track_id        TEXT NOT NULL,
            pack_id         TEXT,
            kind            TEXT NOT NULL CHECK (kind IN ('badge', 'certificate')),
            level           TEXT NOT NULL CHECK (level IN ('Associate', 'Practitioner', 'Professional', 'Completion')),
            issued_at       TEXT NOT NULL DEFAULT (datetime('now')),
            mastery_score   REAL NOT NULL,
            payload_json    TEXT NOT NULL,
            signature       TEXT NOT NULL,
            key_fingerprint TEXT NOT NULL,
            track_topic     TEXT NOT NULL,
            UNIQUE (learner_id, track_id, level)
        );

        CREATE INDEX IF NOT EXISTS idx_achievements_learner
            ON achievements(learner_id, issued_at DESC);
        CREATE INDEX IF NOT EXISTS idx_achievements_track
            ON achievements(track_id);
        "#,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    /// CERT-02 — v009 idempotency + post-condition contract.
    ///
    /// Asserts:
    /// 1. `achievements` table exists after a double apply.
    /// 2. CHECK (kind IN ('badge','certificate')) rejects bogus values.
    /// 3. UNIQUE (learner_id, track_id, level) rejects duplicates.
    /// 4. Both indexes exist.
    #[test]
    fn v009_idempotent_creates_table_and_indexes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES)
            .expect("baseline tables");
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        // 1. achievements table exists
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='achievements'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "achievements table must exist after v009");

        // 2. CHECK (kind IN ('badge','certificate')) — bogus kind rejected
        let res = conn.execute(
            "INSERT INTO achievements (id, learner_id, track_id, kind, level, \
             mastery_score, payload_json, signature, key_fingerprint, track_topic) \
             VALUES ('a1','l1','t1','bogus','Associate', 0.5, '{}', 'sig', 'fp', 'Kubernetes')",
            [],
        );
        assert!(
            res.is_err(),
            "CHECK (kind IN ('badge','certificate')) must reject 'bogus'"
        );

        // Seed a learner so the FK on learner_id succeeds for the next inserts.
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('l1', 'Test')",
            [],
        )
        .expect("seed learner");

        // 3. UNIQUE (learner_id, track_id, level) — second insert rejected
        conn.execute(
            "INSERT INTO achievements (id, learner_id, track_id, kind, level, \
             mastery_score, payload_json, signature, key_fingerprint, track_topic) \
             VALUES ('a2','l1','t1','badge','Associate', 0.5, '{}', 'sig', 'fp', 'Kubernetes')",
            [],
        )
        .expect("first valid insert must succeed");
        let dup = conn.execute(
            "INSERT INTO achievements (id, learner_id, track_id, kind, level, \
             mastery_score, payload_json, signature, key_fingerprint, track_topic) \
             VALUES ('a3','l1','t1','badge','Associate', 0.5, '{}', 'sig', 'fp', 'Kubernetes')",
            [],
        );
        assert!(
            dup.is_err(),
            "UNIQUE (learner_id, track_id, level) must reject the same triple twice"
        );

        // 4. Both indexes exist
        let learner_idx: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_achievements_learner'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(learner_idx, 1, "idx_achievements_learner must exist");
        let track_idx: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_achievements_track'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(track_idx, 1, "idx_achievements_track must exist");
    }

    /// R4 — Pitfall 5: deleting a learning_tracks row must NOT cascade to
    /// achievements. The achievement is the historical record; track_topic
    /// snapshot preserves the displayable identifier.
    #[test]
    fn v009_track_delete_does_not_cascade_to_achievements() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES)
            .expect("baseline tables");
        apply_migrations(&conn).expect("apply migrations");

        // Seed learner + track.
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('l1', 'T')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) \
             VALUES ('trk1', 'l1', 'Kubernetes', 'devops', 'CKA')",
            [],
        )
        .unwrap();

        // Issue an achievement bound to that track.
        conn.execute(
            "INSERT INTO achievements (id, learner_id, track_id, kind, level, \
             mastery_score, payload_json, signature, key_fingerprint, track_topic) \
             VALUES ('a1','l1','trk1','badge','Associate', 0.5, '{}', 'sig', 'fp', 'Kubernetes')",
            [],
        )
        .expect("insert achievement");

        // Delete the track. Should succeed (no FK on achievements.track_id).
        conn.execute("DELETE FROM learning_tracks WHERE id = 'trk1'", [])
            .expect("track delete must succeed");

        // R4: achievement row MUST remain.
        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM achievements WHERE id = 'a1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            remaining, 1,
            "achievement row must survive track deletion (R4 immutability)"
        );

        // The track_topic snapshot survives the delete.
        let topic: String = conn
            .query_row(
                "SELECT track_topic FROM achievements WHERE id = 'a1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(topic, "Kubernetes", "track_topic snapshot preserves display value");
    }
}
