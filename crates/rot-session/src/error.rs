//! Error types for the rot-session crate.

/// Errors that can occur in session management.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// Session not found
    #[error("Session not found: {0}")]
    NotFound(String),

    /// Invalid session format
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    /// I/O error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
