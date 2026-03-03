//! LSP tool — experimental language-server-assisted intelligence with fallback.

use crate::builtin::codesearch::run_codesearch;
use crate::error::ToolError;
use crate::path_guard::resolve_existing_path;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const DEFAULT_MAX_RESULTS: usize = 10;

fn default_path() -> String {
    ".".to_string()
}

fn default_max_results() -> usize {
    DEFAULT_MAX_RESULTS
}

/// Types of code-intelligence queries.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LspAction {
    /// Find definitions for a symbol or query.
    Definition,
    /// Find references for a symbol or query.
    References,
    /// Lookup hover-like information for a symbol.
    Hover,
    /// Search workspace symbols.
    Symbols,
}

impl Default for LspAction {
    fn default() -> Self {
        Self::Symbols
    }
}

/// Parameters for the experimental LSP tool.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LspParams {
    /// Query action to run.
    #[serde(default)]
    pub action: LspAction,
    /// Query text (symbol/function/type/etc.).
    pub query: String,
    /// Root path to run against.
    #[serde(default = "default_path")]
    pub path: String,
    /// Optional language server command override.
    #[serde(default)]
    pub language_server: Option<String>,
    /// Maximum search results in fallback mode.
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

pub struct LspTool;

#[async_trait]
impl Tool for LspTool {
    fn name(&self) -> &str {
        "lsp"
    }

    fn label(&self) -> &str {
        "LSP (Experimental)"
    }

    fn description(&self) -> &str {
        "EXPERIMENTAL: Language-server code intelligence with graceful codesearch fallback."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(LspParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: LspParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let query = params.query.trim();
        if query.is_empty() {
            return Err(ToolError::InvalidParameters(
                "query must not be empty".to_string(),
            ));
        }

        let root = resolve_root(&params.path, ctx)?;
        let requested_server = params
            .language_server
            .or_else(|| std::env::var("ROT_LSP_SERVER").ok());

        let (availability, reason) = match requested_server {
            Some(server) => {
                let available = tokio::process::Command::new(&server)
                    .arg("--version")
                    .output()
                    .await
                    .is_ok();
                if available {
                    (
                        "configured".to_string(),
                        format!(
                            "Server '{}' detected, but protocol wiring is not enabled in this build",
                            server
                        ),
                    )
                } else {
                    (
                        "missing".to_string(),
                        format!("Configured server '{}' is not available", server),
                    )
                }
            }
            None => (
                "missing".to_string(),
                "No language server configured (set `language_server` or `ROT_LSP_SERVER`)"
                    .to_string(),
            ),
        };

        let hits = run_codesearch(
            &root,
            query,
            None,
            params.max_results.clamp(1, 100),
            1,
            1,
            false,
        )?;
        let fallback_summary = if hits.is_empty() {
            "(no fallback matches found)".to_string()
        } else {
            hits.iter()
                .take(10)
                .enumerate()
                .map(|(i, hit)| format!("{}. {} (score={})", i + 1, hit.path, hit.score))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let action = match params.action {
            LspAction::Definition => "definition",
            LspAction::References => "references",
            LspAction::Hover => "hover",
            LspAction::Symbols => "symbols",
        };

        let output = format!(
            "[EXPERIMENTAL] lsp/{action} query '{query}'\n{reason}\nFallback (codesearch) results:\n{fallback_summary}"
        );

        Ok(ToolResult::success_with_metadata(
            output,
            serde_json::json!({
                "experimental": true,
                "action": action,
                "query": query,
                "server_availability": availability,
                "fallback": "codesearch",
                "fallback_count": hits.len(),
            }),
        ))
    }
}

fn resolve_root(path: &str, ctx: &ToolContext) -> Result<PathBuf, ToolError> {
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
                ToolError::ExecutionError(format!("Failed to resolve path '{}': {e}", path))
            })
        }
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

    #[test]
    fn test_lsp_description_is_experimental() {
        let tool = LspTool;
        assert!(tool.description().contains("EXPERIMENTAL"));
    }

    #[tokio::test]
    async fn test_lsp_fallback_when_no_server_configured() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn parse_ast() {}\n").unwrap();

        let result = LspTool
            .execute(
                serde_json::json!({
                    "action":"symbols",
                    "query":"parse_ast"
                }),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.output.contains("Fallback (codesearch)"));
        assert_eq!(result.metadata["experimental"], true);
        assert_eq!(result.metadata["fallback"], "codesearch");
    }

    #[tokio::test]
    async fn test_lsp_gracefully_handles_missing_server_binary() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn parse_ast() {}\n").unwrap();

        let result = LspTool
            .execute(
                serde_json::json!({
                    "action":"definition",
                    "query":"parse_ast",
                    "language_server":"not-a-real-lsp-binary"
                }),
                &test_ctx(&dir),
            )
            .await
            .unwrap();

        assert!(result.output.contains("Configured server"));
        assert_eq!(result.metadata["server_availability"], "missing");
    }
}
