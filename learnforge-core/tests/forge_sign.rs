//! Integration tests for the `forge-sign` CLI bin (plan 14-03).
//!
//! Shells out to the actual built binary via `CARGO_BIN_EXE_forge-sign`
//! (a cargo-provided env var pointing at the compiled bin for this package
//! — no extra test-harness dependency needed) so these tests exercise the
//! exact same code path a real user/CI invocation would.

use ed25519_dalek::pkcs8::{EncodePrivateKey, EncodePublicKey};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::path::Path;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_forge-sign"))
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
