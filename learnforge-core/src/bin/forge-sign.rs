//! forge-sign — Phase 14 pack-signing CLI (D-13).
//!
//! Subcommands: `keygen-root`, `issue-cert`, `sign`. Composes
//! `learnforge_core::pack_trust` (JCS + chain-of-trust) with a FS-backed key
//! recipe mirrored verbatim from `src-tauri/src/storage_impl/signing.rs`'s
//! `FsKeyStore::get_or_init` (lazy-init, 0o600 private key on Unix).
//!
//! WASM note (14-RESEARCH Pitfall 1): this bin shares the crate's
//! `[dependencies]` table and would be compiled by an unscoped
//! `cargo build --target wasm32-unknown-unknown -p learnforge-core` gate.
//! Its own dependency footprint is kept to `std::fs` + `clap` + the existing
//! `pack_trust`/`signing` stack — nothing that fails to *compile* on
//! wasm32-unknown-unknown. The documented gate is additionally narrowed to
//! `--lib` (see `docs/DEVELOPMENT.md`) so a future bin-only dependency can
//! never break it (RESEARCH Open Question 2 RESOLVED).

use clap::{Parser, Subcommand};
use ed25519_dalek::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey};
use ed25519_dalek::SigningKey;
use learnforge_core::canonical_json::canonical_json_bytes;
use learnforge_core::pack_trust::{self, IssuerCert};
use learnforge_core::reports::ReportEnvelopeV1;
use learnforge_core::signing;
use pkcs8::LineEnding;
use rand::rngs::OsRng;
use std::path::{Path, PathBuf};

/// File name for a private signing key PEM (root or issuer — same layout).
const PRIVATE_PEM: &str = "root_private.pem";
/// File name for a public signing key PEM.
const PUBLIC_PEM: &str = "root_public.pem";

/// LearnForge pack-signing tool: root keygen, issuer cert issuance,
/// and pack signing (root → issuer cert → signed pack chain of trust).
#[derive(Parser)]
#[command(name = "forge-sign", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate the root signing keypair (trust anchor). Idempotent — a
    /// pre-existing root key at `--out-dir` is loaded, not overwritten.
    KeygenRoot {
        /// Directory to hold root_private.pem (0600) and root_public.pem.
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// Issue an issuer certificate signed by the root key.
    IssueCert {
        /// Path to the root's private key PEM.
        #[arg(long)]
        root_key: PathBuf,
        /// Stable issuer identity (tenant key for Hub).
        #[arg(long)]
        issuer_id: String,
        /// Human-readable publisher name.
        #[arg(long)]
        name: String,
        /// Path to the issuer's PUBLIC key PEM to embed in the cert.
        #[arg(long)]
        issuer_pubkey: PathBuf,
        /// Output path for the issued cert JSON.
        #[arg(long)]
        out: PathBuf,
    },
    /// Sign a pack JSON file with an issuer's signing key.
    Sign {
        /// Path to the issuer's private key PEM.
        #[arg(long)]
        issuer_key: PathBuf,
        /// Path to the issuer cert JSON (from issue-cert).
        #[arg(long)]
        cert: PathBuf,
        /// Input pack JSON file.
        #[arg(long = "in")]
        input: PathBuf,
        /// Output path for the signed pack JSON.
        #[arg(long)]
        out: PathBuf,
    },
    /// Verify a signed skill-report JSON's signature (D-14 — team
    /// aggregation tooling). Reuses `learnforge_core::signing::verify_payload`
    /// — the SAME verify path the app's Verify panel uses — zero crypto
    /// drift, zero second Ed25519 implementation.
    VerifyReport {
        /// Path to the report JSON file (the `ReportEnvelopeV1` shape
        /// `export_report_json` writes).
        #[arg(long)]
        input: PathBuf,
        /// Path to the signing public-key PEM to verify against.
        #[arg(long)]
        public_key: PathBuf,
    },
}

/// Typed error for forge-sign's own I/O + composition failures. Distinct
/// from `PackTrustError` (that's the pure crypto layer) — this wraps file
/// I/O and JSON (de)serialization around it, mirroring the project's
/// existing `From`-remap-at-the-boundary convention.
#[derive(Debug, thiserror::Error)]
enum ForgeSignError {
    #[error("io error: {0}")]
    Io(String),
    #[error("key encoding error: {0}")]
    KeyEncoding(String),
    #[error("json error: {0}")]
    Json(String),
    #[error("pack-trust error: {0}")]
    PackTrust(#[from] pack_trust::PackTrustError),
}

/// Load an existing root/issuer private key from `priv_path`, or generate
/// a fresh keypair and persist both PEMs to `out_dir` if none exists yet.
///
/// **Recipe mirrored verbatim from `FsKeyStore::get_or_init`**
/// (`src-tauri/src/storage_impl/signing.rs:84-126`): check-exists →
/// load-if-present, else create_dir_all → generate → write private PEM →
/// chmod 0o600 on Unix → write public PEM (world-readable). Idempotent:
/// calling this twice on the same `out_dir` never overwrites an existing
/// key (T-14-08 mitigation).
fn keygen_root_inner(out_dir: &Path) -> Result<SigningKey, ForgeSignError> {
    let priv_p = out_dir.join(PRIVATE_PEM);
    let pub_p = out_dir.join(PUBLIC_PEM);

    if priv_p.exists() {
        let pem = std::fs::read_to_string(&priv_p)
            .map_err(|e| ForgeSignError::Io(format!("read private pem: {e}")))?;
        let key = SigningKey::from_pkcs8_pem(&pem)
            .map_err(|e| ForgeSignError::KeyEncoding(format!("decode private pem: {e}")))?;
        return Ok(key);
    }

    std::fs::create_dir_all(out_dir)
        .map_err(|e| ForgeSignError::Io(format!("create_dir_all: {e}")))?;
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);

    let priv_pem = signing_key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| ForgeSignError::KeyEncoding(format!("encode private pem: {e}")))?;
    std::fs::write(&priv_p, priv_pem.as_bytes())
        .map_err(|e| ForgeSignError::Io(format!("write private pem: {e}")))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&priv_p, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| ForgeSignError::Io(format!("chmod 0600 private pem: {e}")))?;
    }

    let pub_pem = signing_key
        .verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| ForgeSignError::KeyEncoding(format!("encode public pem: {e}")))?;
    std::fs::write(&pub_p, pub_pem)
        .map_err(|e| ForgeSignError::Io(format!("write public pem: {e}")))?;

    Ok(signing_key)
}

/// Load a SigningKey from a private-key PEM file at `path`.
fn load_signing_key(path: &Path) -> Result<SigningKey, ForgeSignError> {
    let pem = std::fs::read_to_string(path)
        .map_err(|e| ForgeSignError::Io(format!("read private pem {}: {e}", path.display())))?;
    SigningKey::from_pkcs8_pem(&pem)
        .map_err(|e| ForgeSignError::KeyEncoding(format!("decode private pem: {e}")))
}

/// Issue an issuer cert signed by the root key, given a root private-key
/// PEM path and an issuer PUBLIC-key PEM path.
fn issue_cert_inner(
    root_key_path: &Path,
    issuer_id: &str,
    name: &str,
    issuer_pubkey_path: &Path,
) -> Result<IssuerCert, ForgeSignError> {
    let root_key = load_signing_key(root_key_path)?;
    let issuer_pub_pem = std::fs::read_to_string(issuer_pubkey_path).map_err(|e| {
        ForgeSignError::Io(format!(
            "read issuer public pem {}: {e}",
            issuer_pubkey_path.display()
        ))
    })?;
    // Validate the PEM decodes to a real Ed25519 public key before embedding
    // it in the cert (fail closed on malformed input rather than persisting
    // a cert nothing can ever verify against).
    ed25519_dalek::VerifyingKey::from_public_key_pem(&issuer_pub_pem)
        .map_err(|e| ForgeSignError::KeyEncoding(format!("decode issuer public pem: {e}")))?;

    let cert = pack_trust::issue_cert(&root_key, issuer_id, name, issuer_pub_pem.trim())?;
    Ok(cert)
}

/// Machine-readable result of `verify-report`, printed to stdout as JSON.
#[derive(serde::Serialize)]
struct VerifyReportResult {
    valid: bool,
    #[serde(rename = "learnerName")]
    learner_name: String,
    #[serde(rename = "capabilityCount")]
    capability_count: usize,
    #[serde(rename = "keyFingerprint")]
    key_fingerprint: String,
}

/// Verify a signed skill-report JSON file against a public-key PEM.
///
/// Deserializes the FIXED `ReportEnvelopeV1` shape 18-03's
/// `export_report_json` emits (do NOT guess or re-parse an ad-hoc format),
/// recomputes canonical bytes over `envelope.payload` via
/// `canonical_json_bytes`, and calls `signing::verify_payload` — the SAME
/// path the app's Verify panel uses. Returns the result regardless of
/// validity (the caller decides the process exit code); only file I/O or
/// JSON-parse failures raise `ForgeSignError`.
fn verify_report_inner(
    input: &Path,
    public_key: &Path,
) -> Result<VerifyReportResult, ForgeSignError> {
    let report_text = std::fs::read_to_string(input)
        .map_err(|e| ForgeSignError::Io(format!("read report {}: {e}", input.display())))?;
    let envelope: ReportEnvelopeV1 = serde_json::from_str(&report_text)
        .map_err(|e| ForgeSignError::Json(format!("parse report envelope: {e}")))?;

    let pem = std::fs::read_to_string(public_key).map_err(|e| {
        ForgeSignError::Io(format!("read public key {}: {e}", public_key.display()))
    })?;

    let canonical = canonical_json_bytes(&envelope.payload)
        .map_err(|e| ForgeSignError::Json(format!("canonicalize report payload: {e}")))?;
    let valid = signing::verify_payload(&pem, &canonical, &envelope.signature_hex);

    Ok(VerifyReportResult {
        valid,
        learner_name: envelope.payload.learner_name.clone(),
        capability_count: envelope.payload.capabilities.len(),
        key_fingerprint: envelope.key_fingerprint,
    })
}

/// Sign a pack JSON value with the issuer's private key + cert.
fn sign_inner(
    issuer_key_path: &Path,
    cert_path: &Path,
    pack_json: &serde_json::Value,
) -> Result<serde_json::Value, ForgeSignError> {
    let issuer_key = load_signing_key(issuer_key_path)?;
    let cert_text = std::fs::read_to_string(cert_path)
        .map_err(|e| ForgeSignError::Io(format!("read cert {}: {e}", cert_path.display())))?;
    let cert: IssuerCert = serde_json::from_str(&cert_text)
        .map_err(|e| ForgeSignError::Json(format!("parse issuer cert: {e}")))?;

    let signed = pack_trust::sign_pack(&issuer_key, &cert, pack_json)?;
    Ok(signed)
}

/// Result of `run()` — distinguishes a hard error (`Err`) from a
/// successful-but-invalid verify-report result (`Ok(false)`), so `main()`
/// can set the right process exit code for each case (0 / 1 / 1
/// respectively) without conflating "the tool failed" with "the report
/// failed verification".
fn run() -> Result<bool, ForgeSignError> {
    let cli = Cli::parse();
    match cli.command {
        Command::KeygenRoot { out_dir } => {
            keygen_root_inner(&out_dir)?;
            println!(
                "root keypair ready at {} ({} / {})",
                out_dir.display(),
                PRIVATE_PEM,
                PUBLIC_PEM
            );
            Ok(true)
        }
        Command::IssueCert {
            root_key,
            issuer_id,
            name,
            issuer_pubkey,
            out,
        } => {
            let cert = issue_cert_inner(&root_key, &issuer_id, &name, &issuer_pubkey)?;
            let json = serde_json::to_string_pretty(&cert)
                .map_err(|e| ForgeSignError::Json(format!("serialize cert: {e}")))?;
            std::fs::write(&out, json)
                .map_err(|e| ForgeSignError::Io(format!("write cert {}: {e}", out.display())))?;
            println!("issuer cert written to {}", out.display());
            Ok(true)
        }
        Command::Sign {
            issuer_key,
            cert,
            input,
            out,
        } => {
            let pack_text = std::fs::read_to_string(&input)
                .map_err(|e| ForgeSignError::Io(format!("read pack {}: {e}", input.display())))?;
            let pack_json: serde_json::Value = serde_json::from_str(&pack_text)
                .map_err(|e| ForgeSignError::Json(format!("parse pack json: {e}")))?;
            let signed = sign_inner(&issuer_key, &cert, &pack_json)?;
            let out_json = serde_json::to_string_pretty(&signed)
                .map_err(|e| ForgeSignError::Json(format!("serialize signed pack: {e}")))?;
            std::fs::write(&out, out_json)
                .map_err(|e| ForgeSignError::Io(format!("write signed pack {}: {e}", out.display())))?;
            println!("signed pack written to {}", out.display());
            Ok(true)
        }
        Command::VerifyReport { input, public_key } => {
            let result = verify_report_inner(&input, &public_key)?;
            let valid = result.valid;
            let json = serde_json::to_string(&result)
                .map_err(|e| ForgeSignError::Json(format!("serialize verify-report result: {e}")))?;
            println!("{json}");
            Ok(valid)
        }
    }
}

fn main() {
    match run() {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("forge-sign error: {e}");
            std::process::exit(1);
        }
    }
}
