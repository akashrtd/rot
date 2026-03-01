//! Task tool — delegate work to a subagent.

use crate::error::ToolError;
use crate::traits::{TaskRequest, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TaskParams {
    /// Subagent name to invoke.
    pub agent: String,
    /// Prompt for the delegated task.
    pub prompt: String,
}

pub struct TaskTool;

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "task"
    }

    fn label(&self) -> &str {
        "Task"
    }

    fn description(&self) -> &str {
        "Delegate a focused task to a subagent and return its final response."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(TaskParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        if ctx.task_depth >= ctx.max_task_depth {
            return Err(ToolError::PermissionDenied(format!(
                "task recursion limit reached ({})",
                ctx.max_task_depth
            )));
        }

        let params: TaskParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let runner = ctx.task_runner.as_ref().ok_or_else(|| {
            ToolError::ExecutionError("task tool is unavailable in this runtime".to_string())
        })?;

        let result = runner
            .run_task(TaskRequest {
                agent: params.agent,
                prompt: params.prompt,
            })
            .await?;

        Ok(ToolResult::success_with_metadata(
            result.final_text,
            serde_json::json!({
                "agent": result.agent,
                "child_session_id": result.child_session_id,
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{TaskExecution, TaskRunner};
    use async_trait::async_trait;
    use std::sync::Arc;

    struct MockTaskRunner;

    #[async_trait]
    impl TaskRunner for MockTaskRunner {
        async fn run_task(&self, request: TaskRequest) -> Result<TaskExecution, ToolError> {
            Ok(TaskExecution {
                final_text: format!("handled by {}", request.agent),
                child_session_id: Some("child-123".to_string()),
                agent: request.agent,
            })
        }
    }

    #[tokio::test]
    async fn test_task_executes_with_runner() {
        let ctx = ToolContext {
            task_runner: Some(Arc::new(MockTaskRunner)),
            ..Default::default()
        };

        let result = TaskTool
            .execute(
                serde_json::json!({"agent": "review", "prompt": "inspect changes"}),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.output, "handled by review");
        assert_eq!(result.metadata["child_session_id"], "child-123");
    }

    #[tokio::test]
    async fn test_task_respects_depth_limit() {
        let ctx = ToolContext {
            task_depth: 1,
            max_task_depth: 1,
            task_runner: Some(Arc::new(MockTaskRunner)),
            ..Default::default()
        };

        let result = TaskTool
            .execute(
                serde_json::json!({"agent": "review", "prompt": "inspect changes"}),
                &ctx,
            )
            .await;

        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }
}
