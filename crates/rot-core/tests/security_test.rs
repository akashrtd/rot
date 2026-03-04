use rot_core::security::{ApprovalPolicy, RuntimeSecurityConfig, SandboxMode};

#[test]
fn test_approval_policy_default() {
    let policy = ApprovalPolicy::default();
    assert_eq!(policy, ApprovalPolicy::OnRequest);
}

#[test]
fn test_sandbox_mode_default() {
    let mode = SandboxMode::default();
    assert_eq!(mode, SandboxMode::WorkspaceWrite);
}

#[test]
fn test_runtime_security_config_default() {
    let config = RuntimeSecurityConfig::default();
    assert_eq!(config.approval_policy, ApprovalPolicy::OnRequest);
    assert_eq!(config.sandbox_mode, SandboxMode::WorkspaceWrite);
    assert!(!config.sandbox_network_access);
}

#[test]
fn test_is_danger_full_access() {
    let config = RuntimeSecurityConfig {
        sandbox_mode: SandboxMode::DangerFullAccess,
        ..Default::default()
    };
    assert!(config.is_danger_full_access());
    
    let config = RuntimeSecurityConfig {
        sandbox_mode: SandboxMode::ReadOnly,
        ..Default::default()
    };
    assert!(!config.is_danger_full_access());
}

#[test]
fn test_effective_network_access() {
    let config = RuntimeSecurityConfig {
        sandbox_mode: SandboxMode::DangerFullAccess,
        sandbox_network_access: false,
        ..Default::default()
    };
    assert!(config.effective_network_access());
    
    let config = RuntimeSecurityConfig {
        sandbox_mode: SandboxMode::WorkspaceWrite,
        sandbox_network_access: true,
        ..Default::default()
    };
    assert!(config.effective_network_access());
    
    let config = RuntimeSecurityConfig {
        sandbox_mode: SandboxMode::WorkspaceWrite,
        sandbox_network_access: false,
        ..Default::default()
    };
    assert!(!config.effective_network_access());
}

#[test]
fn test_requires_explicit_rlm_opt_in() {
    let config = RuntimeSecurityConfig {
        sandbox_mode: SandboxMode::DangerFullAccess,
        ..Default::default()
    };
    assert!(config.requires_explicit_rlm_opt_in());
    
    let config = RuntimeSecurityConfig {
        sandbox_mode: SandboxMode::WorkspaceWrite,
        ..Default::default()
    };
    assert!(!config.requires_explicit_rlm_opt_in());
}

#[test]
fn test_runtime_security_config_clone() {
    let config = RuntimeSecurityConfig::default();
    let cloned = config.clone();
    assert_eq!(config, cloned);
}

#[test]
fn test_approval_policy_equality() {
    assert_eq!(ApprovalPolicy::OnRequest, ApprovalPolicy::OnRequest);
    assert_ne!(ApprovalPolicy::OnRequest, ApprovalPolicy::Never);
    assert_ne!(ApprovalPolicy::OnRequest, ApprovalPolicy::Untrusted);
}

#[test]
fn test_sandbox_mode_equality() {
    assert_eq!(SandboxMode::ReadOnly, SandboxMode::ReadOnly);
    assert_ne!(SandboxMode::ReadOnly, SandboxMode::WorkspaceWrite);
    assert_ne!(SandboxMode::WorkspaceWrite, SandboxMode::DangerFullAccess);
}

#[test]
fn test_approval_policy_serialization() {
    let json = serde_json::to_string(&ApprovalPolicy::OnRequest).unwrap();
    assert_eq!(json, "\"on-request\"");
    
    let json = serde_json::to_string(&ApprovalPolicy::Never).unwrap();
    assert_eq!(json, "\"never\"");
    
    let json = serde_json::to_string(&ApprovalPolicy::Untrusted).unwrap();
    assert_eq!(json, "\"untrusted\"");
}

#[test]
fn test_sandbox_mode_serialization() {
    let json = serde_json::to_string(&SandboxMode::ReadOnly).unwrap();
    assert_eq!(json, "\"read-only\"");
    
    let json = serde_json::to_string(&SandboxMode::WorkspaceWrite).unwrap();
    assert_eq!(json, "\"workspace-write\"");
    
    let json = serde_json::to_string(&SandboxMode::DangerFullAccess).unwrap();
    assert_eq!(json, "\"danger-full-access\"");
}

#[test]
fn test_runtime_security_config_serialization() {
    let config = RuntimeSecurityConfig {
        approval_policy: ApprovalPolicy::Never,
        sandbox_mode: SandboxMode::DangerFullAccess,
        sandbox_network_access: true,
    };
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("never"));
    assert!(json.contains("danger-full-access"));
    assert!(json.contains("sandbox_network_access"));
}
