# Changelog

All notable changes to `learnforge-core` are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

> **API UNSTABLE notice (Phase 7 / D-06):** while this crate is at
> `0.1.x`, breaking changes are permitted in any 0.x release. The API
> stabilizes at `1.0.0`. Read this changelog before upgrading.

## [Unreleased]

### Added

- **`achievements` module (Phase 7 Wave 8 / 07-08)** — achievement-issuance
  algorithm moved from `src-tauri/src/achievements/mod.rs`. This is the
  **final algorithmic move wave**; after Wave 8 every algorithm in the
  learning loop lives in `learnforge-core` and the
  `wasm32-unknown-unknown` build compiles cleanly without ever pulling
  `rusqlite` / `printpdf` / `image` / `qrcode` / `tauri` into the
  dependency graph.

  Exports `Achievement` + `CertPayloadV1` + `TrackCertifications` (all
  `#[serde(rename_all = "camelCase")]` because they cross IPC),
  `IssuanceContext` (the per-track display snapshot the algorithm reads
  via the storage trait), `CertificatePdfInput` + `BadgePngInput` (PDF/PNG
  renderer **input shapes** only — D-03 amendment locks the renderers
  themselves in src-tauri because printpdf / qrcode / image are not
  WASM-portable), `AchievementError` (with `#[from]` for
  `serde_json::Error`, `SigningError`, and `CanonicalJsonError`), the
  `AchievementStore` trait (eighth and final application of the per-module
  storage-trait recipe — A3 lock), and the `maybe_issue<S: AchievementStore,
  K: SigningKeyStore>` free function.

  **A5 clock injection lock:** `maybe_issue` accepts an explicit `now:
  chrono::DateTime<chrono::Utc>` parameter instead of calling
  `Utc::now()` internally. Tests pin `now` so the canonical payload
  bytes (and therefore the Ed25519 signatures) are byte-for-byte
  reproducible across runs — `signature_byte_stable_under_pinned_clock`
  asserts this. The src-tauri shim wrapper supplies `Utc::now()` at the
  call site to preserve production behavior.

  **Wave-4 ↔ Wave-8 seam closed:** the rusqlite `AchievementStore`
  implementation in `src-tauri/src/storage_impl/achievements.rs` wires
  its `track_mastery_aggregate` method body straight into the Wave 4
  parked free fn in `src-tauri/src/storage_impl/threshold.rs`. The
  parking comment in that file is removed; the seam is now visible at
  trait-method scope.

  **What does NOT live here:** PDF/PNG/QR rendering (artifacts.rs stays
  in src-tauri), FS-backed key loading (FsKeyStore stays in
  storage_impl/signing.rs from Wave 5), and `From<rusqlite::Error> for
  AchievementError` (the rusqlite-touching conversion lives only on the
  src-tauri side so the core trait surface stays pure).

- **`packs` module (Phase 7 Wave 7 / 07-07)** — topic-pack subsystem moved
  from `src-tauri/src/topic_packs/`. Exports `Pack`, `PackModule`,
  `PackEdge`, `LoadedPack`, `PackSource` enum (Bundled/Skill — origin
  marker, serialized as snake_case), `ValidationStatus` enum (Ok/Warnings/
  Errors — snake_case), `PackError` enum (Io/Json/Schema/Loader), the
  Draft 2020-12 JSON Schema validator (`compile`, `validate`,
  `SCHEMA_SOURCE` — embedded via `include_str!("../../../topic-packs/
  pack-schema.json")`; **R2 / Pitfall 1 mitigated**: the original Wave-7
  plan over-counted directory depth as 4; `rustc` resolves `include_str!`
  paths relative to the file's directory not the crate root, so 3 segments
  reach the repo root from `learnforge-core/src/packs/schema.rs`. Same
  three-segment string worked at the pre-move site too because both
  source files sit at the same depth.), the `BUNDLED_PACKS` static
  (`include_dir!("$CARGO_MANIFEST_DIR/../topic-packs")` — one `..` up
  from `learnforge-core/` to the repo root), `parse_and_validate`,
  `classify_errors` (D-07 strict/soft classifier), `sentinel_pack`,
  `now_rfc3339`, and the in-memory `PackRegistry`.
  Adds two new traits:
  - **`PackStore` trait** (in `packs::persistence`) — abstract persistence
    over the `topic_packs` SQLite table; methods `upsert_pack`,
    `read_enabled`, `write_enabled`, `delete_skill_rows`. Honors D-09
    (user toggle survives upsert) + CR-02 (source column sticky on
    `bundled`). Seventh application of the per-module storage-trait
    recipe (A3 lock). Rusqlite-backed impl lives in
    `src-tauri/src/storage_impl/packs.rs` via the
    `SqlitePackStore<'a>(&'a Connection)` newtype.
  - **`PackSource` trait** (in `packs::loader`) — abstract runtime
    discovery of skill packs from disk; methods `skills_dir`,
    `read_skill_pack_files`. R3 / Pitfall 4 mitigation: the FS-touching
    code (`std::fs::read_dir`, `std::fs::canonicalize`, `dirs::home_dir`,
    T-05-05 symlink-escape rejection, T-05-06 5 MB cap) moves to
    `FsPackSource` in `src-tauri/src/topic_packs/loader.rs` rather than
    being `#[cfg(not(target_arch = "wasm32"))]`-gated. The trait makes the
    seam visible + testable; the cfg-gate alternative was rejected
    (clutters production source, hides the seam from tooling, and
    misframes the rusqlite-vs-IndexedDB split as "wasm vs not-wasm" when
    it is actually "FS-backed vs browser-backed").

  **Naming note:** the enum [`PackSource`] (Bundled/Skill marker) and the
  trait [`PackSource`] share the identifier but live in different modules
  (`packs::model::PackSource` vs `packs::loader::PackSource`). Only the
  enum is re-exported at `packs::PackSource` to avoid shadowing; trait
  callers reference `learnforge_core::packs::loader::PackSource`.

  **WASM proof (A4):** `cargo build --target wasm32-unknown-unknown -p
  learnforge-core` exit 0 — confirms `jsonschema 0.46` with
  `default-features = false` builds on wasm32. The 0.46 feature trim
  strips `resolve-file` + `resolve-http` (both non-wasm-portable);
  `include_dir` and `chrono` (with `wasmbind`) carry the rest of the
  graph cleanly. This was the wave's medium-confidence open question
  (`07-RESEARCH.md` A4); proven via the build gate.

  Pure data + algorithm types — no `rusqlite`, no `tauri`, no `std::fs`
  read in core (`std::path::PathBuf` is used only in the `PackSource`
  trait return type — host-safe). D-02 boundary intact.

- **`blocks` module (Phase 7 Wave 6 / 07-06)** — block taxonomy moved
  verbatim from `src-tauri/src/db/blocks.rs:1-65` (pre-Wave-6). Exports
  `BlockType` enum (Section, Text, Callout, Quiz, FlashCards, Lab —
  serialized as snake_case), `BlockStatus` enum (Pending, Generating,
  Ready, Failed — serialized as snake_case), `ModuleBlock` row struct
  (camelCase serde **preserved** because the struct itself crosses the
  Tauri IPC boundary in `commands/blocks.rs` — 96.7KB / most-called IPC
  surface; this is the established convention for any future
  domain-type-that-crosses-IPC), `block_type_to_str` +
  `status_to_str` helper fns, and the new `BlockStore` trait + `BlocksError`
  enum. The trait surface (`insert`, `list_for_module`, `get_by_id`,
  `update_payload`, `count_for_module`, `delete_for_module`) was
  enumerated by auditing the six existing CRUD free fns in pre-Wave-6
  src-tauri (A3 lock — sixth application of the per-module storage trait
  recipe). Pure data types — no `rusqlite`, no `tauri`, no `std::fs`;
  WASM-portable. 11 unit tests (8 type-level serde + helper coverage + 3
  trait surface: `block_store_trait_compiles` exercising every method
  against an in-memory stub, `block_store_is_object_safe` locking
  `&dyn BlockStore`, `blocks_error_renders` locking the Display strings)
  + 4 doctests (BlockType / BlockStatus / block_type_to_str /
  status_to_str module examples). WASM build (`cargo build --target
  wasm32-unknown-unknown -p learnforge-core`) green.

- **WASM smoke test (Phase 7 Wave 5 / 07-05)** — `learnforge-core/tests/wasm.rs`
  ships two `#[wasm_bindgen_test]` functions: `bkt_update_runs_in_wasm`
  (proves the pure-math BKT path compiles + runs on wasm32 — closes
  D-04) and `ed25519_sign_runs_in_wasm` (proves `SigningKey::generate`
  → `OsRng` → `getrandom` → `crypto.getRandomValues()` chain links on
  wasm32 — closes R1 + T-07-13 mitigation). Gated with
  `#![cfg(target_arch = "wasm32")]` so host `cargo test` skips them;
  `cargo build --tests --target wasm32-unknown-unknown -p learnforge-core`
  succeeds (validates the test binary compiles end-to-end including
  the Ed25519 + getrandom path). Phase 9 wires CI to actually execute
  the tests via `wasm-pack test`.

### Changed

- **`src-tauri/src/topic_packs/*` is now a transitional shim group**
  (Phase 7 Wave 7 / 07-07) — `mod.rs`, `error.rs`, `model.rs`,
  `schema.rs`, `registry.rs` re-export from `learnforge_core::packs::*`.
  The pre-Wave-7 bodies were lifted into core; the shim files are 1-10
  lines each. `commands.rs` is **UNCHANGED** (Pitfall 7 — Tauri IPC
  handlers cannot move because they use `tauri::AppState`).
- **`src-tauri/src/topic_packs/loader.rs` is now a transitional shim +
  `FsPackSource` impl** (Phase 7 Wave 7 / 07-07) — pure helpers
  (`BUNDLED_PACKS`, `parse_and_validate`, `classify_errors`,
  `sentinel_pack`, `now_rfc3339`) re-exported from
  `learnforge_core::packs::loader`. The FS-touching skill-pack scan
  (`std::fs::read_dir`, `std::fs::canonicalize`, `dirs::home_dir`,
  T-05-05 symlink-escape rejection, T-05-06 5 MB cap) lives here as the
  `FsPackSource` newtype, implementing the new
  `learnforge_core::packs::loader::PackSource` trait. The orchestration
  free fns `load_all(conn)` and `reload_skills_into(reg, conn)` stay
  here (they bind the pure loader to rusqlite via
  `crate::topic_packs::persistence`) so the two pre-Wave-7 call sites
  (`lib.rs:156` and `commands::reload_skills`) compile unchanged. Wave 10
  cleanup rewrites the call sites onto
  `learnforge_core::packs::loader::*` + `FsPackSource` directly and
  deletes the shim. R3 mitigation via TRAIT (chosen over
  `#[cfg(not(target_arch = "wasm32"))]` because the trait makes the seam
  testable + visible in tooling).
- **`src-tauri/src/topic_packs/persistence.rs` is now a transitional
  shim** (Phase 7 Wave 7 / 07-07) — re-exports `PackStore` trait + pure
  mappers (`source_str`, `status_str`) from
  `learnforge_core::packs::persistence`. The four legacy free fns
  (`upsert_pack`, `read_enabled`, `write_enabled`, `delete_skill_rows`)
  are 1-line forwards to `SqlitePackStore(conn).{method}(…)` so existing
  call sites (`topic_packs::commands::*` + the legacy unit tests) compile
  unchanged. **Error-envelope change** — the legacy facades' return type
  shifted from `rusqlite::Result<T>` (pre-Wave-7) to
  `Result<T, PackError>` (post-Wave-7). Every existing call site uses
  `.map_err(|e| format!(\"...: {}\", e))` or `.ok().flatten()?`, both of
  which work unchanged because `PackError` implements `Display` and
  `.ok()` discards the error type. Zero call-site code changes needed.
- **`src-tauri/src/storage_impl/packs.rs` (new, Phase 7 Wave 7 /
  07-07)** — `SqlitePackStore<'a>(pub &'a Connection)` newtype carrying
  the rusqlite-backed `PackStore` impl (D-09 + CR-02 contracts preserved
  verbatim from pre-Wave-7 SQL). Seventh application of the orphan-rule
  recipe established Waves 2-6. 8 lib tests against in-memory
  `Connection`, including a CR-02 regression guard and an
  object-safety smoke.

- **`src-tauri/src/db/blocks.rs` is now a transitional shim** (Phase 7
  Wave 6 / 07-06) — re-exports the type surface (`BlockType`,
  `BlockStatus`, `ModuleBlock`, `BlocksError`, `BlockStore`,
  `block_type_to_str`, `status_to_str`) from `learnforge_core::blocks`
  and keeps six legacy free-fn facades (`insert_block`,
  `list_blocks_by_module`, `get_block`, `update_block_payload`,
  `count_blocks_by_module`, `delete_blocks_by_module`) that delegate to
  the trait impl via `SqliteBlockStore(conn)`. **Zero call-site churn**
  for `commands/blocks.rs` (96.7KB / most-called IPC surface),
  `commands/ai.rs:502`, `labs/{eval,session,session_tests,state}.rs`,
  and `commands/learning.rs:309`. **Error envelope change** — the
  legacy facades now return `Result<_, BlocksError>` instead of the
  pre-Wave-6 `rusqlite::Result<_>` (i.e. `Result<_, rusqlite::Error>`);
  every existing call site uses `.map_err(|e| e.to_string())` or
  `format!("get_block: {}", e)`, both of which work unchanged because
  `BlocksError` derives `thiserror::Error` (Display). Wave 10 deletes
  the shim once callsites migrate onto `SqliteBlockStore(conn)`
  directly. **No `#[deprecated]`** on the `pub use` items (R5 / Pitfall
  6 — rustc silently ignores it).
- **`src-tauri/src/storage_impl/blocks.rs` (new, Phase 7 Wave 6 /
  07-06)** — rusqlite-backed `BlockStore` impl via the local newtype
  `SqliteBlockStore<'a>(pub &'a Connection)` (sixth application of the
  orphan-rule recipe, Waves 2/3/4/5 precedent). All six trait method
  bodies are lifted **verbatim** from pre-Wave-6
  `src-tauri/src/db/blocks.rs:68-185` with the error envelope rewrapped
  from `rusqlite::Error` → `BlocksError::Db` at the trust boundary
  (T-07-05). 6 unit tests cover the full CRUD surface (insert+list,
  get_by_id present/absent, update_payload, count, delete-and-empty,
  trait-object-safety).

- **`achievements::signing` (src-tauri) → transitional shim (Phase 7
  Wave 5 / 07-05)** — pre-Wave-5 the file was the single home for both
  pure crypto and FS-backed key lifecycle; post-Wave-5 it's a thin
  compatibility layer re-exporting the pure surface from
  `learnforge_core::{canonical_json, signing}` and delegating FS-backed
  ops to `crate::storage_impl::signing::FsKeyStore`. Legacy wrapper fns
  `get_or_init_key` + `read_public_pem` + `canonical_json_bytes` keep
  their pre-Wave-5 `Result<_, AchievementError>` signatures so the
  existing callsites (`achievements::mod::maybe_issue` + IPC handlers
  in `commands/achievements.rs`) compile unchanged. `From<SigningError>
  for AchievementError` + `From<CanonicalJsonError> for AchievementError`
  impls live in this shim. Wave 10 deletes the shim once the callsites
  migrate onto `learnforge_core::signing` directly.
- **`achievements::artifacts::share_text` (src-tauri) → re-export
  (Phase 7 Wave 5 / 07-05)** — the canonical template implementation
  moved to `learnforge_core::signing::share_text` per the D-03
  amendment (PDF / PNG renderers stay in src-tauri because printpdf /
  image / qrcode are not reliably WASM-portable). `artifacts.rs`
  re-exports it so the legacy callsite path
  (`achievements::artifacts::share_text`) and the two existing
  template tests in that module compile unchanged.
- **`src-tauri::storage_impl::signing::FsKeyStore` (new, src-tauri
  Phase 7 Wave 5 / 07-05)** — filesystem-backed [`SigningKeyStore`]
  impl. Body lifted **verbatim** from pre-Wave-5
  `achievements/signing.rs:45-89` (the `get_or_init_key` +
  `read_public_pem` halves) so the 0o600 file-mode invariant (R3 /
  Pitfall 4 / V6 ASVS) on Unix is preserved exactly. 5 unit tests
  cover the FS-touching surface: `generate_then_load` +
  `private_key_file_mode_0600` (both lifted verbatim from pre-Wave-5
  src-tauri signing.rs) plus `export_public_pem_roundtrips`,
  `export_public_pem_errors_when_missing`, `fs_key_store_is_object_safe`
  (new — lock the trait surface for the IPC code that holds a boxed
  store).
- **`src-tauri/Cargo.toml` (Phase 7 Wave 5 / 07-05)** — `sha2` removed
  from the `[dependencies]` block: no direct user remains after the
  pure crypto move (`rg "sha2|Sha256" src-tauri/src/ --type rust` → 0
  hits). All other Phase 6 crypto deps stay because they have direct
  callsite users in `src-tauri/src/achievements/mod.rs` +
  `src-tauri/src/commands/achievements.rs` + the new
  `src-tauri/src/storage_impl/signing.rs`: `ed25519-dalek`, `pkcs8`,
  `base64`, `hex`, `rand` remain declared. Transitive sha2 is still
  pulled through `learnforge-core` so no resolution-graph change
  occurs.

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
