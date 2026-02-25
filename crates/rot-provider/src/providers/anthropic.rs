//! Anthropic Claude provider implementation.
//!
//! Implements the Provider trait for Anthropic's Messages API with SSE streaming.

use crate::error::ProviderError;
use crate::traits::Provider;
use crate::types::{
    ModelInfo, ProviderContent, ProviderMessage, Request, Response, StopReason, StreamEvent,
    ToolDefinition, Usage,
};
use async_trait::async_trait;
use futures::stream::{self, BoxStream, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

const API_BASE: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";
const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
const DEFAULT_MAX_TOKENS: usize = 16_384;

/// Anthropic Claude provider.
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: DEFAULT_MODEL.to_string(),
            base_url: API_BASE.to_string(),
        }
    }

    /// Create with a custom base URL (for testing/proxy).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Convert a generic Request into the Anthropic API request body.
    fn build_request_body(&self, request: &Request) -> Value {
        let messages: Vec<Value> = request
            .messages
            .iter()
            .filter_map(|msg| self.convert_message(msg))
            .collect();

        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            "stream": true,
        });

        // Add tools
        if !request.tools.is_empty() {
            body["tools"] = json!(request
                .tools
                .iter()
                .map(|t| self.convert_tool(t))
                .collect::<Vec<_>>());
        }

        // Add system prompt
        if let Some(ref system) = request.system {
            body["system"] = json!(system);
        }

        // Add thinking config
        if let Some(ref thinking) = request.thinking {
            if thinking.enabled {
                body["thinking"] = json!({
                    "type": "enabled",
                    "budget_tokens": thinking.budget_tokens,
                });
            }
        }

        body
    }

    /// Convert a ProviderMessage to the Anthropic JSON format.
    fn convert_message(&self, msg: &ProviderMessage) -> Option<Value> {
        // Anthropic only accepts "user" and "assistant" roles in messages
        let role = match msg.role.as_str() {
            "user" | "assistant" => msg.role.as_str(),
            "tool" => "user", // Tool results are sent as user messages
            _ => return None, // System is handled separately
        };

        let content: Vec<Value> = msg
            .content
            .iter()
            .filter_map(|c| self.convert_content(c))
            .collect();

        if content.is_empty() {
            return None;
        }

        Some(json!({
            "role": role,
            "content": content,
        }))
    }

    /// Convert a ProviderContent block to Anthropic format.
    fn convert_content(&self, content: &ProviderContent) -> Option<Value> {
        match content {
            ProviderContent::Text { text } => Some(json!({
                "type": "text",
                "text": text,
            })),
            ProviderContent::ToolCall {
                id,
                name,
                arguments,
            } => Some(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": arguments,
            })),
            ProviderContent::ToolResult {
                tool_call_id,
                content,
                is_error,
            } => Some(json!({
                "type": "tool_result",
                "tool_use_id": tool_call_id,
                "content": content,
                "is_error": is_error,
            })),
            ProviderContent::Image { data, mime_type } => Some(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": mime_type,
                    "data": data,
                },
            })),
        }
    }

    /// Convert a ToolDefinition to Anthropic format.
    fn convert_tool(&self, tool: &ToolDefinition) -> Value {
        json!({
            "name": tool.name,
            "description": tool.description,
            "input_schema": tool.parameters,
        })
    }

    /// Parse an Anthropic SSE event into StreamEvent(s).
    fn parse_sse_event(&self, event: &AnthropicEvent) -> Vec<StreamEvent> {
        match event {
            AnthropicEvent::ContentBlockDelta {
                delta, index: _, ..
            } => match delta {
                Delta::Text { text } => {
                    vec![StreamEvent::TextDelta {
                        delta: text.clone(),
                    }]
                }
                Delta::Thinking { thinking } => {
                    vec![StreamEvent::ThinkingDelta {
                        delta: thinking.clone(),
                    }]
                }
                Delta::InputJson { partial_json } => {
                    // Tool argument streaming — need to track which tool call this belongs to
                    // For now, we emit a generic delta. The caller tracks tool call state.
                    vec![StreamEvent::ToolCallDelta {
                        id: String::new(), // Will be set by caller from block state
                        delta: partial_json.clone(),
                    }]
                }
            },
            AnthropicEvent::ContentBlockStart {
                index: _,
                content_block,
            } => match content_block {
                ContentBlockInfo::ToolUse { id, name, .. } => {
                    vec![StreamEvent::ToolCallStart {
                        id: id.clone(),
                        name: name.clone(),
                    }]
                }
                ContentBlockInfo::Text { .. } => vec![],
                ContentBlockInfo::Thinking { .. } => vec![],
            },
            AnthropicEvent::ContentBlockStop { index: _ } => {
                // Could be end of text or end of tool call
                // The caller should track state to determine which
                vec![]
            }
            AnthropicEvent::MessageStart { message } => {
                let mut events = Vec::new();
                if let Some(usage) = &message.usage {
                    events.push(StreamEvent::Usage {
                        input: usage.input_tokens.unwrap_or(0),
                        output: usage.output_tokens.unwrap_or(0),
                    });
                }
                events
            }
            AnthropicEvent::MessageDelta { delta, usage } => {
                let mut events = Vec::new();
                if let Some(usage) = usage {
                    events.push(StreamEvent::Usage {
                        input: usage.input_tokens.unwrap_or(0),
                        output: usage.output_tokens.unwrap_or(0),
                    });
                }
                if let Some(reason) = &delta.stop_reason {
                    events.push(StreamEvent::Done {
                        reason: match reason.as_str() {
                            "end_turn" => StopReason::EndTurn,
                            "tool_use" => StopReason::ToolUse,
                            "max_tokens" => StopReason::MaxTokens,
                            "stop_sequence" => StopReason::StopSequence,
                            _ => StopReason::EndTurn,
                        },
                    });
                }
                events
            }
            AnthropicEvent::MessageStop => {
                vec![StreamEvent::Done {
                    reason: StopReason::EndTurn,
                }]
            }
            AnthropicEvent::Ping => vec![],
            AnthropicEvent::Error { error } => {
                vec![StreamEvent::Error(format!(
                    "{}: {}",
                    error.error_type, error.message
                ))]
            }
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "claude-sonnet-4-20250514".to_string(),
                name: "Claude Sonnet 4".to_string(),
                context_window: 200_000,
                max_output_tokens: 16_384,
                supports_thinking: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "claude-3-5-sonnet-20241022".to_string(),
                name: "Claude 3.5 Sonnet".to_string(),
                context_window: 200_000,
                max_output_tokens: 8_192,
                supports_thinking: false,
                supports_tools: true,
            },
        ]
    }

    fn current_model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: &str) -> Result<(), ProviderError> {
        if self.models().iter().any(|m| m.id == model) {
            self.model = model.to_string();
            Ok(())
        } else {
            Err(ProviderError::InvalidModel(model.to_string()))
        }
    }

    async fn stream(
        &self,
        request: Request,
    ) -> Result<BoxStream<'_, Result<StreamEvent, ProviderError>>, ProviderError> {
        let body = self.build_request_body(&request);

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(ProviderError::Http)?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(ProviderError::ApiError(format!(
                "HTTP {}: {}",
                status, error_body
            )));
        }

        // Parse SSE stream from response bytes
        let this = self;
        let byte_stream = response.bytes_stream();
        let mut buffer = String::new();

        let event_stream = byte_stream
            .map(move |chunk| -> Result<Vec<StreamEvent>, ProviderError> {
                let chunk = chunk.map_err(ProviderError::Http)?;
                let text = String::from_utf8_lossy(&chunk);
                buffer.push_str(&text);

                let mut events = Vec::new();

                // Process complete SSE lines from the buffer
                while let Some(pos) = buffer.find("\n\n") {
                    let event_text = buffer[..pos].to_string();
                    buffer = buffer[pos + 2..].to_string();

                    for line in event_text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                continue;
                            }
                            match serde_json::from_str::<AnthropicEvent>(data) {
                                Ok(event) => {
                                    events.extend(this.parse_sse_event(&event));
                                }
                                Err(_) => {
                                    // Skip unparseable events
                                }
                            }
                        }
                    }
                }

                Ok(events)
            })
            .flat_map(|result| match result {
                Ok(events) => stream::iter(events.into_iter().map(Ok).collect::<Vec<_>>()),
                Err(e) => stream::iter(vec![Err(e)]),
            });

        Ok(Box::pin(event_stream))
    }

    async fn complete(&self, request: Request) -> Result<Response, ProviderError> {
        let mut event_stream = self.stream(request).await?;
        let mut content = Vec::new();
        let mut usage = Usage::default();
        let mut stop_reason = StopReason::EndTurn;
        let mut current_text = String::new();

        while let Some(event) = event_stream.next().await {
            match event? {
                StreamEvent::TextDelta { delta } => {
                    current_text.push_str(&delta);
                }
                StreamEvent::Usage { input, output } => {
                    usage.input_tokens = input;
                    usage.output_tokens = output;
                }
                StreamEvent::Done { reason } => {
                    stop_reason = reason;
                    break;
                }
                _ => {}
            }
        }

        if !current_text.is_empty() {
            content.push(ProviderContent::Text {
                text: current_text,
            });
        }

        Ok(Response {
            content,
            stop_reason,
            usage,
        })
    }
}

// ──────────────────────────────────────────────────────────
// Anthropic SSE event types (internal)
// ──────────────────────────────────────────────────────────

/// Top-level SSE event from the Anthropic API.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum AnthropicEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageStartData },

    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockInfo,
    },

    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: Delta },

    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },

    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: MessageDeltaData,
        #[serde(default)]
        usage: Option<UsageData>,
    },

    #[serde(rename = "message_stop")]
    MessageStop,

    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "error")]
    Error { error: ErrorData },
}

#[derive(Debug, Deserialize)]
struct MessageStartData {
    #[serde(default)]
    usage: Option<UsageData>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum ContentBlockInfo {
    #[serde(rename = "text")]
    Text {
        #[serde(default)]
        text: String,
    },

    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },

    #[serde(rename = "thinking")]
    Thinking {
        #[serde(default)]
        thinking: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
enum Delta {
    #[serde(rename = "text_delta")]
    Text { text: String },

    #[serde(rename = "thinking_delta")]
    Thinking { thinking: String },

    #[serde(rename = "input_json_delta")]
    InputJson { partial_json: String },
}

#[derive(Debug, Deserialize)]
struct MessageDeltaData {
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageData {
    #[serde(default)]
    input_tokens: Option<usize>,
    #[serde(default)]
    output_tokens: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ErrorData {
    #[serde(rename = "type", default)]
    error_type: String,
    #[serde(default)]
    message: String,
}

// ──────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_name() {
        let provider = AnthropicProvider::new("test-key");
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_default_model() {
        let provider = AnthropicProvider::new("test-key");
        assert_eq!(provider.current_model(), DEFAULT_MODEL);
    }

    #[test]
    fn test_set_valid_model() {
        let mut provider = AnthropicProvider::new("test-key");
        assert!(provider
            .set_model("claude-3-5-sonnet-20241022")
            .is_ok());
        assert_eq!(provider.current_model(), "claude-3-5-sonnet-20241022");
    }

    #[test]
    fn test_set_invalid_model() {
        let mut provider = AnthropicProvider::new("test-key");
        assert!(provider.set_model("nonexistent-model").is_err());
    }

    #[test]
    fn test_models_list() {
        let provider = AnthropicProvider::new("test-key");
        let models = provider.models();
        assert_eq!(models.len(), 2);
        assert!(models.iter().any(|m| m.id == "claude-sonnet-4-20250514"));
    }

    #[test]
    fn test_build_request_body() {
        let provider = AnthropicProvider::new("test-key");
        let request = Request {
            messages: vec![ProviderMessage {
                role: "user".to_string(),
                content: vec![ProviderContent::Text {
                    text: "Hello".to_string(),
                }],
            }],
            tools: vec![],
            system: Some("You are helpful.".to_string()),
            max_tokens: Some(1024),
            thinking: None,
        };

        let body = provider.build_request_body(&request);
        assert_eq!(body["model"], DEFAULT_MODEL);
        assert_eq!(body["max_tokens"], 1024);
        assert_eq!(body["system"], "You are helpful.");
        assert!(body["stream"].as_bool().unwrap());
    }

    #[test]
    fn test_build_request_with_tools() {
        let provider = AnthropicProvider::new("test-key");
        let request = Request {
            messages: vec![],
            tools: vec![ToolDefinition {
                name: "read".to_string(),
                description: "Read a file".to_string(),
                parameters: json!({"type": "object"}),
            }],
            system: None,
            max_tokens: None,
            thinking: None,
        };

        let body = provider.build_request_body(&request);
        assert!(body["tools"].is_array());
        assert_eq!(body["tools"][0]["name"], "read");
    }

    #[test]
    fn test_parse_text_delta() {
        let provider = AnthropicProvider::new("test-key");
        let event = AnthropicEvent::ContentBlockDelta {
            index: 0,
            delta: Delta::Text {
                text: "Hello".to_string(),
            },
        };

        let events = provider.parse_sse_event(&event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::TextDelta { delta } => assert_eq!(delta, "Hello"),
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_parse_tool_call_start() {
        let provider = AnthropicProvider::new("test-key");
        let event = AnthropicEvent::ContentBlockStart {
            index: 0,
            content_block: ContentBlockInfo::ToolUse {
                id: "tc_1".to_string(),
                name: "read".to_string(),
            },
        };

        let events = provider.parse_sse_event(&event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ToolCallStart { id, name } => {
                assert_eq!(id, "tc_1");
                assert_eq!(name, "read");
            }
            _ => panic!("Expected ToolCallStart"),
        }
    }

    #[test]
    fn test_parse_message_delta_stop() {
        let provider = AnthropicProvider::new("test-key");
        let event = AnthropicEvent::MessageDelta {
            delta: MessageDeltaData {
                stop_reason: Some("end_turn".to_string()),
            },
            usage: Some(UsageData {
                input_tokens: Some(100),
                output_tokens: Some(200),
            }),
        };

        let events = provider.parse_sse_event(&event);
        assert_eq!(events.len(), 2); // Usage + Done
    }

    #[test]
    fn test_parse_error_event() {
        let provider = AnthropicProvider::new("test-key");
        let event = AnthropicEvent::Error {
            error: ErrorData {
                error_type: "rate_limit".to_string(),
                message: "Too many requests".to_string(),
            },
        };

        let events = provider.parse_sse_event(&event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Error(msg) => assert!(msg.contains("rate_limit")),
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_parse_sse_json() {
        // Test that our internal event types deserialize correctly from Anthropic JSON
        let text_delta = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let event: AnthropicEvent = serde_json::from_str(text_delta).unwrap();
        match event {
            AnthropicEvent::ContentBlockDelta { delta, .. } => match delta {
                Delta::Text { text } => assert_eq!(text, "Hello"),
                _ => panic!("Wrong delta type"),
            },
            _ => panic!("Wrong event type"),
        }

        let msg_start = r#"{"type":"message_start","message":{"usage":{"input_tokens":25,"output_tokens":1}}}"#;
        let event: AnthropicEvent = serde_json::from_str(msg_start).unwrap();
        match event {
            AnthropicEvent::MessageStart { message } => {
                assert_eq!(message.usage.unwrap().input_tokens, Some(25));
            }
            _ => panic!("Wrong event type"),
        }
    }
}
