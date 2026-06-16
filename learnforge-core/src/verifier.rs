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
//! pub struct VerifyResult {
//!     pub valid: bool,
//!     pub error: Option<String>,
//!     pub payload_version: u32,
//! }
//! ```
//!
//! `payload_version` is a `u32` so Phase 14 can dispatch on the
//! certificate-payload schema version (e.g. v1, v2) without breaking the
//! struct layout.
//!
//! See `docs/CERT-PAYLOAD-V1.md` in the repo root for the payload format.

use serde::{Deserialize, Serialize};

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
/// **Phase 7 stub:** always returns [`VerifyResult::not_implemented`].
/// Phase 14 replaces the body with the real Ed25519 signature check +
/// public-key fingerprint resolution + payload-version dispatch.
///
/// The signature is byte-stable: `&[u8]` (canonical JSON bytes) → owned
/// [`VerifyResult`]. No async, no I/O, no allocations beyond the result
/// struct's error string.
pub fn verify(_payload: &[u8]) -> VerifyResult {
    VerifyResult::not_implemented()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let result = verify(&[]);
        assert!(!result.valid);
        assert!(result.error.is_some());
        assert_eq!(result.payload_version, 0);
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
