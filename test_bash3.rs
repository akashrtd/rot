use std::process::Stdio;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[tokio::main]
async fn main() {
    let mut cmd = Command::new("bash");
    cmd.args(["--noprofile", "--norc"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        
    let mut child = cmd.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let mut stdout_reader = BufReader::new(stdout);
    let mut stderr_reader = BufReader::new(stderr);
    
    // Mimic the exact setup script injection
    let context_path = "crates/rot-cli/src/main.rs";
    let setup_script = format!(r##"
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
}}
"##, context_path = context_path);

    let setup_path = std::env::temp_dir().join("rot-test-setup.sh");
    tokio::fs::write(&setup_path, &setup_script).await.unwrap();
    
    // Execute the setup script internally
    let init_code = format!("source {}\n", setup_path.display());
    
    let delim = format!("__ROT_DELIM_test__");
    let script = format!(
        "{}\n_rc=$?; echo \"{}{}$_rc\" >&1; echo \"{}{}$_rc\" >&2\n", 
        init_code, delim, delim, delim, delim
    );
        
    if let Err(e) = stdin.write_all(script.as_bytes()).await {
        println!("WRITE ERR: {}", e);
        if let Ok(Some(status)) = child.try_wait() {
            println!("Child exited: {}", status);
        }
        return;
    }
    stdin.flush().await.unwrap();

    println!("Write succeeded, waiting for delim...");

    // Read until delim
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = stdout_reader.read_line(&mut line).await.unwrap();
        if bytes_read == 0 {
            println!("EOF stdout");
            break;
        }
        if line.contains("__ROT_DELIM_test__") {
            println!("DELIM stdout: {}", line.trim());
            break;
        }
        print!("OUT: {}", line);
    }
}
