//! Error type for the topic_packs module.
//!
//! Plain `String` payloads keep the type cheap to construct and easy to
//! convert to Tauri command error strings via `.to_string()`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PackError {
    #[error("io error: {0}")]
    Io(String),

    #[error("invalid JSON: {0}")]
    Json(String),

    #[error("schema violation: {0}")]
    Schema(String),

    #[error("loader error: {0}")]
    Loader(String),
}
