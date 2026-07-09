//! Pack-signing chain of trust: RFC 8785 (JCS) canonicalization + Ed25519
//! root → issuer-cert → pack verification (Phase 14).
//!
//! PURE / WASM-CLEAN (D-08): no filesystem, no network, no clock. Every
//! function here is a pure function of its inputs so the exact same
//! verification path runs on desktop and (later) wasm32.
//!
//! Canonicalization: this module uses `serde_json_canonicalizer` (RFC 8785
//! JSON Canonicalization Scheme) — NOT `crate::canonical_json::canonicalize`,
//! which is the Phase-6 achievement-payload canonicalizer (lexicographic key
//! sort only, not JCS-spec number formatting / string escaping). Mixing the
//! two silently breaks cross-implementation signature interop (14-RESEARCH
//! Pitfall 2).
//!
//! Module contents (verify/cert model, error taxonomy, RED tests) land in
//! plan 14-01 Task 3; implementations land in 14-02.
