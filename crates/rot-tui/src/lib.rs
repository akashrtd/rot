//! rot-tui: Terminal user interface for rot.

pub mod app;
pub mod event;
pub mod runner;

pub use app::{App, AppState, ChatStyle};
pub use runner::run_tui;
