//! Swarm orchestration (planner -> workers -> merge).

use crate::task_scheduler::run_bounded;
use rot_tools::{SwarmExecution, SwarmRequest, SwarmWorkerResult, TaskRequest, TaskRunner, ToolError};

/// Swarm orchestration configuration.
#[derive(Debug, Clone, Copy)]
pub struct SwarmConfig {
    /// Maximum concurrent workers for fan-out.
    pub max_concurrency: usize,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self { max_concurrency: 1 }
    }
}

/// Execute planner -> workers -> merge using the supplied task runner.
pub async fn run_swarm(
    runner: &dyn TaskRunner,
    request: SwarmRequest,
    config: SwarmConfig,
) -> Result<SwarmExecution, ToolError> {
    if request.workers.is_empty() {
        return Err(ToolError::InvalidParameters(
            "swarm requires at least one worker".to_string(),
        ));
    }

    let planner = runner
        .run_task(TaskRequest {
            agent: request.planner_agent.clone(),
            prompt: format!(
                "Plan explicit subtask guidance for {} workers.\nTask:\n{}",
                request.workers.len(),
                request.prompt
            ),
        })
        .await?;

    let jobs = request.workers.clone().into_iter().map(|worker| {
        let prompt = request.prompt.clone();
        let planner_text = planner.final_text.clone();
        async move {
            let result = runner
                .run_task(TaskRequest {
                    agent: worker.clone(),
                    prompt: format!("Task:\n{}\n\nPlanner guidance:\n{}", prompt, planner_text),
                })
                .await;
            (worker, result)
        }
    });

    let worker_runs = run_bounded(jobs, config.max_concurrency).await;
    let mut workers = Vec::with_capacity(worker_runs.len());
    for (agent, result) in worker_runs {
        let result = result?;
        workers.push(SwarmWorkerResult {
            agent,
            final_text: result.final_text,
            child_session_id: result.child_session_id,
        });
    }

    let merge_payload = serde_json::to_string_pretty(&workers)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to encode worker outputs: {e}")))?;
    let merged = runner
        .run_task(TaskRequest {
            agent: request.merge_agent.clone(),
            prompt: format!(
                "Merge worker outputs into one final response.\n\
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

#[cfg(test)]
mod tests {
    use super::{SwarmConfig, run_swarm};
    use async_trait::async_trait;
    use rot_tools::{
        SwarmExecution, SwarmRequest, TaskExecution, TaskRequest, TaskRunner, ToolError,
    };

    struct MockRunner;

    #[async_trait]
    impl TaskRunner for MockRunner {
        async fn run_task(&self, request: TaskRequest) -> Result<TaskExecution, ToolError> {
            Ok(TaskExecution {
                final_text: format!("handled by {}", request.agent),
                child_session_id: Some(format!("child-{}", request.agent)),
                agent: request.agent,
            })
        }

        async fn run_swarm(&self, request: SwarmRequest) -> Result<SwarmExecution, ToolError> {
            run_swarm(self, request, SwarmConfig { max_concurrency: 2 }).await
        }
    }

    #[tokio::test]
    async fn test_run_swarm_produces_worker_outputs() {
        let result = run_swarm(
            &MockRunner,
            SwarmRequest {
                planner_agent: "plan".to_string(),
                workers: vec!["review".to_string(), "explore".to_string()],
                merge_agent: "default".to_string(),
                prompt: "audit and summarize".to_string(),
            },
            SwarmConfig { max_concurrency: 2 },
        )
        .await
        .unwrap();

        assert_eq!(result.workers.len(), 2);
        assert!(result.planner_output.contains("plan"));
        assert!(result.merged_text.contains("default"));
    }
}
