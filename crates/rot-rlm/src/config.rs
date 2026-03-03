//! Configuration for RLM execution.

use crate::runtime::RlmRuntimeKind;
use rot_core::Agent;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// RLM configuration.
#[derive(Clone)]
pub struct RlmConfig {
    /// Maximum number of top-level iterations.
    pub max_iterations: usize,
    /// Global wall-clock timeout for one RLM run.
    pub max_timeout: Option<Duration>,
    /// Optional progress callback.
    pub on_progress: Option<Arc<dyn Fn(String) + Send + Sync>>,
    /// Runtime backend.
    pub runtime: RlmRuntimeKind,
    /// Maximum nested SUBLM recursion depth.
    pub max_subcall_depth: usize,
    /// Maximum number of SUBLM calls per run.
    pub max_subcalls: usize,
    /// Timeout for each SUBLM call.
    pub subcall_timeout: Duration,
    /// Optional max token budget across top-level + subcalls.
    pub max_total_tokens: Option<usize>,
    /// Optional dedicated agent for subcalls (e.g. smaller model routing).
    pub subcall_agent: Option<Arc<Agent>>,
    /// Optional custom output directory for trajectory artifacts.
    pub trajectory_dir: Option<PathBuf>,
    /// Max chars persisted per trace field.
    pub trace_max_chars: usize,
}

impl std::fmt::Debug for RlmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RlmConfig")
            .field("max_iterations", &self.max_iterations)
            .field("max_timeout", &self.max_timeout)
            .field("runtime", &self.runtime)
            .field("max_subcall_depth", &self.max_subcall_depth)
            .field("max_subcalls", &self.max_subcalls)
            .field("subcall_timeout", &self.subcall_timeout)
            .field("max_total_tokens", &self.max_total_tokens)
            .field("subcall_agent", &self.subcall_agent.as_ref().map(|_| "<agent>"))
            .field("trajectory_dir", &self.trajectory_dir)
            .field("trace_max_chars", &self.trace_max_chars)
            .finish()
    }
}

impl Default for RlmConfig {
    fn default() -> Self {
        Self {
            max_iterations: 30,
            max_timeout: Some(Duration::from_secs(300)),
            on_progress: None,
            runtime: RlmRuntimeKind::Python,
            max_subcall_depth: 2,
            max_subcalls: 16,
            subcall_timeout: Duration::from_secs(45),
            max_total_tokens: Some(120_000),
            subcall_agent: None,
            trajectory_dir: None,
            trace_max_chars: 12_000,
        }
    }
}
