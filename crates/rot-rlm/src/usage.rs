//! Usage and budget accounting for RLM runs.

use serde::{Deserialize, Serialize};

/// Aggregated usage metrics for one RLM run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RlmUsage {
    /// Estimated input tokens consumed by top-level iterations and subcalls.
    pub input_tokens: usize,
    /// Estimated output tokens produced by top-level iterations and subcalls.
    pub output_tokens: usize,
    /// Number of nested subcalls executed.
    pub subcall_count: usize,
}

impl RlmUsage {
    /// Add estimated token usage for one model exchange.
    pub fn add_exchange(&mut self, input_text: &str, output_text: &str) {
        self.input_tokens += estimate_tokens(input_text);
        self.output_tokens += estimate_tokens(output_text);
    }

    /// Return total estimated tokens.
    pub fn total_tokens(&self) -> usize {
        self.input_tokens + self.output_tokens
    }
}

/// Rough token estimator used when provider token usage is unavailable.
///
/// Uses a conservative 4 chars/token heuristic.
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.chars().count().div_ceil(4)
}

#[cfg(test)]
mod tests {
    use super::{RlmUsage, estimate_tokens};

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
    }

    #[test]
    fn test_usage_accumulates() {
        let mut usage = RlmUsage::default();
        usage.add_exchange("hello world", "answer");
        assert!(usage.input_tokens > 0);
        assert!(usage.output_tokens > 0);
        assert_eq!(usage.total_tokens(), usage.input_tokens + usage.output_tokens);
    }
}
