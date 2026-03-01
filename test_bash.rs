use std::process::Stdio;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::process::Command;

#[tokio::main]
async fn main() {
    let mut cmd = Command::new("bash");
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    
    match stdin.write_all(b"echo hello\n").await {
        Ok(_) => println!("write 1 ok"),
        Err(e) => println!("write 1 err: {}", e),
    }
    match stdin.flush().await {
        Ok(_) => println!("flush 1 ok"),
        Err(e) => println!("flush 1 err: {}", e),
    }

    let mut stdout = child.stdout.take().unwrap();
    let mut buf = vec![0; 1024];
    match stdout.read(&mut buf).await {
        Ok(n) => println!("out 1: {}", String::from_utf8_lossy(&buf[..n])),
        Err(e) => println!("err 1 out: {}", e),
    }
    
    match stdin.write_all(b"echo world\n").await {
        Ok(_) => println!("write 2 ok"),
        Err(e) => println!("write 2 err: {}", e),
    }
    match stdin.flush().await {
        Ok(_) => println!("flush 2 ok"),
        Err(e) => println!("flush 2 err: {}", e),
    }

    match stdout.read(&mut buf).await {
        Ok(n) => println!("out 2: {}", String::from_utf8_lossy(&buf[..n])),
        Err(e) => println!("err 2 out: {}", e),
    }
}
