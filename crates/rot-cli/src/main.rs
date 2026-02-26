//! rot — Recursive Operations Tool
//!
//! An AI-powered coding agent that runs in your terminal.

mod cli;
mod commands;

use clap::Parser;
use cli::{Cli, Commands, SessionAction};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config_store = rot_core::ConfigStore::new();
    config_store.hydrate_env();
    let config = config_store.load();

    // Initialize logging
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("rot=debug")
            .init();
    }

    match cli.command {
        None | Some(Commands::Chat) => {
            let security = cli.resolve_runtime_security(&config);
            commands::chat::run(cli.model.as_deref(), &cli.provider, security).await?;
        }
        Some(Commands::Exec {
            ref prompt,
            rlm,
            ref context,
            json,
            final_json,
            ref output_schema,
        }) => {
            let security = cli.resolve_runtime_security_for_exec(&config)?;
            let options = commands::exec::ExecOptions {
                json,
                final_json,
                output_schema: output_schema.clone(),
            };
            let machine_output = options.json || options.final_json;
            if let Err(err) = commands::exec::run(
                prompt,
                cli.model.as_deref(),
                &cli.provider,
                rlm,
                context.as_deref(),
                security,
                options,
            )
            .await
            {
                if let Some(exit_err) = err.downcast_ref::<commands::exec::ExecExitError>() {
                    if !machine_output {
                        eprintln!("{}", exit_err.message);
                    }
                    std::process::exit(exit_err.code);
                }
                return Err(err);
            }
        }
        Some(Commands::Session { action }) => match action {
            SessionAction::List { limit } => {
                let store = rot_session::SessionStore::new();
                let cwd = std::env::current_dir()?;
                let sessions = store
                    .list_recent(&cwd, limit)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;

                if sessions.is_empty() {
                    println!("No sessions found.");
                } else {
                    for s in &sessions {
                        println!(
                            "{} | {} | {} msgs | {}",
                            s.id, s.model, s.message_count, s.cwd
                        );
                    }
                }
            }
            SessionAction::Resume { id } => {
                eprintln!("Session resume not yet implemented: {id}");
            }
        },
    }

    Ok(())
}
