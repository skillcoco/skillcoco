//! Migration v009 — achievements (Phase 6 Wave 0 RED scaffold)
//!
//! Plan 06-01 (this file) lands the migration *registration* + *idempotency
//! test* only. The `up()` body is intentionally a no-op until Plan 06-02
//! (Wave 1) fills it.
//!
//! When Plan 06-02 lands, `up()` must produce the `achievements` table per
//! D-12 + R4:
//!   id              TEXT PRIMARY KEY,
//!   learner_id      TEXT NOT NULL REFERENCES learner_profiles(id),
//!   track_id        TEXT NOT NULL REFERENCES learning_tracks(id),
//!                   -- NOTE: NO ON DELETE CASCADE (R4 — Pitfall 5).
//!                   -- Deleting a track must NOT destroy the historical
//!                   -- achievement; track_topic is snapshotted below.
//!   pack_id         TEXT,
//!   kind            TEXT NOT NULL CHECK (kind IN ('badge','certificate')),
//!   level           TEXT NOT NULL CHECK (level IN
//!                       ('Associate','Practitioner','Professional','Completion')),
//!   issued_at       TEXT NOT NULL DEFAULT (datetime('now')),
//!   mastery_score   REAL NOT NULL,
//!   payload_json    TEXT NOT NULL,
//!   signature       TEXT NOT NULL,
//!   key_fingerprint TEXT NOT NULL,
//!   track_topic     TEXT NOT NULL,
//!   UNIQUE (learner_id, track_id, level)   -- D-04 immutability + D-03
//!                                            -- once-per-level enforcement.
//!
//! The `v009_idempotent_creates_table_and_indexes` test below asserts the
//! post-condition Plan 06-02 must satisfy. Today it FAILS — that is the
//! RED contract.

use rusqlite::{Connection, Result};

pub const VERSION: i32 = 9;
pub const NAME: &str = "achievements";

/// Apply the v009 migration. Wave 0 stub — no-op. Wave 1 (Plan 06-02)
/// fills the CREATE TABLE block + CHECK constraints + indexes.
pub fn up(_conn: &Connection) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    /// CERT-02 — v009 idempotency + post-condition contract.
    ///
    /// RED contract: asserts the achievements table exists with the
    /// expected columns + CHECK constraints + UNIQUE constraint. Wave 0
    /// `up()` is a no-op, so every assertion below FAILS. Wave 1 fills
    /// the body and this test flips GREEN.
    #[test]
    #[ignore = "Plan 06-02 (Wave 1) fills up() body — RED until then"]
    fn v009_idempotent_creates_table_and_indexes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES)
            .expect("baseline tables");
        apply_migrations(&conn).expect("first apply must succeed");
        apply_migrations(&conn).expect("second apply must succeed (idempotent)");

        // ── 1. achievements table exists ──
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='achievements'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "achievements table must exist after v009");

        // ── 2. CHECK (kind IN ('badge','certificate')) — bogus kind rejected ──
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

        // ── 3. UNIQUE (learner_id, track_id, level) — second insert rejected ──
        // First valid insert (Wave 1 schema must allow this).
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
    }
}
