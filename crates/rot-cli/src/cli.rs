//! CLI argument and command definitions.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rot", version, about = "Recursive Operations Tool — AI coding agent")]
pub struct Cli {
    /// LLM provider to use.
    #[arg(long, default_value = "anthropic", global = true)]
    pub provider: String,

    /// Model to use (defaults to provider's default model).
    #[arg(long, global = true)]
    pub model: Option<String>,

    /// Enable verbose logging.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start an interactive chat session (default).
    Chat,

    /// Execute a single prompt and exit.
    Exec {
        /// The prompt to execute.
        prompt: String,
    },

    /// Manage sessions.
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
}

#[derive(Subcommand)]
pub enum SessionAction {
    /// List recent sessions.
    List {
        /// Maximum number of sessions to show.
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Resume a previous session.
    Resume {
        /// Session ID to resume.
        id: String,
    },
}
