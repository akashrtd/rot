//! Config-driven external command tools.

use crate::error::ToolError;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use rot_sandbox::{run_shell_command, SandboxPolicy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

const MAX_OUTPUT_BYTES: usize = 50 * 1024; // 50KB

fn default_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": true,
    })
}

/// Config for a custom command-backed tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomToolConfig {
    /// Machine-readable tool name.
    pub name: String,
    /// Human-readable description shown to the model.
    pub description: String,
    /// Shell command to execute.
    pub command: String,
    /// Optional JSON schema describing accepted arguments.
    #[serde(default = "default_schema")]
    pub parameters_schema: Value,
    /// Optional per-tool timeout in seconds.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Register custom tools from config into a registry.
pub fn register_custom_tools(
    registry: &mut crate::ToolRegistry,
    configs: &[CustomToolConfig],
) -> Result<(), ToolError> {
    for config in configs {
        validate_custom_tool_config(config)?;
        if registry.has(&config.name) {
            return Err(ToolError::ExecutionError(format!(
                "Duplicate tool name '{}'",
                config.name
            )));
        }
        registry.register(Arc::new(CustomCommandTool {
            config: config.clone(),
        }));
    }

    Ok(())
}

struct CustomCommandTool {
    config: CustomToolConfig,
}

#[async_trait]
impl Tool for CustomCommandTool {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn label(&self) -> &str {
        &self.config.name
    }

    fn description(&self) -> &str {
        &self.config.description
    }

    fn parameters_schema(&self) -> Value {
        self.config.parameters_schema.clone()
    }

    async fn execute(
        &self,
        args: Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let args_json = serde_json::to_string(&args)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to encode tool args: {e}")))?;
        let args_file = std::env::temp_dir().join(format!("rot-tool-{}.json", ulid::Ulid::new()));
        tokio::fs::write(&args_file, &args_json).await.map_err(|e| {
            ToolError::ExecutionError(format!("Failed to write tool args file: {e}"))
        })?;

        let timeout = std::time::Duration::from_secs(
            self.config.timeout_secs.unwrap_or(ctx.timeout.as_secs()),
        );
        let policy = SandboxPolicy {
            mode: match ctx.sandbox_mode {
                SandboxMode::ReadOnly => rot_sandbox::SandboxMode::ReadOnly,
                SandboxMode::WorkspaceWrite => rot_sandbox::SandboxMode::WorkspaceWrite,
                SandboxMode::DangerFullAccess => rot_sandbox::SandboxMode::DangerFullAccess,
            },
            network_access: ctx.network_access,
        };

        let command = format!(
            "export ROT_TOOL_NAME='{}'; export ROT_TOOL_ARGS_FILE='{}'; export ROT_TOOL_ARGS_JSON='{}'; export ROT_SESSION_ID='{}'; {};",
            shell_escape(&self.config.name),
            shell_escape(&args_file.display().to_string()),
            shell_escape(&args_json),
            shell_escape(&ctx.session_id),
            self.config.command
        );

        let output = run_shell_command(&command, &ctx.working_dir, timeout, &policy).await;
        let _ = tokio::fs::remove_file(&args_file).await;
        let output = output.map_err(|e| match e {
            rot_sandbox::SandboxError::Timeout(_) => ToolError::Timeout(format!(
                "Custom tool '{}' timed out after {}s",
                self.config.name,
                timeout.as_secs()
            )),
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
        if text.len() > MAX_OUTPUT_BYTES {
            text.truncate(MAX_OUTPUT_BYTES);
            text.push_str("\n\n... (truncated at 50KB)");
        }
        if text.is_empty() {
            text = "(no output)".to_string();
        }
        if !output.success {
            text = format!("Exit code: {exit_code}\n{text}");
        }

        Ok(ToolResult {
            output: text,
            metadata: serde_json::json!({
                "exit_code": exit_code,
                "tool_type": "custom_command",
            }),
            is_error: !output.success,
        })
    }
}

fn validate_custom_tool_config(config: &CustomToolConfig) -> Result<(), ToolError> {
    if config.name.is_empty() || !is_valid_tool_name(&config.name) {
        return Err(ToolError::InvalidParameters(format!(
            "Invalid custom tool name '{}'",
            config.name
        )));
    }
    if config.description.trim().is_empty() {
        return Err(ToolError::InvalidParameters(format!(
            "Custom tool '{}' is missing a description",
            config.name
        )));
    }
    if config.command.trim().is_empty() {
        return Err(ToolError::InvalidParameters(format!(
            "Custom tool '{}' is missing a command",
            config.name
        )));
    }
    Ok(())
}

fn is_valid_tool_name(name: &str) -> bool {
    name.chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | '-'))
}

fn shell_escape(input: &str) -> String {
    input.replace('\'', "'\"'\"'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::sync::Arc;

    fn test_ctx(dir: &TempDir) -> ToolContext {
        ToolContext {
            working_dir: dir.path().to_path_buf(),
            sandbox_mode: SandboxMode::DangerFullAccess,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_custom_command_tool_receives_json_env() {
        let dir = TempDir::new().unwrap();
        let config = CustomToolConfig {
            name: "echo_args".to_string(),
            description: "Echo args".to_string(),
            command: "cat \"$ROT_TOOL_ARGS_FILE\"".to_string(),
            parameters_schema: default_schema(),
            timeout_secs: None,
        };
        let tool = CustomCommandTool { config };

        let result = tool
            .execute(serde_json::json!({"path":"src/main.rs"}), &test_ctx(&dir))
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.output.contains("\"path\":\"src/main.rs\""));
    }

    #[test]
    fn test_register_custom_tools_rejects_duplicates() {
        let mut registry = crate::ToolRegistry::new();
        registry.register(Arc::new(crate::builtin::read::ReadTool));

        let err = register_custom_tools(
            &mut registry,
            &[CustomToolConfig {
                name: "read".to_string(),
                description: "bad".to_string(),
                command: "echo hi".to_string(),
                parameters_schema: default_schema(),
                timeout_secs: None,
            }],
        )
        .unwrap_err();

        assert!(matches!(err, ToolError::ExecutionError(_)));
    }
}
