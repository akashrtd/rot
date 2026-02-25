//! Common types used by the provider trait and implementations.

use serde::{Deserialize, Serialize};

/// A message in provider-native format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMessage {
    /// Message role (user, assistant, system, tool).
    pub role: String,
    /// Content blocks.
    pub content: Vec<ProviderContent>,
}

/// Content block in provider-native format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProviderContent {
    /// Plain text.
    #[serde(rename = "text")]
    Text { text: String },

    /// Tool invocation.
    #[serde(rename = "tool_call")]
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },

    /// Tool result.
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_call_id: String,
        content: String,
        is_error: bool,
    },

    /// Image.
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
}

/// Tool definition for the provider API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for the tool's parameters.
    pub parameters: serde_json::Value,
}

/// Configuration for extended thinking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Whether thinking is enabled.
    pub enabled: bool,
    /// Budget in tokens for thinking.
    pub budget_tokens: usize,
}

/// Request to a provider.
#[derive(Debug, Clone)]
pub struct Request {
    /// Conversation messages.
    pub messages: Vec<ProviderMessage>,
    /// Available tools.
    pub tools: Vec<ToolDefinition>,
    /// System prompt.
    pub system: Option<String>,
    /// Maximum tokens in the response.
    pub max_tokens: Option<usize>,
    /// Thinking configuration.
    pub thinking: Option<ThinkingConfig>,
}

/// Non-streaming response from a provider.
#[derive(Debug, Clone)]
pub struct Response {
    /// Content blocks in the response.
    pub content: Vec<ProviderContent>,
    /// Stop reason.
    pub stop_reason: StopReason,
    /// Token usage.
    pub usage: Usage,
}

/// Token usage information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Input tokens consumed.
    pub input_tokens: usize,
    /// Output tokens generated.
    pub output_tokens: usize,
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Normal end of response.
    EndTurn,
    /// Model wants to use a tool.
    ToolUse,
    /// Max tokens reached.
    MaxTokens,
    /// Stop sequence matched.
    StopSequence,
}

/// Events emitted during streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEvent {
    /// A chunk of text output.
    TextDelta { delta: String },
    /// A chunk of thinking/reasoning output.
    ThinkingDelta { delta: String },
    /// Start of a tool call.
    ToolCallStart { id: String, name: String },
    /// Incremental arguments for a tool call.
    ToolCallDelta { id: String, delta: String },
    /// End of a tool call.
    ToolCallEnd { id: String },
    /// Token usage update.
    Usage { input: usize, output: usize },
    /// Stream completed.
    Done { reason: StopReason },
    /// Stream error.
    Error(String),
}

/// Information about a model supported by a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier (e.g., "claude-sonnet-4-20250514").
    pub id: String,
    /// Human-readable model name.
    pub name: String,
    /// Maximum context window in tokens.
    pub context_window: usize,
    /// Maximum output tokens.
    pub max_output_tokens: usize,
    /// Whether the model supports extended thinking.
    pub supports_thinking: bool,
    /// Whether the model supports tool use.
    pub supports_tools: bool,
}
