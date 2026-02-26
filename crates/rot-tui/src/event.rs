//! Terminal event handling.

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use std::time::Duration;

/// Terminal event types.
#[derive(Debug)]
pub enum TermEvent {
    /// A key was pressed.
    Key(KeyEvent),
    /// Mouse scroll.
    MouseScroll(i16),
    /// Terminal was resized.
    Resize(u16, u16),
    /// No event (tick).
    Tick,
}

/// Poll for terminal events with a timeout.
pub fn poll_event(timeout: Duration) -> std::io::Result<TermEvent> {
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) => Ok(TermEvent::Key(key)),
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => Ok(TermEvent::MouseScroll(-3)),
                MouseEventKind::ScrollDown => Ok(TermEvent::MouseScroll(3)),
                _ => Ok(TermEvent::Tick),
            },
            Event::Resize(w, h) => Ok(TermEvent::Resize(w, h)),
            _ => Ok(TermEvent::Tick),
        }
    } else {
        Ok(TermEvent::Tick)
    }
}

/// Check if a key event is Ctrl+C.
pub fn is_quit(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Check if a key event is Enter.
pub fn is_enter(key: &KeyEvent) -> bool {
    key.code == KeyCode::Enter
}

/// Check if a key event is Escape.
pub fn is_escape(key: &KeyEvent) -> bool {
    key.code == KeyCode::Esc
}
