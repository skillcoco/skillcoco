//! `track_mastery_aggregate` — rusqlite-backed SQL aggregate computing a
//! [`TrackAggregate`] from `module_progress` rows. The pure predicates
//! (`which_level_just_crossed`, `levels_met`) live in
//! `learnforge_core::threshold`.
//!
//! ## Wave 4 ↔ Wave 8 seam (CLOSED)
//!
//! Wave 4 (07-04) parked this free function here pending Wave 8's
//! [`AchievementStore`] trait declaration. Wave 8 (07-08) closed the seam:
//! [`AchievementStore::track_mastery_aggregate`] on
//! [`SqliteAchievementStore`] delegates to the body below — the SQL string
//! is unchanged; only the call shape moved from "free fn called by
//! src-tauri" to "trait method dispatched through a newtype-wrapped
//! `&Connection`".
//!
//! Why the free fn still exists: Wave 4's transitional shim at
//! `src-tauri/src/achievements/threshold.rs` re-exports it (`pub use
//! crate::storage_impl::threshold::track_mastery_aggregate`) and the
//! single intra-crate callsite reaches it through that path. Wave 10
//! grep-and-rewrite will switch every callsite to invoke the trait method
//! through `SqliteAchievementStore(&conn)` and delete this file.
//!
//! No cross-wave dependency violation: this file lives in `src-tauri`
//! only; `learnforge_core::threshold` imports nothing from it. The
//! trait-method delegation is intra-`src-tauri` (storage_impl →
//! storage_impl).
//!
//! [`AchievementStore`]: learnforge_core::achievements::AchievementStore
//! [`AchievementStore::track_mastery_aggregate`]:
//!   learnforge_core::achievements::AchievementStore::track_mastery_aggregate
//! [`SqliteAchievementStore`]: crate::storage_impl::achievements::SqliteAchievementStore

use learnforge_core::achievements::AchievementError;
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
/// Body lifted verbatim from `src-tauri/src/achievements/threshold.rs:99-159`
/// (pre-Wave-4 snapshot). Phase 7 Wave 8 closed the seam:
/// [`SqliteAchievementStore::track_mastery_aggregate`] delegates here.
///
/// [`SqliteAchievementStore::track_mastery_aggregate`]:
///   crate::storage_impl::achievements::SqliteAchievementStore
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
    ).map_err(|e| AchievementError::Db(e.to_string()))?;

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
