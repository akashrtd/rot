//! Built-in agent registry.

use crate::agent_profile::{AgentMode, AgentProfile};

const DEFAULT_SYSTEM_PROMPT: &str =
    "You are rot, an AI coding assistant. Be concise and direct.";
const CHAT_SYSTEM_PROMPT: &str = "You are rot, a powerful AI coding assistant. \
You have access to tools for reading, writing, and editing files, \
running shell commands, searching code, and fetching URLs. \
Be concise and helpful. Use markdown formatting in your responses \
(bold for emphasis, backticks for code).";

const BUILTIN_AGENTS: &[AgentProfile] = &[
    AgentProfile {
        name: "default",
        display_name: "Default",
        description: "General-purpose coding assistant.",
        mode: AgentMode::Primary,
        system_prompt: DEFAULT_SYSTEM_PROMPT,
    },
    AgentProfile {
        name: "build",
        display_name: "Build",
        description: "Focus on implementing changes with disciplined execution.",
        mode: AgentMode::Primary,
        system_prompt: "You are rot in build mode. Prioritize correct implementation, \
work incrementally, and keep responses concise and execution-focused.",
    },
    AgentProfile {
        name: "plan",
        display_name: "Plan",
        description: "Focus on decomposition, sequencing, risks, and acceptance criteria.",
        mode: AgentMode::Primary,
        system_prompt: "You are rot in planning mode. Break problems into concrete steps, \
surface assumptions and risks, and optimize for an actionable engineering plan.",
    },
    AgentProfile {
        name: "explore",
        display_name: "Explore",
        description: "Investigate codepaths quickly and return findings with evidence.",
        mode: AgentMode::Subagent,
        system_prompt: "You are rot in explore mode. Gather relevant code context quickly, \
cite concrete evidence, and avoid speculative conclusions.",
    },
    AgentProfile {
        name: "review",
        display_name: "Review",
        description: "Review changes for bugs, regressions, and missing tests.",
        mode: AgentMode::Subagent,
        system_prompt: "You are rot in review mode. Prioritize identifying defects, \
behavioral regressions, edge cases, and missing tests over summaries.",
    },
];

/// Registry over built-in agent profiles.
pub struct AgentRegistry;

impl AgentRegistry {
    /// Returns all built-in agent profiles.
    pub fn builtins() -> &'static [AgentProfile] {
        BUILTIN_AGENTS
    }

    /// Returns the default agent profile.
    pub fn default_agent() -> AgentProfile {
        BUILTIN_AGENTS[0]
    }

    /// Returns the default system prompt for interactive chat.
    pub fn default_chat_system_prompt() -> &'static str {
        CHAT_SYSTEM_PROMPT
    }

    /// Returns the named agent profile if it exists.
    pub fn get(name: &str) -> Option<AgentProfile> {
        BUILTIN_AGENTS
            .iter()
            .copied()
            .find(|profile| profile.name.eq_ignore_ascii_case(name))
    }

    /// Resolve an optional agent name into a concrete profile.
    pub fn resolve(name: Option<&str>) -> Result<AgentProfile, UnknownAgentError> {
        match name {
            Some(name) => Self::get(name).ok_or_else(|| UnknownAgentError {
                requested: name.to_string(),
            }),
            None => Ok(Self::default_agent()),
        }
    }

    /// Returns the built-in primary agents.
    pub fn primary_agents() -> Vec<AgentProfile> {
        BUILTIN_AGENTS
            .iter()
            .copied()
            .filter(|profile| profile.is_primary())
            .collect()
    }
}

/// Error returned when a requested agent does not exist.
#[derive(Debug)]
pub struct UnknownAgentError {
    requested: String,
}

impl UnknownAgentError {
    fn available(&self) -> String {
        AgentRegistry::builtins()
            .iter()
            .map(|profile| profile.name)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl std::fmt::Display for UnknownAgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Unknown agent '{}'. Available agents: {}",
            self.requested,
            self.available()
        )
    }
}

impl std::error::Error for UnknownAgentError {}

#[cfg(test)]
mod tests {
    use super::AgentRegistry;
    use crate::agent_profile::AgentMode;

    #[test]
    fn test_default_agent_is_available() {
        let agent = AgentRegistry::default_agent();
        assert_eq!(agent.name, "default");
        assert_eq!(agent.mode, AgentMode::Primary);
    }

    #[test]
    fn test_resolve_named_agent() {
        let agent = AgentRegistry::resolve(Some("build")).unwrap();
        assert_eq!(agent.name, "build");
    }

    #[test]
    fn test_primary_agents_include_default_and_plan() {
        let names = AgentRegistry::primary_agents()
            .into_iter()
            .map(|profile| profile.name)
            .collect::<Vec<_>>();
        assert!(names.contains(&"default"));
        assert!(names.contains(&"plan"));
        assert!(!names.contains(&"review"));
    }

    #[test]
    fn test_unknown_agent_error_lists_available() {
        let error = AgentRegistry::resolve(Some("missing")).unwrap_err().to_string();
        assert!(error.contains("Unknown agent"));
        assert!(error.contains("default"));
        assert!(error.contains("review"));
    }
}
