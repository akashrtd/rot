//! TUI application state and rendering.
//!
//! Design principles (from ratatui best practices research):
//! - Visual hierarchy through weight (bold, dim, normal) and color
//! - Breathing room with spacing instead of heavy borders
//! - Centralized theme constants for consistency
//! - Clean minimal layout: header | messages | context bar | input | footer

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use std::time::{Duration, Instant};

// ── Theme (Tokyo Night) ───────────────────────────────────────────────

const COLOR_USER: Color = Color::Rgb(42, 195, 222);      // Cyan #2ac3de
const COLOR_ASSISTANT: Color = Color::Rgb(115, 218, 202); // Teal #73daca
const COLOR_TOOL: Color = Color::Rgb(187, 154, 247);      // Purple #bb9af7
const COLOR_ERROR: Color = Color::Rgb(247, 118, 142);     // Red #f7768e
const COLOR_SYSTEM: Color = Color::Rgb(224, 175, 104);    // Yellow #e0af68
const COLOR_THINKING: Color = Color::Rgb(86, 95, 137);    // Comment #565f89
const COLOR_CODE_BG: Color = Color::Rgb(26, 27, 38);      // Background #1a1b26
const COLOR_CODE_FG: Color = Color::Rgb(192, 202, 245);   // Foreground #c0caf5
const COLOR_HEADER_BG: Color = Color::Rgb(22, 22, 30);    // Darker bg #16161e
const COLOR_ACCENT: Color = Color::Rgb(122, 162, 247);    // Blue #7aa2f7
const COLOR_BAR_BG: Color = Color::Rgb(22, 22, 30);       // Darker bg #16161e
const COLOR_BAR_FG: Color = Color::Rgb(86, 95, 137);      // Comment #565f89
const COLOR_BORDER: Color = Color::Rgb(41, 46, 66);       // Surface #292e42
const COLOR_DIM: Color = Color::Rgb(86, 95, 137);         // Comment #565f89
const COLOR_BANNER: Color = Color::Rgb(122, 162, 247);    // Blue #7aa2f7

// ── ASCII Art ──────────────────────────────────────────────────────────

const ASCII_BANNER: &str = "

  ███████████     ███████    ███████████
░░███░░░░░███   ███░░░░░███ ░█░░░███░░░█
 ░███    ░███  ███     ░░███░   ░███  ░
 ░██████████  ░███      ░███    ░███
 ░███░░░░░███ ░███      ░███    ░███
 ░███    ░███ ░░███     ███     ░███
 █████   █████ ░░░███████░      █████
░░░░░   ░░░░░    ░░░░░░░       ░░░░░

";

// Provider context window sizes (approximate)
const CONTEXT_WINDOWS: &[(&str, usize)] = &[
    ("claude-sonnet-4-20250514", 200_000),
    ("glm-5", 128_000),
    ("glm-4.7", 128_000),
];

fn get_context_window(model: &str) -> usize {
    CONTEXT_WINDOWS
        .iter()
        .find(|(m, _)| *m == model)
        .map(|(_, w)| *w)
        .unwrap_or(128_000)
}

// ── State ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Idle,
    Thinking,
    Streaming,
    Approval, // Paused for user permission
    Agents,   // Agent selection overlay
    Config,   // Model & API Key overlay
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigUiState {
    List(usize), // index of selected model
    InputKey { provider: String, model: String, input: String, cursor_pos: usize },
}

impl Default for ConfigUiState {
    fn default() -> Self {
        Self::List(0)
    }
}

pub const AVAILABLE_MODELS: &[(&str, &str)] = &[
    ("anthropic", "claude-3-7-sonnet-latest"),
    ("anthropic", "claude-3-5-sonnet-latest"),
    ("anthropic", "claude-3-5-haiku-latest"),
    ("zai", "glm-5"),
    ("zai", "glm-4.7"),
    ("openai", "gpt-4o"),
    ("openai", "gpt-4o-mini"),
];

pub const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/agents", "Switch agent"),
    ("/children", "Inspect delegated child runs"),
    ("/tools", "List loaded tools"),
    ("/tree", "Show session tree"),
    ("/help", "Show help"),
    ("/clear", "Clear conversation"),
    ("/models", "Switch model"),
    ("/model", "Switch model"),
    ("/rlm", "Toggle RLM"),
    ("/quit", "Exit app"),
    ("/exit", "Exit app"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
}

pub struct App {
    pub state: AppState,
    pub input_mode: InputMode,
    pub running: bool,
    pub input: String,
    pub cursor_pos: usize,
    pub chat_lines: Vec<ChatLine>,
    pub streaming_text: String,
    pub scroll_offset: u16,
    pub auto_scroll: bool,
    pub status: String,
    pub model: String,
    pub provider: String,
    pub agent: String,
    pub thinking_tick: u16,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub last_input_tokens: usize,
    pub last_output_tokens: usize,
    pub response_start: Option<Instant>,
    pub last_elapsed: Option<Duration>,
    pub message_count: usize,
    pub showed_welcome: bool,
    /// Maximum scroll offset (computed during render).
    pub max_scroll: u16,
    
    // Approval state
    pub pending_approval_tool: Option<String>,
    pub pending_approval_args: Option<serde_json::Value>,
    pub pending_approval_tx: Option<tokio::sync::oneshot::Sender<rot_core::permission::ApprovalResponse>>,
    pub rlm_enabled: bool,
    pub rlm_iterating: bool,
    
    // Config state
    pub config_ui_state: ConfigUiState,
    pub config_changed: bool,
    pub agent_changed: bool,
    pub slash_menu_selected: usize,
    pub agent_menu_selected: usize,
}

#[derive(Debug, Clone)]
pub struct ChatLine {
    pub role: String,
    pub content: String,
    pub style: ChatStyle,
}

#[derive(Debug, Clone, Copy)]
pub enum ChatStyle {
    User,
    Assistant,
    System,
    Tool,
    Error,
    Thinking,
    Welcome,
}

// ── App Implementation ─────────────────────────────────────────────────

impl App {
    pub fn new(model: &str, provider: &str, agent: &str) -> Self {
        Self {
            state: AppState::Idle,
            input_mode: InputMode::Insert,
            running: true,
            input: String::new(),
            cursor_pos: 0,
            chat_lines: Vec::new(),
            streaming_text: String::new(),
            scroll_offset: 0,
            auto_scroll: true,
            status: "Ready".to_string(),
            model: model.to_string(),
            provider: provider.to_string(),
            agent: agent.to_string(),
            thinking_tick: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            last_input_tokens: 0,
            last_output_tokens: 0,
            response_start: None,
            last_elapsed: None,
            message_count: 0,
            showed_welcome: false,
            max_scroll: 0,
            pending_approval_tool: None,
            pending_approval_args: None,
            pending_approval_tx: None,
            rlm_enabled: true,
            rlm_iterating: false,
            config_ui_state: ConfigUiState::default(),
            config_changed: false,
            agent_changed: false,
            slash_menu_selected: 0,
            agent_menu_selected: 0,
        }
    }

    // ── Welcome ────────────────────────────────────────────────────────

    pub fn show_welcome(&mut self) {
        if self.showed_welcome {
            return;
        }
        self.showed_welcome = true;

        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let short_cwd = cwd
            .rsplit('/')
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("/");

        let welcome = format!(
            "{}\n\
             ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓\n\
             ┃  provider : {:<23}┃\n\
             ┃  model    : {:<23}┃\n\
             ┃  agent    : {:<23}┃\n\
             ┃  cwd      : {:<23}┃\n\
             ┃  rlm      : {:<23}┃\n\
             ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛\n\
             \n\
             Type a message or use /help for commands.",
            ASCII_BANNER.trim_matches('\n'),
            self.provider,
            self.model,
            self.agent,
            if short_cwd.len() > 23 {
                format!("…{}", &short_cwd[short_cwd.len().saturating_sub(22)..])
            } else {
                short_cwd
            },
            if self.rlm_enabled { "on" } else { "off" }
        );
        self.push_chat("", &welcome, ChatStyle::Welcome);
    }

    // ── State mutators ─────────────────────────────────────────────────

    pub fn push_chat(&mut self, role: &str, content: &str, style: ChatStyle) {
        self.chat_lines.push(ChatLine {
            role: role.to_string(),
            content: content.to_string(),
            style,
        });
        self.auto_scroll = true;
    }

    pub fn start_timer(&mut self) {
        self.response_start = Some(Instant::now());
    }

    pub fn stop_timer(&mut self) {
        if let Some(start) = self.response_start.take() {
            self.last_elapsed = Some(start.elapsed());
        }
    }

    pub fn record_tokens(&mut self, input: usize, output: usize) {
        self.last_input_tokens = input;
        self.last_output_tokens = output;
        self.total_input_tokens += input;
        self.total_output_tokens += output;
    }

    pub fn handle_slash_command(&mut self, cmd: &str) -> bool {
        let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
        match parts[0] {
            "/help" => {
                self.push_chat(
                    "system",
                    "/agents     — switch active agent\n\
                     /children   — list delegated child runs\n\
                     /child ID   — inspect one child session\n\
                     /tools      — list loaded tools\n\
                     /tool NAME  — inspect one tool\n\
                     /tree       — show current session tree\n\
                     /help       — show this message\n\
                     /clear      — clear conversation\n\
                     /model      — show current model\n\
                     /model NAME — switch model\n\
                     /rlm        — toggle RLM engine on/off\n\
                     /quit       — exit rot",
                    ChatStyle::System,
                );
                true
            }
            "/clear" => {
                self.chat_lines.clear();
                self.message_count = 0;
                self.total_input_tokens = 0;
                self.total_output_tokens = 0;
                self.push_chat("system", "Conversation cleared.", ChatStyle::System);
                true
            }
            "/agents" => {
                self.state = AppState::Agents;
                self.sync_agent_menu_selection();
                true
            }
            "/children" | "/tree" | "/tools" => false,
            _ if cmd.starts_with("/child ") || cmd.starts_with("/tool ") => false,
            "/models" | "/model" => {
                self.state = AppState::Config;
                self.config_ui_state = ConfigUiState::List(0);
                true
            }
            "/rlm" => {
                self.rlm_enabled = !self.rlm_enabled;
                let state_str = if self.rlm_enabled { "ON" } else { "OFF" };
                self.push_chat(
                    "system",
                    &format!("RLM Engine is now {}", state_str),
                    ChatStyle::System,
                );
                true
            }
            "/quit" | "/exit" => {
                self.running = false;
                true
            }
            _ if cmd.starts_with('/') => {
                self.push_chat(
                    "system",
                    &format!("Unknown: {}. Try /help", parts[0]),
                    ChatStyle::System,
                );
                true
            }
            _ => false,
        }
    }

    pub fn submit_input(&mut self) -> String {
        let text = self.input.clone();
        self.input.clear();
        self.cursor_pos = 0;
        self.slash_menu_selected = 0;
        text
    }

    pub fn all_agents(&self) -> Vec<rot_core::AgentProfile> {
        rot_core::AgentRegistry::builtins().to_vec()
    }

    pub fn sync_agent_menu_selection(&mut self) {
        let agents = self.all_agents();
        self.agent_menu_selected = agents
            .iter()
            .position(|profile| profile.name == self.agent)
            .unwrap_or(self.agent_menu_selected.min(agents.len().saturating_sub(1)));
    }

    pub fn move_agent_selection_up(&mut self) {
        let count = self.all_agents().len();
        if count == 0 {
            return;
        }
        if self.agent_menu_selected == 0 {
            self.agent_menu_selected = count - 1;
        } else {
            self.agent_menu_selected -= 1;
        }
    }

    pub fn move_agent_selection_down(&mut self) {
        let count = self.all_agents().len();
        if count == 0 {
            return;
        }
        self.agent_menu_selected = (self.agent_menu_selected + 1) % count;
    }

    pub fn select_current_agent(&mut self) -> bool {
        let agents = self.all_agents();
        let Some(profile) = agents.get(self.agent_menu_selected).copied() else {
            return false;
        };

        let changed = self.agent != profile.name;
        self.agent = profile.name.to_string();
        self.state = AppState::Idle;
        self.agent_changed = changed;
        changed
    }

    pub fn parse_agent_mention(input: &str) -> Option<(String, String)> {
        let trimmed = input.trim();
        let rest = trimmed.strip_prefix('@')?;
        let (name, prompt) = rest.split_once(char::is_whitespace)?;
        let profile = rot_core::AgentRegistry::get(name)?;
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return None;
        }
        Some((profile.name.to_string(), prompt.to_string()))
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
        self.sync_slash_menu_selection();
    }

    pub fn insert_newline(&mut self) {
        self.input.insert(self.cursor_pos, '\n');
        self.cursor_pos += 1;
        self.sync_slash_menu_selection();
    }

    pub fn handle_config_key(
        &mut self,
        key_code: crossterm::event::KeyCode,
        config_store: &rot_core::config::ConfigStore,
    ) {
        match &mut self.config_ui_state {
            ConfigUiState::List(idx) => match key_code {
                crossterm::event::KeyCode::Up => *idx = idx.saturating_sub(1),
                crossterm::event::KeyCode::Down => {
                    *idx = (*idx + 1).min(AVAILABLE_MODELS.len().saturating_sub(1))
                }
                crossterm::event::KeyCode::Enter => {
                    let (provider, model) = AVAILABLE_MODELS[*idx];
                    let mut config = config_store.load();

                    let has_key = config.api_keys.get(provider).map(|s| !s.is_empty()).unwrap_or(false)
                        || std::env::var(format!("{}_API_KEY", provider.to_uppercase())).is_ok();

                    if has_key {
                        config.provider = provider.to_string();
                        config.model = model.to_string();
                        let _ = config_store.save(&config);
                        self.provider = provider.to_string();
                        self.model = model.to_string();
                        self.state = AppState::Idle;
                        self.config_changed = true;
                    } else {
                        self.config_ui_state = ConfigUiState::InputKey {
                            provider: provider.to_string(),
                            model: model.to_string(),
                            input: String::new(),
                            cursor_pos: 0,
                        };
                    }
                }
                crossterm::event::KeyCode::Esc => self.state = AppState::Idle,
                _ => {}
            },
            ConfigUiState::InputKey {
                provider,
                model,
                input,
                cursor_pos,
            } => match key_code {
                crossterm::event::KeyCode::Char(c) => {
                    input.insert(*cursor_pos, c);
                    *cursor_pos += c.len_utf8();
                }
                crossterm::event::KeyCode::Backspace => {
                    if *cursor_pos > 0 {
                        let prev = input[..*cursor_pos]
                            .char_indices()
                            .next_back()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        input.remove(prev);
                        *cursor_pos = prev;
                    }
                }
                crossterm::event::KeyCode::Delete => {
                    if *cursor_pos < input.len() {
                        input.remove(*cursor_pos);
                    }
                }
                crossterm::event::KeyCode::Left => {
                    if *cursor_pos > 0 {
                        *cursor_pos = input[..*cursor_pos]
                            .char_indices()
                            .next_back()
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                    }
                }
                crossterm::event::KeyCode::Right => {
                    if *cursor_pos < input.len() {
                        *cursor_pos += input[*cursor_pos..]
                            .chars()
                            .next()
                            .map(|c| c.len_utf8())
                            .unwrap_or(0);
                    }
                }
                crossterm::event::KeyCode::Home => {
                    *cursor_pos = 0;
                }
                crossterm::event::KeyCode::End => {
                    *cursor_pos = input.len();
                }
                crossterm::event::KeyCode::Enter => {
                    let api_key = input.trim();
                    if !api_key.is_empty() {
                        let mut config = config_store.load();
                        config.api_keys.insert(provider.clone(), api_key.to_string());
                        config.provider = provider.clone();
                        config.model = model.clone();
                        let _ = config_store.save(&config);
                        config_store.hydrate_env();

                        self.provider = provider.clone();
                        self.model = model.clone();
                        self.state = AppState::Idle;
                        self.config_ui_state = ConfigUiState::List(0);
                        self.config_changed = true;
                    }
                }
                crossterm::event::KeyCode::Esc => {
                    self.state = AppState::Idle;
                    self.config_ui_state = ConfigUiState::List(0);
                }
                _ => {}
            },
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input.remove(prev);
            self.cursor_pos = prev;
            self.sync_slash_menu_selection();
        }
    }

    pub fn is_slash_menu_active(&self) -> bool {
        self.state == AppState::Idle
            && self.input_mode == InputMode::Insert
            && self.input.starts_with('/')
    }

    pub fn filtered_slash_commands(&self) -> Vec<(&'static str, &'static str)> {
        if !self.is_slash_menu_active() {
            return Vec::new();
        }

        let query = self.input.trim();
        SLASH_COMMANDS
            .iter()
            .copied()
            .filter(|(name, _)| query == "/" || name.starts_with(query))
            .collect()
    }

    pub fn sync_slash_menu_selection(&mut self) {
        let count = self.filtered_slash_commands().len();
        if count == 0 {
            self.slash_menu_selected = 0;
        } else {
            self.slash_menu_selected = self.slash_menu_selected.min(count.saturating_sub(1));
        }
    }

    pub fn move_slash_selection_up(&mut self) {
        let count = self.filtered_slash_commands().len();
        if count == 0 {
            return;
        }
        if self.slash_menu_selected == 0 {
            self.slash_menu_selected = count - 1;
        } else {
            self.slash_menu_selected -= 1;
        }
    }

    pub fn move_slash_selection_down(&mut self) {
        let count = self.filtered_slash_commands().len();
        if count == 0 {
            return;
        }
        self.slash_menu_selected = (self.slash_menu_selected + 1) % count;
    }

    pub fn selected_slash_command(&self) -> Option<&'static str> {
        let commands = self.filtered_slash_commands();
        if commands.is_empty() {
            None
        } else {
            Some(commands[self.slash_menu_selected].0)
        }
    }

    pub fn tick(&mut self) {
        self.thinking_tick = self.thinking_tick.wrapping_add(1);
    }

    // ── Rendering ──────────────────────────────────────────────────────

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Thick outer border around entire TUI
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_ACCENT))
            .border_type(ratatui::widgets::BorderType::Thick)
            .title(" rot ")
            .title_style(Style::default().fg(COLOR_ACCENT).bold());
        let inner = outer_block.inner(area);
        frame.render_widget(outer_block, area);

        // Layout: header(1) | messages(flex) | input(3) | footer(1)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Min(5),   // Messages
                Constraint::Length(3), // Input
                Constraint::Length(1), // Footer (provider + context + tokens + cost)
            ])
            .split(inner);

        self.render_header(frame, chunks[0]);
        self.render_messages(frame, chunks[1]);
        self.render_input(frame, chunks[2]);
        self.render_footer(frame, chunks[3]);
        self.render_slash_menu(frame, chunks[2]);

        // Overlay dialog
        if self.state == AppState::Approval {
            self.render_approval_dialog(frame, area);
        } else if self.state == AppState::Config {
            self.render_config_dialog(frame, area);
        } else if self.state == AppState::Agents {
            self.render_agents_dialog(frame, area);
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        // Left: status indicator + state
        let spinner = match self.state {
            AppState::Idle => "●",
            AppState::Thinking => {
                match (self.thinking_tick / 3) % 4 {
                    0 => "⠋", 1 => "⠙", 2 => "⠸", _ => "⠴",
                }
            }
            AppState::Streaming => "◉",
            AppState::Approval => "⚠",
            AppState::Agents => "◈",
            AppState::Config => "⚙",
            AppState::Error => "✖",
        };

        let state_color = match self.state {
            AppState::Idle => COLOR_ACCENT,
            AppState::Thinking => COLOR_THINKING,
            AppState::Streaming => COLOR_ACCENT,
            AppState::Approval => COLOR_ERROR,
            AppState::Agents => COLOR_BANNER,
            AppState::Config => COLOR_DIM,
            AppState::Error => COLOR_ERROR,
        };

        // Right: elapsed time
        let right_text = if let Some(elapsed) = self.last_elapsed {
            format!("{:.1}s ", elapsed.as_secs_f64())
        } else if let Some(start) = self.response_start {
            format!("{:.1}s ", start.elapsed().as_secs_f64())
        } else {
            String::new()
        };

        let mut header_spans = vec![
            Span::styled(format!(" {spinner} "), Style::default().fg(state_color)),
            Span::styled(format!(" {}", self.status), Style::default().fg(COLOR_BAR_FG)),
        ];

        let mut right_spans = vec![];
        if self.rlm_iterating && self.state == AppState::Thinking {
            let anim = match (self.thinking_tick / 3) % 4 {
                0 => "⠋", 1 => "⠙", 2 => "⠸", _ => "⠴",
            };
            right_spans.push(Span::styled(format!("RLM {}  ", anim), Style::default().fg(COLOR_THINKING)));
        }
        if !right_text.is_empty() {
            right_spans.push(Span::styled(right_text, Style::default().fg(COLOR_DIM)));
        }

        let left_width: usize = header_spans.iter().map(|s| s.width()).sum();
        let right_width: usize = right_spans.iter().map(|s| s.width()).sum();
        
        let pad = (area.width as usize).saturating_sub(left_width + right_width);
        header_spans.push(Span::raw(" ".repeat(pad)));
        header_spans.extend(right_spans);

        let header_line = Line::from(header_spans);

        let bar = Paragraph::new(header_line)
            .style(Style::default().bg(COLOR_HEADER_BG));
        frame.render_widget(bar, area);
    }

    fn render_messages(&mut self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();

        for msg in &self.chat_lines {
            match msg.style {
                ChatStyle::Welcome => {
                    for line in msg.content.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("  {line}"),
                            Style::default().fg(COLOR_BANNER),
                        )));
                    }
                    lines.push(Line::from(""));
                }
                _ => {
                    let (role_color, content_style) = match msg.style {
                        ChatStyle::User => (COLOR_USER, Style::default()),
                        ChatStyle::Assistant => (COLOR_ASSISTANT, Style::default()),
                        ChatStyle::System => (COLOR_SYSTEM, Style::default().fg(COLOR_SYSTEM)),
                        ChatStyle::Tool => (COLOR_TOOL, Style::default().fg(COLOR_TOOL)),
                        ChatStyle::Error => (COLOR_ERROR, Style::default().fg(COLOR_ERROR)),
                        ChatStyle::Thinking => (
                            COLOR_THINKING,
                            Style::default().fg(COLOR_THINKING).italic(),
                        ),
                        ChatStyle::Welcome => unreachable!(),
                    };

                    // Highlight background for message box
                    let msg_bg = Color::Rgb(29, 32, 47);
                    let line_style = Style::default().bg(msg_bg);

                    // Left vertical bar to identify speaker
                    // Note: No more string roles like "you" or "rot".
                    let bar_span = Span::styled("▌ ", Style::default().fg(role_color).bg(msg_bg));

                    let content_lines: Vec<&str> = msg.content.lines().collect();
                    if content_lines.is_empty() {
                        lines.push(
                            Line::from(vec![
                                Span::styled(" ", Style::default().bg(msg_bg)),
                                bar_span,
                            ])
                            .style(line_style),
                        );
                    } else {
                        for content_line in content_lines {
                            let mut spans = vec![
                                Span::styled(" ", Style::default().bg(msg_bg)),
                                bar_span.clone(),
                            ];
                            spans.extend(Self::parse_markdown(content_line, content_style));
                            lines.push(Line::from(spans).style(line_style));
                        }
                    }
                    lines.push(Line::from(""));
                }
            }
        }

        // Streaming text
        if !self.streaming_text.is_empty() {
            let msg_bg = Color::Rgb(29, 32, 47);
            let line_style = Style::default().bg(msg_bg);
            let bar_span = Span::styled("▌ ", Style::default().fg(COLOR_ASSISTANT).bg(msg_bg));

            let stream_lines: Vec<&str> = self.streaming_text.lines().collect();
            for line in stream_lines {
                let mut spans = vec![
                    Span::styled(" ", Style::default().bg(msg_bg)),
                    bar_span.clone(),
                ];
                spans.extend(Self::parse_markdown(line, Style::default()));
                lines.push(Line::from(spans).style(line_style));
            }
        }

        // Thinking indicator
        if self.state == AppState::Thinking {
            let spinner = match (self.thinking_tick / 3) % 4 {
                0 => "⠋", 1 => "⠙", 2 => "⠸", _ => "⠴",
            };
            let elapsed_str = self.response_start
                .map(|s| format!(" {:.1}s", s.elapsed().as_secs_f64()))
                .unwrap_or_default();

            lines.push(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("{spinner} thinking{elapsed_str}"),
                    Style::default().fg(COLOR_THINKING).italic(),
                ),
            ]));
        }

        // No heavy box border — just a subtle bottom border for separation
        let block = Block::default()
            .borders(Borders::NONE);

        // Auto-scroll + clamp
        let inner_height = area.height;
        let content_height = lines.len() as u16;
        self.max_scroll = content_height.saturating_sub(inner_height);

        if self.auto_scroll && content_height > inner_height {
            self.scroll_offset = self.max_scroll;
        }
        // Clamp scroll to valid range
        self.scroll_offset = self.scroll_offset.min(self.max_scroll);

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));

        frame.render_widget(paragraph, area);
    }

    // render_context_bar removed — merged into render_footer

    fn render_input(&self, frame: &mut Frame, area: Rect) {
        let border_color = match self.state {
            AppState::Idle => match self.input_mode {
                InputMode::Insert => COLOR_ACCENT,
                InputMode::Normal => COLOR_BORDER,
            },
            AppState::Thinking | AppState::Streaming => COLOR_BORDER,
            AppState::Approval | AppState::Error => COLOR_ERROR,
            AppState::Agents => COLOR_BANNER,
            AppState::Config => COLOR_BORDER,
        };

        let prompt = match self.input_mode {
            InputMode::Insert => "› ",
            InputMode::Normal => "  ",
        };

        let display_input = self.input.replace('\n', " ↵ ");
        let input_text = format!("{prompt}{display_input}");

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .border_type(ratatui::widgets::BorderType::Thick);

        let style = match self.state {
            AppState::Idle => Style::default().fg(COLOR_CODE_FG),
            AppState::Thinking | AppState::Streaming => Style::default().fg(COLOR_DIM),
            AppState::Approval | AppState::Error => Style::default().fg(COLOR_ERROR),
            AppState::Agents => Style::default().fg(COLOR_DIM),
            AppState::Config => Style::default().fg(COLOR_DIM),
        };

        let paragraph = Paragraph::new(input_text).style(style).block(block);
        frame.render_widget(paragraph, area);

        // Cursor
        if self.input_mode == InputMode::Insert && self.state == AppState::Idle {
            let visible_pos: usize = self.input[..self.cursor_pos]
                .chars()
                .map(|c| if c == '\n' { 3 } else { 1 }) // ↵  = " ↵ " = 3 chars
                .sum();
            let x = area.x + visible_pos as u16 + 3; // +1 border +2 prompt
            let y = area.y + 1;
            frame.set_cursor_position(Position::new(x, y));
        }
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let mode_str = match self.input_mode {
            InputMode::Insert => "INSERT",
            InputMode::Normal => "NORMAL",
        };

        let mode_color = match self.input_mode {
            InputMode::Insert => COLOR_ACCENT,
            InputMode::Normal => COLOR_DIM,
        };

        // Calculate context/token/cost stats
        let total_tokens = self.total_input_tokens + self.total_output_tokens;
        let context_window = get_context_window(&self.model);
        let context_pct = if context_window > 0 {
            (total_tokens as f64 / context_window as f64 * 100.0).min(100.0)
        } else {
            0.0
        };
        let cost = if self.provider == "anthropic" {
            (self.total_input_tokens as f64 * 3.0 / 1_000_000.0)
                + (self.total_output_tokens as f64 * 15.0 / 1_000_000.0)
        } else {
            (self.total_input_tokens as f64 * 0.5 / 1_000_000.0)
                + (self.total_output_tokens as f64 * 0.5 / 1_000_000.0)
        };

        let pct_color = if context_pct > 80.0 {
            COLOR_ERROR
        } else if context_pct > 50.0 {
            COLOR_SYSTEM
        } else {
            COLOR_DIM
        };

        // Left side: MODE │ provider:model │ agent │ context% │ tokens │ cost
        let mut left = vec![
            Span::styled(format!(" {mode_str} "), Style::default().fg(Color::Black).bg(mode_color).bold()),
            Span::styled(
                format!("  {}:{}", self.provider, self.model),
                Style::default().fg(COLOR_BAR_FG),
            ),
            Span::styled("  │  ", Style::default().fg(COLOR_BORDER)),
            Span::styled(
                format!("@{}", self.agent),
                Style::default().fg(COLOR_ACCENT),
            ),
            Span::styled("  │  ", Style::default().fg(COLOR_BORDER)),
            Span::styled(
                format!("{context_pct:.0}%"),
                Style::default().fg(pct_color),
            ),
            Span::styled("  │  ", Style::default().fg(COLOR_BORDER)),
            Span::styled(
                format!("{}tok", Self::format_number(total_tokens)),
                Style::default().fg(COLOR_DIM),
            ),
        ];

        if cost > 0.0001 {
            left.push(Span::styled("  │  ", Style::default().fg(COLOR_BORDER)));
            left.push(Span::styled(
                format!("${cost:.4}"),
                Style::default().fg(COLOR_DIM),
            ));
        }

        let right_text = "/help ";
        let used: usize = left.iter().map(|s| s.width()).sum::<usize>() + right_text.len();
        let pad = (area.width as usize).saturating_sub(used);

        left.push(Span::raw(" ".repeat(pad)));
        left.push(Span::styled(right_text, Style::default().fg(COLOR_DIM)));

        let bar = Paragraph::new(Line::from(left))
            .style(Style::default().bg(COLOR_BAR_BG));
        frame.render_widget(bar, area);
    }

    fn render_slash_menu(&self, frame: &mut Frame, input_area: Rect) {
        let commands = self.filtered_slash_commands();
        if commands.is_empty() {
            return;
        }

        let selected = self
            .slash_menu_selected
            .min(commands.len().saturating_sub(1));
        let visible_count = commands.len().min(8);
        let start = if commands.len() <= visible_count {
            0
        } else {
            selected.saturating_sub(visible_count - 1).min(commands.len() - visible_count)
        };
        let end = start + visible_count;
        let visible = &commands[start..end];

        let max_content_width = visible
            .iter()
            .map(|(name, desc)| name.len() + 2 + desc.len())
            .max()
            .unwrap_or(20) as u16;
        let available_width = input_area.width.saturating_sub(2);
        if available_width == 0 {
            return;
        }
        let preferred_width = (max_content_width + 4).max(26);
        let width = preferred_width.min(available_width);
        let height = visible_count as u16 + 2;

        let menu_area = Rect {
            x: input_area.x + 1,
            y: input_area.y.saturating_sub(height),
            width,
            height,
        };

        frame.render_widget(Clear, menu_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_ACCENT))
            .style(Style::default().bg(COLOR_CODE_BG));
        let inner = block.inner(menu_area);
        frame.render_widget(block, menu_area);

        let max_name_len = visible
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0);

        let mut lines: Vec<Line> = Vec::with_capacity(visible.len());
        for (idx, (name, desc)) in visible.iter().enumerate() {
            let is_selected = (start + idx) == selected;
            let style = if is_selected {
                Style::default().fg(Color::Black).bg(COLOR_ACCENT)
            } else {
                Style::default().fg(COLOR_CODE_FG).bg(COLOR_CODE_BG)
            };
            let desc_style = if is_selected {
                Style::default().fg(Color::Black).bg(COLOR_ACCENT)
            } else {
                Style::default().fg(COLOR_DIM).bg(COLOR_CODE_BG)
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{name:<width$}", width = max_name_len + 1),
                    style,
                ),
                Span::styled(desc.to_string(), desc_style),
            ]));
        }

        let list = Paragraph::new(lines).style(Style::default().bg(COLOR_CODE_BG));
        frame.render_widget(list, inner);
    }

    fn render_approval_dialog(&self, frame: &mut Frame, area: Rect) {
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::widgets::Clear;

        // Create a centered rect for the dialog
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Min(10), // height of dialog
                Constraint::Percentage(30),
            ])
            .split(area);

        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Min(60), // width of dialog
                Constraint::Percentage(20),
            ])
            .split(vertical[1]);

        let dialog_area = horizontal[1];
        
        // Clear background behind dialog
        frame.render_widget(Clear, dialog_area);

        let tool_name = self.pending_approval_tool.as_deref().unwrap_or("unknown");
        let args_str = self
            .pending_approval_args
            .as_ref()
            .map(|a| serde_json::to_string_pretty(a).unwrap_or_default())
            .unwrap_or_default();

        let block = Block::default()
            .title(format!(" ⚠ Permission Request: {} ", tool_name))
            .title_style(Style::default().fg(COLOR_ERROR).bold())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_ERROR))
            .border_type(ratatui::widgets::BorderType::Thick);

        let mut lines = vec![
            Line::from(Span::styled("rot wants to execute the following tool:", Style::default().fg(COLOR_CODE_FG))),
            Line::from(""),
        ];

        for arg_line in args_str.lines() {
            lines.push(Line::from(Span::styled(format!("  {}", arg_line), Style::default().fg(COLOR_DIM))));
        }

        lines.push(Line::from(""));
        let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, dialog_area);

        let yes_btn = Span::styled(" [y]es ", Style::default().fg(COLOR_ASSISTANT).bold());
        let always_btn = Span::styled(" [a]lways ", Style::default().fg(COLOR_SYSTEM).bold());
        let no_btn = Span::styled(" [n]o ", Style::default().fg(COLOR_ERROR).bold());
        let deny_btn = Span::styled(" [d]eny ", Style::default().fg(COLOR_DIM).bold());

        let footer = Line::from(vec![
            Span::raw(" Allow execution? "),
            yes_btn, always_btn, no_btn, deny_btn,
        ]);
        let footer_p = Paragraph::new(footer).alignment(Alignment::Center);
        frame.render_widget(footer_p, dialog_area); // Using dialog_area here instead of chunks[1] 
    }

    fn render_config_dialog(&self, frame: &mut Frame, area: Rect) {
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::widgets::Clear;
        use ratatui::widgets::{Block, Borders, Paragraph};
        use ratatui::layout::Alignment;
        let (desired_width, desired_height) = match self.config_ui_state {
            ConfigUiState::List(_) => (50, 15),
            ConfigUiState::InputKey { .. } => (area.width.saturating_sub(6).clamp(40, 96), 12),
        };

        let popup_area = ratatui::layout::Rect {
            x: area.x + (area.width.saturating_sub(desired_width.min(area.width))) / 2,
            y: area.y + (area.height.saturating_sub(desired_height.min(area.height))) / 2,
            width: desired_width.min(area.width),
            height: desired_height.min(area.height),
        };

        frame.render_widget(Clear, popup_area);

        let block = match self.config_ui_state {
            ConfigUiState::List(_) => Block::default()
                .title(" Select Model & Config (/models) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_BANNER)),
            ConfigUiState::InputKey { .. } => Block::default()
                .title(" API key ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_BANNER)),
        };
        
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        match &self.config_ui_state {
            ConfigUiState::List(selected_idx) => {
                let items: Vec<ratatui::widgets::ListItem> = AVAILABLE_MODELS
                    .iter()
                    .enumerate()
                    .map(|(i, (p, m))| {
                        let mut style = Style::default().fg(COLOR_CODE_FG);
                        let prefix = if i == *selected_idx {
                            style = style.fg(COLOR_ACCENT).bold();
                            " ▶ "
                        } else {
                            "   "
                        };
                        ratatui::widgets::ListItem::new(format!("{prefix}{} / {}", p, m)).style(style)
                    })
                    .collect();

                let list = ratatui::widgets::List::new(items);
                frame.render_widget(list, inner);
            }
            ConfigUiState::InputKey { provider, input, cursor_pos, .. } => {
                let esc_hint = Paragraph::new("esc")
                    .style(Style::default().fg(COLOR_DIM))
                    .alignment(Alignment::Right);
                let esc_hint_area = Rect {
                    x: popup_area.x + 1,
                    y: popup_area.y,
                    width: popup_area.width.saturating_sub(2),
                    height: 1,
                };
                frame.render_widget(esc_hint, esc_hint_area);

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(1), // provider
                        Constraint::Length(1), // input
                        Constraint::Min(1),    // padding
                        Constraint::Length(1), // footer
                    ])
                    .split(inner);

                let provider_label = Paragraph::new(format!("{provider} provider"))
                    .style(Style::default().fg(COLOR_DIM))
                    .alignment(Alignment::Left);
                frame.render_widget(provider_label, chunks[0]);

                let (input_text, input_style) = if input.is_empty() {
                    (
                        "API key".to_string(),
                        Style::default().fg(COLOR_DIM),
                    )
                } else {
                    (
                        input.clone(),
                        Style::default().fg(COLOR_CODE_FG),
                    )
                };
                let input_widget = Paragraph::new(input_text).style(input_style);
                
                frame.render_widget(input_widget, chunks[1]);

                let footer = Paragraph::new(Line::from(vec![
                    Span::styled("enter", Style::default().fg(COLOR_CODE_FG).bold()),
                    Span::raw(" "),
                    Span::styled("submit", Style::default().fg(COLOR_DIM)),
                ]))
                .alignment(Alignment::Left);
                frame.render_widget(footer, chunks[3]);

                let cursor_col = (*cursor_pos).min(chunks[1].width.saturating_sub(1) as usize) as u16;
                frame.set_cursor_position(ratatui::layout::Position::new(
                    chunks[1].x + cursor_col,
                    chunks[1].y,
                ));
            }
        }
    }

    fn render_agents_dialog(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Clear, List, ListItem};

        let agents = self.all_agents();
        if agents.is_empty() {
            return;
        }

        let popup_area = Rect {
            x: area.x + (area.width.saturating_sub(62)) / 2,
            y: area.y + (area.height.saturating_sub(12)) / 2,
            width: 62.min(area.width),
            height: 12.min(area.height),
        };

        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Select Agent (/agents) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_BANNER));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let items: Vec<ListItem> = agents
            .iter()
            .enumerate()
            .map(|(idx, profile)| {
                let selected = idx == self.agent_menu_selected;
                let mode = if profile.is_subagent() { "subagent" } else { "primary" };
                let prefix = if selected { " ▶ " } else { "   " };
                let style = if selected {
                    Style::default().fg(COLOR_ACCENT).bold()
                } else {
                    Style::default().fg(COLOR_CODE_FG)
                };
                ListItem::new(format!(
                    "{prefix}{:<10} {:<9} {}",
                    profile.name, mode, profile.description
                ))
                .style(style)
            })
            .collect();

        frame.render_widget(List::new(items), inner);
    }

    // ── Markdown Parser ────────────────────────────────────────────────

    fn parse_markdown<'a>(text: &'a str, base: Style) -> Vec<Span<'a>> {
        let mut spans = Vec::new();
        let mut chars = text.char_indices().peekable();
        let mut plain_start = 0;

        while let Some(&(i, c)) = chars.peek() {
            match c {
                '*' => {
                    let rest = &text[i..];
                    if let Some(after_stars) = rest.strip_prefix("**") {
                        if let Some(end) = after_stars.find("**") {
                            if i > plain_start {
                                spans.push(Span::styled(&text[plain_start..i], base));
                            }
                            let bold_text = &text[i + 2..i + 2 + end];
                            spans.push(Span::styled(bold_text, base.bold()));
                            let skip_to = i + 2 + end + 2;
                            while let Some(&(j, _)) = chars.peek() {
                                if j >= skip_to { break; }
                                chars.next();
                            }
                            plain_start = skip_to;
                            continue;
                        }
                    }
                    chars.next();
                }
                '`' => {
                    let rest = &text[i..];
                    if rest.starts_with("```") {
                        chars.next();
                        continue;
                    }
                    if let Some(end) = rest[1..].find('`') {
                        if i > plain_start {
                            spans.push(Span::styled(&text[plain_start..i], base));
                        }
                        let code_text = &text[i + 1..i + 1 + end];
                        spans.push(Span::styled(
                            code_text,
                            Style::default().fg(COLOR_CODE_FG).bg(COLOR_CODE_BG),
                        ));
                        let skip_to = i + 1 + end + 1;
                        while let Some(&(j, _)) = chars.peek() {
                            if j >= skip_to { break; }
                            chars.next();
                        }
                        plain_start = skip_to;
                        continue;
                    }
                    chars.next();
                }
                _ => { chars.next(); }
            }
        }

        if plain_start < text.len() {
            spans.push(Span::styled(&text[plain_start..], base));
        }
        if spans.is_empty() {
            spans.push(Span::styled("", base));
        }
        spans
    }

    fn format_number(n: usize) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}k", n as f64 / 1_000.0)
        } else {
            n.to_string()
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = App::new("claude-sonnet-4-20250514", "anthropic", "default");
        assert_eq!(app.state, AppState::Idle);
        assert!(app.running);
        assert!(app.input.is_empty());
        assert!(!app.showed_welcome);
    }

    #[test]
    fn test_input_editing() {
        let mut app = App::new("test", "test", "default");
        app.insert_char('h');
        app.insert_char('i');
        assert_eq!(app.input, "hi");
        assert_eq!(app.cursor_pos, 2);
        app.backspace();
        assert_eq!(app.input, "h");
        assert_eq!(app.cursor_pos, 1);
    }

    #[test]
    fn test_submit_input() {
        let mut app = App::new("test", "test", "default");
        app.insert_char('h');
        app.insert_char('i');
        let text = app.submit_input();
        assert_eq!(text, "hi");
        assert!(app.input.is_empty());
    }

    #[test]
    fn test_push_chat() {
        let mut app = App::new("test", "test", "default");
        app.push_chat("user", "Hello!", ChatStyle::User);
        assert_eq!(app.chat_lines.len(), 1);
        assert!(app.auto_scroll);
    }

    #[test]
    fn test_auto_scroll_on_push() {
        let mut app = App::new("test", "test", "default");
        app.auto_scroll = false;
        app.push_chat("user", "Hello!", ChatStyle::User);
        assert!(app.auto_scroll);
    }

    #[test]
    fn test_slash_help() {
        let mut app = App::new("test", "test", "default");
        assert!(app.handle_slash_command("/help"));
        assert_eq!(app.chat_lines.len(), 1);
    }

    #[test]
    fn test_slash_clear() {
        let mut app = App::new("test", "test", "default");
        app.push_chat("user", "test", ChatStyle::User);
        assert!(app.handle_slash_command("/clear"));
        assert_eq!(app.chat_lines.len(), 1);
    }

    #[test]
    fn test_slash_unknown() {
        let mut app = App::new("test", "test", "default");
        assert!(app.handle_slash_command("/foo"));
        assert!(app.chat_lines[0].content.contains("Unknown"));
    }

    #[test]
    fn test_non_slash() {
        let mut app = App::new("test", "test", "default");
        assert!(!app.handle_slash_command("hello"));
    }

    #[test]
    fn test_multi_line() {
        let mut app = App::new("test", "test", "default");
        app.insert_char('a');
        app.insert_newline();
        app.insert_char('b');
        assert_eq!(app.input, "a\nb");
    }

    #[test]
    fn test_markdown_bold() {
        let spans = App::parse_markdown("hi **world** end", Style::default());
        assert_eq!(spans.len(), 3);
    }

    #[test]
    fn test_markdown_code() {
        let spans = App::parse_markdown("use `foo` here", Style::default());
        assert_eq!(spans.len(), 3);
    }

    #[test]
    fn test_tokens() {
        let mut app = App::new("test", "test", "default");
        app.record_tokens(100, 50);
        assert_eq!(app.total_input_tokens, 100);
        app.record_tokens(200, 100);
        assert_eq!(app.total_output_tokens, 150);
    }

    #[test]
    fn test_format_number() {
        assert_eq!(App::format_number(500), "500");
        assert_eq!(App::format_number(1500), "1.5k");
        assert_eq!(App::format_number(1_500_000), "1.5M");
    }

    #[test]
    fn test_welcome_once() {
        let mut app = App::new("test", "test", "default");
        app.show_welcome();
        let n = app.chat_lines.len();
        app.show_welcome();
        assert_eq!(app.chat_lines.len(), n);
    }

    #[test]
    fn test_context_window() {
        assert_eq!(get_context_window("claude-sonnet-4-20250514"), 200_000);
        assert_eq!(get_context_window("glm-5"), 128_000);
        assert_eq!(get_context_window("unknown"), 128_000);
    }

    #[test]
    fn test_slash_menu_active_and_filtered() {
        let mut app = App::new("test", "test", "default");
        app.input = "/m".to_string();
        app.cursor_pos = app.input.len();
        let items = app.filtered_slash_commands();
        assert!(app.is_slash_menu_active());
        assert!(items.iter().any(|(cmd, _)| *cmd == "/models"));
        assert!(items.iter().any(|(cmd, _)| *cmd == "/model"));
    }

    #[test]
    fn test_slash_selection_wraps() {
        let mut app = App::new("test", "test", "default");
        app.input = "/".to_string();
        app.cursor_pos = 1;
        app.sync_slash_menu_selection();
        app.move_slash_selection_up();
        assert_eq!(
            app.slash_menu_selected,
            app.filtered_slash_commands().len() - 1
        );
        app.move_slash_selection_down();
        assert_eq!(app.slash_menu_selected, 0);
    }

    #[test]
    fn test_slash_agents_opens_dialog() {
        let mut app = App::new("test", "test", "default");
        assert!(app.handle_slash_command("/agents"));
        assert_eq!(app.state, AppState::Agents);
    }

    #[test]
    fn test_slash_tree_is_reserved_for_runner_inspection() {
        let mut app = App::new("test", "test", "default");
        assert!(!app.handle_slash_command("/tree"));
    }

    #[test]
    fn test_slash_tools_is_reserved_for_runner_inspection() {
        let mut app = App::new("test", "test", "default");
        assert!(!app.handle_slash_command("/tools"));
        assert!(!app.handle_slash_command("/tool read"));
    }

    #[test]
    fn test_select_current_agent_updates_active_agent() {
        let mut app = App::new("test", "test", "default");
        app.state = AppState::Agents;
        app.agent_menu_selected = app
            .all_agents()
            .iter()
            .position(|profile| profile.name == "plan")
            .unwrap();

        assert!(app.select_current_agent());
        assert_eq!(app.agent, "plan");
        assert_eq!(app.state, AppState::Idle);
    }

    #[test]
    fn test_parse_agent_mention_extracts_prompt() {
        let parsed = App::parse_agent_mention("@review inspect this diff").unwrap();
        assert_eq!(parsed.0, "review");
        assert_eq!(parsed.1, "inspect this diff");
    }
}
