//! Single-shot exec command.

use rot_core::{Agent, AgentConfig, ContentBlock, Message};
use rot_provider::{AnthropicProvider, Provider, new_zai_provider};
use rot_tools::ToolRegistry;

/// Execute a single prompt and print the result.
pub async fn run(prompt: &str, model: Option<&str>, provider_name: &str) -> anyhow::Result<()> {
    let provider = create_provider(provider_name, model)?;
    let mut tools = ToolRegistry::new();
    rot_tools::register_all(&mut tools);

    let config = AgentConfig {
        system_prompt: Some(
            "You are rot, an AI coding assistant. Be concise and direct.".to_string(),
        ),
        max_tokens: Some(4096),
        ..Default::default()
    };

    let agent = Agent::new(provider, tools, config);
    let mut messages: Vec<Message> = Vec::new();

    let response = agent.process(&mut messages, prompt).await?;

    // Print the text content
    for block in &response.content {
        if let ContentBlock::Text { text } = block {
            println!("{text}");
        }
    }

    Ok(())
}

fn create_provider(
    provider_name: &str,
    model: Option<&str>,
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
            if let Some(m) = model {
                provider
                    .set_model(m)
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
            if let Some(m) = model {
                provider
                    .set_model(m)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            Ok(Box::new(provider))
        }
        other => Err(anyhow::anyhow!(
            "Unknown provider: {other}. Available: anthropic, zai"
        )),
    }
}
