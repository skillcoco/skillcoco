# SkillCoco Certificate Payload — v1

**Status:** Active for Phase 6.
**Authored:** 2026-06-16 (Phase 6 Plan 06-01 / Wave 0).
**Phase 14 commitment:** Hosted verifier MUST honor v1 byte-for-byte for
legacy certs OR migrate them at issuance time (R5 — Pitfall 7).

## Purpose

This document is the canonical contract for the Phase 6 signed-certificate
payload format. It exists to prevent the failure mode described in Phase 6
RESEARCH.md Pitfall 7: Phase 14 ships a hosted verifier that emits a
different envelope and every legacy Phase 6 cert breaks.

Two consumers MUST reproduce the byte sequences described here:
1. The **Phase 6 desktop signer** (`src-tauri/src/achievements/signing.rs`).
2. The **Phase 14 hosted verifier service** (TBD).

If those two ever disagree on byte ordering, encoding, or field schema,
every certificate issued under this contract becomes unverifiable. Treat
this document as load-bearing.

## Payload Version

```
payloadVersion: 1
```

`payloadVersion` is a u32 dispatch tag inside every v1 payload. Phase 14
introduces `2` if/when the hosted verifier switches the envelope to
JWS-EdDSA. Phase 14 verifier MUST accept v1 payloads even after v2 ships
(either by parsing them directly or by performing a one-time re-sign
migration; the choice is Phase 14's, but support for legacy v1 is
mandatory).

## Field Schema

| field | type | required | notes |
|---|---|---|---|
| `learner` | string | yes | `display_name` from `learner_profiles` at issuance time (snapshot, not live) |
| `learnerId` | string | yes | UUID — kept opaque; Phase 11+ may swap for a pseudonym |
| `track` | string | yes | Track topic snapshot at issuance (R4 — survives track deletion) |
| `trackId` | string | yes | UUID — for audit only, NOT used to look up live track state |
| `level` | string | yes | One of `Associate` / `Practitioner` / `Professional` / `Completion` |
| `completionDate` | string | yes | ISO 8601 UTC, e.g. `2026-06-16T14:32:11Z` |
| `masteryScore` | number | yes | Finite IEEE 754 f64 in `[0.0, 1.0]`; encoded as JSON number (no quoting) |
| `keyFingerprint` | string | yes | First 8 hex chars of SHA-256(verifying_key DER bytes). Lowercase. |
| `packId` | string \| null | optional | Source topic-pack id snapshot, or `null` for AI-generated tracks |
| `payloadVersion` | integer | yes | `1` for Phase 6 |

All keys are camelCase. All values are JSON-typed; no field is base64'd
inside the JSON object. Producers MUST include every field marked
`required: yes`. Consumers MUST reject payloads missing any required field.

## Canonical JSON

The bytes Ed25519 signs are NOT the natural `serde_json::to_string` output.
They are the **canonical JSON** form — object keys sorted lexicographically
at every nesting level — to make the signing input byte-identical across
runs, machines, serde versions, and the Phase 14 hosted verifier.

**Canonicalization algorithm:**

1. Serialize the payload struct to `serde_json::Value`.
2. Re-emit the value with all object keys sorted lexicographically. The
   implementation reference uses `serde_json::Map<String, Value>` with
   manual key sort, then `serde_json::to_vec(&sorted_value)`.
3. UTF-8 encode the resulting JSON. The resulting `Vec<u8>` is the
   "canonical bytes" the signer feeds to `ed25519_dalek::Signer::sign`.

**Why this matters:** `serde_json`'s natural output order is
insertion-order (a property of `serde_json::Map` backed by either
`BTreeMap` or `Vec` depending on feature flags). Two app builds with
slightly different feature flag sets WILL produce non-identical bytes for
the same logical payload — every signature will differ; verification will
fail for half the user base.

**Determinism test:** A unit test in
`src-tauri/src/achievements/signing.rs` (`canonical_json_byte_stable`)
asserts two calls with the same input produce byte-identical `Vec<u8>`.
That test is the Wave 1 GREEN gate for this contract.

## Encoding

After canonicalization + signing:

1. **canonical_bytes** — UTF-8 JSON, sorted keys (per "Canonical JSON" above).
2. **signature** — 64-byte raw Ed25519 signature (RFC 8032), hex-encoded
   lowercase (128 chars). The Achievement row stores this in the
   `signature` column.
3. **QR content** — `<base64url(canonical_bytes)>.<hex(signature)>`,
   single dot, no whitespace. Mimics the JWS compact form visually but is
   NOT a JWS (no algorithm header, no JSON metadata). Phase 14 may
   upgrade to a real JWS-EdDSA envelope; v1 stays as-is for legacy
   compatibility.
4. **Base64url** — RFC 4648 §5, no padding (`=` characters stripped).

## Forward-Compat Promise

Phase 14 (hosted verifier service) MUST recognize `payloadVersion: 1`
inside the decoded base64url payload and dispatch to v1 verification. The
allowed migrations are:

1. **Direct v1 parsing** — verifier reproduces the canonical bytes,
   verifies the signature, and renders the certificate metadata as if
   served from the desktop verifier.
2. **One-time re-sign** — verifier (or a migration service) decodes the
   v1 payload, repackages it as v2 (e.g., a JWS-EdDSA envelope), and
   signs with a service-managed key. The original v1 payload MUST still
   be reproducible from the v2 envelope (e.g., stored as a JWS claim) so
   the chain of custody is auditable.

A re-sign migration MUST run dual-format issuance for the duration of the
cohort transition (recommend: 6 months minimum). v1 verification MUST NOT
be removed from the hosted verifier before that window expires.

## Security Notes

- The Ed25519 private key NEVER crosses the IPC boundary. It is stored on
  disk at `<app_data>/keys/cert_signing_private.pem` with mode 0600 on
  Unix (R3 / Pitfall 4). Windows relies on per-user app-data ACLs.
- `keyFingerprint` is the truncated SHA-256 of the verifying key's DER
  bytes (first 8 hex chars) and is safe to display in UI / share
  publicly. The full verifying key (PEM) is exportable from Settings →
  "Show signing public key" for users who want to verify offline.
- Mastery decay does NOT revoke an issued cert (D-04, R4 — the
  `achievements` table is the historical record). The QR's
  `masteryScore` is a **snapshot** at issuance time, not a live value.
- Achievement rows are immutable once written. `UNIQUE (learner_id,
  track_id, level)` on the achievements table makes re-issuance a
  no-op (INSERT OR IGNORE).
- Phase 6 ships NO automatic publication of any achievement (D-07).
  Sharing happens only when the learner manually exports and posts
  elsewhere.
- Track deletion does NOT cascade to achievements (R4 — Pitfall 5). A
  deleted track leaves its certs intact and they remain readable
  because `track_topic` is snapshotted on the achievement row.

## Test Vector (Phase 14 reference)

Phase 14 verifier developers can sanity-check their canonicalization +
verification pipeline against this test vector. Wave 2 (Plan 06-03) will
publish a real signed payload once the signer lands; Wave 0 leaves the
vector unspecified deliberately — the contract above is the source of
truth.

```
TBD — Wave 2 (Plan 06-03) publishes:
  - canonical_bytes hex
  - signature hex
  - keyFingerprint
  - matching public_pem
```

---

*Phase: 06-certification*
*Plan: 06-01 (Wave 0 — TDD scaffolds)*
*Audience: Phase 14 hosted-verifier authors + future SkillCoco security audits*
