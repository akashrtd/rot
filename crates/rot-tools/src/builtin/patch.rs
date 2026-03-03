//! Patch tool — deterministic multi-hunk text replacement in files.

use crate::error::ToolError;
use crate::path_guard::resolve_existing_path;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One deterministic replacement hunk.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PatchHunk {
    /// Exact text to replace.
    pub old_string: String,
    /// Replacement text.
    pub new_string: String,
    /// Replace all matches for this hunk. If false, exactly one match is required.
    #[serde(default)]
    pub replace_all: bool,
}

/// Parameters for the `patch` tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PatchParams {
    /// File path relative to workspace (or absolute in danger mode).
    pub path: String,
    /// Ordered hunks to apply.
    pub hunks: Vec<PatchHunk>,
    /// If true, allow hunks that do not match anything.
    #[serde(default)]
    pub allow_noop: bool,
}

pub struct PatchTool;

#[async_trait]
impl Tool for PatchTool {
    fn name(&self) -> &str {
        "patch"
    }

    fn label(&self) -> &str {
        "Patch File"
    }

    fn description(&self) -> &str {
        "Apply deterministic exact-text hunks to a file in a single operation."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(PatchParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        if ctx.sandbox_mode == SandboxMode::ReadOnly {
            return Err(ToolError::PermissionDenied(
                "patch is disabled in read-only sandbox mode".to_string(),
            ));
        }

        let params: PatchParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        if params.hunks.is_empty() {
            return Err(ToolError::InvalidParameters(
                "hunks must not be empty".to_string(),
            ));
        }

        let path = resolve_target_path(&params.path, ctx)?;
        let original = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read file: {e}")))?;

        let mut patched = original.clone();
        let mut replacements = 0usize;

        for (idx, hunk) in params.hunks.iter().enumerate() {
            if hunk.old_string.is_empty() {
                return Err(ToolError::InvalidParameters(format!(
                    "hunk {} has empty old_string",
                    idx + 1
                )));
            }

            let match_count = patched.matches(&hunk.old_string).count();
            if match_count == 0 {
                if params.allow_noop {
                    continue;
                }
                return Err(ToolError::ExecutionError(format!(
                    "hunk {} did not match any content",
                    idx + 1
                )));
            }

            if match_count > 1 && !hunk.replace_all {
                return Err(ToolError::ExecutionError(format!(
                    "hunk {} matched {} times; set replace_all=true for this hunk",
                    idx + 1,
                    match_count
                )));
            }

            patched = if hunk.replace_all {
                replacements += match_count;
                patched.replace(&hunk.old_string, &hunk.new_string)
            } else {
                replacements += 1;
                patched.replacen(&hunk.old_string, &hunk.new_string, 1)
            };
        }

        let changed = patched != original;
        if changed {
            tokio::fs::write(&path, patched)
                .await
                .map_err(|e| ToolError::ExecutionError(format!("Failed to write file: {e}")))?;
        }

        Ok(ToolResult::success_with_metadata(
            format!(
                "Applied {} hunks ({} replacement(s)) to {}",
                params.hunks.len(),
                replacements,
                params.path
            ),
            serde_json::json!({
                "hunks": params.hunks.len(),
                "replacements": replacements,
                "changed": changed,
            }),
        ))
    }
}

fn resolve_target_path(path: &str, ctx: &ToolContext) -> Result<std::path::PathBuf, ToolError> {
    if ctx.sandbox_mode == SandboxMode::DangerFullAccess {
        let raw = Path::new(path);
        let full = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            ctx.working_dir.join(raw)
        };
        return Ok(full);
    }

    resolve_existing_path(Path::new(path), &ctx.working_dir)
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
    async fn test_patch_multiple_hunks() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("f.txt");
        std::fs::write(&path, "hello world\ncount=1\n").unwrap();

        let result = PatchTool
            .execute(
                serde_json::json!({
                    "path": "f.txt",
                    "hunks": [
                        {"old_string": "hello", "new_string": "goodbye"},
                        {"old_string": "count=1", "new_string": "count=2"}
                    ]
                }),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let new_content = std::fs::read_to_string(path).unwrap();
        assert!(new_content.contains("goodbye world"));
        assert!(new_content.contains("count=2"));
    }

    #[tokio::test]
    async fn test_patch_ambiguous_without_replace_all_fails() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), "x x x").unwrap();

        let result = PatchTool
            .execute(
                serde_json::json!({
                    "path": "f.txt",
                    "hunks": [{"old_string":"x","new_string":"y"}]
                }),
                &test_ctx(&dir),
            )
            .await;

        assert!(matches!(result, Err(ToolError::ExecutionError(_))));
    }

    #[tokio::test]
    async fn test_patch_denied_in_read_only_mode() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), "abc").unwrap();
        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
            sandbox_mode: SandboxMode::ReadOnly,
            ..Default::default()
        };

        let result = PatchTool
            .execute(
                serde_json::json!({
                    "path":"f.txt",
                    "hunks":[{"old_string":"a","new_string":"b"}]
                }),
                &ctx,
            )
            .await;

        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }
}
