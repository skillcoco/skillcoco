//! Ed25519 signing — pure crypto primitives + key-store trait.
//!
//! Moved during Phase 7 Wave 5 (07-05) from
//! `src-tauri/src/achievements/signing.rs`. **Pure half only** —
//! `sign_payload`, `verify_payload`, `public_key_fingerprint`,
//! `fingerprint_from_public_pem`, `share_text`, `SigningError`, and the
//! [`SigningKeyStore`] trait. The FS-backed key lifecycle
//! (`get_or_init_key`, `read_public_pem`, 0o600 file mode) STAYS in
//! `src-tauri/src/storage_impl/signing.rs` as the [`SigningKeyStore`] impl
//! ([`FsKeyStore`](https://github.com/agentixgarage/learnforge)) — D-03
//! amendment + Pitfall 4 (FS-backed key loading is not WASM-portable).
//!
//! ## Invariants preserved from Phase 6
//!
//! - **R5 / A7** — fingerprint is the first 8 lowercase hex chars of
//!   SHA-256 of the verifying key's DER bytes.
//! - **R1 / Pitfall 2** — `verify_payload` accepts a PEM-encoded public key
//!   plus a hex-encoded signature; both are deserialized through
//!   `ed25519_dalek::pkcs8::DecodePublicKey` / `hex::decode` so any I/O
//!   shape (file, IPC string, etc.) re-enters the same path.
//! - **T-07-14** — no `std::fs`, no `std::path::Path`, no Tauri imports in
//!   this module; the trait surface uses only `&[u8]`, `SigningKey`,
//!   `&str`, `String`.
//! - **T-07-13** — Ed25519 key generation goes through `OsRng` (and
//!   ultimately `getrandom`); Wave 1 wired `getrandom 0.3 wasm_js` +
//!   `getrandom 0.2 js` so wasm32 builds get `crypto.getRandomValues()` as
//!   the entropy source.
//!
//! Wave 5 keeps the existing function signatures (`Signature` return,
//! PEM-string + hex-string verify) instead of switching to raw byte
//! buffers as the plan's `<interfaces>` block sketched. The callsites
//! `src-tauri/src/achievements/mod.rs:188-189` and
//! `src-tauri/src/commands/achievements.rs:718-719,819` already pass
//! these shapes; preserving the signatures keeps churn confined to the
//! module boundary (the move is mechanical, not a refactor).
//!
//! ## Example
//!
//! ```
//! use learnforge_core::signing::{sign_payload, verify_payload, public_key_fingerprint};
//! use ed25519_dalek::{SigningKey, pkcs8::EncodePublicKey};
//! use rand::rngs::OsRng;
//!
//! let key = SigningKey::generate(&mut OsRng);
//! let payload = b"learnforge-doctest-payload";
//!
//! let sig = sign_payload(&key, payload);
//! let sig_hex = hex::encode(sig.to_bytes());
//!
//! let pub_pem = key
//!     .verifying_key()
//!     .to_public_key_pem(pkcs8::LineEnding::LF)
//!     .unwrap();
//!
//! assert!(verify_payload(&pub_pem, payload, &sig_hex));
//! assert!(!verify_payload(&pub_pem, b"tampered", &sig_hex));
//!
//! let fp = public_key_fingerprint(&key.verifying_key());
//! assert_eq!(fp.len(), 8);
//! ```

use crate::canonical_json::CanonicalJsonError;
use ed25519_dalek::pkcs8::{DecodePublicKey, EncodePublicKey};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors returned by signing-related operations.
///
/// `Io` is populated only by the FS-backed [`SigningKeyStore`] impl in
/// `src-tauri/src/storage_impl/signing.rs`; the pure functions in this
/// module never touch the filesystem.
#[derive(Debug, Error)]
pub enum SigningError {
    /// Signature failed cryptographic verification.
    #[error("signature invalid")]
    InvalidSignature,

    /// PEM / PKCS#8 encoding or decoding failed.
    #[error("key encoding error: {0}")]
    KeyEncoding(String),

    /// I/O error from the FS-backed key store. Populated by `FsKeyStore`
    /// (src-tauri side); the pure crypto functions never raise this.
    #[error("io error: {0}")]
    Io(String),

    /// Canonical JSON serialization failure (wrapped from
    /// [`crate::canonical_json::CanonicalJsonError`]).
    #[error("canonical json error: {0}")]
    Canonical(#[from] CanonicalJsonError),
}

/// Sign canonical bytes with the cached signing key.
///
/// Returns the raw [`Signature`] (64 bytes). Callers typically render it
/// as hex (`hex::encode(sig.to_bytes())`) for transport.
///
/// # Example
///
/// ```
/// use learnforge_core::signing::sign_payload;
/// use ed25519_dalek::SigningKey;
/// use rand::rngs::OsRng;
///
/// let key = SigningKey::generate(&mut OsRng);
/// let sig = sign_payload(&key, b"payload");
/// assert_eq!(sig.to_bytes().len(), 64);
/// ```
pub fn sign_payload(key: &SigningKey, canonical_bytes: &[u8]) -> Signature {
    key.sign(canonical_bytes)
}

/// Verify a signature against canonical bytes using a PEM-encoded public
/// key. Returns `false` on any decode/parse failure (never panics).
///
/// `sig_hex` is the lowercase hex string produced by
/// `hex::encode(signature.to_bytes())`. Garbage PEM, garbage hex, or a
/// signature that doesn't match the payload all yield `false`.
pub fn verify_payload(public_pem: &str, canonical_bytes: &[u8], sig_hex: &str) -> bool {
    let Ok(verifying) = VerifyingKey::from_public_key_pem(public_pem) else {
        return false;
    };
    let Ok(sig_bytes) = hex::decode(sig_hex) else {
        return false;
    };
    let Ok(sig_array): Result<[u8; 64], _> = sig_bytes.as_slice().try_into() else {
        return false;
    };
    let sig = Signature::from_bytes(&sig_array);
    verifying.verify(canonical_bytes, &sig).is_ok()
}

/// First 8 lowercase hex chars of SHA-256 of the verifying key's DER bytes
/// (R5 / A7). Falls back to raw 32-byte key if DER encoding fails (never
/// panics in production paths).
pub fn public_key_fingerprint(verifying: &VerifyingKey) -> String {
    let der_bytes: Vec<u8> = verifying
        .to_public_key_der()
        .map(|d| d.as_bytes().to_vec())
        .unwrap_or_else(|_| verifying.as_bytes().to_vec());
    let hash = Sha256::digest(&der_bytes);
    hex::encode(&hash[..4])
}

/// Re-derive the 8-hex SHA-256 fingerprint from a public-key PEM string.
///
/// Pure function — no disk I/O. Used by Settings to populate the
/// localFingerprint label on mount without running a full verify pass.
/// Returns [`SigningError::KeyEncoding`] on malformed PEM (never panics).
pub fn fingerprint_from_public_pem(pem: &str) -> Result<String, SigningError> {
    let verifying = VerifyingKey::from_public_key_pem(pem)
        .map_err(|e| SigningError::KeyEncoding(format!("decode public pem: {}", e)))?;
    Ok(public_key_fingerprint(&verifying))
}

/// Locked share-text template (no emoji, no newlines).
///
/// Moved from `src-tauri/src/achievements/artifacts.rs:278` during Phase 7
/// Wave 5 per D-03 amendment. The PDF / PNG renderers (printpdf, image,
/// qrcode) stay in src-tauri because those crates are not reliably
/// WASM-portable; only the pure string template lives here.
pub fn share_text(level: &str, track: &str, key_fingerprint: &str, payload_b64: &str) -> String {
    format!(
        "I just earned {} in {} on LearnForge. Verify with key fingerprint {}: {}",
        level, track, key_fingerprint, payload_b64
    )
}

/// Storage trait for the long-lived per-install signing keypair.
///
/// Declared next to the algorithm (A3 lock — per-module storage trait
/// location); the FS-backed implementation lives in
/// `src-tauri/src/storage_impl/signing.rs` so the WASM build of
/// `learnforge-core` doesn't pull `std::fs`.
///
/// Implementations are responsible for any platform-specific isolation
/// (Unix 0o600 mode, Windows per-user app-data ACL, IndexedDB storage on
/// the web). The trait surface itself promises only that
/// `get_or_init` returns a usable [`SigningKey`] and `export_public_pem`
/// returns the matching PEM-encoded verifying key.
pub trait SigningKeyStore {
    /// Return the per-install [`SigningKey`], generating a fresh keypair
    /// (and persisting it) on first call.
    fn get_or_init(&self) -> Result<SigningKey, SigningError>;

    /// Return the PEM-encoded public (verifying) key matching the most
    /// recent `get_or_init` keypair. Used by the Settings IPC to surface
    /// the public key for sharing / verification.
    fn export_public_pem(&self) -> Result<String, SigningError>;
}

#[cfg(test)]
mod tests {
    //! Pure-crypto tests moved verbatim from pre-Wave-5
    //! `src-tauri/src/achievements/signing.rs:179-323`. FS-backed tests
    //! (`generate_then_load`, `private_key_file_mode_0600`) stay in
    //! src-tauri alongside `FsKeyStore`.

    use super::*;
    use ed25519_dalek::pkcs8::EncodePublicKey;
    use ed25519_dalek::SigningKey;
    use pkcs8::LineEnding;
    use rand::rngs::OsRng;

    fn ephemeral_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn sign_verify_roundtrip() {
        let key = ephemeral_key();
        let payload = b"learnforge-test-payload";
        let sig = sign_payload(&key, payload);
        let sig_hex = hex::encode(sig.to_bytes());
        let pub_pem = key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("encode public pem");

        assert!(verify_payload(&pub_pem, payload, &sig_hex), "real verify");
        assert!(
            !verify_payload(&pub_pem, b"learnforge-test-paylo!d", &sig_hex),
            "tampered payload"
        );
        let mut tampered_sig = sig_hex.clone();
        tampered_sig.replace_range(0..2, "00");
        assert!(
            !verify_payload(&pub_pem, payload, &tampered_sig),
            "tampered sig"
        );
        assert!(
            !verify_payload(&pub_pem, payload, "not-hex"),
            "garbage hex"
        );
        assert!(
            !verify_payload("not a pem", payload, &sig_hex),
            "garbage pem"
        );
    }

    /// Negative test required by 07-05-PLAN behavior block: flip one byte
    /// of the payload and assert verify returns false. (The
    /// `sign_verify_roundtrip` test above already exercises tampered
    /// payload + tampered sig; this one explicitly asserts the
    /// single-byte-flip case for the plan's `<behavior>` documentation.)
    #[test]
    fn sign_then_tamper_payload_fails_verify() {
        let key = ephemeral_key();
        let payload = b"the quick brown fox jumps over the lazy dog";
        let sig = sign_payload(&key, payload);
        let sig_hex = hex::encode(sig.to_bytes());
        let pub_pem = key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("encode public pem");

        // Flip exactly one byte (the first one).
        let mut tampered = payload.to_vec();
        tampered[0] ^= 0x01;

        assert!(
            verify_payload(&pub_pem, payload, &sig_hex),
            "original verifies"
        );
        assert!(
            !verify_payload(&pub_pem, &tampered, &sig_hex),
            "single-byte-flip payload must fail verify"
        );
    }

    /// Signature is 64 bytes (Ed25519 invariant — sanity check the locked
    /// length so future ed25519-dalek upgrades don't silently change it).
    #[test]
    fn signature_is_64_bytes() {
        let key = ephemeral_key();
        let sig = sign_payload(&key, b"x");
        assert_eq!(sig.to_bytes().len(), 64);
    }

    /// R5 / A7 — fingerprint is exactly 8 lowercase hex chars.
    #[test]
    fn public_key_fingerprint_8_hex_chars() {
        let fp = public_key_fingerprint(&ephemeral_key().verifying_key());
        assert_eq!(fp.len(), 8, "fingerprint must be 8 chars");
        assert!(
            fp.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "fingerprint must be lowercase hex (got {})",
            fp
        );
        assert_ne!(fp, "00000000", "must not be Wave 0 placeholder");
    }

    /// Two random keypairs must yield two distinct fingerprints (sanity).
    #[test]
    fn fingerprint_differs_across_keys() {
        let fp1 = public_key_fingerprint(&ephemeral_key().verifying_key());
        let fp2 = public_key_fingerprint(&ephemeral_key().verifying_key());
        assert_ne!(fp1, fp2);
    }

    /// fingerprint_from_public_pem returns the same 8-hex chars as
    /// public_key_fingerprint(&verifying) for any valid key.
    #[test]
    fn fingerprint_from_public_pem_roundtrip() {
        let key = ephemeral_key();
        let verifying = key.verifying_key();
        let pem = verifying
            .to_public_key_pem(LineEnding::LF)
            .expect("encode public pem");
        let fp_from_pem = fingerprint_from_public_pem(&pem).expect("fingerprint from pem");
        let fp_direct = public_key_fingerprint(&verifying);
        assert_eq!(fp_from_pem, fp_direct);
        assert_eq!(fp_from_pem.len(), 8);
    }

    /// Malformed PEM yields a typed error (never panics).
    #[test]
    fn fingerprint_from_public_pem_rejects_malformed() {
        let result = fingerprint_from_public_pem("not a pem");
        assert!(result.is_err(), "malformed PEM must error");
        if let Err(SigningError::KeyEncoding(_)) = result {
            // expected branch
        } else {
            panic!("expected SigningError::KeyEncoding, got {:?}", result);
        }
    }

    /// share_text template assertions moved from
    /// src-tauri/src/achievements/artifacts.rs:401-426.
    #[test]
    fn share_text_template() {
        let s = share_text("Professional", "Kubernetes Fundamentals", "a1b2c3d4", "QUJD");
        assert_eq!(
            s,
            "I just earned Professional in Kubernetes Fundamentals on LearnForge. \
             Verify with key fingerprint a1b2c3d4: QUJD"
                .replace("             ", "")
        );
    }

    #[test]
    fn share_text_no_emoji() {
        let s = share_text("Associate", "DevOps", "abcd1234", "payload");
        // No characters in emoji ranges.
        for c in s.chars() {
            let u = c as u32;
            assert!(
                !((0x1F300..=0x1FAFF).contains(&u)
                    || (0x2600..=0x27BF).contains(&u)
                    || u == 0xFE0F),
                "share_text must not contain emoji, found {:?} (U+{:04X})",
                c,
                u
            );
        }
    }

    /// SigningError's Display strings stay stable (renderer-facing).
    #[test]
    fn signing_error_renders() {
        assert_eq!(
            SigningError::InvalidSignature.to_string(),
            "signature invalid"
        );
        assert_eq!(
            SigningError::KeyEncoding("oops".to_string()).to_string(),
            "key encoding error: oops"
        );
        assert_eq!(
            SigningError::Io("disk full".to_string()).to_string(),
            "io error: disk full"
        );
        let canonical_err: SigningError = CanonicalJsonError::NonFiniteFloat.into();
        assert!(canonical_err.to_string().starts_with("canonical json error"));
    }

    /// The trait can be implemented and dispatched dynamically (compile-
    /// time check that the surface is object-safe and ergonomic).
    #[test]
    fn signing_key_store_is_implementable() {
        struct InMemoryStore {
            key: SigningKey,
        }
        impl SigningKeyStore for InMemoryStore {
            fn get_or_init(&self) -> Result<SigningKey, SigningError> {
                Ok(SigningKey::from_bytes(&self.key.to_bytes()))
            }
            fn export_public_pem(&self) -> Result<String, SigningError> {
                self.key
                    .verifying_key()
                    .to_public_key_pem(LineEnding::LF)
                    .map_err(|e| SigningError::KeyEncoding(format!("encode public pem: {}", e)))
            }
        }

        let store = InMemoryStore {
            key: ephemeral_key(),
        };
        let k1 = store.get_or_init().expect("get");
        let k2 = store.get_or_init().expect("get again");
        assert_eq!(k1.to_bytes(), k2.to_bytes(), "stable across calls");

        let pem = store.export_public_pem().expect("export pem");
        assert!(pem.starts_with("-----BEGIN PUBLIC KEY-----"));

        // dyn-dispatch ergonomics
        let dyn_store: &dyn SigningKeyStore = &store;
        let _ = dyn_store.get_or_init().expect("dyn get_or_init");
    }
}
