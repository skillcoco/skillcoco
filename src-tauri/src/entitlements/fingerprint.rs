// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)

//! SHA-256 fingerprint of a license key (D-06).
//!
//! D-06: the raw license key is NEVER stored — only its SHA-256 fingerprint,
//! computed client-side before any persistence. Mirrors the
//! `learnforge_core::signing::public_key_fingerprint` pure-fn,
//! never-panic shape (15-PATTERNS.md).
//!
//! Wave 0 (15-01): `sha256_fingerprint` is a stub returning `String::new()`.
//! 15-02 fills in the real `sha2::Sha256` + `hex::encode` body.

/// Compute a stable, non-reversible fingerprint of `license_key` for local
/// storage (D-06 — the raw key itself must never be persisted). Wave 0 stub
/// returns an empty string; 15-02 implements `hex::encode(Sha256::digest(..))`.
pub fn sha256_fingerprint(_license_key: &str) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D-06 / T-15-01 — the fingerprint must be (a) stable across calls for
    /// the same input and (b) never contain the raw key as a substring, so a
    /// leaked fingerprint can never be reversed back to the license key by
    /// simple inspection. RED until 15-02 implements the real hash.
    #[test]
    fn sha256_fingerprint_is_stable_and_never_raw() {
        let key = "secret-key";
        let fp1 = sha256_fingerprint(key);
        let fp2 = sha256_fingerprint(key);
        assert_eq!(fp1, fp2, "fingerprint must be stable across calls");
        assert!(
            !fp1.is_empty(),
            "15-02: sha256_fingerprint must not return an empty string for a non-empty key"
        );
        assert!(
            !fp1.contains(key),
            "fingerprint must never contain the raw key substring (D-06)"
        );
    }
}
