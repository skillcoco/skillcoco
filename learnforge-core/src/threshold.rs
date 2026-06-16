//! Threshold predicates for the three skill tiers (Associate / Practitioner
//! / Professional), moved verbatim from Phase 6 `achievements/threshold.rs`
//! during Phase 7 Wave 4 (07-04). Pure logic only — no `rusqlite` import,
//! no DB access — so the WASM/web build does not pull a SQL backend.
//!
//! Three skill tiers, uniform across all packs in Phase 6:
//! - Associate:    25% of modules at BKT mastery `>= 0.7`
//! - Practitioner: 60% of modules at mastery `>= 0.7`
//! - Professional: 100% of modules at mastery `>= 0.7`
//!   AND average mastery across the track `>= 0.85`
//!   AND every `practical_required` lab passed
//!
//! ## Architecture seam (Wave 4 ↔ Wave 8)
//!
//! The SQL aggregate function `track_mastery_aggregate` does NOT live here
//! — it lives in `src-tauri/src/storage_impl/threshold.rs` as a free
//! function. Wave 8 (`07-08-PLAN.md`) introduces an `AchievementStore`
//! trait whose method body delegates to that free function; at that point
//! the SQL touch point will be hidden behind a trait, matching the
//! `BktStore` / `SrStore` / `MicrolearningStore` pattern. Wave 4
//! deliberately defers that step so the move stays mechanical.
//!
//! A9 (Phase 6 decision): `mastery_level` is the live high-water mark from
//! `module_progress`; the achievements row preserves the historical proof
//! even if mastery decays later (R4).
//!
//! ## Example
//!
//! ```
//! use learnforge_core::threshold::{TrackAggregate, levels_met, which_level_just_crossed};
//!
//! let prev = TrackAggregate {
//!     modules_total: 4,
//!     modules_mastered: 0,
//!     avg_mastery: 0.0,
//!     all_practical_labs_passed: false,
//!     has_practical_required: false,
//! };
//! let curr = TrackAggregate {
//!     modules_total: 4,
//!     modules_mastered: 1,    // 25%
//!     avg_mastery: 0.7,
//!     all_practical_labs_passed: false,
//!     has_practical_required: false,
//! };
//! assert_eq!(which_level_just_crossed(&prev, &curr), Some("Associate"));
//! assert!(levels_met(&curr).contains(&"Associate"));
//! ```

/// Per-track snapshot used to decide which levels (if any) are met now.
///
/// Built by the SQL aggregate `track_mastery_aggregate` (in
/// `src-tauri/src/storage_impl/threshold.rs` during Wave 4; promoted to a
/// trait method in Wave 8) and consumed by the pure predicates
/// [`which_level_just_crossed`] / [`levels_met`].
///
/// # Example
///
/// ```
/// use learnforge_core::threshold::TrackAggregate;
///
/// let agg = TrackAggregate {
///     modules_total: 10,
///     modules_mastered: 6,
///     avg_mastery: 0.72,
///     all_practical_labs_passed: true,
///     has_practical_required: false,
/// };
/// assert_eq!(agg.modules_total, 10);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TrackAggregate {
    /// Total number of modules in the latest path version for the track.
    pub modules_total: usize,
    /// Number of modules whose `mastery_level >= 0.7`.
    pub modules_mastered: usize,
    /// Average mastery across ALL modules in the track (missing
    /// `module_progress` rows count as 0.0 per the SQL `COALESCE`).
    pub avg_mastery: f64,
    /// `true` iff every module flagged `practical_required` has
    /// `practical_mastery >= 0.7`. Trivially `true` when no module is
    /// flagged.
    pub all_practical_labs_passed: bool,
    /// `true` iff at least one module in the track sets
    /// `content_json.practical_required = true`.
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
/// requires a batch mastery update from 0% to `>= 60%`), this returns the
/// highest newly-crossed tier. `maybe_issue` (in src-tauri until Wave 8)
/// separately uses [`levels_met`] + the DB to insert any previously-missed
/// badges, so the caller never relies on this function alone.
///
/// # Example
///
/// ```
/// use learnforge_core::threshold::{TrackAggregate, which_level_just_crossed};
///
/// let prev = TrackAggregate {
///     modules_total: 10, modules_mastered: 4, avg_mastery: 0.7,
///     all_practical_labs_passed: false, has_practical_required: false,
/// };
/// let curr = TrackAggregate {
///     modules_total: 10, modules_mastered: 6, avg_mastery: 0.72,
///     all_practical_labs_passed: false, has_practical_required: false,
/// };
/// assert_eq!(which_level_just_crossed(&prev, &curr), Some("Practitioner"));
/// ```
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
///
/// # Example
///
/// ```
/// use learnforge_core::threshold::{TrackAggregate, levels_met};
///
/// let agg = TrackAggregate {
///     modules_total: 4, modules_mastered: 4, avg_mastery: 0.90,
///     all_practical_labs_passed: true, has_practical_required: true,
/// };
/// assert_eq!(levels_met(&agg), vec!["Associate", "Practitioner", "Professional"]);
/// ```
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

#[cfg(test)]
mod tests {
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
            modules_total: 4,
            modules_mastered: 3,
            avg_mastery: 0.80,
            all_practical_labs_passed: false,
            has_practical_required: true,
        };
        let curr_full = |avg, labs| TrackAggregate {
            modules_total: 4,
            modules_mastered: 4,
            avg_mastery: avg,
            all_practical_labs_passed: labs,
            has_practical_required: true,
        };
        // Missing labs / low avg — NOT Professional.
        assert_eq!(
            which_level_just_crossed(&prev, &curr_full(0.90, false)),
            None,
            "missing labs"
        );
        assert_eq!(
            which_level_just_crossed(&prev, &curr_full(0.80, true)),
            None,
            "avg below 0.85"
        );
        // Both gates pass — Professional.
        assert_eq!(
            which_level_just_crossed(&prev, &curr_full(0.90, true)),
            Some("Professional")
        );
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

    #[test]
    fn levels_met_empty_track_returns_empty() {
        // modules_total == 0 — early return; even with avg/labs set we get [].
        let agg = TrackAggregate {
            modules_total: 0,
            modules_mastered: 0,
            avg_mastery: 1.0,
            all_practical_labs_passed: true,
            has_practical_required: false,
        };
        assert!(levels_met(&agg).is_empty());
    }

    #[test]
    fn professional_with_no_practical_required_still_works() {
        // `has_practical_required = false` → the labs gate is trivially satisfied
        // (`!has_practical_required || all_practical_labs_passed` short-circuits).
        let agg = TrackAggregate {
            modules_total: 4,
            modules_mastered: 4,
            avg_mastery: 0.90,
            all_practical_labs_passed: false, // doesn't matter
            has_practical_required: false,
        };
        assert!(is_professional(&agg));
    }
}
