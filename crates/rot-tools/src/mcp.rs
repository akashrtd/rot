//! MCP-backed external tools discovered from configured stdio servers.

use crate::error::ToolError;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use rot_mcp::{McpClient, StdioServerConfig};
use rot_sandbox::SandboxPolicy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

fn default_startup_timeout_secs() -> u64 {
    20
}

fn default_tool_timeout_secs() -> u64 {
    60
}

fn default_enabled() -> bool {
    true
}

/// Config for a stdio MCP server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerConfig {
    /// Unique server name used in tool namespacing.
    pub name: String,
    /// Whether this server should be loaded.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Executable to spawn.
    pub command: String,
    /// Optional command arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional working directory override for the server process.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Optional environment overrides for the server process.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Startup timeout in seconds.
    #[serde(default = "default_startup_timeout_secs")]
    pub startup_timeout_secs: u64,
    /// Per-tool call timeout in seconds.
    #[serde(default = "default_tool_timeout_secs")]
    pub tool_timeout_secs: u64,
}

/// Register MCP tools from configured servers into a registry.
pub async fn register_mcp_tools(
    registry: &mut crate::ToolRegistry,
    configs: &[McpServerConfig],
    cwd: &Path,
    sandbox_mode: SandboxMode,
    network_access: bool,
) -> Result<(), ToolError> {
    for config in configs {
        if !config.enabled {
            continue;
        }
        validate_mcp_server_config(config)?;
        let server_cwd = resolve_server_cwd(cwd, config.cwd.as_deref())?;
        let (client, tools) = McpClient::connect(StdioServerConfig {
            name: config.name.clone(),
            command: config.command.clone(),
            args: config.args.clone(),
            env: config.env.clone(),
            cwd: server_cwd,
            startup_timeout: Duration::from_secs(config.startup_timeout_secs),
            tool_timeout: Duration::from_secs(config.tool_timeout_secs),
            policy: SandboxPolicy {
                mode: match sandbox_mode {
                    SandboxMode::ReadOnly => rot_sandbox::SandboxMode::ReadOnly,
                    SandboxMode::WorkspaceWrite => rot_sandbox::SandboxMode::WorkspaceWrite,
                    SandboxMode::DangerFullAccess => rot_sandbox::SandboxMode::DangerFullAccess,
                },
                network_access,
            },
        })
        .await
        .map_err(map_mcp_error)?;

        let client = Arc::new(client);
        for tool in tools {
            let exported_name = exported_tool_name(&config.name, &tool.name);
            if registry.has(&exported_name) {
                return Err(ToolError::ExecutionError(format!(
                    "Duplicate tool name '{exported_name}'"
                )));
            }
            registry.register(Arc::new(McpTool {
                exported_name,
                server_name: config.name.clone(),
                remote_name: tool.name,
                description: tool.description,
                input_schema: tool.input_schema,
                client: Arc::clone(&client),
            }));
        }
    }

    Ok(())
}

struct McpTool {
    exported_name: String,
    server_name: String,
    remote_name: String,
    description: String,
    input_schema: Value,
    client: Arc<McpClient>,
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.exported_name
    }

    fn label(&self) -> &str {
        &self.exported_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        self.input_schema.clone()
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let result = self
            .client
            .call_tool(&self.remote_name, args)
            .await
            .map_err(map_mcp_error)?;
        Ok(ToolResult {
            output: result.text,
            metadata: serde_json::json!({
                "tool_type": "mcp",
                "server": self.server_name,
                "remote_tool": self.remote_name,
                "structured_content": result.structured_content,
                "raw_content": result.raw_content,
            }),
            is_error: result.is_error,
        })
    }
}

fn validate_mcp_server_config(config: &McpServerConfig) -> Result<(), ToolError> {
    if config.name.is_empty() || !is_valid_name_component(&config.name) {
        return Err(ToolError::InvalidParameters(format!(
            "Invalid MCP server name '{}'",
            config.name
        )));
    }
    if config.command.trim().is_empty() {
        return Err(ToolError::InvalidParameters(format!(
            "MCP server '{}' is missing a command",
            config.name
        )));
    }
    Ok(())
}

fn resolve_server_cwd(base_cwd: &Path, configured_cwd: Option<&str>) -> Result<std::path::PathBuf, ToolError> {
    match configured_cwd {
        Some(configured_cwd) if !configured_cwd.trim().is_empty() => {
            let path = Path::new(configured_cwd);
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                base_cwd.join(path)
            };
            Ok(resolved)
        }
        _ => Ok(base_cwd.to_path_buf()),
    }
}

fn exported_tool_name(server_name: &str, tool_name: &str) -> String {
    format!(
        "mcp__{}__{}",
        sanitize_name_component(server_name),
        sanitize_name_component(tool_name)
    )
}

fn sanitize_name_component(input: &str) -> String {
    let mut sanitized = String::new();
    for ch in input.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            sanitized.push(ch);
        } else if ch.is_ascii_uppercase() {
            sanitized.push(ch.to_ascii_lowercase());
        } else {
            sanitized.push('_');
        }
    }
    while sanitized.contains("__") {
        sanitized = sanitized.replace("__", "_");
    }
    sanitized.trim_matches('_').to_string()
}

fn is_valid_name_component(input: &str) -> bool {
    input.chars().all(|ch| {
        ch.is_ascii_lowercase() || ch.is_ascii_uppercase() || ch.is_ascii_digit() || matches!(ch, '_' | '-')
    })
}

fn map_mcp_error(error: rot_mcp::McpError) -> ToolError {
    match error {
        rot_mcp::McpError::Timeout { server, seconds } => ToolError::Timeout(format!(
            "MCP server '{server}' timed out after {seconds}s"
        )),
        rot_mcp::McpError::Sandbox(message) => ToolError::PermissionDenied(message),
        other => ToolError::ExecutionError(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_server_script(dir: &TempDir) -> std::path::PathBuf {
        let path = dir.path().join("fake-mcp.sh");
        std::fs::write(
            &path,
            r#"#!/bin/sh
read line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"fake","version":"1.0.0"}}}'
read line
read line
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"echo","description":"Echo input","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}}]}}'
read line
printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"pong"}],"structuredContent":{"ok":true},"isError":false}}'
"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&path, permissions).unwrap();
        }
        path
    }

    #[tokio::test]
    async fn test_register_and_execute_mcp_tool() {
        let dir = TempDir::new().unwrap();
        let script = test_server_script(&dir);
        let mut registry = crate::ToolRegistry::new();

        register_mcp_tools(
            &mut registry,
            &[McpServerConfig {
                name: "fake".to_string(),
                enabled: true,
                command: script.display().to_string(),
                args: Vec::new(),
                cwd: None,
                env: HashMap::new(),
                startup_timeout_secs: 5,
                tool_timeout_secs: 5,
            }],
            dir.path(),
            SandboxMode::DangerFullAccess,
            false,
        )
        .await
        .unwrap();

        let tool = registry.get("mcp__fake__echo").unwrap();
        let result = tool
            .execute(
                serde_json::json!({"text":"hello"}),
                &ToolContext {
                    working_dir: dir.path().to_path_buf(),
                    sandbox_mode: SandboxMode::DangerFullAccess,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.output, "pong");
        assert_eq!(result.metadata["tool_type"], "mcp");
    }

    #[tokio::test]
    async fn test_disabled_mcp_server_is_skipped() {
        let dir = TempDir::new().unwrap();
        let script = test_server_script(&dir);
        let mut registry = crate::ToolRegistry::new();

        register_mcp_tools(
            &mut registry,
            &[McpServerConfig {
                name: "fake".to_string(),
                enabled: false,
                command: script.display().to_string(),
                args: Vec::new(),
                cwd: None,
                env: HashMap::new(),
                startup_timeout_secs: 5,
                tool_timeout_secs: 5,
            }],
            dir.path(),
            SandboxMode::DangerFullAccess,
            false,
        )
        .await
        .unwrap();

        assert!(!registry.has("mcp__fake__echo"));
    }

    #[test]
    fn test_resolve_server_cwd_relative_to_workspace() {
        let dir = TempDir::new().unwrap();
        let resolved = resolve_server_cwd(dir.path(), Some("servers/fs")).unwrap();
        assert_eq!(resolved, dir.path().join("servers/fs"));
    }
}
