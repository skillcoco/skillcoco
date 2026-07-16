# SuperMemo 2 (SM-2) Spaced Repetition

> **Author:** SkillCoco OSS contributors
> **Date:** 2026-06-16
> **License:** [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)
> **Module:** [`skillcoco_core::sm2`](../src/sm2.rs)

This whitepaper documents the SuperMemo 2 (SM-2) spaced repetition
algorithm as implemented in `skillcoco_core::sm2`. It is intended for
engineers, learning scientists, and curious learners who want to
understand exactly how SkillCoco schedules card reviews against the
forgetting curve.

The implementation is the canonical SM-2 algorithm published by Piotr
Wozniak in *Optimization of repetition spacing in the course of
learning* (1990). The math is reproduced here so the code, the docs,
and the original literature stay in lockstep.

---

## 1. Why spaced repetition

The single most-replicated finding in cognitive psychology is the
**forgetting curve** (Ebbinghaus, 1885): without active review, recall
decays approximately exponentially over time. A learner who memorizes
a fact today retains roughly:

| Time elapsed | Recall (no review)   |
|--------------|----------------------|
| 20 minutes   | ~ 60%                |
| 1 hour       | ~ 45%                |
| 9 hours      | ~ 35%                |
| 1 day        | ~ 30%                |
| 6 days       | ~ 25%                |
| 31 days      | ~ 20%                |

(Numbers are approximate; Ebbinghaus's original constants varied with
material difficulty and individual memory.)

Two effects work in the learner's favor:

1. **The testing effect** (Roediger & Karpicke 2006): *retrieving*
   information from memory strengthens the trace far more than passively
   re-reading it. Spaced repetition systems exploit this by quizzing
   the learner rather than re-showing the answer.
2. **The spacing effect** (Cepeda et al. 2006): the same total study
   time produces dramatically better long-term retention when spread
   across multiple sessions than when massed into one. *When* you
   review matters as much as *how much* you review.

The optimal interval between reviews depends on three things:
- How recently you reviewed (just now is too soon).
- How well you remembered last time (perfect recall justifies a longer
  next interval than a strained recall).
- How long the current trace has been stable (older traces tolerate
  longer intervals).

SM-2 is a closed-form algorithm that produces a per-card schedule based
on these inputs. It is the foundation of SuperMemo (1985), Anki (2006),
Mnemosyne, and many smaller systems.

SkillCoco uses SM-2 for **card-level spaced repetition** within each
module. Per-module mastery is tracked by
[BKT](./BKT.md); per-card review scheduling is tracked by SM-2. Both
algorithms inform the [microlearning](../src/microlearning.rs)
daily-challenge selector, which combines them with a decay model to
pick the best next interaction for the learner.

---

## 2. SM-2 origin

The SM-2 algorithm was published by Piotr Wozniak in 1990 as part of
his graduate work on the SuperMemo software. Wozniak had been
experimenting with spaced-repetition schedules since 1985; SM-2 was
the second iteration of the algorithm and is the version that survived
into mainstream open-source flashcard tools.

The algorithm has three goals:

1. **Schedule each review just before predicted forgetting.** Too early
   wastes the learner's time; too late lets the memory trace lapse and
   forces re-learning.
2. **Adapt per card.** Cards the learner finds easy stretch their
   intervals quickly; difficult cards stay short until the learner
   demonstrates stable recall.
3. **Be computable without any external state.** SM-2 stores three
   numbers per card (repetitions, ease factor, interval) and updates
   them with closed-form arithmetic. No machine learning, no
   per-learner training data, no opaque parameter estimation.

The simplicity is by design. SM-2 is suboptimal compared to modern
alternatives like FSRS (see §7), but its determinism, auditability, and
zero-state-per-learner profile make it the default for systems where
those properties matter more than the last 5% of efficiency.

---

## 3. The quality scale (0-5)

After each review the learner self-reports how well they recalled the
card on a six-point scale:

| Quality | Semantics                                                 |
|---------|-----------------------------------------------------------|
| `0`     | Complete blackout. No recall at all.                     |
| `1`     | Wrong, but the answer felt familiar once shown.          |
| `2`     | Wrong, but the answer was easy to recall upon seeing it. |
| `3`     | Correct, but with serious difficulty. The boundary pass. |
| `4`     | Correct after hesitation.                                |
| `5`     | Perfect recall. Effortless.                              |

The cutoff is `q >= 3 ⇒ pass`. A quality of `3` is a boundary pass:
the learner technically remembered, but only after notable struggle.
Quality `< 3` triggers the failure-reset rule (§5.3).

SkillCoco's IPC layer surfaces this scale verbatim through the IPC
JSON contract; the in-app UI maps it to plain-English buttons but the
underlying numerical scale is what `sm2_calculate` sees.

---

## 4. The three SM-2 state variables

Each card carries three numbers in storage:

| Field          | Type | Meaning                                              |
|----------------|------|------------------------------------------------------|
| `repetitions`  | i32  | Successful reviews in the current run. Resets to 0  |
|                |      | on any quality `< 3`.                                |
| `ease_factor`  | f64  | Difficulty multiplier. Higher = easier card. Floor  |
|                |      | of `1.3`; no upper bound.                            |
| `interval`     | f64  | Days until next review. Computed by §5.             |

These three numbers, plus the new quality, fully determine the next
`(repetitions, ease_factor, interval)`. The algorithm is **stateless
beyond the card row**.

In SkillCoco the row is persisted in the `sr_cards` SQLite table
(desktop) or an equivalent IndexedDB key (web). The `SrStore` trait in
`skillcoco-core` abstracts the storage so the algorithm itself stays
pure.

---

## 5. The update rules

The algorithm runs three steps in order:

1. Compute the new ease factor (§5.1).
2. Either advance the interval (§5.2) or reset on failure (§5.3).
3. Persist the new triple via the storage trait.

### 5.1 Ease factor update

```text
EF' = EF + (0.1 - (5 - q) * (0.08 + (5 - q) * 0.02))
EF' = max(EF', 1.3)
```

For each quality value the increment evaluates to:

| Quality `q` | `5 - q` | Increment        | Notes                       |
|-------------|---------|------------------|-----------------------------|
| `5`         | `0`     | `+0.10`          | Reward perfect recall       |
| `4`         | `1`     | `+0.00`          | Hold ease steady            |
| `3`         | `2`     | `-0.14`          | Penalize struggle           |
| `2`         | `3`     | `-0.32`          | Heavy penalty               |
| `1`         | `4`     | `-0.54`          | Near floor                  |
| `0`         | `5`     | `-0.80`          | Hard cap at floor           |

The floor at `1.3` prevents pathological cards from collapsing to a
schedule of "review every day forever" — even cards the learner can
never remember are held at the floor and revisited at the floor's
interval. The ease factor is a multiplicative knob on the interval
growth, so a card at floor grows its interval by a factor of `1.3`
per successful review; a card at the standard `2.5` grows by a factor
of `2.5`.

### 5.2 Interval growth (quality `>= 3`)

```text
repetitions == 0  →  interval = 1   (next review in 1 day)
repetitions == 1  →  interval = 6   (next review in 6 days)
repetitions >= 2  →  interval = interval * EF'
repetitions      +=  1
```

The first two intervals are fixed (1 day, 6 days) regardless of ease
factor. From the third repetition onward the interval is multiplied by
the new ease factor. The sequence for a card with `EF = 2.5` and
five perfect reviews is:

```text
review 1: interval = 1   day
review 2: interval = 6   days
review 3: interval = 15  days  (6 * 2.5)
review 4: interval = 37.5 days (15 * 2.5)
review 5: interval = 93.75 days
```

The intervals grow exponentially in the number of successful reviews,
matching the empirical shape of the forgetting curve under successful
recall.

### 5.3 Failure reset (quality `< 3`)

```text
repetitions = 0
interval    = 1   (review again tomorrow)
EF          = max(EF', 1.3)   // ease factor decays per §5.1, then floor
```

A failed review resets the schedule but **preserves the ease-factor
decay**. A card that fails once carries a permanent ease penalty,
which makes its future intervals shorter than they would have been
without the failure — encoding the empirical fact that "leech" cards
(cards the learner repeatedly fails) need shorter intervals than
average even after they finally pass.

### 5.4 Worked example

Start state: fresh card. `EF = 2.5`, `repetitions = 0`, `interval = 0`.

```text
Review 1 (q=5): EF' = 2.5 + 0.10        = 2.60
                repetitions: 0 → 1
                interval: 1 day

Review 2 (q=5): EF' = 2.60 + 0.10       = 2.70
                repetitions: 1 → 2
                interval: 6 days

Review 3 (q=4): EF' = 2.70 + 0.00       = 2.70
                repetitions: 2 → 3
                interval: 6.0 * 2.70    = 16.2 days

Review 4 (q=2): q < 3 → failure-reset
                EF' = 2.70 - 0.32       = 2.38
                repetitions: → 0
                interval: → 1 day

Review 5 (q=5): EF' = 2.38 + 0.10       = 2.48
                repetitions: 0 → 1
                interval: 1 day
```

The card lapsed once, lost some ease, and now restarts the interval
sequence carrying the penalized ease forward. Future review-3 interval
will be `6 * 2.48 = 14.88` days, slightly shorter than the original
`16.2` — encoding the lapse history.

---

## 6. Implementation notes

`skillcoco_core::sm2::sm2_calculate` is a **pure function**: same
`(quality, repetitions, ease_factor, interval)` → same output, no I/O,
no allocation. This matters because:

1. **Tests are deterministic** without time injection.
2. **WASM portability is trivial**: no `std::fs`, no `chrono` (the
   *card row* carries the timestamp; the algorithm only takes the
   `interval` in days). The function compiles unchanged on
   `wasm32-unknown-unknown`.
3. **The algorithm is auditable.** The numeric output for any input is
   reproducible by anyone with a calculator.

Quality is **clamped** at `[0, 5]` inside the function — a caller that
passes `q=7` will be treated as `q=5`. This defensive clamping mirrors
the Phase 6 SR implementation that this code was moved from.

Persistence is **not** part of the algorithm. The `(repetitions,
ease_factor, interval)` triple is serialized to host storage via the
[`SrStore`] trait. Mocking `SrStore` lets you unit-test the higher-
level call sites (review submission, daily-challenge selection)
without ever touching a database.

[`SrStore`]: ../src/sm2.rs

The `SrCardRow` struct on the trait surface carries ISO-8601 date-time
strings for `next_review` and `last_review`. This shape matches the
SQLite reference schema (`datetime('now', ...)` returns TEXT), so the
desktop adapter is a 1:1 row mapping. Web/IndexedDB adapters
serializing dates differently can convert at the trait boundary.

---

## 7. Comparison with FSRS (and choice rationale)

A modern alternative to SM-2 is **FSRS** (Free Spaced Repetition
Scheduler), published by Jarrett Ye in 2023. FSRS uses a learner-
specific parameterized model (`DSR` — difficulty, stability,
retrievability) and is empirically more efficient than SM-2 on most
benchmarks: roughly 20-30% fewer reviews for the same retention
target on standard datasets.

SkillCoco ships SM-2 in Phase 7 for three reasons:

1. **Migration cost.** The pre-Phase-7 SkillCoco already used SM-2;
   replacing it would require migrating every existing `sr_cards`
   row, retraining FSRS parameters per learner, and reconciling the
   different state vectors.
2. **Audibility.** SM-2 has been studied for thirty years; its
   limitations are well-understood and its outputs are by-hand
   reproducible. FSRS requires per-learner gradient-descent fitting
   that is harder to explain and harder to audit.
3. **WASM portability.** SM-2 is closed-form arithmetic. FSRS uses
   per-learner parameter optimization; the standard `fsrs-rs` crate
   pulls `ndarray` and `optimize`, which complicate WASM builds.

A future SkillCoco release may switch to FSRS once per-learner
calibration is available and the migration path is well-mapped. The
trait surface (`SrStore`) was designed to be replaceable — the SM-2
math is confined to `sm2.rs` and can be swapped without touching the
persistence layer.

---

## 8. Limitations

The SM-2 algorithm as implemented in SkillCoco has known
limitations.

### 8.1 Self-reported quality

The quality scale (§3) is set by the learner. Strategic learners can
game it by under-rating their recall to schedule more reviews than
necessary, or over-rating it to push reviews further out. SM-2 has no
defense against this; FSRS variants that fit the model on objective
recall data (time-to-answer, edit distance from the canonical answer)
are more robust.

### 8.2 No per-learner ease initialization

Every new card starts with the same `EF = 2.5`. A learner who is
strong in a subject area gets the same initial interval as one who is
new to it; the model converges over a few reviews but the early
intervals are uncalibrated.

### 8.3 No card-difficulty modeling beyond ease factor

The ease factor is a single per-card number. It cannot distinguish
between "this card is fundamentally harder" and "this learner is
having a bad week". Modern models (FSRS, DKT) separate the two; SM-2
conflates them.

### 8.4 Interval cap absence

SkillCoco does not cap the interval. A card reviewed perfectly six
times will reach an interval of ~94 days; ten perfect reviews and
the interval exceeds five years. This is the canonical SM-2 behavior
and is not necessarily wrong — well-mastered cards genuinely tolerate
long intervals — but some implementations (e.g. Anki) impose a soft
cap to keep the deck visually reasonable.

### 8.5 No leech detection

A card that the learner repeatedly fails (a "leech") will cycle
through the failure-reset rule forever, losing ease each time. SM-2
itself has no leech-detection; Anki added one later (cards that fail
N times get flagged for review or suspension). SkillCoco defers
leech-handling to higher layers.

---

## 9. References

- **Wozniak, P. A. (1990).** Optimization of repetition spacing in
  the course of learning. *Acta Neurobiologiae Experimentalis*,
  50(1-2), 59-62. The canonical SM-2 paper.
- **Ebbinghaus, H. (1885).** *Über das Gedächtnis. Untersuchungen
  zur experimentellen Psychologie.* The original forgetting-curve
  research.
- **Roediger, H. L., & Karpicke, J. D. (2006).** Test-Enhanced
  Learning: Taking Memory Tests Improves Long-Term Retention.
  *Psychological Science*, 17(3), 249-255. The testing effect.
- **Cepeda, N. J., Vul, E., Rohrer, D., Wixted, J. T., & Pashler, H.
  (2006).** Distributed practice in verbal recall tasks: A review
  and quantitative synthesis. *Psychological Bulletin*, 132(3),
  354-380. The spacing effect.
- **Ye, J. (2023).** FSRS: A modern alternative to SuperMemo
  algorithms. https://github.com/open-spaced-repetition/fsrs-rs
- **Elmes, D. (2006).** Anki — open-source flashcard software using
  modified SM-2. https://github.com/ankitects/anki
- **Wozniak, P. A. (1998-2024).** SuperMemo documentation:
  https://supermemo.guru/wiki/Algorithm_SM-2

---

## 10. Reproducing the worked examples

```rust,ignore
use skillcoco_core::sm2::sm2_calculate;

// Review 1 (q=5, fresh card, EF=2.5)
let r = sm2_calculate(5, 0, 2.5, 0.0);
assert_eq!(r.repetitions, 1);
assert_eq!(r.interval, 1.0);
assert!((r.ease_factor - 2.6).abs() < 1e-9);

// Review 4 (q=2, the failure-reset)
let r = sm2_calculate(2, 3, 2.7, 16.2);
assert_eq!(r.repetitions, 0);
assert_eq!(r.interval, 1.0);
assert!((r.ease_factor - 2.38).abs() < 1e-9);
```

Run `cargo run -p skillcoco-core --example sm2_schedule` to see the
interval trajectory printed for a longer review sequence.

---

*This whitepaper is licensed under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). You may
reuse it with attribution to "SkillCoco OSS contributors, 2026".*
