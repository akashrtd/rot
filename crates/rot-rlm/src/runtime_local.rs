//! Local host runtime process launcher.

use std::collections::HashMap;
use std::path::Path;
use tokio::process::Child;

use rot_sandbox::{SandboxError, SandboxPolicy, spawn_command};

/// Spawn a local subprocess under the configured sandbox policy.
pub fn spawn_local(
    program: &str,
    args: &[String],
    cwd: &Path,
    policy: &SandboxPolicy,
) -> anyhow::Result<Child> {
    spawn_command(program, args, cwd, &HashMap::new(), policy)
        .map_err(map_sandbox_err)
}

fn map_sandbox_err(err: SandboxError) -> anyhow::Error {
    match err {
        SandboxError::BackendUnavailable(msg) => anyhow::anyhow!(
            "RLM local runtime sandbox backend unavailable: {msg}"
        ),
        SandboxError::Timeout(secs) => anyhow::anyhow!(
            "RLM local runtime timed out while spawning process ({secs}s)"
        ),
        SandboxError::Execution(msg) => anyhow::anyhow!(
            "RLM local runtime process spawn failed: {msg}"
        ),
    }
}
