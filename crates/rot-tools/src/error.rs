//! Error types for the rot-tools crate.

/// Errors that can occur during tool execution.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// Invalid parameters passed to tool
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    /// Tool execution failed
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// Permission denied for operation
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Operation timed out
    #[error("Timeout: {0}")]
    Timeout(String),

    /// I/O error during tool execution
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
