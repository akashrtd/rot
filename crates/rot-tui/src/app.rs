//! TUI application state and rendering.
//!
//! Design principles (from ratatui best practices research):
//! - Visual hierarchy through weight (bold, dim, normal) and color
//! - Breathing room with spacing instead of heavy borders
//! - Centralized theme constants for consistency
//! - Clean minimal layout: header | messages | context bar | input | footer

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::time::{Duration, Instant};

// ── Theme ──────────────────────────────────────────────────────────────

const COLOR_USER: Color = Color::Rgb(137, 180, 250);     // Blue (Lavender)
const COLOR_ASSISTANT: Color = Color::Rgb(166, 227, 161); // Green
const COLOR_TOOL: Color = Color::Rgb(203, 166, 247);      // Mauve
const COLOR_ERROR: Color = Color::Rgb(243, 139, 168);     // Red/Maroon
const COLOR_SYSTEM: Color = Color::Rgb(250, 179, 135);    // Peach
const COLOR_THINKING: Color = Color::Rgb(108, 112, 134);  // Overlay0
const COLOR_CODE_BG: Color = Color::Rgb(30, 30, 46);      // Base
const COLOR_CODE_FG: Color = Color::Rgb(205, 214, 244);   // Text
const COLOR_HEADER_BG: Color = Color::Rgb(24, 24, 37);    // Mantle
const COLOR_ACCENT: Color = Color::Rgb(166, 227, 161);    // Green accent
const COLOR_BAR_BG: Color = Color::Rgb(24, 24, 37);       // Mantle
const COLOR_BAR_FG: Color = Color::Rgb(147, 153, 178);    // Overlay1
const COLOR_BORDER: Color = Color::Rgb(49, 50, 68);       // Surface0 (subtle)
const COLOR_DIM: Color = Color::Rgb(88, 91, 112);         // Overlay0
const COLOR_BANNER: Color = Color::Rgb(203, 166, 247);    // Mauve for banner

// ── ASCII Art ──────────────────────────────────────────────────────────

const ASCII_BANNER: &str = "
 ███████████      ███████    ███████████
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
    Error,
}

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
    pub thinking_tick: u16,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub last_input_tokens: usize,
    pub last_output_tokens: usize,
    pub response_start: Option<Instant>,
    pub last_elapsed: Option<Duration>,
    pub message_count: usize,
    pub showed_welcome: bool,
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
             ╭────────────────────────────────────╮\n\
             │  provider : {:<23}│\n\
             │  model    : {:<23}│\n\
             │  cwd      : {:<23}│\n\
             ╰────────────────────────────────────╯\n\
             \n\
             Type a message or use /help for commands.",
            ASCII_BANNER.trim(),
            self.provider,
            self.model,
            if short_cwd.len() > 23 {
                format!("…{}", &short_cwd[short_cwd.len().saturating_sub(22)..])
            } else {
                short_cwd
            },
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
                    "/help       — show this message\n\
                     /clear      — clear conversation\n\
                     /model      — show current model\n\
                     /model NAME — switch model\n\
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
                    self.push_chat(
                        "system",
                        &format!("Model → {} (takes effect next message)", parts[1]),
                        ChatStyle::System,
                    );
                } else {
                    self.push_chat(
                        "system",
                        &format!("{} / {}", self.provider, self.model),
                        ChatStyle::System,
                    );
                }
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
            AppState::Error => "✖",
        };

        let state_color = match self.state {
            AppState::Idle => COLOR_ACCENT,
            AppState::Thinking => COLOR_THINKING,
            AppState::Streaming => COLOR_ACCENT,
            AppState::Error => COLOR_ERROR,
        };

        let left = Line::from(vec![
            Span::styled(format!(" {spinner} "), Style::default().fg(state_color)),
            Span::styled("rot", Style::default().fg(COLOR_ACCENT).bold()),
            Span::styled(
                format!("  {}", self.status),
                Style::default().fg(COLOR_BAR_FG),
            ),
        ]);

        // Right: elapsed time
        let right_text = if let Some(elapsed) = self.last_elapsed {
            format!("{:.1}s ", elapsed.as_secs_f64())
        } else if let Some(start) = self.response_start {
            format!("{:.1}s ", start.elapsed().as_secs_f64())
        } else {
            String::new()
        };

        let used = left.width() + right_text.len();
        let pad = (area.width as usize).saturating_sub(used);

        let header_line = Line::from(vec![
            Span::styled(format!(" {spinner} "), Style::default().fg(state_color)),
            Span::styled("rot", Style::default().fg(COLOR_ACCENT).bold()),
            Span::styled(
                format!("  {}", self.status),
                Style::default().fg(COLOR_BAR_FG),
            ),
            Span::raw(" ".repeat(pad)),
            Span::styled(right_text, Style::default().fg(COLOR_DIM)),
        ]);

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
                    let (role_style, content_style) = match msg.style {
                        ChatStyle::User => (
                            Style::default().fg(COLOR_USER).bold(),
                            Style::default(),
                        ),
                        ChatStyle::Assistant => (
                            Style::default().fg(COLOR_ASSISTANT).bold(),
                            Style::default(),
                        ),
                        ChatStyle::System => (
                            Style::default().fg(COLOR_SYSTEM).bold(),
                            Style::default().fg(COLOR_SYSTEM),
                        ),
                        ChatStyle::Tool => (
                            Style::default().fg(COLOR_TOOL),
                            Style::default().fg(COLOR_TOOL),
                        ),
                        ChatStyle::Error => (
                            Style::default().fg(COLOR_ERROR).bold(),
                            Style::default().fg(COLOR_ERROR),
                        ),
                        ChatStyle::Thinking => (
                            Style::default().fg(COLOR_THINKING).italic(),
                            Style::default().fg(COLOR_THINKING).italic(),
                        ),
                        ChatStyle::Welcome => unreachable!(),
                    };

                    let role_prefix = if msg.role.is_empty() {
                        String::new()
                    } else {
                        format!("{}  ", msg.role)
                    };
                    let indent_len = if msg.role.is_empty() {
                        0
                    } else {
                        msg.role.len() + 2
                    };

                    let content_lines: Vec<&str> = msg.content.lines().collect();
                    if content_lines.is_empty() {
                        lines.push(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(role_prefix, role_style),
                        ]));
                    } else {
                        for (i, content_line) in content_lines.iter().enumerate() {
                            if i == 0 {
                                let mut spans = vec![
                                    Span::raw(" "),
                                    Span::styled(role_prefix.clone(), role_style),
                                ];
                                spans.extend(Self::parse_markdown(content_line, content_style));
                                lines.push(Line::from(spans));
                            } else {
                                let mut spans = vec![
                                    Span::raw(" "),
                                    Span::raw(" ".repeat(indent_len)),
                                ];
                                spans.extend(Self::parse_markdown(content_line, content_style));
                                lines.push(Line::from(spans));
                            }
                        }
                    }
                    lines.push(Line::from(""));
                }
            }
        }

        // Streaming text
        if !self.streaming_text.is_empty() {
            let stream_lines: Vec<&str> = self.streaming_text.lines().collect();
            for (i, line) in stream_lines.iter().enumerate() {
                if i == 0 {
                    let mut spans = vec![
                        Span::raw(" "),
                        Span::styled("rot  ", Style::default().fg(COLOR_ASSISTANT).bold()),
                    ];
                    spans.extend(Self::parse_markdown(line, Style::default()));
                    lines.push(Line::from(spans));
                } else {
                    let mut spans = vec![Span::raw("      ")];
                    spans.extend(Self::parse_markdown(line, Style::default()));
                    lines.push(Line::from(spans));
                }
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

        // Auto-scroll
        let inner_height = area.height;
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

    // render_context_bar removed — merged into render_footer

    fn render_input(&self, frame: &mut Frame, area: Rect) {
        let border_color = match self.state {
            AppState::Idle => match self.input_mode {
                InputMode::Insert => COLOR_ACCENT,
                InputMode::Normal => COLOR_BORDER,
            },
            AppState::Thinking | AppState::Streaming => COLOR_BORDER,
            AppState::Error => COLOR_ERROR,
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
            .border_type(ratatui::widgets::BorderType::Rounded);

        let style = match self.state {
            AppState::Idle => Style::default().fg(Color::White),
            AppState::Thinking | AppState::Streaming => Style::default().fg(COLOR_DIM),
            AppState::Error => Style::default().fg(COLOR_ERROR),
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

        // Left side: MODE │ provider:model │ context% │ tokens │ cost
        let mut left = vec![
            Span::styled(format!(" {mode_str} "), Style::default().fg(Color::Black).bg(mode_color).bold()),
            Span::styled(
                format!("  {}:{}", self.provider, self.model),
                Style::default().fg(COLOR_BAR_FG),
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
    }

    #[test]
    fn test_push_chat() {
        let mut app = App::new("test", "test");
        app.push_chat("user", "Hello!", ChatStyle::User);
        assert_eq!(app.chat_lines.len(), 1);
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
        assert!(app.handle_slash_command("/clear"));
        assert_eq!(app.chat_lines.len(), 1);
    }

    #[test]
    fn test_slash_unknown() {
        let mut app = App::new("test", "test");
        assert!(app.handle_slash_command("/foo"));
        assert!(app.chat_lines[0].content.contains("Unknown"));
    }

    #[test]
    fn test_non_slash() {
        let mut app = App::new("test", "test");
        assert!(!app.handle_slash_command("hello"));
    }

    #[test]
    fn test_multi_line() {
        let mut app = App::new("test", "test");
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
        let mut app = App::new("test", "test");
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
        let mut app = App::new("test", "test");
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
}
