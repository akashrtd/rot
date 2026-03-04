use rot_core::AgentProcessError;
use std::time::Duration;

#[test]
fn test_agent_process_error_suggestions() {
    let err = AgentProcessError::Timeout(Duration::from_secs(30));
    let suggestions = err.suggestions();
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.contains("--auto-approve")));
}

#[test]
fn test_agent_process_error_to_detailed_string() {
    let err = AgentProcessError::ApprovalRequired("bash".to_string());
    let detailed = err.to_detailed_string();
    assert!(detailed.contains("bash"));
    assert!(detailed.contains("Suggestions:"));
    assert!(detailed.contains("--auto-approve"));
}

#[test]
fn test_max_iterations_error() {
    let err = AgentProcessError::MaxIterations(50);
    let detailed = err.to_detailed_string();
    assert!(detailed.contains("50"));
    assert!(detailed.contains("Suggestions:"));
}

#[test]
fn test_provider_error_no_panic() {
    let err = AgentProcessError::Provider(rot_provider::ProviderError::ApiError("test".to_string()));
    let suggestions = err.suggestions();
    assert!(!suggestions.is_empty());
}
