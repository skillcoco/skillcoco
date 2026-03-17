/// SM-2 spaced repetition algorithm implementation.
///
/// Based on the SuperMemo SM-2 algorithm with modifications for
/// technical learning (active recall emphasis).

/// Quality ratings for review responses
/// 0 - Complete blackout
/// 1 - Wrong, but remembered upon seeing answer
/// 2 - Wrong, but answer seemed easy to recall
/// 3 - Correct with serious difficulty
/// 4 - Correct after hesitation
/// 5 - Perfect recall

#[derive(Debug, Clone)]
pub struct SM2Result {
    pub interval: f64,      // days until next review
    pub ease_factor: f64,   // updated ease factor
    pub repetitions: i32,   // updated repetition count
}

/// Calculate next review interval using SM-2 algorithm
pub fn sm2_calculate(
    quality: i32,           // 0-5 quality rating
    repetitions: i32,       // current repetition count
    ease_factor: f64,       // current ease factor (>= 1.3)
    interval: f64,          // current interval in days
) -> SM2Result {
    let quality = quality.clamp(0, 5);

    if quality < 3 {
        // Failed review - reset
        SM2Result {
            interval: 1.0,
            ease_factor: ease_factor.max(1.3),
            repetitions: 0,
        }
    } else {
        // Successful review
        let new_repetitions = repetitions + 1;
        let new_interval = match new_repetitions {
            1 => 1.0,
            2 => 6.0,
            _ => interval * ease_factor,
        };

        // Update ease factor: EF' = EF + (0.1 - (5-q) * (0.08 + (5-q) * 0.02))
        let q = quality as f64;
        let new_ef = ease_factor + (0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02));
        let new_ef = new_ef.max(1.3); // Minimum ease factor

        SM2Result {
            interval: new_interval,
            ease_factor: new_ef,
            repetitions: new_repetitions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_successful_review() {
        let result = sm2_calculate(4, 0, 2.5, 0.0);
        assert_eq!(result.interval, 1.0);
        assert_eq!(result.repetitions, 1);
    }

    #[test]
    fn test_second_successful_review() {
        let result = sm2_calculate(4, 1, 2.5, 1.0);
        assert_eq!(result.interval, 6.0);
        assert_eq!(result.repetitions, 2);
    }

    #[test]
    fn test_failed_review_resets() {
        let result = sm2_calculate(1, 5, 2.5, 30.0);
        assert_eq!(result.interval, 1.0);
        assert_eq!(result.repetitions, 0);
    }

    #[test]
    fn test_ease_factor_minimum() {
        let result = sm2_calculate(3, 3, 1.3, 10.0);
        assert!(result.ease_factor >= 1.3);
    }

    #[test]
    fn test_third_review_uses_ease_factor() {
        let result = sm2_calculate(5, 2, 2.5, 6.0);
        assert_eq!(result.repetitions, 3);
        assert_eq!(result.interval, 6.0 * 2.5); // interval * EF
    }

    #[test]
    fn test_perfect_recall_increases_ease() {
        let result = sm2_calculate(5, 3, 2.5, 15.0);
        assert!(result.ease_factor > 2.5, "Perfect recall should increase ease factor");
    }

    #[test]
    fn test_difficult_recall_decreases_ease() {
        let result = sm2_calculate(3, 3, 2.5, 15.0);
        assert!(result.ease_factor < 2.5, "Difficult recall should decrease ease factor");
    }

    #[test]
    fn test_quality_clamped() {
        // Quality below 0 should be treated as 0 (failed)
        let result = sm2_calculate(-1, 5, 2.5, 30.0);
        assert_eq!(result.interval, 1.0);
        assert_eq!(result.repetitions, 0);

        // Quality above 5 should be treated as 5
        let result = sm2_calculate(10, 0, 2.5, 0.0);
        assert_eq!(result.repetitions, 1);
    }

    #[test]
    fn test_boundary_quality_2_fails() {
        let result = sm2_calculate(2, 3, 2.5, 15.0);
        assert_eq!(result.interval, 1.0);
        assert_eq!(result.repetitions, 0);
    }

    #[test]
    fn test_boundary_quality_3_passes() {
        let result = sm2_calculate(3, 0, 2.5, 0.0);
        assert_eq!(result.repetitions, 1);
        assert_eq!(result.interval, 1.0);
    }

    #[test]
    fn test_long_sequence_intervals_grow() {
        let mut interval = 0.0;
        let mut ef = 2.5;
        let mut reps = 0;

        for _ in 0..10 {
            let result = sm2_calculate(4, reps, ef, interval);
            assert!(result.interval >= interval || reps < 2, "Intervals should generally grow");
            interval = result.interval;
            ef = result.ease_factor;
            reps = result.repetitions;
        }
        assert!(interval > 30.0, "After 10 successful reviews, interval should be > 30 days");
    }
}
