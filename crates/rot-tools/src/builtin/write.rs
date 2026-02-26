//! Write file tool — creates or overwrites files.

use crate::error::ToolError;
use crate::path_guard::resolve_path_for_write;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
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
        if ctx.sandbox_mode == SandboxMode::ReadOnly {
            return Err(ToolError::PermissionDenied(
                "write is disabled in read-only sandbox mode".to_string(),
            ));
        }

        let params: WriteParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let path = if ctx.sandbox_mode == SandboxMode::WorkspaceWrite {
            resolve_path_for_write(Path::new(&params.path), &ctx.working_dir)?
        } else if Path::new(&params.path).is_absolute() {
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

    #[tokio::test]
    async fn test_write_denied_in_read_only_mode() {
        let dir = TempDir::new().unwrap();
        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
            sandbox_mode: SandboxMode::ReadOnly,
            ..Default::default()
        };

        let result = WriteTool
            .execute(
                serde_json::json!({"path":"x.txt","content":"x"}),
                &ctx,
            )
            .await;
        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn test_write_workspace_write_blocks_outside_workspace() {
        let dir = TempDir::new().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_path = outside.path().join("blocked.txt");
        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
            sandbox_mode: SandboxMode::WorkspaceWrite,
            ..Default::default()
        };

        let result = WriteTool
            .execute(
                serde_json::json!({"path": outside_path.display().to_string(), "content":"x"}),
                &ctx,
            )
            .await;
        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }
}
