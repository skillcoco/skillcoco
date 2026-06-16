//! Phase 6 — Ed25519 keypair lifecycle + canonical-JSON payload helpers.
//!
//! Security invariants:
//!   - R3 / Pitfall 4: Unix private key file is 0600 immediately after write.
//!   - R1 / Pitfall 2: `canonical_json_bytes` sorts object keys lexicographically
//!     at every nesting level — byte-stable signing input.
//!   - R5 / A7: fingerprint = first 8 lowercase hex of SHA-256 of verifying
//!     key DER bytes.
//!   - Private key never crosses the IPC boundary.

use super::AchievementError;
use ed25519_dalek::pkcs8::{
    DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use pkcs8::LineEnding;
use rand::rngs::OsRng;
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// File name (relative to the keys directory) for the private signing key.
const PRIV_FILE: &str = "cert_signing_private.pem";
/// File name (relative to the keys directory) for the public signing key.
const PUB_FILE: &str = "cert_signing_public.pem";

/// Resolve the on-disk private-key path inside the keys directory.
fn priv_path(key_dir: &Path) -> PathBuf {
    key_dir.join(PRIV_FILE)
}

/// Resolve the on-disk public-key path inside the keys directory.
fn pub_path(key_dir: &Path) -> PathBuf {
    key_dir.join(PUB_FILE)
}

/// Load the per-install signing key from `<key_dir>/cert_signing_private.pem`,
/// generating a fresh keypair (and writing both PEMs to disk with 0600
/// perms on Unix) on first call.
///
/// On Unix, the private file is chmod'd to 0o600 immediately after write
/// (R3 / Pitfall 4). On Windows, per-user app-data ACLs provide the
/// isolation; no explicit mode change.
pub fn get_or_init_key(key_dir: &Path) -> Result<SigningKey, AchievementError> {
    let priv_p = priv_path(key_dir);

    if priv_p.exists() {
        let pem = std::fs::read_to_string(&priv_p)?;
        let key = SigningKey::from_pkcs8_pem(&pem)
            .map_err(|e| AchievementError::Pkcs8(format!("decode private pem: {}", e)))?;
        return Ok(key);
    }

    // Fresh keypair path.
    std::fs::create_dir_all(key_dir)?;
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);

    // Encode + write the private PEM.
    let priv_pem = signing_key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| AchievementError::Pkcs8(format!("encode private pem: {}", e)))?;
    std::fs::write(&priv_p, priv_pem.as_bytes())?;

    // R3 / Pitfall 4 — enforce 0600 on Unix immediately after write.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&priv_p, std::fs::Permissions::from_mode(0o600))?;
    }

    // Encode + write the public PEM. World-readable on disk is acceptable
    // (verifying keys are public information).
    let pub_pem = signing_key
        .verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| AchievementError::Pkcs8(format!("encode public pem: {}", e)))?;
    std::fs::write(pub_path(key_dir), pub_pem)?;

    Ok(signing_key)
}

/// Read the on-disk public-key PEM (convenience for the future Settings
/// "Show signing public key" panel — Wave 5).
pub fn read_public_pem(key_dir: &Path) -> Result<String, AchievementError> {
    let pem = std::fs::read_to_string(pub_path(key_dir))?;
    Ok(pem)
}

/// Recursively canonicalize a JSON value: sort object keys lexicographically
/// at every nesting level; reject non-finite numbers (R1).
fn canonicalize(v: Value) -> Result<Value, AchievementError> {
    match v {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut sorted = Map::with_capacity(entries.len());
            for (k, val) in entries {
                sorted.insert(k, canonicalize(val)?);
            }
            Ok(Value::Object(sorted))
        }
        Value::Array(items) => items
            .into_iter()
            .map(canonicalize)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if !f.is_finite() {
                    return Err(AchievementError::Validation(
                        "non-finite number in payload".to_string(),
                    ));
                }
            }
            Ok(Value::Number(n))
        }
        other => Ok(other),
    }
}

/// Serialize `payload` to JSON with object keys sorted lexicographically —
/// the byte sequence Ed25519 then signs.
///
/// Determinism is mandatory: the Phase 14 hosted verifier must reproduce the
/// same bytes from the same logical payload (R1 / Pitfall 2 / CERT-PAYLOAD-V1).
pub fn canonical_json_bytes<T: Serialize>(payload: &T) -> Result<Vec<u8>, AchievementError> {
    let v: Value = serde_json::to_value(payload)?;
    let canonical = canonicalize(v)?;
    let bytes = serde_json::to_vec(&canonical)?;
    Ok(bytes)
}

/// Sign canonical bytes with the cached signing key.
pub fn sign_payload(key: &SigningKey, canonical_bytes: &[u8]) -> Signature {
    key.sign(canonical_bytes)
}

/// Verify a signature against canonical bytes using a PEM-encoded public
/// key. Returns `false` on any decode/parse failure (never panics).
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
/// Pure function — no disk I/O. Used by Wave 5 Settings to populate the
/// localFingerprint label on mount without running a full verify pass.
/// Returns `AchievementError::Pkcs8(...)` on malformed PEM (never panics).
pub fn fingerprint_from_public_pem(pem: &str) -> Result<String, AchievementError> {
    let verifying = VerifyingKey::from_public_key_pem(pem)
        .map_err(|e| AchievementError::Pkcs8(format!("decode public pem: {}", e)))?;
    Ok(public_key_fingerprint(&verifying))
}

#[cfg(test)]
mod tests {
    //! Wave 1 GREEN tests. Each asserts a Wave 1+ invariant required by
    //! `docs/CERT-PAYLOAD-V1.md` and 06-CONTEXT.md D-04..D-13.

    use super::*;
    use ed25519_dalek::SigningKey;
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
        assert!(!verify_payload(&pub_pem, payload, &tampered_sig), "tampered sig");
        assert!(!verify_payload(&pub_pem, payload, "not-hex"), "garbage hex");
        assert!(!verify_payload("not a pem", payload, &sig_hex), "garbage pem");
    }

    #[test]
    fn generate_then_load() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let key_dir = tmp.path();
        let k1 = get_or_init_key(key_dir).expect("first init");
        assert!(key_dir.join(PRIV_FILE).exists() && key_dir.join(PUB_FILE).exists());
        let k2 = get_or_init_key(key_dir).expect("reload");
        assert_eq!(k1.to_bytes(), k2.to_bytes(), "reload yields same key");
        assert_eq!(
            public_key_fingerprint(&k1.verifying_key()),
            public_key_fingerprint(&k2.verifying_key())
        );
    }

    /// R3 / Pitfall 4 — private key file is 0600 on Unix immediately after write.
    #[test]
    #[cfg(unix)]
    fn private_key_file_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().expect("tempdir");
        let _k = get_or_init_key(tmp.path()).expect("init");
        let meta = std::fs::metadata(tmp.path().join(PRIV_FILE)).expect("metadata");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {:o}", mode);
    }

    /// R1 / Pitfall 2 — byte-stable AND keys sort lexicographically at every level.
    #[test]
    fn canonical_json_byte_stable() {
        #[derive(serde::Serialize)]
        struct Probe { b: u32, a: u32, nested: Nested }
        #[derive(serde::Serialize)]
        struct Nested { z: u32, m: u32, a: u32 }
        let p = Probe { b: 2, a: 1, nested: Nested { z: 99, m: 5, a: 0 } };
        let first = canonical_json_bytes(&p).unwrap();
        assert_eq!(first, canonical_json_bytes(&p).unwrap(), "byte-stable");

        let s = std::str::from_utf8(&first).unwrap();
        let pa = s.find("\"a\"").unwrap();
        let pb = s.find("\"b\"").unwrap();
        let pn = s.find("\"nested\"").unwrap();
        assert!(pa < pb && pb < pn, "top-level keys must sort (got: {})", s);

        let nested = &s[pn..];
        let na = nested.find("\"a\":").unwrap();
        let nm = nested.find("\"m\":").unwrap();
        let nz = nested.find("\"z\":").unwrap();
        assert!(na < nm && nm < nz, "nested keys must sort (got: {})", nested);
    }

    /// Finite floats pass; serde_json::Number itself rejects NaN/Inf at
    /// construction (canonicalize's non-finite check is second-line defense).
    #[test]
    fn canonical_json_rejects_non_finite_mastery() {
        #[derive(serde::Serialize)]
        struct GoodPayload { mastery_score: f64 }
        canonical_json_bytes(&GoodPayload { mastery_score: 0.85 }).expect("finite ok");
        assert!(serde_json::Number::from_f64(f64::NAN).is_none());
        assert!(serde_json::Number::from_f64(f64::INFINITY).is_none());
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
    /// public_key_fingerprint(&verifying) for any valid key (Wave 5 enabler).
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
        if let Err(AchievementError::Pkcs8(_)) = result {
            // expected branch
        } else {
            panic!("expected AchievementError::Pkcs8, got {:?}", result);
        }
    }
}
