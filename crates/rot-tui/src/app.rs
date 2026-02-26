//! TUI application state and rendering.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Current application state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// Waiting for user input.
    Idle,
    /// Waiting for LLM response.
    Thinking,
    /// Streaming response from LLM.
    Streaming,
    /// An error occurred.
    Error,
}

/// Input mode for the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Normal mode — keyboard shortcuts active.
    Normal,
    /// Insert mode — typing into the input.
    Insert,
}

/// Main application struct.
pub struct App {
    /// Current state.
    pub state: AppState,
    /// Input mode.
    pub input_mode: InputMode,
    /// Whether the app is running.
    pub running: bool,
    /// User input buffer.
    pub input: String,
    /// Cursor position in input.
    pub cursor_pos: usize,
    /// Chat messages for display.
    pub chat_lines: Vec<ChatLine>,
    /// Current streaming text being accumulated.
    pub streaming_text: String,
    /// Scroll offset for the messages pane.
    pub scroll_offset: u16,
    /// Whether to auto-scroll to bottom.
    pub auto_scroll: bool,
    /// Status bar text.
    pub status: String,
    /// Model name for display.
    pub model: String,
    /// Provider name for display.
    pub provider: String,
    /// Thinking animation frame counter.
    pub thinking_tick: u16,
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
}

impl App {
    /// Create a new application.
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
        }
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

    /// Submit the current input, returning it and clearing the buffer.
    pub fn submit_input(&mut self) -> String {
        let text = self.input.clone();
        self.input.clear();
        self.cursor_pos = 0;
        text
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    /// Delete the character before the cursor.
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

    /// Advance the thinking animation tick.
    pub fn tick(&mut self) {
        self.thinking_tick = self.thinking_tick.wrapping_add(1);
    }

    /// Render the application.
    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Layout: messages (top) | input (bottom) | status bar
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Messages
                Constraint::Length(3), // Input
                Constraint::Length(1), // Status bar
            ])
            .split(area);

        self.render_messages(frame, chunks[0]);
        self.render_input(frame, chunks[1]);
        self.render_status(frame, chunks[2]);
    }

    fn render_messages(&mut self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();

        for msg in &self.chat_lines {
            let role_style = match msg.style {
                ChatStyle::User => Style::default().fg(Color::Cyan).bold(),
                ChatStyle::Assistant => Style::default().fg(Color::Green).bold(),
                ChatStyle::System => Style::default().fg(Color::Yellow).bold(),
                ChatStyle::Tool => Style::default().fg(Color::Magenta).bold(),
                ChatStyle::Error => Style::default().fg(Color::Red).bold(),
                ChatStyle::Thinking => Style::default().fg(Color::DarkGray).italic(),
            };

            // For multi-line content, wrap each line
            let content_lines: Vec<&str> = msg.content.lines().collect();
            if content_lines.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled(format!("{}: ", msg.role), role_style),
                    Span::raw(""),
                ]));
            } else {
                for (i, content_line) in content_lines.iter().enumerate() {
                    if i == 0 {
                        lines.push(Line::from(vec![
                            Span::styled(format!("{}: ", msg.role), role_style),
                            Span::raw(*content_line),
                        ]));
                    } else {
                        // Indent continuation lines
                        let indent = " ".repeat(msg.role.len() + 2);
                        lines.push(Line::from(format!("{indent}{content_line}")));
                    }
                }
            }
            lines.push(Line::from(""));
        }

        // Show streaming text with animation
        if !self.streaming_text.is_empty() {
            let stream_lines: Vec<&str> = self.streaming_text.lines().collect();
            for (i, line) in stream_lines.iter().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "rot: ",
                            Style::default().fg(Color::Green).bold(),
                        ),
                        Span::raw(*line),
                    ]));
                } else {
                    lines.push(Line::from(format!("     {line}")));
                }
            }
        }

        // Show thinking indicator
        if self.state == AppState::Thinking {
            let dots = match (self.thinking_tick / 5) % 4 {
                0 => "   ",
                1 => ".  ",
                2 => ".. ",
                _ => "...",
            };
            lines.push(Line::from(vec![
                Span::styled(
                    "rot: ",
                    Style::default().fg(Color::Green).bold(),
                ),
                Span::styled(
                    format!("thinking{dots}"),
                    Style::default().fg(Color::DarkGray).italic(),
                ),
            ]));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" rot ");

        // Calculate content height for auto-scroll
        let inner_height = area.height.saturating_sub(2); // minus borders
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
            AppState::Thinking | AppState::Streaming => Style::default().fg(Color::DarkGray),
            AppState::Error => Style::default().fg(Color::Red),
        };

        let prompt = match self.input_mode {
            InputMode::Insert => "› ",
            InputMode::Normal => "  ",
        };

        let input_text = format!("{prompt}{}", self.input);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let paragraph = Paragraph::new(input_text).style(style).block(block);
        frame.render_widget(paragraph, area);

        // Show cursor
        if self.input_mode == InputMode::Insert && self.state == AppState::Idle {
            let x = area.x + self.cursor_pos as u16 + 3; // +1 border +2 prompt
            let y = area.y + 1;
            frame.set_cursor_position(Position::new(x, y));
        }
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let state_indicator = match self.state {
            AppState::Idle => "●",
            AppState::Thinking => "◌",
            AppState::Streaming => "◉",
            AppState::Error => "✖",
        };

        let status = format!(
            " {state_indicator} {} | {} / {} | Ctrl+C: quit",
            self.status, self.provider, self.model,
        );

        let bar = Paragraph::new(status)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White));
        frame.render_widget(bar, area);
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
        assert!(app.auto_scroll, "push_chat should enable auto_scroll");
    }
}
