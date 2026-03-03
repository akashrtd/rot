//! RLM runtime backend selection.

use serde::{Deserialize, Serialize};

/// Runtime backend used by RLM execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RlmRuntimeKind {
    /// Python runtime (default).
    Python,
    /// Legacy bash runtime.
    Bash,
}

impl Default for RlmRuntimeKind {
    fn default() -> Self {
        Self::Python
    }
}
