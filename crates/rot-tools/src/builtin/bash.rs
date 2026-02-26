//! Bash tool — shell command execution.

use crate::error::ToolError;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use rot_sandbox::{SandboxPolicy, run_shell_command};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const MAX_OUTPUT_BYTES: usize = 50 * 1024; // 50KB

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct BashParams {
    /// Shell command to execute.
    pub command: String,
    /// Optional timeout in seconds. Default: 120.
    #[serde(default)]
    pub timeout: Option<u64>,
}

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }
    fn label(&self) -> &str {
        "Bash"
    }
    fn description(&self) -> &str {
        "Execute a shell command and return stdout/stderr."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(BashParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: BashParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let timeout_secs = params.timeout.unwrap_or(ctx.timeout.as_secs());
        let timeout = std::time::Duration::from_secs(timeout_secs);

        let policy = SandboxPolicy {
            mode: match ctx.sandbox_mode {
                SandboxMode::ReadOnly => rot_sandbox::SandboxMode::ReadOnly,
                SandboxMode::WorkspaceWrite => rot_sandbox::SandboxMode::WorkspaceWrite,
                SandboxMode::DangerFullAccess => rot_sandbox::SandboxMode::DangerFullAccess,
            },
            network_access: ctx.network_access,
        };

        let output = run_shell_command(&params.command, &ctx.working_dir, timeout, &policy)
            .await
            .map_err(|e| match e {
                rot_sandbox::SandboxError::Timeout(_) => {
                    ToolError::Timeout(format!("Command timed out after {timeout_secs}s"))
                }
                rot_sandbox::SandboxError::BackendUnavailable(msg) => {
                    ToolError::PermissionDenied(msg)
                }
                other => ToolError::ExecutionError(other.to_string()),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.exit_code;

        let mut text = String::new();
        if !stdout.is_empty() {
            text.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str("STDERR:\n");
            text.push_str(&stderr);
        }

        // Truncate
        if text.len() > MAX_OUTPUT_BYTES {
            text.truncate(MAX_OUTPUT_BYTES);
            text.push_str("\n\n... (truncated at 50KB)");
        }

        if text.is_empty() {
            text = "(no output)".to_string();
        }

        let is_error = !output.success;
        if is_error {
            text = format!("Exit code: {exit_code}\n{text}");
        }

        Ok(ToolResult {
            output: text,
            metadata: serde_json::json!({"exit_code": exit_code}),
            is_error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_ctx(dir: &TempDir) -> ToolContext {
        ToolContext {
            working_dir: dir.path().to_path_buf(),
            sandbox_mode: SandboxMode::DangerFullAccess,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_bash_echo() {
        let dir = TempDir::new().unwrap();
        let result = BashTool
            .execute(
                serde_json::json!({"command": "echo hello"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.output.contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_exit_code() {
        let dir = TempDir::new().unwrap();
        let result = BashTool
            .execute(
                serde_json::json!({"command": "exit 42"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.output.contains("42"));
    }

    #[tokio::test]
    async fn test_bash_timeout() {
        let dir = TempDir::new().unwrap();
        let result = BashTool
            .execute(
                serde_json::json!({"command": "sleep 30", "timeout": 1}),
                &test_ctx(&dir),
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::Timeout(_) => {}
            other => panic!("Expected Timeout, got: {other:?}"),
        }
    }
}
