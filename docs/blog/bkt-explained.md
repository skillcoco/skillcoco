---
title: "What is Bayesian Knowledge Tracing? An accessible explainer."
slug: bkt-explained
date: 2026-06-17
tags: [adaptive-learning, bkt, education, beginner-friendly]
canonical_url: https://learnforge.dev/blog/bkt-explained
author: LearnForge OSS contributors
license: CC BY 4.0
---

# What is Bayesian Knowledge Tracing? An accessible explainer.

Someone you know just finished a six-hour Kubernetes video course on a popular
learning platform. The completion certificate is sitting in their inbox. Ask
them what a pod is and they will probably get it right. Ask them what
`terminationGracePeriodSeconds` controls, or when a `PreStop` hook fires
relative to `SIGTERM`, and you will get a long pause.

Did they learn Kubernetes? They watched all the videos. The progress bar said
100%. The platform thinks they did.

Bayesian Knowledge Tracing — BKT for short — is a thirty-year-old educational
model that argues completion is the wrong question. The right question is
**mastery**, and mastery is something you have to estimate, not record. This
post explains how BKT does that estimation, why it makes adaptive learning
actually adaptive, and what goes wrong when you build a learning platform
without it.

No equations in the body. The math is real and worth understanding, but it is
not the point of this post. The point is the shift in thinking.

## The "completion vs mastery" gap

Most learning platforms are completion engines. They track what you have
opened, what you have watched, what you have submitted. They report
percentages. They award certificates when the percentage hits 100. This is
easy to count, easy to display in a dashboard, easy to sell to an employer who
wants to know whether their team has finished the compliance training.

The problem is that opening, watching, and submitting are not the same as
knowing. Two people can both have a 100% completion score on the same
Kubernetes course and have wildly different mental models of how Kubernetes
works. One of them grinds through the videos at 2x speed with the browser
muted. The other pauses, looks up the docs, tries the examples, runs into a
problem, fixes it, and moves on. Both finish. Only one of them learned.

BKT was designed to make this distinction quantifiable. Instead of asking "did
this person finish the material?", it asks "given everything I have observed
about this person — every question they answered correctly, every question
they got wrong — how likely is it that they actually know this skill?"

The answer is a number between zero and one. We call it the **mastery
probability**, and unlike completion percentage, it can go up *and* it can go
down. It moves in response to evidence. It refuses to settle on certainty
until the evidence is strong enough to justify it.

## The four parameters, in plain English

BKT models learning with four numbers per skill. Three of them are properties
of the world; one is a property of the learner. Here is what each one means in
ordinary language.

**P(L0) — starting knowledge.** Before you have answered a single question
about a topic, what is the chance you already know it? In LearnForge we
default this to 0.3, which roughly says "most adult learners arrive with some
vague prior exposure but cannot reliably answer questions yet." It is the
mental model of someone who has heard the word *Kubernetes* in standup but
could not draw the pod-to-service-to-deployment hierarchy if you asked them
to.

**P(T) — learning rate.** Each time you attempt a problem on this skill,
what is the chance you transition from "not knowing" to "knowing"? Think of
it as the conversion rate of practice into mastery. We default to 0.15 — a
solid attempt at a well-designed problem moves the needle, but it does not
guarantee enlightenment. Real learning happens over many small steps, not one
big jump.

**P(G) — guess rate.** This is the chance you answer correctly *without
actually knowing* the material. On a multiple-choice question with four
options, the floor for P(G) is 0.25 — a coin flip with three losing sides.
For an open-ended exercise where the learner has to write code, P(G) is much
lower because there are no buttons to press at random. The lower the guess
rate, the more informative each correct answer is.

**P(S) — slip rate.** Even when you know the material, sometimes you make a
mistake. A typo. A misread. A bad day. The slip rate is the chance you answer
incorrectly *despite* knowing. It is usually small — single digits — but it is
not zero, and ignoring it is how naive systems decide one wrong answer means
the learner has forgotten everything.

These four numbers — starting knowledge, learning rate, guess rate, slip rate
— let BKT do the trick at the heart of the model: update a continuous
estimate of what the learner knows, given a stream of binary observations
about whether they answered each problem correctly.

## How the math actually moves

Imagine your starting mastery for "Kubernetes pods" is 0.3. You attempt your
first exercise.

You get it right. BKT does not immediately conclude that you have mastered
pods, because P(G) — the guess rate — says you might have lucked out. But a
correct answer is still evidence, and Bayes' rule turns the prior into a
posterior. After one correct answer, your mastery might rise to roughly 0.55.
The model has updated.

You attempt a second problem. Right again. Now mastery climbs to about 0.78.
Two consistent correct answers are harder to explain by guessing alone.

You attempt a third problem. This time you get it wrong. A naive completion
system would treat this as zero progress; a binary
got-it-right-or-didn't-get-it-right system would erase everything. BKT does
something more honest: it asks "given my current estimate of your mastery and
my belief about the slip rate, how surprised should I be by this answer?" The
answer is *moderately surprised but not shocked*. Mastery drops, but not all
the way back to where you started. Maybe it lands around 0.45.

This is the headline behaviour: **mastery moves with evidence, in both
directions, and the rate of movement reflects the strength of the evidence
relative to the noise.** A long run of correct answers slowly pushes mastery
toward 1. A wrong answer in the middle of that run pulls it back, but not
catastrophically. A wrong answer when mastery is already high is treated as
likely a slip; a wrong answer when mastery is low is treated as more
informative.

The formal update equation is small — a few lines of arithmetic — but the
*intuition* is the part that matters. If you want the math itself, with all
the conditional probabilities laid out and the recurrence relation derived
from first principles, see the [BKT whitepaper](../../learnforge-core/docs/BKT.md)
in this repository. It is written for the engineer who wants to verify the
implementation matches the literature.

## Why this beats "completed: 60%"

Consider two learners who have both worked through 60% of a course.

Learner A is the 2x-speed mute-the-audio video grinder. They opened every
module, they marked every page as read, but when they hit the exercises they
got most of them wrong. Their completion score says 60%. Their BKT mastery
across the modules they have touched is around 0.2 — closer to "barely
started" than "well underway."

Learner B has only worked through 60% of the same course, but they have
nailed every exercise they attempted, often on the first try. Their
completion score is also 60%. Their BKT mastery is around 0.85 across the
covered material — closer to "solidly competent" than "halfway through."

These two learners are not at the same point in their learning journey, even
though the completion bar says they are. A system that gates further content
on completion will treat them identically. A system that gates on mastery
will recognize that Learner A needs more practice on what they have already
"covered" before unlocking new material, while Learner B is ready to move
forward — and might actually be ready to skip ahead.

This is what we mean by an *adaptive* learning system. The system adapts not
to what the learner has seen, but to what the learner has demonstrated. BKT
is the mechanism that lets that distinction exist.

## Why this matters for the platform

LearnForge uses BKT for per-module mastery estimation. Every exercise outcome
— every correct or incorrect submission — feeds the BKT update for the
relevant skill. The mastery estimate is then used in three places:

1. **Module unlocking.** A downstream module unlocks when the prerequisite
   modules have mastery scores above a calibrated threshold. The threshold
   itself is a separate piece of the puzzle — see
   [THRESHOLD.md](../../learnforge-core/docs/THRESHOLD.md) for the
   calibration logic — but the *input* to that threshold is BKT mastery, not
   completion.

2. **Spaced-repetition scheduling.** Once a module reaches mastery, its
   review cards enter the SM-2 spaced-repetition pipeline. The interval
   between reviews is computed from the SM-2 algorithm; the *trigger* that
   says "this module is ready for SR scheduling" comes from BKT crossing a
   threshold.

3. **Certification.** Track-level certifications (the signed Ed25519
   certificates LearnForge issues for completed tracks) are awarded based on
   aggregated BKT mastery across the modules in the track, not on the
   completion percentage. The certificate says something the recipient can
   actually defend.

Tying mastery to platform decisions — rather than tying completion to
decisions — is the difference between a learning platform that respects what
you know and a learning platform that respects what you have clicked.

## Where this lives in the code

BKT is implemented in [`learnforge_core::bkt`](../../learnforge-core/src/bkt.rs)
as part of the `learnforge-core` Rust crate, which is published on crates.io
under the MIT license (algorithm docs CC BY 4.0). The crate compiles to both
native Rust and WebAssembly, so any future LearnForge consumer — desktop
app, web platform, embedded scoring engine — uses the same canonical
implementation.

The algorithm itself is twenty-eight lines of code. The whitepaper is four
hundred lines. That ratio is not a mistake: the value is in the calibration,
the edge cases, and the precise reasoning about *why each parameter is what
it is*, not in the arithmetic.

## Further reading

- **The formal treatment** — [the BKT whitepaper in
  learnforge-core](../../learnforge-core/docs/BKT.md). All the math, all the
  references, all the calibration notes.
- **The companion algorithm** — [the SM-2 whitepaper](../../learnforge-core/docs/SM2.md)
  on spaced repetition, which is what catches mastery decay over time.
- **The original Corbett & Anderson 1995 paper** — the BKT whitepaper cites
  it in full. Thirty years old and still load-bearing.

If you build educational software, BKT is the single most consequential idea
you can put into your platform. The shift from "what have you watched?" to
"what do you actually know?" changes everything downstream — content
ordering, exercise selection, certificate semantics, even the way you talk
about a learner's progress to themselves. Worth the investment.

---

*This article is licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
Reuse with attribution to LearnForge OSS contributors.*
