/// Response from the interactive permission prompt
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalResponse {
    /// Allow the tool execution this one time
    AllowOnce,
    /// Allow this tool execution automatically for the rest of the session
    AllowAlways,
    /// Deny the tool execution this one time
    DenyOnce,
    /// Deny this tool execution automatically for the rest of the session
    DenyAlways,
}

/// The permission system manages auto-approval and denial of tool execution.
pub struct PermissionSystem {
    /// Tools that are always allowed unconditionally
    always_allow: Vec<String>,
    /// Tools that have been permanently allowed for this session
    session_allowed: Vec<String>,
    /// Tools that have been permanently denied for this session
    session_denied: Vec<String>,
}

impl Default for PermissionSystem {
    fn default() -> Self {
        Self {
            // These tools are considered "safe" and run without prompting
            always_allow: vec![
                "read".to_string(),
                "grep".to_string(),
                "glob".to_string(),
                "webfetch".to_string(),
            ],
            session_allowed: Vec::new(),
            session_denied: Vec::new(),
        }
    }
}

impl PermissionSystem {
    /// Check if a tool needs explicit approval based on existing rules
    pub fn requires_approval(&self, tool_name: &str) -> bool {
        if self.always_allow.contains(&tool_name.to_string()) {
            return false;
        }
        if self.session_allowed.contains(&tool_name.to_string()) {
            return false;
        }
        true // Anything else defaults to asking
    }

    /// Check if a tool has been permanently denied
    pub fn is_denied(&self, tool_name: &str) -> bool {
        self.session_denied.contains(&tool_name.to_string())
    }

    /// Update the permission rules based on user response
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
            // Once responses don't modify permanent rules
            _ => {}
        }
    }
}
