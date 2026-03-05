//! Shared provider registry and factory helpers for CLI commands.

use rot_provider::{
    AnthropicProvider, Provider, new_google_provider, new_mock_provider, new_ollama_provider,
    new_openai_provider, new_openrouter_provider, new_zai_provider,
};
use rot_provider::types::ModelInfo;
use std::collections::HashMap;
use std::path::PathBuf;

/// Built-in provider metadata.
#[derive(Debug, Clone, Copy)]
pub struct ProviderDescriptor {
    /// Provider identifier used in CLI/config.
    pub name: &'static str,
    /// Primary API-key environment variable, if required.
    pub api_key_env: Option<&'static str>,
    /// Fallback API-key environment variable.
    pub api_key_env_fallback: Option<&'static str>,
}

pub const BUILTIN_PROVIDERS: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        name: "anthropic",
        api_key_env: Some("ANTHROPIC_API_KEY"),
        api_key_env_fallback: None,
    },
    ProviderDescriptor {
        name: "zai",
        api_key_env: Some("ZAI_API_KEY"),
        api_key_env_fallback: None,
    },
    ProviderDescriptor {
        name: "openai",
        api_key_env: Some("OPENAI_API_KEY"),
        api_key_env_fallback: None,
    },
    ProviderDescriptor {
        name: "ollama",
        api_key_env: None,
        api_key_env_fallback: None,
    },
    ProviderDescriptor {
        name: "openrouter",
        api_key_env: Some("OPENROUTER_API_KEY"),
        api_key_env_fallback: None,
    },
    ProviderDescriptor {
        name: "google",
        api_key_env: Some("GOOGLE_API_KEY"),
        api_key_env_fallback: Some("GEMINI_API_KEY"),
    },
    ProviderDescriptor {
        name: "mock",
        api_key_env: None,
        api_key_env_fallback: None,
    },
];

/// Return all built-in provider names.
pub fn provider_names() -> Vec<&'static str> {
    BUILTIN_PROVIDERS.iter().map(|p| p.name).collect()
}

/// Look up a provider descriptor by name.
pub fn descriptor_for(name: &str) -> Option<&'static ProviderDescriptor> {
    BUILTIN_PROVIDERS.iter().find(|p| p.name == name)
}

/// Resolve custom models injected via ~/.rot/models.json
fn resolve_custom_models(provider_name: &str) -> Vec<ModelInfo> {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".rot");
    path.push("models.json");

    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(custom_config) = serde_json::from_str::<HashMap<String, Vec<ModelInfo>>>(&content) {
            if let Some(models) = custom_config.get(provider_name) {
                return models.clone();
            }
        }
    }
    Vec::new()
}

/// Create a provider by name.
///
/// If `require_credentials` is true, missing API keys become a hard error for providers
/// that require keys. If false, empty keys are allowed for metadata commands like `rot models`.
pub fn create_provider(
    provider_name: &str,
    model: Option<&str>,
    require_credentials: bool,
) -> anyhow::Result<Box<dyn Provider>> {
    let custom_models = resolve_custom_models(provider_name);

    let mut provider: Box<dyn Provider> = match provider_name {
        "anthropic" => Box::new(AnthropicProvider::new(
            resolve_api_key(descriptor_for("anthropic").unwrap(), require_credentials)?,
            custom_models,
        )),
        "zai" => Box::new(new_zai_provider(
            resolve_api_key(descriptor_for("zai").unwrap(), require_credentials)?,
            custom_models,
        )),
        "openai" => Box::new(new_openai_provider(
            resolve_api_key(descriptor_for("openai").unwrap(), require_credentials)?,
            custom_models,
        )),
        "ollama" => Box::new(new_ollama_provider(
            resolve_api_key(descriptor_for("ollama").unwrap(), require_credentials)?,
            custom_models,
        )),
        "openrouter" => Box::new(new_openrouter_provider(
            resolve_api_key(descriptor_for("openrouter").unwrap(), require_credentials)?,
            custom_models,
        )),
        "google" => Box::new(new_google_provider(
            resolve_api_key(descriptor_for("google").unwrap(), require_credentials)?,
            custom_models,
        )),
        "mock" => Box::new(new_mock_provider(resolve_mock_responses()?)),
        other => {
            return Err(anyhow::anyhow!(
                "Unknown provider: {other}. Available: {}",
                provider_names().join(", ")
            ));
        }
    };

    if let Some(model_id) = model {
        provider
            .set_model(model_id)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    Ok(provider)
}

fn resolve_mock_responses() -> anyhow::Result<Vec<String>> {
    if let Ok(raw) = std::env::var("ROT_MOCK_RESPONSES") {
        let parsed: Vec<String> = serde_json::from_str(&raw).map_err(|e| {
            anyhow::anyhow!("Invalid ROT_MOCK_RESPONSES JSON (expected string array): {e}")
        })?;
        if parsed.is_empty() {
            return Err(anyhow::anyhow!(
                "ROT_MOCK_RESPONSES must contain at least one scripted response"
            ));
        }
        Ok(parsed)
    } else {
        Ok(vec!["```repl\nFINAL('mock')\n```".to_string()])
    }
}

fn resolve_api_key(desc: &ProviderDescriptor, require_credentials: bool) -> anyhow::Result<String> {
    let key = match (desc.api_key_env, desc.api_key_env_fallback) {
        (None, _) => String::new(),
        (Some(primary), Some(fallback)) => std::env::var(primary)
            .or_else(|_| std::env::var(fallback))
            .unwrap_or_default(),
        (Some(primary), None) => std::env::var(primary).unwrap_or_default(),
    };

    if require_credentials && desc.api_key_env.is_some() && key.is_empty() {
        let primary = desc.api_key_env.unwrap();
        let fallback_text = desc
            .api_key_env_fallback
            .map(|v| format!(" (or {v})"))
            .unwrap_or_default();
        return Err(anyhow::anyhow!(
            "{primary} not set{fallback_text}. Configure credentials for provider '{}'.",
            desc.name
        ));
    }

    Ok(key)
}
