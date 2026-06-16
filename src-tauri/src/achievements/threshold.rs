//! Transitional shim — Phase 7 Wave 4 moved the pure threshold predicates
//! (`TrackAggregate`, `which_level_just_crossed`, `levels_met`) to
//! `learnforge_core::threshold`. The SQL aggregate `track_mastery_aggregate`
//! moved to `crate::storage_impl::threshold` (parked until Wave 8 promotes
//! it into an `AchievementStore` trait method).
//!
//! This file re-exports both surfaces so the existing call site
//! (`achievements::mod::maybe_issue`) continues to compile unchanged.
//!
//! No `#[deprecated]` — rustc silently ignores it on `pub use` (R5 /
//! Pitfall 6 from 07-RESEARCH.md). Wave 10 grep-and-rewrite is the
//! eventual cleanup.

pub use crate::storage_impl::threshold::track_mastery_aggregate;
pub use learnforge_core::threshold::{levels_met, which_level_just_crossed, TrackAggregate};

#[cfg(test)]
mod tests {
    //! SQL-touching tests stay in src-tauri because they need an
    //! in-memory `rusqlite::Connection`. The pure-predicate tests moved
    //! to `learnforge-core/src/threshold.rs` with the algorithm.

    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    /// modules: (id, mastery_level, practical_required, practical_mastery).
    fn seed_track(
        conn: &Connection,
        track_id: &str,
        learner_id: &str,
        modules: &[(&str, f64, bool, f64)],
    ) {
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id, display_name) VALUES (?1, 'Test')",
            [learner_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES (?1, ?2, 'Kubernetes', 'devops', 'CKA')",
            rusqlite::params![track_id, learner_id],
        )
        .unwrap();
        let path_id = format!("path-{}", track_id);
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES (?1, ?2, 1, '[]', '[]', 'test')",
            rusqlite::params![path_id, track_id],
        )
        .unwrap();
        for (i, (mid, ml, pr, pm)) in modules.iter().enumerate() {
            let content_json = if *pr { r#"{"practical_required": true}"# } else { "{}" };
            conn.execute(
                "INSERT INTO modules (id, path_id, title, ordering, content_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![mid, path_id, format!("M{}", i), i as i64, content_json],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level, practical_mastery) VALUES (?1, ?2, ?3, 'in_progress', ?4, ?5)",
                rusqlite::params![format!("mp-{}", mid), mid, learner_id, ml, pm],
            )
            .unwrap();
        }
    }

    #[test]
    fn track_mastery_aggregate_reads_module_progress() {
        let conn = fresh_conn();
        seed_track(
            &conn,
            "trk1",
            "lp1",
            &[
                ("m1", 0.80, false, 0.0),
                ("m2", 0.30, true, 0.0),
                ("m3", 0.75, false, 0.0),
                ("m4", 0.5, false, 0.0),
            ],
        );
        let agg = track_mastery_aggregate(&conn, "trk1", "lp1").expect("aggregate");
        assert_eq!(agg.modules_total, 4);
        assert_eq!(agg.modules_mastered, 2, "m1 + m3 above 0.7");
        assert!((agg.avg_mastery - 0.5875).abs() < 1e-9, "avg = 2.35 / 4");
        assert!(agg.has_practical_required);
        assert!(!agg.all_practical_labs_passed, "m2 lab not passed");
    }

    #[test]
    fn track_mastery_aggregate_no_practical_required() {
        let conn = fresh_conn();
        seed_track(
            &conn,
            "trk2",
            "lp1",
            &[
                ("m1", 0.90, false, 0.0),
                ("m2", 0.92, false, 0.0),
                ("m3", 0.95, false, 0.0),
                ("m4", 0.88, false, 0.0),
            ],
        );
        let agg = track_mastery_aggregate(&conn, "trk2", "lp1").expect("aggregate");
        assert_eq!(agg.modules_mastered, 4);
        assert!(!agg.has_practical_required);
        assert!(agg.all_practical_labs_passed, "0 == 0 trivially true");
        // Re-derive the Professional gate locally from the core predicates —
        // we no longer have access to the (private) `is_professional` helper.
        let levels = levels_met(&agg);
        assert!(
            levels.contains(&"Professional"),
            "track with 4/4 mastered + avg >= 0.85 + no practical-required must hit Professional"
        );
    }

    #[test]
    fn track_mastery_aggregate_empty_track_returns_zeros() {
        let conn = fresh_conn();
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'T')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk-empty', 'lp1', 'X', 'd', 'g')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES ('p-empty', 'trk-empty', 1, '[]', '[]', 'test')",
            [],
        )
        .unwrap();
        let agg = track_mastery_aggregate(&conn, "trk-empty", "lp1").expect("aggregate");
        assert_eq!(agg.modules_total, 0);
        assert_eq!(levels_met(&agg).len(), 0);
    }
}
