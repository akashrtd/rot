//! RLM runtime backend selection.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Child;

use rot_core::{RuntimeSecurityConfig, SandboxMode as CoreSandboxMode};
use rot_sandbox::{SandboxMode, SandboxPolicy};

/// Runtime backend used by RLM execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RlmRuntimeKind {
    /// Python runtime (default).
    Python,
    /// Legacy bash runtime.
    Bash,
}

impl Default for RlmRuntimeKind {
    fn default() -> Self {
        Self::Python
    }
}

/// Process isolation backend for RLM runtime subprocesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RlmIsolationKind {
    /// Use local host subprocesses with OS sandbox policy.
    Local,
    /// Use a Docker container as process boundary.
    Docker,
}

impl Default for RlmIsolationKind {
    fn default() -> Self {
        Self::Local
    }
}

/// Effective process runtime policy used by REPL environments.
#[derive(Debug, Clone)]
pub struct RlmProcessPolicy {
    /// Subprocess isolation mode.
    pub isolation: RlmIsolationKind,
    /// Sandbox policy applied to local execution, and used to derive Docker flags.
    pub sandbox: SandboxPolicy,
    /// Docker image to use when `isolation=docker`.
    pub docker_image: String,
}

impl Default for RlmProcessPolicy {
    fn default() -> Self {
        Self {
            isolation: RlmIsolationKind::Local,
            sandbox: SandboxPolicy {
                mode: SandboxMode::DangerFullAccess,
                network_access: true,
            },
            docker_image: "python:3.11-slim".to_string(),
        }
    }
}

impl RlmProcessPolicy {
    /// Build runtime process policy from effective command security settings.
    pub fn from_security(
        security: &RuntimeSecurityConfig,
        isolation: RlmIsolationKind,
        docker_image: Option<String>,
    ) -> Self {
        let mode = match security.sandbox_mode {
            CoreSandboxMode::ReadOnly => SandboxMode::ReadOnly,
            CoreSandboxMode::WorkspaceWrite => SandboxMode::WorkspaceWrite,
            CoreSandboxMode::DangerFullAccess => SandboxMode::DangerFullAccess,
        };
        let network_access = security.sandbox_network_access
            || security.sandbox_mode == CoreSandboxMode::DangerFullAccess;

        Self {
            isolation,
            sandbox: SandboxPolicy {
                mode,
                network_access,
            },
            docker_image: docker_image.unwrap_or_else(|| "python:3.11-slim".to_string()),
        }
    }
}

/// Spawn a long-lived runtime process under the requested policy.
pub fn spawn_runtime_process(
    program: &str,
    args: &[String],
    cwd: &Path,
    policy: &RlmProcessPolicy,
) -> anyhow::Result<Child> {
    let child = match policy.isolation {
        RlmIsolationKind::Local => crate::runtime_local::spawn_local(program, args, cwd, &policy.sandbox),
        RlmIsolationKind::Docker => crate::runtime_docker::spawn_docker(
            program,
            args,
            cwd,
            &policy.sandbox,
            &policy.docker_image,
        ),
    }?;
    Ok(child)
}

/// Configure common stdio pipe setup for spawned processes.
pub(crate) fn with_piped_stdio(cmd: &mut tokio::process::Command) {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
}
