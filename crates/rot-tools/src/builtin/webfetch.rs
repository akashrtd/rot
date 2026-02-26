//! WebFetch tool — fetch URL content.

use crate::error::ToolError;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const MAX_BODY_BYTES: usize = 100 * 1024; // 100KB

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebFetchParams {
    /// URL to fetch.
    pub url: String,
}

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "webfetch"
    }
    fn label(&self) -> &str {
        "Web Fetch"
    }
    fn description(&self) -> &str {
        "Fetch the contents of a URL and return the response body as text."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(WebFetchParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        if !ctx.network_access && ctx.sandbox_mode != SandboxMode::DangerFullAccess {
            return Err(ToolError::PermissionDenied(
                "webfetch is disabled because sandbox network access is off".to_string(),
            ));
        }

        let params: WebFetchParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let client = reqwest::Client::builder()
            .timeout(ctx.timeout)
            .user_agent("rot/0.1")
            .build()
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create HTTP client: {e}")))?;

        let response = client
            .get(&params.url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("HTTP request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            return Ok(ToolResult::error(format!(
                "HTTP {status} for {}",
                params.url
            )));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read response: {e}")))?;

        let mut body = String::from_utf8_lossy(&bytes).to_string();
        let total_bytes = body.len();

        if body.len() > MAX_BODY_BYTES {
            body.truncate(MAX_BODY_BYTES);
            body.push_str("\n\n... (truncated at 100KB)");
        }

        Ok(ToolResult::success_with_metadata(
            body,
            serde_json::json!({
                "status": status.as_u16(),
                "content_type": content_type,
                "bytes": total_bytes,
            }),
        ))
    }
}

// Note: WebFetch tests require network access and are kept minimal.
// Integration tests with a mock server would be added separately.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webfetch_schema() {
        let tool = WebFetchTool;
        let schema = tool.parameters_schema();
        assert!(schema.get("properties").is_some());
        assert!(schema["properties"]["url"].is_object());
    }

    #[tokio::test]
    async fn test_webfetch_denied_when_network_disabled() {
        let ctx = ToolContext {
            network_access: false,
            sandbox_mode: SandboxMode::WorkspaceWrite,
            ..Default::default()
        };
        let result = WebFetchTool
            .execute(serde_json::json!({"url":"https://example.com"}), &ctx)
            .await;
        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }
}
