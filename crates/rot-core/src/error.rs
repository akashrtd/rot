//! Error types for the rot-core crate.

use std::time::Duration;
use rot_provider::ProviderError;

/// Core error type for the rot agent.
#[derive(Debug, thiserror::Error)]
pub enum RotError {
    /// I/O error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Generic error with message
    #[error("{0}")]
    Other(String),
}

/// Errors that can occur during agent processing.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("Maximum iterations reached ({0})")]
    MaxIterationsReached(usize),

    #[error("Tool execution failed: {0}")]
    ToolExecution(String),

    #[error("Tool '{0}' requires approval but running in non-interactive mode")]
    ApprovalRequired(String),

    #[error("Operation timed out after {0:?}")]
    Timeout(Duration),
    
    #[error("Unknown agent: {0}")]
    UnknownAgent(String),
}

impl AgentError {
    pub fn suggestions(&self) -> Vec<String> {
        match self {
            AgentError::Timeout(duration) => vec![
                format!("Increase timeout with --timeout {}s", duration.as_secs() * 2),
                "Simplify your request to reduce processing time".to_string(),
                "Use --auto-approve for non-interactive mode".to_string(),
                "Run in interactive mode (without 'exec' subcommand) for complex tasks".to_string(),
            ],
            AgentError::ApprovalRequired(tool) => vec![
                "Use --auto-approve to allow all tool calls".to_string(),
                format!("Use --approve-list {} to allow this specific tool", tool),
                "Run in interactive mode (without 'exec' subcommand) to approve manually".to_string(),
            ],
            AgentError::MaxIterationsReached(_) => vec![
                "Break down your request into smaller tasks".to_string(),
                "Use --max-iterations to increase the limit".to_string(),
            ],
            _ => vec![],
        }
    }

    pub fn to_detailed_string(&self) -> String {
        let mut msg = self.to_string();
        let suggestions = self.suggestions();
        if !suggestions.is_empty() {
            msg.push_str("\n\nSuggestions:\n");
            for (i, suggestion) in suggestions.iter().enumerate() {
                msg.push_str(&format!("  {}. {}\n", i + 1, suggestion));
            }
        }
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_suggestions() {
        let err = AgentError::Timeout(Duration::from_secs(120));
        let detailed = err.to_detailed_string();
        assert!(detailed.contains("Operation timed out after 120s"), "Missing error message");
        assert!(detailed.contains("Suggestions:"), "Missing suggestions header");
        assert!(detailed.contains("Increase timeout with --timeout 240s"), "Missing timeout suggestion");

        let err2 = AgentError::ApprovalRequired("bash".to_string());
        let detailed2 = err2.to_detailed_string();
        assert!(detailed2.contains("requires approval"), "Missing error message");
        assert!(detailed2.contains("Use --approve-list bash to allow this specific tool"), "Missing specific tool suggestion");
        
        let err3 = AgentError::MaxIterationsReached(50);
        let detailed3 = err3.to_detailed_string();
        assert!(detailed3.contains("Maximum iterations reached (50)"), "Missing error message");
        assert!(detailed3.contains("Break down your request into smaller tasks"), "Missing iteration suggestion");
    }
}
