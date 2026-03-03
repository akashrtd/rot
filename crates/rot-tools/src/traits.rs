//! Tool trait definition and common types.

use crate::error::ToolError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Filesystem sandbox mode for tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    /// Read-only workspace access.
    ReadOnly,
    /// Allow workspace writes, deny writes outside workspace.
    #[default]
    WorkspaceWrite,
    /// No sandbox restrictions.
    DangerFullAccess,
}

/// Request payload for delegated subagent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    /// Subagent name to invoke.
    pub agent: String,
    /// Prompt for the delegated task.
    pub prompt: String,
}

/// Result of delegated subagent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecution {
    /// Subagent final text.
    pub final_text: String,
    /// Child session identifier if one was created.
    pub child_session_id: Option<String>,
    /// Agent that handled the task.
    pub agent: String,
}

/// Request payload for orchestrated swarm delegation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmRequest {
    /// Planner agent used for initial decomposition.
    pub planner_agent: String,
    /// Worker agents that execute planned subtasks.
    pub workers: Vec<String>,
    /// Merger agent used to synthesize worker outputs.
    pub merge_agent: String,
    /// Root prompt for the swarm run.
    pub prompt: String,
}

/// One worker output inside a swarm execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmWorkerResult {
    /// Worker agent name.
    pub agent: String,
    /// Worker final output.
    pub final_text: String,
    /// Child session identifier for audit linkage.
    pub child_session_id: Option<String>,
}

/// Result of orchestrated swarm delegation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmExecution {
    /// Planner output text.
    pub planner_output: String,
    /// Merged final output text.
    pub merged_text: String,
    /// Structured worker outputs.
    pub workers: Vec<SwarmWorkerResult>,
}

/// Callback interface used by the `task` tool.
#[async_trait]
pub trait TaskRunner: Send + Sync {
    /// Execute a delegated task and return the final subagent response.
    async fn run_task(&self, request: TaskRequest) -> Result<TaskExecution, ToolError>;

    /// Execute a planner/worker/merge swarm workflow.
    ///
    /// Default behavior is sequential and conservative; runtimes can override this
    /// for bounded parallelism.
    async fn run_swarm(&self, request: SwarmRequest) -> Result<SwarmExecution, ToolError> {
        if request.workers.is_empty() {
            return Err(ToolError::InvalidParameters(
                "swarm requires at least one worker".to_string(),
            ));
        }

        let planner = self
            .run_task(TaskRequest {
                agent: request.planner_agent.clone(),
                prompt: format!(
                    "Create a concise execution plan for {} worker(s).\nTask:\n{}",
                    request.workers.len(),
                    request.prompt
                ),
            })
            .await?;

        let mut workers = Vec::with_capacity(request.workers.len());
        for worker in &request.workers {
            let result = self
                .run_task(TaskRequest {
                    agent: worker.clone(),
                    prompt: format!(
                        "Task:\n{}\n\nPlanner guidance:\n{}",
                        request.prompt, planner.final_text
                    ),
                })
                .await?;
            workers.push(SwarmWorkerResult {
                agent: result.agent,
                final_text: result.final_text,
                child_session_id: result.child_session_id,
            });
        }

        let merge_payload = serde_json::to_string_pretty(&workers)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to encode worker results: {e}")))?;
        let merged = self
            .run_task(TaskRequest {
                agent: request.merge_agent.clone(),
                prompt: format!(
                    "Merge these worker outputs into one final answer for the original task.\n\
                     Original task:\n{}\n\nWorker outputs (JSON):\n{}",
                    request.prompt, merge_payload
                ),
            })
            .await?;

        Ok(SwarmExecution {
            planner_output: planner.final_text,
            merged_text: merged.final_text,
            workers,
        })
    }
}

/// Context provided to tools during execution.
#[derive(Clone)]
pub struct ToolContext {
    /// Current working directory.
    pub working_dir: PathBuf,
    /// Active session ID.
    pub session_id: String,
    /// Execution timeout.
    pub timeout: Duration,
    /// Filesystem sandbox mode.
    pub sandbox_mode: SandboxMode,
    /// Whether outbound network access is allowed.
    pub network_access: bool,
    /// Current delegated task depth.
    pub task_depth: usize,
    /// Maximum delegated task depth allowed.
    pub max_task_depth: usize,
    /// Optional delegated task runner for the `task` tool.
    pub task_runner: Option<Arc<dyn TaskRunner>>,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("working_dir", &self.working_dir)
            .field("session_id", &self.session_id)
            .field("timeout", &self.timeout)
            .field("sandbox_mode", &self.sandbox_mode)
            .field("network_access", &self.network_access)
            .field("task_depth", &self.task_depth)
            .field("max_task_depth", &self.max_task_depth)
            .field("has_task_runner", &self.task_runner.is_some())
            .finish()
    }
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_default(),
            session_id: String::new(),
            timeout: Duration::from_secs(120),
            sandbox_mode: SandboxMode::WorkspaceWrite,
            network_access: false,
            task_depth: 0,
            max_task_depth: 1,
            task_runner: None,
        }
    }
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Output text.
    pub output: String,
    /// Optional metadata (e.g., line count, file size).
    pub metadata: serde_json::Value,
    /// Whether the result represents an error.
    pub is_error: bool,
}

impl ToolResult {
    /// Create a successful tool result.
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            metadata: serde_json::Value::Null,
            is_error: false,
        }
    }

    /// Create a successful result with metadata.
    pub fn success_with_metadata(output: impl Into<String>, metadata: serde_json::Value) -> Self {
        Self {
            output: output.into(),
            metadata,
            is_error: false,
        }
    }

    /// Create an error tool result.
    pub fn error(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            metadata: serde_json::Value::Null,
            is_error: true,
        }
    }
}

/// Trait that all tools must implement.
///
/// Tools are the primary way the agent interacts with the environment
/// (file system, shell, web, etc.).
#[async_trait]
pub trait Tool: Send + Sync {
    /// Machine-readable tool name (e.g., "read").
    fn name(&self) -> &str;

    /// Human-readable label (e.g., "Read File").
    fn label(&self) -> &str;

    /// Description of what the tool does.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's parameters.
    fn parameters_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given arguments.
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError>;
}

// Compile-time check: Tool must be object-safe
const _: () = {
    fn _assert_object_safe(_: &dyn Tool) {}
};
