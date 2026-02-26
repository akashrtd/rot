//! Edit file tool — surgical string replacement in files.

use crate::error::ToolError;
use crate::path_guard::resolve_path_for_write;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditParams {
    /// File path to edit (relative to working directory).
    pub path: String,
    /// The exact string to find in the file.
    pub old_string: String,
    /// The string to replace it with.
    pub new_string: String,
    /// If true, replace ALL occurrences. Default: false.
    #[serde(default)]
    pub replace_all: bool,
}

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }
    fn label(&self) -> &str {
        "Edit File"
    }
    fn description(&self) -> &str {
        "Edit a file by replacing an exact string match. Fails if the string is not found \
         or appears multiple times without replace_all=true."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(EditParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        if ctx.sandbox_mode == SandboxMode::ReadOnly {
            return Err(ToolError::PermissionDenied(
                "edit is disabled in read-only sandbox mode".to_string(),
            ));
        }

        let params: EditParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let path = if ctx.sandbox_mode == SandboxMode::WorkspaceWrite {
            resolve_path_for_write(Path::new(&params.path), &ctx.working_dir)?
        } else if Path::new(&params.path).is_absolute() {
            std::path::PathBuf::from(&params.path)
        } else {
            ctx.working_dir.join(&params.path)
        };

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read file: {e}")))?;

        let count = content.matches(&params.old_string).count();

        if count == 0 {
            return Err(ToolError::ExecutionError(
                "old_string not found in file".to_string(),
            ));
        }

        if count > 1 && !params.replace_all {
            return Err(ToolError::ExecutionError(format!(
                "old_string found {count} times. Use replace_all=true to replace all occurrences."
            )));
        }

        let new_content = if params.replace_all {
            content.replace(&params.old_string, &params.new_string)
        } else {
            content.replacen(&params.old_string, &params.new_string, 1)
        };

        tokio::fs::write(&path, &new_content)
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to write file: {e}")))?;

        Ok(ToolResult::success(format!(
            "Replaced {count} occurrence(s) in {}",
            params.path
        )))
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
    async fn test_edit_single_replace() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), "hello world").unwrap();

        let result = EditTool
            .execute(
                serde_json::json!({
                    "path": "f.txt",
                    "old_string": "hello",
                    "new_string": "goodbye"
                }),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let content = std::fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "goodbye world");
    }

    #[tokio::test]
    async fn test_edit_replace_all() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), "aaa bbb aaa").unwrap();

        let result = EditTool
            .execute(
                serde_json::json!({
                    "path": "f.txt",
                    "old_string": "aaa",
                    "new_string": "ccc",
                    "replace_all": true
                }),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let content = std::fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "ccc bbb ccc");
    }

    #[tokio::test]
    async fn test_edit_not_found() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), "hello world").unwrap();

        let result = EditTool
            .execute(
                serde_json::json!({
                    "path": "f.txt",
                    "old_string": "missing",
                    "new_string": "x"
                }),
                &test_ctx(&dir),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_edit_multiple_without_flag() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), "aaa bbb aaa").unwrap();

        let result = EditTool
            .execute(
                serde_json::json!({
                    "path": "f.txt",
                    "old_string": "aaa",
                    "new_string": "ccc"
                }),
                &test_ctx(&dir),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_edit_denied_in_read_only_mode() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), "hello").unwrap();
        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
            sandbox_mode: SandboxMode::ReadOnly,
            ..Default::default()
        };

        let result = EditTool
            .execute(
                serde_json::json!({
                    "path":"f.txt",
                    "old_string":"hello",
                    "new_string":"world"
                }),
                &ctx,
            )
            .await;

        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn test_edit_workspace_write_blocks_outside_workspace() {
        let dir = TempDir::new().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_path = outside.path().join("outside.txt");
        std::fs::write(&outside_path, "hello").unwrap();
        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
            sandbox_mode: SandboxMode::WorkspaceWrite,
            ..Default::default()
        };

        let result = EditTool
            .execute(
                serde_json::json!({
                    "path": outside_path.display().to_string(),
                    "old_string":"hello",
                    "new_string":"world"
                }),
                &ctx,
            )
            .await;

        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }
}
