//! TUI runner — sets up the terminal and runs the main loop.
//!
//! Agent processing runs in a background tokio task so the TUI stays
//! responsive, showing thinking animation and streaming text while
//! the LLM generates its response.

use crate::app::{App, AppState, ChatStyle, ConfigUiState, InputMode};
use crate::event::{is_quit, poll_event, TermEvent};
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, KeyCode,
    KeyModifiers,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use rot_core::permission::ApprovalResponse;
use rot_core::{Agent, AgentConfig, AgentRegistry, ContentBlock, Message};
use rot_session::{Session, SessionEntry};
use tokio::sync::{mpsc, oneshot};

use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
    /// Streaming text emitted by provider callbacks.
    StreamTextDelta(String),
    /// Streaming thinking emitted by provider callbacks.
    StreamThinkingDelta(String),
}

/// Run the TUI application.
#[allow(clippy::too_many_arguments)]
pub async fn run_tui(
    provider: Box<dyn rot_provider::Provider>,
    tools: rot_tools::ToolRegistry,
    session_store: rot_session::SessionStore,
    model: &str,
    provider_name: &str,
    agent_name: &str,
    system_prompt: String,
    runtime_security: rot_core::RuntimeSecurityConfig,
    allow_unsafe_rlm: bool,
) -> std::io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableMouseCapture)?;
    stdout().execute(EnableBracketedPaste)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let config_store = rot_core::config::ConfigStore::new();
    let mcp_count = config_store.load().mcp_servers.iter().filter(|s| s.enabled).count();

    // Create session (needed for App::new)
    let cwd = std::env::current_dir()?;
    let mut session = session_store
        .create(&cwd, model, provider_name)
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    let mut app = App::new(model, provider_name, agent_name, mcp_count, &session.id, None);

    // Show welcome banner
    app.show_welcome();

    // Build agent (shared for background tasks)
    let config = agent_config(agent_name, Some(system_prompt));

    // Channel for agent results
    let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();

    // We clone tx to use it inside the on_approval callback
    let approval_tx = tx.clone();
    let approval_tx_clone = approval_tx.clone();
    let mut current_security = runtime_security.clone();

    let mut agent = build_agent(
        provider,
        tools.clone(),
        config.clone(),
        current_security.clone(),
        session.id.clone(),
        approval_tx_clone.clone(),
    );
    
    let messages: Arc<Mutex<Vec<Message>>> = Arc::new(Mutex::new(Vec::new()));

    // Check if the current provider needs an API key on first launch
    let has_key = config_store.load().api_keys.get(app.provider.as_str()).map(|s| !s.is_empty()).unwrap_or(false)
        || std::env::var(format!("{}_API_KEY", app.provider.to_uppercase())).is_ok();

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
    let mut needs_redraw = true;

    while app.running {
        if needs_redraw {
            terminal.draw(|frame| app.render(frame))?;
            needs_redraw = false;
        }

        if app.state == AppState::Sessions && app.sessions_ui_state.sessions.is_empty() {
            if let Ok(sessions) = session_store.list_all(&cwd).await {
                app.sessions_ui_state.sessions = sessions;
            }
        }

        // Check for agent completion (non-blocking)
        let mut handled_agent_event = false;
        while let Ok(event) = rx.try_recv() {
            handled_agent_event = true;
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
                AgentEvent::StreamTextDelta(delta) => {
                    if app.state == AppState::Thinking {
                        app.state = AppState::Streaming;
                        app.status = "Streaming...".to_string();
                    }
                    app.streaming_text.push_str(&delta);
                }
                AgentEvent::StreamThinkingDelta(delta) => {
                    if app.state == AppState::Thinking {
                        app.state = AppState::Streaming;
                        app.status = "Streaming...".to_string();
                    }
                    if !delta.trim().is_empty() {
                        if !app.streaming_text.is_empty() && !app.streaming_text.ends_with('\n') {
                            app.streaming_text.push('\n');
                        }
                        app.streaming_text.push_str("… ");
                        app.streaming_text.push_str(&delta);
                        if !app.streaming_text.ends_with('\n') {
                            app.streaming_text.push('\n');
                        }
                    }
                }
            }
        }

        if handled_agent_event {
            needs_redraw = true;
        }

        // Check if access mode changed
        if app.access_changed {
            app.access_changed = false;
            
            current_security = match app.access_mode {
                crate::app::AccessMode::Default => runtime_security.clone(),
                crate::app::AccessMode::Full => rot_core::RuntimeSecurityConfig {
                    sandbox_mode: rot_core::security::SandboxMode::DangerFullAccess,
                    approval_policy: rot_core::security::ApprovalPolicy::Never,
                    ..runtime_security.clone()
                },
            };

            match create_provider(&app.provider, &app.model) {
                Ok(new_provider) => {
                    let config = agent_config(&app.agent, None);
                    agent = build_agent(
                        new_provider,
                        tools.clone(),
                        config,
                        current_security.clone(),
                        session.id.clone(),
                        approval_tx.clone(),
                    );
                    app.push_chat(
                        "system",
                        &format!("Access mode changed to {:?}", app.access_mode),
                        ChatStyle::System,
                    );
                }
                Err(e) => {
                    app.push_chat(
                        "error",
                        &format!("Failed to rebuild agent for access mode change: {}", e),
                        ChatStyle::Error,
                    );
                }
            }
        }

        // Animate thinking dots
        if app.state == AppState::Thinking || app.state == AppState::Streaming {
            let old_tick = app.thinking_tick;
            app.tick();
            if app.thinking_tick != old_tick {
                needs_redraw = true;
            }
        }

        match poll_event(Duration::from_millis(80))? {
            TermEvent::Key(key) => {
                needs_redraw = true;
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

                if app.state == AppState::Agents {
                    match key.code {
                        KeyCode::Up => app.move_agent_selection_up(),
                        KeyCode::Down => app.move_agent_selection_down(),
                        KeyCode::Enter => {
                            let changed = app.select_current_agent();
                            if changed {
                                app.is_switching_agent = true;
                                needs_redraw = true;
                                match create_provider(&app.provider, &app.model) {
                                    Ok(new_provider) => {
                                        let profile = AgentRegistry::get(&app.agent)
                                            .unwrap_or_else(AgentRegistry::default_agent);
                                        let config = agent_config(profile.name, None);
                                        agent = build_agent(
                                            new_provider,
                                            tools.clone(),
                                            config,
                                            current_security.clone(),
                                            session.id.clone(),
                                            approval_tx.clone(),
                                        );
                                        app.push_chat(
                                            "system",
                                            &format!("Switched agent to @{}", app.agent),
                                            ChatStyle::System,
                                        );
                                    }
                                    Err(e) => {
                                        app.push_chat(
                                            "error",
                                            &format!("Failed to switch agent: {}", e),
                                            ChatStyle::Error,
                                        );
                                    }
                                }
                                app.is_switching_agent = false;
                            }
                        }
                        KeyCode::Esc => app.state = AppState::Idle,
                        _ => {}
                    }
                    continue;
                }

                if app.state == AppState::Sessions {
                    match key.code {
                        KeyCode::Esc => {
                            if app.sessions_ui_state.is_renaming {
                                app.sessions_ui_state.is_renaming = false;
                            } else {
                                app.state = AppState::Idle;
                            }
                        }
                        KeyCode::Up => {
                            if !app.sessions_ui_state.is_renaming {
                                app.sessions_ui_state.selected_index = app.sessions_ui_state.selected_index.saturating_sub(1);
                            }
                        }
                        KeyCode::Down => {
                            if !app.sessions_ui_state.is_renaming {
                                app.sessions_ui_state.selected_index += 1;
                            }
                        }
                        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if !app.sessions_ui_state.is_renaming {
                                let filtered: Vec<_> = app.sessions_ui_state.sessions.iter()
                                    .filter(|s| {
                                        let q = app.sessions_ui_state.query.to_lowercase();
                                        s.title.as_ref().map(|t| t.to_lowercase().contains(&q)).unwrap_or(false) ||
                                        s.id.to_lowercase().contains(&q)
                                    })
                                    .collect();
                                
                                if let Some(meta) = filtered.get(app.sessions_ui_state.selected_index) {
                                    let id_to_delete = meta.id.clone();
                                    if let Ok(_) = session_store.delete(&cwd, &id_to_delete).await {
                                        if let Ok(sessions) = session_store.list_all(&cwd).await {
                                            app.sessions_ui_state.sessions = sessions;
                                            app.sessions_ui_state.selected_index = app.sessions_ui_state.selected_index.min(app.sessions_ui_state.sessions.len().saturating_sub(1));
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if !app.sessions_ui_state.is_renaming {
                                let filtered: Vec<_> = app.sessions_ui_state.sessions.iter()
                                    .filter(|s| {
                                        let q = app.sessions_ui_state.query.to_lowercase();
                                        s.title.as_ref().map(|t| t.to_lowercase().contains(&q)).unwrap_or(false) ||
                                        s.id.to_lowercase().contains(&q)
                                    })
                                    .collect();

                                if let Some(meta) = filtered.get(app.sessions_ui_state.selected_index) {
                                    app.sessions_ui_state.is_renaming = true;
                                    app.sessions_ui_state.rename_input = meta.title.clone().unwrap_or_else(|| meta.id.clone());
                                }
                            }
                        }
                        KeyCode::Enter => {
                            if app.sessions_ui_state.is_renaming {
                                let filtered: Vec<_> = app.sessions_ui_state.sessions.iter()
                                    .filter(|s| {
                                        let q = app.sessions_ui_state.query.to_lowercase();
                                        s.title.as_ref().map(|t| t.to_lowercase().contains(&q)).unwrap_or(false) ||
                                        s.id.to_lowercase().contains(&q)
                                    })
                                    .collect();

                                if let Some(meta) = filtered.get(app.sessions_ui_state.selected_index) {
                                    let id = meta.id.clone();
                                    let new_title = app.sessions_ui_state.rename_input.clone();
                                    if let Ok(_) = session_store.rename(&cwd, &id, &new_title).await {
                                        app.sessions_ui_state.is_renaming = false;
                                        if let Ok(sessions) = session_store.list_all(&cwd).await {
                                            app.sessions_ui_state.sessions = sessions;
                                        }
                                    }
                                }
                            } else {
                                // Load session
                                let filtered: Vec<_> = app.sessions_ui_state.sessions.iter()
                                    .filter(|s| {
                                        let q = app.sessions_ui_state.query.to_lowercase();
                                        s.title.as_ref().map(|t| t.to_lowercase().contains(&q)).unwrap_or(false) ||
                                        s.id.to_lowercase().contains(&q)
                                    })
                                    .collect();

                                if let Some(meta) = filtered.get(app.sessions_ui_state.selected_index) {
                                    let id = meta.id.clone();
                                    if let Ok(new_session) = session_store.load(&cwd, &id).await {
                                        session = new_session;
                                        app.session_id = id.clone();
                                        app.chat_lines.clear();
                                        for entry in &session.entries {
                                            if let rot_session::SessionEntry::Message { role, content, .. } = entry {
                                                let style = match role.as_str() {
                                                    "user" => ChatStyle::User,
                                                    "assistant" => ChatStyle::Assistant,
                                                    "tool" => ChatStyle::Tool,
                                                    _ => ChatStyle::System,
                                                };
                                                // Simplified content display for now
                                                let text = if let Some(arr) = content.as_array() {
                                                    arr.iter().filter_map(|v| v.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join("")
                                                } else {
                                                    content.to_string()
                                                };
                                                app.push_chat(role, &text, style);
                                            }
                                        }
                                        
                                        // Rebuild agent with new session id
                                        match create_provider(&app.provider, &app.model) {
                                            Ok(new_provider) => {
                                                let config = agent_config(&app.agent, None);
                                                agent = build_agent(
                                                    new_provider,
                                                    tools.clone(),
                                                    config,
                                                    current_security.clone(),
                                                    session.id.clone(),
                                                    approval_tx.clone(),
                                                );
                                                app.push_chat("system", &format!("Switched to session {}", id), ChatStyle::System);
                                            }
                                            Err(_) => {}
                                        }
                                        app.state = AppState::Idle;
                                    }
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            if app.sessions_ui_state.is_renaming {
                                app.sessions_ui_state.rename_input.push(c);
                            } else {
                                app.sessions_ui_state.query.push(c);
                                app.sessions_ui_state.selected_index = 0;
                            }
                        }
                        KeyCode::Backspace => {
                            if app.sessions_ui_state.is_renaming {
                                app.sessions_ui_state.rename_input.pop();
                            } else {
                                app.sessions_ui_state.query.pop();
                                app.sessions_ui_state.selected_index = 0;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }

                if app.state == AppState::Config {
                    app.handle_config_key(key.code, &config_store);

                    if app.config_changed {
                        app.config_changed = false;
                        app.is_switching_model = true;
                        needs_redraw = true;
                                match create_provider(&app.provider, &app.model) {
                                    Ok(new_provider) => {
                                        let config = agent_config(&app.agent, None);
                                        agent = build_agent(
                                            new_provider,
                                            tools.clone(),
                                            config,
                                            runtime_security.clone(),
                                            session.id.clone(),
                                            approval_tx.clone(),
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
                        app.is_switching_model = false;
                    }
                    continue;
                }

                if app.state == AppState::Idle {
                    match app.input_mode {
                        InputMode::Insert => match key.code {
                            KeyCode::Enter => {
                                // If slash menu is active, plain Enter selects the command
                                if app.is_slash_menu_active() {
                                    if let Some(selected) = app.selected_slash_command() {
                                        if handle_session_inspection_command(
                                            &mut app,
                                            &tools,
                                            &session_store,
                                            &cwd,
                                            &session.id,
                                            selected,
                                        )
                                        .await
                                        {
                                            app.input.clear();
                                            app.cursor_pos = 0;
                                            app.sync_slash_menu_selection();
                                            continue;
                                        }
                                        if app.handle_slash_command(selected) {
                                            app.input.clear();
                                            app.cursor_pos = 0;
                                            app.sync_slash_menu_selection();
                                            continue;
                                        }
                                    }
                                }

                                // Super+Enter (Cmd+Enter on macOS) submits text. Plain Enter = newline if no slash menu.
                                let is_submit = key.modifiers.contains(KeyModifiers::SUPER) || key.modifiers.contains(KeyModifiers::META);
                                if !is_submit {
                                    app.insert_newline();
                                    continue;
                                }

                                let input = app.submit_input();
                                if input.trim().is_empty() {
                                    continue;
                                }

                                if handle_session_inspection_command(
                                    &mut app,
                                    &tools,
                                    &session_store,
                                    &cwd,
                                    &session.id,
                                    input.trim(),
                                )
                                .await
                                {
                                    continue;
                                }

                                // Handle slash commands locally
                                if app.handle_slash_command(input.trim()) {
                                    continue;
                                }

                                let (agent_for_run, prompt_for_run, routed_agent_name) =
                                    if let Some((mentioned_agent, prompt)) =
                                        App::parse_agent_mention(&input)
                                    {
                                        match create_provider(&app.provider, &app.model) {
                                            Ok(provider) => {
                                                let profile = AgentRegistry::get(&mentioned_agent)
                                                    .unwrap_or_else(AgentRegistry::default_agent);
                                                let config = agent_config(profile.name, None);
                                                let child_approval_tx = approval_tx.clone();
                                                (
                                                    build_agent(
                                                        provider,
                                                        tools.clone(),
                                                        // Pass shared configuration down to child
                                                        config,
                                                        current_security.clone(),
                                                        session.id.clone(),
                                                        child_approval_tx,
                                                    ),
                                                    prompt,
                                                    Some(profile.name.to_string()),
                                                )
                                            }
                                            Err(e) => {
                                                app.push_chat(
                                                    "error",
                                                    &format!("Failed to route to @{}: {}", mentioned_agent, e),
                                                    ChatStyle::Error,
                                                );
                                                continue;
                                            }
                                        }
                                    } else {
                                        (agent.clone(), input.clone(), None)
                                    };

                                // Regular message — send to agent
                                app.message_count += 1;
                                app.push_chat("you", &input, ChatStyle::User);
                                app.state = AppState::Thinking;
                                app.status = if app.rlm_enabled { "RLM Thinking...".to_string() } else { "Thinking...".to_string() };
                                app.streaming_text.clear();
                                app.thinking_tick = 0;
                                app.start_timer();

                                // Spawn agent processing in background
                                let messages_clone = messages.clone();
                                let tx_clone = tx.clone();
                                let progress_tx = tx.clone();
                                let input_owned = prompt_for_run.clone();
                                let routed_agent_name = routed_agent_name.clone();
                                let is_rlm = app.rlm_enabled;
                                let rlm_security = current_security.clone();
                                let allow_unsafe = allow_unsafe_rlm;
                                let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

                                let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
                                app.cancel_tx = Some(cancel_tx);

                                tokio::spawn(async move {
                                    let execution_agent = agent_for_run;
                                    
                                    let run_future = async {
                                        if is_rlm {
                                            if rlm_security.requires_explicit_rlm_opt_in() && !allow_unsafe {
                                                let _ = tx_clone.send(AgentEvent::Error(
                                                    "RLM is blocked in danger-full-access mode unless --allow-unsafe-rlm is set."
                                                        .to_string(),
                                                ));
                                                return;
                                            }

                                            let rlm_config = rot_rlm::RlmConfig {
                                                on_progress: Some(Arc::new(move |msg: String| {
                                                    let _ = progress_tx.send(AgentEvent::Progress(msg));
                                                })),
                                                runtime_security: rlm_security,
                                                ..Default::default()
                                            };
                                            
                                            let mut engine = rot_rlm::RlmEngine::new(rlm_config, execution_agent);
                                            let result = engine.process(&input_owned, cwd.to_str().unwrap_or(".")).await;
                                            
                                            match result {
                                                Ok(ans) => {
                                                    let _ = tx_clone.send(AgentEvent::Response {
                                                        text: ans,
                                                        tool_names: routed_agent_name
                                                            .map(|name| vec![format!("@{}", name), "RLM Loop".to_string()])
                                                            .unwrap_or_else(|| vec!["RLM Loop".to_string()]),
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
                                                execution_agent.process(&mut msgs, &input_owned).await;

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

                                                let mut tool_names: Vec<String> = response
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
                                                if let Some(name) = routed_agent_name {
                                                    tool_names.insert(0, format!("@{}", name));
                                                }

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
                                    };

                                    tokio::select! {
                                        _ = &mut cancel_rx => {
                                            let _ = tx_clone.send(AgentEvent::Error("Generation cancelled by user.".to_string()));
                                        }
                                        _ = run_future => {}
                                    }
                                });
                            }
                            KeyCode::Backspace => {
                                if key.modifiers.contains(KeyModifiers::SUPER) || key.modifiers.contains(KeyModifiers::META) {
                                    app.handle_slash_command("/clear");
                                } else {
                                    app.backspace();
                                }
                            }
                            KeyCode::Delete => app.delete_char(),
                            KeyCode::Left => app.cursor_left(),
                            KeyCode::Right => app.cursor_right(),
                            KeyCode::Home => app.cursor_home(),
                            KeyCode::End => app.cursor_end(),
                            KeyCode::Up => {
                                if app.is_slash_menu_active() {
                                    app.move_slash_selection_up();
                                } else {
                                    app.input_history_up();
                                }
                            }
                            KeyCode::Down => {
                                if app.is_slash_menu_active() {
                                    app.move_slash_selection_down();
                                } else {
                                    app.input_history_down();
                                }
                            }
                            KeyCode::Char(c) => {
                                let has_meta = key.modifiers.contains(KeyModifiers::SUPER) || key.modifiers.contains(KeyModifiers::META);
                                let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                                let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);

                                if (c == 'p' || c == 'P') && (has_meta || has_ctrl) {
                                    app.input.clear();
                                    app.cursor_pos = 0;
                                    app.handle_slash_command("/models");
                                } else if (c == 'c' || c == 'C') && has_meta && has_shift {
                                    if let Some(clip) = &app.clipboard {
                                        if let Some(last_msg) = app.chat_lines.iter().rev().find(|msg| msg.role == "assistant") {
                                            if let Ok(mut clipboard) = clip.lock() {
                                                let _ = clipboard.set_text(last_msg.content.clone());
                                                app.status = "Copied response!".to_string();
                                                app.copy_feedback_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
                                            }
                                        }
                                    }
                                } else if has_meta || has_ctrl || key.modifiers.contains(KeyModifiers::ALT) {
                                    // Ignore passing arbitrary modified chars to insert buffer natively
                                } else {
                                    app.insert_char(c);
                                }
                            }
                            KeyCode::Esc => {
                                if let Some(last) = app.last_esc {
                                    if last.elapsed() < Duration::from_millis(500) {
                                        if let Some(tx) = app.cancel_tx.take() {
                                            let _ = tx.send(());
                                            app.status = "Cancelled.".to_string();
                                        }
                                        app.last_esc = None;
                                    } else {
                                        app.last_esc = Some(Instant::now());
                                    }
                                } else {
                                    app.last_esc = Some(Instant::now());
                                }
                                app.input_mode = InputMode::Normal;
                            }
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
                            KeyCode::Char('g') => {
                                // Double 'g' for top
                                app.auto_scroll = false;
                                app.scroll_offset = 0;
                            }
                            KeyCode::Char('G') => {
                                app.auto_scroll = true;
                                app.scroll_offset = app.max_scroll;
                            }
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.auto_scroll = false;
                                let height = terminal.size()?.height;
                                app.scroll_offset = app.scroll_offset.saturating_sub(height / 2);
                            }
                            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                let height = terminal.size()?.height;
                                app.scroll_offset = app.scroll_offset.saturating_add(height / 2).min(app.max_scroll);
                            }
                            KeyCode::Char('y') => {
                                if let Some(msg) = app.chat_lines.last() {
                                    app.copy_to_clipboard(msg.content.clone());
                                }
                            }
                            KeyCode::Esc => {
                                if let Some(last) = app.last_esc {
                                    if last.elapsed() < Duration::from_millis(500) {
                                        if let Some(tx) = app.cancel_tx.take() {
                                            let _ = tx.send(());
                                            app.status = "Cancelled.".to_string();
                                        }
                                        app.last_esc = None;
                                    } else {
                                        app.last_esc = Some(Instant::now());
                                    }
                                } else {
                                    app.last_esc = Some(Instant::now());
                                }
                            }
                            _ => {}
                        },
                    }
                }
            }
            TermEvent::Paste(content) => {
                needs_redraw = true;
                if app.input_mode == InputMode::Insert {
                    app.insert_string(&content);
                }
            }
            TermEvent::MouseDown(x, y) => {
                needs_redraw = true;
                let size = terminal.size()?;
                if y == size.height.saturating_sub(1) && x > size.width.saturating_sub(30) {
                    app.access_mode = match app.access_mode {
                        crate::app::AccessMode::Default => crate::app::AccessMode::Full,
                        crate::app::AccessMode::Full => crate::app::AccessMode::Default,
                    };
                    app.access_changed = true;
                } else if y > 5 && y < size.height.saturating_sub(5) {
                    if let Some(msg) = app.chat_lines.last() {
                        app.copy_to_clipboard(msg.content.clone());
                    }
                }
            }
            TermEvent::MouseScroll(delta) => {
                needs_redraw = true;
                if delta < 0 {
                    app.auto_scroll = false;
                    app.scroll_offset = app.scroll_offset.saturating_sub((-delta) as u16);
                } else {
                    app.scroll_offset = app.scroll_offset.saturating_add(delta as u16).min(app.max_scroll);
                }
            }
            TermEvent::Resize(_, _) => {
                needs_redraw = true;
            }
            TermEvent::Tick => {}
        }
    }

    // Cleanup
    disable_raw_mode()?;
    stdout().execute(DisableMouseCapture)?;
    stdout().execute(DisableBracketedPaste)?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

async fn handle_session_inspection_command(
    app: &mut App,
    tools: &rot_tools::ToolRegistry,
    session_store: &rot_session::SessionStore,
    cwd: &std::path::Path,
    session_id: &str,
    command: &str,
) -> bool {
    let trimmed = command.trim();
    if trimmed == "/children" {
        match render_child_sessions_summary(session_store, cwd, session_id).await {
            Ok(summary) => app.push_chat("system", &summary, ChatStyle::System),
            Err(error) => app.push_chat(
                "error",
                &format!("Failed to inspect child sessions: {}", error),
                ChatStyle::Error,
            ),
        }
        return true;
    }

    if trimmed == "/tree" {
        match render_session_tree_summary(session_store, cwd, session_id).await {
            Ok(summary) => app.push_chat("system", &summary, ChatStyle::System),
            Err(error) => app.push_chat(
                "error",
                &format!("Failed to inspect session tree: {}", error),
                ChatStyle::Error,
            ),
        }
        return true;
    }

    if trimmed == "/tools" {
        app.push_chat("system", &render_tools_summary(tools), ChatStyle::System);
        return true;
    }

    if let Some(child_id) = trimmed.strip_prefix("/child ").map(str::trim) {
        match render_child_session_detail(session_store, cwd, session_id, child_id).await {
            Ok(detail) => app.push_chat("system", &detail, ChatStyle::System),
            Err(error) => app.push_chat(
                "error",
                &format!("Failed to inspect child session {}: {}", child_id, error),
                ChatStyle::Error,
            ),
        }
        return true;
    }

    if let Some(tool_name) = trimmed.strip_prefix("/tool ").map(str::trim) {
        match render_tool_detail(tools, tool_name) {
            Ok(detail) => app.push_chat("system", &detail, ChatStyle::System),
            Err(error) => app.push_chat("error", &error, ChatStyle::Error),
        }
        return true;
    }

    false
}

fn render_tools_summary(tools: &rot_tools::ToolRegistry) -> String {
    let mut names = tools.names();
    names.sort();

    let mut lines = vec![format!("Loaded tools ({})", names.len()), String::new()];
    for name in names {
        let description = tools
            .get(&name)
            .map(|tool| tool.description().to_string())
            .unwrap_or_else(|| "unknown tool".to_string());
        lines.push(format!("{name} [{}]", tool_kind(&name)));
        lines.push(format!("  {}", truncate_line(&description, 88)));
    }
    lines.push(String::new());
    lines.push("Use /tool <name> to inspect a tool schema.".to_string());
    lines.join("\n")
}

fn render_tool_detail(tools: &rot_tools::ToolRegistry, name: &str) -> Result<String, String> {
    let tool = tools
        .get(name)
        .ok_or_else(|| format!("Unknown tool: {name}"))?;
    let schema = serde_json::to_string_pretty(&tool.parameters_schema())
        .map_err(|e| format!("Failed to render schema: {e}"))?;

    Ok(format!(
        "Tool {}\n\nkind        {}\nlabel       {}\ndescription {}\nparameters\n{}",
        tool.name(),
        tool_kind(tool.name()),
        tool.label(),
        tool.description(),
        schema
    ))
}

fn tool_kind(name: &str) -> &'static str {
    if name.starts_with("mcp__") {
        "mcp"
    } else if matches!(
        name,
        "read" | "write" | "edit" | "bash" | "glob" | "grep" | "task" | "webfetch"
    ) {
        "builtin"
    } else {
        "custom"
    }
}

async fn render_child_sessions_summary(
    session_store: &rot_session::SessionStore,
    cwd: &std::path::Path,
    session_id: &str,
) -> Result<String, String> {
    let session = session_store
        .load(cwd, session_id)
        .await
        .map_err(|e| e.to_string())?;
    let links = session
        .entries
        .iter()
        .filter_map(|entry| match entry {
            SessionEntry::ChildSessionLink {
                child_session_id,
                agent,
                prompt,
                ..
            } => Some((child_session_id.clone(), agent.clone(), prompt.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();

    if links.is_empty() {
        return Ok("No delegated child sessions for this parent session.".to_string());
    }

    let mut lines = vec![
        "Delegated child sessions:".to_string(),
        String::new(),
    ];

    for (child_session_id, agent, prompt) in links {
        let preview = match session_store.load(cwd, &child_session_id).await {
            Ok(child) => child_session_preview(&child),
            Err(_) => "(child session unavailable)".to_string(),
        };
        lines.push(format!(
            "{}  @{}  {}",
            child_session_id,
            agent,
            truncate_line(&prompt, 56)
        ));
        lines.push(format!("  {}", truncate_line(&preview, 84)));
        lines.push(String::new());
    }

    lines.push("Use /child <id> to inspect a full child transcript.".to_string());
    Ok(lines.join("\n"))
}

async fn render_child_session_detail(
    session_store: &rot_session::SessionStore,
    cwd: &std::path::Path,
    parent_session_id: &str,
    child_session_id: &str,
) -> Result<String, String> {
    let parent = session_store
        .load(cwd, parent_session_id)
        .await
        .map_err(|e| e.to_string())?;
    let is_linked = parent.entries.iter().any(|entry| {
        matches!(
            entry,
            SessionEntry::ChildSessionLink {
                child_session_id: linked_id,
                ..
            } if linked_id == child_session_id
        )
    });

    if !is_linked {
        return Err("child session is not linked to the current parent session".to_string());
    }

    let child = session_store
        .load(cwd, child_session_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(format_child_session_detail(&child))
}

fn child_session_preview(session: &Session) -> String {
    session
        .entries
        .iter()
        .rev()
        .find_map(|entry| match entry {
            SessionEntry::Message { role, content, .. } if role == "assistant" => {
                extract_text_from_content(content)
            }
            SessionEntry::ToolResult { output, .. } => Some(output.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "No transcript captured yet.".to_string())
}

async fn render_session_tree_summary(
    session_store: &rot_session::SessionStore,
    cwd: &std::path::Path,
    session_id: &str,
) -> Result<String, String> {
    let tree = session_store
        .tree(cwd, Some(session_id))
        .await
        .map_err(|e| e.to_string())?;

    let mut lines = vec!["Session tree:".to_string(), String::new()];
    append_session_tree_lines(&tree.root, &tree.focus_id, "", true, true, &mut lines);
    Ok(lines.join("\n"))
}

fn append_session_tree_lines(
    node: &rot_session::SessionTreeNode,
    focus_id: &str,
    prefix: &str,
    is_last: bool,
    is_root: bool,
    lines: &mut Vec<String>,
) {
    let branch = if is_root {
        ""
    } else if is_last {
        "└─ "
    } else {
        "├─ "
    };
    let marker = if node.meta.id == focus_id { ">" } else { " " };
    lines.push(format!(
        "{}{}{} {} @{} {}",
        prefix,
        branch,
        marker,
        node.meta.id,
        node.meta.agent.as_deref().unwrap_or("root"),
        node.meta.model
    ));

    let next_prefix = if is_root {
        String::new()
    } else if is_last {
        format!("{}   ", prefix)
    } else {
        format!("{}│  ", prefix)
    };

    for (idx, child) in node.children.iter().enumerate() {
        append_session_tree_lines(
            child,
            focus_id,
            &next_prefix,
            idx == node.children.len() - 1,
            false,
            lines,
        );
    }
}

fn format_child_session_detail(session: &Session) -> String {
    let mut lines = vec![format!("Child session {}", session.id), String::new()];

    for entry in &session.entries {
        match entry {
            SessionEntry::SessionStart {
                model,
                provider,
                agent,
                ..
            } => {
                lines.push(format!(
                    "start   provider={} model={} agent={}",
                    provider,
                    model,
                    agent.clone().unwrap_or_else(|| "unknown".to_string())
                ));
            }
            SessionEntry::Message { role, content, .. } => {
                let text = extract_text_from_content(content)
                    .unwrap_or_else(|| "(non-text content)".to_string());
                lines.push(format!("{role:<7} {}", truncate_line(&text, 100)));
            }
            SessionEntry::ToolCall {
                name, arguments, ..
            } => {
                lines.push(format!(
                    "tool    {} {}",
                    name,
                    truncate_line(&arguments.to_string(), 88)
                ));
            }
            SessionEntry::ToolResult {
                output, is_error, ..
            } => {
                let prefix = if *is_error { "error" } else { "result" };
                lines.push(format!("{prefix:<7} {}", truncate_line(output, 100)));
            }
            SessionEntry::ChildSessionLink { .. }
            | SessionEntry::Compaction { .. }
            | SessionEntry::Branch { .. } => {}
            SessionEntry::Artifact { kind, path, .. } => {
                lines.push(format!("meta    {} {}", kind, truncate_line(path, 88)));
            }
        }
    }

    lines.join("\n")
}

fn extract_text_from_content(content: &serde_json::Value) -> Option<String> {
    let blocks = content.as_array()?;
    let text = blocks
        .iter()
        .filter_map(|block| {
            (block.get("type")?.as_str()? == "text")
                .then(|| block.get("text")?.as_str().map(str::to_string))
                .flatten()
        })
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn truncate_line(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

// Helper to rebuild provider mid-session when configuration details change
fn create_provider(provider_name: &str, model: &str) -> std::result::Result<Box<dyn rot_provider::Provider>, String> {
    use rot_provider::Provider;
    match provider_name {
        "anthropic" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| "ANTHROPIC_API_KEY not set".to_string())?;
            let mut provider = rot_provider::AnthropicProvider::new(api_key, vec![]);
            provider.set_model(model).map_err(|e| e.to_string())?;
            Ok(Box::new(provider))
        }
        "zai" => {
            let api_key = std::env::var("ZAI_API_KEY")
                .map_err(|_| "ZAI_API_KEY not set".to_string())?;
            let mut provider = rot_provider::new_zai_provider(api_key, vec![]);
            provider.set_model(model).map_err(|e| e.to_string())?;
            Ok(Box::new(provider))
        }
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY")
                .map_err(|_| "OPENAI_API_KEY not set".to_string())?;
            let mut provider = rot_provider::new_openai_provider(api_key, vec![]);
            provider.set_model(model).map_err(|e| e.to_string())?;
            Ok(Box::new(provider))
        }
        "ollama" => {
            let mut provider = rot_provider::new_ollama_provider(String::new(), vec![]);
            provider.set_model(model).map_err(|e| e.to_string())?;
            Ok(Box::new(provider))
        }
        "openrouter" => {
            let api_key = std::env::var("OPENROUTER_API_KEY")
                .map_err(|_| "OPENROUTER_API_KEY not set".to_string())?;
            let mut provider = rot_provider::new_openrouter_provider(api_key, vec![]);
            provider.set_model(model).map_err(|e| e.to_string())?;
            Ok(Box::new(provider))
        }
        "google" => {
            let api_key = std::env::var("GOOGLE_API_KEY")
                .or_else(|_| std::env::var("GEMINI_API_KEY"))
                .map_err(|_| "GOOGLE_API_KEY (or GEMINI_API_KEY) not set".to_string())?;
            let mut provider = rot_provider::new_google_provider(api_key, vec![]);
            provider.set_model(model).map_err(|e| e.to_string())?;
            Ok(Box::new(provider))
        }
        other => Err(format!("Unknown provider: {}", other)),
    }
}

fn build_agent(
    provider: Box<dyn rot_provider::Provider>,
    tools: rot_tools::ToolRegistry,
    config: AgentConfig,
    runtime_security: rot_core::RuntimeSecurityConfig,
    session_id: String,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
) -> Arc<Agent> {
    let stream_tx = event_tx.clone();
    let approval_tx = event_tx.clone();
    Arc::new(
        Agent::new(provider, tools, config, runtime_security)
            .with_session_id(session_id)
            .on_event(Box::new(move |event| match event {
                rot_provider::StreamEvent::TextDelta { delta } => {
                    let _ = stream_tx.send(AgentEvent::StreamTextDelta(delta.clone()));
                }
                rot_provider::StreamEvent::ThinkingDelta { delta } => {
                    let _ = stream_tx.send(AgentEvent::StreamThinkingDelta(delta.clone()));
                }
                _ => {}
            }))
            .on_approval(Box::new(move |tool_name, args| {
                let tx_clone = approval_tx.clone();
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
            })),
    )
}

fn agent_config(agent_name: &str, initial_system_prompt: Option<String>) -> AgentConfig {
    let system_prompt = if let Some(system_prompt) = initial_system_prompt {
        system_prompt
    } else if agent_name.eq_ignore_ascii_case("default") {
        AgentRegistry::default_chat_system_prompt().to_string()
    } else {
        AgentRegistry::get(agent_name)
            .unwrap_or_else(AgentRegistry::default_agent)
            .system_prompt
            .to_string()
    };

    AgentConfig {
        agent_name: agent_name.to_string(),
        system_prompt: Some(system_prompt),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_tools_summary_includes_builtin_tool() {
        let mut tools = rot_tools::ToolRegistry::new();
        rot_tools::register_all(&mut tools);
        let expected_count = tools.names().len();

        let summary = render_tools_summary(&tools);
        assert!(summary.contains(&format!("Loaded tools ({expected_count})")));
        assert!(summary.contains("read [builtin]"));
    }

    #[test]
    fn test_render_tool_detail_includes_schema() {
        let mut tools = rot_tools::ToolRegistry::new();
        rot_tools::register_all(&mut tools);

        let detail = render_tool_detail(&tools, "read").unwrap();
        assert!(detail.contains("Tool read"));
        assert!(detail.contains("\"path\""));
    }

    #[test]
    fn test_extract_text_from_content() {
        let content = serde_json::json!([
            {"type":"text","text":"hello"},
            {"type":"tool_call","id":"1","name":"read","arguments":{}},
            {"type":"text","text":"world"}
        ]);

        assert_eq!(
            extract_text_from_content(&content).as_deref(),
            Some("hello\nworld")
        );
    }

    #[test]
    fn test_format_child_session_detail_includes_messages() {
        let session = Session {
            id: "child-1".to_string(),
            file_path: std::path::PathBuf::from("child-1.jsonl"),
            cwd: std::path::PathBuf::from("."),
            current_leaf: "msg-1".to_string(),
            entries: vec![
                SessionEntry::SessionStart {
                    id: "child-1".to_string(),
                    timestamp: 1,
                    cwd: ".".to_string(),
                    model: "gpt-4o".to_string(),
                    provider: "openai".to_string(),
                    parent_session_id: Some("parent-1".to_string()),
                    parent_tool_call_id: None,
                    agent: Some("review".to_string()),
                    title: None,
                },
                SessionEntry::Message {
                    id: "msg-1".to_string(),
                    parent_id: None,
                    timestamp: 2,
                    role: "assistant".to_string(),
                    content: serde_json::json!([{"type":"text","text":"done"}]),
                },
            ],
        };

        let formatted = format_child_session_detail(&session);
        assert!(formatted.contains("Child session child-1"));
        assert!(formatted.contains("assistant done"));
    }

    #[test]
    fn test_append_session_tree_lines_renders_children() {
        let tree = rot_session::SessionTreeNode {
            meta: rot_session::SessionMeta {
                id: "root".to_string(),
                created_at: 1,
                updated_at: 1,
                title: None,
                cwd: ".".to_string(),
                model: "claude".to_string(),
                provider: "anthropic".to_string(),
                parent_session_id: None,
                agent: None,
                message_count: 1,
            },
            children: vec![rot_session::SessionTreeNode {
                meta: rot_session::SessionMeta {
                    id: "child".to_string(),
                    created_at: 2,
                    updated_at: 2,
                    title: None,
                    cwd: ".".to_string(),
                    model: "claude".to_string(),
                    provider: "anthropic".to_string(),
                    parent_session_id: Some("root".to_string()),
                    agent: Some("review".to_string()),
                    message_count: 1,
                },
                children: Vec::new(),
            }],
        };

        let mut lines = Vec::new();
        append_session_tree_lines(&tree, "child", "", true, true, &mut lines);
        assert!(lines.iter().any(|line| line.contains("root @root")));
        assert!(lines.iter().any(|line| line.contains("└─ > child @review")));
    }
}
