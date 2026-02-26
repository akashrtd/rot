//! TUI application state and rendering.
//!
//! Features:
//! - Welcome banner on startup
//! - Markdown-aware rendering (bold, code, code blocks)
//! - Header bar with model/provider/session info
//! - Enhanced status bar with token count and elapsed time
//! - Thinking animation with spinner
//! - Polished color palette
//! - Multi-line input (newlines in input buffer)
//! - Auto-scroll with manual override

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::time::{Duration, Instant};

// -- Catppuccin-inspired color palette --
const COLOR_USER: Color = Color::Rgb(93, 228, 199);      // Teal
const COLOR_ASSISTANT: Color = Color::Rgb(166, 227, 161); // Green
const COLOR_TOOL: Color = Color::Rgb(203, 166, 247);      // Mauve/Purple
const COLOR_ERROR: Color = Color::Rgb(243, 139, 168);     // Red/Pink
const COLOR_SYSTEM: Color = Color::Rgb(250, 179, 135);    // Peach/Orange
const COLOR_THINKING: Color = Color::Rgb(108, 112, 134);  // Overlay0 (gray)
const COLOR_CODE_BG: Color = Color::Rgb(30, 30, 46);      // Base dark
const COLOR_CODE_FG: Color = Color::Rgb(205, 214, 244);   // Text light
const COLOR_HEADER_BG: Color = Color::Rgb(30, 30, 46);    // Base dark
const COLOR_HEADER_FG: Color = Color::Rgb(166, 227, 161); // Green accent
const COLOR_STATUS_BG: Color = Color::Rgb(24, 24, 37);    // Mantle
const COLOR_STATUS_FG: Color = Color::Rgb(186, 194, 222); // Subtext1
const COLOR_BORDER: Color = Color::Rgb(69, 71, 90);       // Surface1
const COLOR_DIM: Color = Color::Rgb(88, 91, 112);         // Overlay0

/// Current application state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Idle,
    Thinking,
    Streaming,
    Error,
}

/// Input mode for the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
}

/// Main application struct.
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
    pub thinking_tick: u16,
    /// Token usage tracking.
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub last_input_tokens: usize,
    pub last_output_tokens: usize,
    /// Elapsed time for last response.
    pub response_start: Option<Instant>,
    pub last_elapsed: Option<Duration>,
    /// Message counter.
    pub message_count: usize,
    /// Whether welcome banner has been shown.
    pub showed_welcome: bool,
}

/// A line in the chat display.
#[derive(Debug, Clone)]
pub struct ChatLine {
    pub role: String,
    pub content: String,
    pub style: ChatStyle,
}

/// Styling for chat lines.
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

impl App {
    pub fn new(model: &str, provider: &str) -> Self {
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
            thinking_tick: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            last_input_tokens: 0,
            last_output_tokens: 0,
            response_start: None,
            last_elapsed: None,
            message_count: 0,
            showed_welcome: false,
        }
    }

    /// Show the welcome banner.
    pub fn show_welcome(&mut self) {
        if self.showed_welcome {
            return;
        }
        self.showed_welcome = true;

        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());
        // Shorten the path to last 2 components
        let short_cwd = cwd
            .rsplit('/')
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("/");

        let welcome = format!(
            "Welcome to rot — AI coding agent\n\
             ─────────────────────────────────\n\
             Provider : {} / {}\n\
             Directory: {}\n\
             ─────────────────────────────────\n\
             Type a message to start. Commands:\n\
             /help  — show available commands\n\
             /clear — clear conversation\n\
             /model — switch model\n\
             Ctrl+C — quit",
            self.provider, self.model, short_cwd,
        );
        self.push_chat("", &welcome, ChatStyle::Welcome);
    }

    /// Add a chat line and auto-scroll to bottom.
    pub fn push_chat(&mut self, role: &str, content: &str, style: ChatStyle) {
        self.chat_lines.push(ChatLine {
            role: role.to_string(),
            content: content.to_string(),
            style,
        });
        self.auto_scroll = true;
    }

    /// Start timing a response.
    pub fn start_timer(&mut self) {
        self.response_start = Some(Instant::now());
    }

    /// Stop timing and record elapsed.
    pub fn stop_timer(&mut self) {
        if let Some(start) = self.response_start.take() {
            self.last_elapsed = Some(start.elapsed());
        }
    }

    /// Record token usage.
    pub fn record_tokens(&mut self, input: usize, output: usize) {
        self.last_input_tokens = input;
        self.last_output_tokens = output;
        self.total_input_tokens += input;
        self.total_output_tokens += output;
    }

    /// Handle a slash command. Returns true if handled.
    pub fn handle_slash_command(&mut self, cmd: &str) -> bool {
        let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
        match parts[0] {
            "/help" => {
                self.push_chat("system", 
                    "Available commands:\n\
                     /help       — show this message\n\
                     /clear      — clear conversation history\n\
                     /model      — show current model\n\
                     /model NAME — switch to a different model\n\
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
            "/model" => {
                if parts.len() > 1 {
                    self.push_chat("system", &format!("Model switching to: {} (restart required for full effect)", parts[1]), ChatStyle::System);
                } else {
                    self.push_chat("system", &format!("Current model: {} ({})", self.model, self.provider), ChatStyle::System);
                }
                true
            }
            "/quit" | "/exit" => {
                self.running = false;
                true
            }
            _ if cmd.starts_with('/') => {
                self.push_chat("system", &format!("Unknown command: {}. Type /help for available commands.", parts[0]), ChatStyle::System);
                true
            }
            _ => false,
        }
    }

    pub fn submit_input(&mut self) -> String {
        let text = self.input.clone();
        self.input.clear();
        self.cursor_pos = 0;
        text
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn insert_newline(&mut self) {
        self.input.insert(self.cursor_pos, '\n');
        self.cursor_pos += 1;
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
        }
    }

    pub fn tick(&mut self) {
        self.thinking_tick = self.thinking_tick.wrapping_add(1);
    }

    // -- Rendering --

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header bar
                Constraint::Min(3),   // Messages
                Constraint::Length(3), // Input
                Constraint::Length(1), // Status bar
            ])
            .split(area);

        self.render_header(frame, chunks[0]);
        self.render_messages(frame, chunks[1]);
        self.render_input(frame, chunks[2]);
        self.render_status(frame, chunks[3]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let left = format!(" rot • {}/{}", self.provider, self.model);
        let right = " /help for commands ";

        let available = area.width as usize;
        let padding = available.saturating_sub(left.len() + right.len());

        let header_text = format!("{left}{}{right}", " ".repeat(padding));

        let bar = Paragraph::new(header_text)
            .style(Style::default().bg(COLOR_HEADER_BG).fg(COLOR_HEADER_FG));
        frame.render_widget(bar, area);
    }

    fn render_messages(&mut self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();

        for msg in &self.chat_lines {
            match msg.style {
                ChatStyle::Welcome => {
                    // Welcome banner — render each line with dim style
                    for line in msg.content.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("  {line}"),
                            Style::default().fg(COLOR_DIM),
                        )));
                    }
                    lines.push(Line::from(""));
                }
                _ => {
                    let role_style = match msg.style {
                        ChatStyle::User => Style::default().fg(COLOR_USER).bold(),
                        ChatStyle::Assistant => Style::default().fg(COLOR_ASSISTANT).bold(),
                        ChatStyle::System => Style::default().fg(COLOR_SYSTEM).bold(),
                        ChatStyle::Tool => Style::default().fg(COLOR_TOOL).bold(),
                        ChatStyle::Error => Style::default().fg(COLOR_ERROR).bold(),
                        ChatStyle::Thinking => Style::default().fg(COLOR_THINKING).italic(),
                        ChatStyle::Welcome => unreachable!(),
                    };

                    let role_prefix = if msg.role.is_empty() {
                        String::new()
                    } else {
                        format!("{}: ", msg.role)
                    };
                    let indent = if msg.role.is_empty() {
                        String::new()
                    } else {
                        " ".repeat(msg.role.len() + 2)
                    };

                    let content_lines: Vec<&str> = msg.content.lines().collect();
                    if content_lines.is_empty() {
                        lines.push(Line::from(Span::styled(role_prefix, role_style)));
                    } else {
                        for (i, content_line) in content_lines.iter().enumerate() {
                            if i == 0 {
                                let mut spans = vec![Span::styled(role_prefix.clone(), role_style)];
                                spans.extend(Self::parse_markdown(content_line));
                                lines.push(Line::from(spans));
                            } else {
                                let mut spans = vec![Span::raw(indent.clone())];
                                spans.extend(Self::parse_markdown(content_line));
                                lines.push(Line::from(spans));
                            }
                        }
                    }
                    lines.push(Line::from(""));
                }
            }
        }

        // Show streaming text
        if !self.streaming_text.is_empty() {
            let stream_lines: Vec<&str> = self.streaming_text.lines().collect();
            for (i, line) in stream_lines.iter().enumerate() {
                if i == 0 {
                    let mut spans = vec![Span::styled(
                        "rot: ",
                        Style::default().fg(COLOR_ASSISTANT).bold(),
                    )];
                    spans.extend(Self::parse_markdown(line));
                    lines.push(Line::from(spans));
                } else {
                    let mut spans = vec![Span::raw("     ")];
                    spans.extend(Self::parse_markdown(line));
                    lines.push(Line::from(spans));
                }
            }
        }

        // Thinking indicator with spinner
        if self.state == AppState::Thinking {
            let spinner = match (self.thinking_tick / 3) % 4 {
                0 => "⠋",
                1 => "⠙",
                2 => "⠸",
                _ => "⠴",
            };
            let elapsed_str = self.response_start
                .map(|s| format!(" ({:.1}s)", s.elapsed().as_secs_f64()))
                .unwrap_or_default();

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{spinner} "),
                    Style::default().fg(COLOR_ASSISTANT),
                ),
                Span::styled(
                    format!("thinking{elapsed_str}"),
                    Style::default().fg(COLOR_THINKING).italic(),
                ),
            ]));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_BORDER))
            .border_type(ratatui::widgets::BorderType::Rounded);

        // Auto-scroll
        let inner_height = area.height.saturating_sub(2);
        let content_height = lines.len() as u16;

        if self.auto_scroll && content_height > inner_height {
            self.scroll_offset = content_height.saturating_sub(inner_height);
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset, 0));

        frame.render_widget(paragraph, area);
    }

    fn render_input(&self, frame: &mut Frame, area: Rect) {
        let style = match self.state {
            AppState::Idle => Style::default().fg(Color::White),
            AppState::Thinking | AppState::Streaming => Style::default().fg(COLOR_DIM),
            AppState::Error => Style::default().fg(COLOR_ERROR),
        };

        let prompt = match self.input_mode {
            InputMode::Insert => "› ",
            InputMode::Normal => "  ",
        };

        // Show line count if multi-line
        let line_count = self.input.lines().count();
        let line_hint = if line_count > 1 {
            format!(" [{line_count} lines]")
        } else {
            String::new()
        };

        let display_input = self.input.replace('\n', "↵ ");
        let input_text = format!("{prompt}{display_input}{line_hint}");

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(COLOR_BORDER))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let paragraph = Paragraph::new(input_text).style(style).block(block);
        frame.render_widget(paragraph, area);

        // Show cursor
        if self.input_mode == InputMode::Insert && self.state == AppState::Idle {
            // Count visible position (newlines become "↵ " = 2 chars)
            let visible_pos: usize = self.input[..self.cursor_pos]
                .chars()
                .map(|c| if c == '\n' { 2 } else { 1 })
                .sum();
            let x = area.x + visible_pos as u16 + 3; // +1 border +2 prompt
            let y = area.y + 1;
            frame.set_cursor_position(Position::new(x, y));
        }
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let spinner = match self.state {
            AppState::Idle => "●",
            AppState::Thinking => {
                match (self.thinking_tick / 3) % 4 {
                    0 => "⠋",
                    1 => "⠙",
                    2 => "⠸",
                    _ => "⠴",
                }
            }
            AppState::Streaming => "◉",
            AppState::Error => "✖",
        };

        let left = format!(" {spinner} {}", self.status);

        // Build right side info
        let mut right_parts: Vec<String> = Vec::new();

        // Token count
        let total_tokens = self.total_input_tokens + self.total_output_tokens;
        if total_tokens > 0 {
            right_parts.push(format!("{}tok", Self::format_number(total_tokens)));
        }

        // Elapsed time
        if let Some(elapsed) = self.last_elapsed {
            right_parts.push(format!("{:.1}s", elapsed.as_secs_f64()));
        } else if let Some(start) = self.response_start {
            right_parts.push(format!("{:.1}s", start.elapsed().as_secs_f64()));
        }

        // Message count
        if self.message_count > 0 {
            right_parts.push(format!("{}msgs", self.message_count));
        }

        right_parts.push("Ctrl+C: quit".to_string());

        let right = right_parts.join(" │ ");
        let available = area.width as usize;
        let padding = available.saturating_sub(left.len() + right.len() + 1);

        let status_text = format!("{left}{}{right} ", " ".repeat(padding));

        let bar = Paragraph::new(status_text)
            .style(Style::default().bg(COLOR_STATUS_BG).fg(COLOR_STATUS_FG));
        frame.render_widget(bar, area);
    }

    // -- Markdown Parser --

    /// Parse inline markdown: **bold**, `code`, and plain text.
    fn parse_markdown(text: &str) -> Vec<Span<'_>> {
        let mut spans = Vec::new();
        let mut chars = text.char_indices().peekable();
        let mut plain_start = 0;

        while let Some(&(i, c)) = chars.peek() {
            match c {
                '*' => {
                    // Check for **bold**
                    let rest = &text[i..];
                    if let Some(after_stars) = rest.strip_prefix("**") {
                        if let Some(end) = after_stars.find("**") {
                            // Push preceding plain text
                            if i > plain_start {
                                spans.push(Span::raw(&text[plain_start..i]));
                            }
                            let bold_text = &text[i + 2..i + 2 + end];
                            spans.push(Span::styled(bold_text, Style::default().bold()));
                            // Advance past **text**
                            let skip_to = i + 2 + end + 2;
                            while let Some(&(j, _)) = chars.peek() {
                                if j >= skip_to {
                                    break;
                                }
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
                    // Skip ``` code fences (rendered as-is for now)
                    if rest.starts_with("```") {
                        chars.next();
                        continue;
                    }
                    // Inline `code`
                    if let Some(end) = rest[1..].find('`') {
                        if i > plain_start {
                            spans.push(Span::raw(&text[plain_start..i]));
                        }
                        let code_text = &text[i + 1..i + 1 + end];
                        spans.push(Span::styled(
                            code_text,
                            Style::default().fg(COLOR_CODE_FG).bg(COLOR_CODE_BG),
                        ));
                        let skip_to = i + 1 + end + 1;
                        while let Some(&(j, _)) = chars.peek() {
                            if j >= skip_to {
                                break;
                            }
                            chars.next();
                        }
                        plain_start = skip_to;
                        continue;
                    }
                    chars.next();
                }
                _ => {
                    chars.next();
                }
            }
        }

        // Remaining plain text
        if plain_start < text.len() {
            spans.push(Span::raw(&text[plain_start..]));
        }

        if spans.is_empty() {
            spans.push(Span::raw(""));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = App::new("claude-sonnet-4-20250514", "anthropic");
        assert_eq!(app.state, AppState::Idle);
        assert!(app.running);
        assert!(app.input.is_empty());
        assert!(!app.showed_welcome);
    }

    #[test]
    fn test_input_editing() {
        let mut app = App::new("test", "test");
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
        let mut app = App::new("test", "test");
        app.insert_char('h');
        app.insert_char('i');

        let text = app.submit_input();
        assert_eq!(text, "hi");
        assert!(app.input.is_empty());
        assert_eq!(app.cursor_pos, 0);
    }

    #[test]
    fn test_push_chat() {
        let mut app = App::new("test", "test");
        app.push_chat("user", "Hello!", ChatStyle::User);
        assert_eq!(app.chat_lines.len(), 1);
        assert_eq!(app.chat_lines[0].content, "Hello!");
        assert!(app.auto_scroll);
    }

    #[test]
    fn test_auto_scroll_on_push() {
        let mut app = App::new("test", "test");
        app.auto_scroll = false;
        app.push_chat("user", "Hello!", ChatStyle::User);
        assert!(app.auto_scroll);
    }

    #[test]
    fn test_slash_help() {
        let mut app = App::new("test", "test");
        assert!(app.handle_slash_command("/help"));
        assert_eq!(app.chat_lines.len(), 1);
    }

    #[test]
    fn test_slash_clear() {
        let mut app = App::new("test", "test");
        app.push_chat("user", "test", ChatStyle::User);
        app.push_chat("rot", "reply", ChatStyle::Assistant);
        assert!(app.handle_slash_command("/clear"));
        assert_eq!(app.chat_lines.len(), 1); // "Conversation cleared"
    }

    #[test]
    fn test_slash_unknown() {
        let mut app = App::new("test", "test");
        assert!(app.handle_slash_command("/foo"));
        assert!(app.chat_lines[0].content.contains("Unknown command"));
    }

    #[test]
    fn test_non_slash_not_handled() {
        let mut app = App::new("test", "test");
        assert!(!app.handle_slash_command("hello"));
    }

    #[test]
    fn test_multi_line_input() {
        let mut app = App::new("test", "test");
        app.insert_char('h');
        app.insert_char('i');
        app.insert_newline();
        app.insert_char('!');
        assert_eq!(app.input, "hi\n!");
    }

    #[test]
    fn test_markdown_bold() {
        let spans = App::parse_markdown("hello **world** end");
        assert_eq!(spans.len(), 3);
    }

    #[test]
    fn test_markdown_code() {
        let spans = App::parse_markdown("use `foo` here");
        assert_eq!(spans.len(), 3);
    }

    #[test]
    fn test_token_tracking() {
        let mut app = App::new("test", "test");
        app.record_tokens(100, 50);
        assert_eq!(app.total_input_tokens, 100);
        assert_eq!(app.total_output_tokens, 50);
        app.record_tokens(200, 100);
        assert_eq!(app.total_input_tokens, 300);
        assert_eq!(app.total_output_tokens, 150);
    }

    #[test]
    fn test_format_number() {
        assert_eq!(App::format_number(500), "500");
        assert_eq!(App::format_number(1500), "1.5k");
        assert_eq!(App::format_number(1_500_000), "1.5M");
    }

    #[test]
    fn test_welcome_only_once() {
        let mut app = App::new("test", "test");
        app.show_welcome();
        let count = app.chat_lines.len();
        app.show_welcome();
        assert_eq!(app.chat_lines.len(), count, "Welcome should only show once");
    }
}
