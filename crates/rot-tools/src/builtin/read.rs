//! Read file tool — reads file contents with optional offset/limit.

use crate::error::ToolError;
use crate::path_guard::resolve_existing_path;
use crate::traits::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

const DEFAULT_LIMIT: usize = 2000;
const MAX_OUTPUT_BYTES: usize = 50 * 1024; // 50KB

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReadParams {
    /// File path to read (relative to working directory).
    pub path: String,
    /// Line offset (0-indexed). Default: 0.
    #[serde(default)]
    pub offset: Option<usize>,
    /// Maximum number of lines to return. Default: 2000.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }
    fn label(&self) -> &str {
        "Read File"
    }
    fn description(&self) -> &str {
        "Read the contents of a file. Supports line offset and limit."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(ReadParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: ReadParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let path = resolve_existing_path(Path::new(&params.path), &ctx.working_dir)?;

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read file: {e}")))?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit;

        if offset >= total_lines {
            return Ok(ToolResult::success_with_metadata(
                format!("(empty — offset {offset} exceeds {total_lines} total lines)"),
                serde_json::json!({"total_lines": total_lines}),
            ));
        }

        let end = (offset + limit).min(total_lines);
        let selected: String = lines[offset..end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>4} | {}", offset + i + 1, line))
            .collect::<Vec<_>>()
            .join("\n");

        // Truncate if too large
        let output = if selected.len() > MAX_OUTPUT_BYTES {
            format!(
                "{}\n\n... (truncated at 50KB, showing {end}/{total_lines} lines)",
                &selected[..MAX_OUTPUT_BYTES]
            )
        } else if end < total_lines {
            format!(
                "{selected}\n\n({end}/{total_lines} lines shown, use offset to see more)"
            )
        } else {
            selected
        };

        Ok(ToolResult::success_with_metadata(
            output,
            serde_json::json!({
                "total_lines": total_lines,
                "offset": offset,
                "lines_shown": end - offset,
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn test_ctx(dir: &TempDir) -> ToolContext {
        ToolContext {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_read_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "line1\nline2\nline3\n").unwrap();

        let result = ReadTool
            .execute(serde_json::json!({"path": "test.txt"}), &test_ctx(&dir))
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.output.contains("line1"));
        assert!(result.output.contains("line2"));
        assert!(result.output.contains("line3"));
    }

    #[tokio::test]
    async fn test_read_with_offset() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "a\nb\nc\nd\ne\n").unwrap();

        let result = ReadTool
            .execute(
                serde_json::json!({"path": "test.txt", "offset": 2, "limit": 2}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.output.contains("c"));
        assert!(result.output.contains("d"));
        assert!(!result.output.contains("   1 |"));
    }

    #[tokio::test]
    async fn test_read_nonexistent() {
        let dir = TempDir::new().unwrap();
        let result = ReadTool
            .execute(
                serde_json::json!({"path": "nope.txt"}),
                &test_ctx(&dir),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_path_traversal_blocked() {
        let dir = TempDir::new().unwrap();
        let result = ReadTool
            .execute(
                serde_json::json!({"path": "../../../etc/passwd"}),
                &test_ctx(&dir),
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::PermissionDenied(_) | ToolError::ExecutionError(_) => {}
            other => panic!("Expected PermissionDenied, got: {other:?}"),
        }
    }
}
