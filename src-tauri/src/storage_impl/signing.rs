//! `FsKeyStore` — filesystem-backed [`SigningKeyStore`] implementation.
//!
//! Phase 7 Wave 5 (07-05): the FS-backed half of `achievements::signing`
//! moved here so `learnforge-core::signing` stays pure (no `std::fs`, no
//! `std::path::Path`) and the WASM build of `learnforge-core` compiles
//! cleanly. D-03 amendment + Pitfall 4 lock — `std::fs` is not
//! WASM-portable.
//!
//! ## Security invariants (preserved verbatim from Phase 6)
//!
//! - **R3 / Pitfall 4 / V6 ASVS** — On Unix, the private-key file is
//!   `chmod 0o600` immediately after write.
//! - **Lazy init (Pattern 2)** — A fresh keypair is generated only on the
//!   first call; subsequent calls re-read the persisted PEM.
//! - The private key never crosses the IPC boundary. `FsKeyStore` signs the
//!   local completion badge; the `export_public_pem` accessor remains for the
//!   `SigningKeyStore` trait contract.
//!
//! The body of `get_or_init` is lifted **verbatim** from
//! `src-tauri/src/achievements/signing.rs:45-82` (pre-Wave-5 snapshot); the
//! body of `export_public_pem` is lifted verbatim from lines 84-89.
//! Behavioral equivalence is guaranteed by the existing Phase 6 test
//! suite — `private_key_file_mode_0600` + `generate_then_load` + every
//! achievements integration test exercises this code through the shim.

use ed25519_dalek::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey};
use ed25519_dalek::SigningKey;
use learnforge_core::signing::{SigningError, SigningKeyStore};
use pkcs8::LineEnding;
use rand::rngs::OsRng;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// File name (relative to the keys directory) for the private signing key.
const PRIV_FILE: &str = "cert_signing_private.pem";
/// File name (relative to the keys directory) for the public signing key.
const PUB_FILE: &str = "cert_signing_public.pem";

/// Filesystem-backed [`SigningKeyStore`].
///
/// Caller supplies the directory; `FsKeyStore` owns the two file names
/// (`cert_signing_private.pem` + `cert_signing_public.pem`) inside it.
///
/// ## Path layout
///
/// ```text
/// <key_dir>/
///   ├─ cert_signing_private.pem   # 0600 on Unix
///   └─ cert_signing_public.pem    # world-readable
/// ```
pub struct FsKeyStore {
    /// Directory containing the two PEM files. Caller is responsible for
    /// choosing an isolated location (per-user app-data on every
    /// platform).
    pub key_dir: PathBuf,
}

impl FsKeyStore {
    /// Construct from any path-like value.
    pub fn new(key_dir: impl Into<PathBuf>) -> Self {
        Self {
            key_dir: key_dir.into(),
        }
    }

    fn priv_path(&self) -> PathBuf {
        self.key_dir.join(PRIV_FILE)
    }

    fn pub_path(&self) -> PathBuf {
        self.key_dir.join(PUB_FILE)
    }
}

impl SigningKeyStore for FsKeyStore {
    /// Load the per-install signing key, generating a fresh keypair (and
    /// writing both PEMs to disk with 0o600 perms on Unix) on first call.
    ///
    /// **Body lifted verbatim from `src-tauri/src/achievements/signing.rs:45-82`
    /// (pre-Wave-5 snapshot).** Only the error envelope changed —
    /// `AchievementError` → [`SigningError`] — so the trait surface stays
    /// algorithm-crate-pure. The src-tauri shim re-wraps to
    /// `AchievementError` for the existing `maybe_issue` callsite.
    fn get_or_init(&self) -> Result<SigningKey, SigningError> {
        let priv_p = self.priv_path();

        if priv_p.exists() {
            let pem = std::fs::read_to_string(&priv_p)
                .map_err(|e| SigningError::Io(format!("read private pem: {}", e)))?;
            let key = SigningKey::from_pkcs8_pem(&pem)
                .map_err(|e| SigningError::KeyEncoding(format!("decode private pem: {}", e)))?;
            return Ok(key);
        }

        // Fresh keypair path.
        std::fs::create_dir_all(&self.key_dir)
            .map_err(|e| SigningError::Io(format!("create_dir_all keys: {}", e)))?;
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);

        // Encode + write the private PEM.
        let priv_pem = signing_key
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|e| SigningError::KeyEncoding(format!("encode private pem: {}", e)))?;
        std::fs::write(&priv_p, priv_pem.as_bytes())
            .map_err(|e| SigningError::Io(format!("write private pem: {}", e)))?;

        // R3 / Pitfall 4 — enforce 0600 on Unix immediately after write.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&priv_p, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| SigningError::Io(format!("chmod 0600 private pem: {}", e)))?;
        }

        // Encode + write the public PEM. World-readable on disk is acceptable
        // (verifying keys are public information).
        let pub_pem = signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| SigningError::KeyEncoding(format!("encode public pem: {}", e)))?;
        std::fs::write(self.pub_path(), pub_pem)
            .map_err(|e| SigningError::Io(format!("write public pem: {}", e)))?;

        Ok(signing_key)
    }

    /// Read the on-disk public-key PEM (powers the Settings "Show signing
    /// public key" IPC).
    ///
    /// **Body lifted verbatim from `src-tauri/src/achievements/signing.rs:84-89`
    /// (pre-Wave-5 snapshot).**
    fn export_public_pem(&self) -> Result<String, SigningError> {
        std::fs::read_to_string(self.pub_path())
            .map_err(|e| SigningError::Io(format!("read public pem: {}", e)))
    }
}

/// Resolve the private-key path inside the given keys directory.
///
/// Helper exposed so external callers can stat the file (e.g., the
/// `private_key_file_mode_0600` test in the shim's tests module needs to
/// peek at the file's permissions without going through the trait).
pub fn priv_path(key_dir: &Path) -> PathBuf {
    key_dir.join(PRIV_FILE)
}

/// Resolve the public-key path inside the given keys directory. Symmetric
/// counterpart of [`priv_path`].
pub fn pub_path(key_dir: &Path) -> PathBuf {
    key_dir.join(PUB_FILE)
}

// ── Process-level mutex-cached key store ─────────────────────────────────
//
// The Tauri runtime stores the lazy-loaded `SigningKey` in
// `AppState.signing_key: Arc<Mutex<Option<SigningKey>>>`. Phase 6's
// pre-Wave-7 implementation handled the lazy-init dance in a
// `get_or_load_into_mutex` helper inside `src-tauri/src/achievements/mod.rs`.
// Wave 7 wrapped that helper in a `MutexCachedKeyStore` newtype living in
// the `achievements/mod.rs` shim so the legacy `maybe_issue(conn, track,
// learner, &Mutex, &Path)` callsite kept its pre-Wave-5 shape.
//
// Wave 10 deletes the `achievements/mod.rs` shim and lifts the
// `MutexCachedKeyStore` into this module. It is the production
// `SigningKeyStore` impl for the Tauri binary: every IPC handler that
// triggers `maybe_issue` constructs one with the `AppState`-owned cache +
// the on-disk key directory and passes it as the `key_store: &dyn
// SigningKeyStore` argument to the core algorithm.

/// `SigningKeyStore` adapter that consults a process-level
/// `Mutex<Option<SigningKey>>` cache before falling back to [`FsKeyStore`]
/// for cold load + first-time generation.
///
/// Mirrors the pre-Wave-8 `get_or_load_into_mutex` helper that lived in
/// `src-tauri/src/achievements/mod.rs:138-158` (lifted verbatim to the
/// Wave 7 shim, then promoted here in Wave 10 cleanup).
///
/// Behavior:
/// * **Fast path** — if the mutex already holds `Some(key)`, returns a
///   fresh clone of the cached key without touching the filesystem.
/// * **Cold path** — calls [`FsKeyStore::get_or_init`] (which reads or
///   generates+writes the PEMs), then populates the cache with a clone of
///   the result so subsequent calls hit the fast path.
///
/// `export_public_pem` always defers to [`FsKeyStore`] — there's no
/// in-memory cache for the public PEM string.
pub struct MutexCachedKeyStore<'a> {
    cache: &'a Mutex<Option<SigningKey>>,
    fallback: FsKeyStore,
}

impl<'a> MutexCachedKeyStore<'a> {
    /// Construct from an `AppState`-owned cache + the on-disk keys
    /// directory.
    pub fn new(cache: &'a Mutex<Option<SigningKey>>, key_dir: &Path) -> Self {
        Self {
            cache,
            fallback: FsKeyStore::new(key_dir.to_path_buf()),
        }
    }
}

impl<'a> SigningKeyStore for MutexCachedKeyStore<'a> {
    fn get_or_init(&self) -> Result<SigningKey, SigningError> {
        // Fast path — return a fresh clone of the cached key.
        {
            let guard = self
                .cache
                .lock()
                .map_err(|_| SigningError::Io("signing key mutex poisoned".to_string()))?;
            if let Some(k) = guard.as_ref() {
                return Ok(SigningKey::from_bytes(&k.to_bytes()));
            }
        }
        // Cold path — generate or load via FsKeyStore + cache the result.
        let key = self.fallback.get_or_init()?;
        {
            let mut guard = self
                .cache
                .lock()
                .map_err(|_| SigningError::Io("signing key mutex poisoned".to_string()))?;
            if guard.is_none() {
                *guard = Some(SigningKey::from_bytes(&key.to_bytes()));
            }
        }
        Ok(SigningKey::from_bytes(&key.to_bytes()))
    }

    fn export_public_pem(&self) -> Result<String, SigningError> {
        self.fallback.export_public_pem()
    }
}

#[cfg(test)]
mod tests {
    //! FS-backed tests that need a real `tempfile::TempDir`. Pure-crypto
    //! tests live in `learnforge-core/src/signing.rs`.

    use super::*;

    /// `generate_then_load` — verbatim from pre-Wave-5 src-tauri lines 215-227.
    /// First call generates, second call reloads, both yield bytewise-equal
    /// keys and identical fingerprints.
    #[test]
    fn generate_then_load() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsKeyStore::new(tmp.path());

        let k1 = store.get_or_init().expect("first init");
        assert!(
            tmp.path().join(PRIV_FILE).exists() && tmp.path().join(PUB_FILE).exists(),
            "both PEMs persisted"
        );

        let k2 = store.get_or_init().expect("reload");
        assert_eq!(
            k1.to_bytes(),
            k2.to_bytes(),
            "reload yields same key bytes"
        );
        use learnforge_core::signing::public_key_fingerprint;
        assert_eq!(
            public_key_fingerprint(&k1.verifying_key()),
            public_key_fingerprint(&k2.verifying_key())
        );
    }

    /// R3 / Pitfall 4 — private key file is 0600 on Unix immediately after
    /// write. **Verbatim from pre-Wave-5 src-tauri lines 232-239.**
    #[test]
    #[cfg(unix)]
    fn private_key_file_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsKeyStore::new(tmp.path());
        let _k = store.get_or_init().expect("init");
        let meta =
            std::fs::metadata(tmp.path().join(PRIV_FILE)).expect("metadata for private key");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {:o}", mode);
    }

    /// Public PEM export round-trips through the trait surface.
    #[test]
    fn export_public_pem_roundtrips() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsKeyStore::new(tmp.path());
        let _k = store.get_or_init().expect("init");
        let pem = store.export_public_pem().expect("export public pem");
        assert!(
            pem.starts_with("-----BEGIN PUBLIC KEY-----"),
            "PEM header present (got: {:?})",
            &pem.chars().take(64).collect::<String>()
        );
        assert!(pem.contains("-----END PUBLIC KEY-----"), "PEM footer present");
    }

    /// `export_public_pem` reports an I/O error when the public PEM doesn't
    /// exist (cold-start case where `get_or_init` hasn't been called).
    #[test]
    fn export_public_pem_errors_when_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsKeyStore::new(tmp.path());
        let result = store.export_public_pem();
        assert!(result.is_err(), "missing PEM must error");
        if let Err(SigningError::Io(_)) = result {
            // expected
        } else {
            panic!("expected SigningError::Io, got {:?}", result);
        }
    }

    /// `FsKeyStore` is usable as `&dyn SigningKeyStore` (trait surface is
    /// object-safe — Wave 5 invariant for the IPC code that holds a boxed
    /// store).
    #[test]
    fn fs_key_store_is_object_safe() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsKeyStore::new(tmp.path());
        let dyn_store: &dyn SigningKeyStore = &store;
        let _k = dyn_store.get_or_init().expect("dyn get_or_init");
        let _p = dyn_store.export_public_pem().expect("dyn export_public_pem");
    }
}
