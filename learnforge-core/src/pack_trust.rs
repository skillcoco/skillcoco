//! Pack-signing chain of trust: RFC 8785 (JCS) canonicalization + Ed25519
//! root → issuer-cert → pack verification (Phase 14).
//!
//! PURE / WASM-CLEAN (D-08): no filesystem, no network, no clock. Every
//! function here is a pure function of its inputs so the exact same
//! verification path runs on desktop and (later) wasm32. No `std::fs`, no
//! `std::path::Path`, no Tauri imports — the surface uses only `&str`,
//! `serde_json::Value`, and typed structs (same discipline as
//! [`crate::signing`], T-07-14).
//!
//! Canonicalization: this module uses `serde_json_canonicalizer` (RFC 8785
//! JSON Canonicalization Scheme) — NOT `crate::canonical_json::canonicalize`,
//! which is the Phase-6 achievement-payload canonicalizer (lexicographic key
//! sort only, not JCS-spec number formatting / string escaping). Mixing the
//! two silently breaks cross-implementation signature interop (14-RESEARCH
//! Pitfall 2).
//!
//! ## Never-panic contract (T-14-01)
//!
//! Verification operates on attacker-controlled input (imported pack files).
//! Once implemented (plan 14-02), every fallible step — cert parse, sig-block
//! field extraction, hex/PEM decode, canonicalization — MUST return a typed
//! [`PackTrustError`], never `panic!`/`unwrap()`, mirroring the
//! [`crate::signing::verify_payload`] discipline (returns `false` on any
//! decode failure). The `unimplemented!()` bodies below are RED-scaffold
//! stubs only (14-01 Wave 0) and are exercised exclusively by tests.
//!
//! ## Signature-block field trust rule (D-04)
//!
//! Fields inside the top-level `signature` object (`alg`, `issuerCert`,
//! `keyFingerprint`, `sig`) are NEVER covered by the pack signature and must
//! never be treated as security-relevant data by verification code. Anything
//! security-relevant (`signedAt`, buyer stamp, order id — always JSON
//! *strings*, never numbers, per 14-RESEARCH Pitfall 3) belongs in the pack
//! BODY, which IS covered.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// The bundled root PUBLIC-key PEM — the single canonical trust anchor
/// (D-06, D-08 no-drift) shared by `forge-sign` (14-03), Creator Studio, and
/// the app import gate (14-04 Step 3.5). No component embeds its own copy
/// of the root PEM; every verifier references this constant.
///
/// PLACEHOLDER: this is a freshly-generated Ed25519 public key for Phase 14
/// development. The real production root PEM (with its private half held
/// offline) is dropped in before release.
pub const BUNDLED_ROOT_PUBLIC_PEM: &str = include_str!("../keys/root_public.pem");

/// Typed error taxonomy for pack-trust verification (D-11).
///
/// The first three variants map 1:1 to the three plain-language rejection
/// messages the import UI must distinguish: tampered pack, untrusted
/// publisher, missing required signature. A NEW enum (not a reuse of
/// [`crate::signing::SigningError`]) per 14-RESEARCH Pitfall 5 — callers
/// match on variants, never string-match on messages.
#[derive(Debug, Error)]
pub enum PackTrustError {
    /// The pack body does not match its signature — modified after signing.
    #[error("This pack was modified after it was signed, so it can't be trusted. Re-download it from the original source.")]
    TamperedPack,

    /// The issuer certificate is not signed by the app's trusted root key.
    #[error("This pack's publisher isn't recognized by LearnForge, so the pack can't be verified.")]
    UntrustedIssuer,

    /// The pack's provenance tier requires a signature but none is present.
    #[error("This pack needs a publisher signature to be imported, but it doesn't have one.")]
    MissingSignature,

    /// The embedded issuer certificate could not be parsed.
    #[error("The pack's publisher certificate is malformed: {0}")]
    MalformedCert(String),

    /// The signature block is present but structurally invalid (bad hex, missing fields).
    #[error("The pack's signature data is malformed and can't be checked.")]
    MalformedSignature,

    /// The pack JSON is not a top-level object.
    #[error("The pack file isn't a valid course pack (not a JSON object).")]
    NotAnObject,

    /// RFC 8785 canonicalization failed.
    #[error("Couldn't canonicalize the pack for verification: {0}")]
    Canonicalize(String),
}

/// Issuer certificate: `{issuerId, name, publicKeyPem, rootSig}` where
/// `rootSig = ed25519(root_key, JCS(cert minus rootSig))` — verified with
/// the SAME strip-then-canonicalize-then-verify path as packs (D-05, one
/// crypto path).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuerCert {
    /// Stable issuer identity (tenant key for Hub; string, never a number).
    pub issuer_id: String,
    /// Human-readable publisher name (rendered as React text child only).
    pub name: String,
    /// PEM-encoded Ed25519 public key that signs packs for this issuer.
    pub public_key_pem: String,
    /// Hex-encoded root signature over JCS(cert minus this field).
    pub root_sig: String,
}

/// Top-level `signature` envelope carried in signed pack JSON (D-04 minimal
/// shape). None of these fields are covered by the pack signature itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureBlock {
    /// Signature algorithm identifier — always `"ed25519"` in Phase 14.
    pub alg: String,
    /// The issuer certificate whose key produced `sig`.
    pub issuer_cert: IssuerCert,
    /// 8-hex-char fingerprint of the issuer signing key (R5/A7 convention).
    pub key_fingerprint: String,
    /// Hex-encoded Ed25519 signature over JCS(pack minus `signature`).
    pub sig: String,
}

/// RFC 8785 (JCS) canonical bytes of a JSON value, via
/// `serde_json_canonicalizer` — deliberately named distinctly from
/// `crate::canonical_json` (14-RESEARCH Pitfall 2).
pub fn jcs_bytes(v: &Value) -> Result<Vec<u8>, PackTrustError> {
    serde_json_canonicalizer::to_vec(v).map_err(|e| PackTrustError::Canonicalize(e.to_string()))
}

/// Strip a named top-level key from a JSON object, then JCS-canonicalize the
/// remainder. Generic over the field name so both pack verification
/// (strips `signature`, D-03/D-04) and issuer-cert verification (strips
/// `rootSig`, D-05) share the exact same strip-then-canonicalize recipe —
/// one crypto path, not two. Never panics on attacker-controlled input: a
/// non-object value is a typed error.
fn strip_field_and_canonicalize(json: &Value, field: &str) -> Result<Vec<u8>, PackTrustError> {
    let obj = json.as_object().ok_or(PackTrustError::NotAnObject)?;
    let mut stripped = obj.clone();
    stripped.remove(field);
    jcs_bytes(&Value::Object(stripped))
}

/// Strip the top-level `signature` key from a pack JSON object, then
/// JCS-canonicalize the remainder (D-03 whole-pack-minus-signature, D-04 —
/// signature-block fields are NOT covered by the sig).
fn strip_and_canonicalize(pack_json: &Value) -> Result<Vec<u8>, PackTrustError> {
    strip_field_and_canonicalize(pack_json, "signature")
}

/// Verify an issuer cert against the root public key: strip `rootSig`,
/// JCS-canonicalize the remainder, verify with `root_pem` — the SAME
/// strip-canonicalize-verify path as packs, parameterized (D-05).
pub fn verify_issuer_cert(root_pem: &str, cert: &IssuerCert) -> Result<(), PackTrustError> {
    let cert_value = serde_json::to_value(cert)
        .map_err(|e| PackTrustError::MalformedCert(e.to_string()))?;
    let bytes = strip_field_and_canonicalize(&cert_value, "rootSig")?;
    let ok = crate::signing::verify_payload(root_pem, &bytes, &cert.root_sig);
    if !ok {
        return Err(PackTrustError::UntrustedIssuer);
    }
    Ok(())
}

/// Verify a pack's full chain of trust: root verifies the embedded issuer
/// cert, then the issuer cert's key verifies the pack signature over
/// JCS(pack minus `signature`) (D-01/D-03/D-07).
pub fn verify_pack(root_pem: &str, pack_json: &Value) -> Result<(), PackTrustError> {
    let obj = pack_json.as_object().ok_or(PackTrustError::NotAnObject)?;
    let sig_block = obj.get("signature").ok_or(PackTrustError::MissingSignature)?;

    let issuer_cert_value = sig_block
        .get("issuerCert")
        .ok_or_else(|| PackTrustError::MalformedCert("missing issuerCert".to_string()))?;
    let issuer_cert: IssuerCert = serde_json::from_value(issuer_cert_value.clone())
        .map_err(|e| PackTrustError::MalformedCert(e.to_string()))?;

    // Step 1 of the chain: root verifies the issuer cert.
    verify_issuer_cert(root_pem, &issuer_cert)?;

    // Step 2 of the chain: the (now-trusted) issuer key verifies the pack body.
    let sig_hex = sig_block
        .get("sig")
        .and_then(|v| v.as_str())
        .ok_or(PackTrustError::MalformedSignature)?;
    let canonical_bytes = strip_and_canonicalize(pack_json)?;
    let ok = crate::signing::verify_payload(&issuer_cert.public_key_pem, &canonical_bytes, sig_hex);
    if !ok {
        return Err(PackTrustError::TamperedPack);
    }
    Ok(())
}

/// Issue a new issuer cert signed by the root key: build `{issuerId, name,
/// publicKeyPem}`, JCS-canonicalize, root-sign, hex-encode into `rootSig`
/// (the sign-side counterpart to [`verify_issuer_cert`] — same crypto path,
/// D-08).
pub fn issue_cert(
    root_key: &ed25519_dalek::SigningKey,
    issuer_id: &str,
    name: &str,
    issuer_public_pem: &str,
) -> Result<IssuerCert, PackTrustError> {
    let unsigned = serde_json::json!({
        "issuerId": issuer_id,
        "name": name,
        "publicKeyPem": issuer_public_pem,
    });
    let bytes = jcs_bytes(&unsigned)?;
    let sig = crate::signing::sign_payload(root_key, &bytes);
    Ok(IssuerCert {
        issuer_id: issuer_id.to_string(),
        name: name.to_string(),
        public_key_pem: issuer_public_pem.to_string(),
        root_sig: hex::encode(sig.to_bytes()),
    })
}

/// Sign a pack body with the issuer key, attaching a full `signature` block
/// (the sign-side counterpart to [`verify_pack`] — same crypto path, D-08).
/// Any pre-existing `signature` key is stripped and replaced.
pub fn sign_pack(
    signing_key: &ed25519_dalek::SigningKey,
    issuer_cert: &IssuerCert,
    pack_json: &Value,
) -> Result<Value, PackTrustError> {
    let obj = pack_json.as_object().ok_or(PackTrustError::NotAnObject)?;
    let mut stripped = obj.clone();
    stripped.remove("signature");
    let stripped_value = Value::Object(stripped.clone());

    let bytes = jcs_bytes(&stripped_value)?;
    let sig = crate::signing::sign_payload(signing_key, &bytes);
    let key_fingerprint = crate::signing::public_key_fingerprint(&signing_key.verifying_key());

    let issuer_cert_value =
        serde_json::to_value(issuer_cert).map_err(|e| PackTrustError::MalformedCert(e.to_string()))?;

    let mut result = stripped;
    result.insert(
        "signature".to_string(),
        serde_json::json!({
            "alg": "ed25519",
            "issuerCert": issuer_cert_value,
            "keyFingerprint": key_fingerprint,
            "sig": hex::encode(sig.to_bytes()),
        }),
    );
    Ok(Value::Object(result))
}

// ── RED tests (Wave 0, plan 14-01) ────────────────────────────────────────
//
// These tests define the Phase 14 verification contract and FAIL (panic on
// `unimplemented!()`) until 14-02 lands the implementations. Do not weaken
// the assertions to make them pass — implement the stubs.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::sign_payload;
    use ed25519_dalek::pkcs8::EncodePublicKey;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    /// Generate an Ed25519 keypair, returning (signing key, public PEM).
    fn keypair() -> (SigningKey, String) {
        let key = SigningKey::generate(&mut OsRng);
        let pem = key
            .verifying_key()
            .to_public_key_pem(pkcs8::LineEnding::LF)
            .expect("PEM-encode test verifying key");
        (key, pem)
    }

    /// Issue an issuer cert signed by `root`: rootSig = ed25519(root,
    /// JCS(cert minus rootSig)) — mirrors what forge-sign issue-cert
    /// (plan 14-03) will produce.
    fn make_cert(root: &SigningKey, issuer_id: &str, issuer_pub_pem: &str) -> IssuerCert {
        let unsigned = serde_json::json!({
            "issuerId": issuer_id,
            "name": format!("Test Issuer {issuer_id}"),
            "publicKeyPem": issuer_pub_pem,
        });
        let bytes = jcs_bytes(&unsigned).expect("JCS bytes for cert");
        let sig = sign_payload(root, &bytes);
        IssuerCert {
            issuer_id: issuer_id.to_string(),
            name: format!("Test Issuer {issuer_id}"),
            public_key_pem: issuer_pub_pem.to_string(),
            root_sig: hex::encode(sig.to_bytes()),
        }
    }

    /// A minimal pack body (no signature block yet). Identifiers are JSON
    /// strings, never numbers (14-RESEARCH Pitfall 3 / resolved Q1).
    fn pack_body() -> serde_json::Value {
        serde_json::json!({
            "id": "pack-trust-test",
            "title": "Signed Pack",
            "exportedFrom": "licensed:pack-trust-test|Test Licensor",
            "orderId": "ORD-000123",
            "modules": [],
        })
    }

    /// Sign `body` with `issuer_key`, attaching a full signature block —
    /// mirrors what forge-sign sign (plan 14-03) will produce.
    fn signed_pack(issuer_key: &SigningKey, cert: &IssuerCert, body: serde_json::Value) -> serde_json::Value {
        let bytes = jcs_bytes(&body).expect("JCS bytes for pack body");
        let sig = sign_payload(issuer_key, &bytes);
        let mut pack = body;
        pack["signature"] = serde_json::json!({
            "alg": "ed25519",
            "issuerCert": serde_json::to_value(cert).unwrap(),
            "keyFingerprint": crate::signing::public_key_fingerprint(&issuer_key.verifying_key()),
            "sig": hex::encode(sig.to_bytes()),
        });
        pack
    }

    /// TRUST-01 — a correctly signed pack verifies end-to-end.
    #[test]
    fn verify_pack_accepts_valid_signature() {
        let (root_key, root_pem) = keypair();
        let (issuer_key, issuer_pem) = keypair();
        let cert = make_cert(&root_key, "issuer-001", &issuer_pem);
        let pack = signed_pack(&issuer_key, &cert, pack_body());

        let result = verify_pack(&root_pem, &pack);
        assert!(result.is_ok(), "valid signed pack must verify; got {result:?}");
    }

    /// TRUST-01 — chain composition: root verifies the cert, cert verifies
    /// the pack; verify_issuer_cert succeeds standalone AND as step 1 of
    /// verify_pack.
    #[test]
    fn verify_issuer_cert_then_pack() {
        let (root_key, root_pem) = keypair();
        let (issuer_key, issuer_pem) = keypair();
        let cert = make_cert(&root_key, "issuer-002", &issuer_pem);

        verify_issuer_cert(&root_pem, &cert).expect("root-signed issuer cert must verify");

        let pack = signed_pack(&issuer_key, &cert, pack_body());
        verify_pack(&root_pem, &pack).expect("pack signed by verified issuer must verify");
    }

    /// TRUST-02 — a brand-new issuer cert (signed by the same root) is
    /// trusted with ZERO app changes: no rebuild, no allowlist edit — the
    /// only trust anchor is the root key.
    #[test]
    fn new_issuer_cert_trusted_without_rebuild() {
        let (root_key, root_pem) = keypair();

        // First issuer works…
        let (issuer_a_key, issuer_a_pem) = keypair();
        let cert_a = make_cert(&root_key, "issuer-a", &issuer_a_pem);
        verify_pack(&root_pem, &signed_pack(&issuer_a_key, &cert_a, pack_body()))
            .expect("issuer A pack must verify");

        // …and a SECOND, never-before-seen issuer minted at runtime works too,
        // purely because its cert chains to the same root.
        let (issuer_b_key, issuer_b_pem) = keypair();
        let cert_b = make_cert(&root_key, "issuer-b-brand-new", &issuer_b_pem);
        verify_pack(&root_pem, &signed_pack(&issuer_b_key, &cert_b, pack_body()))
            .expect("TRUST-02: new issuer cert signed by root must be trusted without rebuild");
    }

    /// TRUST-03 — valid issuer cert but a pack signature that does not match
    /// the body (body edited after signing) must be rejected as TamperedPack.
    #[test]
    fn valid_cert_invalid_pack_sig_rejected() {
        let (root_key, root_pem) = keypair();
        let (issuer_key, issuer_pem) = keypair();
        let cert = make_cert(&root_key, "issuer-003", &issuer_pem);

        let mut pack = signed_pack(&issuer_key, &cert, pack_body());
        // Tamper AFTER signing — any body byte, including provenance.
        pack["title"] = serde_json::json!("Tampered Title");

        let result = verify_pack(&root_pem, &pack);
        assert!(
            matches!(result, Err(PackTrustError::TamperedPack)),
            "tampered body with valid cert must yield TamperedPack; got {result:?}"
        );
    }

    /// D-02 — JCS canonicalization is byte-stable across a parse round-trip:
    /// jcs(v) == jcs(parse(jcs(v))). Includes a string-typed order id so the
    /// byte output is independent of number formatting (resolved Q1).
    #[test]
    fn jcs_round_trip_is_byte_stable() {
        let v = serde_json::json!({
            "zeta": "last-key-first",
            "alpha": {"nested": ["a", "b"], "n": 42},
            "orderId": "ORD-000123",
            "unicode": "café ✓",
        });
        let first = jcs_bytes(&v).expect("JCS bytes");
        let reparsed: serde_json::Value =
            serde_json::from_slice(&first).expect("JCS output must be valid JSON");
        let second = jcs_bytes(&reparsed).expect("JCS bytes of reparsed value");
        assert_eq!(first, second, "JCS must be byte-stable across parse round-trip");
    }

    /// D-11 — the three user-facing rejection classes render three DISTINCT
    /// plain-language messages (no string-matching ambiguity at the IPC/UI
    /// boundary).
    #[test]
    fn pack_trust_error_messages_distinct() {
        let tampered = PackTrustError::TamperedPack.to_string();
        let untrusted = PackTrustError::UntrustedIssuer.to_string();
        let missing = PackTrustError::MissingSignature.to_string();

        assert_ne!(tampered, untrusted, "TamperedPack vs UntrustedIssuer must differ");
        assert_ne!(tampered, missing, "TamperedPack vs MissingSignature must differ");
        assert_ne!(untrusted, missing, "UntrustedIssuer vs MissingSignature must differ");

        for (name, msg) in [("TamperedPack", &tampered), ("UntrustedIssuer", &untrusted), ("MissingSignature", &missing)] {
            assert!(!msg.is_empty(), "{name} message must be non-empty");
            assert!(
                !msg.contains("Error") && !msg.contains("panic"),
                "{name} must be plain language, got: {msg}"
            );
        }
    }

    /// A forged issuer cert — self-signed instead of signed by the real
    /// root — must be rejected as `UntrustedIssuer` (T-14-03 mitigation:
    /// root PEM verifies `cert.rootSig` before the cert's `publicKeyPem` is
    /// ever trusted).
    #[test]
    fn forged_cert_rejected() {
        let (_root_key, root_pem) = keypair();
        let (issuer_key, issuer_pem) = keypair();

        // Attacker self-signs the cert with the issuer key, NOT the root.
        let forged_cert = make_cert(&issuer_key, "issuer-forged", &issuer_pem);

        let result = verify_issuer_cert(&root_pem, &forged_cert);
        assert!(
            matches!(result, Err(PackTrustError::UntrustedIssuer)),
            "self-signed (non-root) cert must be rejected as UntrustedIssuer; got {result:?}"
        );

        // The same forged cert embedded in an otherwise-valid pack signature
        // must also be rejected end-to-end via verify_pack.
        let pack = signed_pack(&issuer_key, &forged_cert, pack_body());
        let pack_result = verify_pack(&root_pem, &pack);
        assert!(
            matches!(pack_result, Err(PackTrustError::UntrustedIssuer)),
            "verify_pack must reject a pack whose issuer cert isn't root-signed; got {pack_result:?}"
        );
    }

    /// A pack with no `signature` key at all must be rejected as
    /// `MissingSignature` (D-09 tiering relies on this being distinguishable
    /// from tamper/untrusted-issuer rejections).
    #[test]
    fn missing_signature_returns_missing_signature() {
        let (_root_key, root_pem) = keypair();
        let unsigned_pack = pack_body();

        let result = verify_pack(&root_pem, &unsigned_pack);
        assert!(
            matches!(result, Err(PackTrustError::MissingSignature)),
            "pack with no signature key must yield MissingSignature; got {result:?}"
        );
    }

    /// A1 mitigation — JCS conformance smoke test against a known RFC 8785
    /// vector. Confirms `jcs_bytes` produces spec-correct output, not just
    /// internally-consistent output (byte-stability alone wouldn't catch a
    /// systematic divergence from the RFC 8785 spec).
    #[test]
    fn jcs_conformance_rfc8785_vector() {
        // RFC 8785 Appendix B "French" example, restricted to a subset that
        // exercises key-ordering + unicode escaping without relying on
        // locale-specific float formatting.
        let v = serde_json::json!({
            "peach": "This sorts after appple",
            "appple": "This sorts before peach",
            "1": "One",
            "10": "Ten",
            "2": "Two",
        });
        let bytes = jcs_bytes(&v).expect("JCS bytes");
        let s = String::from_utf8(bytes).expect("JCS output is valid UTF-8");
        // RFC 8785 §3.2.3: object keys sort by UTF-16 code unit, so numeric
        // string keys "1" < "10" < "2" (lexicographic, not numeric).
        assert_eq!(
            s,
            r#"{"1":"One","10":"Ten","2":"Two","appple":"This sorts before peach","peach":"This sorts after appple"}"#
        );
    }
}
