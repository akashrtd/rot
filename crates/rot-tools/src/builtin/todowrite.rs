//! Todo write tool — mutate structured task state.

use crate::builtin::todostate::{read_state, status_summary, write_state, TodoItem, TodoStatus};
use crate::error::ToolError;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TodoWriteAction {
    Set,
    Add,
    Update,
    Remove,
    Clear,
}

fn default_action() -> TodoWriteAction {
    TodoWriteAction::Set
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TodoWriteItem {
    pub id: Option<String>,
    pub content: Option<String>,
    pub status: Option<TodoStatus>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TodoWriteParams {
    #[serde(default = "default_action")]
    pub action: TodoWriteAction,
    #[serde(default)]
    pub items: Vec<TodoWriteItem>,
    #[serde(default)]
    pub ids: Vec<String>,
}

pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "todowrite"
    }

    fn label(&self) -> &str {
        "Todo Write"
    }

    fn description(&self) -> &str {
        "Create, update, remove, or replace structured todo/task state for this workspace."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(TodoWriteParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        if ctx.sandbox_mode == SandboxMode::ReadOnly {
            return Err(ToolError::PermissionDenied(
                "todowrite is disabled in read-only sandbox mode".to_string(),
            ));
        }

        let params: TodoWriteParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let mut state = read_state(ctx).await?;

        match params.action {
            TodoWriteAction::Set => {
                if params.items.is_empty() {
                    return Err(ToolError::InvalidParameters(
                        "set action requires non-empty items".to_string(),
                    ));
                }
                state.items = params
                    .items
                    .iter()
                    .map(item_to_todo)
                    .collect::<Result<Vec<_>, _>>()?;
            }
            TodoWriteAction::Add => {
                if params.items.is_empty() {
                    return Err(ToolError::InvalidParameters(
                        "add action requires non-empty items".to_string(),
                    ));
                }
                for item in &params.items {
                    let mut todo = item_to_todo(item)?;
                    if todo.id.is_empty() {
                        todo.id = ulid::Ulid::new().to_string();
                    }
                    if state.items.iter().any(|existing| existing.id == todo.id) {
                        return Err(ToolError::InvalidParameters(format!(
                            "todo id '{}' already exists",
                            todo.id
                        )));
                    }
                    state.items.push(todo);
                }
            }
            TodoWriteAction::Update => {
                if params.items.is_empty() {
                    return Err(ToolError::InvalidParameters(
                        "update action requires non-empty items".to_string(),
                    ));
                }
                for item in &params.items {
                    let id = item.id.clone().ok_or_else(|| {
                        ToolError::InvalidParameters(
                            "update action requires id for each item".to_string(),
                        )
                    })?;
                    let todo = state
                        .items
                        .iter_mut()
                        .find(|entry| entry.id == id)
                        .ok_or_else(|| {
                            ToolError::InvalidParameters(format!("todo id '{}' not found", id))
                        })?;
                    if let Some(content) = &item.content {
                        todo.content = content.clone();
                    }
                    if let Some(status) = &item.status {
                        todo.status = status.clone();
                    }
                }
            }
            TodoWriteAction::Remove => {
                if params.ids.is_empty() {
                    return Err(ToolError::InvalidParameters(
                        "remove action requires non-empty ids".to_string(),
                    ));
                }
                state
                    .items
                    .retain(|item| !params.ids.iter().any(|id| id == &item.id));
            }
            TodoWriteAction::Clear => {
                state.items.clear();
            }
        }

        write_state(ctx, &state).await?;
        let summary = status_summary(&state);

        Ok(ToolResult::success_with_metadata(
            format!(
                "Todo state updated: {} total (pending: {}, in_progress: {}, completed: {})",
                summary["total"],
                summary["pending"],
                summary["in_progress"],
                summary["completed"],
            ),
            summary,
        ))
    }
}

fn item_to_todo(item: &TodoWriteItem) -> Result<TodoItem, ToolError> {
    let content = item.content.clone().ok_or_else(|| {
        ToolError::InvalidParameters("todo item content is required".to_string())
    })?;
    let id = item.id.clone().unwrap_or_default();
    let status = item.status.clone().unwrap_or(TodoStatus::Pending);

    Ok(TodoItem { id, content, status })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::todostate::read_state;
    use tempfile::TempDir;

    fn test_ctx(dir: &TempDir) -> ToolContext {
        ToolContext {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_todowrite_set_and_update() {
        let dir = TempDir::new().unwrap();
        let ctx = test_ctx(&dir);

        TodoWriteTool
            .execute(
                serde_json::json!({
                    "action":"set",
                    "items":[{"id":"a","content":"plan","status":"pending"}]
                }),
                &ctx,
            )
            .await
            .unwrap();

        TodoWriteTool
            .execute(
                serde_json::json!({
                    "action":"update",
                    "items":[{"id":"a","status":"completed"}]
                }),
                &ctx,
            )
            .await
            .unwrap();

        let state = read_state(&ctx).await.unwrap();
        assert_eq!(state.items.len(), 1);
        assert_eq!(state.items[0].status, TodoStatus::Completed);
    }

    #[tokio::test]
    async fn test_todowrite_remove() {
        let dir = TempDir::new().unwrap();
        let ctx = test_ctx(&dir);

        TodoWriteTool
            .execute(
                serde_json::json!({
                    "action":"set",
                    "items":[{"id":"a","content":"x","status":"pending"}]
                }),
                &ctx,
            )
            .await
            .unwrap();

        TodoWriteTool
            .execute(
                serde_json::json!({"action":"remove","ids":["a"]}),
                &ctx,
            )
            .await
            .unwrap();

        let state = read_state(&ctx).await.unwrap();
        assert!(state.items.is_empty());
    }

    #[tokio::test]
    async fn test_todowrite_denied_in_read_only() {
        let dir = TempDir::new().unwrap();
        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
            sandbox_mode: SandboxMode::ReadOnly,
            ..Default::default()
        };

        let result = TodoWriteTool
            .execute(
                serde_json::json!({"action":"clear"}),
                &ctx,
            )
            .await;

        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }
}
