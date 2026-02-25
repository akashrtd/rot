//! Tool trait definition and common types.

use crate::error::ToolError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Context provided to tools during execution.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Current working directory.
    pub working_dir: PathBuf,
    /// Active session ID.
    pub session_id: String,
    /// Execution timeout.
    pub timeout: Duration,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_default(),
            session_id: String::new(),
            timeout: Duration::from_secs(120),
        }
    }
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Output text.
    pub output: String,
    /// Optional metadata (e.g., line count, file size).
    pub metadata: serde_json::Value,
    /// Whether the result represents an error.
    pub is_error: bool,
}

impl ToolResult {
    /// Create a successful tool result.
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            metadata: serde_json::Value::Null,
            is_error: false,
        }
    }

    /// Create a successful result with metadata.
    pub fn success_with_metadata(output: impl Into<String>, metadata: serde_json::Value) -> Self {
        Self {
            output: output.into(),
            metadata,
            is_error: false,
        }
    }

    /// Create an error tool result.
    pub fn error(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            metadata: serde_json::Value::Null,
            is_error: true,
        }
    }
}

/// Trait that all tools must implement.
///
/// Tools are the primary way the agent interacts with the environment
/// (file system, shell, web, etc.).
#[async_trait]
pub trait Tool: Send + Sync {
    /// Machine-readable tool name (e.g., "read").
    fn name(&self) -> &str;

    /// Human-readable label (e.g., "Read File").
    fn label(&self) -> &str;

    /// Description of what the tool does.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's parameters.
    fn parameters_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given arguments.
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError>;
}

// Compile-time check: Tool must be object-safe
const _: () = {
    fn _assert_object_safe(_: &dyn Tool) {}
};
