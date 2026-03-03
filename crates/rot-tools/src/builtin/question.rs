//! Question tool — request a clarification question in the conversation flow.

use crate::error::ToolError;
use crate::traits::{Tool, ToolContext, ToolResult};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QuestionParams {
    /// Clarification question to ask the user.
    pub question: String,
    /// Optional suggested answers.
    #[serde(default)]
    pub options: Vec<String>,
}

pub struct QuestionTool;

#[async_trait]
impl Tool for QuestionTool {
    fn name(&self) -> &str {
        "question"
    }

    fn label(&self) -> &str {
        "Ask Question"
    }

    fn description(&self) -> &str {
        "Request clarification from the user inside the conversation flow."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(QuestionParams))
            .expect("schema serialization should not fail")
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let params: QuestionParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParameters(e.to_string()))?;

        let mut output = format!("User clarification required: {}", params.question.trim());
        if !params.options.is_empty() {
            output.push_str("\nSuggested options:\n");
            for (idx, option) in params.options.iter().enumerate() {
                output.push_str(&format!("{}. {}\n", idx + 1, option));
            }
        }

        Ok(ToolResult::success_with_metadata(
            output,
            serde_json::json!({
                "requires_user_input": true,
                "options_count": params.options.len(),
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_question_tool_output() {
        let result = QuestionTool
            .execute(
                serde_json::json!({
                    "question":"Which module should I prioritize?",
                    "options":["core","tui"]
                }),
                &ToolContext::default(),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.output.contains("User clarification required"));
        assert_eq!(result.metadata["requires_user_input"], true);
    }
}
