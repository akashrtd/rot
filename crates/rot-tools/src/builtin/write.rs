//! Write file tool — creates or overwrites files.

use crate::error::ToolError;
use crate::traits::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WriteParams {
    /// File path to write (relative to working directory).
    pub path: String,
    /// Content to write to the file.
    pub content: String,
}

pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }
    fn label(&self) -> &str {
        "Write File"
    }
    fn description(&self) -> &str {
        "Create or overwrite a file with the given content. Creates parent directories if needed."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(WriteParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: WriteParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let path = if Path::new(&params.path).is_absolute() {
            std::path::PathBuf::from(&params.path)
        } else {
            ctx.working_dir.join(&params.path)
        };

        // Create parent directories
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ToolError::ExecutionError(format!("Failed to create directories: {e}"))
            })?;
        }

        let bytes = params.content.len();
        tokio::fs::write(&path, &params.content)
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to write file: {e}")))?;

        let lines = params.content.lines().count();
        Ok(ToolResult::success_with_metadata(
            format!("Wrote {} bytes ({} lines) to {}", bytes, lines, params.path),
            serde_json::json!({"bytes": bytes, "lines": lines}),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_ctx(dir: &TempDir) -> ToolContext {
        ToolContext {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_write_new_file() {
        let dir = TempDir::new().unwrap();
        let result = WriteTool
            .execute(
                serde_json::json!({"path": "hello.txt", "content": "Hello, world!\n"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let content = std::fs::read_to_string(dir.path().join("hello.txt")).unwrap();
        assert_eq!(content, "Hello, world!\n");
    }

    #[tokio::test]
    async fn test_write_creates_dirs() {
        let dir = TempDir::new().unwrap();
        let result = WriteTool
            .execute(
                serde_json::json!({"path": "sub/dir/file.txt", "content": "nested"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let content = std::fs::read_to_string(dir.path().join("sub/dir/file.txt")).unwrap();
        assert_eq!(content, "nested");
    }

    #[tokio::test]
    async fn test_write_overwrites() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), "old").unwrap();

        WriteTool
            .execute(
                serde_json::json!({"path": "f.txt", "content": "new"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        let content = std::fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "new");
    }
}
