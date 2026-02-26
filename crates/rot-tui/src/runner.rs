//! TUI runner — sets up the terminal and runs the main loop.
//!
//! Agent processing runs in a background tokio task so the TUI stays
//! responsive, showing thinking animation and streaming text while
//! the LLM generates its response.

use crate::app::{App, AppState, ChatStyle, InputMode};
use crate::event::{is_quit, poll_event, TermEvent};
use crossterm::event::{EnableMouseCapture, DisableMouseCapture, KeyCode};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use rot_core::{Agent, AgentConfig, ContentBlock, Message};
use tokio::sync::mpsc;

use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Messages sent from the background processing task back to the TUI.
enum AgentEvent {
    /// Agent finished successfully with text response.
    Response {
        text: String,
        tool_names: Vec<String>,
    },
    /// Agent encountered an error.
    Error(String),
}

/// Run the TUI application.
pub async fn run_tui(
    provider: Box<dyn rot_provider::Provider>,
    tools: rot_tools::ToolRegistry,
    session_store: rot_session::SessionStore,
    model: &str,
    provider_name: &str,
) -> std::io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = App::new(model, provider_name);

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
             Be concise and helpful."
                .to_string(),
        ),
        ..Default::default()
    };

    let agent = Arc::new(Agent::new(provider, tools, config));
    let messages: Arc<Mutex<Vec<Message>>> = Arc::new(Mutex::new(Vec::new()));

    // Channel for agent results
    let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();

    // Main loop
    while app.running {
        terminal.draw(|frame| app.render(frame))?;

        // Check for agent completion (non-blocking)
        while let Ok(event) = rx.try_recv() {
            match event {
                AgentEvent::Response { text, tool_names } => {
                    app.push_chat("rot", &text, ChatStyle::Assistant);
                    for name in &tool_names {
                        app.push_chat("tool", &format!("↳ {name}"), ChatStyle::Tool);
                    }
                    app.state = AppState::Idle;
                    app.status = "Ready".to_string();
                    app.streaming_text.clear();
                }
                AgentEvent::Error(e) => {
                    app.push_chat("error", &e, ChatStyle::Error);
                    app.state = AppState::Idle;
                    app.status = "Ready".to_string();
                    app.streaming_text.clear();
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

                if app.state == AppState::Idle {
                    match app.input_mode {
                        InputMode::Insert => match key.code {
                            KeyCode::Enter => {
                                let input = app.submit_input();
                                if input.trim().is_empty() {
                                    continue;
                                }

                                if input.trim() == "/quit" || input.trim() == "/exit" {
                                    app.running = false;
                                    continue;
                                }

                                app.push_chat("you", &input, ChatStyle::User);
                                app.state = AppState::Thinking;
                                app.status = "Thinking...".to_string();
                                app.streaming_text.clear();
                                app.thinking_tick = 0;

                                // Spawn agent processing in background
                                let agent_clone = agent.clone();
                                let messages_clone = messages.clone();
                                let tx_clone = tx.clone();
                                let input_owned = input.clone();

                                tokio::spawn(async move {
                                    let mut msgs = messages_clone.lock().unwrap().clone();
                                    let result = agent_clone.process(&mut msgs, &input_owned).await;

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
                                                    if let ContentBlock::ToolCall { name, .. } = c {
                                                        Some(name.clone())
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect();

                                            AgentEvent::Response { text, tool_names }
                                        }
                                        Err(e) => AgentEvent::Error(e.to_string()),
                                    };

                                    let _ = tx_clone.send(event);
                                });
                            }
                            KeyCode::Backspace => app.backspace(),
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
                                app.scroll_offset = app.scroll_offset.saturating_add(1);
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
                    // Scroll up
                    app.auto_scroll = false;
                    app.scroll_offset = app.scroll_offset.saturating_sub((-delta) as u16);
                } else {
                    // Scroll down
                    app.scroll_offset = app.scroll_offset.saturating_add(delta as u16);
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
