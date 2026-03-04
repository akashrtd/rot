use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn rot_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rot"))
}

fn gather_jsonl_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                    out.push(path);
                }
            }
        }
    }

    out
}

fn wait_for_session_contents(
    sessions_root: &Path,
    expected_substrings: &[&str],
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        let files = gather_jsonl_files(sessions_root);
        if !files.is_empty() {
            for file in &files {
                if let Ok(content) = fs::read_to_string(file) {
                    if expected_substrings.iter().all(|needle| content.contains(needle)) {
                        return Ok(());
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    let files = gather_jsonl_files(sessions_root);
    let debug_dump = files
        .iter()
        .filter_map(|file| fs::read_to_string(file).ok().map(|c| (file, c)))
        .map(|(file, content)| format!("file={}\n{}", file.display(), content))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    Err(format!(
        "timed out waiting for session content under {}. expected={:?}\n{}",
        sessions_root.display(),
        expected_substrings,
        debug_dump
    ))
}

#[test]
fn test_chat_send_receive_and_persist_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let data_home = temp.path().join("xdg-data");
    fs::create_dir_all(&data_home).expect("create xdg-data");

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(rot_bin());
    cmd.arg("chat");
    cmd.arg("--provider");
    cmd.arg("mock");
    cmd.cwd(temp.path());
    cmd.env("ROT_MOCK_RESPONSES", r#"["hello from mock provider"]"#);
    cmd.env("MOCK_API_KEY", "test-key");
    cmd.env("HOME", temp.path());
    cmd.env("XDG_DATA_HOME", &data_home);

    let mut child = pair.slave.spawn_command(cmd).expect("spawn chat");
    drop(pair.slave);

    let output = Arc::new(Mutex::new(String::new()));
    let output_clone = Arc::clone(&output);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let drain_thread = thread::spawn(move || {
        let mut buf = [0_u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]);
                    let mut out = output_clone.lock().expect("lock output");
                    out.push_str(&chunk);
                    if out.len() > 500_000 {
                        let to_trim = out.len() - 500_000;
                        out.drain(..to_trim);
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut writer = pair.master.take_writer().expect("take writer");

    // Wait for session file creation (chat runner creates one immediately at startup).
    // Search under temp root because dirs::data_dir() location can vary by platform.
    let sessions_root = temp.path();
    let created = wait_for_session_contents(
        sessions_root,
        &["\"type\":\"session/start\""],
        Duration::from_secs(10),
    );
    if let Err(err) = created {
        let captured = output.lock().expect("lock output").clone();
        panic!("{}\noutput:\n{}", err, captured);
    }

    // Send one message and expect the mock assistant response to be persisted.
    writer
        .write_all(b"/rlm\r")
        .expect("disable rlm in chat");
    writer.flush().expect("flush rlm toggle");
    thread::sleep(Duration::from_millis(250));

    writer
        .write_all(b"hello from tui\r")
        .expect("write prompt to pty");
    writer.flush().expect("flush prompt");

    let persisted = wait_for_session_contents(
        sessions_root,
        &["hello from tui", "hello from mock provider"],
        Duration::from_secs(15),
    );
    if let Err(err) = persisted {
        let captured = output.lock().expect("lock output").clone();
        panic!("{}\noutput:\n{}", err, captured);
    }

    // Exit the TUI. Ctrl+C should trigger graceful shutdown.
    let _ = writer.write_all(&[3]);
    let _ = writer.flush();
    drop(writer);

    thread::sleep(Duration::from_millis(300));
    let _ = child.kill();
    let _ = child.wait();

    drop(pair.master);
    let _ = drain_thread.join();

    let files = gather_jsonl_files(sessions_root);
    assert!(!files.is_empty(), "expected persisted session file");
}
