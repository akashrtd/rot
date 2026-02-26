use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, ChildStderr, Command};

pub struct ReplResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub variables: HashMap<String, String>,
}

pub struct ReplEnv {
    working_dir: PathBuf,
    temp_dir: PathBuf,
    variables: HashMap<String, String>,
    
    // Process handles
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    stderr: Option<BufReader<ChildStderr>>,
}

impl ReplEnv {
    pub fn new() -> Self {
        let temp_dir = std::env::temp_dir().join(format!("rot-repl-{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&temp_dir).ok();

        Self {
            working_dir: std::env::current_dir().unwrap_or_default(),
            temp_dir,
            variables: HashMap::new(),
            process: None,
            stdin: None,
            stdout: None,
            stderr: None,
        }
    }

    pub async fn init(&mut self, context_path: &str) -> anyhow::Result<()> {
        // Start bash shell for REPL
        let mut cmd = Command::new("/bin/bash");
        cmd.args(["--noprofile", "--norc"])
            .current_dir(&self.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        
        let stdin = child.stdin.take().expect("Failed to open stdin");
        let stdout = child.stdout.take().expect("Failed to open stdout");
        let stderr = child.stderr.take().expect("Failed to open stderr");

        self.process = Some(child);
        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.stderr = Some(BufReader::new(stderr));

        // Generate and execute setup script inside the bash shell
        let setup_script = self.generate_setup_script(context_path);
        let setup_path = self.temp_dir.join("setup.sh");
        tokio::fs::write(&setup_path, &setup_script).await?;
        
        // Execute the setup script internally
        let init_code = format!("source {}\n", setup_path.display());
        self.execute(&init_code).await?;

        Ok(())
    }

    pub async fn execute(&mut self, code: &str) -> anyhow::Result<ReplResult> {
        let stdin = self.stdin.as_mut().expect("REPL not initialized");
        let stdout = self.stdout.as_mut().expect("REPL not initialized");
        let stderr = self.stderr.as_mut().expect("REPL not initialized");

        let delim = format!("__ROT_DELIM_{}__", ulid::Ulid::new());
        
        let cmd_path = self.temp_dir.join(format!("cmd_{}.sh", ulid::Ulid::new()));
        tokio::fs::write(&cmd_path, code).await?;
        
        // Write the block of code, followed by our custom delimiter which prints to stdout
        // We capture the exit code of the last statement run before the delimiter
        let script = format!(
            "source \"{}\"\n_rc=$?; echo \"{}{}$_rc\" >&1; echo \"{}{}$_rc\" >&2\n", 
            cmd_path.display(), delim, delim, delim, delim
        );
        
        if let Err(e) = stdin.write_all(script.as_bytes()).await {
            let mut status_msg = "Unknown status".to_string();
            if let Ok(Some(status)) = self.process.as_mut().unwrap().try_wait() {
                status_msg = format!("Exited with {}", status);
            }
            // If the write failed, the child likely exited. Let's see if we can read stderr.
            let mut buf = String::new();
            use tokio::io::AsyncReadExt;
            let _ = self.stderr.as_mut().unwrap().read_to_string(&mut buf).await;
            return Err(anyhow::anyhow!("Write to REPL failed (Broken Pipe).\nBash Status: {}\nBash stderr: {}\nOriginal error: {}", status_msg, buf, e));
        }
        
        if let Err(e) = stdin.flush().await {
            return Err(anyhow::anyhow!("Flush to REPL failed: {}", e));
        }

        // Read stdout and stderr concurrently until they hit the delimiter
        let (out_res, err_res) = tokio::join!(
            read_until_delim(stdout, &delim),
            read_until_delim(stderr, &delim)
        );

        let (mut stdout_str, stdout_rc) = out_res?;
        let (stderr_str, _) = err_res?;

        self.extract_variables(&stdout_str);

        // Strip any LLM queries or variables out of stdout since they're intercepted 
        // We might need to refine this based on how the RLM operates
        stdout_str = stdout_str
            .lines()
            .filter(|l| !l.starts_with("VAR_SET:") && !l.starts_with("LLM_QUERY:") && !l.starts_with("RLM_QUERY:"))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ReplResult {
            stdout: stdout_str,
            stderr: stderr_str,
            exit_code: stdout_rc,
            variables: self.variables.clone(),
        })
    }

    pub fn get_var(&self, name: &str) -> Option<String> {
        self.variables.get(name).cloned()
    }

    fn generate_setup_script(&self, context_path: &str) -> String {
        format!(r##"
# Context file path
export CONTEXT_FILE="{context_path}"

# Helper to get context
get_context() {{
    cat "$CONTEXT_FILE"
}}

# Context preview (first 1000 chars)
context_preview() {{
    head -c 1000 "$CONTEXT_FILE"
}}

# Context length
context_length() {{
    wc -c < "$CONTEXT_FILE"
}}

# LLM query function (calls back to rot)
llm_query() {{
    local prompt="$1"
    echo "LLM_QUERY:$prompt:END_QUERY"
}}

# Final answer
FINAL() {{
    echo "FINAL_ANSWER:$1"
}}

# Show variables
SHOW_VARS() {{
    echo "AVAILABLE_VARS:$(compgen -v | grep -vE '^(BASH.*|HOSTNAME|PWD|SHLVL|_)$' | tr '\n' ',')"
}}"##, context_path = context_path)
    }

    fn extract_variables(&mut self, output: &str) {
        for line in output.lines() {
            if line.starts_with("VAR_SET:") {
                let parts: Vec<&str> = line[8..].splitn(2, ':').collect();
                if parts.len() == 2 {
                    self.variables.insert(parts[0].to_string(), parts[1].to_string());
                }
            }
        }
    }
}

impl Drop for ReplEnv {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.start_kill();
        }
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
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
            break; // EOF
        }

        // Check if the delim is in the line twice
        let search_delim = format!("{}{}", delim, delim);
        if line.contains(&search_delim) {
            // Found delimiter line. Extract exit code.
            let parts: Vec<&str> = line.trim_end().split(&search_delim).collect();
            if parts.len() == 2 {
                if let Ok(code) = parts[1].parse::<i32>() {
                    exit_code = Some(code);
                }
            }
            break;
        }

        output.push_str(&line);
    }

    Ok((output, exit_code))
}

#[cfg(test)]
include!("repl_tests.rs");
