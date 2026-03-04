use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn rot_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rot"))
}

#[test]
fn test_exec_auto_approve_flag_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(rot_bin())
        .args([
            "exec",
            "hello",
            "--provider",
            "mock",
            "--auto-approve",
            "--final-json",
        ])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["auto-approve works"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["final_text"], "auto-approve works");
}

#[test]
fn test_exec_approve_list_flag_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(rot_bin())
        .args([
            "exec",
            "hello",
            "--provider",
            "mock",
            "--approve-list",
            "read,write,bash",
            "--final-json",
        ])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["approve-list works"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["final_text"], "approve-list works");
}

#[test]
fn test_exec_both_flags_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(rot_bin())
        .args([
            "exec",
            "hello",
            "--provider",
            "mock",
            "--auto-approve",
            "--approve-list",
            "read",
            "--final-json",
        ])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["both flags work"]).unwrap(),
        )
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["final_text"], "both flags work");
}
