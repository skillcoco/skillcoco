//! Phase 6 (Certification) — Ed25519 keypair lifecycle + canonical-JSON
//! payload helpers.
//!
//! Wave 0: every function declared, every body either `unimplemented!()` or
//! returns a sentinel. The RED unit tests at the bottom assert the shape
//! Wave 1 (Plan 06-02) must satisfy.
//!
//! Security invariants (R3, R1):
//!   - Private key never crosses IPC.
//!   - On Unix the key file is written 0600 (Pitfall 4).
//!   - Canonical JSON sorts keys lexicographically before signing
//!     (Pitfall 2 — see `docs/CERT-PAYLOAD-V1.md`).
//!   - Key fingerprint = first 8 hex chars of SHA-256 of the verifying key's
//!     DER bytes (R5 — locked answer A7).

use super::AchievementError;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use serde::Serialize;
use std::path::Path;

/// Load the per-install signing key from `<key_dir>/cert_signing_private.pem`,
/// generating a fresh keypair (and writing both PEMs to disk with 0600
/// perms on Unix) on first call.
///
/// Wave 0 stub panics. Wave 1 (Plan 06-02) fills the real impl per
/// 06-RESEARCH.md "Ed25519 keypair generation + PKCS#8 PEM round-trip".
pub fn get_or_init_key(_key_path: &Path) -> Result<SigningKey, AchievementError> {
    unimplemented!("Plan 06-02 (Wave 1) implements get_or_init_key")
}

/// Serialize `payload` to JSON with object keys sorted lexicographically —
/// the byte sequence Ed25519 then signs. Determinism is mandatory: the
/// Phase 14 hosted verifier must reproduce the same bytes from the same
/// logical payload (R1, Pitfall 2).
///
/// Wave 0 returns Err. Wave 1 fills via `serde_json::Value` ->
/// `serde_json::Map<String, Value>` with sorted keys.
pub fn canonical_json_bytes<T: Serialize>(_payload: &T) -> Result<Vec<u8>, AchievementError> {
    Err(AchievementError::Validation(
        "Plan 06-02 (Wave 1) implements canonical_json_bytes".to_string(),
    ))
}

/// Sign canonical bytes with the cached signing key. Wave 0 panics.
pub fn sign_payload(_key: &SigningKey, _canonical_bytes: &[u8]) -> Signature {
    unimplemented!("Plan 06-02 (Wave 1) implements sign_payload")
}

/// Verify a signature against canonical bytes using a PEM-encoded public
/// key. Returns `false` on any decode/parse failure (never panics).
///
/// Wave 0 always returns `false`. Wave 1 (Plan 06-02) fills.
pub fn verify_payload(
    _public_pem: &str,
    _canonical_bytes: &[u8],
    _sig_hex: &str,
) -> bool {
    false
}

/// Compute the 8-character key fingerprint (first 8 hex chars of
/// SHA-256(verifying_key.to_public_key_der())). R5 / locked answer A7.
///
/// Wave 0 returns `"00000000"` (RED — Wave 1 fills).
pub fn public_key_fingerprint(_verifying: &VerifyingKey) -> String {
    "00000000".to_string()
}

#[cfg(test)]
mod tests {
    //! Wave 0 RED contract tests. Each asserts a Wave 1+ invariant.

    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn ephemeral_key() -> SigningKey {
        // Generate a throwaway key in-memory ONLY (never written to disk).
        // The rand_core feature on ed25519-dalek 2.x pulls OsRng.
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    #[ignore = "Plan 06-02 (Wave 1) implements sign + verify"]
    fn sign_verify_roundtrip() {
        // RED contract: sign(canonical) then verify(public_pem, canonical, sig_hex) == true.
        let key = ephemeral_key();
        let payload = b"learnforge-test-payload";
        let sig = sign_payload(&key, payload);
        let sig_hex = hex::encode(sig.to_bytes());
        // Wave 1 fills `public_key_pem` extraction; for now we just assert
        // the verify wrapper returns true given a real signature.
        // Stub returns false unconditionally — RED.
        let dummy_pem = "-----BEGIN PUBLIC KEY-----\n-----END PUBLIC KEY-----\n";
        assert!(
            verify_payload(dummy_pem, payload, &sig_hex),
            "Wave 1: verify_payload(real_pem, canonical, sig_hex) must return true"
        );
    }

    #[test]
    #[ignore = "Plan 06-02 (Wave 1) implements canonical_json_bytes"]
    fn canonical_json_byte_stable() {
        // RED contract: two calls with the same payload yield byte-identical Vec<u8>.
        // Pitfall 2 — map-key ordering must be deterministic.
        #[derive(serde::Serialize)]
        struct Probe {
            b: u32,
            a: u32,
        }
        let p = Probe { b: 2, a: 1 };
        let first = canonical_json_bytes(&p).expect("first canonicalization");
        let second = canonical_json_bytes(&p).expect("second canonicalization");
        assert_eq!(
            first, second,
            "canonical_json_bytes must be byte-stable across calls"
        );
        // Sorted-key invariant: 'a' must precede 'b' in the output.
        let s = std::str::from_utf8(&first).expect("utf-8");
        let pos_a = s.find("\"a\"").expect("a key present");
        let pos_b = s.find("\"b\"").expect("b key present");
        assert!(
            pos_a < pos_b,
            "canonical JSON must sort keys lexicographically (got: {})",
            s
        );
    }

    #[test]
    #[ignore = "Plan 06-02 (Wave 1) implements public_key_fingerprint"]
    fn fingerprint_is_8_hex_chars() {
        // RED contract: fingerprint is 8 lowercase hex chars (SHA-256[..8]).
        // Stub returns "00000000" — Wave 1 fills.
        let key = ephemeral_key();
        let fp = public_key_fingerprint(&key.verifying_key());
        assert_eq!(fp.len(), 8, "fingerprint must be 8 chars");
        assert!(
            fp.chars().all(|c| c.is_ascii_hexdigit()),
            "fingerprint must be lowercase hex (got {})",
            fp
        );
        assert_ne!(
            fp, "00000000",
            "Wave 1 fingerprint must NOT be the Wave 0 placeholder"
        );
    }
}
