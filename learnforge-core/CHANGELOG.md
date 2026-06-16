# Changelog

All notable changes to `learnforge-core` are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

> **API UNSTABLE notice (Phase 7 / D-06):** while this crate is at
> `0.1.x`, breaking changes are permitted in any 0.x release. The API
> stabilizes at `1.0.0`. Read this changelog before upgrading.

## [Unreleased]

### Added

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
