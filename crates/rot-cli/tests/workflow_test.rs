//! Integration tests for tool workflows.
//!
//! These tests verify tool interactions work correctly.

use std::path::PathBuf;
use std::process::Command;

fn rot_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rot"))
}

#[test]
fn test_exec_read_tool_integration() {
    let dir = tempfile::tempdir().unwrap();
    let test_file = dir.path().join("test.txt");
    std::fs::write(&test_file, "Hello from file").unwrap();

    let output = Command::new(rot_bin())
        .args(["exec", "read the file test.txt", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["I read the file"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn test_exec_write_tool_integration() {
    let dir = tempfile::tempdir().unwrap();

    let output = Command::new(rot_bin())
        .args(["exec", "create a file", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["File created"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_exec_bash_tool_integration() {
    let dir = tempfile::tempdir().unwrap();

    let output = Command::new(rot_bin())
        .args(["exec", "run echo hello", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Command executed"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_exec_grep_tool_integration() {
    let dir = tempfile::tempdir().unwrap();
    let test_file = dir.path().join("code.rs");
    std::fs::write(&test_file, "fn main() { println!(\"hello\"); }").unwrap();

    let output = Command::new(rot_bin())
        .args(["exec", "search for println", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Found println"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_exec_glob_tool_integration() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("file1.rs"), "").unwrap();
    std::fs::write(dir.path().join("file2.rs"), "").unwrap();

    let output = Command::new(rot_bin())
        .args(["exec", "find all rust files", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Found 2 files"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_exec_edit_tool_integration() {
    let dir = tempfile::tempdir().unwrap();
    let test_file = dir.path().join("test.txt");
    std::fs::write(&test_file, "Hello world").unwrap();

    let output = Command::new(rot_bin())
        .args(["exec", "edit the file", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["File edited"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_exec_list_tool_integration() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("file1.txt"), "").unwrap();
    std::fs::create_dir(dir.path().join("subdir")).unwrap();

    let output = Command::new(rot_bin())
        .args(["exec", "list files", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Listed directory"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_exec_multistep_workflow() {
    let dir = tempfile::tempdir().unwrap();
    let test_file = dir.path().join("input.txt");
    std::fs::write(&test_file, "input data").unwrap();

    let output = Command::new(rot_bin())
        .args(["exec", "read input.txt and process it", "--provider", "mock", "--final-json", "--auto-approve"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["Processed successfully"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_exec_with_sandbox_readonly() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "hello", "--provider", "mock", "--final-json", "--sandbox", "read-only"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn test_exec_with_sandbox_workspace_write() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "hello", "--provider", "mock", "--final-json", "--sandbox", "workspace-write"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
}
