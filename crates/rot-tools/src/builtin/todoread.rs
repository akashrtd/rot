//! Todo read tool — read structured task state.

use crate::builtin::todostate::{read_state, status_summary};
use crate::error::ToolError;
use crate::traits::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct TodoReadParams {
    /// If true, return pretty JSON only.
    #[serde(default)]
    pub json_only: bool,
}

pub struct TodoReadTool;

#[async_trait]
impl Tool for TodoReadTool {
    fn name(&self) -> &str {
        "todoread"
    }

    fn label(&self) -> &str {
        "Todo Read"
    }

    fn description(&self) -> &str {
        "Read structured todo/task state for the current workspace."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(TodoReadParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: TodoReadParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let state = read_state(ctx).await?;
        let summary = status_summary(&state);
        let pretty = serde_json::to_string_pretty(&state.items)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to encode todo items: {e}")))?;

        let output = if params.json_only {
            pretty
        } else {
            format!(
                "Todo items: {} (pending: {}, in_progress: {}, completed: {})\n\n{}",
                summary["total"],
                summary["pending"],
                summary["in_progress"],
                summary["completed"],
                pretty
            )
        };

        Ok(ToolResult::success_with_metadata(output, summary))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::todostate::{write_state, TodoItem, TodoState, TodoStatus};
    use tempfile::TempDir;

    fn test_ctx(dir: &TempDir) -> ToolContext {
        ToolContext {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_todoread_returns_items() {
        let dir = TempDir::new().unwrap();
        let ctx = test_ctx(&dir);
        write_state(
            &ctx,
            &TodoState {
                items: vec![TodoItem {
                    id: "1".to_string(),
                    content: "Implement list tool".to_string(),
                    status: TodoStatus::Pending,
                }],
            },
        )
        .await
        .unwrap();

        let result = TodoReadTool
            .execute(serde_json::json!({}), &ctx)
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.output.contains("Implement list tool"));
        assert_eq!(result.metadata["total"], 1);
    }
}
