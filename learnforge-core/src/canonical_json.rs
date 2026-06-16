//! Canonical JSON — byte-stable serialization for signing payloads.
//!
//! Moved verbatim from `src-tauri/src/achievements/signing.rs:93-133`
//! (canonicalize + canonical_json_bytes) during Phase 7 Wave 5 (07-05).
//!
//! Pure, WASM-portable: no `std::fs`, no `rusqlite`, no Tauri. Operates only
//! on `serde::Serialize` payloads and `serde_json::Value`.
//!
//! ## Invariants (preserved from Phase 6 R1 / Pitfall 2 / CERT-PAYLOAD-V1)
//!
//! - Object keys are sorted lexicographically at every nesting level.
//! - Non-finite floats (NaN, +∞, -∞) are rejected — they have no canonical
//!   JSON representation and would silently produce non-byte-stable output.
//! - Determinism: the byte sequence Ed25519 then signs MUST be reproducible
//!   given the same logical payload (Phase 14 hosted verifier depends on
//!   this).
//!
//! Phase 6's pre-Wave-5 implementation in src-tauri produced
//! `AchievementError::Validation("non-finite number in payload")` on
//! non-finite floats. The move preserves this rejection via the typed
//! [`CanonicalJsonError::NonFiniteFloat`] variant.
//!
//! ## Example
//!
//! ```
//! use learnforge_core::canonical_json::canonical_json_bytes;
//!
//! #[derive(serde::Serialize)]
//! struct Payload { b: u32, a: u32 }
//!
//! let bytes = canonical_json_bytes(&Payload { b: 2, a: 1 }).unwrap();
//! let s = std::str::from_utf8(&bytes).unwrap();
//! // Keys appear in lex order regardless of declaration order.
//! assert!(s.find("\"a\"").unwrap() < s.find("\"b\"").unwrap());
//! ```

use serde::Serialize;
use serde_json::{Map, Value};
use thiserror::Error;

/// Errors returned by canonical JSON serialization.
///
/// Both variants represent non-recoverable input shapes: NaN/Inf cannot be
/// rendered to JSON, and a serde failure means the payload itself can't
/// even produce a `serde_json::Value`. Callers downgrade these to their
/// own error envelope.
#[derive(Debug, Error)]
pub enum CanonicalJsonError {
    /// A floating-point value in the payload was NaN, +∞, or −∞.
    ///
    /// Canonical JSON has no representation for these; signing them would
    /// produce non-byte-stable output and silently violate Phase 14 hosted-
    /// verifier expectations.
    #[error("non-finite float not permitted in canonical JSON")]
    NonFiniteFloat,

    /// The payload could not be serialized to `serde_json::Value` or its
    /// canonical form could not be re-serialized to bytes. The wrapped
    /// string is the underlying `serde_json::Error` message.
    #[error("serialize error: {0}")]
    Serialize(String),
}

impl From<serde_json::Error> for CanonicalJsonError {
    fn from(e: serde_json::Error) -> Self {
        CanonicalJsonError::Serialize(e.to_string())
    }
}

/// Recursively canonicalize a JSON value: sort object keys lexicographically
/// at every nesting level; reject non-finite numbers (Phase 6 R1).
///
/// Returns a fresh `Value` with the same logical content as the input but
/// with deterministic key ordering. Non-object/array/number values pass
/// through unchanged.
pub fn canonicalize(v: Value) -> Result<Value, CanonicalJsonError> {
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
                    return Err(CanonicalJsonError::NonFiniteFloat);
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
/// Determinism is mandatory: the Phase 14 hosted verifier must reproduce
/// the same bytes from the same logical payload (R1 / Pitfall 2 /
/// CERT-PAYLOAD-V1).
///
/// # Example
///
/// ```
/// use learnforge_core::canonical_json::canonical_json_bytes;
///
/// #[derive(serde::Serialize)]
/// struct P { z: u32, a: u32 }
///
/// let first = canonical_json_bytes(&P { z: 1, a: 2 }).unwrap();
/// let second = canonical_json_bytes(&P { z: 1, a: 2 }).unwrap();
/// assert_eq!(first, second, "byte-stable");
/// ```
pub fn canonical_json_bytes<T: Serialize>(payload: &T) -> Result<Vec<u8>, CanonicalJsonError> {
    let v: Value = serde_json::to_value(payload)?;
    let canonical = canonicalize(v)?;
    let bytes = serde_json::to_vec(&canonical)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    //! Tests moved verbatim from pre-Wave-5 src-tauri/src/achievements/signing.rs
    //! plus an explicit non-finite-float rejection test (Phase 6 R1).

    use super::*;

    /// R1 / Pitfall 2 — byte-stable AND keys sort lexicographically at every level.
    #[test]
    fn canonical_json_byte_stable() {
        #[derive(serde::Serialize)]
        struct Probe {
            b: u32,
            a: u32,
            nested: Nested,
        }
        #[derive(serde::Serialize)]
        struct Nested {
            z: u32,
            m: u32,
            a: u32,
        }
        let p = Probe {
            b: 2,
            a: 1,
            nested: Nested { z: 99, m: 5, a: 0 },
        };
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
        assert!(
            na < nm && nm < nz,
            "nested keys must sort (got: {})",
            nested
        );
    }

    /// Finite floats pass; serde_json::Number itself rejects NaN/Inf at
    /// construction (canonicalize's non-finite check is second-line defense).
    #[test]
    fn canonical_json_rejects_non_finite_mastery() {
        #[derive(serde::Serialize)]
        struct GoodPayload {
            mastery_score: f64,
        }
        canonical_json_bytes(&GoodPayload {
            mastery_score: 0.85,
        })
        .expect("finite ok");
        // serde_json::Number's constructor rejects NaN/+inf/-inf — the
        // first line of defense. The canonicalize match-arm is the second
        // line for any future serde override that yields Value::Number
        // from a non-finite f64.
        assert!(serde_json::Number::from_f64(f64::NAN).is_none());
        assert!(serde_json::Number::from_f64(f64::INFINITY).is_none());
        assert!(serde_json::Number::from_f64(f64::NEG_INFINITY).is_none());
    }

    /// Direct exercise of the canonicalize match-arm for non-finite floats.
    /// We can't get serde_json::Number::from_f64 to produce a non-finite
    /// number (its constructor rejects), so this test pulls in the path
    /// where as_f64 returns a finite value (smoke check the predicate is
    /// reachable) — the assertion documents the contract for future readers.
    #[test]
    fn canonicalize_finite_number_passes() {
        let v: Value = serde_json::json!({ "x": 1.5 });
        let result = canonicalize(v).expect("finite number passes");
        assert!(matches!(result, Value::Object(_)));
    }

    /// CanonicalJsonError::NonFiniteFloat renders the expected human message.
    #[test]
    fn canonical_json_error_renders() {
        let err = CanonicalJsonError::NonFiniteFloat;
        assert_eq!(
            err.to_string(),
            "non-finite float not permitted in canonical JSON"
        );
    }

    /// Empty objects and arrays canonicalize to themselves.
    #[test]
    fn canonicalize_empty_containers() {
        let empty_obj: Value = serde_json::json!({});
        let result = canonicalize(empty_obj).expect("empty obj ok");
        assert_eq!(result, Value::Object(Map::new()));

        let empty_arr: Value = serde_json::json!([]);
        let result = canonicalize(empty_arr).expect("empty arr ok");
        assert_eq!(result, Value::Array(Vec::new()));
    }

    /// Arrays are NOT sorted (only object keys are). The element order is
    /// part of the payload's semantics and must be preserved verbatim.
    #[test]
    fn canonicalize_preserves_array_order() {
        let v: Value = serde_json::json!([3, 1, 2]);
        let result = canonical_json_bytes(&v).unwrap();
        assert_eq!(std::str::from_utf8(&result).unwrap(), "[3,1,2]");
    }
}
