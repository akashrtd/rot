//! TUI runner — sets up the terminal and runs the main loop.
//!
//! Agent processing runs in a background tokio task so the TUI stays
//! responsive, showing thinking animation and streaming text while
//! the LLM generates its response.

use crate::app::{App, AppState, ChatStyle, InputMode, ConfigUiState};
use crate::event::{is_quit, poll_event, TermEvent};
use crossterm::event::{EnableMouseCapture, DisableMouseCapture, KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use rot_core::permission::ApprovalResponse;
use rot_core::{Agent, AgentConfig, ContentBlock, Message};
use tokio::sync::{mpsc, oneshot};

use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Messages sent from the background processing task back to the TUI.
enum AgentEvent {
    Response {
        text: String,
        tool_names: Vec<String>,
        input_tokens: usize,
        output_tokens: usize,
    },
    /// Agent is requesting permission to run a tool.
    ApprovalRequest {
        tool_name: String,
        args: serde_json::Value,
        tx: oneshot::Sender<ApprovalResponse>,
    },
    /// Agent encountered an error.
    Error(String),
    /// Iterative progress update from background task.
    Progress(String),
}

/// Run the TUI application.
pub async fn run_tui(
    provider: Box<dyn rot_provider::Provider>,
    tools: rot_tools::ToolRegistry,
    session_store: rot_session::SessionStore,
    model: &str,
    provider_name: &str,
    runtime_security: rot_core::RuntimeSecurityConfig,
) -> std::io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = App::new(model, provider_name);

    // Show welcome banner
    app.show_welcome();

    // Create session
    let cwd = std::env::current_dir()?;
    let _session = session_store
        .create(&cwd, model, provider_name)
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    // Build agent (shared for background tasks)
    let config = AgentConfig {
        system_prompt: Some(
            "You are rot, a powerful AI coding assistant. \
             You have access to tools for reading, writing, and editing files, \
             running shell commands, searching code, and fetching URLs. \
             Be concise and helpful. Use markdown formatting in your responses \
             (bold for emphasis, backticks for code)."
                .to_string(),
        ),
        ..Default::default()
    };

    // Channel for agent results
    let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();

    let config_store = rot_core::config::ConfigStore::new();

    // We clone tx to use it inside the on_approval callback
    let approval_tx = tx.clone();
    let approval_tx_clone = approval_tx.clone();
    let runtime_security_for_agent = runtime_security.clone();

    let mut agent = Arc::new(
        Agent::new(
            provider,
            tools.clone(),
            config.clone(),
            runtime_security_for_agent,
        )
        .on_approval(Box::new(
            move |tool_name, args| {
                let tx_clone = approval_tx_clone.clone();
                let tool_name = tool_name.to_string();
                let args = args.clone();
                Box::pin(async move {
                    let (res_tx, res_rx) = oneshot::channel();
                    let _ = tx_clone.send(AgentEvent::ApprovalRequest {
                        tool_name,
                        args,
                        tx: res_tx,
                    });
                    res_rx.await.unwrap_or(ApprovalResponse::DenyOnce)
                })
            },
        )),
    );
    
    let messages: Arc<Mutex<Vec<Message>>> = Arc::new(Mutex::new(Vec::new()));

    // Check if the current provider needs an API key on first launch
    let has_key = config_store.load().api_keys.get(app.provider.as_str()).map(|s| !s.is_empty()).unwrap_or(false)
        || std::env::var(&format!("{}_API_KEY", app.provider.to_uppercase())).is_ok();

    if !has_key {
        app.state = AppState::Config;
        app.config_ui_state = ConfigUiState::InputKey {
            provider: app.provider.clone(),
            model: app.model.clone(),
            input: String::new(),
            cursor_pos: 0,
        };
    }

    // Main loop
    while app.running {
        terminal.draw(|frame| app.render(frame))?;

        // Check for agent completion (non-blocking)
        while let Ok(event) = rx.try_recv() {
            match event {
                AgentEvent::Response {
                    text,
                    tool_names,
                    input_tokens,
                    output_tokens,
                } => {
                    // Show tool calls before the response
                    for name in &tool_names {
                        app.push_chat("tool", &format!("↳ {name}"), ChatStyle::Tool);
                    }
                    app.push_chat("rot", &text, ChatStyle::Assistant);
                    app.stop_timer();
                    app.record_tokens(input_tokens, output_tokens);
                    app.state = AppState::Idle;
                    app.rlm_iterating = false;
                    app.status = "Ready".to_string();
                    app.streaming_text.clear();
                }
                AgentEvent::ApprovalRequest { tool_name, args, tx } => {
                    app.state = AppState::Approval;
                    app.pending_approval_tool = Some(tool_name);
                    app.pending_approval_args = Some(args);
                    app.pending_approval_tx = Some(tx);
                }
                AgentEvent::Error(e) => {
                    app.push_chat("error", &e, ChatStyle::Error);
                    app.stop_timer();
                    app.state = AppState::Idle;
                    app.rlm_iterating = false;
                    app.status = "Ready".to_string();
                    app.streaming_text.clear();
                }
                AgentEvent::Progress(_msg) => {
                    app.rlm_iterating = true;
                }
            }
        }

        // Animate thinking dots
        if app.state == AppState::Thinking || app.state == AppState::Streaming {
            app.tick();
        }

        match poll_event(Duration::from_millis(80))? {
            TermEvent::Key(key) => {
                if is_quit(&key) {
                    app.running = false;
                    continue;
                }
                if app.state == AppState::Approval {
                    let response = match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => Some(ApprovalResponse::AllowOnce),
                        KeyCode::Char('a') | KeyCode::Char('A') => Some(ApprovalResponse::AllowAlways),
                        KeyCode::Char('n') | KeyCode::Char('N') => Some(ApprovalResponse::DenyOnce),
                        KeyCode::Char('d') | KeyCode::Char('D') => Some(ApprovalResponse::DenyAlways),
                        // Also treat Esc as "No"
                        KeyCode::Esc => Some(ApprovalResponse::DenyOnce),
                        _ => None,
                    };

                    if let Some(resp) = response {
                        // Send response back to the waiting agent
                        if let Some(res_tx) = app.pending_approval_tx.take() {
                            let _ = res_tx.send(resp);
                        }
                        app.state = AppState::Thinking; // resume thinking state
                        app.pending_approval_tool = None;
                        app.pending_approval_args = None;
                    }
                    continue; // Skip normal key handling while in approval mode
                }

                if app.state == AppState::Config {
                    app.handle_config_key(key.code, &config_store);

                    if app.config_changed {
                        app.config_changed = false;
                        match create_provider(&app.provider, &app.model) {
                            Ok(new_provider) => {
                                let tx_clone = approval_tx.clone();
                agent = Arc::new(
                                    Agent::new(
                                        new_provider,
                                        tools.clone(),
                                        config.clone(),
                                        runtime_security.clone(),
                                    )
                                    .on_approval(Box::new(move |tool_name, args| {
                                        let t_clone = tx_clone.clone();
                                        let tool_name = tool_name.to_string();
                                        let args = args.clone();
                                        Box::pin(async move {
                                            let (res_tx, res_rx) = oneshot::channel();
                                            let _ = t_clone.send(AgentEvent::ApprovalRequest {
                                                tool_name,
                                                args,
                                                tx: res_tx,
                                            });
                                            res_rx.await.unwrap_or(ApprovalResponse::DenyOnce)
                                        })
                                    })),
                                );
                                app.push_chat(
                                    "system",
                                    &format!("Switched model to {} / {}", app.provider, app.model),
                                    ChatStyle::System,
                                );
                            }
                            Err(e) => {
                                app.push_chat(
                                    "error",
                                    &format!("Failed to switch model: {}", e),
                                    ChatStyle::Error,
                                );
                            }
                        }
                    }
                    continue;
                }

                if app.state == AppState::Idle {
                    match app.input_mode {
                        InputMode::Insert => match key.code {
                            KeyCode::Enter => {
                                // Shift+Enter = newline, plain Enter = send
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.insert_newline();
                                    continue;
                                }

                                if app.is_slash_menu_active() {
                                    if let Some(selected) = app.selected_slash_command() {
                                        if app.handle_slash_command(selected) {
                                            app.input.clear();
                                            app.cursor_pos = 0;
                                            app.sync_slash_menu_selection();
                                            continue;
                                        }
                                    }
                                }

                                let input = app.submit_input();
                                if input.trim().is_empty() {
                                    continue;
                                }

                                // Handle slash commands locally
                                if app.handle_slash_command(input.trim()) {
                                    continue;
                                }

                                // Regular message — send to agent
                                app.message_count += 1;
                                app.push_chat("you", &input, ChatStyle::User);
                                app.state = AppState::Thinking;
                                app.status = if app.rlm_enabled { "RLM Thinking...".to_string() } else { "Thinking...".to_string() };
                                app.streaming_text.clear();
                                app.thinking_tick = 0;
                                app.start_timer();

                                // Spawn agent processing in background
                                let agent_clone = agent.clone();
                                let messages_clone = messages.clone();
                                let tx_clone = tx.clone();
                                let progress_tx = tx.clone();
                                let input_owned = input.clone();
                                let is_rlm = app.rlm_enabled;
                                let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

                                tokio::spawn(async move {
                                    if is_rlm {
                                        let mut rlm_config = rot_rlm::RlmConfig::default();
                                        rlm_config.on_progress = Some(Arc::new(move |msg: String| {
                                            let _ = progress_tx.send(AgentEvent::Progress(msg));
                                        }));
                                        
                                        let mut engine = rot_rlm::RlmEngine::new(rlm_config, agent_clone);
                                        let result = engine.process(&input_owned, cwd.to_str().unwrap_or(".")).await;
                                        
                                        match result {
                                            Ok(ans) => {
                                                let _ = tx_clone.send(AgentEvent::Response {
                                                    text: ans,
                                                    tool_names: vec!["RLM Loop".to_string()],
                                                    input_tokens: 0,
                                                    output_tokens: 0, // Need accurate count later
                                                });
                                            }
                                            Err(e) => {
                                                let _ = tx_clone.send(AgentEvent::Error(format!("RLM Error: {}", e)));
                                            }
                                        }
                                    } else {
                                        let mut msgs = messages_clone.lock().unwrap().clone();
                                        let result =
                                            agent_clone.process(&mut msgs, &input_owned).await;

                                        // Update shared messages
                                        *messages_clone.lock().unwrap() = msgs;

                                        let event = match result {
                                        Ok(response) => {
                                            let text = response
                                                .content
                                                .iter()
                                                .filter_map(|c| {
                                                    if let ContentBlock::Text { text } = c {
                                                        Some(text.as_str())
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect::<Vec<_>>()
                                                .join("\n");

                                            let tool_names: Vec<String> = response
                                                .content
                                                .iter()
                                                .filter_map(|c| {
                                                    if let ContentBlock::ToolCall {
                                                        name, ..
                                                    } = c
                                                    {
                                                        Some(name.clone())
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect();

                                            // Estimate tokens from text (~4 chars/token)
                                            let est_output = text.len() / 4;

                                            AgentEvent::Response {
                                                text,
                                                tool_names,
                                                input_tokens: 0,
                                                output_tokens: est_output,
                                            }
                                        }
                                        Err(e) => AgentEvent::Error(e.to_string()),
                                    };

                                    let _ = tx_clone.send(event);
                                    } // End if !is_rlm
                                });
                            }
                            KeyCode::Backspace => app.backspace(),
                            KeyCode::Up => {
                                if app.is_slash_menu_active() {
                                    app.move_slash_selection_up();
                                }
                            }
                            KeyCode::Down => {
                                if app.is_slash_menu_active() {
                                    app.move_slash_selection_down();
                                }
                            }
                            KeyCode::Char(c) => app.insert_char(c),
                            KeyCode::Esc => app.input_mode = InputMode::Normal,
                            _ => {}
                        },
                        InputMode::Normal => match key.code {
                            KeyCode::Char('i') => app.input_mode = InputMode::Insert,
                            KeyCode::Char('q') => app.running = false,
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.auto_scroll = false;
                                app.scroll_offset = app.scroll_offset.saturating_sub(1);
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.scroll_offset = app.scroll_offset.saturating_add(1).min(app.max_scroll);
                            }
                            KeyCode::Char('G') => {
                                app.auto_scroll = true;
                            }
                            _ => {}
                        },
                    }
                }
            }
            TermEvent::MouseScroll(delta) => {
                if delta < 0 {
                    app.auto_scroll = false;
                    app.scroll_offset = app.scroll_offset.saturating_sub((-delta) as u16);
                } else {
                    app.scroll_offset = app.scroll_offset.saturating_add(delta as u16).min(app.max_scroll);
                }
            }
            TermEvent::Resize(_, _) => {}
            TermEvent::Tick => {}
        }
    }

    // Cleanup
    disable_raw_mode()?;
    stdout().execute(DisableMouseCapture)?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

// Helper to rebuild provider mid-session when configuration details change
fn create_provider(provider_name: &str, model: &str) -> std::result::Result<Box<dyn rot_provider::Provider>, String> {
    use rot_provider::Provider;
    match provider_name {
        "anthropic" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| "ANTHROPIC_API_KEY not set".to_string())?;
            let mut provider = rot_provider::AnthropicProvider::new(api_key);
            provider.set_model(model).map_err(|e| e.to_string())?;
            Ok(Box::new(provider))
        }
        "zai" => {
            let api_key = std::env::var("ZAI_API_KEY")
                .map_err(|_| "ZAI_API_KEY not set".to_string())?;
            let mut provider = rot_provider::new_zai_provider(api_key);
            provider.set_model(model).map_err(|e| e.to_string())?;
            Ok(Box::new(provider))
        }
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY")
                .map_err(|_| "OPENAI_API_KEY not set".to_string())?;
            let mut provider = rot_provider::new_openai_provider(api_key);
            provider.set_model(model).map_err(|e| e.to_string())?;
            Ok(Box::new(provider))
        }
        other => Err(format!("Unknown provider: {}", other)),
    }
}
