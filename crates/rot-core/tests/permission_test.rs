use rot_core::permission::{is_auto_allowed_by_policy, ApprovalResponse, PermissionSystem};
use rot_core::security::ApprovalPolicy;

#[test]
fn test_permission_system_default() {
    let ps = PermissionSystem::default();
    assert_eq!(ps.policy(), ApprovalPolicy::OnRequest);
}

#[test]
fn test_permission_system_new() {
    let ps = PermissionSystem::new(ApprovalPolicy::Untrusted);
    assert_eq!(ps.policy(), ApprovalPolicy::Untrusted);
}

#[test]
fn test_permission_system_never_policy() {
    let ps = PermissionSystem::new(ApprovalPolicy::Never);
    assert!(!ps.requires_approval("bash"));
    assert!(!ps.requires_approval("write"));
    assert!(!ps.requires_approval("read"));
}

#[test]
fn test_permission_system_untrusted_read_tools() {
    let ps = PermissionSystem::new(ApprovalPolicy::Untrusted);
    assert!(!ps.requires_approval("read"));
    assert!(!ps.requires_approval("grep"));
    assert!(!ps.requires_approval("glob"));
    assert!(!ps.requires_approval("list"));
}

#[test]
fn test_permission_system_untrusted_write_tools() {
    let ps = PermissionSystem::new(ApprovalPolicy::Untrusted);
    assert!(ps.requires_approval("write"));
    assert!(ps.requires_approval("edit"));
    assert!(ps.requires_approval("bash"));
    assert!(ps.requires_approval("task"));
}

#[test]
fn test_permission_system_on_request_read_tools() {
    let ps = PermissionSystem::new(ApprovalPolicy::OnRequest);
    assert!(!ps.requires_approval("read"));
    assert!(!ps.requires_approval("write"));
    assert!(!ps.requires_approval("edit"));
}

#[test]
fn test_permission_system_on_request_dangerous_tools() {
    let ps = PermissionSystem::new(ApprovalPolicy::OnRequest);
    assert!(ps.requires_approval("bash"));
    assert!(ps.requires_approval("task"));
    assert!(ps.requires_approval("webfetch"));
}

#[test]
fn test_permission_system_allow_once() {
    let mut ps = PermissionSystem::new(ApprovalPolicy::OnRequest);
    ps.handle_response("bash", &ApprovalResponse::AllowOnce);
    assert!(ps.requires_approval("bash"));
}

#[test]
fn test_permission_system_allow_always() {
    let mut ps = PermissionSystem::new(ApprovalPolicy::OnRequest);
    ps.handle_response("bash", &ApprovalResponse::AllowAlways);
    assert!(!ps.requires_approval("bash"));
}

#[test]
fn test_permission_system_deny_once() {
    let mut ps = PermissionSystem::new(ApprovalPolicy::OnRequest);
    ps.handle_response("bash", &ApprovalResponse::DenyOnce);
    assert!(!ps.is_denied("bash"));
    assert!(ps.requires_approval("bash"));
}

#[test]
fn test_permission_system_deny_always() {
    let mut ps = PermissionSystem::new(ApprovalPolicy::OnRequest);
    ps.handle_response("bash", &ApprovalResponse::DenyAlways);
    assert!(ps.is_denied("bash"));
}

#[test]
fn test_permission_system_multiple_allow_always() {
    let mut ps = PermissionSystem::new(ApprovalPolicy::OnRequest);
    ps.handle_response("bash", &ApprovalResponse::AllowAlways);
    ps.handle_response("bash", &ApprovalResponse::AllowAlways);
    assert!(!ps.requires_approval("bash"));
}

#[test]
fn test_approval_response_variants() {
    let allow_once = ApprovalResponse::AllowOnce;
    let allow_always = ApprovalResponse::AllowAlways;
    let deny_once = ApprovalResponse::DenyOnce;
    let deny_always = ApprovalResponse::DenyAlways;
    
    assert_ne!(allow_once, allow_always);
    assert_ne!(deny_once, deny_always);
}

#[test]
fn test_is_auto_allowed_by_policy_never() {
    assert!(is_auto_allowed_by_policy(ApprovalPolicy::Never, "bash"));
    assert!(is_auto_allowed_by_policy(ApprovalPolicy::Never, "write"));
    assert!(is_auto_allowed_by_policy(ApprovalPolicy::Never, "task"));
}

#[test]
fn test_is_auto_allowed_by_policy_untrusted() {
    assert!(is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "read"));
    assert!(is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "grep"));
    assert!(!is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "write"));
    assert!(!is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "bash"));
}

#[test]
fn test_is_auto_allowed_by_policy_on_request() {
    assert!(is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "read"));
    assert!(is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "write"));
    assert!(is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "edit"));
    assert!(!is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "bash"));
    assert!(!is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "task"));
}

#[test]
fn test_permission_system_clone() {
    let ps = PermissionSystem::new(ApprovalPolicy::OnRequest);
    let cloned = ps.clone();
    assert_eq!(ps.policy(), cloned.policy());
}
