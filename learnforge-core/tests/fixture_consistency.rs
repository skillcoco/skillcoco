//! Fixture-consistency tests for the committed `pack_trust` fixtures
//! (plan 14-03, Task 2) — asserts each of the four fixture files behaves
//! exactly as documented in `tests/fixtures/pack_trust/README.md` against
//! `pack_trust::verify_pack`, using the ALSO-committed `root-public.pem`
//! trust anchor for these fixtures (independent of the app's own bundled
//! `BUNDLED_ROOT_PUBLIC_PEM` — this is the fixture-specific root that
//! signed these four files).

use learnforge_core::pack_trust::{verify_pack, PackTrustError};

const FIXTURE_ROOT_PEM: &str =
    include_str!("fixtures/pack_trust/root-public.pem");

fn load_fixture(name: &str) -> serde_json::Value {
    let path = format!("tests/fixtures/pack_trust/{name}");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse fixture {path}: {e}"))
}

/// `valid-signed.json` verifies cleanly end-to-end.
#[test]
fn valid_signed_fixture_verifies_ok() {
    let pack = load_fixture("valid-signed.json");
    let result = verify_pack(FIXTURE_ROOT_PEM, &pack);
    assert!(result.is_ok(), "valid-signed.json must verify Ok; got {result:?}");
}

/// `tampered-body.json` (title edited after signing) is rejected as
/// `TamperedPack`.
#[test]
fn tampered_body_fixture_rejected_as_tampered() {
    let pack = load_fixture("tampered-body.json");
    let result = verify_pack(FIXTURE_ROOT_PEM, &pack);
    assert!(
        matches!(result, Err(PackTrustError::TamperedPack)),
        "tampered-body.json must be rejected as TamperedPack; got {result:?}"
    );
}

/// `forged-cert.json` (issuerCert.rootSig NOT signed by the real root) is
/// rejected as `UntrustedIssuer`.
#[test]
fn forged_cert_fixture_rejected_as_untrusted_issuer() {
    let pack = load_fixture("forged-cert.json");
    let result = verify_pack(FIXTURE_ROOT_PEM, &pack);
    assert!(
        matches!(result, Err(PackTrustError::UntrustedIssuer)),
        "forged-cert.json must be rejected as UntrustedIssuer; got {result:?}"
    );
}

/// `stripped-signature.json` (no `signature` key, `licensed:` provenance
/// retained) is rejected as `MissingSignature`.
#[test]
fn stripped_signature_fixture_rejected_as_missing_signature() {
    let pack = load_fixture("stripped-signature.json");
    let result = verify_pack(FIXTURE_ROOT_PEM, &pack);
    assert!(
        matches!(result, Err(PackTrustError::MissingSignature)),
        "stripped-signature.json must be rejected as MissingSignature; got {result:?}"
    );
}

/// D-09 target: `valid-signed.json` and `stripped-signature.json` both
/// carry `exported_from` starting with `licensed:` so 14-04's tier check
/// has a live fixture to test against.
#[test]
fn licensed_provenance_present_on_relevant_fixtures() {
    for name in ["valid-signed.json", "stripped-signature.json"] {
        let pack = load_fixture(name);
        let exported_from = pack
            .get("exportedFrom")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{name} must carry an exportedFrom field"));
        assert!(
            exported_from.starts_with("licensed:"),
            "{name}'s exported_from must start with 'licensed:', got: {exported_from}"
        );
    }
}
