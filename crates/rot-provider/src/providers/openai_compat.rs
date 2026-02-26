//! Generic OpenAI-compatible provider.
//!
//! Handles the OpenAI chat completions API format used by z.ai, OpenAI,
//! Ollama, OpenRouter, and many other providers.

use crate::error::ProviderError;
use crate::traits::Provider;
use crate::types::{
    ModelInfo, ProviderContent, Request, Response, StopReason, StreamEvent,
    Usage,
};
use async_trait::async_trait;
use futures::stream::{self, BoxStream, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

/// Configuration for an OpenAI-compatible provider.
#[derive(Debug, Clone)]
pub struct OpenAiCompatConfig {
    pub base_url: String,
    pub api_key: String,
    pub provider_name: String,
    pub default_model: String,
    pub models: Vec<ModelInfo>,
}

/// A provider that speaks the OpenAI chat completions protocol.
pub struct OpenAiCompatProvider {
    config: OpenAiCompatConfig,
    model: String,
    client: Client,
}

impl OpenAiCompatProvider {
    /// Create a new OpenAI-compatible provider.
    pub fn new(config: OpenAiCompatConfig) -> Self {
        let model = config.default_model.clone();
        Self {
            config,
            model,
            client: Client::new(),
        }
    }

    /// Build the JSON request body.
    fn build_request_body(&self, request: Request) -> Value {
        let messages = self.convert_messages(&request);

        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
        });

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }

        if !request.tools.is_empty() {
            body["tools"] = json!(request.tools.iter().map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            }).collect::<Vec<_>>());
        }

        body
    }

    /// Convert messages to OpenAI format.
    fn convert_messages(&self, request: &Request) -> Vec<Value> {
        let mut messages = Vec::new();

        // System message
        if let Some(ref system) = request.system {
            messages.push(json!({"role": "system", "content": system}));
        }

        for msg in &request.messages {
            let role = msg.role.as_str();

            // Check for tool results — OpenAI sends these as role=tool
            let has_tool_results = msg.content.iter().any(|c| {
                matches!(c, ProviderContent::ToolResult { .. })
            });

            if has_tool_results {
                for block in &msg.content {
                    if let ProviderContent::ToolResult {
                        tool_call_id,
                        content,
                        ..
                    } = block
                    {
                        messages.push(json!({
                            "role": "tool",
                            "tool_call_id": tool_call_id,
                            "content": content,
                        }));
                    }
                }
                continue;
            }

            // Check for assistant with tool calls
            let has_tool_calls = msg.content.iter().any(|c| {
                matches!(c, ProviderContent::ToolCall { .. })
            });

            if has_tool_calls && role == "assistant" {
                let text_content: String = msg
                    .content
                    .iter()
                    .filter_map(|c| {
                        if let ProviderContent::Text { text } = c {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("");

                let tool_calls: Vec<Value> = msg
                    .content
                    .iter()
                    .filter_map(|c| {
                        if let ProviderContent::ToolCall {
                            id,
                            name,
                            arguments,
                        } = c
                        {
                            Some(json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": arguments.to_string(),
                                }
                            }))
                        } else {
                            None
                        }
                    })
                    .collect();

                let mut msg_json = json!({
                    "role": "assistant",
                    "tool_calls": tool_calls,
                });
                if !text_content.is_empty() {
                    msg_json["content"] = json!(text_content);
                }
                messages.push(msg_json);
                continue;
            }

            // Regular message
            let content: String = msg
                .content
                .iter()
                .filter_map(|c| {
                    if let ProviderContent::Text { text } = c {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("");

            messages.push(json!({"role": role, "content": content}));
        }

        messages
    }

    /// Parse an SSE line into stream events.
    fn parse_sse_event(data: &str) -> Vec<StreamEvent> {
        if data == "[DONE]" {
            return vec![StreamEvent::Done {
                reason: StopReason::EndTurn,
            }];
        }

        let chunk: OpenAiChunk = match serde_json::from_str(data) {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        let mut events = Vec::new();

        for choice in &chunk.choices {
            // Text delta
            if let Some(ref content) = choice.delta.content {
                if !content.is_empty() {
                    events.push(StreamEvent::TextDelta {
                        delta: content.clone(),
                    });
                }
            }

            // Tool call deltas
            if let Some(ref tool_calls) = choice.delta.tool_calls {
                for tc in tool_calls {
                    if let Some(ref func) = tc.function {
                        // If we have a function name, it's a new tool call
                        if let Some(ref name) = func.name {
                            let id = tc.id.clone().unwrap_or_default();
                            events.push(StreamEvent::ToolCallStart {
                                id: id.clone(),
                                name: name.clone(),
                            });
                        }
                        // If we have arguments, it's a delta
                        if let Some(ref args) = func.arguments {
                            if !args.is_empty() {
                                events.push(StreamEvent::ToolCallDelta {
                                    id: tc.id.clone().unwrap_or_default(),
                                    delta: args.clone(),
                                });
                            }
                        }
                    }
                }
            }

            // Finish reason
            if let Some(ref reason) = choice.finish_reason {
                let stop = match reason.as_str() {
                    "stop" => StopReason::EndTurn,
                    "tool_calls" => StopReason::ToolUse,
                    "length" => StopReason::MaxTokens,
                    _ => StopReason::EndTurn,
                };
                events.push(StreamEvent::Done { reason: stop });
            }
        }

        events
    }
}

#[async_trait]
impl Provider for OpenAiCompatProvider {
    fn name(&self) -> &str {
        &self.config.provider_name
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.config.models.clone()
    }

    fn current_model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: &str) -> Result<(), ProviderError> {
        if self.config.models.iter().any(|m| m.id == model) {
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
        let body = self.build_request_body(request);
        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(ProviderError::Http)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(ProviderError::ApiError(format!("HTTP {status}: {body}")));
        }

        let byte_stream = response.bytes_stream();

        let event_stream = byte_stream
            .map(|chunk: Result<_, reqwest::Error>| -> Result<String, ProviderError> {
                let bytes = chunk.map_err(ProviderError::Http)?;
                Ok(String::from_utf8_lossy(&bytes).to_string())
            })
            .flat_map(|result: Result<String, ProviderError>| match result {
                Ok(text) => {
                    let mut events: Vec<Result<StreamEvent, ProviderError>> = Vec::new();
                    for line in text.lines() {
                        let line = line.trim();
                        if let Some(data) = line.strip_prefix("data: ") {
                            events.extend(Self::parse_sse_event(data).into_iter().map(Ok));
                        }
                    }
                    stream::iter(events).boxed()
                }
                Err(e) => stream::once(async move { Err(e) }).boxed(),
            });

        Ok(event_stream.boxed())
    }

    async fn complete(&self, request: Request) -> Result<Response, ProviderError> {
        let mut body = self.build_request_body(request);
        body["stream"] = json!(false);

        let url = format!("{}/chat/completions", self.config.base_url);

        let response: reqwest::Response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(ProviderError::Http)?;

        if !response.status().is_success() {
            let status = response.status();
            let body: String = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(ProviderError::ApiError(format!("HTTP {status}: {body}")));
        }

        let resp: OpenAiResponse = response
            .json()
            .await
            .map_err(ProviderError::Http)?;

        let choice = resp
            .choices
            .first()
            .ok_or_else(|| ProviderError::StreamError("No choices in response".to_string()))?;

        let mut content = Vec::new();
        if let Some(ref text) = choice.message.content {
            content.push(ProviderContent::Text { text: text.clone() });
        }

        let stop_reason = match choice.finish_reason.as_deref() {
            Some("tool_calls") => StopReason::ToolUse,
            Some("length") => StopReason::MaxTokens,
            _ => StopReason::EndTurn,
        };

        Ok(Response {
            content,
            usage: Usage {
                input_tokens: resp.usage.prompt_tokens,
                output_tokens: resp.usage.completion_tokens,
            },
            stop_reason,
        })
    }
}

// — OpenAI response types for deserialization —

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiChunk {
    #[serde(default)]
    choices: Vec<OpenAiChunkChoice>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiChunkChoice {
    delta: OpenAiDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiToolCallDelta {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<OpenAiFunctionDelta>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiResponse {
    choices: Vec<OpenAiResponseChoice>,
    usage: OpenAiUsage,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiResponseChoice {
    message: OpenAiMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenAiUsage {
    #[serde(default)]
    prompt_tokens: usize,
    #[serde(default)]
    completion_tokens: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ProviderMessage, ToolDefinition};

    fn test_config() -> OpenAiCompatConfig {
        OpenAiCompatConfig {
            base_url: "https://api.example.com/v1".to_string(),
            api_key: "test-key".to_string(),
            provider_name: "test".to_string(),
            default_model: "test-model".to_string(),
            models: vec![
                ModelInfo {
                    id: "test-model".to_string(),
                    name: "Test Model".to_string(),
                    context_window: 8192,
                    max_output_tokens: 4096,
                    supports_thinking: false,
                    supports_tools: true,
                },
                ModelInfo {
                    id: "test-model-2".to_string(),
                    name: "Test Model 2".to_string(),
                    context_window: 32000,
                    max_output_tokens: 8192,
                    supports_thinking: false,
                    supports_tools: true,
                },
            ],
        }
    }

    #[test]
    fn test_provider_name() {
        let p = OpenAiCompatProvider::new(test_config());
        assert_eq!(p.name(), "test");
    }

    #[test]
    fn test_default_model() {
        let p = OpenAiCompatProvider::new(test_config());
        assert_eq!(p.current_model(), "test-model");
    }

    #[test]
    fn test_set_valid_model() {
        let mut p = OpenAiCompatProvider::new(test_config());
        assert!(p.set_model("test-model-2").is_ok());
        assert_eq!(p.current_model(), "test-model-2");
    }

    #[test]
    fn test_set_invalid_model() {
        let mut p = OpenAiCompatProvider::new(test_config());
        assert!(p.set_model("nonexistent").is_err());
    }

    #[test]
    fn test_build_request_body() {
        let p = OpenAiCompatProvider::new(test_config());
        let request = Request {
            messages: vec![ProviderMessage {
                role: "user".to_string(),
                content: vec![ProviderContent::Text {
                    text: "Hello".to_string(),
                }],
            }],
            tools: vec![],
            system: Some("Be helpful".to_string()),
            max_tokens: Some(1024),
            thinking: None,
        };

        let body = p.build_request_body(request);
        assert_eq!(body["model"], "test-model");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], 1024);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[1]["content"], "Hello");
    }

    #[test]
    fn test_parse_text_delta() {
        let data = r#"{"choices":[{"delta":{"content":"Hello"},"index":0}]}"#;
        let events = OpenAiCompatProvider::parse_sse_event(data);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::TextDelta { delta } => assert_eq!(delta, "Hello"),
            other => panic!("Expected TextDelta, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_tool_call_start() {
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"id":"call_1","function":{"name":"read","arguments":""}}]},"index":0}]}"#;
        let events = OpenAiCompatProvider::parse_sse_event(data);
        assert!(events.iter().any(|e| matches!(e, StreamEvent::ToolCallStart { name, .. } if name == "read")));
    }

    #[test]
    fn test_parse_done() {
        let events = OpenAiCompatProvider::parse_sse_event("[DONE]");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], StreamEvent::Done { reason: StopReason::EndTurn }));
    }

    #[test]
    fn test_parse_finish_reason_tool_calls() {
        let data = r#"{"choices":[{"delta":{},"finish_reason":"tool_calls","index":0}]}"#;
        let events = OpenAiCompatProvider::parse_sse_event(data);
        assert!(events.iter().any(|e| matches!(e, StreamEvent::Done { reason: StopReason::ToolUse })));
    }

    #[test]
    fn test_build_request_with_tools() {
        let p = OpenAiCompatProvider::new(test_config());
        let request = Request {
            messages: vec![],
            tools: vec![ToolDefinition {
                name: "read".to_string(),
                description: "Read a file".to_string(),
                parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            }],
            system: None,
            max_tokens: None,
            thinking: None,
        };

        let body = p.build_request_body(request);
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["function"]["name"], "read");
    }
}
