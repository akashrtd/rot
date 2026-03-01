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
    
    // First write
    let script = "export FOO=bar\n_rc=$?; echo \"__DELIM__$_rc\" >&1\n";
    match stdin.write_all(script.as_bytes()).await {
        Ok(_) => println!("write 1 ok"),
        Err(e) => println!("write 1 err: {}", e),
    }
    match stdin.flush().await {
        Ok(_) => println!("flush 1 ok"),
        Err(e) => println!("flush 1 err: {}", e),
    }

    // Read until delim
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = stdout_reader.read_line(&mut line).await.unwrap();
        if bytes_read == 0 {
            println!("EOF 1");
            break;
        }
        if line.contains("__DELIM__") {
            println!("DELIM 1: {}", line.trim());
            break;
        }
        print!("OUT 1: {}", line);
    }
    
    // Check if child exited
    if let Ok(Some(status)) = child.try_wait() {
        println!("child exited after 1: {}", status);
    } else {
        println!("child still running after 1");
    }

    // Second write
    let script2 = "echo $FOO\n_rc=$?; echo \"__DELIM__$_rc\" >&1\n";
    match stdin.write_all(script2.as_bytes()).await {
        Ok(_) => println!("write 2 ok"),
        Err(e) => {
            println!("write 2 err: {}", e);
            if let Ok(Some(status)) = child.try_wait() {
                println!("child exited: {}", status);
            }
        }
    }
    match stdin.flush().await {
        Ok(_) => println!("flush 2 ok"),
        Err(e) => println!("flush 2 err: {}", e),
    }

    // Read until delim
    loop {
        line.clear();
        let bytes_read = stdout_reader.read_line(&mut line).await.unwrap();
        if bytes_read == 0 {
            println!("EOF 2");
            break;
        }
        if line.contains("__DELIM__") {
            println!("DELIM 2: {}", line.trim());
            break;
        }
        print!("OUT 2: {}", line);
    }
}
