//! Shared todo state storage for `todoread` and `todowrite`.

use crate::error::ToolError;
use crate::path_guard::resolve_path_for_write;
use crate::traits::{SandboxMode, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

const TODO_STATE_REL_PATH: &str = ".rot/todos.json";

/// Allowed statuses for each todo item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TodoStatus {
    /// Task has not started.
    Pending,
    /// Task is currently in progress.
    InProgress,
    /// Task is complete.
    Completed,
}

/// One structured todo entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct TodoItem {
    /// Stable identifier for the task.
    pub id: String,
    /// Human-readable task content.
    pub content: String,
    /// Current task status.
    pub status: TodoStatus,
}

/// Persisted todo state for a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, JsonSchema)]
pub struct TodoState {
    /// Ordered todo items.
    pub items: Vec<TodoItem>,
}

/// Read todo state from workspace storage.
pub async fn read_state(ctx: &ToolContext) -> Result<TodoState, ToolError> {
    let path = state_path(ctx)?;
    if !path.exists() {
        return Ok(TodoState::default());
    }

    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| ToolError::ExecutionError(format!("Failed to read todo state: {e}")))?;

    if content.trim().is_empty() {
        return Ok(TodoState::default());
    }

    serde_json::from_str(&content)
        .map_err(|e| ToolError::ExecutionError(format!("Invalid todo state JSON: {e}")))
}

/// Persist todo state to workspace storage.
pub async fn write_state(ctx: &ToolContext, state: &TodoState) -> Result<(), ToolError> {
    if ctx.sandbox_mode == SandboxMode::ReadOnly {
        return Err(ToolError::PermissionDenied(
            "todowrite is disabled in read-only sandbox mode".to_string(),
        ));
    }

    let path = state_path(ctx)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            ToolError::ExecutionError(format!("Failed to create todo state directory: {e}"))
        })?;
    }

    let json = serde_json::to_string_pretty(state)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to encode todo state: {e}")))?;

    tokio::fs::write(&path, json)
        .await
        .map_err(|e| ToolError::ExecutionError(format!("Failed to write todo state: {e}")))
}

fn state_path(ctx: &ToolContext) -> Result<std::path::PathBuf, ToolError> {
    let rel = Path::new(TODO_STATE_REL_PATH);
    match ctx.sandbox_mode {
        SandboxMode::WorkspaceWrite => resolve_path_for_write(rel, &ctx.working_dir),
        SandboxMode::DangerFullAccess | SandboxMode::ReadOnly => Ok(ctx.working_dir.join(rel)),
    }
}

/// Build a summary count by status for UI and tool metadata.
pub fn status_summary(state: &TodoState) -> serde_json::Value {
    let pending = state
        .items
        .iter()
        .filter(|item| item.status == TodoStatus::Pending)
        .count();
    let in_progress = state
        .items
        .iter()
        .filter(|item| item.status == TodoStatus::InProgress)
        .count();
    let completed = state
        .items
        .iter()
        .filter(|item| item.status == TodoStatus::Completed)
        .count();

    serde_json::json!({
        "total": state.items.len(),
        "pending": pending,
        "in_progress": in_progress,
        "completed": completed,
    })
}
