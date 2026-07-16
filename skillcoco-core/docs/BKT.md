# Bayesian Knowledge Tracing (BKT)

> **Author:** SkillCoco OSS contributors
> **Date:** 2026-06-16
> **License:** [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)
> **Module:** [`skillcoco_core::bkt`](../src/bkt.rs)

This whitepaper documents the Bayesian Knowledge Tracing (BKT) model as
implemented in `skillcoco-core::bkt`. It is intended for engineers,
learning scientists, and curious learners who want to understand exactly
how SkillCoco estimates per-skill mastery from a stream of correct /
incorrect observations.

The implementation is the canonical four-parameter BKT model first
articulated by Corbett & Anderson (1995). The math is reproduced here so
the code, the docs, and the academic literature stay in lockstep.

---

## 1. Why Bayesian Knowledge Tracing

Traditional progress tracking treats learning as a binary, monotonic
state — either the learner has completed a module or they have not.
This conflates **exposure** with **mastery** and fails three real-world
scenarios at the heart of practical technical training:

1. **A learner reads a module but cannot answer a single question.**
   Completion percentage is `100%`, mastery is near zero.
2. **A learner answers correctly by guessing.** One observation is not
   evidence of stable knowledge; the signal-to-noise ratio of a single
   correct answer is poor.
3. **A learner slips and answers incorrectly despite stable knowledge.**
   Treating one wrong answer as evidence of complete ignorance is
   disastrous for adaptive systems that gate further content on it.

BKT addresses all three by modeling mastery as a **latent probability**
rather than a binary state. The estimate is updated incrementally by
Bayes' rule on every observation, and the model accommodates both
**guessing** (`P(G)`) and **slipping** (`P(S)`) as first-class
phenomena.

The result is a continuous mastery score in `[0, 1]` that integrates
naturally with adaptive instruction policies: gate further content at a
threshold, surface review when mastery decays, drive spaced-repetition
schedules, and compute course-level competency without lossy rounding.

SkillCoco uses BKT for **per-module mastery estimation**. The track-
level skill-tier predicates (Associate / Practitioner / Professional)
live in [`skillcoco_core::threshold`](../src/threshold.rs) and aggregate
per-module mastery into track-level certification readiness.

---

## 2. The four-parameter model

BKT posits two latent variables and four parameters per skill.

**Latent variable.** `L_n` ∈ `{0, 1}` — whether the learner has mastered
the skill **after** the `n`-th observation. The learner sees only
correct / incorrect responses; the model never observes `L_n`
directly.

**Observed variable.** `O_n` ∈ `{correct, incorrect}` — the response
to the `n`-th question on the skill.

The four parameters govern how `L_n` evolves and how `O_n` is generated.

### `P(L_0)` — prior knowledge

The probability the learner has mastered the skill **before** any
observation. Equivalent to the initial belief that the learner already
knows the material upon first encountering it.

SkillCoco default: **0.3**. This reflects the empirical reality that
adult learners enrolling in a technical course often bring partial prior
exposure — they have heard of Docker, they have a vague idea what a
Kubernetes pod is — but cannot reliably answer questions on the topic.
A prior of `0.3` keeps the model from punishing the first incorrect
answer too harshly while still leaving substantial room for the
posterior to grow.

### `P(T)` — transition (learning) rate

The probability that an unmastered learner becomes mastered between
observations `n` and `n+1`. Captured per observation — so a learner who
sees one question gets one chance to learn.

SkillCoco default: **0.1**. This conservative value reflects that one
question (read-and-respond) provides limited learning signal. Combined
with a `P(L_0) = 0.3`, it takes roughly five to seven correct
observations to drive posterior mastery from `0.3` to the
[mastery threshold](#5-mastery-threshold) of `0.7`.

### `P(G)` — guess

The probability that an **unmastered** learner answers correctly anyway.
Models the fact that multiple-choice questions have non-zero floor
probability and that contextual reasoning can sometimes produce a
correct answer without underlying mastery.

SkillCoco default: **0.2**. Calibrated against the mix of four-option
MCQs (P(G) ≈ 0.25 floor) and short-form / typed-answer questions
(P(G) ≈ 0.1 floor). The blended `0.2` averages the two modalities and
matches the common BKT calibration in the literature.

### `P(S)` — slip

The probability that a **mastered** learner answers incorrectly. Models
fatigue, typos, ambiguous question wording, and the noise floor of human
performance on any task.

SkillCoco default: **0.1**. Per-skill calibration is possible (a
notoriously ambiguous question set could justify a higher slip rate)
but Phase 7 ships a uniform default.

---

## 3. The update equation

On every observation the model applies Bayes' rule to compute the
posterior probability of mastery given the response, then applies the
learning step to get the new mastery estimate.

Let `m = P(L_n = 1 | O_{1..n})` be the current mastery estimate (this is
the value persisted to storage). Let `O_{n+1}` be the new observation.

### 3.1 Bayesian update

```text
P(L_n = 1 | O_{n+1} = correct)   = m * (1 - P(S))
                                   / [ m * (1 - P(S)) + (1 - m) * P(G) ]

P(L_n = 1 | O_{n+1} = incorrect) = m * P(S)
                                   / [ m * P(S) + (1 - m) * (1 - P(G)) ]
```

This is the **conditional probability that the learner had already
mastered the skill at observation `n`**, given the new evidence.

### 3.2 Learning step

```text
P(L_{n+1} = 1) = P(L_n = 1 | O_{n+1}) + (1 - P(L_n = 1 | O_{n+1})) * P(T)
```

The conditional posterior is the lower bound on the new mastery (the
learner certainly hasn't *un*learned the skill); the additive `P(T)`
term reflects that even if they hadn't mastered it, this observation
gave them a chance to. The result is clamped to `[0, 1]` (numerically
the operations cannot exceed `1` but defensive clamping protects against
floating-point drift).

The Rust implementation is `skillcoco_core::bkt::update_mastery`:

```rust,ignore
pub fn update_mastery(params: &BKTParams, prior: f64, is_correct: bool) -> f64 {
    let conditional = if is_correct {
        let num = prior * (1.0 - params.p_slip);
        let den = num + (1.0 - prior) * params.p_guess;
        num / den
    } else {
        let num = prior * params.p_slip;
        let den = num + (1.0 - prior) * (1.0 - params.p_guess);
        num / den
    };
    (conditional + (1.0 - conditional) * params.p_learn).clamp(0.0, 1.0)
}
```

### 3.3 Worked example

Start state: `params = default()`, `m = 0.3`. Observation: correct.

```text
conditional = 0.3 * (1 - 0.1) / [ 0.3 * (1 - 0.1) + (1 - 0.3) * 0.2 ]
            = 0.27 / [ 0.27 + 0.14 ]
            = 0.27 / 0.41
            ≈ 0.6585

m_new = 0.6585 + (1 - 0.6585) * 0.1
      ≈ 0.6927
```

One correct observation moves the mastery estimate from `0.30` to
`0.69` — large jumps are expected when the prior is far from the
posterior. A second correct observation moves it to `≈ 0.92`. A third
to `≈ 0.98`. The model saturates near `1.0`; further observations
contribute diminishing evidence.

A worked example for an **incorrect** observation starting from
`m = 0.7`:

```text
conditional = 0.7 * 0.1 / [ 0.7 * 0.1 + (1 - 0.7) * (1 - 0.2) ]
            = 0.07 / [ 0.07 + 0.24 ]
            ≈ 0.2258

m_new = 0.2258 + (1 - 0.2258) * 0.1
      ≈ 0.3032
```

One incorrect response collapses the estimate from `0.70` to `0.30`.
This is intentional — incorrect responses are strong negative evidence
under the default parameters. Calibrating `P(G)` and `P(S)` per skill is
the standard knob if this proves too aggressive in production.

---

## 4. Determinism and implementation notes

`skillcoco_core::bkt::update_mastery` is a **pure function**: same
`(params, prior, is_correct)` → same output, no I/O, no allocation
beyond the returned `f64`. This matters because:

1. **Unit tests are deterministic** without time/RNG injection.
2. **WASM portability is trivial**: no `std::fs`, no `chrono`, no
   syscalls. The function compiles unchanged on
   `wasm32-unknown-unknown`.
3. **The reference implementation is auditable.** Anyone can re-derive
   the worked examples above by hand and confirm the code produces the
   same numbers.

Persistence is **not** part of the algorithm. Mastery values are
serialized to the host's storage (rusqlite on desktop, IndexedDB on
web) via the [`BktStore`] trait. Mocking `BktStore` lets you unit-test
the higher-level call sites (prerequisite gating, path traversal)
without ever touching a database.

[`BktStore`]: ../src/bkt.rs

---

## 5. Mastery threshold

SkillCoco defines a single, project-wide mastery threshold:

```rust,ignore
pub const MASTERY_THRESHOLD: f64 = 0.7;
```

Modules with `BKT(m) >= 0.7` are considered **mastered**. This value
appears in three places:

1. The prerequisite gate
   ([`skillcoco_core::path::all_prerequisites_mastered`](../src/path.rs)).
2. The track-level skill-tier predicates
   ([`skillcoco_core::threshold`](../src/threshold.rs)).
3. Microlearning candidate selection
   ([`skillcoco_core::microlearning`](../src/microlearning.rs)) treats
   modules with mastery in the band `[BKT_LOWER, BKT_UPPER)` (a strict
   subset of `[0, 0.7)`) as the prime daily-challenge candidates.

### Why `0.7`?

A high enough value to bar the gate against learners who got lucky on
two questions; low enough that learners who genuinely know the material
cross it within five to seven correct observations. The number is
common in the BKT literature (e.g. Corbett & Anderson 1995 used `0.95`,
but later applications calibrated downward for less verbose
instructional contexts).

Changing the threshold requires coordinated migration:

- Every persisted `module_progress` row that was mastered at the old
  threshold but not the new one must be reconsidered.
- Track-level skill-tier predicates depend on per-module mastery via
  this threshold — moving it shifts certification math.
- UI affordances that surface "X modules to go" recompute from this
  constant.

Treat `MASTERY_THRESHOLD` as a project-level invariant. The Phase 7
extract preserves the existing value verbatim; any future calibration
work happens at the project level, not at the algorithm level.

---

## 6. Decay and the microlearning intersection

The base BKT model has **no notion of time** — mastery does not decay
between observations. In real learning, knowledge does fade: a learner
who hasn't seen Kubernetes for six months will not remember service
discovery as well as they did the day they passed the module quiz.

SkillCoco handles decay **outside** the core BKT model, in the
[microlearning selection algorithm](../src/microlearning.rs). The
selector applies a **logarithmic decay penalty** to a module's
last-observed mastery, computed against the time since the last BKT
update:

```text
decay_days = julianday(now) - julianday(last_bkt_update_at)
penalty    = some_function_of(decay_days)
```

Modules that have decayed past a threshold re-enter the daily-challenge
selection pool even if their persisted BKT estimate is still above
mastery. This intersects with [SM-2 spaced repetition](./SM2.md), which
operates on individual cards within a module; BKT operates at the module
level.

This separation of concerns (pure BKT in `bkt.rs`, time/decay in
`microlearning.rs`) keeps each algorithm independently testable and
faithful to its literature.

---

## 7. Limitations

The four-parameter BKT model in SkillCoco has known limitations.
Documenting them here so consumers can decide whether to extend the
model or migrate to a successor.

### 7.1 Single-skill assumption

The model treats each module as a single, independent skill. Real
curricula have skill **dependencies** (Kubernetes services depend on
Kubernetes pods, which depend on container basics). SkillCoco models
the dependency graph at the **path level** (see
[`skillcoco_core::path`](../src/path.rs)) and gates prerequisite
mastery accordingly, but the BKT model itself does not propagate
mastery across the DAG.

### 7.2 No item-difficulty modeling

The model uses uniform `P(G)` and `P(S)` per skill regardless of the
specific question. A multi-step lab task and a quick multiple-choice
question are treated as having identical guess / slip probabilities.
Item-Response-Theory (IRT) extensions to BKT exist (e.g.
Knowledge-Tracing-with-Item-Difficulty); SkillCoco defers these to a
future phase.

### 7.3 No individualization

The parameters are global per skill, not per learner. Empirical research
on individualized BKT (e.g. learner-specific `P(L_0)`) shows
improvements but requires substantially more data per learner than a
typical SkillCoco enrollment provides at Phase 7.

### 7.4 No conjugate prior

Some BKT variants treat the four parameters as random variables with a
Beta-Bernoulli conjugate prior and update them as well as the latent
state. SkillCoco ships a fixed-parameter model for simplicity and
auditability; switching to a Bayesian-parameter variant is a clean
future extension because the trait surface (`BktStore`) need not
change.

### 7.5 Forgetting (formally)

As discussed in §6, the model has no built-in forgetting. The
microlearning decay term is a pragmatic workaround. A formal
forgetting-aware extension (e.g. "Deep Knowledge Tracing" with LSTM
memory cells) is excluded from `skillcoco-core` because it would
introduce a heavy ML dependency that breaks the WASM portability
guarantee.

---

## 8. References

- **Corbett, A. T., & Anderson, J. R. (1995).** Knowledge Tracing:
  Modeling the acquisition of procedural knowledge. *User Modeling and
  User-Adapted Interaction*, 4(4), 253-278. The canonical BKT paper.
- **Yudelson, M. V., Koedinger, K. R., & Gordon, G. J. (2013).**
  Individualized Bayesian Knowledge Tracing Models. *International
  Conference on Artificial Intelligence in Education*. Per-learner BKT.
- **Pardos, Z. A., & Heffernan, N. T. (2010).** Modeling
  individualization in a Bayesian networks implementation of Knowledge
  Tracing. *International Conference on User Modeling, Adaptation, and
  Personalization*.
- **Piech, C., Bassen, J., Huang, J., Ganguli, S., Sahami, M.,
  Guibas, L., & Sohl-Dickstein, J. (2015).** Deep Knowledge Tracing.
  *NeurIPS 2015*. Modern neural-network alternative to BKT.
- **Khajah, M., Lindsey, R. V., & Mozer, M. C. (2016).** How Deep is
  Knowledge Tracing? *EDM 2016*. Comparison of BKT, IRT, and DKT.

---

## 9. Reproducing the worked examples

```rust,ignore
use skillcoco_core::bkt::{update_mastery, BKTParams, MASTERY_THRESHOLD};

let params = BKTParams::default();
assert!((params.p_know - 0.3).abs() < 1e-9);

// Example 3.3 (correct from 0.3)
let m1 = update_mastery(&params, 0.3, true);
assert!((m1 - 0.6927).abs() < 1e-3);

// Three correct in a row
let m2 = update_mastery(&params, m1, true);
let m3 = update_mastery(&params, m2, true);
assert!(m3 > MASTERY_THRESHOLD);

// Example 3.3 (incorrect from 0.7)
let m4 = update_mastery(&params, 0.7, false);
assert!((m4 - 0.3032).abs() < 1e-3);
```

Run `cargo run -p skillcoco-core --example bkt_update` to see the
trajectory printed for a longer observation sequence.

---

*This whitepaper is licensed under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). You may
reuse it with attribution to "SkillCoco OSS contributors, 2026".*
