use rot_core::{AgentConfig, AgentProcessError, TaskExecutionPolicy};
use std::time::Duration;

#[test]
fn test_agent_config_default() {
    let config = AgentConfig::default();
    assert_eq!(config.max_iterations, 50);
    assert_eq!(config.agent_name, "default");
    assert!(config.system_prompt.is_none());
    assert_eq!(config.max_tokens, None);
}

#[test]
fn test_agent_config_custom() {
    let config = AgentConfig {
        max_iterations: 10,
        agent_name: "custom".to_string(),
        system_prompt: Some("Be helpful".to_string()),
        max_tokens: Some(2048),
        task_policy: TaskExecutionPolicy::default(),
    };
    assert_eq!(config.max_iterations, 10);
    assert_eq!(config.agent_name, "custom");
    assert_eq!(config.system_prompt, Some("Be helpful".to_string()));
    assert_eq!(config.max_tokens, Some(2048));
}

#[test]
fn test_task_policy_default() {
    let policy = TaskExecutionPolicy::default();
    assert_eq!(policy.max_depth, 1);
    assert_eq!(policy.max_total_tasks, 8);
    assert_eq!(policy.max_concurrent_tasks, 1);
    assert_eq!(policy.task_timeout, Duration::from_secs(120));
}

#[test]
fn test_task_policy_custom() {
    let policy = TaskExecutionPolicy {
        max_depth: 3,
        max_total_tasks: 16,
        max_concurrent_tasks: 4,
        task_timeout: Duration::from_secs(60),
    };
    assert_eq!(policy.max_depth, 3);
    assert_eq!(policy.max_total_tasks, 16);
    assert_eq!(policy.max_concurrent_tasks, 4);
}

#[test]
fn test_agent_process_error_timeout_suggestions() {
    let err = AgentProcessError::Timeout(Duration::from_secs(30));
    let suggestions = err.suggestions();
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.contains("--auto-approve")));
}

#[test]
fn test_agent_process_error_approval_suggestions() {
    let err = AgentProcessError::ApprovalRequired("bash".to_string());
    let suggestions = err.suggestions();
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.contains("bash")));
}

#[test]
fn test_agent_process_error_max_iterations_suggestions() {
    let err = AgentProcessError::MaxIterations(50);
    let suggestions = err.suggestions();
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.contains("smaller")));
}

#[test]
fn test_agent_process_error_detailed_string() {
    let err = AgentProcessError::ApprovalRequired("write".to_string());
    let detailed = err.to_detailed_string();
    assert!(detailed.contains("write"));
    assert!(detailed.contains("Suggestions:"));
}

#[test]
fn test_agent_process_error_tool_execution_suggestions() {
    let err = AgentProcessError::ToolExecution("test error".to_string());
    let suggestions = err.suggestions();
    assert!(!suggestions.is_empty());
}

#[test]
fn test_agent_config_clone() {
    let config = AgentConfig::default();
    let cloned = config.clone();
    assert_eq!(config.max_iterations, cloned.max_iterations);
    assert_eq!(config.agent_name, cloned.agent_name);
}

#[test]
fn test_task_policy_clone() {
    let policy = TaskExecutionPolicy::default();
    let cloned = policy.clone();
    assert_eq!(policy.max_depth, cloned.max_depth);
    assert_eq!(policy.max_total_tasks, cloned.max_total_tasks);
}

#[test]
fn test_agent_process_error_timeout_display() {
    let err = AgentProcessError::Timeout(Duration::from_secs(30));
    let msg = err.to_string();
    assert!(msg.contains("30"));
    assert!(msg.contains("timed out"));
}

#[test]
fn test_agent_process_error_max_iterations_display() {
    let err = AgentProcessError::MaxIterations(50);
    let msg = err.to_string();
    assert!(msg.contains("50"));
    assert!(msg.contains("iterations"));
}

#[test]
fn test_agent_process_error_approval_display() {
    let err = AgentProcessError::ApprovalRequired("read".to_string());
    let msg = err.to_string();
    assert!(msg.contains("read"));
    assert!(msg.contains("approval"));
}
