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

    // Initialize logging
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("rot=debug")
            .init();
    }

    match cli.command {
        None | Some(Commands::Chat) => {
            commands::chat::run(&cli.model, &cli.provider).await?;
        }
        Some(Commands::Exec { prompt }) => {
            commands::exec::run(&prompt, &cli.model, &cli.provider).await?;
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
