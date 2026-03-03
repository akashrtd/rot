//! Structured trajectory logging for RLM runs.

use crate::subcall::SubcallRecord;
use crate::usage::RlmUsage;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Per-run trajectory artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmTrajectory {
    /// Run identifier.
    pub run_id: String,
    /// Unix timestamp in seconds.
    pub started_at: u64,
    /// Unix timestamp in seconds.
    pub finished_at: u64,
    /// Execution status (`ok` or `error`).
    pub status: String,
    /// Root task prompt.
    pub prompt: String,
    /// Context source path.
    pub context_path: String,
    /// Detected context type.
    pub context_type: String,
    /// Runtime kind label.
    pub runtime: String,
    /// Iteration records.
    pub iterations: Vec<IterationTrace>,
    /// Nested subcall records.
    pub subcalls: Vec<SubcallRecord>,
    /// Aggregated usage counters.
    pub usage: RlmUsage,
    /// Final answer text on success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_text: Option<String>,
    /// Error string on failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One top-level engine iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationTrace {
    /// One-based iteration index.
    pub index: usize,
    /// Prompt sent to the model for this iteration (possibly truncated).
    pub step_prompt: String,
    /// Runtime code blocks executed this iteration.
    pub code_blocks: Vec<String>,
    /// Runtime execution results.
    pub executions: Vec<ExecutionTrace>,
    /// Iteration elapsed time.
    pub elapsed_ms: u128,
}

/// One runtime code block execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Source code sent to runtime.
    pub code: String,
    /// Captured stdout (possibly truncated).
    pub stdout: String,
    /// Captured stderr (possibly truncated).
    pub stderr: String,
    /// Runtime exit code, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Whether stdout/stderr payloads were truncated.
    pub truncated: bool,
    /// Subcalls emitted from this execution.
    pub subcall_ids: Vec<String>,
}

/// Persist trajectory as JSON to a file and return its path.
pub async fn persist_trajectory(
    trajectory: &RlmTrajectory,
    out_dir: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    let base = out_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("rot-rlm").join("trajectories"));
    tokio::fs::create_dir_all(&base).await.map_err(|e| {
        anyhow::anyhow!(
            "failed to create trajectory dir '{}': {e}",
            base.display()
        )
    })?;

    let path = base.join(format!("{}.json", trajectory.run_id));
    let body = serde_json::to_string_pretty(trajectory)
        .map_err(|e| anyhow::anyhow!("failed to serialize trajectory: {e}"))?;
    tokio::fs::write(&path, body)
        .await
        .map_err(|e| anyhow::anyhow!("failed to write trajectory '{}': {e}", path.display()))?;

    Ok(path)
}

/// Truncate a string for trajectory logging, returning `(text, truncated_flag)`.
pub fn truncate_for_trace(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() <= max_chars {
        return (text.to_string(), false);
    }
    let prefix: String = text.chars().take(max_chars).collect();
    (format!("{prefix}\n...[truncated]..."), true)
}

#[cfg(test)]
mod tests {
    use super::{RlmTrajectory, persist_trajectory, truncate_for_trace};
    use crate::usage::RlmUsage;

    #[tokio::test]
    async fn test_persist_trajectory() {
        let dir = tempfile::tempdir().unwrap();
        let trajectory = RlmTrajectory {
            run_id: "test-run".to_string(),
            started_at: 1,
            finished_at: 2,
            status: "ok".to_string(),
            prompt: "prompt".to_string(),
            context_path: "/tmp/ctx.txt".to_string(),
            context_type: "text".to_string(),
            runtime: "python".to_string(),
            iterations: Vec::new(),
            subcalls: Vec::new(),
            usage: RlmUsage::default(),
            final_text: Some("answer".to_string()),
            error: None,
        };

        let path = persist_trajectory(&trajectory, Some(dir.path())).await.unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_truncate_for_trace() {
        let (short, truncated) = truncate_for_trace("abcdef", 10);
        assert_eq!(short, "abcdef");
        assert!(!truncated);

        let (long, truncated) = truncate_for_trace("abcdefghij", 4);
        assert!(truncated);
        assert!(long.contains("...[truncated]..."));
    }
}
