//! Glob tool — file pattern matching with .gitignore awareness.

use crate::error::ToolError;
use crate::traits::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use ignore::WalkBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const MAX_RESULTS: usize = 1000;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GlobParams {
    /// Glob pattern (e.g. `**/*.rs`, `src/**/*.ts`).
    pub pattern: String,
}

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }
    fn label(&self) -> &str {
        "Glob"
    }
    fn description(&self) -> &str {
        "Find files matching a glob pattern. Respects .gitignore."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(GlobParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: GlobParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let glob_pattern = glob::Pattern::new(&params.pattern)
            .map_err(|e| ToolError::InvalidParameters(format!("Invalid glob pattern: {e}")))?;

        let walker = WalkBuilder::new(&ctx.working_dir)
            .git_ignore(true)
            .hidden(false)
            .build();

        let mut matches: Vec<String> = Vec::new();
        for entry in walker.flatten() {
            if entry.file_type().is_some_and(|ft| ft.is_file()) {
                if let Ok(rel) = entry.path().strip_prefix(&ctx.working_dir) {
                    let rel_str = rel.to_string_lossy();
                    if glob_pattern.matches(&rel_str) {
                        matches.push(rel_str.to_string());
                        if matches.len() >= MAX_RESULTS {
                            break;
                        }
                    }
                }
            }
        }

        matches.sort();

        let truncated = matches.len() >= MAX_RESULTS;
        let count = matches.len();
        let mut output = matches.join("\n");
        if truncated {
            output.push_str(&format!("\n\n... (truncated at {MAX_RESULTS} results)"));
        }

        if output.is_empty() {
            output = "(no matching files)".to_string();
        }

        Ok(ToolResult::success_with_metadata(
            output,
            serde_json::json!({"count": count, "truncated": truncated}),
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
    async fn test_glob_rs_files() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "").unwrap();
        fs::write(dir.path().join("src/lib.rs"), "").unwrap();
        fs::write(dir.path().join("README.md"), "").unwrap();

        let result = GlobTool
            .execute(
                serde_json::json!({"pattern": "**/*.rs"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.output.contains("main.rs"));
        assert!(result.output.contains("lib.rs"));
        assert!(!result.output.contains("README.md"));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let dir = TempDir::new().unwrap();

        let result = GlobTool
            .execute(
                serde_json::json!({"pattern": "**/*.xyz"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.output.contains("no matching"));
    }
}
