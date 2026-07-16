//! SuperMemo 2 (SM-2) — spaced-repetition scheduling.
//!
//! Pure-math implementation of the canonical SM-2 algorithm (Wozniak 1987)
//! with the project's "active recall" emphasis for technical learning. On each
//! review the model takes a quality rating in `[0, 5]` and the card's current
//! `(repetitions, ease_factor, interval)` triple, and returns the next
//! triple — no I/O, no schema dependency.
//!
//! ## Quality scale
//!
//! - `0` — complete blackout
//! - `1` — wrong, but remembered upon seeing the answer
//! - `2` — wrong, but answer seemed easy to recall
//! - `3` — correct with serious difficulty (boundary "pass")
//! - `4` — correct after hesitation
//! - `5` — perfect recall
//!
//! Quality `< 3` resets `repetitions → 0` and `interval → 1 day` (clamping
//! `ease_factor` to a floor of `1.3`). Quality `>= 3` increments
//! `repetitions` and lengthens the interval by the standard
//! `1, 6, interval * ease_factor` schedule. The ease-factor update rule is
//! `EF' = EF + (0.1 − (5 − q)(0.08 + (5 − q) × 0.02))`, floored at `1.3`.
//!
//! ## Storage abstraction
//!
//! The algorithm itself is pure. Persistence of due cards and review updates
//! lives behind the [`SrStore`] trait — host crates implement it against
//! their datastore. The trait lives next to the algorithm per the per-module
//! storage pattern (decision A3 / Pattern 1 from `07-RESEARCH.md`); error
//! translation at the boundary keeps `skillcoco-core` free of backend
//! types (decision D-02 / T-07-07).
//!
//! ## Example
//!
//! ```
//! use skillcoco_core::sm2::sm2_calculate;
//!
//! // First successful review (quality=4, fresh card)
//! let r = sm2_calculate(4, 0, 2.5, 0.0);
//! assert_eq!(r.repetitions, 1);
//! assert_eq!(r.interval, 1.0);
//! ```

use thiserror::Error;

/// Outcome of an SM-2 review — the next scheduling triple.
///
/// All three fields are produced together by [`sm2_calculate`]; callers
/// persist them to advance the card's schedule.
///
/// # Example
///
/// ```
/// use skillcoco_core::sm2::{SM2Result, sm2_calculate};
///
/// let r: SM2Result = sm2_calculate(5, 2, 2.5, 6.0);
/// assert_eq!(r.repetitions, 3);
/// assert_eq!(r.interval, 15.0); // 6.0 * 2.5
/// ```
#[derive(Debug, Clone)]
pub struct SM2Result {
    /// Days until next review.
    pub interval: f64,
    /// Updated ease factor (floored at `1.3`).
    pub ease_factor: f64,
    /// Updated repetition count (resets to `0` on quality `< 3`).
    pub repetitions: i32,
}

/// Calculate next review scheduling using the SM-2 algorithm.
///
/// # Arguments
///
/// - `quality` — recall quality in `[0, 5]` (clamped at the boundary).
/// - `repetitions` — current repetition count (will be incremented on pass).
/// - `ease_factor` — current ease factor (will be floored at `1.3`).
/// - `interval` — current interval in days.
///
/// # Behavior
///
/// - Quality `< 3` (failed review): reset `repetitions → 0`, `interval → 1`,
///   `ease_factor` floored at `1.3`.
/// - Quality `>= 3` (successful review): increment `repetitions`; new interval
///   follows `1, 6, interval * ease_factor` schedule. Ease factor updated per
///   `EF + (0.1 − (5 − q)(0.08 + (5 − q) × 0.02))`, floored at `1.3`.
///
/// Quality outside `[0, 5]` is clamped at the boundary (≤0 → 0, ≥5 → 5).
///
/// # Example
///
/// ```
/// use skillcoco_core::sm2::sm2_calculate;
///
/// // Failed review resets the card
/// let failed = sm2_calculate(1, 5, 2.5, 30.0);
/// assert_eq!(failed.repetitions, 0);
/// assert_eq!(failed.interval, 1.0);
///
/// // Perfect recall (quality=5) raises the ease factor
/// let perfect = sm2_calculate(5, 3, 2.5, 15.0);
/// assert!(perfect.ease_factor > 2.5);
/// ```
pub fn sm2_calculate(
    quality: i32,
    repetitions: i32,
    ease_factor: f64,
    interval: f64,
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

/// Errors returned by [`SrStore`] implementations.
///
/// `Db(String)` carries backend-specific error messages stringified at the
/// boundary so `skillcoco-core` itself never depends on rusqlite / IndexedDB
/// / etc. — preserving the WASM-portability and anti-leakage invariants
/// (decision D-02 / T-07-07 mitigation). Mirrors the [`crate::bkt::BktError`]
/// pattern from Wave 2.
#[derive(Debug, Error)]
pub enum SrError {
    /// Backend-reported error — message stringified at the trust boundary.
    #[error("db error: {0}")]
    Db(String),
    /// No SR card row exists for the given `card_id`.
    #[error("sr card not found: {card_id}")]
    NotFound {
        /// Card identifier that was not found.
        card_id: String,
    },
}

/// One row from the SR card store.
///
/// The shape mirrors the actual `sr_cards` SQLite table consumed by the
/// reference impl in `src-tauri`. Note that `next_review` and `last_review`
/// are ISO datetime strings (not millisecond timestamps) — the underlying
/// schema stores them as SQLite `TEXT` produced via `datetime('now', ...)`;
/// the trait keeps that shape so the rusqlite adapter is a 1:1 row mapping
/// rather than an in-flight unit conversion.
///
/// Field naming is rust-idiomatic snake_case — the IPC boundary in src-tauri
/// applies camelCase serde renaming on its own `SRCard` DTO.
#[derive(Debug, Clone, PartialEq)]
pub struct SrCardRow {
    /// Card primary key.
    pub id: String,
    /// Module the card belongs to.
    pub module_id: String,
    /// Concept tag (free-form).
    pub concept: String,
    /// Card type (`active_recall`, `flash_card`, ...).
    pub card_type: String,
    /// Front face text.
    pub front: String,
    /// Back face text.
    pub back: String,
    /// Current interval in days.
    pub interval_days: f64,
    /// Current ease factor.
    pub ease_factor: f64,
    /// Repetition count.
    pub repetitions: i32,
    /// ISO datetime string for when the card is next due.
    pub next_review: String,
    /// ISO datetime string for the most recent review, or `None` if never reviewed.
    pub last_review: Option<String>,
}

/// Per-card spaced-repetition persistence contract.
///
/// Hosts implement this trait against their datastore. The trait lives next
/// to the SM-2 algorithm (A3 lock — see `07-RESEARCH.md`) rather than in a
/// central `storage.rs`, so each algorithm module owns its persistence
/// shape.
///
/// ## Surface minimality
///
/// Wave 3 enumerated the SR call sites by grepping `sr_cards` SQL in
/// `src-tauri`. Three read paths and one write path cover every existing
/// caller (see `07-03-SUMMARY.md` "SR call-site audit"). Generation /
/// insertion of cards is intentionally out of scope — it's tightly coupled
/// to flash-block content generation, lives in `commands/learning.rs`, and
/// will be revisited if/when block content generation moves to core.
///
/// # Example
///
/// ```
/// use skillcoco_core::sm2::{SrCardRow, SrError, SrStore, SM2Result};
///
/// struct InMemoryStub;
/// impl SrStore for InMemoryStub {
///     fn read_due_cards(&self, _limit: i32) -> Result<Vec<SrCardRow>, SrError> {
///         Ok(Vec::new())
///     }
///     fn count_due_cards_for_module(&self, _module_id: &str) -> Result<i64, SrError> {
///         Ok(0)
///     }
///     fn read_card_by_id(&self, card_id: &str) -> Result<SrCardRow, SrError> {
///         Err(SrError::NotFound { card_id: card_id.to_string() })
///     }
///     fn apply_review_update(
///         &self,
///         _card_id: &str,
///         _result: &SM2Result,
///     ) -> Result<String, SrError> {
///         Ok("2026-06-17T00:00:00Z".to_string())
///     }
/// }
///
/// let store = InMemoryStub;
/// assert_eq!(store.count_due_cards_for_module("mod-1").unwrap(), 0);
/// ```
pub trait SrStore {
    /// Read all SR cards whose `next_review` is at or before "now",
    /// ordered by `next_review` ascending, capped at `limit` rows.
    ///
    /// Mirrors the `commands/learning.rs::get_due_cards` query. The reference
    /// schema does NOT carry a `learner_id` column on `sr_cards`, so the
    /// trait surface deliberately omits per-learner filtering — Wave 9 may
    /// revisit if the schema gains learner segmentation.
    fn read_due_cards(&self, limit: i32) -> Result<Vec<SrCardRow>, SrError>;

    /// Count how many SR cards are currently due for a given module.
    ///
    /// Mirrors the `learning::microlearning_selection::module_has_due_sr_card`
    /// query (returns the integer count so callers can apply their own
    /// "≥1 means due" predicate).
    fn count_due_cards_for_module(&self, module_id: &str) -> Result<i64, SrError>;

    /// Read a single SR card by its primary key.
    ///
    /// Returns [`SrError::NotFound`] when no row matches.
    fn read_card_by_id(&self, card_id: &str) -> Result<SrCardRow, SrError>;

    /// Persist the SM-2 review outcome for a card.
    ///
    /// Updates `interval_days`, `ease_factor`, `repetitions`, advances
    /// `next_review` by `result.interval` days, and stamps `last_review` to
    /// the current time. Returns the ISO datetime string of the new
    /// `next_review` (so callers can render "next review in N days" without
    /// a second query — preserves the existing `commands/learning.rs::submit_review`
    /// behavior).
    fn apply_review_update(
        &self,
        card_id: &str,
        result: &SM2Result,
    ) -> Result<String, SrError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 11 unit tests moved verbatim from src-tauri/src/learning/spaced_repetition.rs ──

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
        assert!(
            result.ease_factor > 2.5,
            "Perfect recall should increase ease factor"
        );
    }

    #[test]
    fn test_difficult_recall_decreases_ease() {
        let result = sm2_calculate(3, 3, 2.5, 15.0);
        assert!(
            result.ease_factor < 2.5,
            "Difficult recall should decrease ease factor"
        );
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
            assert!(
                result.interval >= interval || reps < 2,
                "Intervals should generally grow"
            );
            interval = result.interval;
            ef = result.ease_factor;
            reps = result.repetitions;
        }
        assert!(
            interval > 30.0,
            "After 10 successful reviews, interval should be > 30 days"
        );
    }

    // ── New Wave 3 tests: SrStore trait dispatch + compile-check ──

    #[test]
    fn sr_store_trait_compiles() {
        // Smallest possible impl proving the trait surface is satisfiable
        // without rusqlite — implementing it on `()` keeps the test alloc-free.
        struct Stub;
        impl SrStore for Stub {
            fn read_due_cards(&self, _limit: i32) -> Result<Vec<SrCardRow>, SrError> {
                Ok(Vec::new())
            }
            fn count_due_cards_for_module(
                &self,
                _module_id: &str,
            ) -> Result<i64, SrError> {
                Ok(0)
            }
            fn read_card_by_id(&self, card_id: &str) -> Result<SrCardRow, SrError> {
                Err(SrError::NotFound {
                    card_id: card_id.to_string(),
                })
            }
            fn apply_review_update(
                &self,
                _card_id: &str,
                _result: &SM2Result,
            ) -> Result<String, SrError> {
                Ok("2026-06-17T00:00:00Z".to_string())
            }
        }
        let stub: &dyn SrStore = &Stub;
        assert_eq!(stub.count_due_cards_for_module("m1").unwrap(), 0);
        assert_eq!(stub.read_due_cards(10).unwrap().len(), 0);
    }

    #[test]
    fn apply_review_update_dispatches_to_store() {
        // Captures the SM2Result value passed to apply_review_update and
        // verifies it equals sm2_calculate(4, 0, 2.5, 0.0).
        use std::cell::RefCell;

        struct CapturingStub {
            captured: RefCell<Option<SM2Result>>,
        }
        impl SrStore for CapturingStub {
            fn read_due_cards(&self, _limit: i32) -> Result<Vec<SrCardRow>, SrError> {
                Ok(Vec::new())
            }
            fn count_due_cards_for_module(
                &self,
                _module_id: &str,
            ) -> Result<i64, SrError> {
                Ok(0)
            }
            fn read_card_by_id(&self, card_id: &str) -> Result<SrCardRow, SrError> {
                Err(SrError::NotFound {
                    card_id: card_id.to_string(),
                })
            }
            fn apply_review_update(
                &self,
                _card_id: &str,
                result: &SM2Result,
            ) -> Result<String, SrError> {
                *self.captured.borrow_mut() = Some(result.clone());
                Ok("2026-06-17T00:00:00Z".to_string())
            }
        }

        let stub = CapturingStub {
            captured: RefCell::new(None),
        };

        // Compute the SM-2 result then dispatch it to the store
        let computed = sm2_calculate(4, 0, 2.5, 0.0);
        let _ = stub.apply_review_update("card-1", &computed).unwrap();

        let captured = stub.captured.borrow();
        let captured = captured.as_ref().expect("apply_review_update must capture");
        assert_eq!(captured.interval, 1.0);
        assert_eq!(captured.repetitions, 1);
        // Ease factor for q=4 starting at 2.5: EF + (0.1 - 1*(0.08 + 1*0.02))
        //                                      = 2.5 + (0.1 - 0.10) = 2.5
        assert!((captured.ease_factor - 2.5).abs() < 1e-9);
    }

    #[test]
    fn sr_error_not_found_renders() {
        let err = SrError::NotFound {
            card_id: "card-1".to_string(),
        };
        assert_eq!(err.to_string(), "sr card not found: card-1");
    }

    #[test]
    fn sr_error_db_renders() {
        let err = SrError::Db("connection closed".to_string());
        assert_eq!(err.to_string(), "db error: connection closed");
    }
}
