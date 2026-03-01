//! MCP stdio client for loading and calling external tools.

use rot_sandbox::{spawn_command, SandboxError, SandboxPolicy};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

/// Configuration for a stdio MCP server process.
#[derive(Debug, Clone)]
pub struct StdioServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: PathBuf,
    pub startup_timeout: Duration,
    pub tool_timeout: Duration,
    pub policy: SandboxPolicy,
}

/// Metadata returned by an MCP server for one tool.
#[derive(Debug, Clone)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Normalized result returned from an MCP tool call.
#[derive(Debug, Clone)]
pub struct McpToolCallResult {
    pub text: String,
    pub structured_content: Option<Value>,
    pub raw_content: Vec<Value>,
    pub is_error: bool,
}

/// stdio-backed MCP client.
#[derive(Debug, Clone)]
pub struct McpClient {
    inner: Arc<Mutex<Connection>>,
}

#[derive(Debug)]
struct Connection {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_request_id: u64,
    server_name: String,
    tool_timeout: Duration,
}

/// Errors returned by the MCP client.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("MCP server '{0}' closed the stdio connection")]
    ConnectionClosed(String),
    #[error("MCP server '{server}' returned protocol error {code}: {message}")]
    ServerError {
        server: String,
        code: i64,
        message: String,
    },
    #[error("MCP server '{server}' sent invalid response: {message}")]
    InvalidResponse { server: String, message: String },
    #[error("MCP request to server '{server}' timed out after {seconds}s")]
    Timeout { server: String, seconds: u64 },
    #[error("{0}")]
    Sandbox(String),
    #[error("Failed to spawn MCP server '{server}': {message}")]
    Spawn { server: String, message: String },
    #[error("IO error while talking to MCP server '{server}': {message}")]
    Io { server: String, message: String },
    #[error("JSON error while talking to MCP server '{server}': {message}")]
    Json { server: String, message: String },
}

impl McpClient {
    /// Spawn a stdio MCP server, initialize it, and return discovered tools.
    pub async fn connect(
        config: StdioServerConfig,
    ) -> Result<(Self, Vec<McpToolInfo>), McpError> {
        let mut child = spawn_command(
            &config.command,
            &config.args,
            &config.cwd,
            &config.env,
            &config.policy,
        )
        .map_err(|err| map_sandbox_error(&config.name, err))?;

        let stdin = child.stdin.take().ok_or_else(|| McpError::Spawn {
            server: config.name.clone(),
            message: "child stdin was not piped".to_string(),
        })?;
        let stdout = child.stdout.take().ok_or_else(|| McpError::Spawn {
            server: config.name.clone(),
            message: "child stdout was not piped".to_string(),
        })?;
        if let Some(stderr) = child.stderr.take() {
            let server_name = config.name.clone();
            tokio::spawn(async move {
                let mut stderr = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match stderr.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            tracing::debug!(server = %server_name, stderr = line.trim_end(), "mcp stderr");
                        }
                        Err(err) => {
                            tracing::debug!(server = %server_name, error = %err, "failed to read mcp stderr");
                            break;
                        }
                    }
                }
            });
        }

        let client = Self {
            inner: Arc::new(Mutex::new(Connection {
                child,
                stdin,
                stdout: BufReader::new(stdout),
                next_request_id: 1,
                server_name: config.name.clone(),
                tool_timeout: config.tool_timeout,
            })),
        };

        client.initialize(config.startup_timeout).await?;
        let tools = client.list_tools(config.startup_timeout).await?;
        Ok((client, tools))
    }

    /// Call one tool on the connected MCP server.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<McpToolCallResult, McpError> {
        let timeout = {
            let conn = self.inner.lock().await;
            conn.tool_timeout
        };
        let result = self
            .request_with_timeout(
                "tools/call",
                serde_json::json!({
                    "name": name,
                    "arguments": arguments,
                }),
                timeout,
            )
            .await?;

        let content = result
            .get("content")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let structured_content = result.get("structuredContent").cloned();
        let is_error = result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        Ok(McpToolCallResult {
            text: render_content_text(&content, structured_content.as_ref()),
            structured_content,
            raw_content: content,
            is_error,
        })
    }

    async fn initialize(&self, timeout: Duration) -> Result<(), McpError> {
        let _ = self
            .request_with_timeout(
                "initialize",
                serde_json::json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "rot",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                }),
                timeout,
            )
            .await?;
        self.notify("notifications/initialized", serde_json::json!({}))
            .await?;
        Ok(())
    }

    async fn list_tools(&self, timeout: Duration) -> Result<Vec<McpToolInfo>, McpError> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        let server_name = self.server_name().await;
        loop {
            let params = match &cursor {
                Some(cursor) => serde_json::json!({ "cursor": cursor }),
                None => serde_json::json!({}),
            };
            let result = self.request_with_timeout("tools/list", params, timeout).await?;
            let listed = result
                .get("tools")
                .and_then(Value::as_array)
                .ok_or_else(|| McpError::InvalidResponse {
                    server: server_name.clone(),
                    message: "tools/list result missing tools array".to_string(),
                })?;

            for tool in listed {
                tools.push(parse_tool_info(&server_name, tool)?);
            }

            cursor = result
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            if cursor.is_none() {
                break;
            }
        }

        Ok(tools)
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), McpError> {
        let mut conn = self.inner.lock().await;
        let server_name = conn.server_name.clone();
        let payload = serde_json::to_vec(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
        .map_err(|err| McpError::Json {
            server: server_name.clone(),
            message: err.to_string(),
        })?;
        conn.stdin
            .write_all(&payload)
            .await
            .map_err(|err| McpError::Io {
                server: server_name.clone(),
                message: err.to_string(),
            })?;
        conn.stdin.write_all(b"\n").await.map_err(|err| McpError::Io {
            server: server_name.clone(),
            message: err.to_string(),
        })?;
        conn.stdin.flush().await.map_err(|err| McpError::Io {
            server: server_name,
            message: err.to_string(),
        })?;
        Ok(())
    }

    async fn request_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, McpError> {
        let mut conn = self.inner.lock().await;
        let request_id = conn.next_request_id;
        conn.next_request_id += 1;
        let server_name = conn.server_name.clone();
        let payload = serde_json::to_vec(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params,
        }))
        .map_err(|err| McpError::Json {
            server: server_name.clone(),
            message: err.to_string(),
        })?;
        conn.stdin
            .write_all(&payload)
            .await
            .map_err(|err| McpError::Io {
                server: server_name.clone(),
                message: err.to_string(),
            })?;
        conn.stdin.write_all(b"\n").await.map_err(|err| McpError::Io {
            server: server_name.clone(),
            message: err.to_string(),
        })?;
        conn.stdin.flush().await.map_err(|err| McpError::Io {
            server: server_name.clone(),
            message: err.to_string(),
        })?;

        loop {
            let message = tokio::time::timeout(timeout, read_message(&mut conn.stdout, &server_name))
                .await
                .map_err(|_| McpError::Timeout {
                    server: server_name.clone(),
                    seconds: timeout.as_secs(),
                })??;

            if let Some(method) = message.get("method").and_then(Value::as_str) {
                if let Some(id) = message.get("id").cloned() {
                    let response = serde_json::to_vec(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32601,
                            "message": format!("rot client does not implement MCP client method '{method}'"),
                        }
                    }))
                    .map_err(|err| McpError::Json {
                        server: server_name.clone(),
                        message: err.to_string(),
                    })?;
                    conn.stdin.write_all(&response).await.map_err(|err| McpError::Io {
                        server: server_name.clone(),
                        message: err.to_string(),
                    })?;
                    conn.stdin.write_all(b"\n").await.map_err(|err| McpError::Io {
                        server: server_name.clone(),
                        message: err.to_string(),
                    })?;
                    conn.stdin.flush().await.map_err(|err| McpError::Io {
                        server: server_name.clone(),
                        message: err.to_string(),
                    })?;
                }
                continue;
            }

            let response_id = message.get("id").and_then(Value::as_u64);
            if response_id != Some(request_id) {
                continue;
            }

            if let Some(err) = message.get("error") {
                return Err(McpError::ServerError {
                    server: server_name,
                    code: err.get("code").and_then(Value::as_i64).unwrap_or(-32000),
                    message: err
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown error")
                        .to_string(),
                });
            }

            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    async fn server_name(&self) -> String {
        let conn = self.inner.lock().await;
        conn.server_name.clone()
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if let Ok(mut conn) = self.inner.try_lock() {
            let _ = conn.child.start_kill();
        }
    }
}

fn map_sandbox_error(server: &str, err: SandboxError) -> McpError {
    match err {
        SandboxError::BackendUnavailable(message) => McpError::Sandbox(message),
        SandboxError::Execution(message) => McpError::Spawn {
            server: server.to_string(),
            message,
        },
        SandboxError::Timeout(seconds) => McpError::Timeout {
            server: server.to_string(),
            seconds,
        },
    }
}

async fn read_message(
    stdout: &mut BufReader<ChildStdout>,
    server_name: &str,
) -> Result<Value, McpError> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = stdout
            .read_line(&mut line)
            .await
            .map_err(|err| McpError::Io {
                server: server_name.to_string(),
                message: err.to_string(),
            })?;
        if bytes == 0 {
            return Err(McpError::ConnectionClosed(server_name.to_string()));
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return serde_json::from_str(trimmed).map_err(|err| McpError::Json {
            server: server_name.to_string(),
            message: err.to_string(),
        });
    }
}

fn parse_tool_info(server_name: &str, value: &Value) -> Result<McpToolInfo, McpError> {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| McpError::InvalidResponse {
            server: server_name.to_string(),
            message: "tool entry missing name".to_string(),
        })?;
    let input_schema = value
        .get("inputSchema")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({"type":"object","additionalProperties":true}));
    Ok(McpToolInfo {
        name: name.to_string(),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("MCP tool")
            .to_string(),
        input_schema,
    })
}

fn render_content_text(content: &[Value], structured_content: Option<&Value>) -> String {
    let mut parts = Vec::new();
    for item in content {
        match item.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    parts.push(text.to_string());
                }
            }
            Some("resource") => {
                if let Some(text) = item
                    .get("resource")
                    .and_then(|v| v.get("text"))
                    .and_then(Value::as_str)
                {
                    parts.push(text.to_string());
                } else {
                    parts.push("[resource content omitted]".to_string());
                }
            }
            Some(kind) => parts.push(format!("[{kind} content omitted]")),
            None => {}
        }
    }

    if parts.is_empty() {
        if let Some(structured_content) = structured_content {
            return serde_json::to_string_pretty(structured_content)
                .unwrap_or_else(|_| structured_content.to_string());
        }
        return "(no output)".to_string();
    }

    parts.join("\n")
}
