//! Transitional shim — Phase 7 Wave 5 (07-05) moved the pure crypto
//! primitives to `learnforge_core::{canonical_json, signing}` and the
//! FS-backed key store to `crate::storage_impl::signing::FsKeyStore`
//! (D-03 amendment + Pitfall 4 lock).
//!
//! This file is now a thin compatibility layer:
//!
//! - **Pure surface** is re-exported from `learnforge_core` so existing
//!   callers (`achievements::mod::maybe_issue` +
//!   `commands::achievements`) compile unchanged.
//! - **FS-backed wrappers** (`get_or_init_key`, `read_public_pem`) keep
//!   their pre-Wave-5 signatures returning `AchievementError` so the `?`
//!   propagation in `maybe_issue` and the `.map_err(|e| e.to_string())`
//!   in the IPC handlers don't have to change.
//! - **`canonical_json_bytes` wrapper** keeps the `AchievementError`
//!   return type so `signing::canonical_json_bytes(&payload)?` continues
//!   to propagate naturally inside `build_signed_achievement`.
//!
//! No `#[deprecated]` — rustc silently ignores it on `pub use` (R5 /
//! Pitfall 6 from 07-RESEARCH.md). Wave 10 grep-and-rewrite is the
//! eventual cleanup target — at that point the achievements module
//! migrates onto `learnforge_core::signing` + `FsKeyStore` directly and
//! this shim deletes.

use super::AchievementError;
use ed25519_dalek::SigningKey;
use serde::Serialize;
use std::path::Path;

// ── Re-exports of the pure surface ─────────────────────────────────────────

pub use learnforge_core::canonical_json::{canonicalize, CanonicalJsonError};
pub use learnforge_core::signing::{
    fingerprint_from_public_pem, public_key_fingerprint, share_text, sign_payload, verify_payload,
    SigningError, SigningKeyStore,
};

// FsKeyStore lives next to the other rusqlite-adjacent impls in
// storage_impl, but re-export it from here for callsites that already
// reach in through `achievements::signing::*`.
pub use crate::storage_impl::signing::FsKeyStore;

// ── Error envelope conversions ─────────────────────────────────────────────
//
// `From<SigningError> for AchievementError` and `From<CanonicalJsonError>
// for AchievementError` now live in `learnforge_core::achievements`
// (Wave 8). They moved with the type. This file imports + uses them.

// ── Legacy FS-backed wrappers (delegate to FsKeyStore) ─────────────────────

/// Legacy wrapper preserving the pre-Wave-5 signature
/// `Result<SigningKey, AchievementError>` so
/// `achievements::mod::maybe_issue` continues to compile unchanged.
///
/// Internally constructs an [`FsKeyStore`] for the supplied directory and
/// dispatches through the [`SigningKeyStore`] trait. The 0o600 file-mode
/// invariant on Unix lives in the trait impl body (verbatim from the
/// pre-Wave-5 src-tauri implementation).
pub fn get_or_init_key(key_dir: &Path) -> Result<SigningKey, AchievementError> {
    FsKeyStore::new(key_dir.to_path_buf())
        .get_or_init()
        .map_err(AchievementError::from)
}

/// Legacy wrapper preserving the pre-Wave-5 signature
/// `Result<String, AchievementError>`. Powers the
/// `get_signing_public_key` IPC handler.
pub fn read_public_pem(key_dir: &Path) -> Result<String, AchievementError> {
    FsKeyStore::new(key_dir.to_path_buf())
        .export_public_pem()
        .map_err(AchievementError::from)
}

// ── Legacy canonical_json_bytes wrapper ────────────────────────────────────

/// Legacy wrapper around [`learnforge_core::canonical_json::canonical_json_bytes`]
/// returning `Result<Vec<u8>, AchievementError>` so the `?` propagation in
/// `build_signed_achievement` continues to work without manual error
/// conversion.
pub fn canonical_json_bytes<T: Serialize>(payload: &T) -> Result<Vec<u8>, AchievementError> {
    learnforge_core::canonical_json::canonical_json_bytes(payload).map_err(AchievementError::from)
}
