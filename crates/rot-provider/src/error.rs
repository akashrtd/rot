//! Error types for the rot-provider crate.

/// Errors that can occur in LLM provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// HTTP request failed
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// API returned an error response
    #[error("API error: {0}")]
    ApiError(String),

    /// Invalid model specified
    #[error("Invalid model: {0}")]
    InvalidModel(String),

    /// Stream parsing error
    #[error("Stream error: {0}")]
    StreamError(String),

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Authentication error
    #[error("Authentication error: {0}")]
    Auth(String),
}
