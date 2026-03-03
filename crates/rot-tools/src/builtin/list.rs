//! List tool — list files and directories without shelling out.

use crate::error::ToolError;
use crate::path_guard::resolve_existing_path;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const DEFAULT_MAX_ENTRIES: usize = 200;
const HARD_MAX_ENTRIES: usize = 2000;

fn default_path() -> String {
    ".".to_string()
}

fn default_max_entries() -> usize {
    DEFAULT_MAX_ENTRIES
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListParams {
    /// Path to list. Defaults to current directory.
    #[serde(default = "default_path")]
    pub path: String,
    /// Include files/directories whose names start with '.'.
    #[serde(default)]
    pub include_hidden: bool,
    /// List recursively.
    #[serde(default)]
    pub recursive: bool,
    /// Maximum entries to return.
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}

pub struct ListTool;

#[derive(Debug, Clone)]
struct ListedEntry {
    rel_path: String,
    is_dir: bool,
    size_bytes: Option<u64>,
}

#[async_trait]
impl Tool for ListTool {
    fn name(&self) -> &str {
        "list"
    }

    fn label(&self) -> &str {
        "List Directory"
    }

    fn description(&self) -> &str {
        "List files and directories with optional recursion and hidden-file filtering."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(ListParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: ListParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let max_entries = params.max_entries.clamp(1, HARD_MAX_ENTRIES);
        let root = resolve_list_root(&params.path, ctx)?;

        let entries = collect_entries(&root, params.recursive, params.include_hidden, max_entries)
            .await?;

        let mut entries = entries;
        entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

        let mut lines = Vec::with_capacity(entries.len());
        for entry in &entries {
            let kind = if entry.is_dir { "dir " } else { "file" };
            let size = entry
                .size_bytes
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string());
            lines.push(format!("{kind:>4} {size:>10} {}", entry.rel_path));
        }

        let output = if lines.is_empty() {
            "(no entries)".to_string()
        } else {
            lines.join("\n")
        };

        let dir_count = entries.iter().filter(|entry| entry.is_dir).count();
        let file_count = entries.len().saturating_sub(dir_count);

        Ok(ToolResult::success_with_metadata(
            output,
            serde_json::json!({
                "count": entries.len(),
                "files": file_count,
                "directories": dir_count,
                "recursive": params.recursive,
                "include_hidden": params.include_hidden,
                "max_entries": max_entries,
                "path": root.display().to_string(),
            }),
        ))
    }
}

fn resolve_list_root(path: &str, ctx: &ToolContext) -> Result<PathBuf, ToolError> {
    match ctx.sandbox_mode {
        SandboxMode::WorkspaceWrite | SandboxMode::ReadOnly => {
            resolve_existing_path(Path::new(path), &ctx.working_dir)
        }
        SandboxMode::DangerFullAccess => {
            let raw = Path::new(path);
            let full = if raw.is_absolute() {
                raw.to_path_buf()
            } else {
                ctx.working_dir.join(raw)
            };
            full.canonicalize().map_err(|e| {
                ToolError::ExecutionError(format!("Failed to resolve list path '{}': {e}", path))
            })
        }
    }
}

async fn collect_entries(
    root: &Path,
    recursive: bool,
    include_hidden: bool,
    max_entries: usize,
) -> Result<Vec<ListedEntry>, ToolError> {
    let mut queue = vec![root.to_path_buf()];
    let mut entries = Vec::new();

    while let Some(dir) = queue.pop() {
        let mut read_dir = tokio::fs::read_dir(&dir)
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to list '{}': {e}", dir.display())))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read directory entry: {e}")))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            if !include_hidden && name.starts_with('.') {
                continue;
            }

            let path = entry.path();
            let file_type = entry
                .file_type()
                .await
                .map_err(|e| ToolError::ExecutionError(format!("Failed to inspect '{}': {e}", path.display())))?;

            let rel_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            if file_type.is_dir() {
                entries.push(ListedEntry {
                    rel_path: format!("{rel_path}/"),
                    is_dir: true,
                    size_bytes: None,
                });

                if recursive {
                    queue.push(path);
                }
            } else if file_type.is_file() {
                let size_bytes = entry
                    .metadata()
                    .await
                    .ok()
                    .map(|meta| meta.len());
                entries.push(ListedEntry {
                    rel_path,
                    is_dir: false,
                    size_bytes,
                });
            }

            if entries.len() >= max_entries {
                return Ok(entries);
            }
        }
    }

    Ok(entries)
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
    async fn test_list_non_recursive() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\n").unwrap();

        let result = ListTool
            .execute(serde_json::json!({"path":"."}), &test_ctx(&dir))
            .await
            .unwrap();

        assert!(result.output.contains("Cargo.toml"));
        assert!(result.output.contains("src/"));
    }

    #[tokio::test]
    async fn test_list_recursive() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/b/file.txt"), "x").unwrap();

        let result = ListTool
            .execute(
                serde_json::json!({"path":".","recursive":true}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.output.contains("a/"));
        assert!(result.output.contains("a/b/"));
        assert!(result.output.contains("a/b/file.txt"));
    }

    #[tokio::test]
    async fn test_list_hidden_filtered_by_default() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".secret"), "x").unwrap();

        let result = ListTool
            .execute(serde_json::json!({"path":"."}), &test_ctx(&dir))
            .await
            .unwrap();

        assert!(!result.output.contains(".secret"));
    }

    #[tokio::test]
    async fn test_list_workspace_guard() {
        let dir = TempDir::new().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
            sandbox_mode: SandboxMode::WorkspaceWrite,
            ..Default::default()
        };

        let result = ListTool
            .execute(
                serde_json::json!({"path": outside.path().display().to_string()}),
                &ctx,
            )
            .await;

        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }
}
