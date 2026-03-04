use rot_sandbox::{run_shell_command, SandboxMode, SandboxPolicy};
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_workspace_write_allows_internal_write() {
    let workspace = TempDir::new().unwrap();
    let policy = SandboxPolicy {
        mode: SandboxMode::WorkspaceWrite,
        network_access: false,
    };
    
    let cmd = "echo 'hello' > test.txt";
    let res = run_shell_command(cmd, workspace.path(), Duration::from_secs(5), &policy)
        .await
        .expect("backend should be available");

    assert!(res.success, "stderr: {}", String::from_utf8_lossy(&res.stderr));
    
    let content = std::fs::read_to_string(workspace.path().join("test.txt")).unwrap();
    assert_eq!(content.trim(), "hello");
}

#[tokio::test]
async fn test_workspace_write_blocks_external_write() {
    let workspace = TempDir::new().unwrap();
    let external_dir = TempDir::new().unwrap();
    
    let policy = SandboxPolicy {
        mode: SandboxMode::WorkspaceWrite,
        network_access: false,
    };
    
    let external_file = external_dir.path().join("blocked.txt");
    let cmd = format!("echo 'blocked' > {}", external_file.display());
    
    let res = run_shell_command(&cmd, workspace.path(), Duration::from_secs(5), &policy)
        .await
        .expect("backend should be available");

    // The command should fail to execute because sandbox-exec denies write access
    assert!(!res.success, "expected command to be blocked, but it succeeded");
    assert!(!external_file.exists(), "file was written despite sandbox restrictions");
}

#[tokio::test]
async fn test_read_only_blocks_internal_write() {
    let workspace = TempDir::new().unwrap();
    
    let policy = SandboxPolicy {
        mode: SandboxMode::ReadOnly,
        network_access: false,
    };
    
    let cmd = "echo 'blocked' > test.txt";
    let res = run_shell_command(cmd, workspace.path(), Duration::from_secs(5), &policy)
        .await
        .expect("backend should be available");

    assert!(!res.success, "expected command to be blocked, but it succeeded");
    assert!(!workspace.path().join("test.txt").exists());
}
