//! D-05 mastery-band predicate — pure logic over a raw mastery fraction.
//!
//! Modeled on `crate::threshold::levels_met` (a pure predicate over a
//! numeric aggregate). `band_for` maps a `0.0..=1.0` mastery fraction to
//! one of four named bands per the 18-UI-SPEC.md Mastery Bands table:
//! Novice (0-24%), Working (25-59%), Proficient (60-84%), Mastered
//! (85-100%). Out-of-range input is clamped defensively (never panics).

/// Map a raw mastery fraction (`0.0..=1.0`) to its named band.
///
/// Boundaries (D-05 / 18-UI-SPEC.md):
/// - `< 0.25` -> `"Novice"`
/// - `0.25..0.60` -> `"Working"`
/// - `0.60..0.85` -> `"Proficient"`
/// - `>= 0.85` -> `"Mastered"`
///
/// Values below `0.0` are clamped to `0.0` (Novice); values above `1.0`
/// are clamped to `1.0` (Mastered). NaN is treated as `0.0` (Novice) —
/// defensive handling since mastery fractions should never be NaN in
/// practice.
///
/// # Example
///
/// ```
/// use learnforge_core::reports::bands::band_for;
///
/// assert_eq!(band_for(0.0), "Novice");
/// assert_eq!(band_for(0.24), "Novice");
/// assert_eq!(band_for(0.25), "Working");
/// assert_eq!(band_for(0.59), "Working");
/// assert_eq!(band_for(0.60), "Proficient");
/// assert_eq!(band_for(0.84), "Proficient");
/// assert_eq!(band_for(0.85), "Mastered");
/// assert_eq!(band_for(1.0), "Mastered");
/// ```
pub fn band_for(mastery: f64) -> &'static str {
    let m = if mastery.is_nan() {
        0.0
    } else {
        mastery.clamp(0.0, 1.0)
    };
    if m >= 0.85 {
        "Mastered"
    } else if m >= 0.60 {
        "Proficient"
    } else if m >= 0.25 {
        "Working"
    } else {
        "Novice"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_0_0_is_novice() {
        assert_eq!(band_for(0.0), "Novice");
    }

    #[test]
    fn boundary_0_24_is_novice() {
        assert_eq!(band_for(0.24), "Novice");
    }

    #[test]
    fn boundary_0_25_is_working() {
        assert_eq!(band_for(0.25), "Working");
    }

    #[test]
    fn boundary_0_59_is_working() {
        assert_eq!(band_for(0.59), "Working");
    }

    #[test]
    fn boundary_0_60_is_proficient() {
        assert_eq!(band_for(0.60), "Proficient");
    }

    #[test]
    fn boundary_0_84_is_proficient() {
        assert_eq!(band_for(0.84), "Proficient");
    }

    #[test]
    fn boundary_0_85_is_mastered() {
        assert_eq!(band_for(0.85), "Mastered");
    }

    #[test]
    fn boundary_1_0_is_mastered() {
        assert_eq!(band_for(1.0), "Mastered");
    }

    #[test]
    fn out_of_range_clamps_defensively() {
        assert_eq!(band_for(-1.0), "Novice");
        assert_eq!(band_for(2.0), "Mastered");
        assert_eq!(band_for(f64::NAN), "Novice");
    }
}
