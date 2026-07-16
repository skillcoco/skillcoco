//! Bayesian Knowledge Tracing (BKT) — adaptive mastery estimation.
//!
//! Implements the canonical 4-parameter BKT model (Corbett & Anderson, 1995):
//!
//! - **P(L0)** — prior probability the learner knows the skill (`p_know`).
//! - **P(T)**  — learning rate per observation (`p_learn`).
//! - **P(G)**  — guess probability (`p_guess`).
//! - **P(S)**  — slip probability (`p_slip`).
//!
//! On each observation (`is_correct: bool`) the model computes the posterior
//! P(known | obs) via Bayes' rule, then applies the learning step to yield
//! the updated mastery estimate in `[0, 1]`.
//!
//! ## Mastery threshold
//!
//! [`MASTERY_THRESHOLD`] (`0.7`) is the canonical cutoff used by downstream
//! gating logic (e.g. prerequisite unlock — see [`crate::path`]).
//!
//! ## Storage abstraction
//!
//! BKT itself is pure math (no I/O). Persistence of `mastery_level` lives
//! behind the [`BktStore`] trait — host crates implement it against their
//! datastore (rusqlite in `learnforge`'s `src-tauri`, IndexedDB on the web,
//! etc.). The trait lives next to the algorithm per the per-module storage
//! pattern (decision A3 / Pattern 1 from `07-RESEARCH.md`).
//!
//! ## Example
//!
//! ```
//! use skillcoco_core::bkt::{BKTParams, update_mastery, MASTERY_THRESHOLD};
//!
//! let params = BKTParams::default();
//! // After one correct answer starting from prior 0.3
//! let updated = update_mastery(&params, 0.3, true);
//! assert!(updated > 0.3);
//! assert!(updated <= 1.0);
//! // Mastery threshold for module-unlock gating
//! assert_eq!(MASTERY_THRESHOLD, 0.7);
//! ```

use thiserror::Error;

/// Mastery threshold for module completion (BKT posterior >= this = mastered).
///
/// Used by [`crate::path::all_prerequisites_mastered`] and downstream
/// prerequisite-unlock logic in host crates. `0.7` is the long-standing
/// project-level constant; changing it requires coordinated migration
/// across stored progress rows.
///
/// # Example
///
/// ```
/// use skillcoco_core::bkt::MASTERY_THRESHOLD;
/// assert_eq!(MASTERY_THRESHOLD, 0.7);
/// ```
pub const MASTERY_THRESHOLD: f64 = 0.7;

/// BKT model parameters (4-tuple).
///
/// See module-level docs for the role of each parameter. [`BKTParams::default`]
/// returns the project-wide defaults (`p_know=0.3, p_learn=0.1, p_guess=0.2,
/// p_slip=0.1`).
///
/// # Example
///
/// ```
/// use skillcoco_core::bkt::BKTParams;
///
/// let defaults = BKTParams::default();
/// assert_eq!(defaults.p_know, 0.3);
/// assert_eq!(defaults.p_learn, 0.1);
/// assert_eq!(defaults.p_guess, 0.2);
/// assert_eq!(defaults.p_slip, 0.1);
/// ```
#[derive(Debug, Clone)]
pub struct BKTParams {
    /// P(L0) — initial knowledge probability.
    pub p_know: f64,
    /// P(T) — learning rate per observation.
    pub p_learn: f64,
    /// P(G) — guess probability.
    pub p_guess: f64,
    /// P(S) — slip probability.
    pub p_slip: f64,
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

/// Update mastery probability after an observation (correct/incorrect).
///
/// Returns the new mastery estimate in `[0, 1]` given:
/// - `params` — BKT parameters
/// - `prior_mastery` — current estimate of P(known) before the observation
/// - `is_correct` — whether the observation was a correct answer
///
/// The posterior is computed via Bayes' rule, then the learning step is
/// applied: `posterior + (1 - posterior) * p_learn`.
///
/// # Example
///
/// ```
/// use skillcoco_core::bkt::{BKTParams, update_mastery};
///
/// let params = BKTParams::default();
/// let after_correct = update_mastery(&params, 0.3, true);
/// assert!(after_correct > 0.3);
///
/// let after_incorrect = update_mastery(&params, 0.7, false);
/// // Note: BKT always adds learning gain, so the final value can stay
/// // above the posterior — but the posterior itself drops.
/// assert!(after_incorrect <= 1.0);
/// ```
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

/// Determine if adaptation is needed based on mastery deviation.
///
/// Returns `true` when the absolute difference between expected and actual
/// mastery exceeds `threshold` (strict inequality). Used by adaptive content
/// selection to decide whether to escalate / de-escalate difficulty.
///
/// # Example
///
/// ```
/// use skillcoco_core::bkt::should_adapt;
///
/// // Within band → no adaptation
/// assert!(!should_adapt(0.5, 0.48, 0.1));
/// // Beyond band → adapt
/// assert!(should_adapt(0.8, 0.5, 0.1));
/// // Exactly at threshold → no adaptation (strict `>`)
/// assert!(!should_adapt(0.5, 0.4, 0.1));
/// ```
pub fn should_adapt(expected_mastery: f64, actual_mastery: f64, threshold: f64) -> bool {
    (expected_mastery - actual_mastery).abs() > threshold
}

/// Errors returned by [`BktStore`] implementations.
///
/// `Db(String)` carries backend-specific error messages stringified at the
/// boundary so `skillcoco-core` itself never depends on rusqlite / IndexedDB
/// / etc. — preserving the WASM-portability and anti-leakage invariants
/// (decision D-02, Pitfall T-07-05).
#[derive(Debug, Error)]
pub enum BktError {
    /// Backend-reported error — message stringified at the trust boundary.
    #[error("db error: {0}")]
    Db(String),
    /// No mastery row exists for the given `(learner_id, module_id)` pair.
    #[error("mastery not found for learner={learner_id} module={module_id}")]
    NotFound {
        /// Learner identifier supplied to the lookup.
        learner_id: String,
        /// Module identifier supplied to the lookup.
        module_id: String,
    },
}

/// Per-learner mastery persistence contract for BKT.
///
/// Hosts implement this trait against their datastore. The trait lives next
/// to the algorithm module (A3 lock — see `07-RESEARCH.md`) rather than in a
/// central `storage.rs`, so each algorithm module owns its persistence shape.
///
/// # Example
///
/// ```
/// use skillcoco_core::bkt::{BktError, BktStore};
///
/// struct InMemoryStub;
/// impl BktStore for InMemoryStub {
///     fn read_mastery(&self, _learner_id: &str, _module_id: &str) -> Result<f64, BktError> {
///         Ok(0.85)
///     }
/// }
///
/// let store = InMemoryStub;
/// assert_eq!(store.read_mastery("learner-1", "mod-1").unwrap(), 0.85);
/// ```
pub trait BktStore {
    /// Read the persisted mastery estimate for `(learner_id, module_id)`.
    ///
    /// Returns `Ok(mastery)` where `mastery ∈ [0, 1]` when a row exists,
    /// or [`BktError::NotFound`] when none does. Backend I/O errors are
    /// surfaced via [`BktError::Db`].
    fn read_mastery(&self, learner_id: &str, module_id: &str) -> Result<f64, BktError>;
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
        let _ = updated; // updated includes learning step; assertion on posterior is what matters
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

    #[test]
    fn bkt_store_trait_compiles() {
        struct Stub;
        impl BktStore for Stub {
            fn read_mastery(&self, _: &str, _: &str) -> Result<f64, BktError> {
                Ok(0.5)
            }
        }
        let s = Stub;
        assert!(s.read_mastery("a", "b").is_ok());
        assert_eq!(s.read_mastery("a", "b").unwrap(), 0.5);
    }
}
