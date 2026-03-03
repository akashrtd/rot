use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

fn run_rot(args: &[&str], cwd: &Path, responses: &[&str]) -> Output {
    let mock = serde_json::to_string(&responses).unwrap();
    Command::new(rot_bin())
        .args(args)
        .current_dir(cwd)
        .env("ROT_MOCK_RESPONSES", mock)
        .output()
        .unwrap()
}

#[cfg(unix)]
fn create_fake_pdftotext(bin_dir: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let bin = bin_dir.join("pdftotext");
    std::fs::write(
        &bin,
        "#!/bin/sh\nout=\"$2\"\nprintf 'pdf extracted text\\n' > \"$out\"\n",
    )
    .unwrap();
    let mut perms = std::fs::metadata(&bin).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&bin, perms).unwrap();
}

#[cfg(windows)]
fn create_fake_pdftotext(bin_dir: &Path) {
    let bin = bin_dir.join("pdftotext.bat");
    std::fs::write(&bin, "@echo off\r\nset OUT=%2\r\necho pdf extracted text>%OUT%\r\n").unwrap();
}

#[test]
fn test_exec_rlm_json_mode_text_context() {
    if !python_available() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let ctx = dir.path().join("ctx.txt");
    std::fs::write(&ctx, "hello rlm").unwrap();

    let output = run_rot(
        &[
            "exec",
            "summarize",
            "--provider",
            "mock",
            "--rlm",
            "--context",
            ctx.to_str().unwrap(),
            "--json",
        ],
        dir.path(),
        &["```repl\nFINAL('text ok')\n```"],
    );

    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8(output.stdout).unwrap();
    let events = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(events[0]["type"], "session_start");
    let final_event = events
        .iter()
        .find(|e| e["type"] == "final")
        .expect("final event missing");
    assert_eq!(final_event["status"], "ok");
    assert_eq!(final_event["final_text"], "text ok");
    assert!(final_event["usage"]["input_tokens"].as_u64().unwrap() > 0);
    assert!(final_event["usage"]["output_tokens"].as_u64().unwrap() > 0);
}

#[test]
fn test_exec_rlm_final_json_and_schema_validation() {
    if !python_available() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let ctx = dir.path().join("ctx.txt");
    std::fs::write(&ctx, "hello schema").unwrap();

    let schema = dir.path().join("schema.json");
    std::fs::write(
        &schema,
        r#"{ "type": "object", "required": ["answer"], "properties": { "answer": {"type":"string"} } }"#,
    )
    .unwrap();

    let ok = run_rot(
        &[
            "exec",
            "schema",
            "--provider",
            "mock",
            "--rlm",
            "--context",
            ctx.to_str().unwrap(),
            "--final-json",
            "--output-schema",
            schema.to_str().unwrap(),
        ],
        dir.path(),
        &["```repl\nFINAL('{\"answer\":\"ok\"}')\n```"],
    );

    assert!(ok.status.success(), "stderr={}", String::from_utf8_lossy(&ok.stderr));
    let ok_json: Value = serde_json::from_slice(&ok.stdout).unwrap();
    assert_eq!(ok_json["status"], "ok");
    assert_eq!(ok_json["final_text"], "{\"answer\":\"ok\"}");

    let fail = run_rot(
        &[
            "exec",
            "schema",
            "--provider",
            "mock",
            "--rlm",
            "--context",
            ctx.to_str().unwrap(),
            "--final-json",
            "--output-schema",
            schema.to_str().unwrap(),
        ],
        dir.path(),
        &["```repl\nFINAL('not-json')\n```"],
    );

    assert_eq!(fail.status.code(), Some(2));
    let fail_json: Value = serde_json::from_slice(&fail.stdout).unwrap();
    assert_eq!(fail_json["status"], "error");
    assert!(fail_json["error"]
        .as_str()
        .unwrap_or_default()
        .contains("not valid JSON"));
}

#[test]
fn test_exec_rlm_pdf_context_without_live_credentials() {
    if !python_available() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let bin_dir = dir.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    create_fake_pdftotext(&bin_dir);

    let pdf = dir.path().join("ctx.pdf");
    std::fs::write(&pdf, b"%PDF-1.4\n%fixture\n").unwrap();

    let mock = serde_json::to_string(&["```repl\nFINAL('pdf ok')\n```"]).unwrap();
    let old_path = std::env::var("PATH").unwrap_or_default();
    let sep = if cfg!(windows) { ";" } else { ":" };
    let merged_path = format!("{}{}{}", bin_dir.display(), sep, old_path);
    let output = Command::new(rot_bin())
        .args([
            "exec",
            "pdf",
            "--provider",
            "mock",
            "--rlm",
            "--context",
            pdf.to_str().unwrap(),
            "--final-json",
        ])
        .current_dir(dir.path())
        .env("ROT_MOCK_RESPONSES", mock)
        .env("PATH", merged_path)
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    let out_json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(out_json["status"], "ok");
    assert_eq!(out_json["final_text"], "pdf ok");
}
