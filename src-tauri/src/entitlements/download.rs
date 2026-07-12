// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)

//! Download the buyer-stamped signed pack from the redeem response's
//! `downloadUrl`, retain the artifact under `app_data_dir` (D-07), and
//! import it through the existing fail-closed gate (`import_course_impl`).
//!
//! Wave 0 (15-01): `download_and_store` is a compiling `unimplemented!`
//! stub. 15-02 fills the body following the reqwest GET + retained-artifact
//! write pattern (`app_data_dir.join("entitlements").join("{pack_id}.json")`,
//! 15-PATTERNS.md "Retained-artifact write under app_data_dir").
//!
//! 15-03 adds the `sanitize_pack_id` path-traversal guard (T-15-08) — see
//! `pack_id_with_path_separators_rejected_before_download` below, the named
//! RED starting point for that guard. This Wave 0 plan does NOT add the
//! guard itself, only the failing test that names it.

use super::RedeemLicenseError;

/// Download the buyer-stamped pack from `download_url`, write it to the
/// retained-artifact path under `app_data_dir` (D-07), and return that path.
/// 15-02 implements the real reqwest GET + file write; this Wave 0 stub is a
/// named pending assertion.
pub async fn download_and_store(
    _download_url: &str,
    _pack_id: &str,
    _app_data_dir: &std::path::Path,
) -> Result<String, RedeemLicenseError> {
    unimplemented!("15-02")
}

#[cfg(test)]
mod tests {
    /// ENT-04 — previously redeemed packs re-import from the retained
    /// artifact (D-07) with ZERO network calls (offline-first, no
    /// per-launch phone-home). 15-02 GREEN target: reimport must succeed
    /// purely from the on-disk retained artifact + local
    /// `pack_trust::verify_pack`, no `reqwest` call in the path.
    ///
    /// Marked `#[ignore]` + `todo!()` body so it compiles as a named
    /// pending assertion rather than a hard RED failure at Wave 0 (mirrors
    /// the reimport scaffold convention) — the assertion itself lands in
    /// 15-02.
    #[test]
    #[ignore = "15-02 GREEN target"]
    fn reimport_from_retained_artifact_requires_no_network() {
        todo!()
    }

    /// T-15-08 (Tampering) — a `pack_id` containing a path separator (`/`,
    /// `\`), a `..` traversal segment, a leading separator, or an absolute
    /// path must be REJECTED before any write/GET happens in
    /// `download_and_store`. This is the RED starting point for the 15-03
    /// `sanitize_pack_id` traversal guard — pinned here from Wave 0 so that
    /// guard is TDD-driven, not bolted on after the fact.
    ///
    /// Marked `#[ignore]` + `todo!("15-03")` so it compiles as a named
    /// pending assertion; the guard itself (and this test's real body) is
    /// 15-03's GREEN step, not this plan's.
    #[test]
    #[ignore = "15-03 GREEN target"]
    fn pack_id_with_path_separators_rejected_before_download() {
        todo!("15-03")
    }
}
