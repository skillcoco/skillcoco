//! Transitional re-export — Phase 7 Wave 2 (Plan 07-02) moved the pure DAG
//! pieces to [`learnforge_core::path`] (Pitfall 8 mixed pure/DB split).
//!
//! `all_prerequisites_mastered` remains here as a thin wrapper that resolves
//! the [`BktStore`] impl from `&rusqlite::Connection` (the impl lives in
//! `crate::storage_impl::bkt`) and downgrades the typed
//! [`learnforge_core::bkt::BktError`] back to `String` to preserve the legacy
//! caller signature. Wave 10 deletes this file after rewriting call sites
//! to use `learnforge_core::path::*` directly.
//!
//! `#[deprecated]` is intentionally NOT used on the `pub use` re-exports —
//! rustc may silently ignore the attribute (R5 / Pitfall 6). The reliable
//! cleanup mechanism is the Wave 10 grep-and-rewrite.

use rusqlite::Connection;

pub use learnforge_core::path::{
    EdgeRecord, PathEdge, PathError, PathNode, parse_edges_json as core_parse_edges_json,
    validate_dag as core_validate_dag,
};

// Wave-2 deviation note: the BKT trait impl uses a local newtype
// `SqliteBktStore<'a>(&'a Connection)` instead of `impl BktStore for
// &Connection` directly. The plan-verbatim wording would violate Rust's
// orphan rule (E0117) because both `BktStore` (from learnforge_core) and
// `Connection` (from rusqlite) are foreign. The newtype is zero-cost and
// the wrapper below hides it from the rest of src-tauri.
use crate::storage_impl::bkt::SqliteBktStore;

/// Legacy signature wrapper for `parse_edges_json` that returns `Result<_, String>`
/// (the pre-Wave-2 error type). Call sites stay unchanged through the
/// transition. Wave 10 rewrites callers to `learnforge_core::path::parse_edges_json`
/// directly and consume `PathError`.
pub fn parse_edges_json(edges_json: &str) -> Result<Vec<EdgeRecord>, String> {
    core_parse_edges_json(edges_json).map_err(|e| e.to_string())
}

/// Legacy signature wrapper for `validate_dag` returning `Result<_, String>`.
pub fn validate_dag(nodes: &[PathNode], edges: &[PathEdge]) -> Result<(), String> {
    core_validate_dag(nodes, edges).map_err(|e| e.to_string())
}

/// Legacy signature preserved for src-tauri call sites. Internally delegates
/// to [`learnforge_core::path::all_prerequisites_mastered`] using the
/// `&Connection` [`learnforge_core::bkt::BktStore`] impl from
/// `crate::storage_impl::bkt`.
///
/// Diamond-DAG correctness + the legacy `.unwrap_or(0.0)` "missing row treated
/// as not mastered" semantic are both preserved (the core fn absorbs
/// `BktError::NotFound` as `0.0` internally).
pub fn all_prerequisites_mastered(
    conn: &Connection,
    learner_id: &str,
    module_id: &str,
    edges: &[EdgeRecord],
) -> Result<bool, String> {
    let store = SqliteBktStore(conn);
    learnforge_core::path::all_prerequisites_mastered(&store, learner_id, module_id, edges)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    //! Wave 2 retains the rusqlite-backed integration tests here — the
    //! pure-Rust algorithm tests (with a `BktStore` stub) moved to
    //! `learnforge_core::path::tests`. These tests exercise the actual
    //! &Connection → BktStore path through the wrapper fn so the cross-crate
    //! seam stays under test.

    use super::*;

    fn setup_test_db_for_path() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE learner_profiles (id TEXT PRIMARY KEY);
             CREATE TABLE learning_tracks (id TEXT PRIMARY KEY, learner_id TEXT, topic TEXT, domain_module TEXT, goal TEXT);
             CREATE TABLE learning_paths (id TEXT PRIMARY KEY, track_id TEXT, modules_json TEXT DEFAULT '[]', edges_json TEXT DEFAULT '[]', generated_by_model TEXT);
             CREATE TABLE modules (id TEXT PRIMARY KEY, path_id TEXT, title TEXT, ordering INTEGER DEFAULT 0);
             CREATE TABLE module_progress (
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
        ).unwrap();
        conn
    }

    #[test]
    fn all_prereqs_mastered_linear_chain() {
        let conn = setup_test_db_for_path();
        let learner = "learner-1";

        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level, status) VALUES ('p1', 'a', ?1, 0.8, 'completed')", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level, status) VALUES ('p2', 'b', ?1, 0.0, 'locked')", [learner]).unwrap();

        let edges = vec![EdgeRecord {
            from: "a".into(),
            to: "b".into(),
            edge_type: "prerequisite".into(),
        }];

        assert!(all_prerequisites_mastered(&conn, learner, "b", &edges).unwrap());
    }

    #[test]
    fn all_prereqs_mastered_diamond_partial() {
        let conn = setup_test_db_for_path();
        let learner = "learner-1";

        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p1','a',?1,0.8)", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p2','b',?1,0.8)", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p3','c',?1,0.2)", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p4','d',?1,0.0)", [learner]).unwrap();

        let edges = vec![
            EdgeRecord { from: "a".into(), to: "b".into(), edge_type: "prerequisite".into() },
            EdgeRecord { from: "a".into(), to: "c".into(), edge_type: "prerequisite".into() },
            EdgeRecord { from: "b".into(), to: "d".into(), edge_type: "prerequisite".into() },
            EdgeRecord { from: "c".into(), to: "d".into(), edge_type: "prerequisite".into() },
        ];

        // d has two prereqs (b and c); c is not mastered → false
        assert!(!all_prerequisites_mastered(&conn, learner, "d", &edges).unwrap());
    }

    #[test]
    fn all_prereqs_mastered_diamond_complete() {
        let conn = setup_test_db_for_path();
        let learner = "learner-1";

        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p1','b',?1,0.8)", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p2','c',?1,0.75)", [learner]).unwrap();
        conn.execute("INSERT INTO module_progress (id, module_id, learner_id, mastery_level) VALUES ('p3','d',?1,0.0)", [learner]).unwrap();

        let edges = vec![
            EdgeRecord { from: "b".into(), to: "d".into(), edge_type: "prerequisite".into() },
            EdgeRecord { from: "c".into(), to: "d".into(), edge_type: "prerequisite".into() },
        ];

        assert!(all_prerequisites_mastered(&conn, learner, "d", &edges).unwrap());
    }
}
