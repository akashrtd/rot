//! Python runtime for RLM iterations.

use crate::repl::ReplResult;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

/// Persistent Python REPL environment backed by a custom stdin protocol.
pub struct PythonReplEnv {
    working_dir: PathBuf,
    temp_dir: PathBuf,
    delim: String,
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    stderr: Option<BufReader<ChildStderr>>,
}

impl Default for PythonReplEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonReplEnv {
    /// Create a new Python REPL environment.
    pub fn new() -> Self {
        let temp_dir = std::env::temp_dir().join(format!("rot-py-repl-{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&temp_dir).ok();
        Self {
            working_dir: std::env::current_dir().unwrap_or_default(),
            temp_dir,
            delim: format!("__ROT_PY_DELIM_{}__", ulid::Ulid::new()),
            process: None,
            stdin: None,
            stdout: None,
            stderr: None,
        }
    }

    /// Initialize Python process and helper functions.
    pub async fn init(&mut self, context_path: &str) -> anyhow::Result<()> {
        let mut cmd = Command::new("python3");
        cmd.args(["-u", "-c", &harness_script(&self.delim)])
            .current_dir(&self.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn python3: {e}"))?;

        self.stdin = Some(
            child
                .stdin
                .take()
                .ok_or_else(|| anyhow::anyhow!("python stdin unavailable"))?,
        );
        self.stdout = Some(BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("python stdout unavailable"))?,
        ));
        self.stderr = Some(BufReader::new(
            child
                .stderr
                .take()
                .ok_or_else(|| anyhow::anyhow!("python stderr unavailable"))?,
        ));
        self.process = Some(child);

        let setup = self.generate_setup_script(context_path);
        self.execute(&setup).await?;
        Ok(())
    }

    /// Execute Python code while preserving interpreter state.
    pub async fn execute(&mut self, code: &str) -> anyhow::Result<ReplResult> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("python repl not initialized"))?;
        let stdout = self
            .stdout
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("python repl not initialized"))?;
        let stderr = self
            .stderr
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("python repl not initialized"))?;

        let payload = format!("{code}\n{}\n", self.delim);
        stdin.write_all(payload.as_bytes()).await?;
        stdin.flush().await?;

        let (out_res, err_res) = tokio::time::timeout(Duration::from_secs(30), async {
            tokio::join!(
                read_until_delim(stdout, &self.delim),
                read_until_delim(stderr, &self.delim)
            )
        })
        .await
        .map_err(|_| anyhow::anyhow!("Python runtime execution timed out waiting for delimiter"))?;

        let (stdout_str, exit_code) = out_res?;
        let (stderr_str, _) = err_res?;

        Ok(ReplResult {
            stdout: stdout_str,
            stderr: stderr_str,
            exit_code,
            variables: HashMap::new(),
        })
    }

    fn generate_setup_script(&self, context_path: &str) -> String {
        format!(
            r#"
import json
from pathlib import Path

CONTEXT_FILE = r"{context_path}"
CONTEXT = Path(CONTEXT_FILE).read_text(encoding="utf-8")

def context_preview():
    return CONTEXT[:1000]

def context_length():
    return len(CONTEXT)

def context_slice(start, end):
    return CONTEXT[start:end]

def context_find(pattern):
    return CONTEXT.find(pattern)

def context_chunks(size, overlap=0):
    chunks = []
    i = 0
    while i < len(CONTEXT):
        chunks.append(CONTEXT[i:i+size])
        step = max(1, size - max(0, overlap))
        i += step
    return chunks

def SUBLM(query, text_or_var=None):
    payload = {{"query": str(query)}}
    if text_or_var is not None:
        if isinstance(text_or_var, str) and text_or_var in globals():
            payload["input"] = str(globals()[text_or_var])
            payload["input_ref"] = text_or_var
        else:
            payload["input"] = str(text_or_var)
    print("__ROT_SUBLM__" + json.dumps(payload, ensure_ascii=False))

def llm_query(prompt):
    SUBLM(prompt)

def FINAL(text):
    print(f"FINAL_ANSWER:{{text}}")

def FINAL_VAR(name):
    if name in globals():
        print("FINAL_VAR_ANSWER:" + json.dumps(globals()[name], default=str))
    else:
        print("FINAL_VAR_ANSWER:null")
"#
        )
    }
}

impl Drop for PythonReplEnv {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.start_kill();
        }
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

fn harness_script(delim: &str) -> String {
    format!(
        r#"
import traceback
import sys

DELIM = {delim:?}
globals_dict = {{}}
buffer = []

def run_code(src: str):
    exit_code = 0
    try:
        exec(src, globals_dict, globals_dict)
    except Exception:
        traceback.print_exc()
        exit_code = 1
    print(f"{{DELIM}}{{exit_code}}", flush=True)
    print(f"{{DELIM}}{{exit_code}}", file=sys.stderr, flush=True)

for line in sys.stdin:
    if line.rstrip('\n') == DELIM:
        src = ''.join(buffer)
        buffer = []
        run_code(src)
    else:
        buffer.append(line)
"#
    )
}

async fn read_until_delim<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    delim: &str,
) -> anyhow::Result<(String, Option<i32>)> {
    let mut output = String::new();
    let mut exit_code = None;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }

        let trimmed = line.trim_end();
        if let Some(rest) = trimmed.strip_prefix(delim) {
            if let Ok(code) = rest.parse::<i32>() {
                exit_code = Some(code);
            }
            break;
        }

        output.push_str(&line);
    }

    Ok((output, exit_code))
}

#[cfg(test)]
mod tests {
    use super::PythonReplEnv;

    #[tokio::test]
    async fn test_python_repl_state_and_final_var() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let ctx = dir.path().join("ctx.txt");
        std::fs::write(&ctx, "abcdef").unwrap();

        let mut repl = PythonReplEnv::new();
        repl.init(ctx.to_string_lossy().as_ref()).await.unwrap();

        let first = repl.execute("x = context_slice(0, 3)").await.unwrap();
        assert_eq!(first.exit_code, Some(0));

        let second = repl.execute("FINAL_VAR('x')").await.unwrap();
        assert!(second.stdout.contains("FINAL_VAR_ANSWER"));
        assert!(second.stdout.contains("\"abc\""));
    }
}
