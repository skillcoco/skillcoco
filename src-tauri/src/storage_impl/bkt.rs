//! Wave 2 (Plan 07-02) — rusqlite-backed impl of
//! [`learnforge_core::bkt::BktStore`].
//!
//! Reads `mastery_level` from the `module_progress` table. SQL lifted
//! verbatim from the prereq-check fragment in
//! `src-tauri/src/learning/path.rs:32-66` (pre-Wave-2). Wave 10 may move
//! this file under a different name; the impl itself is stable.
//!
//! ## Orphan-rule note (Wave 2 deviation from PLAN.md verbatim)
//!
//! The plan specified `impl BktStore for &rusqlite::Connection` directly,
//! but `BktStore` is foreign (from `learnforge_core`) and `Connection` is
//! foreign (from `rusqlite`), so Rust's orphan rule (E0117) rejects that
//! impl: at least one of the trait OR the impl-target type must belong to
//! the current crate. We satisfy this by introducing the local newtype
//! [`SqliteBktStore`], which owns a `&Connection` and carries the impl.
//!
//! The wrapper is zero-cost (single-field tuple struct around a reference)
//! and the call-site ergonomics are preserved by callers constructing
//! `SqliteBktStore(&conn)` and invoking
//! `learnforge_core::path::all_prerequisites_mastered(&store, …)` directly
//! (Wave 10 cleanup; the pre-Wave-10 `crate::learning::path::*` shim was
//! deleted).
//!
//! ## Trust boundary (T-07-05)
//!
//! `rusqlite::Error` is stringified into `BktError::Db` here so
//! `learnforge-core` never depends on rusqlite. `QueryReturnedNoRows` is
//! mapped to `BktError::NotFound` so callers can distinguish "no row" from
//! "I/O failure" without leaking the rusqlite type.

use learnforge_core::bkt::{BktError, BktStore};
use rusqlite::Connection;

/// Rusqlite-backed [`BktStore`] adapter.
///
/// Construct via `SqliteBktStore(&conn)` at the call site; the wrapper holds
/// the connection reference for the duration of the read. The newtype satisfies
/// Rust's orphan-rule requirement that at least one of the trait OR the impl
/// target must be local to the impl-defining crate.
pub struct SqliteBktStore<'a>(pub &'a Connection);

impl<'a> BktStore for SqliteBktStore<'a> {
    fn read_mastery(&self, learner_id: &str, module_id: &str) -> Result<f64, BktError> {
        self.0
            .query_row(
                "SELECT mastery_level FROM module_progress WHERE learner_id = ?1 AND module_id = ?2",
                rusqlite::params![learner_id, module_id],
                |row| row.get::<_, f64>(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => BktError::NotFound {
                    learner_id: learner_id.to_string(),
                    module_id: module_id.to_string(),
                },
                other => BktError::Db(other.to_string()),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE module_progress (
                 id TEXT PRIMARY KEY,
                 module_id TEXT NOT NULL,
                 learner_id TEXT NOT NULL,
                 status TEXT NOT NULL DEFAULT 'locked',
                 score REAL,
                 time_spent INTEGER NOT NULL DEFAULT 0,
                 attempts INTEGER NOT NULL DEFAULT 0,
                 mastery_level REAL NOT NULL DEFAULT 0.0,
                 started_at TEXT,
                 completed_at TEXT,
                 UNIQUE(module_id, learner_id)
             );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn read_mastery_returns_persisted_value() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["p1", "mod-a", "learner-1", 0.85f64],
        )
        .unwrap();

        let store = SqliteBktStore(&conn);
        let mastery = store.read_mastery("learner-1", "mod-a").unwrap();
        assert!((mastery - 0.85).abs() < 1e-9);
    }

    #[test]
    fn read_mastery_missing_row_returns_not_found() {
        let conn = setup_test_db();
        let store = SqliteBktStore(&conn);
        match store.read_mastery("learner-1", "mod-a").unwrap_err() {
            BktError::NotFound {
                learner_id,
                module_id,
            } => {
                assert_eq!(learner_id, "learner-1");
                assert_eq!(module_id, "mod-a");
            }
            other => panic!("expected NotFound, got {:?}", other),
        }
    }
}
