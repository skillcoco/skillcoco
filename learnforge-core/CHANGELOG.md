# Changelog

All notable changes to `learnforge-core` are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

> **API UNSTABLE notice (Phase 7 / D-06):** while this crate is at
> `0.1.x`, breaking changes are permitted in any 0.x release. The API
> stabilizes at `1.0.0`. Read this changelog before upgrading.

## [Unreleased]

### Added

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
