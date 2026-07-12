// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)

//! SHA-256 fingerprint of a license key (D-06).
//!
//! D-06: the raw license key is NEVER stored — only its SHA-256 fingerprint,
//! computed client-side before any persistence. Mirrors the
//! `learnforge_core::signing::public_key_fingerprint` pure-fn,
//! never-panic shape (15-PATTERNS.md).
//!
use sha2::{Digest, Sha256};

/// Compute a stable, non-reversible fingerprint of `license_key` for local
/// storage (D-06 — the raw key itself must never be persisted). Full
/// 32-byte / 64-hex SHA-256 digest for collision safety. Pure fn, never
/// panics.
pub fn sha256_fingerprint(license_key: &str) -> String {
    hex::encode(Sha256::digest(license_key.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D-06 / T-15-01 — the fingerprint must be (a) stable across calls for
    /// the same input, (b) match the pinned SHA-256 hex digest, and (c)
    /// never contain the raw key as a substring, so a leaked fingerprint can
    /// never be reversed back to the license key by simple inspection.
    #[test]
    fn sha256_fingerprint_is_stable_and_never_raw() {
        let key = "secret-key";
        let fp1 = sha256_fingerprint(key);
        let fp2 = sha256_fingerprint(key);
        assert_eq!(fp1, fp2, "fingerprint must be stable across calls");
        assert_eq!(
            fp1, "85dbe15d75ef9308c7ae0f33c7a324cc6f4bf519a2ed2f3027bd33c140a4f9aa",
            "fingerprint must match the pinned SHA-256 hex digest of 'secret-key'"
        );
        assert!(
            !fp1.is_empty(),
            "sha256_fingerprint must not return an empty string for a non-empty key"
        );
        assert!(
            !fp1.contains(key),
            "fingerprint must never contain the raw key substring (D-06)"
        );
    }
}
