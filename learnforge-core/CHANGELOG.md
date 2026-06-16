# Changelog

All notable changes to `learnforge-core` are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

> **API UNSTABLE notice (Phase 7 / D-06):** while this crate is at
> `0.1.x`, breaking changes are permitted in any 0.x release. The API
> stabilizes at `1.0.0`. Read this changelog before upgrading.

## [Unreleased]

### Added

- **`canonical_json` module (Phase 7 Wave 5 / 07-05)** — byte-stable JSON
  serializer moved verbatim from
  `src-tauri/src/achievements/signing.rs:93-133` (canonicalize +
  canonical_json_bytes). Object keys are sorted lexicographically at
  every nesting level; non-finite floats (NaN, +∞, -∞) are rejected via
  the typed `CanonicalJsonError::NonFiniteFloat` variant (Phase 6 R1 /
  Pitfall 2 preserved). Pure, WASM-portable — no `std::fs`, no
  `rusqlite`. 6 unit tests (5 moved verbatim + 1 new
  `canonicalize_preserves_array_order` locking the "arrays are not
  sorted" semantic) + 2 doctests.
- **`signing` module (Phase 7 Wave 5 / 07-05)** — pure Ed25519 sign /
  verify primitives + `SigningKeyStore` trait + `share_text` template.
  `sign_payload`, `verify_payload`, `public_key_fingerprint`,
  `fingerprint_from_public_pem` moved verbatim from
  `src-tauri/src/achievements/signing.rs:135-177`; `share_text` moved
  from `src-tauri/src/achievements/artifacts.rs:278` per the D-03
  amendment (PDF / PNG renderers stay in src-tauri because printpdf /
  image / qrcode are not WASM-portable). Adds the `SigningKeyStore`
  trait (A3 lock — per-module storage trait): `get_or_init` +
  `export_public_pem`. The FS-backed impl (`FsKeyStore`) lives in
  `src-tauri/src/storage_impl/signing.rs` (D-03 amendment + Pitfall 4 —
  `std::fs` is not WASM-portable). `SigningError` enum
  (`thiserror::Error` derive) preserves Phase 6 error semantics with
  `From<CanonicalJsonError>` for ergonomic propagation. **Function
  signatures preserved verbatim** from the pre-Wave-5 src-tauri form
  (`Signature` return, PEM-string + hex-string verify) instead of
  switching to raw byte buffers as the plan's `<interfaces>` block
  sketched — keeps the call-site churn confined to the module
  boundary. 11 unit tests (8 moved verbatim + 3 new —
  `sign_then_tamper_payload_fails_verify` for the plan's behavior
  contract, `signature_is_64_bytes` sanity, `signing_error_renders` +
  `signing_key_store_is_implementable` lock the trait surface) + 2
  doctests. WASM build (`cargo build --target wasm32-unknown-unknown
  -p learnforge-core`) green — Ed25519 + getrandom-wasm_js chain
  validated end-to-end.
- **`microlearning` module (Phase 7 Wave 4 / 07-04)** — daily-challenge
  selection algorithm moved verbatim from
  `src-tauri/src/learning/microlearning_selection.rs`. Adds the
  `MicrolearningStore` trait (A3 lock — per-module storage trait) with
  five methods covering the four SQL touch points the pre-Wave-4 file
  exposed (`candidate_modules`, `blocks_for_module`, `is_recently_seen`,
  `module_has_due_sr_card`, `decay_days_for_module`) — Pitfall 9
  resolution. `select_daily_challenge<S: MicrolearningStore>` is
  parameterized with an explicit `now: DateTime<Utc>` (A5 clock
  injection / Pitfall 10 mitigation): the algorithm never calls
  `chrono::Utc::now()` internally, so WASM builds cannot leak the
  1970 epoch and unit tests pin a deterministic timestamp. Exports
  the public scoring constants (`BKT_LOWER`, `BKT_UPPER`, `W_DECAY`,
  `W_SR_DUE`, `W_RECENCY`, `RECENCY_PENALTY_HOURS`,
  `DECAY_HALF_LIFE_DAYS`, `DECAY_DAYS_CAP_MULT`). Also adds the
  `MicrolearningError` enum (`thiserror::Error` derive) backed by a
  single `Backend(String)` variant — same T-07-05 trust-boundary
  stringification pattern as `BktError` / `SrError`. Rustdoc on every
  public item; 10 unit tests using inline stub stores + 1 doctest. No
  `rusqlite` in this module; WASM build still succeeds (R1 / D-02
  intact).
- **`threshold` module (Phase 7 Wave 4 / 07-04)** — pure skill-tier
  predicates moved verbatim from
  `src-tauri/src/achievements/threshold.rs`: `TrackAggregate` struct,
  `which_level_just_crossed`, `levels_met`, and private `ratio` /
  `is_professional` helpers. **No `rusqlite` in this module** — the SQL
  aggregate query that builds a `TrackAggregate` from `module_progress`
  rows (`track_mastery_aggregate`) is **parked in
  `src-tauri/src/storage_impl/threshold.rs`** as a free function until
  Wave 8 (`07-08-PLAN.md`) promotes it into a method on the forthcoming
  `AchievementStore` trait. Wave 4 deliberately defers that step so the
  move stays mechanical. 8 unit tests moved verbatim + 5 doctests. WASM
  build still green (R1 / D-02 boundary intact).
- **`sm2` module (Phase 7 Wave 3 / 07-03)** — `SM2Result`, `sm2_calculate`
  moved verbatim from `src-tauri/src/learning/spaced_repetition.rs`. Adds
  the `SrStore` trait, `SrError` enum, and `SrCardRow` row type next to
  the algorithm (A3 lock — per-module storage trait). Trait surface
  enumerated via a grep audit of `src-tauri` `sr_cards` SQL: four methods
  cover every existing read/write path (`read_due_cards`,
  `count_due_cards_for_module`, `read_card_by_id`, `apply_review_update`).
  `SrCardRow` keeps the reference schema's ISO-datetime string shape for
  `next_review` / `last_review` (the SQLite table stores them as `TEXT`
  produced via `datetime('now', ...)`), so the rusqlite adapter is a 1:1
  row mapping rather than an in-flight unit conversion. Rustdoc on every
  public item; 15 unit tests (11 moved verbatim + 4 new — trait-compiles,
  apply-review-update dispatch, error rendering) and 4 doctests. No
  `rusqlite` in `learnforge-core::sm2`; WASM build still succeeds (R1 /
  D-02 / T-07-07 mitigations intact).
- **`path` module (Phase 7 Wave 2 / 07-02)** — pure DAG primitives moved
  from `src-tauri/src/learning/path.rs`: `EdgeRecord`, `PathNode`,
  `PathEdge`, `PathError`, `parse_edges_json`, `validate_dag`.
  `all_prerequisites_mastered` is reimplemented as a trait-driven free
  function `pub fn all_prerequisites_mastered<S: BktStore>(...)` —
  closes Pitfall 8 (mixed pure/DB code split) by making the prereq
  check depend on `BktStore` rather than `rusqlite::Connection`.
  Diamond-DAG correctness preserved (legacy `.unwrap_or(0.0)` semantics
  for missing rows). Five new tests use an inline `MapStore: BktStore`
  stub — no DB needed.
- **`bkt` module (Phase 7 Wave 2 / 07-02)** — `BKTParams`, `MASTERY_THRESHOLD`,
  `update_mastery`, `should_adapt` moved verbatim from
  `src-tauri/src/learning/adaptive.rs`. Adds the `BktStore` trait and
  `BktError` enum next to the algorithm (A3 lock — per-module trait
  location). Rustdoc on every public item; doctest examples for the
  threshold constant, `BKTParams::default`, `update_mastery`,
  `should_adapt`, `BktStore`. WASM-portable (no rusqlite leak; D-02 +
  T-07-05 mitigated by stringified `BktError::Db`).
- Per-Phase-7-wave deliverables continue to append here.

### Changed

- **`src-tauri/src/learning/microlearning_selection.rs` is now a
  transitional shim** (Phase 7 Wave 4 / 07-04) — re-exports the algorithm
  surface (`Candidate`, `CandidateModule`, `MicrolearningError`,
  `MicrolearningStore`, and the scoring constants) from
  `learnforge_core::microlearning`, and keeps a legacy
  `select_daily_challenge(&Connection, &str) -> Result<Option<Candidate>, String>`
  wrapper that supplies `chrono::Utc::now()` at the call site so the
  single existing caller (`commands/microlearning.rs:32`) compiles
  unchanged. The rusqlite-backed impl lives at
  `src-tauri/src/storage_impl/microlearning.rs::SqliteMicrolearningStore<'a>(&'a Connection)`
  — same orphan-rule newtype recipe Waves 2/3 introduced for
  `SqliteBktStore` / `SqliteSrStore` (E0117 prevents
  `impl MicrolearningStore for &Connection` directly). 6 adapter unit
  tests + 6 cross-crate integration tests at the shim cover end-to-end
  behavior. Wave 10 grep-and-rewrite will switch the command caller to
  invoke the core fn directly with its own clock + typed error.
- **`src-tauri/src/achievements/threshold.rs` is now a transitional
  shim** (Phase 7 Wave 4 / 07-04) — pure predicates re-export from
  `learnforge_core::threshold` while the SQL aggregate
  (`track_mastery_aggregate`) re-exports from
  `crate::storage_impl::threshold`. The single caller
  (`achievements::mod::maybe_issue`) compiles unchanged. Wave 8 will
  promote `track_mastery_aggregate` into a method on the forthcoming
  `AchievementStore` trait — that's the moment the SQL also gets hidden
  behind a trait, matching the `BktStore` / `SrStore` /
  `MicrolearningStore` pattern.
- **`src-tauri/src/learning/spaced_repetition.rs` is now a transitional
  shim** (Phase 7 Wave 3 / 07-03) — `pub use learnforge_core::sm2::{SM2Result,
  sm2_calculate}`. The single remaining caller
  (`commands/learning.rs::submit_review`) compiles unchanged. The
  rusqlite-backed `SrStore` impl lives in
  `src-tauri/src/storage_impl/sr.rs` as
  `SqliteSrStore<'a>(pub &'a Connection)` — same newtype recipe as
  `SqliteBktStore` from Wave 2 (orphan-rule E0117 prevents
  `impl SrStore for &Connection` directly). 6 adapter unit tests cover the
  four trait methods (due-card read + limit, count-due-for-module,
  read-by-id present + not-found, apply-review-update persist).
  Adapter stringifies `rusqlite::Error::QueryReturnedNoRows` to
  `SrError::NotFound { card_id }` and all other rusqlite errors to
  `SrError::Db(string)` — T-07-07 trust-boundary mitigation matches
  Wave 2's `BktError` pattern.
- **src-tauri now depends on `learnforge-core` via path dep** (workspace
  D-09 wired). `src-tauri/src/learning/adaptive.rs` and
  `src-tauri/src/learning/path.rs` are transitional shims that re-export
  from `learnforge_core::bkt` and `learnforge_core::path` respectively
  (deleted in Wave 10). All four pre-existing call sites
  (`commands/learning.rs`, `commands/ai.rs`, `learning/microlearning_selection.rs`,
  `learning/path.rs`) compile UNCHANGED through the shims. The shims
  intentionally do NOT use `#[deprecated]` (rustc may silently ignore it
  on `pub use` items — R5 / Pitfall 6).
- **Rusqlite-backed `BktStore` impl** lives in
  `src-tauri/src/storage_impl/bkt.rs` as `SqliteBktStore<'a>(pub &'a
  Connection)`. The plan-verbatim wording `impl BktStore for &Connection`
  would violate Rust's orphan rule (E0117) because both the trait
  (learnforge_core) and the target type (rusqlite) are foreign — the
  zero-cost newtype wrapper satisfies the local-type requirement.

## [0.1.0] - 2026-06-16

### Added

- **Crate scaffold** — `learnforge-core` published as a new workspace
  member alongside `src-tauri`, `pro/src-tauri-pro`, and
  `pro/src-tauri-pro/licensing`. Phase 7 Wave 1 (`07-01-PLAN.md`,
  decisions D-01 / D-09).
- **`verifier` module stub** — `VerifyResult` struct with `camelCase`
  serde renaming for IPC compatibility + `VerifyResult::not_implemented()`
  + `verify(_payload: &[u8]) -> VerifyResult` returning the
  not-implemented sentinel. Locks the Phase 14 hosted-verifier interface
  contract (decision D-08).
- **Workspace dependency block** — root `Cargo.toml` carries a
  `[workspace.dependencies]` section declaring the shared dep set
  (serde, serde_json, thiserror, chrono w/ `wasmbind`, ed25519-dalek,
  pkcs8, sha2, hex, base64, rand, uuid w/ `js`, jsonschema w/
  `default-features = false`, include_dir, log, tempfile). Future waves
  reference these via `serde = { workspace = true }` to prevent version
  drift between core and src-tauri (Open Q A2 = YES lock).
- **WASM target wiring** — `[target.'cfg(target_arch = "wasm32")']`
  block in `learnforge-core/Cargo.toml` declares `getrandom 0.3` with
  the `wasm_js` feature AND `getrandom 0.2` (renamed package) with the
  `js` feature, since `ed25519-dalek 2.x` pulls both major versions
  transitively. `wasm-bindgen-test 0.3` is wired as a wasm-only
  dev-dep for the smoke test infrastructure (preps Wave 5 + Wave 9
  per decision D-04). `cargo build --target wasm32-unknown-unknown -p
  learnforge-core` returns exit 0.
- **Publish artifacts** — `README.md` (API-unstable callout, architecture
  diagram, WASM build instructions, whitepaper pointers), `CHANGELOG.md`
  (this file), and `LICENSE` (MIT) shipped at the crate root for the
  Phase 8 crates.io publish gate.
- **Standard Stack dependencies** — `serde`, `serde_json`, `thiserror`,
  `chrono` (w/ `wasmbind`), `log`, `uuid` (w/ `js`), `ed25519-dalek`,
  `pkcs8`, `sha2`, `hex`, `base64`, `rand`, `jsonschema`
  (w/ `default-features = false`), `include_dir`. No `rusqlite`, no
  `tauri`, no `printpdf` / `image` / `qrcode`, no `reqwest`, no
  `tokio`, no `async-trait` (decision D-02 anti-leakage boundary).

### Notes

- This release ships **no algorithm code**. The BKT / SM-2 / threshold /
  microlearning / signing / packs / blocks / achievements modules land
  in Waves 2-8 (per decision D-05).
- Crate is **not yet published** to crates.io. Phase 8 (Publishing &
  OSS Launch) will publish 0.1.x after Phase 7 lands fully.

[Unreleased]: https://github.com/schoolofdevops/learnforge/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/schoolofdevops/learnforge/releases/tag/v0.1.0
