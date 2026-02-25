//! Error types for the rot-core crate.

/// Core error type for the rot agent.
///
/// Additional `#[from]` variants for `ProviderError`, `ToolError`, and
/// `SessionError` will be added as those crates are implemented (T1.2–T1.4).
#[derive(Debug, thiserror::Error)]
pub enum RotError {
    /// I/O error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Generic error with message
    #[error("{0}")]
    Other(String),
}
