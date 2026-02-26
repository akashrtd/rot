use rot_provider::{AnthropicProvider, Provider, new_zai_provider};
use rot_session::SessionStore;
use rot_tools::ToolRegistry;

/// Run interactive chat mode.
pub async fn run(model: &str, provider_name: &str) -> anyhow::Result<()> {
    let provider = create_provider(provider_name, model)?;
    let mut tools = ToolRegistry::new();
    rot_tools::register_all(&mut tools);

    let session_store = SessionStore::new();

    rot_tui::run_tui(provider, tools, session_store, model, provider_name)
        .await
        .map_err(|e| anyhow::anyhow!("TUI error: {e}"))?;

    Ok(())
}

fn create_provider(
    provider_name: &str,
    model: &str,
) -> anyhow::Result<Box<dyn Provider>> {
    match provider_name {
        "anthropic" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
                anyhow::anyhow!(
                    "ANTHROPIC_API_KEY not set. Set it with:\n  \
                     export ANTHROPIC_API_KEY=your-key-here"
                )
            })?;
            let mut provider = AnthropicProvider::new(api_key);
            if model != "claude-sonnet-4-20250514" {
                provider
                    .set_model(model)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            Ok(Box::new(provider))
        }
        "zai" => {
            let api_key = std::env::var("ZAI_API_KEY").map_err(|_| {
                anyhow::anyhow!(
                    "ZAI_API_KEY not set. Set it with:\n  \
                     export ZAI_API_KEY=your-key-here\n\n\
                     Get your key from https://z.ai"
                )
            })?;
            let mut provider = new_zai_provider(api_key);
            if model != "glm-5" {
                provider
                    .set_model(model)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            Ok(Box::new(provider))
        }
        other => Err(anyhow::anyhow!(
            "Unknown provider: {other}. Available: anthropic, zai"
        )),
    }
}
