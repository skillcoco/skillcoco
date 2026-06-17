# Microlearning — Daily-Challenge Selection Scoring

> **Author:** LearnForge OSS contributors
> **Date:** 2026-06-17
> **License:** [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)
> **Module:** [`learnforge_core::microlearning`](../src/microlearning.rs)

This whitepaper documents the daily-challenge selection algorithm as
implemented in `learnforge_core::microlearning`. It is intended for
engineers, learning scientists, and curious learners who want to
understand exactly how LearnForge picks "the best next interaction" for
a learner on a given day — combining BKT mastery estimates, SM-2 review
schedules, recency penalties, and a deliberately-chosen mastery "zone"
that targets desirable difficulty.

The selector sits **above** the per-module [BKT model](./BKT.md) and the
per-card [SM-2 algorithm](./SM2.md). Both produce scalar state on a
single module or card; the microlearning selector aggregates the two
signals (plus time decay and recency exclusion) into a per-block score
and picks the highest-scoring block as the day's challenge. The result
appears in the LearnForge UI as the "Daily Challenge" card on the
learner home screen.

---

## 1. Abstract

LearnForge tracks per-module mastery (BKT) and per-card review schedules
(SM-2) on a continuous basis. Both signals are useful for adaptive
instruction, but neither answers the single concrete UX question "what
should the learner do **today**, in five-to-ten minutes, that is
maximally useful?" This whitepaper specifies the pure-functional
selection score that combines (a) BKT mastery being inside a "desirable
difficulty" zone, (b) BKT staleness via a decay term, (c) SM-2 card
being due today, and (d) recency exclusion of recently-seen blocks
into a single ranking. The selector is closed-form, deterministic,
WASM-portable, and runs against an injectable `MicrolearningStore`
trait so the algorithm and the SQL access shapes stay decoupled. The
calibration constants (`0.3-0.7` mastery zone, `48h` recency window,
`3.0-day` decay half-life, `W_DECAY=1.0` / `W_SR_DUE=1.2` /
`W_RECENCY=-100.0`) are grounded in cognitive-science literature on
desirable difficulty and spacing, in the BKT mastery threshold from
Phase 7, and in empirical UX testing on the LearnForge daily-challenge
flow.

---

## 2. Problem statement — why naive "pick the next module" fails

The desktop adaptive engine has, at any given moment, many things it
could surface to a learner who has five minutes:

1. **The next un-mastered module** in the active track (`mastery <
   0.7`).
2. **A module the learner mastered weeks ago** whose memory has
   probably decayed.
3. **An SM-2 card** whose `next_review` timestamp is at or before
   today (the canonical spaced-repetition signal).
4. **A new module** in a track the learner just enrolled in.
5. **A module that depends on a prerequisite** the learner has not
   yet mastered.

A naive "always pick the next un-mastered module" policy fails because
it ignores spacing, fails the learner whose memory of an old module is
fading, and creates a monotonic linear-progression experience that
defeats the entire point of microlearning ("five-minute interesting
practice, every day").

A naive "always pick the next due SM-2 card" policy fails because it
ignores BKT mastery (a learner who is brand-new to a track has no SM-2
cards to be due on) and skips the broader "module-level practice" use
case.

A naive "pick a random module from the candidate set" policy is
defensible — but it forfeits the algorithm's ability to **prefer**
certain blocks, and burns the learner's attention on suboptimal
interactions. Five minutes a day is a precious budget; spending two of
them on a card the learner already saw yesterday is a real cost.

The daily-challenge selection score solves all of these by:

- Restricting candidates to modules in the **desirable-difficulty zone**
  (`BKT mastery ∈ [0.3, 0.7)`) — too-easy modules are mastered and
  out; too-hard modules are gated by prerequisites and have nothing
  yet to practice.
- Boosting modules whose BKT estimate is **stale** (decay term).
- Boosting modules with a **due SM-2 card** (review schedule term).
- Penalizing blocks the learner has **recently seen** (48-hour window).
- Returning the single highest-scoring block via deterministic
  tie-breaking on `(ordering, block_id)`.

The result is a closed-form score per block that captures all four
signals without ML, without per-learner training, and without any
non-determinism.

---

## 3. The algorithm

### 3.1 Inputs and the storage trait

The algorithm consumes a `MicrolearningStore` trait that provides five
data-access methods. Each method is a single SQL query in the desktop
adapter; the algorithm itself never sees the SQL.

```rust,ignore
pub trait MicrolearningStore {
    fn candidate_modules(&self, learner_id: &str)
        -> Result<Vec<CandidateModule>, MicrolearningError>;
    fn blocks_for_module(&self, module_id: &str)
        -> Result<Vec<(String, String, i32)>, MicrolearningError>;
    fn is_recently_seen(&self, learner_id: &str, block_id: &str, recency_hours: i64)
        -> Result<bool, MicrolearningError>;
    fn module_has_due_sr_card(&self, learner_id: &str, module_id: &str, now: DateTime<Utc>)
        -> Result<bool, MicrolearningError>;
    fn decay_days_for_module(&self, learner_id: &str, module_id: &str)
        -> Result<f64, MicrolearningError>;
}
```

The algorithm body — `select_daily_challenge` — takes a `&S:
MicrolearningStore`, a `learner_id`, and a `now: DateTime<Utc>`. The
wall-clock instant is **injected** as a parameter so unit tests can
pin a deterministic time and WASM builds never accidentally read the
Unix epoch (`Utc::now()` returns `1970-01-01` on
`wasm32-unknown-unknown` without `wasmbind`).

### 3.2 The tuning constants

Six tuning constants govern the score (all `const` — Q5 lock from
Phase 4 research):

```rust,ignore
pub const DECAY_HALF_LIFE_DAYS: f64 = 3.0;
pub const RECENCY_PENALTY_HOURS: i64 = 48;
pub const W_DECAY: f64 = 1.0;
pub const W_SR_DUE: f64 = 1.2;
pub const W_RECENCY: f64 = -100.0;
pub const BKT_LOWER: f64 = 0.3;
pub const BKT_UPPER: f64 = MASTERY_THRESHOLD;  // 0.7
pub const DECAY_DAYS_CAP_MULT: f64 = 5.0;
```

| Constant                | Role                                                                                |
| ----------------------- | ----------------------------------------------------------------------------------- |
| `DECAY_HALF_LIFE_DAYS`  | Days at which the decay-contribution doubles relative to "one day stale."            |
| `RECENCY_PENALTY_HOURS` | Block-seen window — a block seen within this window scores `W_RECENCY`.              |
| `W_DECAY`               | Linear weight on the BKT-decay signal.                                              |
| `W_SR_DUE`              | Linear bonus when the module has at least one due SM-2 card.                         |
| `W_RECENCY`             | Hard penalty (large negative) for recently-seen blocks; effectively excludes them.   |
| `BKT_LOWER`             | Lower bound of the candidate mastery zone (the "struggle zone").                     |
| `BKT_UPPER`             | Upper bound — reuses [`MASTERY_THRESHOLD`](./BKT.md) for a single source of truth.   |
| `DECAY_DAYS_CAP_MULT`   | Cap on the decay multiplier so months-old modules don't dominate forever.            |

The constants are `pub const`, not env vars or config-file keys, so the
calibration is part of the published API. Changing them is a minor
version bump per [LearnForge versioning policy](../../CHANGELOG.md).

### 3.3 The selection score (per block)

For each candidate block in each candidate module the algorithm
computes a scalar score:

```text
score(block) = W_DECAY * min(decay_days / DECAY_HALF_LIFE_DAYS, DECAY_DAYS_CAP_MULT)
             + (W_SR_DUE if module_has_due_sr_card else 0)
             + (W_RECENCY if block_recently_seen else 0)
```

The three contributions add linearly. The recency penalty is large
enough (`-100`) that any recently-seen block falls far below any
fresh block; the algorithm then picks the highest-scoring block in the
candidate set and tie-breaks deterministically.

#### Decomposition

1. **Decay contribution** —
   `W_DECAY * min(decay_days / DECAY_HALF_LIFE_DAYS, DECAY_DAYS_CAP_MULT)`

   `decay_days` is the SQL-computed
   `julianday('now') - julianday(last_bkt_update_at)`. A module touched
   today contributes `0`. A module touched three days ago contributes
   `1.0` (one half-life). A module touched fifteen days ago contributes
   `5.0` (the cap — without it, a module touched a year ago would
   dominate any SM-2-due signal).

2. **SR-due contribution** — `W_SR_DUE` (constant `1.2`) added if the
   module has at least one SM-2 card with `next_review <= now`.

   The slight bias toward SR-due modules (the constant is `1.2`, not
   `1.0`) reflects the empirical observation that spaced-repetition
   reviews have a stronger evidence base for long-term retention than
   does generic module re-exposure. When a learner has both a decaying
   module and a due card, the algorithm prefers the due card.

3. **Recency penalty** — `W_RECENCY` (`-100.0`) added if the block was
   shown to this learner within the last `RECENCY_PENALTY_HOURS` (48h).

   The recency signal is intentionally a **hard** penalty rather than
   a smooth one. The product decision is that a daily challenge MUST
   NOT show the same block two days running unless every alternative
   is also recently-seen (which triggers the empty-zone fallback in
   §3.5).

#### Candidate filter (Step 1)

Before scoring, the algorithm filters modules to the candidate set:

- **BKT mastery in `[BKT_LOWER, BKT_UPPER) = [0.3, 0.7)`** — the
  "struggle zone." A learner has done enough on the module to have a
  non-trivial mastery estimate but has not yet mastered it.
- **Active track only** — modules outside the learner's currently-active
  track are excluded.

Modules outside the zone are skipped entirely. Never-touched modules
(no `module_progress` row) are excluded — the storage adapter
implements this filter via the SQL aggregate.

Within each candidate module, the eligible blocks are filtered by:

- `status = 'ready'` — modules with draft / under-review blocks
  don't surface them.
- `block_type IN ('flash_cards', 'quiz', 'section')` — the three
  block types that support a one-shot microlearning interaction.

A module with zero eligible blocks is silently skipped.

### 3.4 Worked example

Suppose a learner has three candidate modules at a given moment:

| Module    | mastery | decay_days | sr_due | blocks (id, type, ordering)                |
| --------- | ------- | ---------- | ------ | ------------------------------------------ |
| `m-pods`  | `0.45`  | `9.0`      | false  | `[(blk-pods-1, section, 0)]`               |
| `m-svcs`  | `0.62`  | `1.0`      | true   | `[(blk-svcs-1, flash_cards, 0)]`           |
| `m-ingrs` | `0.55`  | `2.0`      | false  | `[(blk-ingrs-1, quiz, 0), (blk-ingrs-2, section, 1)]` |

`blk-svcs-1` was shown to this learner 12 hours ago (within the
48-hour window).

Per-block scoring:

```text
blk-pods-1 :  W_DECAY * min(9.0 / 3.0, 5.0)   + 0    + 0
            = 1.0   * min(3.0, 5.0)            + 0    + 0
            = 3.0

blk-svcs-1 :  W_DECAY * min(1.0 / 3.0, 5.0)   + 1.2  + (-100)
            = 1.0   * 0.333                    + 1.2  - 100
            ≈ -98.47

blk-ingrs-1:  W_DECAY * min(2.0 / 3.0, 5.0)   + 0    + 0
            = 1.0   * 0.667                    + 0    + 0
            ≈ 0.67

blk-ingrs-2:  (same module) → same module-base score
            ≈ 0.67
```

`blk-pods-1` wins with score `3.0`. The most-stale module triumphs over
the SR-due module because the SR-due module's only block was recently
seen. If the recency-penalty had not fired, `blk-svcs-1` would have
scored `0.333 + 1.2 = 1.53` — beaten by `blk-pods-1`'s `3.0` decay
contribution. Stale-by-several-half-lives is the strongest signal in
this scenario.

### 3.5 The empty-zone fallback

A learner may have nothing useful to surface — either the BKT zone is
empty (no module in `[0.3, 0.7)`) or every candidate block has been
recently seen. The algorithm returns `Ok(None)` in both cases.

The recency-only fallback condition is:

```text
if every score <= W_RECENCY / 2  →  return None
```

Why `W_RECENCY / 2 = -50` as the cutoff? Even the maximum non-penalty
contribution (`W_DECAY * DECAY_DAYS_CAP_MULT + W_SR_DUE = 5.0 + 1.2
= 6.2`) cannot bring a recency-penalized block (`-100`) above `-50`.
Conversely any non-penalized block scores at least `0`, far above
`-50`. The `/2` cutoff is therefore the unique value that distinguishes
"all blocks penalized" from "at least one viable block exists."

The frontend renders the `None` case as the "no challenge today"
placeholder card. This is the third "Q3 fallback" lock from the
Phase 4 research notes.

### 3.6 Deterministic tie-breaking

After scoring, the algorithm sorts the candidate list by:

```text
1. score descending (highest first)
2. ordering ascending (lower-numbered block first)
3. block_id ascending (lexicographic, final tiebreaker)
```

The triple sort is total: any two distinct blocks have a unique
ordering. The algorithm therefore returns a **single** candidate per
call, and that candidate is reproducible given identical store state.
Tests pin a fixed `now` and assert the exact block id.

---

## 4. Calibration in LearnForge

### 4.1 Why the `0.3-0.7` mastery zone?

The candidate filter restricts to modules with BKT mastery
`m ∈ [0.3, 0.7)`. This is the "desirable difficulty" zone — small
enough to be focused, wide enough to capture most in-progress modules.

The lower bound (`0.3`) is the BKT prior `P(L_0)` for an adult learner
on a new module. A learner with `m < 0.3` has only just been introduced
to the topic (or has decayed back below the prior — see §6.1) and is
better served by the linear "next-module" path than by a microlearning
challenge. Surfacing a brand-new module as the daily challenge skips
the introductory content the learner needs to see first.

The upper bound (`0.7`) reuses the project-wide
[`MASTERY_THRESHOLD`](./BKT.md). Modules at or above `0.7` are
"mastered" — the prerequisite gate has unblocked downstream modules,
the certification math counts them, and the learner has demonstrated
working competence. Surfacing them again as the daily challenge wastes
the learner's time on something they have already proven.

The zone is closed-open `[0.3, 0.7)` rather than closed-closed
`[0.3, 0.7]` to make the upper-bound semantics unambiguous: `m = 0.7`
exactly is a mastered module, not a struggle-zone candidate.

### 4.2 The "struggle zone" — Vygotsky-style ZPD

The `0.3-0.7` zone matches the educational-psychology concept of the
**Zone of Proximal Development** (ZPD; Vygotsky, 1978): the band of
tasks where the learner can succeed with effort but cannot succeed
trivially. Below the ZPD, instruction is wasted on material the
learner already knows; above it, the learner gets stuck. Targeting
the ZPD is one of the most-replicated findings in educational
intervention research.

The BKT mastery interval `[0.3, 0.7)` is the LearnForge operational
proxy for the ZPD. A learner with mastery `0.45` on a module has
seen the introductory material (so `m > 0.3`), has gotten some
questions right and some wrong (so `m < 0.7`), and is in the band
where another five-minute interaction has the highest expected
learning value.

This is the same "desirable difficulty" framing as
**Bjork & Bjork (2011)**: items that are *neither* trivially easy
*nor* unsolvably hard maximize encoding and retrieval strength.
The LearnForge zone is one operational instantiation of that
framing, calibrated against the specific scale of the BKT model.

### 4.3 Why the `48h` recency window?

The recency penalty excludes blocks seen in the last 48 hours. This
choice comes from three constraints:

1. **The forgetting curve.** [SM-2 / Ebbinghaus](./SM2.md) data show
   recall dropping to roughly 30% within 24 hours and roughly 25%
   within six days for unreviewed material. Re-showing a block at
   24 hours is too soon — the trace is still strong. Re-showing at
   72 hours is allowable. 48 hours is the midpoint of the
   "just-shown" tail of the forgetting curve.
2. **The daily-challenge cadence.** LearnForge expects roughly one
   daily challenge per learner per day. A 48-hour window therefore
   excludes exactly *the previous two daily challenges' blocks*
   from the next selection. This produces a visible rotation
   pattern: a learner never sees the same block on consecutive days.
3. **The empty-zone fallback safety.** A 24-hour window would
   risk near-empty zones for learners with small candidate sets;
   72-hour would risk learners feeling the rotation is sluggish.
   48 hours is the engineering trade-off.

The window is measured per-block, not per-module. Two blocks in the
same module can both surface in consecutive days as long as neither
individual block was shown twice — this captures the intuition that
re-encountering a module is fine, but re-encountering the *exact same
question* feels like a software bug.

The "recency" history is read from the `daily_challenges` table only
(Phase 4 Q6 lock). Regular module-progress views or quiz-answer
events do NOT count as "recently seen" for this purpose — the
microlearning selector specifically tracks its own historical
surfaces.

### 4.4 Why the `3.0-day` decay half-life?

The decay term measures how stale a module's BKT estimate is. A
half-life of 3 days means:

- `0 days` stale → decay-contribution = `0` (no boost).
- `3 days` stale → decay-contribution = `W_DECAY * 1.0 = 1.0`.
- `6 days` stale → decay-contribution = `W_DECAY * 2.0 = 2.0`.
- `15 days+` stale → decay-contribution = `W_DECAY * 5.0 = 5.0`
  (cap).

This is a **linear** schedule, not exponential — the half-life is a
labeling convention, not a true forgetting-curve coefficient. The
choice of "linear, capped at 5x" reflects the fact that:

1. The BKT module already handles per-observation mastery updates
   correctly. The decay term is a *priority* signal, not a
   *correction* signal — it nudges the daily challenge toward
   stale modules without overwriting the underlying mastery.
2. A true exponential decay would produce decay-contribution =
   `~32x` for a 15-day-old module, which would dominate every
   other signal forever. The linear-with-cap shape produces a
   plateau where months-stale modules and weeks-stale modules
   score similarly, freeing the algorithm to choose between them
   on other dimensions (SR-due, tie-break).
3. The 3-day half-life matches the typical work-week cadence:
   a learner who takes the weekend off does not get a "you have
   stale modules!" deluge on Monday; they get a moderate boost
   that mixes with the SR-due signal.

### 4.5 Why the SR-due weight is `1.2`?

The SM-2-due bonus is `W_SR_DUE = 1.2`. Numerically this is
slightly more than one decay-half-life: a fresh SR-due module
(`decay_days = 1`) scores `0.33 + 1.2 = 1.53`, comparable to a
five-day-stale non-SR-due module (`5.0/3.0 = 1.67`).

The intent is that SM-2 reviews carry *slightly* more weight than
generic mastery decay, capturing the literature finding (testing
effect; Roediger & Karpicke 2006) that retrieval practice is more
effective per minute than re-exposure. The constant is small (`1.2`,
not `5.0`) because the algorithm should not always pick the SR-due
module — a deeply-stale non-SR module is still the better choice in
many cases.

### 4.6 Why the `W_RECENCY = -100` magnitude?

The recency penalty is `-100`. This is **two orders of magnitude**
larger than any positive contribution. Numerically:

- Max positive contribution: `W_DECAY * DECAY_DAYS_CAP_MULT + W_SR_DUE
  = 5.0 + 1.2 = 6.2`.
- Recency penalty: `-100`.
- Net: a recently-seen block can never out-score a fresh block.

The two-orders-of-magnitude gap is intentional. The recency signal is
a **hard exclusion**, not a soft weight. If it were close to the
positive-contribution magnitude, a deeply-stale recently-seen block
could still surface — defeating the rotation invariant. By making
the penalty effectively infinite, the algorithm guarantees that the
"never see the same block two days running" UX promise holds for any
combination of decay / SR-due signals.

The cliff-shaped penalty is also what makes the empty-zone fallback
detectable: every penalized block scores `< -50`, every fresh block
scores `>= 0`, so the cutoff at `-50` cleanly separates the two
classes.

### 4.7 The cap on the decay multiplier

`DECAY_DAYS_CAP_MULT = 5.0` caps the decay multiplier at 5 half-lives
(15 days). Without it, a module touched once a year ago would have a
decay contribution of `~122`, which would dominate every other signal
forever — the algorithm would surface ancient stale modules over
recently-active learning. The cap turns the "weeks-stale" and
"months-stale" cases into the same priority class, freeing the
algorithm to choose between them on other dimensions.

---

## 5. Implementation notes

### 5.1 Purity, determinism, WASM portability

`learnforge_core::microlearning::select_daily_challenge` is a **pure
function** in the strictest sense (modulo the `&S: MicrolearningStore`
trait calls, which the test suite stubs deterministically):

- Same `(store-state, learner_id, now)` → same `Candidate` output.
- No I/O, no `std::fs`, no Tauri, no `rusqlite`.
- The wall-clock `now` is injected as a `DateTime<Utc>` parameter
  (A5 lock — Pitfall 10 mitigation). Tests pin `2026-06-16T12:00:00Z`
  and the algorithm is fully reproducible.

WASM portability is verified by Phase 7 Wave 5's `tests/wasm.rs`
which builds the crate on `wasm32-unknown-unknown` and runs a smoke
test. The `chrono` workspace dep enables the `wasmbind` feature so
`Utc::now()` returns wall-clock time (not the Unix epoch) when run
in a browser.

### 5.2 The storage trait pattern

The five-method `MicrolearningStore` trait keeps the algorithm
decoupled from the SQL access shape. Each method corresponds to one
SQL query in the desktop adapter (`src-tauri/src/storage_impl/microlearning.rs`).
Web/WASM consumers implement the trait against IndexedDB or any other
backend without touching the algorithm code.

A trait-based seam (rather than a generic data parameter) lets the
algorithm stream results — `blocks_for_module` is called once per
candidate module, not once for the full set — so a learner with many
candidates does not require loading the entire `module_blocks` table
into memory.

### 5.3 The `Backend(String)` error

`MicrolearningError::Backend(String)` stringifies the underlying
storage error at the trust boundary. `rusqlite::Error` (or
`idb::Error`, etc.) never leaks into the public surface of
`learnforge-core` (T-07-05 mitigation). This matches the
`BktError` / `SrError` pattern.

The downside is that callers cannot programmatically pattern-match
on specific storage failure modes; the upside is that the public
surface of `learnforge-core` stays free of storage-implementation
detail and the crate compiles on any target the trait can be
implemented for.

### 5.4 No async, no async-trait

The algorithm is fully synchronous. The trait methods return
`Result<T, MicrolearningError>`, not `Future<Output = Result<T, ...>>`.
This is deliberate:

- BKT, SM-2, and threshold all run synchronously; the daily-challenge
  selector aligns with that pattern.
- The SQL adapter is `rusqlite` (synchronous); making the trait async
  would add an async wrapper for no benefit on desktop.
- The desktop selector runs on a Tokio blocking task spawned from the
  IPC handler; making the underlying algorithm async would not add
  parallelism (the SQL queries are short and would not yield).

A future migration to a fully async desktop runtime would require a
sister `MicrolearningStoreAsync` trait; until then, sync is correct.

---

## 6. Limitations

### 6.1 Cold-start trajectory

A learner with no `module_progress` rows yet has zero candidates in
the `[0.3, 0.7)` zone. The algorithm returns `Ok(None)` and the
frontend shows the "no challenge today" placeholder.

This is correct but conservative. A future extension could fall
back to a "next-module recommendation" when the candidate zone is
empty — but that crosses a conceptual line (microlearning becomes
indistinguishable from the linear path) and was rejected for Phase
4. The cold-start trajectory is a known limitation that the UI
mitigates by surfacing the active path's next module elsewhere on
the home screen.

### 6.2 Tie-break determinism vs. fairness

The deterministic tie-break on `(ordering, block_id)` makes the
algorithm reproducible but produces a **bias** — when many blocks
tie on score, the lowest-ordering / lowest-block-id block always
wins. A learner who chronically ties on score could see the same
block "win" on every tied selection.

This is mitigated in practice by:
1. The recency penalty — a previously-selected block is excluded
   from the next 48 hours.
2. The variety in score from decay + SR-due — true ties are rare.

But it is a known shape of the algorithm. A randomized tie-break
(seeded on `learner_id || now.timestamp() % 86400`) would distribute
ties uniformly while keeping daily reproducibility. This is a
backlogged option.

### 6.3 The `0.3-0.7` zone is a hard cutoff

A learner with mastery `0.71` on a module that they last touched
two weeks ago is **not** a microlearning candidate. The cutoff is
strict. In real learning that learner might benefit from a stale-
mastery refresher — and indeed, decay-adjusted mastery could pull
them back into the zone — but the snapshot mastery (from the
`module_progress.mastery_level` column) does not. The decay term
is for *priority*, not for *candidacy*.

A future extension could compute a decay-adjusted mastery on the
fly and use that for the candidate filter, but this complicates
the SQL aggregate and tests. Phase 7 ships the simpler shape.

### 6.4 Single-block selection

The algorithm returns at most one `Candidate`. A learner who has
multiple useful blocks does not get a queue; they get the single
top-scoring block. The product decision is that a daily challenge
is one focused interaction, not a session.

A multi-block daily plan (e.g., "do these three things today") is
a separate feature, not a generalization of this algorithm.

### 6.5 Decay is at the module level, not block level

The `decay_days_for_module` method returns one number per module.
Blocks within a module share the same decay contribution. A learner
who has seen the flashcards block recently but not the quiz block
gets the same per-module decay signal for both. The recency penalty
operates at the block level, so the *recently-seen* block is still
penalized — but the decay term does not differentiate.

This is a deliberate simplification. Per-block decay would require
per-block "last touched" tracking, which the schema does not currently
support. The current shape is sufficient for the daily-challenge
use case.

### 6.6 No multi-learner / cohort awareness

Each call selects for one `learner_id`. There is no signal of
"this block is hot in your cohort right now" or "your peers are
working on this concept." Phase 11+ may add cohort signals; the
current algorithm is single-learner-only.

### 6.7 No learner-specified preferences

A learner cannot say "prefer flashcards" or "skip videos" — block-
type preferences are not part of the score. The block-type filter
is hard-coded to `('flash_cards', 'quiz', 'section')` in the SQL
adapter; a future product extension could surface a per-learner
preference and add a weight on `block_type` to the score.

### 6.8 No track-weighting

A learner enrolled in multiple active tracks gets one candidate
list pooled across all of them, with no per-track quota or weighting.
Tracks compete for the daily-challenge slot purely on score.
A learner working primarily on track A might see most challenges
from track B if B has staler modules. This was acceptable in Phase
4 design but is a known UX wrinkle for multi-track learners.

---

## 7. References

- **Phase 4 D-03 / D-05 (LearnForge):** Daily-challenge selection
  scoring formula + 0.3-0.7 zone + 48h recency window + decay
  half-life calibration. See `.planning/ROADMAP.md` §Phase 4:
  Adaptive Practice.
- **Phase 4 A5 lock (LearnForge):** Clock injection via parameter
  (`now: DateTime<Utc>`) to avoid Unix-epoch trap on WASM.
- **Phase 4 Q5 lock (LearnForge):** Tuning constants are `const`,
  not env vars or config keys — calibration is part of the
  published API.
- **Phase 4 Q6 lock (LearnForge):** Recency window measured against
  `daily_challenges` history only, not the broader module-progress
  views.
- **Phase 7 Wave 4 / 07-04 (LearnForge):** Move from
  `src-tauri/src/learning/microlearning_selection.rs` into
  `learnforge-core`, behind the `MicrolearningStore` trait. See
  `.planning/phases/07-core-extraction/07-04-SUMMARY.md`.
- **Vygotsky, L. S. (1978).** *Mind in Society: The Development of
  Higher Psychological Processes.* Harvard University Press. The
  original Zone of Proximal Development theory underlying the
  `0.3-0.7` zone choice.
- **Bjork, R. A., & Bjork, E. L. (2011).** Making things hard on
  yourself, but in a good way: Creating desirable difficulties to
  enhance learning. In M. A. Gernsbacher et al. (Eds.),
  *Psychology and the real world: Essays illustrating fundamental
  contributions to society*, 56-64. The "desirable difficulty"
  framing.
- **Cepeda, N. J., Vul, E., Rohrer, D., Wixted, J. T., & Pashler, H.
  (2006).** Distributed practice in verbal recall tasks: A review
  and quantitative synthesis. *Psychological Bulletin*, 132(3),
  354-380. Spacing-effect literature underlying the 48h window.
- **Roediger, H. L., & Karpicke, J. D. (2006).** Test-Enhanced
  Learning: Taking Memory Tests Improves Long-Term Retention.
  *Psychological Science*, 17(3), 249-255. Testing-effect
  literature underlying the `W_SR_DUE = 1.2` slight bias.
- **Ebbinghaus, H. (1885).** *Über das Gedächtnis. Untersuchungen
  zur experimentellen Psychologie.* The original forgetting-curve
  empirical work; underlies the decay half-life choice.
- **Duolingo half-life regression.** Duolingo (2016): "A
  trainable spaced repetition model for language learning." ACL
  2016. Cross-reference for industry-standard half-life
  calibration in microlearning UX.
- **Wozniak, P. A. (1990).** Optimization of repetition spacing
  in the course of learning — see [SM2.md](./SM2.md) for the SR
  scheduler the SR-due signal is sourced from.
- **LearnForge whitepaper:** [BKT](./BKT.md) — per-module mastery
  model the candidate filter consumes.
- **LearnForge whitepaper:** [SM2](./SM2.md) — per-card SR
  scheduler whose `next_review` timestamps drive `W_SR_DUE`.
- **LearnForge whitepaper:** [THRESHOLD](./THRESHOLD.md) — the
  track-level certification ladder that mastery dynamics
  ultimately feed.

---

## 8. Reproducing the worked example

```rust,ignore
use chrono::{TimeZone, Utc};
use learnforge_core::microlearning::{select_daily_challenge, /* ... */};

let now = Utc.with_ymd_and_hms(2026, 6, 16, 12, 0, 0).unwrap();

// (Stub implementation of `MicrolearningStore` omitted for brevity;
// see `learnforge-core/src/microlearning.rs` tests for a complete
// runnable example.)
let result = select_daily_challenge(&store, "learner-1", now).unwrap();

// In the §3.4 scenario, the algorithm returns `blk-pods-1`:
let cand = result.expect("non-empty candidate set");
assert_eq!(cand.block_id, "blk-pods-1");
assert!((cand.score - 3.0).abs() < 1e-9);
```

The pure-store tests in `learnforge-core/src/microlearning.rs` (10
tests covering empty store, BKT-zone candidate, no-blocks exclusion,
recency penalty, SR-due preference, ordering tie-break,
all-recently-seen fallback, clock injection, decay-signal cap, and
error rendering) serve as the executable specification for the
algorithm.

---

*This whitepaper is licensed under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). You may
reuse it with attribution to "LearnForge OSS contributors, 2026".*
