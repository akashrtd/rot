//! WebSearch tool — search the web through a public JSON endpoint.

use crate::error::ToolError;
use crate::traits::{SandboxMode, Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const DEFAULT_LIMIT: usize = 5;
const MAX_LIMIT: usize = 10;

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

/// Parameters for web search.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchParams {
    /// Search query text.
    pub query: String,
    /// Maximum number of results to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct SearchHit {
    title: String,
    url: Option<String>,
    snippet: String,
}

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "websearch"
    }

    fn label(&self) -> &str {
        "Web Search"
    }

    fn description(&self) -> &str {
        "Search the web for a query and return concise structured results."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(WebSearchParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        if !ctx.network_access && ctx.sandbox_mode != SandboxMode::DangerFullAccess {
            return Err(ToolError::PermissionDenied(
                "websearch is disabled because sandbox network access is off".to_string(),
            ));
        }

        let params: WebSearchParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;
        let query = params.query.trim();
        if query.is_empty() {
            return Err(ToolError::InvalidParameters(
                "query must not be empty".to_string(),
            ));
        }

        let limit = params.limit.clamp(1, MAX_LIMIT);

        let mut url = reqwest::Url::parse("https://api.duckduckgo.com/")
            .map_err(|e| ToolError::ExecutionError(format!("Failed to build search URL: {e}")))?;
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("format", "json")
            .append_pair("no_html", "1")
            .append_pair("skip_disambig", "1")
            .append_pair("no_redirect", "1");

        let client = reqwest::Client::builder()
            .timeout(ctx.timeout)
            .user_agent("rot/0.1")
            .build()
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create HTTP client: {e}")))?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("HTTP request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            return Ok(ToolResult::error(format!(
                "HTTP {status} while searching for '{}'",
                query
            )));
        }

        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to parse search response: {e}")))?;

        let results = parse_search_results(&payload, limit);
        let output = format_results(&results);

        Ok(ToolResult::success_with_metadata(
            output,
            serde_json::json!({
                "query": query,
                "count": results.len(),
                "limit": limit,
                "engine": "duckduckgo-instant-answer",
                "results": results,
            }),
        ))
    }
}

fn parse_search_results(payload: &serde_json::Value, limit: usize) -> Vec<SearchHit> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    if let Some(answer) = payload.get("Answer").and_then(|v| v.as_str()) {
        let answer = answer.trim();
        if !answer.is_empty() {
            push_result(
                &mut results,
                &mut seen,
                SearchHit {
                    title: "Instant answer".to_string(),
                    url: payload
                        .get("AnswerURL")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string),
                    snippet: answer.to_string(),
                },
                limit,
            );
        }
    }

    if let Some(abstract_text) = payload.get("AbstractText").and_then(|v| v.as_str()) {
        let abstract_text = abstract_text.trim();
        if !abstract_text.is_empty() {
            let title = payload
                .get("Heading")
                .and_then(|v| v.as_str())
                .filter(|heading| !heading.trim().is_empty())
                .unwrap_or("Abstract");
            push_result(
                &mut results,
                &mut seen,
                SearchHit {
                    title: title.to_string(),
                    url: payload
                        .get("AbstractURL")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string),
                    snippet: abstract_text.to_string(),
                },
                limit,
            );
        }
    }

    if let Some(topics) = payload.get("RelatedTopics").and_then(|v| v.as_array()) {
        collect_related_topics(topics, &mut results, &mut seen, limit);
    }

    results.truncate(limit);
    results
}

fn collect_related_topics(
    topics: &[serde_json::Value],
    results: &mut Vec<SearchHit>,
    seen: &mut HashSet<String>,
    limit: usize,
) {
    for topic in topics {
        if results.len() >= limit {
            return;
        }

        if let Some(nested) = topic.get("Topics").and_then(|v| v.as_array()) {
            collect_related_topics(nested, results, seen, limit);
            continue;
        }

        let text = topic
            .get("Text")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());

        if let Some(text) = text {
            let title = text
                .split(" - ")
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("Result")
                .to_string();

            push_result(
                results,
                seen,
                SearchHit {
                    title,
                    url: topic
                        .get("FirstURL")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string),
                    snippet: text.to_string(),
                },
                limit,
            );
        }
    }
}

fn push_result(
    results: &mut Vec<SearchHit>,
    seen: &mut HashSet<String>,
    hit: SearchHit,
    limit: usize,
) {
    if results.len() >= limit {
        return;
    }

    let key = format!(
        "{}|{}",
        hit.title,
        hit.url.as_deref().unwrap_or_default()
    );
    if seen.insert(key) {
        results.push(hit);
    }
}

fn format_results(results: &[SearchHit]) -> String {
    if results.is_empty() {
        return "(no search results)".to_string();
    }

    let mut lines = Vec::with_capacity(results.len() * 3);
    for (idx, hit) in results.iter().enumerate() {
        lines.push(format!("{}. {}", idx + 1, hit.title));
        if let Some(url) = &hit.url {
            lines.push(format!("   {}", url));
        }
        lines.push(format!("   {}", hit.snippet));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websearch_schema() {
        let tool = WebSearchTool;
        let schema = tool.parameters_schema();
        assert!(schema.get("properties").is_some());
        assert!(schema["properties"]["query"].is_object());
    }

    #[tokio::test]
    async fn test_websearch_denied_when_network_disabled() {
        let ctx = ToolContext {
            network_access: false,
            sandbox_mode: SandboxMode::WorkspaceWrite,
            ..Default::default()
        };
        let result = WebSearchTool
            .execute(serde_json::json!({"query":"rust"}), &ctx)
            .await;
        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }

    #[test]
    fn test_parse_search_results_extracts_abstract_and_topics() {
        let payload = serde_json::json!({
            "Heading": "Rust",
            "AbstractText": "Rust is a language.",
            "AbstractURL": "https://www.rust-lang.org/",
            "RelatedTopics": [
                {"Text": "Cargo - Rust package manager", "FirstURL": "https://doc.rust-lang.org/cargo/"},
                {"Name": "Nested", "Topics": [
                    {"Text": "Clippy - Rust lints", "FirstURL": "https://doc.rust-lang.org/clippy/"}
                ]}
            ]
        });

        let results = parse_search_results(&payload, 3);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].title, "Rust");
        assert!(results[1].snippet.contains("Cargo"));
        assert!(results[2].snippet.contains("Clippy"));
    }
}
