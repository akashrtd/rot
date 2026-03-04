//! Neovim integration tools.

use crate::error::ToolError;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NvimReadParams {
    /// The buffer to read (defaults to current buffer).
    #[serde(default)]
    pub buffer: Option<String>,
}

pub struct NvimReadTool;

#[async_trait]
impl Tool for NvimReadTool {
    fn name(&self) -> &str {
        "nvim_read_buffer"
    }
    fn label(&self) -> &str {
        "Neovim Read Buffer"
    }
    fn description(&self) -> &str {
        "Reads the content of a Neovim buffer if rot is running inside a Neovim terminal."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(NvimReadParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let nvim_server = std::env::var("NVIM").map_err(|_| {
            ToolError::ExecutionError("$NVIM environment variable not set. Are you running rot inside Neovim?".to_string())
        })?;

        let params: NvimReadParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        // We use nvim --server $NVIM --remote-expr to get buffer content
        let expr = match params.buffer {
            Some(b) => format!("join(getbufline({b}, 1, '$'), \"\\n\")"),
            None => "join(getline(1, '$'), \"\\n\")".to_string(),
        };

        let output = Command::new("nvim")
            .arg("--server")
            .arg(&nvim_server)
            .arg("--remote-expr")
            .arg(&expr)
            .output()
            .map_err(|e| ToolError::ExecutionError(format!("Failed to execute nvim: {e}")))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionError(format!("Nvim error: {err}")));
        }

        Ok(ToolResult::success(String::from_utf8_lossy(&output.stdout).to_string()))
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NvimWriteParams {
    /// The text to write/insert.
    pub content: String,
    /// The mode to write: "replace" (whole buffer) or "append". Default: "append"
    #[serde(default)]
    pub mode: Option<String>,
}

pub struct NvimWriteTool;

#[async_trait]
impl Tool for NvimWriteTool {
    fn name(&self) -> &str {
        "nvim_write_buffer"
    }
    fn label(&self) -> &str {
        "Neovim Write Buffer"
    }
    fn description(&self) -> &str {
        "Writes or appends text to the current Neovim buffer."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(NvimWriteParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        if ctx.sandbox_mode == SandboxMode::ReadOnly {
            return Err(ToolError::PermissionDenied(
                "nvim_write_buffer is disabled in read-only sandbox mode".to_string(),
            ));
        }

        let nvim_server = std::env::var("NVIM").map_err(|_| {
            ToolError::ExecutionError("$NVIM environment variable not set. Are you running rot inside Neovim?".to_string())
        })?;

        let params: NvimWriteParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let mode = params.mode.as_deref().unwrap_or("append");
        
        // Escape content for Vim strings
        let escaped_content = params.content.replace('\'', "''");
        
        let command = match mode {
            "replace" => format!(":1,$delete | put = '{}' | 1delete", escaped_content),
            _ => format!(":put = '{}' ", escaped_content),
        };

        let output = Command::new("nvim")
            .arg("--server")
            .arg(&nvim_server)
            .arg("--remote-send")
            .arg(&command)
            .arg("<CR>")
            .output()
            .map_err(|e| ToolError::ExecutionError(format!("Failed to execute nvim: {e}")))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionError(format!("Nvim error: {err}")));
        }

        Ok(ToolResult::success(format!("Buffer update sent to Neovim ({mode})")))
    }
}
