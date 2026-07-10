//! Integration tests for the `forge-sign` CLI bin (plan 14-03).
//!
//! Shells out to the actual built binary via `CARGO_BIN_EXE_forge-sign`
//! (a cargo-provided env var pointing at the compiled bin for this package
//! — no extra test-harness dependency needed) so these tests exercise the
//! exact same code path a real user/CI invocation would.

use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
use ed25519_dalek::SigningKey;
use learnforge_core::canonical_json::canonical_json_bytes;
use learnforge_core::reports::{
    CapabilityRow, EvidenceClass, EvidenceItem, MasteryDimension, ReportEnvelopeV1, ReportMetadata,
    ReportPayloadV1,
};
use learnforge_core::signing::{public_key_fingerprint, sign_payload};
use rand::rngs::OsRng;
use std::path::Path;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_forge-sign"))
}

/// Build a signed `ReportEnvelopeV1` fixture directly (no DB/store needed —
/// hand-assembled to exercise the exact fixed shape 18-03's
/// `export_report_json` emits), using the SAME `canonical_json_bytes` +
/// `sign_payload` chain the app uses. Returns the envelope plus the signing
/// key's public PEM (for `--public-key`).
fn signed_report_fixture() -> (ReportEnvelopeV1, String) {
    let key = SigningKey::generate(&mut OsRng);
    let pub_pem = key
        .verifying_key()
        .to_public_key_pem(pkcs8::LineEnding::LF)
        .expect("encode public pem");
    let key_fingerprint = public_key_fingerprint(&key.verifying_key());

    let payload = ReportPayloadV1 {
        learner_name: "Ada Lovelace".to_string(),
        learner_id: "learner-cli-001".to_string(),
        scope_label: "Kubernetes Fundamentals".to_string(),
        capabilities: vec![CapabilityRow {
            slug: "can-configure-rbac-policies".to_string(),
            label: "Can configure RBAC policies".to_string(),
            knowledge: MasteryDimension {
                band: "Proficient".to_string(),
                pct: 0.82,
            },
            practical: Some(MasteryDimension {
                band: "Working".to_string(),
                pct: 0.55,
            }),
            contributing_tracks: vec!["track-k8s-fundamentals".to_string()],
            evidence: vec![EvidenceItem {
                class: EvidenceClass::Quiz,
                label: "Quiz: RBAC basics".to_string(),
                detail: "8/10 correct".to_string(),
                date: "2026-07-01T00:00:00+00:00".to_string(),
                track_id: Some("track-k8s-fundamentals".to_string()),
                track_topic: Some("Kubernetes Fundamentals".to_string()),
            }],
        }],
        metadata: ReportMetadata {
            generated_at: "2026-07-10T00:00:00+00:00".to_string(),
            app_version: "0.0.0-test".to_string(),
            pack_provenance: None,
            verified_issuer: None,
        },
        issuer: None,
        key_fingerprint: key_fingerprint.clone(),
        payload_version: 1,
    };

    let canonical = canonical_json_bytes(&payload).expect("canonical json bytes");
    let sig = sign_payload(&key, &canonical);
    let signature_hex = hex::encode(sig.to_bytes());

    let envelope = ReportEnvelopeV1 {
        payload,
        signature_hex,
        key_fingerprint,
    };

    (envelope, pub_pem)
}

/// `verify_report_valid_fixture_prints_valid_true` — a freshly signed
/// report envelope verifies as valid via the CLI, exits 0, and the printed
/// JSON carries learnerName/capabilityCount/keyFingerprint.
#[test]
fn verify_report_valid_fixture_prints_valid_true() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (envelope, pub_pem) = signed_report_fixture();

    let report_path = tmp.path().join("report.json");
    std::fs::write(
        &report_path,
        serde_json::to_string_pretty(&envelope).expect("serialize envelope"),
    )
    .expect("write report json");

    let pubkey_path = tmp.path().join("pub.pem");
    std::fs::write(&pubkey_path, &pub_pem).expect("write public pem");

    let output = bin()
        .args(["verify-report", "--input"])
        .arg(&report_path)
        .arg("--public-key")
        .arg(&pubkey_path)
        .output()
        .expect("run forge-sign verify-report");

    assert!(
        output.status.success(),
        "verify-report must exit 0 for a valid report, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("verify-report stdout must be valid JSON");
    assert_eq!(parsed["valid"], serde_json::json!(true));
    assert_eq!(parsed["learnerName"], serde_json::json!("Ada Lovelace"));
    assert_eq!(parsed["capabilityCount"], serde_json::json!(1));
    assert_eq!(
        parsed["keyFingerprint"],
        serde_json::json!(envelope.key_fingerprint)
    );
}

/// `verify_report_tampered_pct_prints_valid_false_and_nonzero_exit` — a
/// report file with an edited capability pct fails verification: prints
/// valid:false and exits non-zero.
#[test]
fn verify_report_tampered_pct_prints_valid_false_and_nonzero_exit() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (envelope, pub_pem) = signed_report_fixture();

    let mut tampered_json: serde_json::Value = serde_json::to_value(&envelope)
        .expect("envelope to json value");
    tampered_json["payload"]["capabilities"][0]["knowledge"]["pct"] =
        serde_json::json!(0.99);

    let report_path = tmp.path().join("tampered-report.json");
    std::fs::write(
        &report_path,
        serde_json::to_string_pretty(&tampered_json).expect("serialize tampered envelope"),
    )
    .expect("write tampered report json");

    let pubkey_path = tmp.path().join("pub.pem");
    std::fs::write(&pubkey_path, &pub_pem).expect("write public pem");

    let output = bin()
        .args(["verify-report", "--input"])
        .arg(&report_path)
        .arg("--public-key")
        .arg(&pubkey_path)
        .output()
        .expect("run forge-sign verify-report on tampered input");

    assert!(
        !output.status.success(),
        "verify-report must exit non-zero for a tampered report"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("verify-report stdout must be valid JSON even for invalid reports");
    assert_eq!(parsed["valid"], serde_json::json!(false));
}

/// Generate a standalone Ed25519 keypair and write its PUBLIC PEM to
/// `path` — simulates an issuer's own key material (independent of
/// forge-sign's own keygen-root, which is root-specific).
fn write_issuer_pubkey(path: &Path) -> SigningKey {
    let key = SigningKey::generate(&mut OsRng);
    let pem = key
        .verifying_key()
        .to_public_key_pem(pkcs8::LineEnding::LF)
        .expect("encode issuer public pem");
    std::fs::write(path, pem).expect("write issuer public pem");
    key
}

/// `keygen_root_writes_0600_private_key` — after keygen-root, the private
/// PEM exists at 0600 on Unix and the public PEM is world-readable.
#[test]
fn keygen_root_writes_0600_private_key() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let status = bin()
        .args(["keygen-root", "--out-dir"])
        .arg(tmp.path())
        .status()
        .expect("run forge-sign keygen-root");
    assert!(status.success(), "keygen-root must exit 0");

    let priv_path = tmp.path().join("root_private.pem");
    let pub_path = tmp.path().join("root_public.pem");
    assert!(priv_path.exists(), "private key PEM must exist");
    assert!(pub_path.exists(), "public key PEM must exist");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&priv_path)
            .expect("stat private pem")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "private key must be 0600, got {:o}", mode);
    }

    let pub_pem = std::fs::read_to_string(&pub_path).expect("read public pem");
    assert!(pub_pem.starts_with("-----BEGIN PUBLIC KEY-----"));
}

/// `keygen_root_is_idempotent` — running keygen-root twice does not
/// overwrite an existing key (load-if-present).
#[test]
fn keygen_root_is_idempotent() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let status1 = bin()
        .args(["keygen-root", "--out-dir"])
        .arg(tmp.path())
        .status()
        .expect("run forge-sign keygen-root (1st)");
    assert!(status1.success());
    let first_priv = std::fs::read_to_string(tmp.path().join("root_private.pem"))
        .expect("read private pem after 1st run");

    let status2 = bin()
        .args(["keygen-root", "--out-dir"])
        .arg(tmp.path())
        .status()
        .expect("run forge-sign keygen-root (2nd)");
    assert!(status2.success());
    let second_priv = std::fs::read_to_string(tmp.path().join("root_private.pem"))
        .expect("read private pem after 2nd run");

    assert_eq!(
        first_priv, second_priv,
        "second keygen-root run must not overwrite the existing root key"
    );
}

/// `issue_cert_output_verifies_against_root` — issue-cert output JSON,
/// parsed as IssuerCert, passes verify_issuer_cert with the root public
/// PEM.
#[test]
fn issue_cert_output_verifies_against_root() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let status = bin()
        .args(["keygen-root", "--out-dir"])
        .arg(tmp.path())
        .status()
        .expect("run keygen-root");
    assert!(status.success());

    let issuer_pub_path = tmp.path().join("issuer_public.pem");
    let _issuer_key = write_issuer_pubkey(&issuer_pub_path);

    let cert_out = tmp.path().join("cert.json");
    let status = bin()
        .args(["issue-cert", "--root-key"])
        .arg(tmp.path().join("root_private.pem"))
        .args(["--issuer-id", "issuer-cli-001", "--name", "CLI Test Issuer"])
        .arg("--issuer-pubkey")
        .arg(&issuer_pub_path)
        .arg("--out")
        .arg(&cert_out)
        .status()
        .expect("run issue-cert");
    assert!(status.success(), "issue-cert must exit 0");

    let cert_json = std::fs::read_to_string(&cert_out).expect("read cert output");
    let cert: learnforge_core::pack_trust::IssuerCert =
        serde_json::from_str(&cert_json).expect("cert output parses as IssuerCert");

    let root_pem =
        std::fs::read_to_string(tmp.path().join("root_public.pem")).expect("read root public pem");
    learnforge_core::pack_trust::verify_issuer_cert(&root_pem, &cert)
        .expect("issue-cert output must verify against the root public PEM");
}

/// `sign_output_verifies_against_root` — sign output pack, passed to
/// verify_pack(root_pem, ..), returns Ok(()).
#[test]
fn sign_output_verifies_against_root() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let status = bin()
        .args(["keygen-root", "--out-dir"])
        .arg(tmp.path())
        .status()
        .expect("run keygen-root");
    assert!(status.success());

    let issuer_pub_path = tmp.path().join("issuer_public.pem");
    let issuer_key = write_issuer_pubkey(&issuer_pub_path);
    let issuer_priv_path = tmp.path().join("issuer_private.pem");
    let issuer_priv_pem = issuer_key
        .to_pkcs8_pem(pkcs8::LineEnding::LF)
        .expect("encode issuer private pem");
    std::fs::write(&issuer_priv_path, issuer_priv_pem.as_bytes())
        .expect("write issuer private pem");

    let cert_out = tmp.path().join("cert.json");
    let status = bin()
        .args(["issue-cert", "--root-key"])
        .arg(tmp.path().join("root_private.pem"))
        .args(["--issuer-id", "issuer-cli-002", "--name", "CLI Test Issuer 2"])
        .arg("--issuer-pubkey")
        .arg(&issuer_pub_path)
        .arg("--out")
        .arg(&cert_out)
        .status()
        .expect("run issue-cert");
    assert!(status.success());

    let pack_in = tmp.path().join("pack.json");
    std::fs::write(
        &pack_in,
        serde_json::json!({
            "id": "cli-test-pack",
            "title": "CLI Test Pack",
            "exportedFrom": "licensed:cli-test-pack|CLI Test Publisher",
            "modules": [],
        })
        .to_string(),
    )
    .expect("write input pack");

    let signed_out = tmp.path().join("signed.json");
    let status = bin()
        .args(["sign", "--issuer-key"])
        .arg(&issuer_priv_path)
        .arg("--cert")
        .arg(&cert_out)
        .arg("--in")
        .arg(&pack_in)
        .arg("--out")
        .arg(&signed_out)
        .status()
        .expect("run sign");
    assert!(status.success(), "sign must exit 0");

    let signed_json = std::fs::read_to_string(&signed_out).expect("read signed pack");
    let signed_value: serde_json::Value =
        serde_json::from_str(&signed_json).expect("signed pack output parses as JSON");

    let root_pem =
        std::fs::read_to_string(tmp.path().join("root_public.pem")).expect("read root public pem");
    learnforge_core::pack_trust::verify_pack(&root_pem, &signed_value)
        .expect("sign output must verify against the root public PEM");
}
