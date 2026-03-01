//! Built-in agent profiles and metadata.

use serde::{Deserialize, Serialize};

/// Runtime classification for an agent profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    /// Main interactive agent intended for direct user selection.
    Primary,
    /// Delegable helper agent intended for focused subtasks.
    Subagent,
}

/// Agent profile metadata and prompt template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProfile {
    /// Stable machine-readable name.
    pub name: &'static str,
    /// Human-readable label.
    pub display_name: &'static str,
    /// Short description for selection UIs and help text.
    pub description: &'static str,
    /// Intended usage mode.
    pub mode: AgentMode,
    /// System prompt applied when the agent is selected.
    pub system_prompt: &'static str,
}

impl AgentProfile {
    /// Whether this agent is intended for direct user selection.
    pub fn is_primary(self) -> bool {
        self.mode == AgentMode::Primary
    }

    /// Whether this agent is intended for delegated subagent use.
    pub fn is_subagent(self) -> bool {
        self.mode == AgentMode::Subagent
    }
}
