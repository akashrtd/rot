//! TUI runner — sets up the terminal and runs the main loop.

use crate::app::{App, AppState, ChatStyle, InputMode};
use crate::event::{is_quit, poll_event, TermEvent};
use crossterm::event::KeyCode;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use rot_core::{Agent, AgentConfig, ContentBlock, Message};

use std::io::stdout;
use std::time::Duration;

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
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = App::new(model, provider_name);

    // Create session
    let cwd = std::env::current_dir()?;
    let _session = session_store
        .create(&cwd, model, provider_name)
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    // Build agent

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

    let agent = Agent::new(provider, tools, config);
    let mut messages: Vec<Message> = Vec::new();

    // Main loop
    while app.running {
        terminal.draw(|frame| app.render(frame))?;

        match poll_event(Duration::from_millis(50))? {
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

                                // Process in background
                                let result = agent.process(&mut messages, &input).await;

                                match result {
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

                                        app.push_chat("rot", &text, ChatStyle::Assistant);

                                        // Show tool calls
                                        for block in &response.content {
                                            if let ContentBlock::ToolCall { name, .. } = block {
                                                app.push_chat(
                                                    "tool",
                                                    &format!("↳ {name}"),
                                                    ChatStyle::Tool,
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        app.push_chat(
                                            "error",
                                            &e.to_string(),
                                            ChatStyle::Error,
                                        );
                                    }
                                }

                                app.state = AppState::Idle;
                                app.status = "Ready".to_string();
                                app.streaming_text.clear();
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
                                app.scroll_offset = app.scroll_offset.saturating_sub(1);
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.scroll_offset = app.scroll_offset.saturating_add(1);
                            }
                            _ => {}
                        },
                    }
                }
            }
            TermEvent::Resize(_, _) => {}
            TermEvent::Tick => {}
        }
    }

    // Cleanup
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
