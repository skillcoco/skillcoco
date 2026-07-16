//! Demonstrate Ed25519 sign / verify against a canonical JSON payload.
//!
//! Generates a fresh Ed25519 key, canonicalizes a sample JSON payload via
//! `canonical_json_bytes`, signs it, derives the public-key fingerprint,
//! and verifies the round-trip — including a deliberate tampered-payload
//! negative case.
//!
//! Demonstrates **byte stability**: re-canonicalizing the payload with the
//! object keys in a different declaration order produces the same byte
//! sequence, so the same signature verifies.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p skillcoco-core --example verify_payload
//! ```

use ed25519_dalek::{pkcs8::EncodePublicKey, SigningKey};
use skillcoco_core::canonical_json::canonical_json_bytes;
use skillcoco_core::signing::{public_key_fingerprint, sign_payload, verify_payload};
use rand::rngs::OsRng;
use serde_json::json;

fn main() {
    let key = SigningKey::generate(&mut OsRng);
    let public_pem = key
        .verifying_key()
        .to_public_key_pem(pkcs8::LineEnding::LF)
        .expect("encode public key as PEM");
    let fp = public_key_fingerprint(&key.verifying_key());

    println!("Generated Ed25519 key");
    println!("  fingerprint (first 8 hex chars of SHA-256(pub-DER)): {fp}");
    println!();

    // Payload — keys deliberately declared OUT of lexicographic order so
    // we can demonstrate that canonicalization sorts them.
    let payload_a = json!({
        "track": "kubernetes-fundamentals",
        "level": "Practitioner",
        "issuedAt": "2026-06-16T22:30:00Z",
        "learner": "alice@example.com"
    });
    let bytes_a = canonical_json_bytes(&payload_a).expect("canonicalize");
    let sig_a = sign_payload(&key, &bytes_a);
    let sig_hex = hex::encode(sig_a.to_bytes());

    println!("Canonical payload bytes ({} bytes):", bytes_a.len());
    println!("  {}", std::str::from_utf8(&bytes_a).unwrap());
    println!("Signature (hex): {sig_hex}");
    println!();

    // Verify the round-trip.
    let ok = verify_payload(&public_pem, &bytes_a, &sig_hex);
    println!("verify (untampered): {ok}");
    assert!(ok, "signature must verify");

    // Byte-stability check: same logical payload, keys in different order.
    let payload_b = json!({
        "learner": "alice@example.com",
        "issuedAt": "2026-06-16T22:30:00Z",
        "level": "Practitioner",
        "track": "kubernetes-fundamentals"
    });
    let bytes_b = canonical_json_bytes(&payload_b).expect("canonicalize");
    let same_bytes = bytes_a == bytes_b;
    let ok_reordered = verify_payload(&public_pem, &bytes_b, &sig_hex);
    println!("canonical bytes match across key orderings: {same_bytes}");
    println!("verify (reordered keys): {ok_reordered}");
    assert!(same_bytes, "canonical JSON must be byte-stable");
    assert!(ok_reordered, "reordered keys must verify with the same signature");

    // Negative case: tamper with the payload.
    let mut tampered = bytes_a.clone();
    tampered[10] ^= 0x01;
    let ok_tampered = verify_payload(&public_pem, &tampered, &sig_hex);
    println!("verify (tampered): {ok_tampered}");
    assert!(!ok_tampered, "tampered payload must fail verification");

    println!();
    println!("All assertions passed.");
}
