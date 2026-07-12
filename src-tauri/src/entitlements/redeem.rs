// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)

//! Redeem-license request/response wire shapes + Hub POST call.
//!
//! Wave 0 (15-01): `call_redeem_endpoint` was a compiling `unimplemented!`
//! stub. 15-03 fills the body following the `submit_evidence_report_impl`
//! SSRF-scheme-check + typed-error-mapping pattern
//! (`src-tauri/src/commands/reports.rs` lines 480-546, 15-PATTERNS.md).
//! No DB access here — this is a pure service fn composed by the Wave 2 IPC
//! layer.
//!
//! Wire shapes are camelCase over IPC/HTTP per the authoritative contract
//! (`.planning/notes/entitlement-api-contract.md` "Redeem request/response").

use serde::{Deserialize, Serialize};

use super::RedeemLicenseError;

/// Small typed error body the Hub returns on non-2xx `/v1/entitlements/redeem`
/// responses. Tolerates both `error` and `code` field names defensively
/// (the contract doc specifies a typed taxonomy but doesn't pin the exact
/// JSON key) — never used for substring matching on free text.
#[derive(Debug, Clone, Deserialize)]
struct RedeemErrorBody {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    code: Option<String>,
}

/// Pure status+body -> typed-error mapping, factored out of the network call
/// so behavior tests can drive it directly with fixture JSON strings
/// (mirrors how `reports.rs` isolates its parse/map logic). NEVER branches
/// on HTTP status alone and NEVER substring-searches the body text — only
/// the parsed error-code field decides the variant (T-15-09).
fn map_redeem_error(body: &str) -> RedeemLicenseError {
    let parsed: Result<RedeemErrorBody, _> = serde_json::from_str(body);
    let code = match parsed {
        Ok(b) => b.error.or(b.code),
        Err(_) => None,
    };
    match code.as_deref() {
        Some("invalid_key") => RedeemLicenseError::InvalidKey,
        Some("already_redeemed") => RedeemLicenseError::AlreadyRedeemed,
        Some("revoked") => RedeemLicenseError::Revoked,
        Some("issuer_unreachable") => RedeemLicenseError::IssuerUnreachable,
        Some(other) => RedeemLicenseError::MalformedResponse(format!(
            "unrecognized redeem error code: {other}"
        )),
        None => RedeemLicenseError::MalformedResponse(
            "redeem error response missing a recognized error-code field".to_string(),
        ),
    }
}

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
/// download URL).
///
/// The raw license key exists only as part of `request`'s JSON body — it is
/// never logged or embedded in any error message (D-04 / T-15 hygiene).
pub async fn call_redeem_endpoint(
    hub_base_url: &str,
    request: &RedeemLicenseRequest,
) -> Result<RedeemLicenseResult, RedeemLicenseError> {
    // T-18-19-style SSRF hygiene — reject non-http(s) schemes BEFORE any
    // request is attempted.
    let is_http_scheme = hub_base_url.starts_with("http://") || hub_base_url.starts_with("https://");
    if !is_http_scheme {
        return Err(RedeemLicenseError::IssuerUnreachable);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| RedeemLicenseError::IssuerUnreachable)?;

    let endpoint = format!(
        "{}/v1/entitlements/redeem",
        hub_base_url.trim_end_matches('/')
    );

    let send_result = client.post(&endpoint).json(request).send().await;

    match send_result {
        Ok(resp) if resp.status().is_success() => {
            let body = resp
                .text()
                .await
                .map_err(|e| RedeemLicenseError::MalformedResponse(e.to_string()))?;
            serde_json::from_str::<RedeemLicenseResult>(&body)
                .map_err(|e| RedeemLicenseError::MalformedResponse(e.to_string()))
        }
        Ok(resp) => {
            let body = resp.text().await.unwrap_or_default();
            Err(map_redeem_error(&body))
        }
        // Network-level failure (timeout, DNS, refused, etc.) — always maps
        // to IssuerUnreachable regardless of the underlying reqwest error
        // kind.
        Err(_) => Err(RedeemLicenseError::IssuerUnreachable),
    }
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
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["licenseKey"].as_str(), Some("ABCD-1234"), "got: {json}");
        assert_eq!(value["deviceFingerprint"].as_str(), Some("fp-1"), "got: {json}");
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
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["packId"].as_str(), Some("pack-1"), "got: {json}");
        assert!(value.get("downloadUrl").is_some(), "got: {json}");
        assert!(value.get("redeemedAt").is_some(), "got: {json}");
    }

    /// redeem_success_parses_all_response_fields — a mocked 200 body with
    /// all 7 fields deserializes into RedeemLicenseResult with each field
    /// populated. Drives the same `serde_json::from_str` seam
    /// `call_redeem_endpoint` uses on success, without a live Hub.
    #[test]
    fn redeem_success_parses_all_response_fields() {
        let body = serde_json::json!({
            "packId": "pack-1",
            "issuerId": "issuer-1",
            "issuerName": "Test Issuer",
            "buyerName": "Jane Buyer",
            "orderId": "ORD-1",
            "downloadUrl": "https://hub.example.org/download/1",
            "redeemedAt": "2026-07-12T00:00:00Z",
        })
        .to_string();

        let result: RedeemLicenseResult = serde_json::from_str(&body).unwrap();
        assert_eq!(result.pack_id, "pack-1");
        assert_eq!(result.issuer_id, "issuer-1");
        assert_eq!(result.issuer_name, "Test Issuer");
        assert_eq!(result.buyer_name, "Jane Buyer");
        assert_eq!(result.order_id, "ORD-1");
        assert_eq!(result.download_url, "https://hub.example.org/download/1");
        assert_eq!(result.redeemed_at, "2026-07-12T00:00:00Z");
    }

    /// redeem_invalid_key_maps_typed — error-code "invalid_key" maps to
    /// RedeemLicenseError::InvalidKey via the parsed code field.
    #[test]
    fn redeem_invalid_key_maps_typed() {
        let body = serde_json::json!({ "error": "invalid_key" }).to_string();
        assert!(matches!(
            map_redeem_error(&body),
            RedeemLicenseError::InvalidKey
        ));
    }

    /// redeem_already_redeemed_maps_typed — error-code "already_redeemed" ->
    /// AlreadyRedeemed.
    #[test]
    fn redeem_already_redeemed_maps_typed() {
        let body = serde_json::json!({ "error": "already_redeemed" }).to_string();
        assert!(matches!(
            map_redeem_error(&body),
            RedeemLicenseError::AlreadyRedeemed
        ));
    }

    /// redeem_revoked_maps_typed — error-code "revoked" -> Revoked.
    #[test]
    fn redeem_revoked_maps_typed() {
        let body = serde_json::json!({ "error": "revoked" }).to_string();
        assert!(matches!(
            map_redeem_error(&body),
            RedeemLicenseError::Revoked
        ));
    }

    /// redeem_unknown_error_code_falls_back_to_malformed — an unrecognized
    /// error-code maps to MalformedResponse, never a panic.
    #[test]
    fn redeem_unknown_error_code_falls_back_to_malformed() {
        let body = serde_json::json!({ "error": "some_new_code_from_the_future" }).to_string();
        assert!(matches!(
            map_redeem_error(&body),
            RedeemLicenseError::MalformedResponse(_)
        ));
    }

    /// redeem_never_string_matches_body — the mapping is a structural match
    /// on the parsed code field, never a substring search on free text. A
    /// body whose free-text mentions "already redeemed" but whose code
    /// field is "invalid_key" must map to InvalidKey, proving code-field
    /// precedence over any body text (T-15-09).
    #[test]
    fn redeem_never_string_matches_body() {
        let body = serde_json::json!({
            "error": "invalid_key",
            "message": "This key has already been redeemed by another buyer.",
        })
        .to_string();
        assert!(matches!(
            map_redeem_error(&body),
            RedeemLicenseError::InvalidKey
        ));
    }
}
