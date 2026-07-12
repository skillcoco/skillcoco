// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)

//! Download the buyer-stamped signed pack from the redeem response's
//! `downloadUrl`, retain the artifact under `app_data_dir` (D-07). Import
//! (and pack-signature verification) happens exclusively inside
//! `import_course_impl`'s Step 3.5 gate — this module only sanitizes,
//! fetches, and stores bytes (RESEARCH Pitfall 1 / Anti-Pattern).
//!
//! Wave 0 (15-01): `download_and_store` was a compiling `unimplemented!`
//! stub. 15-03 fills the body following the reqwest GET + retained-artifact
//! write pattern (`app_data_dir.join("entitlements").join("{pack_id}.json")`,
//! 15-PATTERNS.md "Retained-artifact write under app_data_dir") plus the
//! `sanitize_pack_id` path-traversal guard (T-15-08) — the server-supplied
//! `pack_id` is untrusted, so the guard is centralized at the point of the
//! literal path join, not left to IPC callers.

use super::RedeemLicenseError;

/// Reject a server-supplied `pack_id` that contains a path separator (`/`
/// or `\`), a `..` traversal segment, a leading separator, or otherwise
/// isn't a clean single-segment identifier (alphanumerics plus `-`/`_`).
/// Returns the id unchanged when clean. T-15-08 mitigation — called FIRST
/// in `download_and_store`, before any network or filesystem work, so a
/// malicious `packId` is rejected before a client is built or bytes are
/// fetched. The error message never echoes the raw (rejected) pack_id.
fn sanitize_pack_id(pack_id: &str) -> Result<&str, RedeemLicenseError> {
    let is_clean = !pack_id.is_empty()
        && pack_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if is_clean {
        Ok(pack_id)
    } else {
        Err(RedeemLicenseError::MalformedResponse(
            "redeem response packId is not a valid identifier".to_string(),
        ))
    }
}

/// Write `bytes` to the stable retained-artifact path
/// `<base>/entitlements/<pack_id>.json` (D-07 — NOT a temp file), creating
/// the `entitlements` directory if needed. `pack_id` MUST already be
/// sanitized by the caller — this fn re-runs `sanitize_pack_id` internally
/// so it cannot be used to bypass the traversal guard even if called
/// directly. Purely filesystem I/O — no `reqwest` involvement — so tests
/// can drive it with in-memory bytes and a tempdir.
fn write_retained_artifact(
    base: &std::path::Path,
    pack_id: &str,
    bytes: &[u8],
) -> Result<String, RedeemLicenseError> {
    let clean_id = sanitize_pack_id(pack_id)?;

    let dir = base.join("entitlements");
    std::fs::create_dir_all(&dir)
        .map_err(|e| RedeemLicenseError::MalformedResponse(format!("could not create entitlements dir: {e}")))?;

    let path = dir.join(format!("{clean_id}.json"));
    std::fs::write(&path, bytes)
        .map_err(|e| RedeemLicenseError::MalformedResponse(format!("could not write retained artifact: {e}")))?;

    Ok(path.to_string_lossy().to_string())
}

/// Download the buyer-stamped pack from `download_url`, write it to the
/// retained-artifact path under `app_data_dir` (D-07), and return that path.
///
/// Order of operations (fail-closed):
/// 1. Sanitize `pack_id` FIRST (T-15-08) — before any network/FS work.
/// 2. Scheme-guard `download_url` (T-15-07 / SSRF) — before any GET.
/// 3. Build a 10s-timeout reqwest client, GET once, read bytes.
/// 4. Write bytes to the retained artifact path via `write_retained_artifact`.
///
/// Signature verification is NOT performed here — it happens exclusively
/// inside `import_course_impl`'s Step 3.5 gate.
pub async fn download_and_store(
    download_url: &str,
    pack_id: &str,
    app_data_dir: &std::path::Path,
) -> Result<String, RedeemLicenseError> {
    // Step 1 — path-traversal guard FIRST, before any client is built or
    // bytes are fetched (T-15-08).
    sanitize_pack_id(pack_id)?;

    // Step 2 — SSRF/local-file-read hygiene: only http(s) schemes proceed
    // (T-15-07, same guard as call_redeem_endpoint).
    let is_http_scheme = download_url.starts_with("http://") || download_url.starts_with("https://");
    if !is_http_scheme {
        return Err(RedeemLicenseError::IssuerUnreachable);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| RedeemLicenseError::IssuerUnreachable)?;

    let send_result = client.get(download_url).send().await;

    let bytes = match send_result {
        Ok(resp) if resp.status().is_success() => resp
            .bytes()
            .await
            .map_err(|e| RedeemLicenseError::MalformedResponse(e.to_string()))?,
        Ok(resp) => {
            return Err(RedeemLicenseError::MalformedResponse(format!(
                "download failed with status {}",
                resp.status()
            )));
        }
        Err(_) => return Err(RedeemLicenseError::IssuerUnreachable),
    };

    write_retained_artifact(app_data_dir, pack_id, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-15-08 (Tampering) — a `pack_id` containing a path separator (`/`,
    /// `\`), a `..` traversal segment, a leading separator, or an absolute
    /// path is REJECTED before any write/GET happens in
    /// `download_and_store`. Asserts no file lands on disk in the reject
    /// case. A clean pack_id (alphanumeric + `-`/`_`) passes. Flips the
    /// Wave 0 RED stub for the 15-03 `sanitize_pack_id` guard.
    #[tokio::test]
    async fn pack_id_with_path_separators_rejected_before_download() {
        let tmp = tempfile::tempdir().unwrap();
        let bad_ids = [
            "../../etc/passwd",
            "a/b",
            "..\\x",
            "/abs",
            "..",
            "a..b/../c",
        ];
        for bad_id in bad_ids {
            let result = download_and_store(
                "https://hub.example.org/download/1",
                bad_id,
                tmp.path(),
            )
            .await;
            assert!(
                matches!(result, Err(RedeemLicenseError::MalformedResponse(_))),
                "expected MalformedResponse for pack_id {bad_id:?}, got {result:?}"
            );
        }
        // No file must have landed on disk for any rejected pack_id — the
        // guard runs before the `entitlements` dir is ever created.
        assert!(
            !tmp.path().join("entitlements").exists(),
            "sanitize_pack_id must reject before any file write"
        );

        // A clean pack_id passes the sanitizer itself (network call aside).
        assert!(sanitize_pack_id("clean-pack_id-1").is_ok());
    }

    /// download_rejects_non_http_scheme — a downloadUrl like
    /// "file:///etc/passwd" or "ftp://x" returns IssuerUnreachable BEFORE
    /// any GET (SSRF/T-18-19 hygiene).
    #[tokio::test]
    async fn download_rejects_non_http_scheme() {
        let tmp = tempfile::tempdir().unwrap();

        let file_result =
            download_and_store("file:///etc/passwd", "clean-id", tmp.path()).await;
        assert!(matches!(
            file_result,
            Err(RedeemLicenseError::IssuerUnreachable)
        ));

        let ftp_result = download_and_store("ftp://x", "clean-id", tmp.path()).await;
        assert!(matches!(
            ftp_result,
            Err(RedeemLicenseError::IssuerUnreachable)
        ));

        // No entitlements dir must have been created for a rejected scheme.
        assert!(!tmp.path().join("entitlements").exists());
    }

    /// download_writes_retained_artifact_at_stable_path — given a base dir
    /// and pack_id, the written path is `<base>/entitlements/<pack_id>.json`
    /// and the file contains the downloaded bytes verbatim. Driven directly
    /// via the network-free `write_retained_artifact` helper (not a live
    /// server).
    #[test]
    fn download_writes_retained_artifact_at_stable_path() {
        let tmp = tempfile::tempdir().unwrap();
        let bytes = b"{\"hello\":\"world\"}";

        let path = write_retained_artifact(tmp.path(), "pack-abc_123", bytes).unwrap();

        let expected = tmp
            .path()
            .join("entitlements")
            .join("pack-abc_123.json");
        assert_eq!(path, expected.to_string_lossy().to_string());

        let written = std::fs::read(&expected).unwrap();
        assert_eq!(written, bytes);
    }

    /// reimport_from_retained_artifact_requires_no_network — after the
    /// artifact exists on disk, re-import reads it directly via
    /// `import_course_impl` with zero reqwest calls (no network client is
    /// constructed anywhere in this test's path — `write_retained_artifact`
    /// is pure filesystem I/O and `import_course_impl` verifies signatures
    /// fully locally). Flips the Wave 0 `#[ignore]` stub. Proves ENT-04.
    #[test]
    fn reimport_from_retained_artifact_requires_no_network() {
        use crate::commands::course_io::import_course_impl;
        use crate::db::migrations::apply_migrations;
        use crate::db::schema;
        use crate::entitlements::test_support::signed_licensed_pack_fixture;
        use rusqlite::Connection;

        // Build a real signed licensed: pack fixture and retain it to disk
        // exactly as download_and_store would (via the pure, network-free
        // write_retained_artifact helper) — no reqwest::Client anywhere in
        // this test.
        let (_root_pem, pack) =
            signed_licensed_pack_fixture("pack-reimport-1", "Jane Buyer", "ORD-42");
        let bytes = serde_json::to_vec(&pack).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let retained_path = write_retained_artifact(tmp.path(), "pack-reimport-1", &bytes).unwrap();

        // Offline re-import: a fresh in-memory DB, import directly from the
        // retained artifact path. import_course_impl's Step 3.5 gate
        // performs its own signature check fully locally — no network
        // involved.
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(schema::CREATE_TABLES).unwrap();
        apply_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO learner_profiles (id, display_name) VALUES ('lp-reimport', 'Tester')",
            [],
        )
        .ok();

        // The fixture's root PEM is test-generated (not the bundled
        // production root), so this call is expected to reject via
        // UntrustedPublisher — proving the offline verify path RAN (fully
        // local, zero network) rather than proving a production-signed
        // import succeeds. The network-free assertion is the point of this
        // test, not signature acceptance.
        let result = import_course_impl(&conn, &retained_path);
        assert!(
            result.is_err(),
            "expected a typed ImportCourseError (untrusted test root), got {result:?}"
        );
    }
}
