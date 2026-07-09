# learnforge-core

> Adaptive learning algorithms — BKT, SM-2, threshold, microlearning
> selection, signing, packs — desktop/web/WASM portable.

[![Crate](https://img.shields.io/badge/crates.io-0.1.0-orange)](https://crates.io/crates/learnforge-core)
[![License](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Whitepapers](https://img.shields.io/badge/whitepapers-CC%20BY%204.0-lightgrey)](./docs)

---

## ⚠ API UNSTABLE

This crate is published at `0.1.x`. **Breaking changes are allowed in any
0.x release.** The public API stabilizes at `1.0.0`. Pin to a specific
minor (`learnforge-core = "0.1"`) and read the [CHANGELOG](./CHANGELOG.md)
before upgrading.

We use [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format and
[Semantic Versioning](https://semver.org/). Phase 8 (Publishing & OSS
Launch) decides crates.io publish timing and the 1.0.0 commitment.

---

## What this is

A focused Rust crate carved out of the LearnForge desktop app, packaging
the **adaptive learning primitives** as a publishable, WASM-portable
library:

- **BKT** — Bayesian Knowledge Tracing for per-skill mastery estimation
  (Corbett & Anderson 1995, four-parameter model). See
  [`docs/BKT.md`](./docs/BKT.md).
- **SM-2** — SuperMemo 2 spaced-repetition scheduler with ease-factor
  decay (Wozniak 1990). See [`docs/SM2.md`](./docs/SM2.md).
- **Threshold predicates** — track-level mastery aggregation +
  certification level computation (foundational / proficient /
  professional).
- **Microlearning selection** — daily-challenge selection with recency
  decay, due-card boosting, and DAG prerequisite gating.
- **Block taxonomy** — module-block types (reading, quiz, video,
  exercise, …) and lifecycle status.
- **Topic packs** — JSON-Schema-validated topic-pack loader for bundled
  and user-installed skill content.
- **Canonical JSON + Ed25519 signing** — deterministic payload
  serialization + signing/verification for certificate issuance.
- **Achievement issuance** — badge / certificate awarding pipeline with
  Phase 14 hosted-verifier forward-compat (see `verifier` module).

## What this is NOT

Explicit non-goals for `learnforge-core`:

- **Not a full LMS.** No user management, no enrollment, no analytics.
- **Not a DB layer.** Persistence is abstracted via small per-module
  `Storage` traits (`BktStore`, `SrStore`, `BlockStore`, `PackStore`,
  `AchievementStore`, `MicrolearningStore`). Bring your own DB. The
  reference impl in [`learnforge`'s `src-tauri`][lf-tauri] uses
  `rusqlite`; web/WASM consumers can implement against IndexedDB.
- **Not a Tauri framework.** Zero Tauri dependencies. The crate compiles
  to `wasm32-unknown-unknown`.
- **Not a renderer.** PDF certificate generation, QR codes, and badge
  rasterization live in `src-tauri` (printpdf / image / qrcode are not
  reliably WASM-portable). This crate ships only the *input shapes*
  (`CertificatePdfInput`, `BadgePngInput`, `share_text()`).
- **Not a hosted verifier.** The `verifier` module is a Phase 14 contract
  stub; the real implementation ships with the hosted verifier service.

[lf-tauri]: https://github.com/agentixgarage/learnforge/tree/main/src-tauri

## Architecture

```text
                ┌─────────────────────────────────────────┐
   Your DB ◀────┤  impl BktStore for &MyConnection {…}    │
                │  impl SrStore for &MyConnection {…}     │
                │  impl PackStore for &MyConnection {…}   │
                │   …per-module Storage traits…           │
                └────────────────┬────────────────────────┘
                                 │
                                 ▼ (sync, no async)
                ┌─────────────────────────────────────────┐
                │  learnforge-core                        │
                │  ┌──────┐ ┌──────┐ ┌─────────────┐      │
                │  │ bkt  │ │ sm2  │ │ threshold   │      │
                │  └──────┘ └──────┘ └─────────────┘      │
                │  ┌──────────────┐ ┌────────────────┐    │
                │  │ microlearning│ │ achievements   │    │
                │  └──────────────┘ └────────────────┘    │
                │  ┌──────────────┐ ┌────────────────┐    │
                │  │ packs        │ │ blocks         │    │
                │  └──────────────┘ └────────────────┘    │
                │  ┌──────────────┐ ┌────────────────┐    │
                │  │canonical_json│ │ signing        │    │
                │  └──────────────┘ └────────────────┘    │
                │  ┌──────────────┐ ┌────────────────┐    │
                │  │ verifier(stub)│ │ storage(traits)│   │
                │  └──────────────┘ └────────────────┘    │
                └─────────────────────────────────────────┘
```

### Per-module Storage trait pattern

Each algorithm module that needs persistence owns a small, focused trait.
The trait surface mirrors the underlying SQL access shape so call-site
churn is minimal during migration. Mocking one trait for a unit test of
(say) `bkt::update_mastery` does not force you to stub `PackStore` or
`AchievementStore`.

This mirrors LearnForge's existing `LabRuntime` + `LearnForgePlugin`
patterns (small focused traits, not god-objects).

## WASM portability

`learnforge-core` is validated for `wasm32-unknown-unknown` at every
Phase 7 wave gate:

```bash
cargo build --target wasm32-unknown-unknown -p learnforge-core
```

The crate's `[target.'cfg(target_arch = "wasm32")']` block in `Cargo.toml`
declares:

- `getrandom 0.3` with the `wasm_js` feature (wires
  `crypto.getRandomValues` as the CSPRNG backend).
- `getrandom 0.2` with the `js` feature (legacy spelling — required
  because `ed25519-dalek 2.x → rand 0.8 → rand_core 0.6` pulls
  `getrandom 0.2` transitively). This duplication disappears when
  `ed25519-dalek 3.x` lands upstream.
- `wasm-bindgen-test 0.3` as a dev-dep for WASM smoke tests (gated to
  `cfg(target_arch = "wasm32")` so host builds don't pull it).

The `chrono` workspace dep enables the `wasmbind` feature so `Utc::now()`
returns wall-clock time (not the Unix epoch) on `wasm32-unknown-unknown`.

### Building for WASM

```bash
# One-time setup (if rustup-managed Rust):
rustup target add wasm32-unknown-unknown

# Build (release recommended; debug bloats heavily on wasm):
cargo build --target wasm32-unknown-unknown -p learnforge-core --release
```

If you see linker errors mentioning `__getrandom_v03_custom_unimpl`,
try setting `RUSTFLAGS='--cfg getrandom_backend="wasm_js"'`. As of
getrandom 0.3.2+ the feature alone is sufficient, but version-dependent
quirks have been reported upstream (see [rust-random/getrandom#267]).

[rust-random/getrandom#267]: https://github.com/rust-random/getrandom/issues/267

### Running WASM tests

Phase 7 Wave 5 (07-05) added `tests/wasm.rs` with two
`#[wasm_bindgen_test]` smoke functions — one exercising
`bkt::update_mastery` and one exercising `ed25519_dalek::SigningKey::generate`
+ `signing::sign_payload` — that prove the pure-math + Ed25519/getrandom
chains both compile and link on `wasm32-unknown-unknown`. The test file
is gated with `#![cfg(target_arch = "wasm32")]` so host `cargo test`
skips it; CI matrix (Phase 9) executes it on the wasm32 target.

Locally, run it with [wasm-pack](https://rustwasm.github.io/wasm-pack/):

```bash
# One-time setup
cargo install wasm-pack

# Node runner (fastest — no browser required)
wasm-pack test --node learnforge-core

# OR browser-side (matches the run_in_browser configure)
wasm-pack test --chrome --headless learnforge-core
```

Verify the test binary at least *compiles* on wasm32 without running it:

```bash
cargo build --tests --target wasm32-unknown-unknown -p learnforge-core
```

## Algorithms

The five core algorithms in this crate ship with whitepaper-style
explainers under [`docs/`](./docs/):

- [`docs/BKT.md`](./docs/BKT.md) — Bayesian Knowledge Tracing model,
  parameters (`P(L_0)`, `P(T)`, `P(G)`, `P(S)`), update equation with
  worked examples, mastery-threshold calibration, decay considerations,
  limitations, references to Corbett & Anderson 1995 and follow-up
  literature.
- [`docs/SM2.md`](./docs/SM2.md) — SuperMemo 2 spaced-repetition
  algorithm, the `0-5` quality scale, ease-factor decay equation,
  interval growth rules, failure-reset behavior, comparison with FSRS,
  worked examples, limitations, references to Wozniak 1990 and the
  testing-effect / spacing-effect literature.
- [`docs/THRESHOLD.md`](./docs/THRESHOLD.md) — track-level achievement
  threshold predicates that aggregate per-module BKT mastery into the
  three skill tiers (Associate 25% / Practitioner 60% / Professional
  100% + 0.85 avg + practical labs), calibration rationale, edge
  cases (decay-vs-mastery, threshold-vs-mastery), references to
  Bloom 1968 mastery learning and AWS / CNCF cert ladders.
- [`docs/MICROLEARNING.md`](./docs/MICROLEARNING.md) — daily-challenge
  selection scoring formula (`W_DECAY` + `W_SR_DUE` + `W_RECENCY`),
  the `[0.3, 0.7)` desirable-difficulty zone (Vygotsky ZPD, Bjork
  desirable difficulty), 48h recency window, 3-day decay half-life,
  empty-zone fallback, references to spacing-effect and testing-effect
  literature.
- [`docs/SIGNING.md`](./docs/SIGNING.md) — Ed25519 + canonical JSON
  byte-stable certificate signing pipeline, `payloadVersion: u32`
  forward-compat dispatch (Phase 14 hosted-verifier contract from
  [`docs/CERT-PAYLOAD-V1.md`](../docs/CERT-PAYLOAD-V1.md)), 8-character
  SHA-256 DER fingerprint, threat model (replay / tampering / untrusted
  signer), references to RFC 8032 (EdDSA), RFC 8785 (JCS), RFC 7515
  (JWS), and Bernstein et al. 2012.

All five whitepapers are MIT licensed (same as the code)
so they may be reused with attribution. Rustdoc on every algorithm
module cross-references the relevant whitepaper for the underlying
mathematics.

## Examples

Runnable single-file demos live in [`examples/`](./examples/):

```bash
# Bayesian Knowledge Tracing: print the mastery trajectory across a
# synthetic observation sequence.
cargo run -p learnforge-core --example bkt_update

# SuperMemo 2: print the schedule across ten reviews, including one
# deliberately-injected failure to show the reset rule.
cargo run -p learnforge-core --example sm2_schedule

# Ed25519 sign/verify against a canonical JSON payload, including a
# tampered-payload negative case and a key-reorder byte-stability proof.
cargo run -p learnforge-core --example verify_payload

# Topic-pack JSON-schema validation against a minimal valid pack and a
# deliberately-broken pack to show the error-list shape.
cargo run -p learnforge-core --example pack_validate
```

Each example is self-contained (no DB, no FS reads beyond the embedded
schema) and runs in well under a second.

## Installation

```toml
[dependencies]
learnforge-core = "0.1"
```

The crate has no platform-conditional features at the consumer level;
the WASM target wiring is transparent.

## Quick example

```rust,ignore
use learnforge_core::verifier;

// Phase 7 stub — the real verifier ships in Phase 14.
let result = verifier::verify(b"<canonical-json-payload>");
assert!(!result.valid);
assert_eq!(result.payload_version, 0);
assert_eq!(
    result.error.as_deref(),
    Some("verifier not implemented in Phase 7; ships in Phase 14"),
);
```

For runnable single-file demos see the [Examples](#examples) section
above (or just run `cargo run -p learnforge-core --example bkt_update`).

## Versioning

- `0.1.x` — Phase 7 (Core Extraction). API unstable. Breaking changes
  allowed between any two 0.1.x releases.
- `0.x` (post-Phase 7) — module surface stabilizes incrementally; each
  module's docs note its stability status.
- `1.0.0` — full API stability commitment. Decided in Phase 8 / 14.

## License

- **Code**: [MIT](./LICENSE).
- **Whitepapers** (`docs/*.md`): MIT (same as the code).

Matches the OSS LearnForge license constraint in `PROJECT.md`.

## Status

**Phase 7 in flight (started 2026-06-16).** This is the publishable
extraction of the adaptive engine from the LearnForge desktop binary.
Wave-by-wave migration is in progress; see the repository's
`.planning/phases/07-core-extraction/` directory for the wave plans and
summaries.

Contributions are welcome via the LearnForge monorepo. See
[CONTRIBUTING.md](https://github.com/agentixgarage/learnforge/blob/main/CONTRIBUTING.md)
and [CLA.md](https://github.com/agentixgarage/learnforge/blob/main/CLA.md)
at the repo root.
