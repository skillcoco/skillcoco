// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)

//! License-key validation seam.
//!
//! `LicenseValidator` is a stub that always returns
//! `LicenseResult::Valid { tier: "oss" }`. Phase 15 (Entitlement &
//! Redeem) fills this seam with real JWT verification and an offline
//! validation cache. The type shapes below (`LicenseClaims`,
//! `CachedValidation`) are the contracts that implementation will use.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The claims embedded in a signed license JWT.
#[derive(Debug, Serialize, Deserialize)]
pub struct LicenseClaims {
    /// Subscriber organization identifier.
    pub sub: String,
    /// License tier: "team" | "business" | "enterprise"
    pub tier: String,
    /// Number of named seats.
    pub seats: u32,
    /// Unix timestamp: license expiry.
    pub exp: i64,
    /// Unix timestamp: issued-at.
    pub iat: i64,
}

/// Cached validation result stored locally for offline grace period.
#[derive(Debug, Serialize, Deserialize)]
pub struct CachedValidation {
    pub claims: LicenseClaims,
    /// Wall time when this cache entry expires (now + 7 days).
    pub cache_until: DateTime<Utc>,
    /// The raw license key that produced this result.
    pub license_key_fingerprint: String,
}

/// Outcome of a license-key validation attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum LicenseResult {
    /// Default OSS result, or a successfully-validated license (Phase 15).
    Valid {
        /// Tier label: `"oss"` today; `"team" | "business" | "enterprise"`
        /// once entitlement validation lands in Phase 15.
        tier: String,
    },
    /// Signature mismatch, expired, or no key present. Unused until
    /// Phase 15 wires real validation.
    Invalid {
        reason: String,
    },
}

/// Validates license keys. Currently a no-op stub that always reports
/// `Valid { tier: "oss" }`. Phase 15 replaces the body with real
/// JWT-based entitlement validation.
#[derive(Debug, Clone, Default)]
pub struct LicenseValidator {
    /// Unused today — reserved so the struct shape stays stable when
    /// Phase 15 adds validation state (e.g. a public-key PEM).
    _placeholder: (),
}

impl LicenseValidator {
    /// Construct a validator. No inputs required until Phase 15.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate a license key. Always returns `Valid { tier: "oss" }`
    /// today. The `_key` argument is reserved for Phase 15, where it
    /// carries the signed JWT string.
    pub fn validate(&self, _key: Option<&str>) -> Result<LicenseResult, LicenseError> {
        Ok(LicenseResult::Valid {
            tier: "oss".to_string(),
        })
    }
}

/// Errors that can occur during license validation. The stub only ever
/// constructs `Valid` results, but the error type exists so Phase 15
/// can return signature/cache failures with the same signature.
#[derive(Debug, thiserror::Error)]
pub enum LicenseError {
    #[error("invalid license signature")]
    InvalidSignature,
    #[error("license expired")]
    Expired,
    #[error("license cache expired — reconnect to revalidate")]
    CacheExpired,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JWT decode error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oss_validator_returns_valid() {
        let v = LicenseValidator::new();
        match v.validate(None).expect("OSS validate must not error") {
            LicenseResult::Valid { tier } => assert_eq!(tier, "oss"),
            LicenseResult::Invalid { reason } => {
                panic!("expected Valid, got Invalid({reason})")
            }
        }
    }

    #[test]
    fn license_result_is_serializable() {
        let r = LicenseResult::Valid {
            tier: "team".to_string(),
        };
        let json = serde_json::to_string(&r).unwrap();
        // tag=kind, rename_all=camelCase => `{"kind":"valid","tier":"team"}`
        assert!(
            json.contains("\"kind\":\"valid\""),
            "expected camelCase 'kind' tag, got: {json}"
        );
        assert!(
            json.contains("\"tier\":\"team\""),
            "expected tier field, got: {json}"
        );
        let back: LicenseResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn license_claims_is_serializable() {
        let claims = LicenseClaims {
            sub: "org-123".to_string(),
            tier: "team".to_string(),
            seats: 5,
            exp: 9999999999,
            iat: 1700000000,
        };
        let json = serde_json::to_string(&claims).unwrap();
        assert!(
            json.contains("\"sub\":\"org-123\""),
            "expected sub field, got: {json}"
        );
        let back: LicenseClaims = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tier, "team");
    }
}
