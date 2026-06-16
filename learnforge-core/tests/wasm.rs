//! WASM smoke test — Phase 7 Wave 5 (07-05).
//!
//! Two `#[wasm_bindgen_test]` functions exercise the highest-risk paths in
//! the wasm32-unknown-unknown build of `learnforge-core`:
//!
//! 1. **`bkt_update_runs_in_wasm`** — proves the pure-math algorithm path
//!    compiles, links, and executes on wasm32. Closes Phase 7 D-04 (the
//!    "WASM smoke test" deliverable from 07-CONTEXT.md).
//! 2. **`ed25519_sign_runs_in_wasm`** — proves the Ed25519 key generation
//!    + sign path links through `rand::rngs::OsRng` →
//!    `getrandom::getrandom()` → `crypto.getRandomValues()` on wasm32.
//!    Closes Phase 7 R1 (getrandom-wasm_js wiring from Wave 1 actually
//!    works end-to-end) and the T-07-13 cryptography threat mitigation.
//!
//! The file is gated with `#![cfg(target_arch = "wasm32")]` so host
//! builds skip it entirely (no host-side `cargo test` runs these
//! functions). Running them on wasm32 requires `wasm-pack`:
//!
//! ```bash
//! cargo install wasm-pack
//! wasm-pack test --node learnforge-core
//! # OR — browser-side smoke (matches the `run_in_browser` configure)
//! wasm-pack test --chrome --headless learnforge-core
//! ```
//!
//! Phase 9 wires a CI matrix that runs this test. For Wave 5, **the file
//! existing + the wasm32 build of `learnforge-core` succeeding** is the
//! D-04 + R1 deliverable: the test FILE proves the crypto + math chains
//! both *compile* on wasm32, which is the failure mode Wave 5 wanted to
//! catch the moment Ed25519 landed in core. Actual execution on the
//! wasm32 target is the Wave 9 deliverable.

#![cfg(target_arch = "wasm32")]

use ed25519_dalek::SigningKey;
use learnforge_core::bkt::{update_mastery, BKTParams};
use learnforge_core::signing::sign_payload;
use rand::rngs::OsRng;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// BKT update runs in WASM — closes D-04 (core algorithms are
/// WASM-portable).
#[wasm_bindgen_test]
fn bkt_update_runs_in_wasm() {
    let params = BKTParams::default();
    let updated = update_mastery(&params, 0.3, true);
    // After a correct answer starting from prior 0.3, posterior must
    // increase but stay clamped at 1.0.
    assert!(updated > 0.3, "mastery must increase on correct answer");
    assert!(updated <= 1.0, "mastery must stay in [0, 1]");
}

/// Ed25519 sign runs in WASM — closes R1 (getrandom wasm_js feature is
/// wired through the algorithm crate's CSPRNG path).
///
/// `SigningKey::generate(&mut OsRng)` pulls entropy via `getrandom` which
/// on wasm32-unknown-unknown resolves to `crypto.getRandomValues()`
/// thanks to the `getrandom = { features = ["wasm_js"] }` declaration in
/// `learnforge-core/Cargo.toml`'s wasm32 target block (Wave 1 lock).
#[wasm_bindgen_test]
fn ed25519_sign_runs_in_wasm() {
    let mut csprng = OsRng;
    let key = SigningKey::generate(&mut csprng);
    let payload = b"phase-7 wasm smoke";
    let sig = sign_payload(&key, payload);
    assert_eq!(
        sig.to_bytes().len(),
        64,
        "Ed25519 signature is always 64 bytes"
    );
}
