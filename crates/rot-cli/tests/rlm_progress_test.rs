use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn rot_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rot"))
}

fn python_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn test_rlm_progress_emitted_in_json_mode() {
    if !python_available() {
        eprintln!("Skipping: python3 not available");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let ctx = dir.path().join("ctx.txt");
    std::fs::write(&ctx, "test context").unwrap();

    let output = Command::new(rot_bin())
        .args([
            "exec",
            "process",
            "--provider",
            "mock",
            "--rlm",
            "--context",
            ctx.to_str().unwrap(),
            "--json",
            "--allow-unsafe-rlm",
        ])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["```repl\nFINAL('done')\n```"]).unwrap(),
        )
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        stderr,
        String::from_utf8_lossy(&output.stdout)
    );

    let has_progress = stderr.lines().any(|line| {
        if let Ok(event) = serde_json::from_str::<Value>(line) {
            event["type"] == "progress"
        } else {
            false
        }
    });

    assert!(has_progress, "Expected progress events in stderr: {}", stderr);
}

#[test]
fn test_rlm_progress_emitted_in_human_mode() {
    if !python_available() {
        eprintln!("Skipping: python3 not available");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let ctx = dir.path().join("ctx.txt");
    std::fs::write(&ctx, "test context").unwrap();

    let output = Command::new(rot_bin())
        .args([
            "exec",
            "process",
            "--provider",
            "mock",
            "--rlm",
            "--context",
            ctx.to_str().unwrap(),
            "--allow-unsafe-rlm",
        ])
        .current_dir(dir.path())
        .env(
            "ROT_MOCK_RESPONSES",
            serde_json::to_string(&["```repl\nFINAL('done')\n```"]).unwrap(),
        )
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        stderr,
        String::from_utf8_lossy(&output.stdout)
    );

    let has_progress = stderr.lines().any(|line| line.contains("[RLM]"));

    assert!(has_progress, "Expected [RLM] progress in stderr: {}", stderr);
}
