//! CodeSearch tool — code-aware text and symbol search without shelling out.

use crate::error::ToolError;
use crate::path_guard::resolve_existing_path;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use ignore::WalkBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const DEFAULT_MAX_RESULTS: usize = 20;
const MAX_RESULTS_CAP: usize = 200;
const MAX_FILE_BYTES: u64 = 1024 * 1024; // 1MB

fn default_path() -> String {
    ".".to_string()
}

fn default_max_results() -> usize {
    DEFAULT_MAX_RESULTS
}

/// Parameters for code-aware search.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CodeSearchParams {
    /// Query string to search for.
    pub query: String,
    /// Root path to search from.
    #[serde(default = "default_path")]
    pub path: String,
    /// Optional glob include filter (e.g. `*.rs`, `src/**/*.ts`).
    #[serde(default)]
    pub include: Option<String>,
    /// Maximum number of ranked files to return.
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    /// Number of context lines before each match.
    #[serde(default)]
    pub before_context: usize,
    /// Number of context lines after each match.
    #[serde(default)]
    pub after_context: usize,
    /// Preserve case in matching.
    #[serde(default)]
    pub case_sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CodeSearchHit {
    /// Relative file path from the search root.
    pub path: String,
    /// Ranking score.
    pub score: usize,
    /// Number of matched lines.
    pub matches: usize,
    /// Snippet lines (`path:line:text`).
    pub snippets: Vec<String>,
}

pub struct CodeSearchTool;

#[async_trait]
impl Tool for CodeSearchTool {
    fn name(&self) -> &str {
        "codesearch"
    }

    fn label(&self) -> &str {
        "Code Search"
    }

    fn description(&self) -> &str {
        "Search code with ranked file matches and contextual snippets."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(CodeSearchParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: CodeSearchParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let max_results = params.max_results.clamp(1, MAX_RESULTS_CAP);
        let root = resolve_search_root(&params.path, ctx)?;

        let hits = run_codesearch(
            &root,
            &params.query,
            params.include.as_deref(),
            max_results,
            params.before_context,
            params.after_context,
            params.case_sensitive,
        )?;

        let output = format_hits(&hits);
        Ok(ToolResult::success_with_metadata(
            output,
            serde_json::json!({
                "query": params.query,
                "count": hits.len(),
                "max_results": max_results,
                "path": root.display().to_string(),
                "hits": hits,
            }),
        ))
    }
}

pub(crate) fn run_codesearch(
    root: &Path,
    query: &str,
    include: Option<&str>,
    max_results: usize,
    before_context: usize,
    after_context: usize,
    case_sensitive: bool,
) -> Result<Vec<CodeSearchHit>, ToolError> {
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return Err(ToolError::InvalidParameters(
            "query must not be empty".to_string(),
        ));
    }

    let include_pattern = include
        .map(glob::Pattern::new)
        .transpose()
        .map_err(|e| ToolError::InvalidParameters(format!("Invalid include pattern: {e}")))?;

    let q = if case_sensitive {
        trimmed_query.to_string()
    } else {
        trimmed_query.to_lowercase()
    };
    let tokens: Vec<String> = q
        .split_whitespace()
        .map(|t| t.to_string())
        .collect();

    let walker = WalkBuilder::new(root)
        .git_ignore(true)
        .hidden(false)
        .build();

    let mut hits: Vec<CodeSearchHit> = Vec::new();

    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        let rel = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let rel_str = rel.to_string_lossy().to_string();

        if let Some(ref pattern) = include_pattern {
            let filename = rel.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
            if !pattern.matches(&filename) && !pattern.matches(&rel_str) {
                continue;
            }
        }

        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut matched_line_indexes = Vec::new();
        let mut score = 0usize;

        let path_score_source = if case_sensitive {
            rel_str.clone()
        } else {
            rel_str.to_lowercase()
        };
        if path_score_source.contains(&q) {
            score += 25;
        }
        for token in &tokens {
            if path_score_source.contains(token) {
                score += 5;
            }
        }

        for (idx, line) in lines.iter().enumerate() {
            let target = if case_sensitive {
                (*line).to_string()
            } else {
                line.to_lowercase()
            };

            let has_query = target.contains(&q);
            let token_hits = tokens
                .iter()
                .filter(|t| target.contains(t.as_str()))
                .count();
            if has_query || token_hits > 0 {
                matched_line_indexes.push(idx);
                score += if has_query { 20 } else { 8 };
                score += token_hits;
            }
        }

        if matched_line_indexes.is_empty() {
            continue;
        }

        let snippets = build_snippets(
            &rel_str,
            &lines,
            &matched_line_indexes,
            before_context,
            after_context,
        );

        hits.push(CodeSearchHit {
            path: rel_str,
            score,
            matches: matched_line_indexes.len(),
            snippets,
        });
    }

    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.matches.cmp(&a.matches))
            .then_with(|| a.path.cmp(&b.path))
    });
    hits.truncate(max_results);

    Ok(hits)
}

fn resolve_search_root(path: &str, ctx: &ToolContext) -> Result<PathBuf, ToolError> {
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
                ToolError::ExecutionError(format!("Failed to resolve search path '{}': {e}", path))
            })
        }
    }
}

fn build_snippets(
    rel_path: &str,
    lines: &[&str],
    match_indexes: &[usize],
    before_context: usize,
    after_context: usize,
) -> Vec<String> {
    let mut snippets = Vec::new();
    let mut seen_lines = HashSet::new();

    for idx in match_indexes {
        let start = idx.saturating_sub(before_context);
        let end = (idx + after_context + 1).min(lines.len());

        for line_no in start..end {
            if seen_lines.insert(line_no) {
                snippets.push(format!(
                    "{}:{}:{}",
                    rel_path,
                    line_no + 1,
                    lines[line_no]
                ));
            }
        }
    }

    snippets
}

fn format_hits(hits: &[CodeSearchHit]) -> String {
    if hits.is_empty() {
        return "(no code matches)".to_string();
    }

    let mut out = Vec::new();
    for (i, hit) in hits.iter().enumerate() {
        out.push(format!(
            "{}. {} (score={}, matches={})",
            i + 1,
            hit.path,
            hit.score,
            hit.matches
        ));
        for snippet in hit.snippets.iter().take(8) {
            out.push(format!("   {}", snippet));
        }
        if hit.snippets.len() > 8 {
            out.push("   ...".to_string());
        }
    }
    out.join("\n")
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
    async fn test_codesearch_finds_symbol() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "pub fn parse_file() {}\nfn helper() {}\n",
        )
        .unwrap();

        let result = CodeSearchTool
            .execute(
                serde_json::json!({"query":"parse_file","path":"src"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.output.contains("parse_file"));
        assert_eq!(result.metadata["count"], 1);
    }

    #[tokio::test]
    async fn test_codesearch_include_filter() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn alpha() {}\n").unwrap();
        std::fs::write(dir.path().join("b.py"), "def alpha():\n    pass\n").unwrap();

        let result = CodeSearchTool
            .execute(
                serde_json::json!({"query":"alpha","include":"*.rs"}),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.output.contains("a.rs"));
        assert!(!result.output.contains("b.py"));
    }

    #[tokio::test]
    async fn test_codesearch_workspace_guard() {
        let dir = TempDir::new().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
            sandbox_mode: SandboxMode::WorkspaceWrite,
            ..Default::default()
        };

        let result = CodeSearchTool
            .execute(
                serde_json::json!({"query":"x","path": outside.path().display().to_string()}),
                &ctx,
            )
            .await;

        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }
}
