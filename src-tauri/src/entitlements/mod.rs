// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)

//! Phase 15 (Entitlement & Redeem) — license-key redeem, buyer-stamped pack
//! download, and local entitlement caching.
//!
//! Wave 0 (15-01): compiling-but-RED scaffolds only. Every fallible path
//! below is `unimplemented!("15-02")` / `unimplemented!("15-03")` until the
//! resolving plan lands real logic — see each submodule's doc comment.
//!
//! ## Typed error taxonomy (D-04)
//!
//! [`RedeemLicenseError`] mirrors the `ImportCourseError`/`PackTrustError`
//! discipline established in Phase 12/14
//! (`src-tauri/src/commands/course_io.rs`): every Hub-supplied error code and
//! every local failure maps to a distinct variant, never a string-matched
//! message. The `#[error(...)]` text on each variant IS the literal UI copy
//! (D-04 Copywriting Contract, `15-UI-SPEC.md`) — a raw/leaky message here
//! would surface directly in the redeem UI (T-15 scaffold error strings -> UI
//! trust boundary).

pub mod download;
pub mod fingerprint;
pub mod redeem;

/// Typed errors for the redeem-license flow (D-04). Every variant's
/// `#[error(...)]` string is the exact plain-language copy rendered inline
/// under the license-key field in `RedeemLicenseFlow` (15-UI-SPEC.md
/// Copywriting Contract) — never a raw/technical message.
#[derive(Debug, thiserror::Error)]
pub enum RedeemLicenseError {
    /// The Hub rejected the key as invalid (typo, unknown key).
    #[error("This license key isn't valid. Check for typos and try again.")]
    InvalidKey,
    /// The key has already been redeemed (single-use per the contract).
    #[error("This license key has already been redeemed.")]
    AlreadyRedeemed,
    /// The key was revoked by the issuer (refund, chargeback, etc).
    #[error("This license key has been revoked.")]
    Revoked,
    /// Network failure or non-2xx response reaching the Hub `/v1/entitlements/redeem`
    /// endpoint. Distinct from the typed Hub error-code variants above — this
    /// is a transport-layer failure, not a Hub-adjudicated rejection. Gets a
    /// Retry button in the UI (D-04).
    #[error("Couldn't reach the license server. Check your connection and try again.")]
    IssuerUnreachable,
    /// The Hub responded 200 but the response body didn't match the expected
    /// `RedeemLicenseResult` shape, or a non-2xx response carried an
    /// unrecognized error code. Technical detail stays in the field for
    /// logs, not the primary message.
    #[error("Redeem request failed: {0}")]
    MalformedResponse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D-04 — every RedeemLicenseError variant renders its exact plain-language
    /// copy from the 15-UI-SPEC.md Copywriting Contract. This is the acceptance
    /// target for the enum skeleton (no real redeem logic needed for this test
    /// to pass — it exercises `Display` on the enum directly).
    #[test]
    fn redeem_error_variants_render_plain_language() {
        assert_eq!(
            RedeemLicenseError::InvalidKey.to_string(),
            "This license key isn't valid. Check for typos and try again."
        );
        assert_eq!(
            RedeemLicenseError::AlreadyRedeemed.to_string(),
            "This license key has already been redeemed."
        );
        assert_eq!(
            RedeemLicenseError::Revoked.to_string(),
            "This license key has been revoked."
        );
        assert_eq!(
            RedeemLicenseError::IssuerUnreachable.to_string(),
            "Couldn't reach the license server. Check your connection and try again."
        );
        assert_eq!(
            RedeemLicenseError::MalformedResponse("boom".to_string()).to_string(),
            "Redeem request failed: boom"
        );
    }
}
