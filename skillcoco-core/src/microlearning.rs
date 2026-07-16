//! Microlearning daily-challenge selection algorithm (moved from
//! `src-tauri/src/learning/microlearning_selection.rs` during Phase 7
//! Wave 4 / 07-04).
//!
//! Pure, deterministic selection function over a [`MicrolearningStore`]
//! trait — no `rusqlite`, no DB connection, no wall-clock side effect.
//! The wall-clock instant (`now`) is **injected** via a `DateTime<Utc>`
//! parameter (A5 lock — Pitfall 10 mitigation), so tests can pin a
//! deterministic time and WASM builds never accidentally read the Unix
//! epoch (`Utc::now()` returns `1970-01-01` on `wasm32-unknown-unknown`
//! without the `wasmbind` feature).
//!
//! ## Algorithm (matches Phase 4 RESEARCH §"Selection Algorithm")
//!
//! 1. Find candidate modules: mastery in `[BKT_LOWER, BKT_UPPER)`,
//!    active track only.
//! 2. List eligible blocks per module: `status='ready'`,
//!    `block_type ∈ {flash_cards, quiz, section}`.
//! 3. Apply 48h recency penalty per block — Q6 lock measures against
//!    `daily_challenges` history only.
//! 4. SR-due signal per module (`sr_cards.next_review <= now`).
//! 5. BKT-decay signal per module
//!    (`julianday(now) - julianday(last_bkt_update_at)`).
//! 6. Pick highest-scoring block; tie-break on `(ordering, block_id)`
//!    for determinism.
//! 7. Return `None` when every candidate scored at or below
//!    `W_RECENCY / 2` (empty-zone fallback).
//!
//! ## Architecture
//!
//! `select_daily_challenge` is generic over [`MicrolearningStore`]
//! (A3 lock — per-module storage trait), so the algorithm lives in
//! `skillcoco-core` and the SQL helpers live in
//! `src-tauri/src/storage_impl/microlearning.rs`. WASM consumers
//! implement [`MicrolearningStore`] against IndexedDB or any other
//! backend.
//!
//! ## Example
//!
//! ```
//! use chrono::{TimeZone, Utc};
//! use skillcoco_core::microlearning::{
//!     select_daily_challenge, CandidateModule, MicrolearningError, MicrolearningStore,
//! };
//!
//! struct EmptyStore;
//! impl MicrolearningStore for EmptyStore {
//!     fn candidate_modules(&self, _learner_id: &str)
//!         -> Result<Vec<CandidateModule>, MicrolearningError> { Ok(vec![]) }
//!     fn blocks_for_module(&self, _module_id: &str)
//!         -> Result<Vec<(String, String, i32)>, MicrolearningError> { Ok(vec![]) }
//!     fn is_recently_seen(&self, _learner_id: &str, _block_id: &str, _recency_hours: i64)
//!         -> Result<bool, MicrolearningError> { Ok(false) }
//!     fn module_has_due_sr_card(&self, _learner_id: &str, _module_id: &str, _now: chrono::DateTime<Utc>)
//!         -> Result<bool, MicrolearningError> { Ok(false) }
//!     fn decay_days_for_module(&self, _learner_id: &str, _module_id: &str)
//!         -> Result<f64, MicrolearningError> { Ok(0.0) }
//! }
//!
//! let now = Utc.with_ymd_and_hms(2026, 6, 16, 12, 0, 0).unwrap();
//! let result = select_daily_challenge(&EmptyStore, "learner-1", now).unwrap();
//! assert!(result.is_none(), "empty store yields no candidate");
//! ```

use chrono::{DateTime, Utc};

use crate::bkt::MASTERY_THRESHOLD;

// ── Tuning constants (Q5 lock — `const`, not env vars) ──

/// BKT decay half-life in days. Used in `decay_days / DECAY_HALF_LIFE_DAYS`
/// scoring contribution. The store's `decay_days_for_module` reads
/// `module_progress.last_bkt_update_at` (added by migration v007) to
/// compute the elapsed delta.
pub const DECAY_HALF_LIFE_DAYS: f64 = 3.0;

/// Recency penalty window. Blocks seen in `daily_challenges` within this
/// many hours score `W_RECENCY` (effectively excluded).
pub const RECENCY_PENALTY_HOURS: i64 = 48;

/// Weight on the BKT-decay signal.
pub const W_DECAY: f64 = 1.0;

/// Weight on the SR-due signal (slight bias toward review).
pub const W_SR_DUE: f64 = 1.2;

/// Weight on the recency penalty (hard penalty if seen within
/// [`RECENCY_PENALTY_HOURS`]).
pub const W_RECENCY: f64 = -100.0;

/// Lower bound of the BKT candidate window (D-05 — "struggle zone").
pub const BKT_LOWER: f64 = 0.3;

/// Upper bound of the BKT candidate window. Reuses
/// [`crate::bkt::MASTERY_THRESHOLD`] (`0.7`) as the single source of
/// truth for the mastered/not-mastered boundary.
pub const BKT_UPPER: f64 = MASTERY_THRESHOLD;

/// Cap on the decay-multiplier contribution. Without a cap, a module
/// that hasn't been touched in months would dominate any SR-due signal
/// forever.
pub const DECAY_DAYS_CAP_MULT: f64 = 5.0;

/// Error type for the microlearning selection algorithm. The
/// `Backend` variant stringifies any underlying storage error at the
/// trust boundary so `rusqlite::Error` (or `idb::Error`, etc.) never
/// leaks into `skillcoco-core`'s public surface — T-07-05 mitigation,
/// matching the `BktError` / `SrError` pattern.
#[derive(Debug, thiserror::Error)]
pub enum MicrolearningError {
    /// Backend-supplied error (rusqlite or any other [`MicrolearningStore`]
    /// implementation's underlying error, stringified at the boundary).
    #[error("microlearning backend error: {0}")]
    Backend(String),
}

/// Result of the selection algorithm. NOT serialized — internal type
/// that the IPC layer translates into a `DailyChallengePayload` for the
/// frontend.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Identifier of the chosen block.
    pub block_id: String,
    /// Module the block belongs to.
    pub module_id: String,
    /// Track the module belongs to.
    pub track_id: String,
    /// Block type (`flash_cards`, `quiz`, or `section`).
    pub block_type: String,
    /// Composite score the algorithm assigned at selection time.
    /// Exposed for debug/observability; not stored in the IPC payload.
    pub score: f64,
}

/// Lightweight projection of `module_progress` joined with `modules` +
/// `learning_paths` + `learning_tracks`. Returned from
/// [`MicrolearningStore::candidate_modules`] as the algorithm's entry
/// point.
///
/// `mastery_level` + `last_bkt_update_at` are kept for debugging
/// visibility and potential future re-scoring; the active algorithm
/// reads `last_bkt_update_at` via [`MicrolearningStore::decay_days_for_module`]
/// (which performs its own julianday math) rather than parsing the
/// string in Rust.
#[derive(Debug, Clone)]
pub struct CandidateModule {
    /// Module id.
    pub module_id: String,
    /// Owning track id.
    pub track_id: String,
    /// Current BKT mastery (must fall in `[BKT_LOWER, BKT_UPPER)` per
    /// the candidate filter — the store enforces this via SQL).
    pub mastery_level: f64,
    /// Last BKT update timestamp (raw string per the schema; `None`
    /// for cold modules).
    pub last_bkt_update_at: Option<String>,
}

/// Per-module storage trait (A3 lock — Pitfall 9 mitigation).
///
/// The algorithm's four SQL touch points become four trait methods +
/// one (decay_days_for_module) for the BKT-decay signal. All
/// implementations live in adapter crates (e.g. `src-tauri` for the
/// rusqlite reference impl); `skillcoco-core` ships only the trait
/// + algorithm + an in-memory stub used by the unit tests.
pub trait MicrolearningStore {
    /// Step 1 — fetch all modules in the `[BKT_LOWER, BKT_UPPER)`
    /// candidate zone for an **active** track. Mastered modules
    /// (`>= BKT_UPPER`) and never-seen modules (no progress row)
    /// must be excluded.
    fn candidate_modules(
        &self,
        learner_id: &str,
    ) -> Result<Vec<CandidateModule>, MicrolearningError>;

    /// Step 2 — list eligible blocks for a module as
    /// `(block_id, block_type, ordering)`. Implementations should
    /// filter to `status='ready'` and
    /// `block_type IN ('flash_cards', 'quiz', 'section')`.
    fn blocks_for_module(
        &self,
        module_id: &str,
    ) -> Result<Vec<(String, String, i32)>, MicrolearningError>;

    /// Step 3 — was this block shown to this learner in the last
    /// `recency_hours` hours? Q6 lock — measure against the
    /// `daily_challenges` history only.
    fn is_recently_seen(
        &self,
        learner_id: &str,
        block_id: &str,
        recency_hours: i64,
    ) -> Result<bool, MicrolearningError>;

    /// Step 4 — does this module have at least one SR card with
    /// `next_review <= now`?
    ///
    /// The `now` parameter is the **only** A5 clock injection point on
    /// the trait surface: the recency window in [`Self::is_recently_seen`]
    /// is window-relative (no absolute clock needed) and the decay days
    /// in [`Self::decay_days_for_module`] use SQLite's `julianday('now')`
    /// internally. Tests stub this method with a fixed return.
    fn module_has_due_sr_card(
        &self,
        learner_id: &str,
        module_id: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, MicrolearningError>;

    /// Step 5 — days since the BKT update last fired for this
    /// learner/module pair. Returns `0.0` when `last_bkt_update_at`
    /// is `NULL` (cold module — Phase 4 treats "warm" as the safe
    /// default; no decay bonus or penalty).
    fn decay_days_for_module(
        &self,
        learner_id: &str,
        module_id: &str,
    ) -> Result<f64, MicrolearningError>;
}

// ── Public selection function ──

/// Picks today's micro-challenge block for `learner_id`.
///
/// Returns `Ok(None)` when no candidate exists (empty 0.3–0.7 BKT
/// zone) OR when every candidate block was already seen within
/// [`RECENCY_PENALTY_HOURS`] — Q3 fallback the frontend renders as
/// the "no challenge today" placeholder.
///
/// `now` is the wall-clock instant used by the algorithm's "due SR
/// card" check. Pass `chrono::Utc::now()` in production; pin a
/// fixed timestamp in tests for byte-stable behavior (A5 lock —
/// Pitfall 10 mitigation).
pub fn select_daily_challenge<S: MicrolearningStore>(
    store: &S,
    learner_id: &str,
    now: DateTime<Utc>,
) -> Result<Option<Candidate>, MicrolearningError> {
    let modules = store.candidate_modules(learner_id)?;
    if modules.is_empty() {
        return Ok(None);
    }

    // Per-module signals
    let mut scored: Vec<(Candidate, i32)> = Vec::new(); // (cand, ordering for tie-break)

    for cm in &modules {
        let blocks = store.blocks_for_module(&cm.module_id)?;
        if blocks.is_empty() {
            continue;
        }

        let sr_due = store.module_has_due_sr_card(learner_id, &cm.module_id, now)?;
        let decay_days = store.decay_days_for_module(learner_id, &cm.module_id)?;
        let decay_contrib =
            W_DECAY * (decay_days / DECAY_HALF_LIFE_DAYS).min(DECAY_DAYS_CAP_MULT);
        let sr_contrib = if sr_due { W_SR_DUE } else { 0.0 };
        let module_base_score = decay_contrib + sr_contrib;

        for (block_id, block_type, ordering) in blocks {
            let recency_penalty =
                if store.is_recently_seen(learner_id, &block_id, RECENCY_PENALTY_HOURS)? {
                    W_RECENCY
                } else {
                    0.0
                };
            let score = module_base_score + recency_penalty;
            scored.push((
                Candidate {
                    block_id,
                    module_id: cm.module_id.clone(),
                    track_id: cm.track_id.clone(),
                    block_type,
                    score,
                },
                ordering,
            ));
        }
    }

    if scored.is_empty() {
        return Ok(None);
    }

    // Q3 empty-zone fallback — every block was recency-penalized.
    // We treat W_RECENCY/2 as the cutoff because even a maxed-out
    // decay+sr_due contribution (W_SR_DUE + W_DECAY*DECAY_DAYS_CAP_MULT
    // ≈ 6.2) cannot bring a recency-penalized block
    // (W_RECENCY = -100) above this line.
    if scored.iter().all(|(c, _)| c.score <= W_RECENCY / 2.0) {
        return Ok(None);
    }

    // Pick highest score; deterministic tie-break on
    // (ordering asc, block_id asc).
    scored.sort_by(|a, b| {
        b.0.score
            .partial_cmp(&a.0.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.0.block_id.cmp(&b.0.block_id))
    });

    Ok(Some(scored.into_iter().next().unwrap().0))
}

#[cfg(test)]
mod tests {
    //! Pure stub-store tests (A5 — fixed `now` for determinism). The
    //! corresponding rusqlite-backed integration tests live in
    //! `src-tauri/src/storage_impl/microlearning.rs` so the in-memory
    //! `Connection` machinery never leaks into `skillcoco-core`.

    use super::*;
    use chrono::TimeZone;
    use std::cell::RefCell;
    use std::collections::HashSet;

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 16, 12, 0, 0).unwrap()
    }

    /// Inline stub `MicrolearningStore`. Records the `now` it was
    /// called with so tests can assert clock-injection happens.
    #[derive(Default)]
    struct StubStore {
        modules: Vec<CandidateModule>,
        // module_id -> list of (block_id, block_type, ordering)
        blocks: Vec<(String, Vec<(String, String, i32)>)>,
        recently_seen: HashSet<String>, // block_ids seen recently
        sr_due_modules: HashSet<String>,
        decay_days: f64,
        observed_now: RefCell<Option<DateTime<Utc>>>,
    }

    impl MicrolearningStore for StubStore {
        fn candidate_modules(
            &self,
            _learner_id: &str,
        ) -> Result<Vec<CandidateModule>, MicrolearningError> {
            Ok(self.modules.clone())
        }

        fn blocks_for_module(
            &self,
            module_id: &str,
        ) -> Result<Vec<(String, String, i32)>, MicrolearningError> {
            for (mid, blocks) in &self.blocks {
                if mid == module_id {
                    return Ok(blocks.clone());
                }
            }
            Ok(vec![])
        }

        fn is_recently_seen(
            &self,
            _learner_id: &str,
            block_id: &str,
            _recency_hours: i64,
        ) -> Result<bool, MicrolearningError> {
            Ok(self.recently_seen.contains(block_id))
        }

        fn module_has_due_sr_card(
            &self,
            _learner_id: &str,
            module_id: &str,
            now: DateTime<Utc>,
        ) -> Result<bool, MicrolearningError> {
            *self.observed_now.borrow_mut() = Some(now);
            Ok(self.sr_due_modules.contains(module_id))
        }

        fn decay_days_for_module(
            &self,
            _learner_id: &str,
            _module_id: &str,
        ) -> Result<f64, MicrolearningError> {
            Ok(self.decay_days)
        }
    }

    fn mod_with(id: &str, mastery: f64) -> CandidateModule {
        CandidateModule {
            module_id: id.into(),
            track_id: "trk-1".into(),
            mastery_level: mastery,
            last_bkt_update_at: None,
        }
    }

    #[test]
    fn empty_store_returns_none() {
        let store = StubStore::default();
        let r = select_daily_challenge(&store, "learner-1", fixed_now()).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn selects_block_in_bkt_zone() {
        let store = StubStore {
            modules: vec![mod_with("m1", 0.5)],
            blocks: vec![(
                "m1".into(),
                vec![("blk-1".into(), "section".into(), 0)],
            )],
            ..Default::default()
        };
        let cand = select_daily_challenge(&store, "learner-1", fixed_now())
            .unwrap()
            .expect("must return some candidate");
        assert_eq!(cand.module_id, "m1");
        assert_eq!(cand.block_id, "blk-1");
        assert_eq!(cand.block_type, "section");
    }

    #[test]
    fn excludes_modules_with_no_blocks() {
        let store = StubStore {
            modules: vec![mod_with("m1", 0.5)],
            blocks: vec![], // no blocks for m1
            ..Default::default()
        };
        let r = select_daily_challenge(&store, "learner-1", fixed_now()).unwrap();
        assert!(r.is_none(), "module with no eligible blocks yields none");
    }

    #[test]
    fn applies_recency_penalty_within_48h() {
        // Two blocks in same module; blk-1 is recently seen, blk-2 is not.
        // The algorithm must prefer blk-2.
        let mut seen = HashSet::new();
        seen.insert("blk-1".to_string());
        let store = StubStore {
            modules: vec![mod_with("m1", 0.5)],
            blocks: vec![(
                "m1".into(),
                vec![
                    ("blk-1".into(), "section".into(), 0),
                    ("blk-2".into(), "section".into(), 1),
                ],
            )],
            recently_seen: seen,
            ..Default::default()
        };
        let cand = select_daily_challenge(&store, "learner-1", fixed_now())
            .unwrap()
            .expect("must return un-penalized block");
        assert_eq!(cand.block_id, "blk-2", "recently-seen blk-1 must be skipped");
    }

    #[test]
    fn prefers_sr_due_modules() {
        // Two modules — mod-A has an SR card due, mod-B does not.
        // Both have one block each. mod-A scores higher (W_SR_DUE = 1.2).
        let mut sr_due = HashSet::new();
        sr_due.insert("mod-A".to_string());
        let store = StubStore {
            modules: vec![mod_with("mod-A", 0.5), mod_with("mod-B", 0.5)],
            blocks: vec![
                (
                    "mod-A".into(),
                    vec![("blk-A".into(), "section".into(), 0)],
                ),
                (
                    "mod-B".into(),
                    vec![("blk-B".into(), "section".into(), 0)],
                ),
            ],
            sr_due_modules: sr_due,
            ..Default::default()
        };
        let cand = select_daily_challenge(&store, "learner-1", fixed_now())
            .unwrap()
            .expect("must return SR-due module's block");
        assert_eq!(cand.module_id, "mod-A");
        assert_eq!(cand.block_id, "blk-A");
    }

    #[test]
    fn picks_block_with_lowest_ordering_on_tie() {
        // Same module, two blocks, no SR-due, no recency: tie on score.
        // Tie-break: smaller `ordering` wins.
        let store = StubStore {
            modules: vec![mod_with("m1", 0.5)],
            blocks: vec![(
                "m1".into(),
                vec![
                    // intentionally inserted in REVERSE ordering
                    ("blk-late".into(), "section".into(), 5),
                    ("blk-early".into(), "section".into(), 1),
                ],
            )],
            ..Default::default()
        };
        let cand = select_daily_challenge(&store, "learner-1", fixed_now())
            .unwrap()
            .expect("must return early-ordering block");
        assert_eq!(cand.block_id, "blk-early");
    }

    #[test]
    fn returns_none_when_all_blocks_recently_seen() {
        // Q3 fallback — every candidate hit W_RECENCY.
        let mut seen = HashSet::new();
        seen.insert("blk-1".to_string());
        seen.insert("blk-2".to_string());
        let store = StubStore {
            modules: vec![mod_with("m1", 0.5)],
            blocks: vec![(
                "m1".into(),
                vec![
                    ("blk-1".into(), "section".into(), 0),
                    ("blk-2".into(), "section".into(), 1),
                ],
            )],
            recently_seen: seen,
            ..Default::default()
        };
        let r = select_daily_challenge(&store, "learner-1", fixed_now()).unwrap();
        assert!(r.is_none(), "all blocks recency-penalized => None");
    }

    #[test]
    fn injects_clock_through_to_store() {
        // A5 — the algorithm forwards `now` to the store; tests can pin it.
        let pinned = Utc.with_ymd_and_hms(2099, 12, 31, 23, 59, 59).unwrap();
        let store = StubStore {
            modules: vec![mod_with("m1", 0.5)],
            blocks: vec![(
                "m1".into(),
                vec![("blk-1".into(), "section".into(), 0)],
            )],
            ..Default::default()
        };
        let _ = select_daily_challenge(&store, "learner-1", pinned).unwrap();
        let observed = store.observed_now.borrow().expect("module_has_due_sr_card must be called");
        assert_eq!(
            observed, pinned,
            "the `now` passed to select_daily_challenge must be forwarded to the store unchanged"
        );
    }

    #[test]
    fn decay_signal_affects_score_within_cap() {
        // decay = 6.0 days; W_DECAY * (6.0 / 3.0).min(5.0) = 2.0
        // With no SR-due, no recency, the module's base score is 2.0;
        // module_b with decay = 0 scores 0. mod_a should win.
        let store_high = StubStore {
            modules: vec![mod_with("m1", 0.5)],
            blocks: vec![(
                "m1".into(),
                vec![("blk-1".into(), "section".into(), 0)],
            )],
            decay_days: 6.0,
            ..Default::default()
        };
        let cand = select_daily_challenge(&store_high, "learner-1", fixed_now())
            .unwrap()
            .expect("ok");
        assert!(
            (cand.score - 2.0).abs() < 1e-9,
            "expected decay-contributed score = 2.0, got {}",
            cand.score
        );
    }

    #[test]
    fn microlearning_error_renders_backend() {
        let e = MicrolearningError::Backend("simulated".into());
        assert_eq!(format!("{}", e), "microlearning backend error: simulated");
    }
}
