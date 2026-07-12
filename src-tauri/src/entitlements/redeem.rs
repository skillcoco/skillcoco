// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)

//! Redeem-license request/response wire shapes + Hub POST call.
//!
//! Wave 0 (15-01): `call_redeem_endpoint` is a compiling `unimplemented!`
//! stub. 15-02 fills the body following the `submit_evidence_report_impl`
//! DB-lock-never-held-across-await + SSRF-scheme-check pattern
//! (`src-tauri/src/commands/reports.rs` lines 480-546, 15-PATTERNS.md).
//!
//! Wire shapes are camelCase over IPC/HTTP per the authoritative contract
//! (`.planning/notes/entitlement-api-contract.md` "Redeem request/response").

use serde::{Deserialize, Serialize};

use super::RedeemLicenseError;

/// `POST /v1/entitlements/redeem` request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemLicenseRequest {
    /// Buyer-entered key, opaque to the app.
    pub license_key: String,
    /// Analytics/abuse-signal only — NOT a DRM binding (D-16).
    pub device_fingerprint: String,
}

/// `POST /v1/entitlements/redeem` 200 response body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemLicenseResult {
    pub pack_id: String,
    pub issuer_id: String,
    pub issuer_name: String,
    pub buyer_name: String,
    pub order_id: String,
    /// Short-lived, single-use signed URL — fetched only after staged-confirm
    /// (D-03).
    pub download_url: String,
    /// ISO 8601 timestamp.
    pub redeemed_at: String,
}

/// Validate `request.license_key` against the issuer's Hub endpoint and
/// return the redeem result (pack id, issuer/buyer attribution, single-use
/// download URL). 15-02 implements the real HTTP call; this Wave 0 stub is a
/// named pending assertion.
pub async fn call_redeem_endpoint(
    _hub_base_url: &str,
    _request: &RedeemLicenseRequest,
) -> Result<RedeemLicenseResult, RedeemLicenseError> {
    unimplemented!("15-02")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wire-shape sanity: RedeemLicenseRequest round-trips through
    /// camelCase JSON matching the contract doc's literal field names.
    #[test]
    fn redeem_license_request_serializes_camel_case() {
        let req = RedeemLicenseRequest {
            license_key: "ABCD-1234".to_string(),
            device_fingerprint: "fp-1".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"licenseKey\":\"ABCD-1234\""), "got: {json}");
        assert!(
            json.contains("\"deviceFingerprint\":\"fp-1\""),
            "got: {json}"
        );
    }

    /// Wire-shape sanity: RedeemLicenseResult round-trips through camelCase
    /// JSON matching the contract doc's literal field names.
    #[test]
    fn redeem_license_result_serializes_camel_case() {
        let result = RedeemLicenseResult {
            pack_id: "pack-1".to_string(),
            issuer_id: "issuer-1".to_string(),
            issuer_name: "Test Issuer".to_string(),
            buyer_name: "Jane Buyer".to_string(),
            order_id: "ORD-1".to_string(),
            download_url: "https://hub.example.org/download/1".to_string(),
            redeemed_at: "2026-07-12T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"packId\":\"pack-1\""), "got: {json}");
        assert!(json.contains("\"downloadUrl\""), "got: {json}");
        assert!(json.contains("\"redeemedAt\""), "got: {json}");
    }
}
