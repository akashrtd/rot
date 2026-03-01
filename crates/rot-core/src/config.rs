use crate::security::{ApprovalPolicy, SandboxMode};
use rot_tools::{CustomToolConfig, McpServerConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Serialized settings from ~/.rot/config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub provider: String,
    pub model: String,
    pub api_keys: HashMap<String, String>,
    pub custom_tools: Vec<CustomToolConfig>,
    pub mcp_servers: Vec<McpServerConfig>,
    pub approval_policy: ApprovalPolicy,
    pub sandbox_mode: SandboxMode,
    pub sandbox_network_access: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: "anthropic".to_string(),
            model: "claude-3-5-sonnet-latest".to_string(),
            api_keys: HashMap::new(),
            custom_tools: Vec::new(),
            mcp_servers: Vec::new(),
            approval_policy: ApprovalPolicy::OnRequest,
            sandbox_mode: SandboxMode::WorkspaceWrite,
            sandbox_network_access: false,
        }
    }
}

/// Helper struct for storing the location to read/write global settings
pub struct ConfigStore {
    path: PathBuf,
}

impl Default for ConfigStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigStore {
    pub fn new() -> Self {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".rot");
        path.push("config.json");
        Self { path }
    }

    /// Load the user's saved config, or fallback to Default
    pub fn load(&self) -> Config {
        if let Ok(content) = fs::read_to_string(&self.path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }
        Config::default()
    }

    /// Save the user's config back to disk
    pub fn save(&self, config: &Config) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(config)?;
        fs::write(&self.path, content)
    }

    /// Hydrate `rot` environment with configured API keys, optionally overwriting process env
    pub fn hydrate_env(&self) {
        let config = self.load();
        for (provider, key) in config.api_keys.iter() {
            if !key.is_empty() {
                let env_var = format!("{}_API_KEY", provider.to_uppercase());
                if std::env::var(&env_var).is_err() {
                    std::env::set_var(&env_var, key);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use crate::security::{ApprovalPolicy, SandboxMode};
    use rot_tools::{CustomToolConfig, McpServerConfig};

    #[test]
    fn test_config_backward_compatible_defaults() {
        let legacy = r#"{
            "provider":"anthropic",
            "model":"claude-3-5-sonnet-latest",
            "api_keys":{"anthropic":"k"}
        }"#;

        let parsed: Config = serde_json::from_str(legacy).unwrap();
        assert!(parsed.custom_tools.is_empty());
        assert!(parsed.mcp_servers.is_empty());
        assert_eq!(parsed.approval_policy, ApprovalPolicy::OnRequest);
        assert_eq!(parsed.sandbox_mode, SandboxMode::WorkspaceWrite);
        assert!(!parsed.sandbox_network_access);
    }

    #[test]
    fn test_config_round_trip_security_fields() {
        let cfg = Config {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_keys: Default::default(),
            custom_tools: vec![CustomToolConfig {
                name: "echo_args".to_string(),
                description: "Echo args".to_string(),
                command: "cat \"$ROT_TOOL_ARGS_FILE\"".to_string(),
                parameters_schema: serde_json::json!({"type":"object"}),
                timeout_secs: Some(30),
            }],
            mcp_servers: vec![McpServerConfig {
                name: "filesystem".to_string(),
                enabled: true,
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
                cwd: Some(".".to_string()),
                env: Default::default(),
                startup_timeout_secs: 20,
                tool_timeout_secs: 60,
            }],
            approval_policy: ApprovalPolicy::Never,
            sandbox_mode: SandboxMode::DangerFullAccess,
            sandbox_network_access: true,
        };

        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.custom_tools.len(), 1);
        assert_eq!(parsed.mcp_servers.len(), 1);
        assert_eq!(parsed.approval_policy, ApprovalPolicy::Never);
        assert_eq!(parsed.sandbox_mode, SandboxMode::DangerFullAccess);
        assert!(parsed.sandbox_network_access);
    }
}
