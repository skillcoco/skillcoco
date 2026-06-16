//! Phase 6 — threshold helpers (D-01 + D-02 + A9).
//!
//! Three skill tiers, uniform across all packs in Phase 6:
//!   - Associate:    25% of modules at BKT mastery >= 0.7
//!   - Practitioner: 60% of modules at mastery >= 0.7
//!   - Professional: 100% of modules at mastery >= 0.7
//!                   AND average mastery across the track >= 0.85
//!                   AND every practical_required lab passed
//!
//! A9: `mastery_level` is the live high-water mark from module_progress;
//! the achievements row preserves the historical proof even if mastery
//! decays later (R4).

use super::AchievementError;
use rusqlite::Connection;

/// Per-track snapshot used to decide which levels (if any) are met now.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackAggregate {
    pub modules_total: usize,
    pub modules_mastered: usize,
    pub avg_mastery: f64,
    pub all_practical_labs_passed: bool,
    pub has_practical_required: bool,
}

/// Fraction of modules mastered (0.0 on empty tracks).
fn ratio(a: &TrackAggregate) -> f64 {
    if a.modules_total == 0 {
        0.0
    } else {
        a.modules_mastered as f64 / a.modules_total as f64
    }
}

/// Pure predicate: does `agg` satisfy the Professional gate?
fn is_professional(agg: &TrackAggregate) -> bool {
    agg.modules_total > 0
        && agg.modules_mastered == agg.modules_total
        && agg.avg_mastery >= 0.85
        && (!agg.has_practical_required || agg.all_practical_labs_passed)
}

/// Compare a previous aggregate to the current aggregate and return the
/// HIGHEST level (if any) the learner just crossed. Returns `None` when no
/// new level is reached.
///
/// Note: when a learner jumps multiple tiers in a single update (rare —
/// requires a batch mastery update from 0% to >= 60%), this returns the
/// highest newly-crossed tier. `maybe_issue` separately uses
/// `levels_met` + the DB to insert any previously-missed badges, so the
/// caller never relies on this function alone.
pub fn which_level_just_crossed(
    prev: &TrackAggregate,
    curr: &TrackAggregate,
) -> Option<&'static str> {
    // Professional first (highest tier).
    if is_professional(curr) && !is_professional(prev) {
        return Some("Professional");
    }
    let r_curr = ratio(curr);
    let r_prev = ratio(prev);
    if r_curr >= 0.60 && r_prev < 0.60 {
        return Some("Practitioner");
    }
    if r_curr >= 0.25 && r_prev < 0.25 {
        return Some("Associate");
    }
    None
}

/// Return ALL levels currently met by `agg` (pure logic). `maybe_issue`
/// subtracts already-issued levels via the achievements row.
pub fn levels_met(agg: &TrackAggregate) -> Vec<&'static str> {
    let mut out = Vec::new();
    if agg.modules_total == 0 {
        return out;
    }
    let r = ratio(agg);
    if r >= 0.25 {
        out.push("Associate");
    }
    if r >= 0.60 {
        out.push("Practitioner");
    }
    if is_professional(agg) {
        out.push("Professional");
    }
    out
}

/// Compute the live track aggregate from `module_progress` rows.
///
/// Single SQL query (per RESEARCH.md Pattern 4 performance constraint):
/// counts modules, counts mastered modules, computes avg mastery,
/// detects practical_required from `modules.content_json` via
/// `json_extract`, and checks practical_mastery >= 0.7 for each
/// practical-required module.
pub fn track_mastery_aggregate(
    conn: &Connection,
    track_id: &str,
    learner_id: &str,
) -> Result<TrackAggregate, AchievementError> {
    // We need:
    //   - modules_total: COUNT(modules in track)
    //   - modules_mastered: COUNT(mastery_level >= 0.7)
    //   - avg_mastery: AVG(mastery_level) over ALL modules (0.0 for modules
    //     without a progress row — COALESCE on the LEFT JOIN)
    //   - has_practical_required: TRUE if any module has content_json
    //     practical_required = true
    //   - all_practical_labs_passed: every practical_required module has
    //     practical_mastery >= 0.7
    //
    // `modules.path_id -> learning_paths.id`; `learning_paths.track_id`
    // is the join key. A single quiz track may have multiple path
    // versions; we aggregate over the latest path version only
    // (ORDER BY learning_paths.version DESC LIMIT 1).
    let row: (i64, i64, f64, i64, i64) = conn.query_row(
        r#"
        WITH latest_path AS (
            SELECT id FROM learning_paths
             WHERE track_id = ?1
             ORDER BY version DESC
             LIMIT 1
        )
        SELECT
            COUNT(m.id) AS modules_total,
            COALESCE(SUM(CASE WHEN mp.mastery_level >= 0.7 THEN 1 ELSE 0 END), 0) AS modules_mastered,
            COALESCE(AVG(COALESCE(mp.mastery_level, 0.0)), 0.0) AS avg_mastery,
            COALESCE(SUM(CASE WHEN json_extract(m.content_json, '$.practical_required') = 1
                                OR json_extract(m.content_json, '$.practical_required') = 'true'
                              THEN 1 ELSE 0 END), 0) AS practical_required_count,
            COALESCE(SUM(CASE WHEN (json_extract(m.content_json, '$.practical_required') = 1
                                    OR json_extract(m.content_json, '$.practical_required') = 'true')
                              AND COALESCE(mp.practical_mastery, 0.0) >= 0.7
                              THEN 1 ELSE 0 END), 0) AS practical_labs_passed
        FROM modules m
        INNER JOIN latest_path lp ON m.path_id = lp.id
        LEFT JOIN module_progress mp
                  ON mp.module_id = m.id AND mp.learner_id = ?2
        "#,
        rusqlite::params![track_id, learner_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;

    let modules_total = row.0 as usize;
    let modules_mastered = row.1 as usize;
    let avg_mastery = row.2;
    let practical_required_count = row.3 as usize;
    let practical_labs_passed = row.4 as usize;

    Ok(TrackAggregate {
        modules_total,
        modules_mastered,
        avg_mastery,
        has_practical_required: practical_required_count > 0,
        all_practical_labs_passed: practical_required_count == practical_labs_passed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::apply_migrations;
    use crate::db::schema;
    use rusqlite::Connection;

    fn zero() -> TrackAggregate {
        TrackAggregate {
            modules_total: 4,
            modules_mastered: 0,
            avg_mastery: 0.0,
            all_practical_labs_passed: false,
            has_practical_required: false,
        }
    }

    #[test]
    fn associate_at_25_percent() {
        // 1/4 = 25% — Associate threshold (D-02).
        let prev = zero();
        let curr = TrackAggregate {
            modules_mastered: 1,
            avg_mastery: 0.7,
            ..zero()
        };
        assert_eq!(
            which_level_just_crossed(&prev, &curr),
            Some("Associate"),
            "1/4 modules mastered must cross Associate threshold"
        );
    }

    #[test]
    fn associate_already_crossed_returns_none() {
        // prev already met Associate (1/4); curr is now 2/4. No NEW crossing.
        let prev = TrackAggregate {
            modules_mastered: 1,
            avg_mastery: 0.7,
            ..zero()
        };
        let curr = TrackAggregate {
            modules_mastered: 2,
            avg_mastery: 0.7,
            ..zero()
        };
        assert_eq!(which_level_just_crossed(&prev, &curr), None);
    }

    #[test]
    fn practitioner_at_60_percent() {
        let prev = TrackAggregate {
            modules_total: 10,
            modules_mastered: 4,
            avg_mastery: 0.7,
            ..zero()
        };
        let curr = TrackAggregate {
            modules_total: 10,
            modules_mastered: 6,
            avg_mastery: 0.72,
            ..zero()
        };
        assert_eq!(which_level_just_crossed(&prev, &curr), Some("Practitioner"));
    }

    #[test]
    fn professional_requires_avg_and_labs() {
        let prev = TrackAggregate {
            modules_total: 4, modules_mastered: 3, avg_mastery: 0.80,
            all_practical_labs_passed: false, has_practical_required: true,
        };
        let curr_full = |avg, labs| TrackAggregate {
            modules_total: 4, modules_mastered: 4, avg_mastery: avg,
            all_practical_labs_passed: labs, has_practical_required: true,
        };
        // Missing labs / low avg — NOT Professional.
        assert_eq!(which_level_just_crossed(&prev, &curr_full(0.90, false)), None, "missing labs");
        assert_eq!(which_level_just_crossed(&prev, &curr_full(0.80, true)), None, "avg below 0.85");
        // Both gates pass — Professional.
        assert_eq!(which_level_just_crossed(&prev, &curr_full(0.90, true)), Some("Professional"));
    }

    #[test]
    fn levels_met_returns_all_now_met() {
        let agg = TrackAggregate {
            modules_total: 10,
            modules_mastered: 6,
            avg_mastery: 0.72,
            ..zero()
        };
        // 60% met -> Associate AND Practitioner; NOT Professional (modules_mastered != total).
        let levels = levels_met(&agg);
        assert!(levels.contains(&"Associate"));
        assert!(levels.contains(&"Practitioner"));
        assert!(!levels.contains(&"Professional"));
    }

    #[test]
    fn levels_met_includes_professional_when_gates_pass() {
        let agg = TrackAggregate {
            modules_total: 4,
            modules_mastered: 4,
            avg_mastery: 0.90,
            all_practical_labs_passed: true,
            has_practical_required: true,
        };
        let levels = levels_met(&agg);
        assert_eq!(levels, vec!["Associate", "Practitioner", "Professional"]);
    }

    // ─── SQL aggregate fixture tests ───

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn
    }

    /// modules: (id, mastery_level, practical_required, practical_mastery).
    fn seed_track(conn: &Connection, track_id: &str, learner_id: &str, modules: &[(&str, f64, bool, f64)]) {
        conn.execute(
            "INSERT OR IGNORE INTO learner_profiles (id, display_name) VALUES (?1, 'Test')",
            [learner_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES (?1, ?2, 'Kubernetes', 'devops', 'CKA')",
            rusqlite::params![track_id, learner_id],
        ).unwrap();
        let path_id = format!("path-{}", track_id);
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES (?1, ?2, 1, '[]', '[]', 'test')",
            rusqlite::params![path_id, track_id],
        ).unwrap();
        for (i, (mid, ml, pr, pm)) in modules.iter().enumerate() {
            let content_json = if *pr { r#"{"practical_required": true}"# } else { "{}" };
            conn.execute(
                "INSERT INTO modules (id, path_id, title, ordering, content_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![mid, path_id, format!("M{}", i), i as i64, content_json],
            ).unwrap();
            conn.execute(
                "INSERT INTO module_progress (id, module_id, learner_id, status, mastery_level, practical_mastery) VALUES (?1, ?2, ?3, 'in_progress', ?4, ?5)",
                rusqlite::params![format!("mp-{}", mid), mid, learner_id, ml, pm],
            ).unwrap();
        }
    }

    #[test]
    fn track_mastery_aggregate_reads_module_progress() {
        let conn = fresh_conn();
        seed_track(&conn, "trk1", "lp1", &[
            ("m1", 0.80, false, 0.0),
            ("m2", 0.30, true, 0.0),
            ("m3", 0.75, false, 0.0),
            ("m4", 0.5, false, 0.0),
        ]);
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
        seed_track(&conn, "trk2", "lp1", &[
            ("m1", 0.90, false, 0.0), ("m2", 0.92, false, 0.0),
            ("m3", 0.95, false, 0.0), ("m4", 0.88, false, 0.0),
        ]);
        let agg = track_mastery_aggregate(&conn, "trk2", "lp1").expect("aggregate");
        assert_eq!(agg.modules_mastered, 4);
        assert!(!agg.has_practical_required);
        assert!(agg.all_practical_labs_passed, "0 == 0 trivially true");
        assert!(is_professional(&agg));
    }

    #[test]
    fn track_mastery_aggregate_empty_track_returns_zeros() {
        let conn = fresh_conn();
        conn.execute("INSERT INTO learner_profiles (id, display_name) VALUES ('lp1', 'T')", []).unwrap();
        conn.execute(
            "INSERT INTO learning_tracks (id, learner_id, topic, domain_module, goal) VALUES ('trk-empty', 'lp1', 'X', 'd', 'g')",
            []
        ).unwrap();
        conn.execute(
            "INSERT INTO learning_paths (id, track_id, version, edges_json, modules_json, generated_by_model) VALUES ('p-empty', 'trk-empty', 1, '[]', '[]', 'test')",
            []
        ).unwrap();
        let agg = track_mastery_aggregate(&conn, "trk-empty", "lp1").expect("aggregate");
        assert_eq!(agg.modules_total, 0);
        assert_eq!(levels_met(&agg).len(), 0);
    }
}
