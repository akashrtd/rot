//! `rot providers` command.

/// List configured and available providers.
pub fn run() -> anyhow::Result<()> {
    let config = rot_core::ConfigStore::new().load();

    let available = crate::provider_factory::provider_names();
    println!("Available providers:");
    for name in &available {
        let desc = crate::provider_factory::descriptor_for(name)
            .ok_or_else(|| anyhow::anyhow!("unknown provider descriptor: {name}"))?;
        let env_present = desc
            .api_key_env
            .map(|primary| {
                std::env::var(primary).is_ok()
                    || desc
                        .api_key_env_fallback
                        .map(|fallback| std::env::var(fallback).is_ok())
                        .unwrap_or(false)
            })
            .unwrap_or(true);

        let configured_in_file = config
            .api_keys
            .get(*name)
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false);

        let status = if desc.api_key_env.is_none() {
            "no-key"
        } else if env_present || configured_in_file {
            "configured"
        } else {
            "missing-key"
        };
        println!("- {name} [{status}]");
    }

    println!();
    println!(
        "Current default: provider={} model={}",
        config.provider, config.model
    );

    Ok(())
}
