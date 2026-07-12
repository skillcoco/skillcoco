# entitlements test fixtures (Phase 15)

Unlike `learnforge-core/tests/fixtures/pack_trust/` (Phase 14), this directory
holds **no static signed JSON files**. The ENT-02 buyer-stamped `licensed:`
pack fixture is generated **in-code** via
`src-tauri/src/entitlements/mod.rs::test_support::signed_licensed_pack_fixture`,
which reuses the same Ed25519 keypair-generation helpers as
`learnforge-core/src/pack_trust.rs`'s own `#[cfg(test)]` module (root +
issuer keypair, `pack_trust::jcs_bytes` + `signing::sign_payload`).

## Why generated, not static

- The real production root PEM's private half is held offline — there is no
  committed root private key in this repo (see
  `learnforge-core/tests/fixtures/pack_trust/README.md`), so a static
  buyer-stamped fixture signed by the production root cannot be committed.
- The pack_trust test convention (proven in `pack_trust.rs` and
  `course_io.rs`) is to generate a fresh test root+issuer keypair per test
  run and sign fixtures with it, then call `pack_trust::verify_pack`
  directly against that test root — never `pack_trust::BUNDLED_ROOT_PUBLIC_PEM`,
  which only a fixture signed by the (offline) real root key could satisfy.
- Regenerating the signature per call also means the fixture can never go
  stale relative to the current JCS canonicalization implementation.

## What the fixture builder produces

`signed_licensed_pack_fixture(pack_id, buyer_name, order_id)` returns
`(root_pem, signed_pack_json)` where `signed_pack_json`:

- has `exportedFrom` starting with `licensed:` (D-11 provenance preserved),
- carries a `"Licensed to {buyer_name}, order #{order_id}"` watermark string
  in the pack body (inside the signed region, per D-01/D-03/D-04),
- verifies successfully via `pack_trust::verify_pack(&root_pem, &pack)`.

## Consumers

- `src-tauri/src/entitlements/mod.rs::tests::redeem_downloaded_licensed_pack_imports_with_provenance_preserved`
  (RED at Wave 0 — 15-02 extends this into a full
  redeem -> download -> `import_course_impl` integration test).
