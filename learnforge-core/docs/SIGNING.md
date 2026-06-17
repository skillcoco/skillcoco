# Certificate Signing — Ed25519 + Canonical JSON

> **Author:** LearnForge OSS contributors
> **Date:** 2026-06-17
> **License:** [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)
> **Module:** [`learnforge_core::signing`](../src/signing.rs)

This whitepaper documents the certificate signing and verification
pipeline as implemented in `learnforge_core::signing` and
`learnforge_core::canonical_json`. It is intended for engineers,
security auditors, learning-platform implementers, and curious learners
who want to understand exactly how LearnForge produces a verifiable
mastery certificate — and why a small, custom-canonical-JSON envelope
was chosen over off-the-shelf signing standards like JWS.

The signing layer sits **above** the [threshold predicates](./THRESHOLD.md):
when a learner crosses a tier (Associate / Practitioner / Professional),
the achievement-issuance pipeline builds a structured payload, runs it
through canonical JSON serialization, and signs the byte sequence with
the per-install Ed25519 private key. The resulting `(payload,
signature, key_fingerprint)` triple is what surfaces in the UI as a
shareable cert.

---

## 1. Abstract

LearnForge certificates are JSON payloads signed with Ed25519. To make
the signature verifiable years later, on different machines, by a
hosted verifier service that may run an entirely different
serialization stack, the bytes Ed25519 signs MUST be **byte-stable**:
two implementations of "JSON-stringify this payload" must agree to the
last byte. This whitepaper specifies the canonical-JSON subset
LearnForge uses (sorted keys, no whitespace, finite-only numbers, no
quoting of numeric fields), the Ed25519 keypair lifecycle (`OsRng`-
backed generation, file-mode-0600 persistence on Unix), the 8-character
SHA-256 fingerprint as a human-displayable key identifier, the
`payloadVersion: u32` forward-compatibility scheme that lets Phase 14's
hosted verifier dispatch on legacy payloads, and the deliberate
exclusion of revocation lists / OCSP / time-bounded validity from the
0.1.0 surface. The implementation is pure-functional crypto (sign,
verify, fingerprint) plus a `SigningKeyStore` trait that the desktop
adapter implements against the local filesystem and the web adapter
can implement against IndexedDB.

---

## 2. Problem statement — why naive JSON-stringify-then-sign breaks

A LearnForge certificate is, at its semantic core, a JSON object:

```json
{
  "learner": "Alice Example",
  "learnerId": "abc-123",
  "track": "Kubernetes Foundations",
  "trackId": "trk-456",
  "level": "Professional",
  "completionDate": "2026-06-17T14:32:11Z",
  "masteryScore": 0.87,
  "keyFingerprint": "a1b2c3d4",
  "packId": "k8s-pack-v1",
  "payloadVersion": 1
}
```

To make this verifiable the issuer signs **some bytes derived from
the JSON** with an Ed25519 private key, and the verifier
recomputes those bytes and checks the signature against the issuer's
public key. The naive approach is:

```rust,ignore
let json = serde_json::to_string(&payload)?;
let signature = ed25519_sign(&private_key, json.as_bytes());
```

This breaks in five distinct ways at the byte level:

1. **Key ordering is implementation-defined.** `serde_json` outputs
   keys in the order the underlying `Map<String, Value>` enumerates
   them. With the default feature flags this is **insertion order**;
   with `preserve_order` it is also insertion order but via a
   `Vec<(String, Value)>` backing store; with `BTreeMap` it is
   lexicographic. Two builds of the same code with different feature
   flags will produce non-identical JSON, and therefore non-identical
   signatures.

2. **Whitespace is implementation-defined.** `serde_json::to_string`
   produces no whitespace; `serde_json::to_string_pretty` produces
   2-space indentation. A documentation example that uses
   `to_string_pretty` for readability accidentally invalidates every
   certificate produced before the maintainer notices.

3. **Number formatting is implementation-defined.** `0.87` may render
   as `"0.87"` or `"0.8700000000000000"` or `"8.7e-1"` depending on
   the underlying float-to-string routine. Most JSON libraries pick
   one consistently, but the choice differs across languages (Python's
   `json.dumps` vs. Rust's `serde_json` vs. JavaScript's
   `JSON.stringify`).

4. **Special float values have no canonical form.** `NaN`, `+∞`, `-∞`
   cannot be represented in JSON at all (JSON has no `NaN` token).
   Some libraries silently emit `"null"`, others throw, others emit
   a non-spec `"NaN"` token. None of these are interoperable.

5. **Unicode normalization is implementation-defined.** A string
   containing the character `é` may be serialized as the precomposed
   `U+00E9` or the decomposed `U+0065 U+0301`. Most libraries
   preserve the input bytes, but a payload that goes through a
   normalization step (e.g., an HTTP middleware applying NFC) will
   produce a different byte sequence than the original.

Any one of these breaks the verifier. **All five are real failure
modes** observed in production certificate-signing systems (see, e.g.,
RFC 8785's motivation section).

The off-the-shelf solution is JWS (RFC 7515) with the EdDSA algorithm
identifier (RFC 8037), which standardizes the envelope. LearnForge
chose **not** to use JWS for three reasons:

1. **JWS is overkill** for a single-issuer, single-payload-type use
   case. The algorithm header (`{"alg":"EdDSA","typ":"JWT"}`) carries
   ~30 bytes that LearnForge does not need (the algorithm is locked
   to Ed25519 in code; the type is implicit from the consuming
   endpoint).
2. **JWS bundles base64url-encoding into the envelope.** LearnForge
   needs base64url at the QR-code transport layer (see §3.6) but not
   at the signing layer — the signature is over raw JSON bytes, not
   base64url. Coupling them couples the on-disk format to the
   transport format unnecessarily.
3. **The canonical-JSON path is auditable.** Anyone reading
   `canonical_json.rs` can re-derive the bytes by hand. JWS adds an
   indirection (header + payload + signature in three base64url
   segments joined by dots) that obscures the underlying contract.

The result is a small, custom envelope that is straightforward to
audit and that addresses all five failure modes explicitly.

---

## 3. The algorithm

### 3.1 Canonical JSON serialization

The signing input is **canonical JSON** — a deterministic subset of
JSON with these constraints:

1. **Object keys are sorted lexicographically** at every nesting
   level. `{"b": 2, "a": 1}` and `{"a": 1, "b": 2}` produce the
   same bytes after canonicalization.
2. **No whitespace.** No spaces, no newlines, no tabs.
3. **Non-finite floats are rejected.** `NaN`, `+∞`, `-∞` raise
   `CanonicalJsonError::NonFiniteFloat`.
4. **Arrays preserve order.** Array element order is semantically
   meaningful (e.g., an ordered list of completed modules), so the
   canonicalizer does NOT sort arrays.
5. **UTF-8 encoded.** Output is a `Vec<u8>` of UTF-8 bytes; downstream
   transport layers (base64url, QR codes) operate on these bytes.

The implementation is two functions in
`learnforge_core::canonical_json`:

```rust,ignore
pub fn canonicalize(v: Value) -> Result<Value, CanonicalJsonError>;

pub fn canonical_json_bytes<T: Serialize>(payload: &T)
    -> Result<Vec<u8>, CanonicalJsonError>;
```

`canonicalize` walks a `serde_json::Value` recursively: at each
object it sorts the entries by key and rebuilds a fresh `Map`; at
each array it recurses into elements without reordering; at each
number it checks `is_finite()` and rejects on failure; other values
pass through unchanged.

`canonical_json_bytes<T: Serialize>` serializes `T` to a
`serde_json::Value`, canonicalizes it, and re-serializes the result
to a `Vec<u8>`. The two-stage approach handles arbitrary
`Serialize`-implementing payloads without requiring callers to
hand-build the `Value`.

This is functionally equivalent to a **strict subset of RFC 8785**
(JSON Canonicalization Scheme). LearnForge does not advertise
RFC 8785 compliance because the full RFC requires a specific
number-serialization grammar (Section 3.2.2) that `serde_json` does
not natively produce; LearnForge instead relies on `serde_json`'s
own number formatting being stable across versions (which it is —
the `serde_json` maintainers treat the number format as part of the
crate's public API).

Future Phase 14 verifier implementations in non-Rust languages MUST
either use RFC 8785 or replicate `serde_json`'s number formatting
specifically. The current docs/CERT-PAYLOAD-V1.md spec recommends
the RFC 8785 path as the safest interop route.

### 3.2 The Ed25519 signature

After canonicalization the payload bytes are signed with Ed25519
(RFC 8032):

```rust,ignore
pub fn sign_payload(key: &SigningKey, canonical_bytes: &[u8]) -> Signature {
    key.sign(canonical_bytes)
}
```

Ed25519 is a deterministic signature scheme — the same `(key,
message)` pair always produces the same 64-byte signature. There is
no `OsRng` requirement at signing time; the only randomness needs are
at **keygen** (via `SigningKey::generate(&mut OsRng)`).

The 64-byte signature is hex-encoded (lowercase) for transport. A
LearnForge cert therefore carries:

- `canonical_bytes` (variable length, UTF-8 JSON, ~200–400 bytes
  typical).
- `signature` (64 raw bytes / 128 hex chars).

### 3.3 The key fingerprint

LearnForge displays a short, human-readable fingerprint of the
public key for verification UX:

```rust,ignore
pub fn public_key_fingerprint(verifying: &VerifyingKey) -> String {
    let der = verifying.to_public_key_der()?;
    let hash = Sha256::digest(der.as_bytes());
    hex::encode(&hash[..4])  // first 8 hex chars
}
```

The fingerprint is the first 8 lowercase hex characters of
SHA-256(public_key_DER_bytes). Example: `a1b2c3d4`.

8 hex characters = 32 bits = ~4.3 billion possible values. A
collision rate consistent with random sampling means two distinct
keys have approximately a 1-in-1.5 billion chance of sharing a
fingerprint per pair-comparison. For the LearnForge use case
(one keypair per install, displayed for human comparison in UI),
this is more than adequate; for high-security cryptographic
identification, a longer fingerprint would be needed.

The fingerprint is computed against the **DER-encoded** public key,
not the raw 32-byte key bytes. This matches the input shape that
PEM serialization produces: PEM is a base64-wrapped DER blob, so a
verifier that decodes the PEM gets the same DER bytes the signer
hashed. Falling back to the raw key bytes (when DER encoding fails)
is a defensive measure that should never trigger in production.

### 3.4 The verify path

The pure-functional verify accepts a PEM-encoded public key, the
canonical payload bytes, and a hex-encoded signature:

```rust,ignore
pub fn verify_payload(public_pem: &str, canonical_bytes: &[u8],
                     sig_hex: &str) -> bool;
```

The function returns `false` on any failure — invalid PEM, invalid
hex, signature mismatch — and **never panics**. The
`Result<bool, Error>` shape was considered but rejected: callers
that distinguish "signature did not verify" from "input was
malformed" can do their own parsing; most callers just want a
single trust/untrust decision.

Returning `false` (rather than panicking or throwing) on malformed
input is a security property: an attacker who can submit garbage
to the verifier cannot induce a panic-based denial of service.

### 3.5 The `payloadVersion` forward-compat field

Every certificate carries a `payloadVersion: u32` field:

```json
{
  "...": "...",
  "payloadVersion": 1
}
```

Phase 7's `learnforge_core::verifier` stub returns
`payload_version: 0` and the error `"verifier not implemented in
Phase 7; ships in Phase 14"`. Production certificates from Phase 6
onward use `payloadVersion: 1`. Phase 14 introduces `payloadVersion:
2` if/when the hosted verifier upgrades to a JWS-EdDSA envelope.

The version field is a **dispatch tag** for the verifier:

```rust,ignore
match payload_version {
    1 => verify_v1(...),
    2 => verify_v2(...),
    _ => Err(VerifierError::UnknownVersion),
}
```

The Phase 14 hosted verifier MUST recognize `payloadVersion: 1`
and dispatch to v1 verification — either by parsing the v1 payload
directly or by performing a one-time re-sign migration that wraps
the v1 payload in a v2 envelope (with the v1 payload preserved as a
JWS claim so the chain of custody is auditable). Removing v1 support
before a 6-month transition window has elapsed is **prohibited** by
the Phase 14 forward-compat promise in `docs/CERT-PAYLOAD-V1.md`.

The `u32` width (vs. `u8` or `u16`) was a defensive choice: 4
billion possible versions covers any conceivable future evolution
of the envelope while costing only 4 bytes per cert. The v0 / v1 /
v2 namespace is reserved as documented; v3+ is unallocated.

### 3.6 Transport encoding (QR codes, share links)

Outside the algorithm core, certificates are transported in a
compact form:

```text
<base64url(canonical_bytes)>.<hex(signature)>
```

Two segments joined by a single dot, no whitespace. Visually
similar to JWS compact form but not a JWS (no algorithm header
segment; the period separates two segments, not three). Phase 14
may upgrade to a real JWS-EdDSA envelope; v1 stays as-is for
legacy compatibility.

The base64url encoding (RFC 4648 §5) is no-padding (`=` characters
stripped). This makes the cert URL-safe and printable on a single
line — important for QR codes and share-text outputs.

### 3.7 Worked example — sign and verify

```rust,ignore
use ed25519_dalek::{SigningKey, pkcs8::EncodePublicKey};
use rand::rngs::OsRng;

// Keygen — one-time per install
let key = SigningKey::generate(&mut OsRng);

// Canonical payload
#[derive(serde::Serialize)]
struct CertPayload<'a> {
    learner: &'a str,
    track: &'a str,
    level: &'a str,
    masteryScore: f64,
    payloadVersion: u32,
}
let payload = CertPayload {
    learner: "Alice",
    track: "Kubernetes",
    level: "Professional",
    masteryScore: 0.87,
    payloadVersion: 1,
};

let bytes = canonical_json_bytes(&payload).unwrap();
// `bytes` is byte-stable: alphabetic key ordering, no whitespace
//   {"learner":"Alice","level":"Professional","masteryScore":0.87,
//    "payloadVersion":1,"track":"Kubernetes"}

let sig = sign_payload(&key, &bytes);
let sig_hex = hex::encode(sig.to_bytes());

let pub_pem = key.verifying_key()
    .to_public_key_pem(pkcs8::LineEnding::LF).unwrap();
assert!(verify_payload(&pub_pem, &bytes, &sig_hex));

// Tamper detection — change a single byte in the payload
let mut tampered = bytes.clone();
tampered[0] ^= 0x01;
assert!(!verify_payload(&pub_pem, &tampered, &sig_hex));
```

The unit tests in `learnforge-core/src/signing.rs` exercise the
roundtrip, single-byte-flip tampering, signature corruption, and
malformed-PEM rejection — all of which return `false` cleanly.

---

## 4. Calibration in LearnForge

### 4.1 Why Ed25519 over RSA or ECDSA?

LearnForge chose Ed25519 (RFC 8032) over RSA-PSS and ECDSA-P256 for
five reasons:

1. **Small keys and signatures.** Ed25519 public keys are 32 bytes,
   signatures are 64 bytes. A QR code carrying both encodes
   comfortably; an RSA-2048 signature (256 bytes) would be marginal
   for a Version-10 QR code (with content + URL + level encoding).
2. **No padding pitfalls.** RSA signatures require careful padding
   choice (PKCS#1 v1.5 vs. PSS) and there is a long history of
   implementations getting it wrong. Ed25519 has a single signing
   mode with no parameters.
3. **Deterministic signatures.** Same `(key, message)` → same
   signature. ECDSA signatures depend on a per-signature random `k`
   value, and `k`-reuse is catastrophic (recoverable private key
   from two signatures). Ed25519 derives the per-signature
   randomness from the private key and message hash, eliminating
   the entropy-failure mode at signing time.
4. **Fast verify.** Ed25519 verify is roughly 4x faster than
   ECDSA-P256 verify in `ed25519-dalek`, and over 10x faster than
   RSA-2048 verify. For the Phase 14 hosted verifier — which may
   process many cert verifications per second — this is material.
5. **Single curve.** Ed25519 is fixed to Curve25519. There is no
   "which curve?" decision to make per cert. RSA has key-size
   choice; ECDSA has curve choice. Ed25519 has neither.

The trade-off is that Ed25519 is **newer** than RSA (standardized in
RFC 8032 in 2017 vs. RSA's PKCS#1 from 1991). Some legacy verifiers
may not support it. LearnForge accepts this — the cert format is
Phase 6-and-later only; there are no legacy verifiers to support.

### 4.2 Why the `payloadVersion` field at all?

A common failure mode in cryptographic protocols is the inability
to evolve the envelope without breaking existing signatures. JWS
addresses this by putting the algorithm + type in a signed header.
LearnForge addresses it by putting the version inside the signed
payload itself.

This has three consequences:

1. **The version is signed.** An attacker cannot downgrade a v2
   payload to v1 by editing the version field — the signature
   would fail.
2. **The version is human-readable.** Anyone looking at a decoded
   cert can see `"payloadVersion": 1` in plain text without needing
   to decode the header separately.
3. **The version is dispatched on by the verifier.** The verifier
   reads the version *first*, then dispatches to the version-
   specific verification routine.

The 0/1/2 namespace allocation:

- **v0** — reserved for the Phase 7 stub (`verifier::verify`
  returns `payload_version: 0` + the "not implemented" error). No
  production cert carries v0.
- **v1** — Phase 6 + Phase 7 + Phase 8 certs. Canonical JSON
  payload + Ed25519 signature + hex/base64url transport.
- **v2** — Reserved for Phase 14's possible JWS-EdDSA upgrade.
  Not yet specified.

### 4.3 Why the fingerprint is 8 hex chars (32 bits)?

The fingerprint serves a **UX role**, not a cryptographic role.
Users compare fingerprints to verify "is this the public key I
expected?" before trusting an incoming cert. The full DER-encoded
public key is ~44 bytes; the SHA-256 hash is 32 bytes (64 hex
chars). Neither is comfortable to compare visually.

8 hex chars (32 bits) is:

- **Long enough** to detect accidental key swaps with high
  probability (~1 in 4 billion for random keys).
- **Short enough** to fit comfortably in a UI label, an email
  signature, or a printed share-text template.
- **Aligned** with common practice for "short fingerprint"
  displays (GPG short key IDs were 8 hex chars; GitHub commit
  short SHAs are 7 hex chars).

For high-security key-identity proof (e.g., an offline cold-storage
backup), a longer fingerprint (e.g., 16 or 32 hex chars) would be
preferable. LearnForge's threat model treats fingerprints as a UX
aid, not as a cryptographic primitive, so the short form is
sufficient.

### 4.4 Why canonical JSON instead of MessagePack or CBOR?

Binary serialization formats (MessagePack, CBOR, BSON) sidestep
some of the JSON canonicalization problems by being deterministic
by construction. So why not use one of them?

1. **Human-readability.** Canonical JSON is plain text. A user
   debugging "why won't my cert verify" can decode the base64url
   and read the JSON in any text editor.
2. **Interoperability.** Every language has a JSON library.
   MessagePack has Rust + Python + JS bindings but is less
   ubiquitous.
3. **The signing input is small.** The argument for binary
   formats is size; LearnForge certs are typically 200–400 bytes
   of JSON, well under the QR-code envelope limit. Binary would
   shave maybe 50 bytes per cert — not worth the lost readability.
4. **The Phase 14 hosted verifier might be in any language.**
   JSON is the safe interop choice.

The custom-canonical-JSON path is a deliberate trade-off:
slightly more failure modes than binary, much better readability
and ecosystem support.

### 4.5 Why no key-rotation in 0.1.0?

The per-install Ed25519 keypair lives at
`<app_data>/keys/cert_signing_private.pem` with mode `0600` on
Unix (R3 / Pitfall 4). There is **no built-in mechanism** to
rotate the keypair: a user who suspects key compromise must
manually delete the file (which causes a fresh keypair to be
generated on next launch) and re-issue all certificates with the
new key. The old key's certs become unverifiable against the new
public key.

This is a deliberate 0.1.0 simplification:

1. The threat model (§4.6) treats key compromise as a low-
   probability event — the key never leaves the user's machine.
2. Key rotation introduces complexity (which key signed which
   cert? key rollover transition? CRL?) that the LearnForge
   single-issuer / single-user model does not need.
3. Phase 14's hosted-verifier ecosystem may introduce centralized
   key management that supersedes per-install keys entirely.

Rotation will be added when (a) Phase 14 lands cohort/corporate
use cases, OR (b) a security audit identifies a need for it,
whichever comes first.

### 4.6 Threat model — what signing protects against

The cert-signing pipeline mitigates four threats:

1. **Tampering with cert contents.** An attacker who modifies the
   `level` field of a cert from `Associate` to `Professional`
   invalidates the signature. The verifier rejects.
2. **Replay of stale certs.** A cert's `completionDate` is part of
   the signed payload. An attacker who replays a cert under a new
   date must re-sign — which requires the private key, which never
   leaves the issuer's machine.
3. **Forgery by an untrusted issuer.** A verifier checks the
   signature against an expected `keyFingerprint`. An attacker
   with their own keypair can sign a cert, but their fingerprint
   does not match the expected issuer's fingerprint; the verifier
   rejects.
4. **Forge-by-collision attacks on the fingerprint.** SHA-256 has
   no known collision attacks; the 32-bit truncation reduces the
   collision space to ~4 billion per comparison, sufficient for
   the UX role described in §4.3.

The pipeline does **not** protect against:

- **Issuer key compromise.** If an attacker obtains the issuer's
  private key, they can sign arbitrary certs. Mitigated by:
  the key never leaving the issuer's machine, file-mode-`0600` on
  Unix, per-user app-data ACL on Windows.
- **Issuer is malicious.** The issuer can sign true things; a
  malicious issuer can sign false things. LearnForge's cert
  pipeline does not address "is the issuer trustworthy?" — that
  is a meta-question solved by reputation, out of band.
- **Verifier-side bypass.** A verifier that does not actually
  check the signature is a vulnerability in the verifier, not the
  signer. The cert is only as trustworthy as the verifier
  implementation; Phase 14's hosted verifier is the canonical
  reference.

---

## 5. Implementation notes

### 5.1 Purity, determinism, WASM portability

The two pure crypto functions — `sign_payload` and `verify_payload`
— are pure modulo their input. They have no I/O, no allocation
beyond the returned `Signature` or boolean, and no clock
dependencies. They compile unchanged on `wasm32-unknown-unknown`.

Key **generation** (`SigningKey::generate(&mut OsRng)`) requires an
entropy source. On `wasm32-unknown-unknown` this is wired through
`getrandom 0.3` with the `wasm_js` feature, which dispatches to
`crypto.getRandomValues()` in the browser. The
`Cargo.toml`'s target-conditional block also pulls `getrandom 0.2`
with the `js` feature because `ed25519-dalek 2.x → rand 0.8 →
rand_core 0.6` transitively requires the legacy spelling. This
duplication disappears when `ed25519-dalek 3.x` lands upstream.

Phase 7 Wave 5's `tests/wasm.rs` includes a
`#[wasm_bindgen_test]` that generates an Ed25519 keypair and signs
a fixed payload on the wasm32 target — proving the entropy source
wires through correctly.

### 5.2 The `SigningKeyStore` trait

The per-install keypair lifecycle (generation, persistence,
loading) lives behind a trait:

```rust,ignore
pub trait SigningKeyStore {
    fn get_or_init(&self) -> Result<SigningKey, SigningError>;
    fn export_public_pem(&self) -> Result<String, SigningError>;
}
```

The desktop adapter (`src-tauri/src/storage_impl/signing.rs`)
implements this against the local filesystem: `get_or_init`
reads `<app_data>/keys/cert_signing_private.pem` or generates a
fresh keypair and writes it with file-mode `0600`.

This trait isolation matters because:

1. **WASM has no filesystem.** A web/WASM consumer implements
   `SigningKeyStore` against IndexedDB or another browser-
   accessible store without `learnforge-core` ever importing
   `std::fs`.
2. **Tests inject ephemeral keys.** The unit tests use an
   in-memory implementation that returns a freshly-generated
   keypair on each call, bypassing any filesystem coupling.
3. **Phase 14 hosted verifier needs none of this.** The hosted
   verifier consumes the public key (PEM) over the wire and the
   signature (hex); it does not need to load a private key at
   all, so it does not need a `SigningKeyStore`.

### 5.3 The error envelope

`SigningError` enumerates the failure modes:

```rust,ignore
pub enum SigningError {
    InvalidSignature,
    KeyEncoding(String),
    Io(String),
    Canonical(#[from] CanonicalJsonError),
}
```

`Io` is populated only by the FS-backed implementation; the pure
functions never raise it. `Canonical` wraps the canonical-JSON
error type, allowing `?`-propagation from canonicalization through
to the signing layer.

The `KeyEncoding(String)` variant is intentionally vague — it
covers both malformed PEM input and DER serialization failures.
A more precise enum was considered but rejected: callers either
trust their key source (in which case they don't care about the
distinction) or are debugging (in which case they read the
underlying error string).

### 5.4 No async, no async-trait

Like the rest of `learnforge-core`, the signing module is fully
synchronous. The signing operations are pure CPU work (no I/O at
the algorithm level); the FS-backed key store does its I/O
synchronously via `std::fs`.

A future migration to a fully async desktop runtime would require
an async `SigningKeyStoreAsync` trait. Until then, sync is correct.

---

## 6. Limitations

### 6.1 No certificate revocation list (CRL) or OCSP

A LearnForge cert, once signed, is verifiable forever. There is no
mechanism to revoke a signed cert. If a user later renounces a
cert (e.g., "I plagiarized the proctored lab"), the cryptographic
signature still verifies — only out-of-band trust signals can
mark the cert as revoked.

Phase 14's hosted verifier may add a CRL endpoint (a list of
revoked cert IDs the verifier consults) but this is explicitly out
of scope for the 0.1.0 surface. The signer must NOT assume the
verifier checks a CRL.

### 6.2 No time-bounded validity

A cert has no `expiresAt` field. Once issued, it is valid in
perpetuity. A learner who earned a Kubernetes Professional cert in
2026 still has a verifiable Kubernetes Professional cert in 2036,
regardless of whether the underlying Kubernetes content has
evolved meanwhile.

This is intentional — the cert is a historical record of past
competence (Phase 6 R4 — "achievements row is the historical
proof"). A future extension could add `validUntil` for "skills
expire" scenarios but the current cert format does not.

### 6.3 No key rotation in 0.1.0

As discussed in §4.5, the per-install keypair is fixed. Rotation
requires manually deleting the key file and re-issuing all
certificates. A coordinated key-rollover mechanism is backlogged.

### 6.4 The canonical-JSON subset is not RFC 8785

LearnForge canonicalization is a strict subset of RFC 8785 but
does not claim full compliance. The difference is in number
formatting: RFC 8785 §3.2.2 specifies a strict grammar for
floating-point output that `serde_json` does not natively produce.
LearnForge relies on `serde_json`'s number formatting being
stable across versions — which it is, but it is not RFC 8785 by
specification.

Phase 14 verifier implementations in non-Rust languages MUST
either:
1. Use RFC 8785 number formatting (the safer interop route).
2. Replicate `serde_json`'s formatting exactly (Rust's
   `ryu`-backed floating-point output).

The current `docs/CERT-PAYLOAD-V1.md` spec recommends option 1.

### 6.5 No protection against issuer impersonation

If an attacker generates their own Ed25519 keypair and signs a
forged cert claiming to be LearnForge, the cert verifies
cryptographically — against the attacker's public key. A verifier
MUST check that the `keyFingerprint` in the cert matches the
expected issuer's fingerprint; the cert format itself does not
embed an "issuer identity proof."

In a single-user / single-machine context (per-install LearnForge
desktop), this is moot — there is only one issuer, and the public
key is exported from Settings → "Show signing public key". In a
multi-issuer ecosystem (cohort / corporate use cases) this becomes
material; Phase 11+ may add an issuer-identity layer.

### 6.6 No nonce / replay protection at the signature level

A LearnForge cert can be replayed indefinitely — the verifier has
no way to know "this cert has already been counted." This is
correct behavior for a *certificate* (a permanent record of past
achievement, replayable by definition); it would be incorrect for
a *credential* (a single-use authentication token). LearnForge
treats certs as the former.

If a Phase 14 hosted verifier needs replay protection (e.g., for
a "claim a discount once per cert" use case), it must add its own
out-of-band uniqueness tracking. The cert format provides
`learnerId` + `trackId` + `level` + `completionDate` as candidate
uniqueness keys.

### 6.7 Side-channel resistance not audited

`ed25519-dalek` claims constant-time signing and verification, but
LearnForge has not commissioned an independent side-channel audit
of the integrated pipeline. A determined attacker with timing
access to the LearnForge desktop process could in principle
extract information from key operations. The threat model treats
this as a low-probability scenario (the attacker would already
have local code execution, at which point key compromise is
direct).

### 6.8 No HSM / smart-card key support

The private key lives on disk. A user with a hardware security
module (HSM) or smart card cannot use it as the signing key
source. The `SigningKeyStore` trait would accommodate this — an
HSM-backed `get_or_init` would return a `SigningKey` proxy — but
the implementation requires platform-specific bindings (PKCS#11,
WebAuthn). Backlogged.

---

## 7. References

- **Phase 6 D-05 (LearnForge):** Ed25519 + canonical JSON signing
  pipeline, `payloadVersion` forward-compat, hosted-verifier
  dispatch contract. See `.planning/ROADMAP.md` §Phase 6:
  Certification.
- **Phase 6 R1 (LearnForge):** Canonical-JSON byte-stable
  serialization invariant. The Wave 1 GREEN gate is the
  `canonical_json_byte_stable` test in `canonical_json.rs`.
- **Phase 6 R3 / Pitfall 4 (LearnForge):** Private key stored on
  disk at `<app_data>/keys/cert_signing_private.pem` with
  mode `0600` on Unix.
- **Phase 6 R5 / A7 (LearnForge):** Key fingerprint is first
  8 lowercase hex chars of SHA-256 of the verifying key's DER
  bytes.
- **Phase 7 Wave 5 / 07-05 (LearnForge):** Move of the pure
  signing functions and the canonical-JSON serializer from
  `src-tauri` into `learnforge-core`. The FS-backed key store
  (`FsKeyStore`) stays in `src-tauri` because `std::fs` is not
  WASM-portable.
- **Phase 14 commitment (LearnForge):** Hosted verifier MUST
  honor `payloadVersion: 1` byte-for-byte for legacy certs OR
  migrate them at issuance time. See
  [`docs/CERT-PAYLOAD-V1.md`](../../docs/CERT-PAYLOAD-V1.md).
- **RFC 8032 — Edwards-Curve Digital Signature Algorithm (EdDSA).**
  Josefsson, S., & Liusvaara, I. (2017). The canonical Ed25519
  specification. https://www.rfc-editor.org/rfc/rfc8032
- **RFC 8785 — JSON Canonicalization Scheme (JCS).** Rundgren, A.,
  Jordan, B., & Erdtman, S. (2020). The IETF canonical-JSON
  serialization specification.
  https://www.rfc-editor.org/rfc/rfc8785
- **RFC 7515 — JSON Web Signature (JWS).** Jones, M., Bradley, J.,
  & Sakimura, N. (2015). The off-the-shelf alternative LearnForge
  evaluated and rejected for the reasons in §2.
- **RFC 8037 — CFRG Elliptic Curve Diffie-Hellman (ECDH) and
  Signatures in JSON Object Signing and Encryption (JOSE).**
  Liusvaara, I. (2017). The EdDSA + JWS integration spec.
- **RFC 4648 — The Base16, Base32, and Base64 Data Encodings.**
  Josefsson, S. (2006). Base64url (§5) is the URL-safe encoding
  used in cert transport.
- **Bernstein, D. J., Duif, N., Lange, T., Schwabe, P., & Yang,
  B.-Y. (2012).** High-speed high-security signatures. *Journal
  of Cryptographic Engineering*, 2(2), 77-89. The original
  Ed25519 paper.
- **Aumasson, J.-P. (2019).** *Serious Cryptography: A Practical
  Introduction to Modern Encryption.* No Starch Press. Chapter
  on signatures for accessible Ed25519 background.
- **`ed25519-dalek` Rust crate.** RustCrypto project.
  https://crates.io/crates/ed25519-dalek — the implementation
  LearnForge depends on.
- **LearnForge whitepaper:** [BKT](./BKT.md) — produces the
  mastery scores that end up in signed cert payloads.
- **LearnForge whitepaper:** [SM2](./SM2.md) — feeds review
  signals into mastery calculations.
- **LearnForge whitepaper:** [THRESHOLD](./THRESHOLD.md) —
  the tier predicates that trigger cert issuance.
- **LearnForge whitepaper:** [MICROLEARNING](./MICROLEARNING.md)
  — feeds practice events into the BKT pipeline.

---

## 8. Reproducing the worked example

```rust,ignore
use ed25519_dalek::{SigningKey, pkcs8::EncodePublicKey};
use learnforge_core::canonical_json::canonical_json_bytes;
use learnforge_core::signing::{sign_payload, verify_payload,
                                public_key_fingerprint};
use rand::rngs::OsRng;

let key = SigningKey::generate(&mut OsRng);

#[derive(serde::Serialize)]
struct CertPayload<'a> {
    learner: &'a str,
    track: &'a str,
    level: &'a str,
    masteryScore: f64,
    payloadVersion: u32,
}
let payload = CertPayload {
    learner: "Alice",
    track: "Kubernetes",
    level: "Professional",
    masteryScore: 0.87,
    payloadVersion: 1,
};

let canonical_bytes = canonical_json_bytes(&payload).unwrap();
let sig = sign_payload(&key, &canonical_bytes);
let sig_hex = hex::encode(sig.to_bytes());

let pub_pem = key.verifying_key()
    .to_public_key_pem(pkcs8::LineEnding::LF).unwrap();

// Real verify
assert!(verify_payload(&pub_pem, &canonical_bytes, &sig_hex));

// Tampering — flip one byte
let mut tampered = canonical_bytes.clone();
tampered[0] ^= 0x01;
assert!(!verify_payload(&pub_pem, &tampered, &sig_hex));

// 8-char fingerprint
let fp = public_key_fingerprint(&key.verifying_key());
assert_eq!(fp.len(), 8);
```

The unit tests in `learnforge-core/src/signing.rs` (10 tests
covering sign/verify roundtrip, single-byte-flip tampering,
signature corruption, malformed PEM rejection, fingerprint
stability, fingerprint roundtrip via PEM, fingerprint divergence
across keys, share-text template, share-text no-emoji guarantee,
and error rendering) and `learnforge-core/src/canonical_json.rs`
(4 tests covering byte-stable output, non-finite float rejection,
empty container handling, and array order preservation) serve as
the executable specification for the pipeline.

---

*This whitepaper is licensed under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). You may
reuse it with attribution to "LearnForge OSS contributors, 2026".*
