//! Phase 14 verification contract — stub.
//!
//! Phase 7 (D-08) locks the interface so Phase 14 has a concrete signature
//! to drop the hosted verifier implementation into. Until then, every call
//! to [`verify`] returns [`VerifyResult::not_implemented`] so consumers can
//! wire UI / IPC plumbing today and get a clear error message at runtime.
//!
//! ## Forward-compat contract
//!
//! ```ignore
//! pub struct VerifyInput<'a> {
//!     pub payload: &'a [u8],
//!     pub signature: &'a [u8],
//!     pub public_key_pem: &'a str,
//! }
//!
//! pub struct VerifyResult {
//!     pub valid: bool,
//!     pub error: Option<String>,
//!     pub payload_version: u32,
//! }
//!
//! pub fn verify(input: VerifyInput<'_>) -> VerifyResult;
//! ```
//!
//! `payload_version` is a `u32` so Phase 14 can dispatch on the
//! certificate-payload schema version (e.g. v1, v2) without breaking the
//! struct layout. [`VerifyInput`] carries the three values Ed25519
//! verification needs (canonical payload bytes, signature bytes,
//! verifying-key PEM) so Phase 14 can fill in the body without altering
//! the call shape — addressing the WR-03 finding from the Phase 7 code
//! review that the previous single-`&[u8]` parameter would force a
//! breaking change in Phase 14.
//!
//! ## Phase 14 dispatch flow (planned)
//!
//! 1. Phase 14 introduces a hosted-verifier registry that maps the
//!    8-hex SHA-256 fingerprint embedded in
//!    [`crate::achievements::CertPayloadV1::key_fingerprint`] to a
//!    trusted signing key. Callers pass the resolved PEM as
//!    [`VerifyInput::public_key_pem`].
//! 2. [`verify`] parses [`VerifyInput::payload`] as canonical JSON,
//!    reads its `payloadVersion` field, and dispatches to the per-version
//!    validation routine (v1 → [`crate::achievements::CertPayloadV1`]
//!    semantics; v2+ TBD).
//! 3. The Ed25519 check uses [`crate::signing::verify_payload`] under the
//!    hood, parameterized by the registry-resolved PEM.
//! 4. `VerifyResult::payload_version` carries the dispatched version
//!    back to the caller so the UI can label what was checked.
//!
//! Wave 0 / Phase 7: the stub returns [`VerifyResult::not_implemented`]
//! (`payload_version = 0`) regardless of input — consumers can wire UI
//! today and observe a clear runtime surface until Phase 14 lights up
//! the body.
//!
//! See `docs/CERT-PAYLOAD-V1.md` in the repo root for the payload format.

use serde::{Deserialize, Serialize};

/// Inputs to [`verify`].
///
/// Bundles the three byte-stable values Ed25519 verification needs into
/// one borrow-only struct so Phase 14 can fill in the verifier body
/// without altering the [`verify`] call shape.
///
/// All three fields are borrowed, so callers can construct a
/// `VerifyInput` directly from row data without owning copies. The
/// borrowed `&str` for the PEM matches the shape returned by
/// [`crate::signing::SigningKeyStore::export_public_pem`] (after the
/// caller takes a reference).
///
/// ## Phase 14 expectations
///
/// - `payload`: byte-stable canonical JSON for the signed message (see
///   [`crate::canonical_json::canonical_json_bytes`]). The verifier will
///   re-parse this to read `payloadVersion` for dispatch.
/// - `signature`: raw Ed25519 signature bytes (64 bytes for the standard
///   `ed25519-dalek` shape). Callers that hold a hex-encoded signature
///   should `hex::decode` before calling.
/// - `public_key_pem`: PKCS#8 / PEM-encoded Ed25519 verifying key. Phase
///   14 will validate that the embedded
///   [`crate::achievements::CertPayloadV1::key_fingerprint`] matches
///   `SHA-256(DER(public_key))[0..8]` to bind the payload to a trusted
///   signer.
#[derive(Debug, Clone, Copy)]
pub struct VerifyInput<'a> {
    /// Byte-stable canonical JSON of the signed payload.
    pub payload: &'a [u8],

    /// Raw Ed25519 signature bytes (typically 64 bytes).
    pub signature: &'a [u8],

    /// PKCS#8 / PEM-encoded Ed25519 verifying key.
    pub public_key_pem: &'a str,
}

/// Result of a verification attempt.
///
/// Phase 7 only emits `not_implemented` results. Phase 14 will populate
/// `valid = true` for well-signed payloads and `valid = false` with a
/// descriptive `error` for bad signatures / unknown public keys / replayed
/// nonces.
///
/// Serialization is `camelCase` so the struct flows unchanged across the
/// Tauri IPC boundary and any future wasm-bindgen JS bindings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResult {
    /// Whether the payload's signature, public key, and structural
    /// invariants all hold.
    pub valid: bool,

    /// Human-readable error string. `None` iff `valid == true`.
    pub error: Option<String>,

    /// Payload schema version the verifier dispatched on. `0` for the
    /// not-implemented stub; `1` for v1 certificate payloads (Phase 14).
    pub payload_version: u32,
}

impl VerifyResult {
    /// Sentinel result emitted by [`verify`] during Phases 7-13.
    ///
    /// Phase 14 replaces the implementation; consumers that match on
    /// `payload_version == 0` can detect the stub explicitly.
    pub fn not_implemented() -> Self {
        Self {
            valid: false,
            error: Some(
                "verifier not implemented in Phase 7; ships in Phase 14".into(),
            ),
            payload_version: 0,
        }
    }
}

/// Verify a signed certificate payload.
///
/// **Phase 7 stub:** always returns [`VerifyResult::not_implemented`],
/// ignoring every field of [`VerifyInput`]. Phase 14 replaces the body
/// with the real Ed25519 signature check + public-key fingerprint
/// resolution + payload-version dispatch; the call shape stays unchanged.
///
/// The signature is byte-stable: a borrowed [`VerifyInput`] → owned
/// [`VerifyResult`]. No async, no I/O, no allocations beyond the result
/// struct's error string. The pre-WR-03 single `&[u8]` parameter could
/// not carry the signature bytes or verifying key without an envelope
/// format; the explicit struct argument removes that constraint.
///
/// # Example
///
/// During Phases 7-13 the stub return value lets consumers wire UI / IPC
/// plumbing today and observe a clear "not implemented" surface at
/// runtime. Phase 14 will fill in the real verification logic; the
/// signature does not change.
///
/// ```
/// use learnforge_core::verifier::{verify, VerifyInput};
///
/// let input = VerifyInput {
///     payload: b"placeholder-canonical-json-payload",
///     signature: &[0u8; 64],
///     public_key_pem: "-----BEGIN PUBLIC KEY-----\nstub\n-----END PUBLIC KEY-----\n",
/// };
/// let result = verify(input);
/// assert!(!result.valid);
/// assert_eq!(result.payload_version, 0);
/// assert!(result.error.is_some());
/// ```
pub fn verify(_input: VerifyInput<'_>) -> VerifyResult {
    VerifyResult::not_implemented()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_input<'a>() -> VerifyInput<'a> {
        VerifyInput {
            payload: b"",
            signature: &[0u8; 64],
            public_key_pem: "-----BEGIN PUBLIC KEY-----\nstub\n-----END PUBLIC KEY-----\n",
        }
    }

    #[test]
    fn not_implemented_payload_version_is_zero() {
        assert_eq!(VerifyResult::not_implemented().payload_version, 0);
    }

    #[test]
    fn not_implemented_is_invalid() {
        assert!(!VerifyResult::not_implemented().valid);
    }

    #[test]
    fn verify_empty_returns_not_implemented() {
        let result = verify(stub_input());
        assert!(!result.valid);
        assert!(result.error.is_some());
        assert_eq!(result.payload_version, 0);
    }

    /// WR-03 — exercise the three-field input shape end-to-end so a
    /// Phase 14 implementor can drop in the real body without renaming
    /// fields. Until then, the stub ignores all inputs and returns
    /// not_implemented unconditionally.
    #[test]
    fn verify_carries_signature_and_public_key_pem() {
        let payload = b"canonical-json-bytes";
        let sig = vec![0u8; 64];
        let pem = "-----BEGIN PUBLIC KEY-----\nstub\n-----END PUBLIC KEY-----\n";
        let input = VerifyInput {
            payload,
            signature: &sig,
            public_key_pem: pem,
        };
        let result = verify(input);
        // Phase-7 invariant: stub stays not_implemented regardless of
        // input shape.
        assert!(!result.valid);
        assert_eq!(result.payload_version, 0);
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("not implemented"));
    }

    #[test]
    fn verify_result_round_trips_through_serde_json() {
        // Lock the camelCase invariant so Phase 14 + the IPC layer can
        // rely on `payloadVersion` (not `payload_version`) on the wire.
        let result = VerifyResult::not_implemented();
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("\"payloadVersion\""));
        let decoded: VerifyResult =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded.payload_version, 0);
        assert!(!decoded.valid);
    }
}
