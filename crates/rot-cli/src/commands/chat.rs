use rot_core::AgentRegistry;
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
    let provider = crate::provider_factory::create_provider(final_provider, Some(final_model), true)?;
    let model_name = provider.current_model().to_string();

    let session_store = SessionStore::new();

    rot_tui::run_tui(
        provider,
        tools,
        session_store,
        &model_name,
        final_provider,
        agent_profile.name,
        system_prompt,
        runtime_security,
    )
        .await
        .map_err(|e| anyhow::anyhow!("TUI error: {e}"))?;

    Ok(())
}
