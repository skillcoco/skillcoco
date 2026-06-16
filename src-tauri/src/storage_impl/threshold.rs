//! Wave 4 parking spot — `track_mastery_aggregate` stays in src-tauri
//! because it issues raw SQL via `rusqlite`. The pure predicates
//! (`TrackAggregate`, `which_level_just_crossed`, `levels_met`) moved to
//! `learnforge_core::threshold` during Wave 4.
//!
//! Wave 8 (`07-08-PLAN.md`) lands the `AchievementStore` trait next to
//! the achievements algorithm in `learnforge-core::achievements`. At
//! that point this free function becomes the body of an
//! `AchievementStore::track_mastery_aggregate` method — the SQL is
//! unchanged; only the call seam moves.
//!
//! No cross-wave dependency violation: this file lives in `src-tauri`
//! only; `learnforge_core::threshold` imports nothing from it.

use crate::achievements::AchievementError;
use learnforge_core::threshold::TrackAggregate;
use rusqlite::Connection;

/// Compute the live track aggregate from `module_progress` rows.
///
/// Single SQL query (per RESEARCH.md Pattern 4 performance constraint):
/// counts modules, counts mastered modules, computes avg mastery,
/// detects practical_required from `modules.content_json` via
/// `json_extract`, and checks practical_mastery >= 0.7 for each
/// practical-required module.
///
/// Wave 4 move (07-04): body lifted verbatim from
/// `src-tauri/src/achievements/threshold.rs:99-159` (pre-Wave-4
/// snapshot). Wave 8 will promote this to a trait method.
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
