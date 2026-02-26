//! Grep tool — content search with regex support.

use crate::error::ToolError;
use crate::path_guard::workspace_root;
use crate::traits::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use ignore::WalkBuilder;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const MAX_RESULTS: usize = 200;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GrepParams {
    /// Regex pattern to search for.
    pub pattern: String,
    /// Optional file glob to filter which files to search (e.g. `*.rs`).
    #[serde(default)]
    pub include: Option<String>,
    /// Number of context lines before a match.
    #[serde(default)]
    pub before_context: Option<usize>,
    /// Number of context lines after a match.
    #[serde(default)]
    pub after_context: Option<usize>,
}

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }
    fn label(&self) -> &str {
        "Grep"
    }
    fn description(&self) -> &str {
        "Search file contents with regex. Supports file filtering and context lines."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(GrepParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: GrepParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;
        let root = workspace_root(&ctx.working_dir)?;

        let re = Regex::new(&params.pattern)
            .map_err(|e| ToolError::InvalidParameters(format!("Invalid regex: {e}")))?;

        let file_filter = params
            .include
            .as_deref()
            .map(glob::Pattern::new)
            .transpose()
            .map_err(|e| ToolError::InvalidParameters(format!("Invalid include pattern: {e}")))?;

        let before = params.before_context.unwrap_or(0);
        let after = params.after_context.unwrap_or(0);

        let walker = WalkBuilder::new(&root)
            .git_ignore(true)
            .hidden(false)
            .build();

        let mut results: Vec<String> = Vec::new();

        for entry in walker.flatten() {
            if results.len() >= MAX_RESULTS {
                break;
            }

            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }

            let path = entry.path();
            let rel = match path.strip_prefix(&root) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let rel_str = rel.to_string_lossy();

            // Apply file filter
            if let Some(ref filter) = file_filter {
                let filename = rel.file_name().map(|f| f.to_string_lossy()).unwrap_or_default();
                if !filter.matches(&filename) && !filter.matches(&rel_str) {
                    continue;
                }
            }

            // Read file (skip binary files)
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let lines: Vec<&str> = content.lines().collect();
            let mut matched_ranges: Vec<(usize, usize)> = Vec::new();

            for (i, line) in lines.iter().enumerate() {
                if re.is_match(line) {
                    let start = i.saturating_sub(before);
                    let end = (i + after + 1).min(lines.len());
                    matched_ranges.push((start, end));
                }
            }

            // Merge overlapping ranges
            let merged = merge_ranges(&matched_ranges);

            for (start, end) in merged {
                for (i, line) in lines.iter().enumerate().take(end).skip(start) {
                    results.push(format!("{}:{}:{}", rel_str, i + 1, line));
                    if results.len() >= MAX_RESULTS {
                        break;
                    }
                }
                if results.len() >= MAX_RESULTS {
                    break;
                }
            }
        }

        let count = results.len();
        let truncated = count >= MAX_RESULTS;
        let mut output = results.join("\n");

        if truncated {
            output.push_str(&format!("\n\n... (truncated at {MAX_RESULTS} results)"));
        }

        if output.is_empty() {
            output = "(no matches)".to_string();
        }

        Ok(ToolResult::success_with_metadata(
            output,
            serde_json::json!({"matches": count, "truncated": truncated}),
        ))
    }
}

fn merge_ranges(ranges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if ranges.is_empty() {
        return Vec::new();
    }
    let mut sorted = ranges.to_vec();
    sorted.sort();
    let mut merged = vec![sorted[0]];
    for &(start, end) in &sorted[1..] {
        let last = merged.last_mut().unwrap();
        if start <= last.1 {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
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
    async fn test_grep_basic() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.rs"), "fn main() {}\nfn helper() {}\nlet x = 1;\n").unwrap();

        let result = GrepTool
            .execute(
                serde_json::json!({"pattern": "fn \\w+"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.output.contains("fn main"));
        assert!(result.output.contains("fn helper"));
        assert!(!result.output.contains("let x"));
    }

    #[tokio::test]
    async fn test_grep_with_filter() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.rs"), "hello rust\n").unwrap();
        fs::write(dir.path().join("b.txt"), "hello text\n").unwrap();

        let result = GrepTool
            .execute(
                serde_json::json!({"pattern": "hello", "include": "*.rs"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.output.contains("a.rs"));
        assert!(!result.output.contains("b.txt"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "nothing here\n").unwrap();

        let result = GrepTool
            .execute(
                serde_json::json!({"pattern": "zzzzz"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.output.contains("no matches"));
    }
}
