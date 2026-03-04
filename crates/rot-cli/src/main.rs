//! rot — Recursive Operations Tool
//!
//! An AI-powered coding agent that runs in your terminal.

mod cli;
mod commands;
mod provider_factory;

use clap::Parser;
use cli::{Cli, Commands, SessionAction};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("\x1b[31;1mError:\x1b[0m {}", err);
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
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

    let provider_from_config = config.provider.as_str();
    let explicit_provider = cli.provider.is_some();
    let effective_provider = cli.provider.as_deref().unwrap_or(provider_from_config);
    let effective_model = match cli.model.as_deref() {
        Some(model) => Some(model),
        None if explicit_provider => None,
        None => Some(config.model.as_str()),
    };

    match cli.command {
        None | Some(Commands::Chat) => {
            let security = cli.resolve_runtime_security(&config);
            commands::chat::run(
                effective_model,
                effective_provider,
                cli.agent.as_deref(),
                security,
                cli.allow_unsafe_rlm,
            )
            .await?;
        }
        Some(Commands::Exec {
            ref prompt,
            ref session,
            fork,
            rlm,
            ref context,
            rlm_runtime,
            rlm_isolation,
            ref rlm_docker_image,
            json,
            final_json,
            ref output_schema,
            auto_approve,
            ref approve_list,
        }) => {
            let security = cli.resolve_runtime_security_for_exec(&config)?;
            let options = commands::exec::ExecOptions {
                json,
                final_json,
                output_schema: output_schema.clone(),
                auto_approve,
                approve_list: approve_list.clone(),
            };
            let machine_output = options.json || options.final_json;
            if let Err(err) = commands::exec::run(
                prompt,
                effective_model,
                effective_provider,
                cli.agent.as_deref(),
                session.as_deref(),
                fork,
                rlm,
                context.as_deref(),
                rlm_runtime.map(Into::into),
                rlm_isolation.map(Into::into),
                rlm_docker_image.clone(),
                cli.allow_unsafe_rlm,
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
                    println!("┏━━━━━━━━━━━━━━━━━━━━━━━━━━┳━━━━━━━━━━━━━━━━━━━━━━━━┳━━━━━━┳━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓");
                    println!("┃ SESSION ID               ┃ MODEL                  ┃ MSGS ┃ CWD                                                 ┃");
                    println!("┣━━━━━━━━━━━━━━━━━━━━━━━━━━╋━━━━━━━━━━━━━━━━━━━━━━━━╋━━━━━━╋━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┫");
                    for s in &sessions {
                        let id = if s.id.len() > 24 {
                            format!("{}…", &s.id[..23])
                        } else {
                            s.id.clone()
                        };
                        let model = if s.model.len() > 22 {
                            format!("{}…", &s.model[..21])
                        } else {
                            s.model.clone()
                        };
                        let cwd = if s.cwd.len() > 51 {
                            format!("…{}", &s.cwd[s.cwd.len() - 50..])
                        } else {
                            s.cwd.clone()
                        };
                        
                        println!(
                            "┃ {:<24} ┃ {:<22} ┃ {:>4} ┃ {:<51} ┃",
                            id, model, s.message_count, cwd
                        );
                    }
                    println!("┗━━━━━━━━━━━━━━━━━━━━━━━━━━┻━━━━━━━━━━━━━━━━━━━━━━━━┻━━━━━━┻━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛");
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
                let store = rot_session::SessionStore::new();
                let cwd = std::env::current_dir()?;
                let session = store
                    .load(&cwd, &id)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                println!("Session {} loaded ({} entries)", session.id, session.entries.len());
            }
            SessionAction::Export { id, output } => {
                let store = rot_session::SessionStore::new();
                let cwd = std::env::current_dir()?;
                store
                    .export_to_path(&cwd, &id, std::path::Path::new(&output))
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                println!("Exported session {} to {}", id, output);
            }
            SessionAction::Import { input, id } => {
                let store = rot_session::SessionStore::new();
                let cwd = std::env::current_dir()?;
                let session = store
                    .import_from_path(&cwd, std::path::Path::new(&input), id.as_deref())
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                println!("Imported session {}", session.id);
            }
        },
        Some(Commands::Tools { ref name }) => {
            let security = cli.resolve_runtime_security(&config);
            commands::tools::run(name.as_deref(), security).await?;
        }
        Some(Commands::Providers) => {
            commands::providers::run()?;
        }
        Some(Commands::Models) => {
            commands::models::run(effective_provider)?;
        }
        Some(Commands::Serve { ref host, port }) => {
            let security = cli.resolve_runtime_security_for_exec(&config)?;
            rot_serve::run(rot_serve::ServeOptions {
                bind: format!("{host}:{port}"),
                provider: effective_provider.to_string(),
                model: effective_model.map(str::to_string),
                agent: cli.agent.clone(),
                runtime_security: security,
            })
            .await?;
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
        "└── "
    } else {
        "├── "
    };
    let marker = if node.meta.id == focus_id { "▶" } else { " " };
    let agent = node.meta.agent.as_deref().unwrap_or("root");
    println!(
        "{}{}{} \x1b[1m{}\x1b[0m \x1b[36m@{:<8}\x1b[0m \x1b[2m{}\x1b[0m ({} msgs)",
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
        format!("{}    ", prefix)
    } else {
        format!("{}│   ", prefix)
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
