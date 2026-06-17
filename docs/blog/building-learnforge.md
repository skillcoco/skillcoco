---
title: "Building LearnForge: a Tauri + Rust + open-core retrospective"
slug: building-learnforge
date: 2026-06-17
tags: [tauri, rust, open-core, engineering, retrospective]
canonical_url: https://learnforge.dev/blog/building-learnforge
author: LearnForge OSS contributors
license: CC BY 4.0
---

# Building LearnForge: a Tauri + Rust + open-core retrospective

This is the retrospective. We have shipped enough of LearnForge — through
seven phases of planned work, a strategic mid-flight reorder, an open-core
history carve-out, and a publishable Rust crate — that it is worth writing
down what we have learned. Some of it generalizes. Some of it is specific
to the corner of the world that runs adaptive learning algorithms inside a
desktop application written in Rust and React on top of Tauri 2. We tried
to be honest about both.

The piece is organized chronologically by the architectural decisions in the
order we made them, because the order ended up mattering more than any
individual choice. Many of the things that worked worked *because they were
not first*; many of the things we would change in hindsight were premature.

## Why Tauri 2 — not Electron

We started with a constraint: the desktop app had to ship locally, run
offline, persist user data on the user's machine, and be small enough that
the install would not be embarrassing on a metered connection. The latter
part eliminated Electron on its own. A modern Electron app, even one that
has been beaten with the optimisation stick, lands somewhere in the
150-200 MB range. Tauri 2 ships an installer that is two orders of magnitude
smaller because the front-end is rendered by the operating system's native
WebView (WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux). We
ship the application binary, the assets, and the JavaScript bundle. We do
not ship Chromium.

The second reason was that we wanted Rust at the core anyway. Tauri's
backend is Rust by construction. The IPC surface between the JavaScript
front-end and the Rust back-end is a strongly typed `#[tauri::command]`
boundary; once you have committed to it, you get end-to-end type safety from
React state down to SQLite queries, which is the kind of property that makes
refactors safe and bugs rare. Electron would have meant either dual-language
plumbing or Node.js for the back-end, and we did not want either.

The third reason was less rational: we *like* writing Rust. The compiler is
a friend. Cargo is a friend. The ecosystem for the algorithm-heavy stuff we
were going to do — Bayesian Knowledge Tracing, SM-2, Ed25519 — is excellent.
Rejecting Tauri would have meant choosing a worse language for emotional
reasons. We chose the better language.

The trade-offs are real. Tauri 2 is younger than Electron and the
ecosystem of pre-built plugins is smaller. The native WebView story means
that platform-specific WebView bugs show up in your application; we have
hit a few. The Rust-IPC boundary requires discipline — `camelCase` serde
across every struct, no exceptions, or you will spend an afternoon debugging
a missing field name. But the bundle size, the type safety, and the
language ergonomics dominated the calculus for us, and we have not regretted
the choice.

## The open-core split (Phase 03.2)

LearnForge has two products. The free desktop app — MIT licensed, GitHub
public, the thing this blog post is about — is the one most people will
ever see. The commercial Studio offering — proprietary, intended for
corporate teams, cohort management, multi-modal content, hosted licensing —
is the revenue engine. Sharing code between them was always part of the
plan; sharing the *history* was the problem.

Phase 03.2 — the open-core split — was where we made the cut. The OSS
repository's history was carved at commit `bf84ac9` to remove every line of
code that had ever been internal-only. The carved-out commits live in a
separate private repository as a single squashed seed; from `bf84ac9`
onward, the public history is the public history, and the cryptographic
hash chain back to that root commit is intact and reviewable. We do not
ship Pro code from the public tree, and we never have.

The technical mechanism that lets the two products share a codebase without
leaking Pro code is the **plugin overlay**. The public repo defines a
`LearnForgePlugin` trait in Rust and a `PluginSlot` mount point in React.
The OSS build wires both to no-op identity implementations and ships
exactly what you see on GitHub. The Studio build wires both to the
proprietary `StudioPlugin` and `StudioPluginSlot` implementations, which
live in a separate `pro/` directory tree in a private repository, and
which the public CI checks (via the `check-pro-leak.yml` workflow) never
appear in the OSS tree. The seam is exercised in every OSS build because
the no-op plugin gets called the same way the real plugin would — which
means we cannot accidentally break the seam between Studio releases.

The lesson here was about *ordering*. We did not start with the open-core
split. We started with a single-product codebase and added the split when
we had enough surface area to know where the actual seams lived. If we had
tried to design the plugin architecture up front, we would have gotten the
shape of the trait wrong — the right number of methods on `LearnForgePlugin`,
the right granularity for the slot system, the right boundary between
"licensing logic" and "feature gating" — because the surface itself was
not yet stable. Letting the codebase evolve until the seams were obvious,
*then* carving them out, was the right call.

The unsurprising surprise was how *little* code needed to move. The actual
open-core extraction was a handful of files: a trait declaration, a stub
implementation, a `LicenseValidator` interface, a few build-config
adjustments. The carving of history was the larger operation. Most of
LearnForge is OSS, has always been OSS, and will always be OSS. The Pro
overlay is small by design and stays that way by discipline.

## The learnforge-core extraction (Phase 7)

The next architectural surgery was Phase 7: extract the algorithm layer
into a standalone Rust crate, publish it on crates.io, make it portable to
WebAssembly so the future web platform can consume it without a separate
implementation.

This is the kind of work that the conventional wisdom says you should do
*early*. "Extract the library before you write the application around it,"
the conventional wisdom says. "It is harder to extract later."

The conventional wisdom is wrong, or at least it was wrong for us. We
waited until Phase 7 — eighteen months into the project — to do the
extraction. The reason was the same as the open-core split: until the
algorithm surface had been exercised by every consumer in the application,
we did not know what the *right* API surface looked like. Extracting in
Phase 1 would have given us a crate with the wrong public types, the wrong
trait shapes, the wrong error envelopes. We would have spent the rest of
the project paying for that early commitment.

By Phase 7 we knew exactly which pieces were stable enough to belong in a
crates.io-published crate:

- BKT — the mastery model. Pure function, no I/O.
- SM-2 — the spaced-repetition scheduler. Pure function, no I/O.
- Threshold evaluation — the certification-readiness predicate. Pure
  function, no I/O.
- Canonical JSON + Ed25519 signing — the certificate issuance and
  verification pipeline. Pure functions plus a thin trait abstraction
  over the key store.
- Microlearning selection — the next-item-to-surface scoring formula.
  Pure function, no I/O.
- Achievement issuance — the orchestrator that ties mastery aggregation
  to certificate signing. Pure-ish; uses a trait abstraction over the
  achievement store.

What we deliberately *did not* extract: the database layer (rusqlite, with
all its SQL), the PDF generator (printpdf, which does not compile to
WASM), the image renderer (image and qrcode crates, ditto), the Tauri
command surface, the OAuth client. These all stay in the desktop crate.
The split was clean because we let it stabilize first.

The pattern that made the extraction tractable was what we ended up
calling the **per-module storage trait recipe**. Every algorithm module
that needed persistence — mastery records, SR cards, achievement events —
declared its own trait abstraction over the store. The desktop crate
implemented those traits using rusqlite; the WASM target either omitted the
implementation entirely or implemented it against an in-memory store. The
algorithm code never knew which backend it was talking to. Rust's
orphan rules made this slightly awkward — you cannot implement a foreign
trait for a foreign type without a newtype wrapper — but the workaround
was a one-line `pub struct SqliteFooStore<'a>(&'a Connection);` and it
generalized across all eight algorithm modules. We applied the same
recipe eight times in a row, in eight consecutive waves of Phase 7. The
discipline of "do not invent a new pattern when you have a working one"
saved us months.

## The strategic pivot (2026-05-03)

Halfway through the project we threw out the v1.0 plan and re-prioritized.
This is worth writing about because it was painful and because it was
correct.

The original plan went something like: build the adaptive loop, then build
the algorithm extraction, then build the web platform, then build the
content-richness layer, then build the certification surface. Architecture
first. Bells and whistles last. Very normal, very enterprise-software, very
wrong.

The trigger was a moment of looking at the actual user experience and
realizing the adaptive loop was broken. Mastery updates were not firing
when exercises completed. Module unlocks were not happening when mastery
crossed the threshold. The dashboard counts were placeholder. The review
queue was a stub. We were about to put a Rust-crate-extraction phase on
top of a foundation that did not actually work.

So we stopped, wrote down a phrase — *the Definition of Usable* — that
said: "a new user installs LearnForge, picks a topic, learns something
real, and feels mastery move, within ten minutes, every time, without
bugs," and we rebuilt the phase ordering around it. The architectural
phases — Core Extraction, Web Platform — got pushed back. Path Quality and
Content Richness got pulled forward. The first thing we shipped after the
pivot was a working adaptive loop. The architectural work came after.

The lesson was about *resisting the urge to build scaffolding on a broken
foundation*. Architectural cleanups are seductive; they feel like progress,
they leave the codebase in a measurably better state, they generate the
kind of artifacts you can point at in a status update. But if the
fundamental user experience is not working, every architectural improvement
you make is building taller scaffolding on a foundation that you will
eventually have to rebuild anyway. We had been about to spend three months
on a Phase 7 extraction that would have wrapped broken algorithms in a
nicer interface. Catching that before it happened was the highest-leverage
single decision in the project.

The corollary, which we discovered later: the post-pivot work was easier.
With the loop working, the eventual core extraction had a stable surface
to extract from. The discipline forced on us by the pivot — *do not
build architecture on a broken loop* — paid off as architecture-work-when-
the-loop-finally-worked. It was not just the right call ethically; it was
the right call tactically.

## What we would do differently

A retrospective without dissent is propaganda. Here are the things we
would change with the benefit of hindsight.

**We would set up the open-core CI guardrail earlier.** The
`check-pro-leak.yml` workflow that prevents Studio code from ever
appearing in the OSS tree was a Phase 03.2 artifact. It should have been a
Phase 1 artifact, written before there was any Studio code to leak. The
cost of writing it after the fact was small but the *psychic* cost — the
anxiety of "did we accidentally commit something proprietary?" — was real
and lasted months.

**We would have committed to the per-module storage trait recipe sooner.**
The pattern we applied eight times in Phase 7 was discoverable in Phase 1.
We did not see it because we had not yet hit the cases that motivated it.
But we *could* have set up the first algorithm module with the trait
abstraction in place and saved ourselves the eventual refactor. The cost
of the abstraction is small; the cost of the refactor was substantial.

**We would have written the strict-rustdoc gate earlier.** Phase 7 turned
on `#![deny(missing_docs)]` for `learnforge-core` and we discovered an
enormous backlog of undocumented public items. Writing the docs as we went
would have been cheaper than writing them all at the end. The "we'll add
docs later" pattern is the documentation equivalent of "we'll write tests
later." It is always more expensive.

**We would have done the WASM-portability work earlier.** Compiling
`learnforge-core` to `wasm32-unknown-unknown` exposed several
non-portable dependency choices that we had to back out (`rusqlite`,
`printpdf`, `image`). Establishing the WASM target as a build gate from
Phase 1 would have prevented these from creeping in. As it was, we had to
extract them in Phase 7 and the extraction was load-bearing for the
crate's value proposition (a Rust crate that does not compile to WASM is a
much less interesting Rust crate in 2026).

## What's next

This is where we are at the moment of the 0.1.0 publish:

- **Phase 8 — Publishing & Open Source Launch** (current). `learnforge-core`
  0.1.0 on crates.io, signed macOS binaries on GitHub Releases, three
  whitepapers in `learnforge-core/docs/` (THRESHOLD, MICROLEARNING, SIGNING),
  the three launch articles you are currently in the middle of (BKT
  explainer, the opinion piece, this retrospective), `SECURITY.md`,
  GitHub Discussions, the versioning policy. Quality-first slow-burn. No
  Hacker News, no Reddit, no Lobsters. Save the big spotlight for v1.0.

- **Phase 9 — Web Platform foundation**. The web app that consumes
  `learnforge-core` via WASM. Same algorithms, different shell. The
  certification verifier is the first surface that needs to ship there.

- **Phase 11+ — Cohorts and corporate features**. The Studio overlay's
  reason to exist. Studio is where multi-tenant orchestration,
  organization-level reporting, and cohort progression live.

- **Phase 14 — Hosted services**. The public verifier endpoint for
  certificates. The pack marketplace. Possibly the static-site generator
  for the docs and blog. The big-launch venue.

- **1.0.0 — target ~December 2026 or later**. The commitment criteria are
  written down in `docs/VERSIONING.md`: two consecutive 0.x.y minor
  releases without breaking-change pressure, at least three external
  consumers, test coverage ≥80%, WASM target on CI, web platform
  consumption stress-tests passing. We will hold the version number until
  all of those are met. Semver is a promise, not a marketing exercise.

## A note on what we are *not* doing

We are not running a Discord. We are not doing webinars. We are not
chasing GitHub stars or vanity metrics. We are not interested in the kind
of growth that comes from optimizing for the launch curve. The slow-burn
strategy is deliberate: we have written the algorithms down, we have
published the code, we have shipped signed binaries, and we are letting
the substance speak for itself. The people who care about adaptive
learning are a small and patient audience. We are building for them, on
their timeline, with the long-term goal of being load-bearing
infrastructure for educational platforms that take learning seriously.

The choice to be load-bearing rather than viral was made in
[PROJECT.md](../../.planning/PROJECT.md) and reaffirmed in every phase
plan since. It is the choice that this whole architecture is consistent
with: pure-function algorithm core, Rust + WASM portability, signed
verifiable credentials, open-source licensing of the substance and
proprietary licensing of the orchestration. Software you can audit. We
think there is a market for that. We are about to find out.

## Further reading

- **[the BKT whitepaper](../../learnforge-core/docs/BKT.md)** — the mastery
  model that anchors the whole platform.
- **[the SM-2 whitepaper](../../learnforge-core/docs/SM2.md)** — spaced
  repetition.
- **[the threshold whitepaper](../../learnforge-core/docs/THRESHOLD.md)**
  — the certification-readiness predicate.
- **[the microlearning whitepaper](../../learnforge-core/docs/MICROLEARNING.md)**
  — selection scoring and desirable difficulty.
- **[the signing whitepaper](../../learnforge-core/docs/SIGNING.md)** —
  Ed25519 + canonical JSON certificate issuance.
- **[the learnforge-core CHANGELOG](../../learnforge-core/CHANGELOG.md)** —
  the per-wave commit history for the Phase 7 extraction.
- **[PROJECT.md](../../.planning/PROJECT.md)** — the project north-star,
  the Definition of Usable, the 2026-05-03 strategic pivot.

---

*This article is licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
Reuse with attribution to LearnForge OSS contributors.*
