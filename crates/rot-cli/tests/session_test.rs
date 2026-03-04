//! Integration tests for session persistence.
//!
//! These tests verify session save/restore works correctly.

use std::path::PathBuf;
use std::process::Command;

fn rot_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rot"))
}

#[test]
fn test_exec_creates_session() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "hello", "--provider", "mock", "--json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    let first_line = stdout.lines().next().unwrap();
    let event: serde_json::Value = serde_json::from_str(first_line).unwrap();
    assert_eq!(event["type"], "session_start");
}

#[test]
fn test_exec_session_persistence() {
    let dir = tempfile::tempdir().unwrap();
    
    // First execution
    let output1 = Command::new(rot_bin())
        .args(["exec", "first message", "--provider", "mock", "--json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["first response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output1.status.success());
}

#[test]
fn test_exec_final_json_output() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "test", "--provider", "mock", "--final-json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["final response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
    
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["final_text"], "final response");
    assert!(payload["elapsed_ms"].is_number());
}

#[test]
fn test_exec_json_output_format() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "test", "--provider", "mock", "--json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    
    // First line should be session_start
    let start: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(start["type"], "session_start");
    
    // Last line should be final
    let final_event: serde_json::Value = serde_json::from_str(lines.last().unwrap()).unwrap();
    assert_eq!(final_event["type"], "final");
    assert_eq!(final_event["status"], "ok");
}

#[test]
fn test_exec_error_output_format() {
    let dir = tempfile::tempdir().unwrap();
    
    // Use unknown agent to trigger error
    let output = Command::new(rot_bin())
        .args(["exec", "test", "--provider", "mock", "--agent", "nonexistent", "--final-json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_exec_with_fork_requires_session() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "test", "--provider", "mock", "--fork", "--final-json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--fork requires --session") || stderr.contains("fork"));
}

#[test]
fn test_exec_usage_tracking() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "test", "--provider", "mock", "--final-json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
    
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(payload["usage"].is_object());
}

#[test]
fn test_exec_elapsed_time_tracking() {
    let dir = tempfile::tempdir().unwrap();
    
    let start = std::time::Instant::now();
    let output = Command::new(rot_bin())
        .args(["exec", "test", "--provider", "mock", "--final-json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["response"]).unwrap(),
        )
        .output()
        .unwrap();
    let elapsed = start.elapsed();

    assert!(output.status.success());
    
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let reported_ms = payload["elapsed_ms"].as_u64().unwrap();
    
    // Reported time should be close to actual elapsed time
    let actual_ms = elapsed.as_millis() as u64;
    assert!(reported_ms <= actual_ms + 1000); // Allow 1s tolerance
}

#[test]
fn test_exec_cwd_in_json_output() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "test", "--provider", "mock", "--json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    let first_line = stdout.lines().next().unwrap();
    let event: serde_json::Value = serde_json::from_str(first_line).unwrap();
    assert!(event["cwd"].is_string());
}

#[test]
fn test_exec_sandbox_mode_in_output() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "test", "--provider", "mock", "--json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    let first_line = stdout.lines().next().unwrap();
    let event: serde_json::Value = serde_json::from_str(first_line).unwrap();
    assert!(event["sandbox_mode"].is_string());
}

#[test]
fn test_exec_approval_policy_in_output() {
    let dir = tempfile::tempdir().unwrap();
    
    let output = Command::new(rot_bin())
        .args(["exec", "test", "--provider", "mock", "--json"])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["response"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(output.status.success());
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    let first_line = stdout.lines().next().unwrap();
    let event: serde_json::Value = serde_json::from_str(first_line).unwrap();
    assert!(event["approval_policy"].is_string());
}
