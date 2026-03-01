//! Tool approval policy and per-session approval state.

use crate::security::ApprovalPolicy;

/// Response from the interactive permission prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalResponse {
    /// Allow the tool execution this one time.
    AllowOnce,
    /// Allow this tool execution automatically for the rest of the session.
    AllowAlways,
    /// Deny the tool execution this one time.
    DenyOnce,
    /// Deny this tool execution automatically for the rest of the session.
    DenyAlways,
}

/// The permission system manages auto-approval and denial of tool execution.
#[derive(Clone, Debug)]
pub struct PermissionSystem {
    policy: ApprovalPolicy,
    session_allowed: Vec<String>,
    session_denied: Vec<String>,
}

impl Default for PermissionSystem {
    fn default() -> Self {
        Self::new(ApprovalPolicy::OnRequest)
    }
}

impl PermissionSystem {
    /// Create a permission system using the provided policy.
    pub fn new(policy: ApprovalPolicy) -> Self {
        Self {
            policy,
            session_allowed: Vec::new(),
            session_denied: Vec::new(),
        }
    }

    /// Returns the current approval policy.
    pub fn policy(&self) -> ApprovalPolicy {
        self.policy
    }

    /// Check if a tool needs explicit approval based on the active policy and session state.
    pub fn requires_approval(&self, tool_name: &str) -> bool {
        if self.policy == ApprovalPolicy::Never {
            return false;
        }
        if self.session_allowed.contains(&tool_name.to_string()) {
            return false;
        }
        !is_auto_allowed_by_policy(self.policy, tool_name)
    }

    /// Check if a tool has been permanently denied in this session.
    pub fn is_denied(&self, tool_name: &str) -> bool {
        self.session_denied.contains(&tool_name.to_string())
    }

    /// Update the permission rules based on user response.
    pub fn handle_response(&mut self, tool_name: &str, response: &ApprovalResponse) {
        match response {
            ApprovalResponse::AllowAlways => {
                if !self.session_allowed.contains(&tool_name.to_string()) {
                    self.session_allowed.push(tool_name.to_string());
                }
            }
            ApprovalResponse::DenyAlways => {
                if !self.session_denied.contains(&tool_name.to_string()) {
                    self.session_denied.push(tool_name.to_string());
                }
            }
            ApprovalResponse::AllowOnce | ApprovalResponse::DenyOnce => {}
        }
    }
}

/// Policy matrix for baseline auto-allow behavior.
pub fn is_auto_allowed_by_policy(policy: ApprovalPolicy, tool_name: &str) -> bool {
    match policy {
        ApprovalPolicy::Never => true,
        ApprovalPolicy::Untrusted => matches!(tool_name, "read" | "grep" | "glob"),
        ApprovalPolicy::OnRequest => {
            matches!(tool_name, "read" | "grep" | "glob" | "write" | "edit")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_auto_allowed_by_policy;
    use crate::security::ApprovalPolicy;

    #[test]
    fn test_untrusted_policy_matrix() {
        assert!(is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "read"));
        assert!(is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "grep"));
        assert!(is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "glob"));
        assert!(!is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "write"));
        assert!(!is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "edit"));
        assert!(!is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "bash"));
        assert!(!is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "task"));
        assert!(!is_auto_allowed_by_policy(ApprovalPolicy::Untrusted, "webfetch"));
    }

    #[test]
    fn test_on_request_policy_matrix() {
        assert!(is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "read"));
        assert!(is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "grep"));
        assert!(is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "glob"));
        assert!(is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "write"));
        assert!(is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "edit"));
        assert!(!is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "bash"));
        assert!(!is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "task"));
        assert!(!is_auto_allowed_by_policy(ApprovalPolicy::OnRequest, "webfetch"));
    }

    #[test]
    fn test_never_policy_matrix() {
        for tool in ["read", "grep", "glob", "write", "edit", "bash", "task", "webfetch"] {
            assert!(is_auto_allowed_by_policy(ApprovalPolicy::Never, tool));
        }
    }
}
