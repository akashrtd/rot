//! Tmux integration tools.

use crate::error::ToolError;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TmuxCaptureParams {
    /// The pane to capture (e.g., "%1", "{last}", or relative offset like "-1").
    /// If empty, captures the current pane.
    #[serde(default)]
    pub pane: Option<String>,
    /// Capture the full history instead of just the visible area.
    #[serde(default)]
    pub full_history: bool,
}

pub struct TmuxCaptureTool;

#[async_trait]
impl Tool for TmuxCaptureTool {
    fn name(&self) -> &str {
        "tmux_capture_pane"
    }
    fn label(&self) -> &str {
        "Tmux Capture Pane"
    }
    fn description(&self) -> &str {
        "Captures the text content of a tmux pane."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(TmuxCaptureParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: TmuxCaptureParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let mut cmd = Command::new("tmux");
        cmd.arg("capture-pane").arg("-p");

        if params.full_history {
            cmd.arg("-S").arg("-").arg("-E").arg("-");
        }

        if let Some(pane) = params.pane {
            cmd.arg("-t").arg(pane);
        }

        let output = cmd.output().map_err(|e| {
            ToolError::ExecutionError(format!("Failed to execute tmux: {e}"))
        })?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionError(format!("Tmux error: {err}")));
        }

        Ok(ToolResult::success(String::from_utf8_lossy(&output.stdout).to_string()))
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TmuxSendKeysParams {
    /// The pane to send keys to.
    pub pane: String,
    /// The keys or string to send.
    pub keys: String,
    /// Whether to suppress the automatic 'Enter' at the end. Default: false (will send Enter).
    #[serde(default)]
    pub no_enter: bool,
}

pub struct TmuxSendKeysTool;

#[async_trait]
impl Tool for TmuxSendKeysTool {
    fn name(&self) -> &str {
        "tmux_send_keys"
    }
    fn label(&self) -> &str {
        "Tmux Send Keys"
    }
    fn description(&self) -> &str {
        "Sends keys or commands to a tmux pane."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(TmuxSendKeysParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        if ctx.sandbox_mode == SandboxMode::ReadOnly {
            return Err(ToolError::PermissionDenied(
                "tmux_send_keys is disabled in read-only sandbox mode".to_string(),
            ));
        }

        let params: TmuxSendKeysParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let mut cmd = Command::new("tmux");
        cmd.arg("send-keys").arg("-t").arg(&params.pane).arg(&params.keys);

        if !params.no_enter {
            cmd.arg("C-m");
        }

        let output = cmd.output().map_err(|e| {
            ToolError::ExecutionError(format!("Failed to execute tmux: {e}"))
        })?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionError(format!("Tmux error: {err}")));
        }

        Ok(ToolResult::success(format!("Sent keys to pane {}", params.pane)))
    }
}
