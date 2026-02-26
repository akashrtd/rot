//! OS sandbox abstraction for shell command execution.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

/// Filesystem sandbox mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    /// Read-only filesystem access.
    ReadOnly,
    /// Workspace write access only.
    WorkspaceWrite,
    /// No sandbox restrictions.
    DangerFullAccess,
}

/// Policy options for sandboxed command execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxPolicy {
    /// Filesystem sandbox mode.
    pub mode: SandboxMode,
    /// Whether outbound network access is allowed.
    pub network_access: bool,
}

/// Result from a sandbox command run.
#[derive(Debug, Clone)]
pub struct SandboxRunResult {
    /// Child process stdout.
    pub stdout: Vec<u8>,
    /// Child process stderr.
    pub stderr: Vec<u8>,
    /// Child process exit code. -1 when unavailable.
    pub exit_code: i32,
    /// Whether process exited with success status.
    pub success: bool,
}

/// Errors returned by the sandbox runner.
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    /// Command execution exceeded timeout.
    #[error("Command timed out after {0}s")]
    Timeout(u64),
    /// Sandboxed backend is unavailable.
    #[error("{0}")]
    BackendUnavailable(String),
    /// Process execution failed.
    #[error("Failed to execute command: {0}")]
    Execution(String),
}

/// Execute a shell command under the requested sandbox policy.
pub async fn run_shell_command(
    command: &str,
    cwd: &Path,
    timeout: Duration,
    policy: &SandboxPolicy,
) -> Result<SandboxRunResult, SandboxError> {
    match policy.mode {
        SandboxMode::DangerFullAccess => run_direct(command, cwd, timeout).await,
        SandboxMode::ReadOnly | SandboxMode::WorkspaceWrite => {
            if cfg!(target_os = "macos") {
                run_macos(command, cwd, timeout, policy).await
            } else if cfg!(target_os = "linux") {
                run_linux(command, cwd, timeout, policy).await
            } else {
                Err(SandboxError::BackendUnavailable(
                    "Sandbox backend unavailable on this OS. Use --sandbox danger-full-access to proceed."
                        .to_string(),
                ))
            }
        }
    }
}

async fn run_direct(
    command: &str,
    cwd: &Path,
    timeout: Duration,
) -> Result<SandboxRunResult, SandboxError> {
    let (shell, flag) = shell_and_flag();
    run_with_timeout(
        {
            let mut cmd = Command::new(shell);
            cmd.arg(flag).arg(command).current_dir(cwd);
            cmd
        },
        timeout,
    )
    .await
}

async fn run_macos(
    command: &str,
    cwd: &Path,
    timeout: Duration,
    policy: &SandboxPolicy,
) -> Result<SandboxRunResult, SandboxError> {
    let workspace = cwd
        .canonicalize()
        .map_err(|e| SandboxError::Execution(format!("Cannot resolve workspace: {e}")))?;
    let profile = seatbelt_profile(&workspace, policy);
    let (shell, flag) = shell_and_flag();

    run_with_timeout(
        {
            let mut cmd = Command::new("sandbox-exec");
            cmd.arg("-p")
                .arg(profile)
                .arg(shell)
                .arg(flag)
                .arg(command)
                .current_dir(&workspace);
            cmd
        },
        timeout,
    )
    .await
    .map_err(|e| match e {
        SandboxError::Execution(msg) if msg.contains("No such file or directory") => {
            SandboxError::BackendUnavailable(
                "macOS sandbox backend (sandbox-exec) unavailable. Use --sandbox danger-full-access to proceed."
                    .to_string(),
            )
        }
        other => other,
    })
}

async fn run_linux(
    command: &str,
    cwd: &Path,
    timeout: Duration,
    policy: &SandboxPolicy,
) -> Result<SandboxRunResult, SandboxError> {
    let workspace = cwd
        .canonicalize()
        .map_err(|e| SandboxError::Execution(format!("Cannot resolve workspace: {e}")))?;
    let (shell, flag) = shell_and_flag();

    let mut cmd = Command::new("bwrap");
    cmd.arg("--die-with-parent")
        .arg("--new-session")
        .arg("--proc")
        .arg("/proc")
        .arg("--dev-bind")
        .arg("/dev")
        .arg("/dev")
        .arg("--ro-bind")
        .arg("/")
        .arg("/");

    match policy.mode {
        SandboxMode::ReadOnly => {
            cmd.arg("--ro-bind").arg(&workspace).arg(&workspace);
        }
        SandboxMode::WorkspaceWrite => {
            cmd.arg("--bind").arg(&workspace).arg(&workspace);
        }
        SandboxMode::DangerFullAccess => {}
    }

    if !policy.network_access {
        cmd.arg("--unshare-net");
    }

    cmd.arg("--chdir")
        .arg(&workspace)
        .arg(shell)
        .arg(flag)
        .arg(command);

    run_with_timeout(cmd, timeout).await.map_err(|e| match e {
        SandboxError::Execution(msg) if msg.contains("No such file or directory") => {
            SandboxError::BackendUnavailable(
                "Linux sandbox backend (bubblewrap) unavailable. Use --sandbox danger-full-access to proceed."
                    .to_string(),
            )
        }
        other => other,
    })
}

async fn run_with_timeout(
    mut command: Command,
    timeout: Duration,
) -> Result<SandboxRunResult, SandboxError> {
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| SandboxError::Timeout(timeout.as_secs()))?
        .map_err(|e| SandboxError::Execution(e.to_string()))?;

    Ok(SandboxRunResult {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: output.status.code().unwrap_or(-1),
        success: output.status.success(),
    })
}

fn shell_and_flag() -> (&'static str, &'static str) {
    if cfg!(target_os = "windows") {
        ("cmd", "/C")
    } else {
        ("/bin/bash", "-lc")
    }
}

#[cfg(target_os = "macos")]
fn seatbelt_profile(workspace: &Path, policy: &SandboxPolicy) -> String {
    let workspace = escape_seatbelt_path(workspace);
    let mut profile = String::from(
        "(version 1)\n\
         (deny default)\n\
         (import \"system.sb\")\n\
         (allow process-exec)\n\
         (allow process-fork)\n\
         (allow signal (target self))\n\
         (allow file-read*)\n",
    );

    if policy.mode == SandboxMode::WorkspaceWrite {
        profile.push_str(&format!(
            "(allow file-write* (subpath \"{workspace}\"))\n"
        ));
    }

    if policy.network_access {
        profile.push_str("(allow network*)\n");
    } else {
        profile.push_str("(deny network*)\n");
    }

    profile
}

#[cfg(target_os = "macos")]
fn escape_seatbelt_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(not(target_os = "macos"))]
fn seatbelt_profile(_workspace: &Path, _policy: &SandboxPolicy) -> String {
    String::new()
}

