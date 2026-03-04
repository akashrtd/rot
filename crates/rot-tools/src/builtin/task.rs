//! Task tool — delegate work to a subagent.

use crate::error::ToolError;
use crate::traits::{SwarmRequest, TaskRequest, TaskRunner, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TaskParams {
    /// Subagent name to invoke for single-task mode.
    #[serde(default)]
    pub agent: Option<String>,
    /// Prompt for delegated task(s).
    pub prompt: String,
    /// Enable swarm orchestration mode.
    #[serde(default)]
    pub swarm: bool,
    /// Worker agents for swarm mode.
    #[serde(default)]
    pub workers: Vec<String>,
    /// Optional planner override for swarm mode.
    #[serde(default)]
    pub planner_agent: Option<String>,
    /// Optional merge override for swarm mode.
    #[serde(default)]
    pub merge_agent: Option<String>,
}

pub struct TaskTool;

impl TaskTool {
    async fn execute_internal(
        &self,
        params: TaskParams,
        runner: &std::sync::Arc<dyn TaskRunner>,
    ) -> Result<ToolResult, ToolError> {
        if params.swarm {
            if params.workers.is_empty() {
                return Err(ToolError::InvalidParameters(
                    "swarm mode requires non-empty workers".to_string(),
                ));
            }

            let swarm_result = runner
                .run_swarm(SwarmRequest {
                    planner_agent: params
                        .planner_agent
                        .unwrap_or_else(|| "plan".to_string()),
                    workers: params.workers,
                    merge_agent: params
                        .merge_agent
                        .unwrap_or_else(|| "default".to_string()),
                    prompt: params.prompt,
                })
                .await?;

            return Ok(ToolResult::success_with_metadata(
                swarm_result.merged_text,
                serde_json::json!({
                    "mode": "swarm",
                    "planner_output": swarm_result.planner_output,
                    "workers": swarm_result.workers,
                }),
            ));
        }

        if !params.workers.is_empty() {
            return Err(ToolError::InvalidParameters(
                "workers provided but swarm=false; set swarm=true to enable orchestrated delegation"
                    .to_string(),
            ));
        }

        let agent = params.agent.ok_or_else(|| {
            ToolError::InvalidParameters("agent is required when swarm=false".to_string())
        })?;

        let result = runner.run_task(TaskRequest { agent, prompt: params.prompt }).await?;

        Ok(ToolResult::success_with_metadata(
            result.final_text,
            serde_json::json!({
                "mode": "single",
                "agent": result.agent,
                "child_session_id": result.child_session_id,
            }),
        ))
    }
}

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

        // Wrap execution with timeout
        let timeout_duration = ctx.timeout;
        let execution_future = self.execute_internal(params, runner);
        
        match tokio::time::timeout(timeout_duration, execution_future).await {
            Ok(result) => result,
            Err(_) => Err(ToolError::Timeout(format!(
                "Task execution exceeded {:?}",
                timeout_duration
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{SwarmExecution, SwarmRequest, TaskExecution, TaskRunner, SwarmWorkerResult};
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

        async fn run_swarm(&self, request: SwarmRequest) -> Result<SwarmExecution, ToolError> {
            Ok(SwarmExecution {
                planner_output: format!("plan by {}", request.planner_agent),
                merged_text: "merged swarm output".to_string(),
                workers: request
                    .workers
                    .into_iter()
                    .map(|agent| SwarmWorkerResult {
                        final_text: format!("output from {}", agent),
                        child_session_id: Some(format!("child-{}", agent)),
                        agent,
                    })
                    .collect(),
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
        assert_eq!(result.metadata["mode"], "single");
    }

    #[tokio::test]
    async fn test_task_swarm_executes_when_enabled() {
        let ctx = ToolContext {
            task_runner: Some(Arc::new(MockTaskRunner)),
            ..Default::default()
        };

        let result = TaskTool
            .execute(
                serde_json::json!({
                    "swarm": true,
                    "workers": vec!["review", "explore"],
                    "planner_agent": "plan",
                    "merge_agent": "default",
                    "prompt": "analyze this change set"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result.output, "merged swarm output");
        assert_eq!(result.metadata["mode"], "swarm");
        assert_eq!(result.metadata["workers"].as_array().unwrap().len(), 2);
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
                serde_json::json!({"agent": "review", "prompt":  "inspect changes"}),
                &ctx,
            )
            .await;

        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn test_task_timeout_returns_informative_error() {
        use std::sync::Arc;
        use tokio::time::{sleep, Duration};
        
        struct SlowTaskRunner;
        
        #[async_trait]
        impl TaskRunner for SlowTaskRunner {
            async fn run_task(&self, _request: TaskRequest) -> Result<TaskExecution, ToolError> {
                sleep(Duration::from_secs(10)).await;
                Ok(TaskExecution {
                    final_text: "done".to_string(),
                    child_session_id: None,
                    agent: "test".to_string(),
                })
            }
            
            async fn run_swarm(&self, _request: SwarmRequest) -> Result<SwarmExecution, ToolError> {
                unimplemented!()
            }
        }
        
        let ctx = ToolContext {
            task_runner: Some(Arc::new(SlowTaskRunner)),
            timeout: Duration::from_millis(100),
            ..Default::default()
        };
        
        let result = TaskTool
            .execute(
                serde_json::json!({"agent": "test", "prompt":  "slow task"}),
                &ctx,
            )
            .await;
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ToolError::Timeout(_)));
        assert!(err.to_string().contains("100ms") || err.to_string().contains("0"));
    }
}
