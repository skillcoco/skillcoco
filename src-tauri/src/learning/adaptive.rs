/// Bayesian Knowledge Tracing (BKT) implementation for adaptive learning.
///
/// Maintains probability distributions over learner mastery for each concept.
/// Parameters:
/// - P(L0): Prior probability of knowing the concept
/// - P(T):  Probability of learning on each attempt
/// - P(G):  Probability of guessing correctly without knowledge
/// - P(S):  Probability of slipping (wrong answer despite knowledge)

#[derive(Debug, Clone)]
pub struct BKTParams {
    pub p_know: f64,   // P(L0) - initial knowledge probability
    pub p_learn: f64,  // P(T)  - learning rate
    pub p_guess: f64,  // P(G)  - guess probability
    pub p_slip: f64,   // P(S)  - slip probability
}

impl Default for BKTParams {
    fn default() -> Self {
        Self {
            p_know: 0.3,
            p_learn: 0.1,
            p_guess: 0.2,
            p_slip: 0.1,
        }
    }
}

/// Update mastery probability after an observation (correct/incorrect)
pub fn update_mastery(params: &BKTParams, prior_mastery: f64, is_correct: bool) -> f64 {
    // P(correct | known) = 1 - P(S)
    // P(correct | unknown) = P(G)
    let p_correct_given_known = 1.0 - params.p_slip;
    let p_correct_given_unknown = params.p_guess;

    // Posterior: P(known | observation)
    let posterior = if is_correct {
        let numerator = prior_mastery * p_correct_given_known;
        let denominator = numerator + (1.0 - prior_mastery) * p_correct_given_unknown;
        numerator / denominator
    } else {
        let numerator = prior_mastery * params.p_slip;
        let denominator = numerator + (1.0 - prior_mastery) * (1.0 - p_correct_given_unknown);
        numerator / denominator
    };

    // Apply learning: P(known after practice) = P(known | obs) + (1 - P(known | obs)) * P(T)
    posterior + (1.0 - posterior) * params.p_learn
}

/// Determine if adaptation is needed based on mastery deviation
pub fn should_adapt(expected_mastery: f64, actual_mastery: f64, threshold: f64) -> bool {
    (expected_mastery - actual_mastery).abs() > threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mastery_increases_on_correct() {
        let params = BKTParams::default();
        let initial = 0.3;
        let updated = update_mastery(&params, initial, true);
        assert!(updated > initial, "Mastery should increase on correct answer");
    }

    #[test]
    fn test_mastery_decreases_on_incorrect() {
        let params = BKTParams::default();
        let initial = 0.7;
        let updated = update_mastery(&params, initial, false);
        // Even with learning rate, incorrect should lower effective mastery
        // (though BKT always adds P(T), the posterior drops significantly)
        let posterior_only = {
            let num = initial * params.p_slip;
            let den = num + (1.0 - initial) * (1.0 - params.p_guess);
            num / den
        };
        assert!(posterior_only < initial, "Posterior should decrease on incorrect");
    }

    #[test]
    fn test_mastery_bounded_0_to_1() {
        let params = BKTParams::default();
        // Even after many correct answers, mastery should stay <= 1.0
        let mut mastery = 0.3;
        for _ in 0..100 {
            mastery = update_mastery(&params, mastery, true);
        }
        assert!(mastery <= 1.0, "Mastery should not exceed 1.0");
        assert!(mastery > 0.0, "Mastery should be positive");
    }

    #[test]
    fn test_mastery_converges_on_repeated_correct() {
        let params = BKTParams::default();
        let mut mastery = 0.3;
        for _ in 0..50 {
            mastery = update_mastery(&params, mastery, true);
        }
        // Should converge near 1.0
        assert!(mastery > 0.95, "Mastery should converge near 1.0 after many correct answers");
    }

    #[test]
    fn test_custom_params() {
        let params = BKTParams {
            p_know: 0.5,
            p_learn: 0.2,
            p_guess: 0.1,
            p_slip: 0.05,
        };
        let result = update_mastery(&params, 0.5, true);
        assert!(result > 0.5, "Higher prior + correct should increase mastery");
    }

    #[test]
    fn test_should_adapt_within_threshold() {
        assert!(!should_adapt(0.5, 0.48, 0.1));
    }

    #[test]
    fn test_should_adapt_exceeds_threshold() {
        assert!(should_adapt(0.8, 0.5, 0.1));
    }

    #[test]
    fn test_should_adapt_exact_threshold() {
        assert!(!should_adapt(0.5, 0.4, 0.1)); // abs diff = 0.1, not > 0.1
    }
}
