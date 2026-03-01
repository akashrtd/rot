use rot_core::AgentRegistry;
use rot_provider::{AnthropicProvider, Provider, new_openai_provider, new_zai_provider};
use rot_session::SessionStore;

/// Run interactive chat mode.
pub async fn run(
    model: Option<&str>,
    provider_name: &str,
    agent_name: Option<&str>,
    runtime_security: rot_core::RuntimeSecurityConfig,
) -> anyhow::Result<()> {
    let config_store = rot_core::config::ConfigStore::new();
    config_store.hydrate_env();
    let (config, tools) = super::load_tool_registry(runtime_security.clone()).await?;
    let agent_profile = AgentRegistry::resolve(agent_name)?;
    let system_prompt = if agent_profile.name == "default" {
        AgentRegistry::default_chat_system_prompt().to_string()
    } else {
        agent_profile.system_prompt.to_string()
    };

    // If no provider or model were specified, fall back to the config store
    let final_provider = if provider_name.is_empty() {
        &config.provider
    } else {
        provider_name
    };

    let final_model = model.unwrap_or(&config.model);
    let provider = create_provider(final_provider, Some(final_model))?;
    let model_name = provider.current_model().to_string();

    let session_store = SessionStore::new();

    rot_tui::run_tui(
        provider,
        tools,
        session_store,
        &model_name,
        provider_name,
        agent_profile.name,
        system_prompt,
        runtime_security,
    )
        .await
        .map_err(|e| anyhow::anyhow!("TUI error: {e}"))?;

    Ok(())
}

fn create_provider(
    provider_name: &str,
    model: Option<&str>,
) -> anyhow::Result<Box<dyn Provider>> {
    match provider_name {
        "anthropic" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
            let mut provider = AnthropicProvider::new(api_key);
            if let Some(m) = model {
                let _ = provider.set_model(m);
            }
            Ok(Box::new(provider))
        }
        "zai" => {
            let api_key = std::env::var("ZAI_API_KEY").unwrap_or_default();
            let mut provider = new_zai_provider(api_key);
            if let Some(m) = model {
                let _ = provider.set_model(m);
            }
            Ok(Box::new(provider))
        }
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
            let mut provider = new_openai_provider(api_key);
            if let Some(m) = model {
                let _ = provider.set_model(m);
            }
            Ok(Box::new(provider))
        }
        other => Err(anyhow::anyhow!(
            "Unknown provider: {other}. Available: anthropic, zai, openai"
        )),
    }
}
