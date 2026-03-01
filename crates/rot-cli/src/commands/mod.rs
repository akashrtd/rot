//! Chat and exec command implementations.

pub mod chat;
pub mod exec;
pub mod tools;

use rot_core::{ConfigStore, RuntimeSecurityConfig, SandboxMode};
use rot_tools::ToolRegistry;

pub async fn load_tool_registry(
    runtime_security: RuntimeSecurityConfig,
) -> anyhow::Result<(rot_core::Config, ToolRegistry)> {
    let config_store = ConfigStore::new();
    let config = config_store.load();

    let mut tools = ToolRegistry::new();
    rot_tools::register_all(&mut tools);
    rot_tools::register_custom_tools(&mut tools, &config.custom_tools)
        .map_err(|e| anyhow::anyhow!("Failed to load custom tools: {e}"))?;
    rot_tools::register_mcp_tools(
        &mut tools,
        &config.mcp_servers,
        &std::env::current_dir()?,
        match runtime_security.sandbox_mode {
            SandboxMode::ReadOnly => rot_tools::SandboxMode::ReadOnly,
            SandboxMode::WorkspaceWrite => rot_tools::SandboxMode::WorkspaceWrite,
            SandboxMode::DangerFullAccess => rot_tools::SandboxMode::DangerFullAccess,
        },
        runtime_security.sandbox_network_access
            || runtime_security.sandbox_mode == SandboxMode::DangerFullAccess,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to load MCP tools: {e}"))?;

    Ok((config, tools))
}
