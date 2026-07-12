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

/// Test-only fixture builders shared across entitlements tests (ENT-02).
/// Mirrors the `labs::test_support` precedent (Phase 03.1-01) — a `#[cfg(test)]`
/// module so `pub(crate)` test helpers don't leak into the production API
/// surface, kept next to the code they exercise rather than duplicated per
/// test file.
#[cfg(test)]
pub mod test_support {
    use ed25519_dalek::pkcs8::EncodePublicKey;
    use ed25519_dalek::SigningKey;
    use learnforge_core::pack_trust::{self, IssuerCert};
    use rand::rngs::OsRng;

    /// Generate an Ed25519 keypair, returning (signing key, public PEM).
    /// Mirrors `pack_trust.rs`'s own `#[cfg(test)]` `keypair()` helper — the
    /// Phase 14 keypair generation helper the ENT-02 fixture must reuse
    /// rather than hand-rolling a second crypto path.
    fn keypair() -> (SigningKey, String) {
        let key = SigningKey::generate(&mut OsRng);
        let pem = key
            .verifying_key()
            .to_public_key_pem(pkcs8::LineEnding::LF)
            .expect("PEM-encode test verifying key");
        (key, pem)
    }

    /// A synthetic buyer-stamped, correctly-signed `licensed:`-provenance
    /// pack body: `exported_from` starts with `licensed:`, and the body
    /// carries a "Licensed to {buyer}, order #{order_id}" watermark string
    /// per the entitlement-api-contract.md buyer-stamping description.
    /// Identifiers are JSON strings, never numbers (14-RESEARCH Pitfall 3).
    fn buyer_stamped_pack_body(pack_id: &str, buyer_name: &str, order_id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": pack_id,
            "title": "Redeemed Licensed Pack",
            "description": format!("Licensed to {buyer_name}, order #{order_id}"),
            "domain_module": "devops",
            "exportedFrom": format!("licensed:{pack_id}|Test Licensor"),
            "orderId": order_id,
            "modules": [
                {
                    "id": "mod-a",
                    "title": "Module A",
                    "description": "First module.",
                    "objectives": ["learn basics"],
                    "difficulty": 1,
                    "estimatedMinutes": 30
                }
            ],
            "edges": [],
            "exportVersion": "1.0.0",
            "exportedAt": "2026-07-12T00:00:00Z",
            "blocks": {},
            "labs": {},
            "videos": {}
        })
    }

    /// Build a fresh root+issuer keypair, sign a synthetic buyer-stamped
    /// `licensed:` pack, and return `(root_pem, signed_pack_json)`. The
    /// signature verifies through the real `pack_trust::verify_pack` path —
    /// NOT a hand-forged blob (T-15-02 mitigation) — using a freshly
    /// generated test root, mirroring how `pack_trust.rs` and
    /// `course_io.rs` test signature verification without a real issuer.
    /// The signature is regenerated on every call per the pack_trust test
    /// convention — no static signed file on disk.
    pub fn signed_licensed_pack_fixture(
        pack_id: &str,
        buyer_name: &str,
        order_id: &str,
    ) -> (String, serde_json::Value) {
        let (root_key, root_pem) = keypair();
        let (issuer_key, issuer_pem) = keypair();

        let unsigned_cert = serde_json::json!({
            "issuerId": "test-issuer",
            "name": "Test Issuer",
            "publicKeyPem": issuer_pem,
        });
        let cert_bytes = pack_trust::jcs_bytes(&unsigned_cert).expect("JCS bytes for cert");
        let sig = learnforge_core::signing::sign_payload(&root_key, &cert_bytes);
        let cert = IssuerCert {
            issuer_id: "test-issuer".to_string(),
            name: "Test Issuer".to_string(),
            public_key_pem: issuer_pem,
            root_sig: hex::encode(sig.to_bytes()),
        };

        let body = buyer_stamped_pack_body(pack_id, buyer_name, order_id);
        let body_bytes = pack_trust::jcs_bytes(&body).expect("JCS bytes for pack body");
        let body_sig = learnforge_core::signing::sign_payload(&issuer_key, &body_bytes);
        let mut pack = body;
        pack["signature"] = serde_json::json!({
            "alg": "ed25519",
            "issuerCert": serde_json::to_value(&cert).unwrap(),
            "keyFingerprint": learnforge_core::signing::public_key_fingerprint(&issuer_key.verifying_key()),
            "sig": hex::encode(body_sig.to_bytes()),
        });

        (root_pem, pack)
    }
}

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

    /// ENT-02 — a downloaded, buyer-stamped `licensed:` pack (fixture built
    /// via `test_support::signed_licensed_pack_fixture`, same crypto path as
    /// `pack_trust.rs`'s own tests) verifies through `pack_trust::verify_pack`
    /// end-to-end, preserves its `licensed:` provenance, and carries the
    /// buyer/order watermark in the pack body. RED until 15-02 wires the
    /// real redeem -> download -> `import_course_impl` pipeline; this test
    /// pins the fixture + the assertions 15-02 must satisfy.
    ///
    /// The fixture uses a freshly-generated test root (NOT
    /// `pack_trust::BUNDLED_ROOT_PUBLIC_PEM`, which has no committed private
    /// key in this repo — offline by design). 15-02's real integration test
    /// exercises the full `import_course_impl` path once the redeem flow
    /// can inject/download a pack signed by the production root; this Wave 0
    /// scaffold proves the fixture-generation + verification mechanics work
    /// today, independent of that wiring.
    #[test]
    fn redeem_downloaded_licensed_pack_imports_with_provenance_preserved() {
        let (root_pem, pack) =
            test_support::signed_licensed_pack_fixture("pack-ent-02", "Jane Buyer", "ORD-9001");

        // Fixture must verify through the REAL pack_trust chain-of-trust path.
        let verify_result = learnforge_core::pack_trust::verify_pack(&root_pem, &pack);
        assert!(
            verify_result.is_ok(),
            "ENT-02 fixture must verify via pack_trust::verify_pack; got {verify_result:?}"
        );

        // `licensed:` provenance preserved verbatim (D-11) — this is the
        // acceptance target for 15-02's redeem-download-import wiring, not
        // yet asserted end-to-end through import_course_impl here.
        let exported_from = pack["exportedFrom"].as_str().unwrap_or_default();
        assert!(
            exported_from.starts_with("licensed:"),
            "15-02: buyer-stamped pack must preserve licensed: provenance; got {exported_from}"
        );

        // Buyer/order watermark lands in the pack BODY (covered by the
        // signature per D-01/D-03/D-04), not metadata alongside it.
        let description = pack["description"].as_str().unwrap_or_default();
        assert!(
            description.contains("Licensed to Jane Buyer, order #ORD-9001"),
            "15-02: pack body must carry the buyer/order watermark string; got {description}"
        );

        // The real redeem -> download -> import_course_impl pipeline is
        // integration-tested in `commands::entitlements_tests` (gate invoked
        // via download_and_import_pack_impl, fail-closed no-signature
        // rejection, and the provenance/export invariant below). A successful
        // full import of THIS fixture is impossible by design: it is signed
        // by a fresh test root, and import_course_impl only trusts the
        // bundled production root (no production private key in this repo).
        // This test remains the permanent ENT-02 fixture + provenance +
        // watermark pin; close it with the same fail-closed export assertion
        // course_io.rs enforces.
        assert!(
            !crate::commands::course_io::is_course_exportable(exported_from),
            "licensed: provenance must remain non-exportable (ENT-02/D-10) — export stays blocked"
        );
    }
}
