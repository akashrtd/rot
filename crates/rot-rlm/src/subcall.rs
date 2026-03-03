//! Structured RLM subcall primitives.

use serde::{Deserialize, Serialize};

/// Prefix emitted by runtime helpers to request a nested model call.
pub const SUBLM_MARKER: &str = "__ROT_SUBLM__";

/// Structured nested model request emitted by runtime code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubcallRequest {
    /// Prompt for the nested call.
    pub query: String,
    /// Optional explicit input slice or payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    /// Optional variable name used as input source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_ref: Option<String>,
}

/// Structured nested model result captured in trajectory logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubcallRecord {
    /// Stable ID for this subcall.
    pub id: String,
    /// Zero-based nesting depth.
    pub depth: usize,
    /// Subcall request payload.
    pub request: SubcallRequest,
    /// Response text returned by the nested call.
    pub response: String,
    /// Optional error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Elapsed execution time.
    pub elapsed_ms: u128,
    /// Input token estimate used for budgeting.
    pub input_tokens: usize,
    /// Output token estimate used for budgeting.
    pub output_tokens: usize,
}

/// Parse a single runtime output line into a `SubcallRequest`.
///
/// Expected format is `__ROT_SUBLM__{json}`.
pub fn parse_subcall_line(line: &str) -> anyhow::Result<Option<SubcallRequest>> {
    let Some(payload) = line.strip_prefix(SUBLM_MARKER) else {
        return Ok(None);
    };

    let req: SubcallRequest = serde_json::from_str(payload).map_err(|e| {
        anyhow::anyhow!("invalid SUBLM payload JSON '{}': {e}", payload)
    })?;

    if req.query.trim().is_empty() {
        return Err(anyhow::anyhow!("SUBLM query cannot be empty"));
    }

    Ok(Some(req))
}

#[cfg(test)]
mod tests {
    use super::{SUBLM_MARKER, parse_subcall_line};

    #[test]
    fn test_parse_subcall_line_ok() {
        let line = format!(
            "{SUBLM_MARKER}{{\"query\":\"Summarize\",\"input\":\"abc\",\"input_ref\":\"chunk\"}}"
        );
        let parsed = parse_subcall_line(&line).unwrap().unwrap();
        assert_eq!(parsed.query, "Summarize");
        assert_eq!(parsed.input.as_deref(), Some("abc"));
        assert_eq!(parsed.input_ref.as_deref(), Some("chunk"));
    }

    #[test]
    fn test_parse_subcall_line_none() {
        let parsed = parse_subcall_line("normal output").unwrap();
        assert!(parsed.is_none());
    }

    #[test]
    fn test_parse_subcall_line_empty_query_rejected() {
        let line = format!("{SUBLM_MARKER}{{\"query\":\"  \"}}");
        assert!(parse_subcall_line(&line).is_err());
    }
}
