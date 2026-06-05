// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah, Vivian Aranha

//! License-key validation scaffold.
//!
//! OSS builds ship `LicenseValidator` as a stub that always returns
//! `LicenseResult::Valid { tier: "oss" }`. The Studio overlay
//! (`pro/src-tauri-pro/licensing/`) provides the real Ed25519-signed
//! JWT verification + 7-day offline cache implementation. See
//! RESEARCH.md "JWT License-Key Validator Scaffold" section.

use serde::{Deserialize, Serialize};

/// Outcome of a license-key validation attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum LicenseResult {
    /// OSS or successfully-validated Pro license.
    Valid {
        /// Tier label: `"oss"` for OSS builds, `"team" | "business" | "enterprise"` in Pro.
        tier: String,
    },
    /// Pro-only: signature mismatch, expired, or no key present.
    Invalid {
        reason: String,
    },
}

/// Validates license keys. OSS implementation is a no-op stub that
/// always reports `Valid { tier: "oss" }`. The Studio overlay replaces
/// this with a JWT/Ed25519 implementation.
#[derive(Debug, Clone, Default)]
pub struct LicenseValidator {
    /// OSS doesn't use this — present so the struct shape matches the
    /// Pro overlay's signature. Pro stores the public-key PEM here.
    _placeholder: (),
}

impl LicenseValidator {
    /// Construct an OSS-mode validator. No inputs required.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate a license key. OSS always returns `Valid { tier: "oss" }`.
    /// The `_key` argument is ignored in OSS and reserved for the Pro
    /// overlay where it carries the signed JWT string.
    pub fn validate(&self, _key: Option<&str>) -> Result<LicenseResult, LicenseError> {
        Ok(LicenseResult::Valid {
            tier: "oss".to_string(),
        })
    }
}

/// Errors that can occur during license validation. OSS only ever
/// constructs `Valid` results, but the error type exists so the Pro
/// overlay can return signature/cache failures with the same signature.
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
}
