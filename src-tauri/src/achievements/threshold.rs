//! Phase 6 (Certification) — pure threshold helpers.
//!
//! Three skill tiers, uniform across all packs in Phase 6:
//!   - Associate:    25% of modules at BKT mastery >= 0.7
//!   - Practitioner: 60% of modules at mastery >= 0.7
//!   - Professional: 100% of modules at mastery >= 0.7
//!                   AND average mastery >= 0.85
//!                   AND all practical_required labs passed
//!
//! Decisions: D-01 (taxonomy), D-02 (formulas), A9 (mastery_level snapshot
//! IS the achievement — read live from module_progress; the achievements
//! row preserves the historical proof even if mastery decays later).
//!
//! Wave 0 stub returns `None` for every call. Wave 1 (Plan 06-02) fills the
//! arithmetic.

use super::AchievementError;
use rusqlite::Connection;

/// Per-track snapshot used to decide which level (if any) was just crossed.
/// Constructed by `track_mastery_aggregate` against `module_progress` rows
/// for the (track_id, learner_id) pair.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackAggregate {
    /// Total modules in the track (modulesJson.length).
    pub modules_total: usize,
    /// Modules with current mastery_level >= 0.7 (D-02 threshold).
    pub modules_mastered: usize,
    /// Mean of mastery_level over ALL modules (mastered or not).
    pub avg_mastery: f64,
    /// True iff every module flagged practical_required has its lab
    /// `module_progress.practical_mastery >= 1.0` (Phase 03.1 v006 column).
    pub all_practical_labs_passed: bool,
    /// True iff the track has at least one module with
    /// `practical_required: true` (Phase 03.1).
    pub has_practical_required: bool,
}

/// Compare a previous aggregate to the current aggregate and return the
/// level (if any) the learner just crossed. Returns `None` when no new
/// level is reached, OR when the level was already reached.
///
/// Wave 0 returns `None` always (RED for tests below).
/// Wave 1 (Plan 06-02) fills the real comparison.
pub fn which_level_just_crossed(
    _prev: &TrackAggregate,
    _curr: &TrackAggregate,
) -> Option<&'static str> {
    None
}

/// Compute the live track aggregate from `module_progress` rows.
/// Wave 0 returns Err — Wave 1 (Plan 06-02) fills.
pub fn track_mastery_aggregate(
    _conn: &Connection,
    _track_id: &str,
    _learner_id: &str,
) -> Result<TrackAggregate, AchievementError> {
    Err(AchievementError::Validation(
        "Plan 06-02 (Wave 1) implements track_mastery_aggregate".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    //! RED contract tests for the three threshold tiers. Each asserts a
    //! known input/output mapping that Wave 1's arithmetic must satisfy.

    use super::*;

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
    #[ignore = "Plan 06-02 (Wave 1) implements which_level_just_crossed"]
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
    #[ignore = "Plan 06-02 (Wave 1) implements which_level_just_crossed"]
    fn practitioner_at_60_percent() {
        // 6/10 = 60% — Practitioner threshold (D-02).
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
        assert_eq!(
            which_level_just_crossed(&prev, &curr),
            Some("Practitioner"),
            "6/10 modules mastered must cross Practitioner threshold"
        );
    }

    #[test]
    #[ignore = "Plan 06-02 (Wave 1) implements which_level_just_crossed"]
    fn professional_requires_avg_and_labs() {
        // 100% modules + 0.85 avg + (if practical_required) all labs.
        let prev = TrackAggregate {
            modules_total: 4,
            modules_mastered: 3,
            avg_mastery: 0.80,
            all_practical_labs_passed: false,
            has_practical_required: true,
        };
        // Variant A — labs not yet passed: NOT Professional.
        let curr_no_labs = TrackAggregate {
            modules_total: 4,
            modules_mastered: 4,
            avg_mastery: 0.90,
            all_practical_labs_passed: false,
            has_practical_required: true,
        };
        assert_eq!(
            which_level_just_crossed(&prev, &curr_no_labs),
            None,
            "100% + 0.85 avg but missing labs must NOT issue Professional"
        );
        // Variant B — avg too low: NOT Professional.
        let curr_low_avg = TrackAggregate {
            modules_total: 4,
            modules_mastered: 4,
            avg_mastery: 0.80,
            all_practical_labs_passed: true,
            has_practical_required: true,
        };
        assert_eq!(
            which_level_just_crossed(&prev, &curr_low_avg),
            None,
            "avg 0.80 < 0.85 must NOT issue Professional"
        );
        // Variant C — both gates met: Professional.
        let curr_full = TrackAggregate {
            modules_total: 4,
            modules_mastered: 4,
            avg_mastery: 0.90,
            all_practical_labs_passed: true,
            has_practical_required: true,
        };
        assert_eq!(
            which_level_just_crossed(&prev, &curr_full),
            Some("Professional"),
            "100% + 0.90 avg + labs passed must issue Professional"
        );
    }
}
