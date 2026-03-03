//! Docker-isolated runtime process launcher.

use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};

use rot_sandbox::{SandboxMode, SandboxPolicy};

/// Spawn a subprocess inside a Docker container.
pub fn spawn_docker(
    program: &str,
    args: &[String],
    cwd: &Path,
    policy: &SandboxPolicy,
    image: &str,
) -> anyhow::Result<Child> {
    if !docker_available() {
        return Err(anyhow::anyhow!(
            "RLM docker runtime unavailable: `docker` is not installed or not on PATH"
        ));
    }

    if !policy.network_access && !docker_image_exists(image) {
        return Err(anyhow::anyhow!(
            "RLM docker runtime requires image '{image}' to be pre-pulled when network is disabled"
        ));
    }

    let workspace = cwd.canonicalize().map_err(|e| {
        anyhow::anyhow!("RLM docker runtime cannot resolve workspace '{}': {e}", cwd.display())
    })?;

    let mount_mode = match policy.mode {
        SandboxMode::ReadOnly => "ro",
        SandboxMode::WorkspaceWrite | SandboxMode::DangerFullAccess => "rw",
    };
    let mount_arg = format!("{}:/workspace:{mount_mode}", workspace.display());

    let mut docker_args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "-i".to_string(),
        "--workdir".to_string(),
        "/workspace".to_string(),
        "-v".to_string(),
        mount_arg,
    ];

    if !policy.network_access {
        docker_args.push("--network".to_string());
        docker_args.push("none".to_string());
    }

    docker_args.push(image.to_string());
    docker_args.push(program.to_string());
    docker_args.extend(args.iter().cloned());

    let mut cmd = Command::new("docker");
    cmd.args(&docker_args).current_dir(&workspace);
    crate::runtime::with_piped_stdio(&mut cmd);

    cmd.spawn().map_err(|e| {
        anyhow::anyhow!("RLM docker runtime failed to spawn container process: {e}")
    })
}

fn docker_available() -> bool {
    std::process::Command::new("docker")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn docker_image_exists(image: &str) -> bool {
    std::process::Command::new("docker")
        .args(["image", "inspect", image])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
