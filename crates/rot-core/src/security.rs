//! Runtime security configuration and policy types.

use serde::{Deserialize, Serialize};

/// Approval behavior for tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalPolicy {
    /// Auto-allow only very low-risk read/search tools.
    Untrusted,
    /// Auto-allow workspace file edits, but prompt for shell/network tools.
    OnRequest,
    /// Never prompt; run with fully non-interactive approvals.
    Never,
}

impl Default for ApprovalPolicy {
    fn default() -> Self {
        Self::OnRequest
    }
}

/// Filesystem sandbox mode for tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    /// Read-only workspace access.
    ReadOnly,
    /// Allow reads everywhere but writes only within workspace.
    WorkspaceWrite,
    /// No sandbox restrictions.
    DangerFullAccess,
}

impl Default for SandboxMode {
    fn default() -> Self {
        Self::WorkspaceWrite
    }
}

/// Effective runtime security options after CLI/config resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSecurityConfig {
    /// Approval behavior for tool execution.
    pub approval_policy: ApprovalPolicy,
    /// Filesystem sandbox mode.
    pub sandbox_mode: SandboxMode,
    /// Whether outbound network access is permitted.
    pub sandbox_network_access: bool,
}

impl Default for RuntimeSecurityConfig {
    fn default() -> Self {
        Self {
            approval_policy: ApprovalPolicy::default(),
            sandbox_mode: SandboxMode::default(),
            sandbox_network_access: false,
        }
    }
}
