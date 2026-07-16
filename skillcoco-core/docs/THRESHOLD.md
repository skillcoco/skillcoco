# Achievement Thresholds — Track-Level Skill-Tier Certification

> **Author:** SkillCoco OSS contributors
> **Date:** 2026-06-17
> **License:** [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)
> **Module:** [`skillcoco_core::threshold`](../src/threshold.rs)

This whitepaper documents the track-level achievement-threshold predicates
as implemented in `skillcoco_core::threshold`. It is intended for
engineers, learning scientists, instructional designers, and curious
learners who want to understand how SkillCoco aggregates per-module BKT
mastery into the three track-level skill tiers — Associate, Practitioner,
and Professional — and why the specific thresholds (`25%`, `60%`, `100%
+ 0.85 avg + practical labs`) were chosen.

The threshold layer sits **above** the [Bayesian Knowledge Tracing
model](./BKT.md): BKT estimates per-module mastery as a probability in
`[0, 1]`; threshold predicates aggregate those probabilities across a
track and decide whether the learner has crossed any new skill tier on
the most recent update. The result is the certification ladder that
appears in the SkillCoco UI as Associate → Practitioner → Professional.

---

## 1. Abstract

SkillCoco tracks per-module mastery as a continuous probability via
BKT, but **learners** and **employers** want a discrete, comparable,
verifiable label — "I am a Kubernetes Practitioner" carries different
weight than "I am 67.3% through the Kubernetes track." This whitepaper
specifies the pure-functional predicates that map a per-track aggregate
of BKT mastery scores into one of three named skill tiers, the
calibration rationale for the chosen thresholds, the edge cases the
predicates handle, and the deliberate separation between the
per-observation mastery model (BKT) and the per-track certification model
(thresholds). The implementation is a small set of pure functions over a
`TrackAggregate` struct — no I/O, deterministic, WASM-portable, and
auditable by inspection.

---

## 2. Problem statement — why per-skill mastery is insufficient for
##    track-level certification

The [BKT model](./BKT.md) produces a continuous mastery score in `[0, 1]`
for each module a learner touches. This is the right shape for adaptive
**instruction** — the next-question selector, the prerequisite gate, the
microlearning daily-challenge picker all consume a continuous mastery
estimate and benefit from its precision.

It is the **wrong** shape for three other use cases:

1. **External-facing labels.** A learner who completes 9 of 12 modules
   at average mastery `0.81` does not have an obvious answer to
   "what level am I at?" The continuous mastery vector
   `[0.85, 0.92, 0.30, 0.78, ...]` does not project cleanly onto a
   resume, an LMS badge, or a verifiable certificate. Employers and
   peers expect discrete, comparable tiers, not 12-dimensional vectors.

2. **Track completion is not a single skill.** A "Kubernetes
   Fundamentals" track might contain a dozen modules covering pods,
   services, ingress, RBAC, networking, storage, observability, and
   troubleshooting. BKT estimates each independently. There is no
   single number called "Kubernetes mastery" — there is only an
   aggregate. The aggregate function must be specified, not left
   implicit.

3. **Certification gating is multi-criterion.** A real-world
   "Professional" certification cannot reduce to a single
   above-threshold flag. The learner must have passed **every** module
   at a high standard, must have demonstrated **practical** (hands-on
   lab) competence where required, and must have an average that
   reflects depth rather than just bare-pass-everywhere. A scalar
   mastery vector aggregated by a mean is too lossy.

Threshold predicates solve all three by:

- Mapping aggregate state (`modules_mastered`, `modules_total`,
  `avg_mastery`, lab status) into a small discrete tier label set
  (`Associate` / `Practitioner` / `Professional`).
- Specifying the aggregate function explicitly so the math is
  inspectable and reproducible.
- Combining ratio thresholds with multi-criterion gates so the highest
  tier captures depth, breadth, and practical competence together.

The result is a continuous-to-discrete projection that preserves the
information that matters for certification while discarding the
information that matters only for instruction.

---

## 3. The algorithm

### 3.1 Inputs — `TrackAggregate`

The pure-logic layer consumes a `TrackAggregate` struct that captures
five numbers about the learner's state on a single track at one instant
in time:

```rust,ignore
pub struct TrackAggregate {
    pub modules_total: usize,
    pub modules_mastered: usize,
    pub avg_mastery: f64,
    pub all_practical_labs_passed: bool,
    pub has_practical_required: bool,
}
```

| Field                         | Meaning                                                                                      |
| ----------------------------- | -------------------------------------------------------------------------------------------- |
| `modules_total`               | Number of modules in the latest path version for the track.                                  |
| `modules_mastered`            | Number of modules whose BKT mastery is at or above [`MASTERY_THRESHOLD`](./BKT.md) (`0.7`).  |
| `avg_mastery`                 | Arithmetic mean of BKT mastery across **all** modules (missing rows count as `0.0`).         |
| `all_practical_labs_passed`   | `true` iff every module flagged `practical_required` has `practical_mastery >= 0.7`.         |
| `has_practical_required`      | `true` iff at least one module in the track sets `content_json.practical_required = true`.   |

`TrackAggregate` is computed by the storage adapter — on desktop, a
SQL aggregate function over `module_progress` joined with `modules`,
`learning_paths`, and `learning_tracks`. The aggregate is computed
**once per BKT update**: after a learner finishes a question and
`update_mastery` fires, the storage layer rebuilds the aggregate for
that learner's active track and passes the result through the pure
predicates documented here.

The pure-logic split (algorithm in `skillcoco-core`, SQL aggregate in
the storage crate) lets the algorithm compile to WASM unchanged: web
consumers implement the aggregate function over IndexedDB or any other
backend without touching the threshold predicates.

### 3.2 The aggregation predicates

Three named predicates decide tier membership.

#### Associate — 25% of modules at mastery

```text
ratio(agg) = modules_mastered / modules_total
Associate := modules_total > 0 ∧ ratio(agg) >= 0.25
```

The Associate gate is a single ratio comparison. It is the **entry**
tier — earned when a learner has mastered at least a quarter of the
modules on the track. It has no average-mastery or lab gates; it
captures the milestone of "the learner has demonstrated working
competence in a meaningful slice of the track."

#### Practitioner — 60% of modules at mastery

```text
Practitioner := modules_total > 0 ∧ ratio(agg) >= 0.60
```

Same shape as Associate, just a higher ratio threshold. Practitioner is
the **production-ready** tier — the learner has mastered roughly
two-thirds of the track and can be entrusted with most real-world tasks
in the subject area.

#### Professional — 100% of modules + 0.85 average + practical labs

```text
Professional := modules_total > 0
              ∧ modules_mastered == modules_total
              ∧ avg_mastery >= 0.85
              ∧ (!has_practical_required ∨ all_practical_labs_passed)
```

Professional is the only tier with a multi-criterion gate:

1. **Every module mastered.** Not 99% — every single one.
2. **Average mastery at or above `0.85`.** Discourages a "bare pass on
   every module" trajectory. The learner must show depth, not just
   breadth.
3. **Practical labs.** If the track flags any module with
   `practical_required = true`, the learner must have passed every
   such lab (`practical_mastery >= 0.7`). When the track has zero
   practical-required modules, this gate short-circuits to `true` (no
   lab gate to fail).

The compound predicate captures the intuition that "Professional" is a
hands-on, comprehensive certification — not a soft acclamation, but a
genuine ceiling that requires sustained effort across the full track.

### 3.3 The transition function — `which_level_just_crossed`

The threshold predicates answer "which level(s) is the learner at
**now**." For certificate issuance SkillCoco needs a related answer:
"which level (if any) did the learner just **cross** with this update?"

```rust,ignore
pub fn which_level_just_crossed(
    prev: &TrackAggregate,
    curr: &TrackAggregate,
) -> Option<&'static str>
```

The function checks `is_professional(curr) && !is_professional(prev)`
first (highest tier), then the ratio gates in descending order. The
**highest** newly-crossed tier wins — a learner who jumps from 0% to
60% in a single batch update gets a single "Practitioner" return value,
not a list. (The caller separately walks `levels_met` to issue any
back-fill certs for tiers crossed previously, so the single-tier return
is a UX convenience, not a correctness compromise.)

The ordering matters. Consider a learner at `(modules_mastered = 3,
modules_total = 4, avg_mastery = 0.84)`. They have already crossed
Associate (`3/4 = 75% >= 25%`) and Practitioner (`75% >= 60%`). When
the BKT update completes their fourth module and bumps avg mastery to
`0.86`, the predicate must report **Professional** — not Practitioner
again (already crossed) and not Associate (already crossed). The order
of checks in the source code is `Professional → Practitioner →
Associate` to make this happen automatically.

### 3.4 Worked example — 5-module Docker track

Suppose a learner is working through a 5-module "Docker Foundations"
track with two practical-required modules (`docker-run`, `dockerfile`).
Their progress over five sessions:

| Session | mastered | total | avg   | labs OK | predicate `levels_met`                             | `just_crossed` |
| ------- | -------- | ----- | ----- | ------- | -------------------------------------------------- | -------------- |
| 0       | 0        | 5     | 0.00  | false   | `[]`                                                | none           |
| 1       | 1        | 5     | 0.45  | false   | `[Associate]` (20% < 25% — actually 1/5 = 20%)     | none           |
| 2       | 2        | 5     | 0.59  | false   | `[Associate]` (2/5 = 40% >= 25%)                    | `Associate`    |
| 3       | 3        | 5     | 0.74  | true    | `[Associate, Practitioner]` (3/5 = 60%)             | `Practitioner` |
| 4       | 4        | 5     | 0.82  | true    | `[Associate, Practitioner]` (4/5 = 80% < 100%)      | none           |
| 5       | 5        | 5     | 0.87  | true    | `[Associate, Practitioner, Professional]`           | `Professional` |

Three certificates are issued across the five sessions — one per
crossing. Note that session 4 does NOT issue a Professional cert even
though `avg_mastery = 0.82` is close: `modules_mastered (4) <
modules_total (5)` fails the all-modules gate. Session 5 finishes the
last module, pushes the average to `0.87 >= 0.85`, and `labs OK = true`
holds, so Professional crosses.

The crossing detector compares the previous aggregate to the current
one. The same `levels_met` call against session 5 returns all three
levels, but `which_level_just_crossed(prev=session-4, curr=session-5)`
returns only `Professional` because the lower tiers were already met
in prior sessions.

### 3.5 The track-level mastery threshold

Every per-module mastery comparison uses the project-wide
[`MASTERY_THRESHOLD`](./BKT.md) (`0.7`). This is intentional: a single
constant means the threshold layer never disagrees with the BKT layer
about what "mastered" means. If a future SkillCoco release calibrates
the project-wide mastery threshold downward (`0.65`, say), the
threshold predicates automatically pick up the new value and all
certificate ladders adjust consistently.

The lab gate uses the same `>= 0.7` cutoff for the practical-mastery
field: the SQL aggregate checks `practical_mastery >= 0.7` and feeds
the boolean result into `all_practical_labs_passed`. This keeps the
mastery-vs-practical comparison symmetric (one continuous estimate
per kind, one threshold).

---

## 4. Calibration in SkillCoco

### 4.1 Why `25%`, `60%`, `100%`?

The three ratio cut-points (`0.25`, `0.60`, `1.0`) were chosen for
three reasons:

1. **Spacing.** A learner who mastered half the modules should fall
   clearly between Associate and Practitioner. A halfway point
   (`50%`) below Practitioner gives visible progress without
   over-promoting. The `60%` cutoff lets the learner aim for the next
   tier with roughly five more modules of work on a 12-module track
   (`50% → 60%` ≈ `1.2 modules`).
2. **Cohort match.** Several adjacent industry certifications use
   roughly the same shape (AWS Cloud Practitioner / Associate /
   Professional ladders; CKA / CKAD / CKS for Kubernetes). SkillCoco
   is not trying to replicate any one of them, but the tier-count
   choice (three named tiers, not five or seven) and the
   spacing-of-the-tiers feel matches industry expectations.
3. **Empirical learning data.** Phase 6 design notes record that
   `25%` was the smallest tier-1 cut-point that consistently
   produced positive reinforcement without diluting the meaning of
   the tier. (Earning Associate at `10%` felt like a participation
   trophy in early user testing; `25%` felt earned. `60%` for
   Practitioner felt like "production-ready" without requiring the
   exhaustive grind to `100%`.)

### 4.2 Why `avg_mastery >= 0.85` for Professional?

The all-modules-mastered gate alone is satisfied by any learner who
crossed `>= 0.7` on every module. Without an average-mastery floor a
learner could earn Professional with twelve modules at exactly `0.70`
each — barely above the per-module cutoff. The average-mastery floor
prevents this and pushes the bar to "the learner consistently performs
above the mastery threshold, not just at it."

The specific value `0.85` was chosen by:

1. Computing the avg-mastery distribution across a synthetic
   trajectory of `n` correct + `1` incorrect responses per module
   under the default BKT parameters (`P(L_0) = 0.3`, `P(T) = 0.1`,
   `P(G) = 0.2`, `P(S) = 0.1`). A learner who passes every module
   cleanly stabilizes around avg `0.92`–`0.95`. A learner who barely
   passes (one wrong answer per module after first crossing
   mastery) stabilizes around `0.78`–`0.83`. `0.85` is the breakpoint
   between the two distributions.
2. Cross-checking against the [BKT mastery threshold](./BKT.md) — the
   per-module `0.7` cutoff and the track-level `0.85` average leave
   meaningful headroom (`0.15`) so the gate is not just a re-statement
   of the per-module rule.

### 4.3 Why a separate practical-lab gate for Professional?

SkillCoco has a long-standing assumption that **practical** competence
(running real commands, debugging real failures, building real
artifacts) is different from **declarative** knowledge (knowing what
a Pod is, knowing the syntax of a Dockerfile). The per-module BKT
score conflates the two — a learner can have BKT mastery `0.95` from
multiple-choice answers and still fail the first practical lab.

The Professional tier is the only tier that distinguishes between
these. Adding the lab gate to Associate or Practitioner would
prevent learners from earning recognition for the declarative work
they have done; restricting it to Professional makes the highest
tier a genuine "ready for the real world" signal.

The gate short-circuits when no module on the track is
`practical_required = true`. This handles theory tracks (e.g.,
"Distributed-Systems Concepts") that have no hands-on lab component
without forcing them to either invent labs or sacrifice the
Professional tier.

### 4.4 Edge cases handled

The implementation handles a small set of edge cases explicitly:

1. **Empty tracks.** `modules_total == 0` early-returns `[]` from
   `levels_met` and `None` from `which_level_just_crossed`. A path
   with no modules can never confer a level.
2. **First crossing after mastery decay.** If a learner crossed
   Associate at session N (mastery `0.72` on one module) and the
   module's BKT mastery later decayed back below `0.7`, the
   `levels_met` predicate at session N+1 will report `[]` because the
   `modules_mastered` count dropped. **However**, the issued Associate
   certificate is **not revoked** — Phase 6 design decision A9
   states that the achievements row preserves the historical proof.
   The certificate is a snapshot of past competence, not a current
   status indicator. (See [BKT decay handling](./BKT.md) for the
   model's relationship to time.)
3. **Multi-tier jumps in one update.** A batch BKT update (rare —
   typically a migration or an import) can push a learner from 0% to
   80% mastery in a single update. `which_level_just_crossed` returns
   the highest newly-crossed tier (`Practitioner` in this example);
   the storage layer separately walks `levels_met` and back-fills any
   missed certificates. This separation keeps the algorithm simple
   while preserving the "every crossed tier gets a cert" guarantee.
4. **`practical_required` toggled mid-track.** If a track's content
   is edited to add a practical-required module after a learner has
   already crossed Professional, the implementation leaves the
   issued cert alone (Phase 6 design — historical record) but the
   `levels_met` predicate at the next aggregate update will no longer
   report Professional unless the new lab is passed. This is a soft
   inconsistency that the UI surfaces as "you have a Professional
   cert for an older version of this track."

---

## 5. Implementation notes

### 5.1 Purity, determinism, WASM portability

`skillcoco_core::threshold::{which_level_just_crossed, levels_met,
is_professional, ratio}` are **pure functions** in the strictest sense:
same `TrackAggregate` input → same output, no I/O, no allocation
beyond the small `Vec<&'static str>` returned by `levels_met`.

This matters because:

1. **Tests are deterministic** without time injection. The unit
   tests in `skillcoco-core/src/threshold.rs` (`associate_at_25_percent`,
   `practitioner_at_60_percent`, `professional_requires_avg_and_labs`,
   etc.) construct `TrackAggregate` values directly and assert
   specific tier returns.
2. **WASM portability is free.** No `std::fs`, no `chrono`, no
   syscalls. The module compiles unchanged on
   `wasm32-unknown-unknown`. The SQL aggregate that produces
   `TrackAggregate` lives in the storage adapter (`src-tauri` on
   desktop), so the algorithm core has no SQL dependency.
3. **The math is auditable.** Anyone can re-derive the worked
   example in §3.4 by hand using the predicate definitions in §3.2.

### 5.2 The split between pure logic and the storage adapter

The threshold module deliberately omits the SQL aggregate that
computes `TrackAggregate` from `module_progress`. That aggregate lives
in `src-tauri/src/storage_impl/threshold.rs` (desktop, rusqlite). The
seam matches the BKT / SR pattern: pure algorithm in `skillcoco-core`,
storage details in the adapter crate.

A future Phase 8 step promotes the aggregate to an
`AchievementStore` trait method (matching `BktStore`, `SrStore`,
`MicrolearningStore`); until then it stays as a free function and the
algorithm code never imports it.

### 5.3 Issuance is separate from prediction

`which_level_just_crossed` and `levels_met` answer **what** tier the
learner is at. They do **not** decide whether to issue a certificate.
Issuance lives in `src-tauri/src/achievements/mod.rs` and is
gated on (a) the predicate returning a tier, (b) the achievements
table not already having a row for `(learner_id, track_id, level)`,
and (c) the issuance being explicitly enabled for the track.

This separation keeps the pure-logic predicates free of side effects.
Tests can call them with synthetic aggregates without provisioning a
database or stubbing the signing key.

---

## 6. Limitations

### 6.1 Sensitivity to BKT decay

The per-module mastery values fed into `TrackAggregate` are the
**live** BKT estimates from `module_progress.mastery_level`. The
[BKT model](./BKT.md) has no built-in decay, but the [microlearning
selector](./MICROLEARNING.md) maintains a `last_bkt_update_at`
timestamp and the storage layer can compute a decay-adjusted mastery
on aggregation.

If decay-adjusted mastery is used for the aggregate, a learner who
has not practiced in months may drop below a previously-crossed tier
in `levels_met` — but the issued cert (in the achievements table) is
not revoked. The UI surfaces this gracefully: the cert exists and
verifies, but the live tier display may show a lower level. This is
the intended behavior for a "historical proof" cert; see §4.4.

### 6.2 Threshold-vs-mastery distinction

A learner can have an `avg_mastery` strictly less than `1.0` and still
pass Professional — the gate requires `>= 0.85`, not `== 1.0`. This
is by design (see §4.2) but is worth noting because a naive reading
of "100% of modules mastered" might suggest perfect mastery. The
correct reading is "100% of modules at-or-above the per-module
mastery threshold" + "average across all modules at or above `0.85`."

The two predicates can disagree at the boundary. A learner with
mastery `[0.70, 0.70, 0.70, 0.70]` is `mastered everywhere` (each
module at threshold) but has `avg_mastery = 0.70 < 0.85`, so they
do not qualify for Professional. The threshold layer treats this as
the correct outcome — bare-pass-everywhere is not the same as
professional-level depth.

### 6.3 No per-track tier override

All tracks share the same three-tier ladder. A short "Intro to Git"
track and a sprawling "Distributed Systems" track both use the same
`(25%, 60%, 100%+)` cut-points. There is no per-track override or
per-pack alternative ladder.

This was intentional in Phase 6 (uniform UI, comparable certs across
tracks) but is a future-extension point: per-track or per-pack
ladders would let a pack author specify a different cut-point set
(`33% / 67% / 100%+`, say) without changing the algorithm.

### 6.4 Two-dimensional skill is flattened

A real "Kubernetes" track has both breadth (modules touched) and
depth (mastery on each touched module). The ratio gate measures
breadth (`modules_mastered / modules_total`); the average-mastery
gate measures depth, but only at the Professional tier. Associate
and Practitioner are pure breadth gates. A learner with very high
mastery on two of twelve modules and zero on the rest qualifies for
neither tier; a learner with bare-pass mastery on three modules
qualifies for Associate.

This is the same flattening trade-off described in §4.2. A future
extension could add a depth criterion to Associate and Practitioner
(e.g., `avg_mastery_on_mastered_modules >= 0.75`); the algorithm
core would change but the data shape (`TrackAggregate`) would not.

### 6.5 No partial-credit modules

The mastered/not-mastered distinction is binary at the module level.
A module with `mastery = 0.69` counts as `0` in the
`modules_mastered` numerator regardless of how many other modules
the learner has nearly-passed. A weighted-by-mastery aggregate would
smooth this out (a learner near the cutoff on every module would
count for slightly more) but at the cost of harder-to-explain math
and a less crisp "you crossed Practitioner" moment.

### 6.6 No multi-track aggregation

Each track is scored independently. A learner who has Associate on
five adjacent tracks does not automatically earn a higher-tier
"Cloud Native Generalist" cert. Multi-track aggregation (and the
question of whether to do it at all) is a Phase 11+ topic for the
cohort/corporate use case, where employers may want a
"polyglot Associate" or "tech-stack Practitioner" label.

---

## 7. References

- **Phase 6 D-02 (SkillCoco):** Threshold-formula derivation and
  calibration rationale; original three-tier design notes at
  `.planning/ROADMAP.md` §Phase 6: Certification.
- **Phase 6 R4 (SkillCoco):** Certificates as historical record;
  mastery decay does not revoke an issued cert.
- **Phase 6 A9 (SkillCoco):** `mastery_level` is the live high-water
  mark; achievements row preserves the snapshot.
- **Bloom, B. S. (1968).** *Learning for Mastery.* Evaluation Comment,
  1(2), 1-12. The original mastery-learning theory — `> 90%` of
  students can master `> 90%` of the material given enough time.
- **Anderson, J. R., Corbett, A. T., Koedinger, K. R., & Pelletier,
  R. (1995).** Cognitive tutors: Lessons learned. *Journal of the
  Learning Sciences*, 4(2), 167-207. The successor work to the
  Corbett & Anderson 1995 BKT paper, including discussion of mastery
  cutoffs for tutoring systems.
- **AWS Certification Program.** Three-tier Cloud Practitioner /
  Associate / Professional ladder is one influence on the cut-point
  choice. https://aws.amazon.com/certification/
- **CNCF Kubernetes Certifications.** CKA / CKAD / CKS three-tier
  practitioner ladder. https://www.cncf.io/training/certification/
- **Black, P., & Wiliam, D. (1998).** Assessment and classroom
  learning. *Assessment in Education*, 5(1), 7-74. Foundational
  work on formative-vs-summative assessment; informs the
  certificate-as-snapshot view in §4.4 / §6.1.
- **SkillCoco whitepaper:** [BKT](./BKT.md) — per-module mastery
  estimation model the threshold layer consumes.
- **SkillCoco whitepaper:** [MICROLEARNING](./MICROLEARNING.md) —
  daily-challenge selection algorithm that depends on the same
  per-module mastery values; relevant for understanding decay
  dynamics (§6.1).
- **SkillCoco whitepaper:** [SIGNING](./SIGNING.md) — Ed25519 +
  canonical JSON pipeline that signs the resulting certificates so
  they remain verifiable across SkillCoco versions.

---

## 8. Reproducing the worked examples

```rust,ignore
use skillcoco_core::threshold::{
    TrackAggregate, levels_met, which_level_just_crossed,
};

// Session 0 → Session 2 (Associate just crossed)
let s0 = TrackAggregate {
    modules_total: 5,
    modules_mastered: 0,
    avg_mastery: 0.00,
    all_practical_labs_passed: false,
    has_practical_required: true,
};
let s2 = TrackAggregate {
    modules_total: 5,
    modules_mastered: 2,    // 40% — crosses 25%
    avg_mastery: 0.59,
    all_practical_labs_passed: false,
    has_practical_required: true,
};
assert_eq!(which_level_just_crossed(&s0, &s2), Some("Associate"));

// Session 4 → Session 5 (Professional just crossed)
let s4 = TrackAggregate {
    modules_total: 5,
    modules_mastered: 4,    // not yet 100%
    avg_mastery: 0.82,
    all_practical_labs_passed: true,
    has_practical_required: true,
};
let s5 = TrackAggregate {
    modules_total: 5,
    modules_mastered: 5,    // 100%
    avg_mastery: 0.87,      // >= 0.85
    all_practical_labs_passed: true,
    has_practical_required: true,
};
assert_eq!(which_level_just_crossed(&s4, &s5), Some("Professional"));

// `levels_met` returns the complete set
assert_eq!(
    levels_met(&s5),
    vec!["Associate", "Practitioner", "Professional"]
);
```

The unit tests in `skillcoco-core/src/threshold.rs` exercise each
edge case from §4.4 and serve as the executable specification for
the predicates.

---

*This whitepaper is licensed under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). You may
reuse it with attribution to "SkillCoco OSS contributors, 2026".*
