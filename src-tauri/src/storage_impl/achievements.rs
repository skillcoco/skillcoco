//! `SqliteAchievementStore` — rusqlite-backed [`AchievementStore`] impl.
//!
//! Phase 7 Wave 8 (07-08): the eighth and final application of the
//! per-module storage-trait recipe. The trait lives in
//! `skillcoco-core/src/achievements.rs`; this file binds it to
//! `rusqlite::Connection` via the same orphan-rule newtype pattern
//! used in Waves 4-7 (`SqliteSrStore`, `SqliteBlockStore`,
//! `SqlitePackStore`, etc.).
//!
//! ## Architecture
//!
//! - **`track_mastery_aggregate`** — delegates to the Wave 4 parked free
//!   fn in `crate::storage_impl::threshold::track_mastery_aggregate`.
//!   Wave 8 closes the Wave-4 forward-declared seam; the SQL string
//!   itself is unchanged (verbatim from the pre-Wave-4 mod.rs body).
//! - Every other trait method body is the SQL string lifted **verbatim**
//!   from the pre-Wave-8 free-fn implementations in
//!   `src-tauri/src/achievements/mod.rs`.
//!
//! ## Error envelope
//!
//! The trait surface returns `Result<_, AchievementError>` (the core
//! error type). `rusqlite::Error` is converted at every call site via
//! the [`db_err`] helper (`AchievementError::Db(e.to_string())`).
//!
//! Rust's orphan rule forbids `impl From<rusqlite::Error> for
//! AchievementError` from living in either crate (both are foreign
//! types from src-tauri's perspective; both are foreign from
//! skillcoco-core's perspective for the rusqlite half). Wave 5 hit
//! the same wall and solved it the same way for `BktError` /
//! `SrError` / `PackError`: stringify at the trust boundary.
//!
//! ## Newtype rationale (orphan rule)
//!
//! Cargo's coherence rules forbid `impl AchievementStore for
//! &Connection` because both the trait (`skillcoco_core::achievements`)
//! and the type (`rusqlite::Connection`) live in upstream crates.
//! The fix is the newtype `SqliteAchievementStore<'a>(pub
//! &'a Connection)` declared here — same pattern as `SqlitePackStore`
//! / `SqliteBlockStore` / `SqliteSrStore`.

use skillcoco_core::achievements::{
    Achievement, AchievementError, AchievementStore, IssuanceContext,
};
use skillcoco_core::threshold::TrackAggregate;
use rusqlite::Connection;

/// Stringify a [`rusqlite::Error`] into [`AchievementError::Db`] at the
/// trust boundary. Orphan-rule mitigation — see module-level docs.
#[inline]
fn db_err(e: rusqlite::Error) -> AchievementError {
    AchievementError::Db(e.to_string())
}

/// Newtype wrapping a borrowed [`rusqlite::Connection`] so we can carry
/// the [`AchievementStore`] trait surface. Cheap to construct
/// (`SqliteAchievementStore(&conn)`).
pub struct SqliteAchievementStore<'a>(pub &'a Connection);

impl<'a> AchievementStore for SqliteAchievementStore<'a> {
    fn track_mastery_aggregate(
        &self,
        track_id: &str,
        learner_id: &str,
    ) -> Result<TrackAggregate, AchievementError> {
        // Wave 8 seam closure — delegate to the Wave 4 parked free fn.
        // The SQL body lives there, unchanged since Wave 4.
        crate::storage_impl::threshold::track_mastery_aggregate(self.0, track_id, learner_id)
    }

    fn existing_levels(
        &self,
        learner_id: &str,
        track_id: &str,
    ) -> Result<Vec<String>, AchievementError> {
        // Verbatim from pre-Wave-8 src-tauri/src/achievements/mod.rs:230-238.
        let mut stmt = self
            .0
            .prepare("SELECT level FROM achievements WHERE learner_id = ?1 AND track_id = ?2")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([learner_id, track_id], |r| r.get::<_, String>(0))
            .map_err(db_err)?
            .filter_map(Result::ok)
            .collect();
        Ok(rows)
    }

    fn insert_achievement_or_ignore(
        &self,
        a: &Achievement,
    ) -> Result<bool, AchievementError> {
        // Verbatim from pre-Wave-8 src-tauri/src/achievements/mod.rs:280-299.
        let changed = self
            .0
            .execute(
                "INSERT OR IGNORE INTO achievements
                (id, learner_id, track_id, pack_id, kind, level, issued_at,
                 mastery_score, payload_json, signature, key_fingerprint, track_topic)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                rusqlite::params![
                    a.id,
                    a.learner_id,
                    a.track_id,
                    a.pack_id,
                    a.kind,
                    a.level,
                    a.issued_at,
                    a.mastery_score,
                    a.payload_json,
                    a.signature,
                    a.key_fingerprint,
                    a.track_topic,
                ],
            )
            .map_err(db_err)?;
        Ok(changed > 0)
    }

    fn lookup_issuance_context(
        &self,
        track_id: &str,
        learner_id: &str,
    ) -> Result<IssuanceContext, AchievementError> {
        // Verbatim from pre-Wave-8 src-tauri/src/achievements/mod.rs:96-133.
        let learner_display: String = self
            .0
            .query_row(
                "SELECT COALESCE(display_name, 'Learner') FROM learner_profiles WHERE id = ?1",
                [learner_id],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "Learner".to_string());
        let track_topic: String = self
            .0
            .query_row(
                "SELECT topic FROM learning_tracks WHERE id = ?1",
                [track_id],
                |r| r.get(0),
            )
            .map_err(|e| {
                AchievementError::Validation(format!("track {} not found: {}", track_id, e))
            })?;

        // CR-03 — derive pack_id from the latest learning_paths row.
        let pack_id: Option<String> = self
            .0
            .query_row(
                "SELECT generated_by_model FROM learning_paths \
                 WHERE track_id = ?1 \
                 ORDER BY version DESC LIMIT 1",
                [track_id],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .and_then(|m| m.strip_prefix("topic-pack:").map(|s| s.to_string()));

        Ok(IssuanceContext {
            learner_display,
            track_topic,
            pack_id,
        })
    }

    fn list_for_learner(&self) -> Result<Vec<Achievement>, AchievementError> {
        // Verbatim from pre-Wave-8 src-tauri/src/achievements/mod.rs:348-374.
        let mut stmt = self
            .0
            .prepare(
                "SELECT id, learner_id, track_id, pack_id, kind, level, issued_at,
                    mastery_score, payload_json, signature, key_fingerprint, track_topic
             FROM achievements
             ORDER BY issued_at DESC, id ASC",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Achievement {
                    id: r.get(0)?,
                    learner_id: r.get(1)?,
                    track_id: r.get(2)?,
                    pack_id: r.get(3)?,
                    kind: r.get(4)?,
                    level: r.get(5)?,
                    issued_at: r.get(6)?,
                    mastery_score: r.get(7)?,
                    payload_json: r.get(8)?,
                    signature: r.get(9)?,
                    key_fingerprint: r.get(10)?,
                    track_topic: r.get(11)?,
                })
            })
            .map_err(db_err)?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        Ok(rows)
    }

    fn lookup_achievement(&self, id: &str) -> Result<Achievement, AchievementError> {
        // Verbatim from pre-Wave-8 src-tauri/src/achievements/mod.rs:310-339.
        self.0
            .query_row(
                "SELECT id, learner_id, track_id, pack_id, kind, level, issued_at,
                        mastery_score, payload_json, signature, key_fingerprint, track_topic
                 FROM achievements WHERE id = ?1",
                [id],
                |r| {
                    Ok(Achievement {
                        id: r.get(0)?,
                        learner_id: r.get(1)?,
                        track_id: r.get(2)?,
                        pack_id: r.get(3)?,
                        kind: r.get(4)?,
                        level: r.get(5)?,
                        issued_at: r.get(6)?,
                        mastery_score: r.get(7)?,
                        payload_json: r.get(8)?,
                        signature: r.get(9)?,
                        key_fingerprint: r.get(10)?,
                        track_topic: r.get(11)?,
                    })
                },
            )
            .map_err(|e| {
                AchievementError::Validation(format!("achievement {} not found: {}", id, e))
            })
    }

    fn earned_badge_levels(
        &self,
        track_id: &str,
        learner_id: &str,
    ) -> Result<Vec<String>, AchievementError> {
        // Verbatim from pre-Wave-8 src-tauri/src/achievements/mod.rs:387-394.
        let mut stmt = self
            .0
            .prepare(
                "SELECT level FROM achievements
             WHERE learner_id = ?1 AND track_id = ?2 AND kind = 'badge'",
            )
            .map_err(db_err)?;
        let rows: Vec<String> = stmt
            .query_map([learner_id, track_id], |r| r.get::<_, String>(0))
            .map_err(db_err)?
            .filter_map(Result::ok)
            .collect();
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    //! SQL-touching tests for the rusqlite-backed AchievementStore.
    //! Pure-algorithm tests live in
    //! `skillcoco_core::achievements::tests` (run against inline
    //! stubs).

    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use skillcoco_core::achievements::{
        Achievement, AchievementError, AchievementStore, IssuanceContext,
    };
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    fn sample_ach(id: &str, level: &str, kind: &str, issued_at: &str) -> Achievement {
        Achievement {
            id: id.to_string(),
            learner_id: "lp1".to_string(),
            track_id: "trk1".to_string(),
            pack_id: None,
            kind: kind.to_string(),
            level: level.to_string(),
            issued_at: issued_at.to_string(),
            mastery_score: 0.85,
            payload_json: "{}".to_string(),
            signature: "deadbeef".to_string(),
            key_fingerprint: "deadbeef".to_string(),
            track_topic: "Kubernetes".to_string(),
        }
    }

    fn seed_learner(conn: &Connection) {
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id, display_name) VALUES ('lp1', 'Ada')",
            [],
        )
        .unwrap();
    }

    fn seed_track_with_pack(conn: &Connection, generated_by_model: &str) {
        seed_learner(conn);
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk1', 'lp1', 'Kubernetes', 'devops', 'CKA')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES ('p-trk1', 'trk1', 1, '[]', '[]', ?1)",
            [generated_by_model],
        )
        .unwrap();
    }

    #[test]
    fn insert_and_existing_levels_roundtrip() {
        let conn = fresh_conn();
        seed_learner(&conn);
        let store = SqliteAchievementStore(&conn);
        let a = sample_ach("a1", "Associate", "badge", "2026-06-15T10:00:00Z");
        let inserted = store.insert_achievement_or_ignore(&a).expect("insert");
        assert!(inserted, "first insert must report true");
        let levels = store.existing_levels("lp1", "trk1").expect("read");
        assert_eq!(levels, vec!["Associate".to_string()]);
    }

    #[test]
    fn insert_or_ignore_suppresses_duplicate_level() {
        let conn = fresh_conn();
        seed_learner(&conn);
        let store = SqliteAchievementStore(&conn);
        let a1 = sample_ach("a1", "Associate", "badge", "2026-06-15T10:00:00Z");
        let a2 = sample_ach("a2", "Associate", "badge", "2026-06-16T10:00:00Z"); // same level
        assert!(store.insert_achievement_or_ignore(&a1).expect("first"));
        assert!(
            !store.insert_achievement_or_ignore(&a2).expect("second"),
            "duplicate level must report inserted=false"
        );
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM achievements WHERE level='Associate'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "UNIQUE constraint preserved");
    }

    #[test]
    fn list_for_learner_sorted_desc() {
        let conn = fresh_conn();
        seed_learner(&conn);
        let store = SqliteAchievementStore(&conn);
        store
            .insert_achievement_or_ignore(&sample_ach(
                "a1", "Associate", "badge", "2026-06-10T00:00:00Z",
            ))
            .unwrap();
        store
            .insert_achievement_or_ignore(&sample_ach(
                "a2", "Practitioner", "badge", "2026-06-12T00:00:00Z",
            ))
            .unwrap();
        store
            .insert_achievement_or_ignore(&sample_ach(
                "a3", "Professional", "badge", "2026-06-15T00:00:00Z",
            ))
            .unwrap();
        let rows = store.list_for_learner().expect("list");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].id, "a3");
        assert_eq!(rows[1].id, "a2");
        assert_eq!(rows[2].id, "a1");
    }

    #[test]
    fn lookup_achievement_returns_validation_error_on_miss() {
        let conn = fresh_conn();
        seed_learner(&conn);
        let store = SqliteAchievementStore(&conn);
        let err = store.lookup_achievement("nope").unwrap_err();
        match err {
            AchievementError::Validation(_) => {}
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn lookup_issuance_context_propagates_pack_id() {
        let conn = fresh_conn();
        seed_track_with_pack(&conn, "topic-pack:k8s-fundamentals");
        let store = SqliteAchievementStore(&conn);
        let ctx: IssuanceContext = store
            .lookup_issuance_context("trk1", "lp1")
            .expect("context");
        assert_eq!(ctx.learner_display, "Ada");
        assert_eq!(ctx.track_topic, "Kubernetes");
        assert_eq!(ctx.pack_id.as_deref(), Some("k8s-fundamentals"));
    }

    #[test]
    fn lookup_issuance_context_returns_none_pack_id_for_ai_track() {
        let conn = fresh_conn();
        seed_track_with_pack(&conn, "gpt-4o-mini");
        let store = SqliteAchievementStore(&conn);
        let ctx = store.lookup_issuance_context("trk1", "lp1").expect("ctx");
        assert!(ctx.pack_id.is_none());
    }

    #[test]
    fn earned_badge_levels_filters_by_kind() {
        let conn = fresh_conn();
        seed_learner(&conn);
        let store = SqliteAchievementStore(&conn);
        store
            .insert_achievement_or_ignore(&sample_ach(
                "a1", "Associate", "badge", "2026-06-10T00:00:00Z",
            ))
            .unwrap();
        store
            .insert_achievement_or_ignore(&sample_ach(
                "a2", "Completion", "certificate", "2026-06-15T00:00:00Z",
            ))
            .unwrap();
        let levels = store
            .earned_badge_levels("trk1", "lp1")
            .expect("badge levels");
        assert_eq!(levels, vec!["Associate".to_string()]);
    }

    /// Object-safety smoke — the trait must be usable as `&dyn
    /// AchievementStore`. Required by any future IPC layer that holds
    /// a boxed store.
    #[test]
    fn sqlite_achievement_store_is_object_safe() {
        let conn = fresh_conn();
        seed_learner(&conn);
        let store = SqliteAchievementStore(&conn);
        let dyn_store: &dyn AchievementStore = &store;
        let _ = dyn_store.list_for_learner().unwrap();
    }
}
