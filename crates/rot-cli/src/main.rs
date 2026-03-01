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
            commands::chat::run(
                cli.model.as_deref(),
                &cli.provider,
                cli.agent.as_deref(),
                security,
            )
            .await?;
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
                cli.agent.as_deref(),
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
            SessionAction::Tree { id } => {
                let store = rot_session::SessionStore::new();
                let cwd = std::env::current_dir()?;
                let tree = store
                    .tree(&cwd, id.as_deref())
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                print_session_tree(&tree.root, &tree.focus_id, "", true, true);
            }
            SessionAction::Resume { id } => {
                eprintln!("Session resume not yet implemented: {id}");
            }
        },
        Some(Commands::Tools { ref name }) => {
            let security = cli.resolve_runtime_security(&config);
            commands::tools::run(name.as_deref(), security).await?;
        }
    }

    Ok(())
}

fn print_session_tree(
    node: &rot_session::SessionTreeNode,
    focus_id: &str,
    prefix: &str,
    is_last: bool,
    is_root: bool,
) {
    let branch = if is_root {
        ""
    } else if is_last {
        "└─ "
    } else {
        "├─ "
    };
    let marker = if node.meta.id == focus_id { ">" } else { " " };
    let agent = node.meta.agent.as_deref().unwrap_or("root");
    println!(
        "{}{}{} {} @{} {} ({} msgs)",
        prefix,
        branch,
        marker,
        node.meta.id,
        agent,
        node.meta.model,
        node.meta.message_count
    );

    let child_prefix = if is_root {
        String::new()
    } else if is_last {
        format!("{}   ", prefix)
    } else {
        format!("{}│  ", prefix)
    };

    for (idx, child) in node.children.iter().enumerate() {
        print_session_tree(
            child,
            focus_id,
            &child_prefix,
            idx == node.children.len() - 1,
            false,
        );
    }
}
