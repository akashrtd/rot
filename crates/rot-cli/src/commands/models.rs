//! `rot models` command.

/// List models for the selected provider.
pub fn run(provider_name: &str) -> anyhow::Result<()> {
    let provider = crate::provider_factory::create_provider(provider_name, None, false)?;
    let models = provider.models();

    println!("Provider: {}", provider.name());
    println!("Current model: {}", provider.current_model());
    println!("Models:");
    for model in models {
        println!(
            "- {} ({}) ctx={} max_out={} tools={} thinking={}",
            model.id,
            model.name,
            model.context_window,
            model.max_output_tokens,
            model.supports_tools,
            model.supports_thinking
        );
    }

    Ok(())
}
