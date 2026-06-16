//! Error type for the `packs` module.
//!
//! Plain `String` payloads keep the type cheap to construct and easy to
//! convert to Tauri command error strings via `.to_string()`.

use thiserror::Error;

/// Error returned by every fallible function in the [`crate::packs`] module.
///
/// The variants are deliberately broad (`String`-payload) so callers can
/// surface free-form upstream messages (rusqlite, std::io, jsonschema, …)
/// without the typed envelope leaking into core.
#[derive(Debug, Error)]
pub enum PackError {
    /// Filesystem error — `std::io::Error` stringified at the trust boundary.
    #[error("io error: {0}")]
    Io(String),

    /// JSON parse / deserialize error.
    #[error("invalid JSON: {0}")]
    Json(String),

    /// JSON Schema validation failed (D-07 strict path).
    #[error("schema violation: {0}")]
    Schema(String),

    /// Bundled-/skill-loader error not covered by [`Self::Io`] / [`Self::Json`] / [`Self::Schema`].
    #[error("loader error: {0}")]
    Loader(String),
}
