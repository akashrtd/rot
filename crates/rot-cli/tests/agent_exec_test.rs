use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn rot_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rot"))
}

#[test]
fn test_exec_with_agent_flag_and_mock_provider() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(rot_bin())
        .args([
            "exec",
            "hello",
            "--provider",
            "mock",
            "--agent",
            "plan",
            "--final-json",
        ])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["agent ok"]).unwrap(),
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
    assert_eq!(payload["final_text"], "agent ok");
}
