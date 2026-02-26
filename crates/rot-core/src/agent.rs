//! Agent loop implementation.
//!
//! The agent loop is the core processing cycle:
//! 1. Send messages to the provider
//! 2. Parse the streaming response
//! 3. Execute any tool calls
//! 4. Repeat until done or max iterations reached

use crate::message::{ContentBlock, Message, Role};
use crate::permission::{ApprovalResponse, PermissionSystem};
use crate::security::{RuntimeSecurityConfig, SandboxMode};
use futures::StreamExt;
use rot_provider::{
    Provider, ProviderContent, ProviderError, ProviderMessage, Request, StopReason, StreamEvent,
    ToolDefinition,
};
use rot_tools::{ToolContext, ToolRegistry};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

const MAX_ITERATIONS: usize = 50;

/// Agent configuration.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum number of tool-use iterations before stopping.
    pub max_iterations: usize,
    /// System prompt.
    pub system_prompt: Option<String>,
    /// Maximum tokens per response.
    pub max_tokens: Option<usize>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: MAX_ITERATIONS,
            system_prompt: None,
            max_tokens: None,
        }
    }
}

/// Callback for streaming events.
pub type EventCallback = Box<dyn Fn(&StreamEvent) + Send + Sync>;

/// Callback to request interactive approval from the user before running a tool.
pub type ApprovalCallback = Box<
    dyn Fn(
            &str, // Tool name
            &serde_json::Value, // Tool arguments
        ) -> Pin<Box<dyn Future<Output = ApprovalResponse> + Send>>
        + Send
        + Sync,
>;

/// The main agent that orchestrates LLM calls and tool execution.
pub struct Agent {
    provider: Box<dyn Provider>,
    tools: ToolRegistry,
    config: AgentConfig,
    runtime_security: RuntimeSecurityConfig,
    on_event: Option<EventCallback>,
    on_approval: Option<ApprovalCallback>,
    permission_system: Arc<Mutex<PermissionSystem>>,
}

impl Agent {
    /// Create a new agent.
    pub fn new(
        provider: Box<dyn Provider>,
        tools: ToolRegistry,
        config: AgentConfig,
        runtime_security: RuntimeSecurityConfig,
    ) -> Self {
        let permission_system = PermissionSystem::new(runtime_security.approval_policy);
        Self {
            provider,
            tools,
            config,
            runtime_security,
            on_event: None,
            on_approval: None,
            permission_system: Arc::new(Mutex::new(permission_system)),
        }
    }

    /// Set the event callback for streaming updates.
    pub fn on_event(mut self, callback: EventCallback) -> Self {
        self.on_event = Some(callback);
        self
    }

    /// Set the approval callback for interactive permission requests.
    pub fn on_approval(mut self, callback: ApprovalCallback) -> Self {
        self.on_approval = Some(callback);
        self
    }

    /// Process user input and return the assistant's response.
    ///
    /// This runs the full agent loop: send to provider → parse response →
    /// execute tools → send tool results → repeat until done.
    pub async fn process(
        &self,
        messages: &mut Vec<Message>,
        user_input: &str,
    ) -> Result<Message, AgentProcessError> {
        // Add user message
        let user_msg = Message::user(user_input);
        messages.push(user_msg);

        let tool_ctx = ToolContext {
            working_dir: std::env::current_dir().unwrap_or_default(),
            session_id: String::new(),
            timeout: std::time::Duration::from_secs(120),
            sandbox_mode: match self.runtime_security.sandbox_mode {
                SandboxMode::ReadOnly => rot_tools::SandboxMode::ReadOnly,
                SandboxMode::WorkspaceWrite => rot_tools::SandboxMode::WorkspaceWrite,
                SandboxMode::DangerFullAccess => rot_tools::SandboxMode::DangerFullAccess,
            },
            network_access: self.runtime_security.sandbox_network_access
                || self.runtime_security.sandbox_mode == SandboxMode::DangerFullAccess,
        };

        for _iteration in 0..self.config.max_iterations {
            // Build provider request
            let provider_messages = self.convert_messages(messages);
            let tool_defs = self.build_tool_definitions();

            let request = Request {
                messages: provider_messages,
                tools: tool_defs,
                system: self.config.system_prompt.clone(),
                max_tokens: self.config.max_tokens,
                thinking: None,
            };

            // Stream the response
            let mut stream = self
                .provider
                .stream(request)
                .await
                .map_err(AgentProcessError::Provider)?;

            let mut text_content = String::new();
            let mut tool_calls: Vec<PendingToolCall> = Vec::new();
            let mut current_tool: Option<PendingToolCall> = None;
            let mut stop_reason = StopReason::EndTurn;

            while let Some(event) = stream.next().await {
                let event = event.map_err(AgentProcessError::Provider)?;

                // Notify callback
                if let Some(ref cb) = self.on_event {
                    cb(&event);
                }

                match event {
                    StreamEvent::TextDelta { delta } => {
                        text_content.push_str(&delta);
                    }
                    StreamEvent::ToolCallStart { id, name } => {
                        current_tool = Some(PendingToolCall {
                            id,
                            name,
                            arguments: String::new(),
                        });
                    }
                    StreamEvent::ToolCallDelta { delta, .. } => {
                        if let Some(ref mut tc) = current_tool {
                            tc.arguments.push_str(&delta);
                        }
                    }
                    StreamEvent::ToolCallEnd { .. } => {
                        if let Some(tc) = current_tool.take() {
                            tool_calls.push(tc);
                        }
                    }
                    StreamEvent::Done { reason } => {
                        stop_reason = reason;
                        break;
                    }
                    _ => {}
                }
            }

            // Build assistant message
            let mut content_blocks = Vec::new();
            if !text_content.is_empty() {
                content_blocks.push(ContentBlock::Text {
                    text: text_content,
                });
            }
            for tc in &tool_calls {
                let args: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);
                content_blocks.push(ContentBlock::ToolCall {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    arguments: args,
                });
            }

            let assistant_msg = Message::assistant(content_blocks);
            messages.push(assistant_msg);

            // If no tool calls, we're done
            if tool_calls.is_empty() || stop_reason != StopReason::ToolUse {
                return Ok(messages.last().cloned().unwrap());
            }

            // Execute tool calls and add results
            for tc in &tool_calls {
                let args: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or(serde_json::Value::Null);

                // Permission check
                let (is_denied, requires_approval) = {
                    let lock = self.permission_system.lock().unwrap();
                    (lock.is_denied(&tc.name), lock.requires_approval(&tc.name))
                };
                
                if is_denied {
                    let tool_msg = Message::tool_result(
                        tc.id.clone(),
                        format!("Execution of '{}' is permanently denied for this session.", tc.name),
                        true,
                    );
                    messages.push(tool_msg);
                    continue;
                }

                if requires_approval {
                    if let Some(ref approval_cb) = self.on_approval {
                        let response = approval_cb(&tc.name, &args).await;
                        self.permission_system.lock().unwrap().handle_response(&tc.name, &response);

                        match response {
                            ApprovalResponse::DenyOnce | ApprovalResponse::DenyAlways => {
                                let tool_msg = Message::tool_result(
                                    tc.id.clone(),
                                    format!("User denied permission to run '{}'", tc.name),
                                    true,
                                );
                                messages.push(tool_msg);
                                continue;
                            }
                            _ => {} // Allowed, proceed to execute
                        }
                    } else {
                        // If no callback is hooked up but approval is required, fail safe.
                        let tool_msg = Message::tool_result(
                            tc.id.clone(),
                            format!("Cannot execute '{}': No interactive approval handler configured.", tc.name),
                            true,
                        );
                        messages.push(tool_msg);
                        continue;
                    }
                }

                let result = if let Some(tool) = self.tools.get(&tc.name) {
                    match tool.execute(args, &tool_ctx).await {
                        Ok(result) => result,
                        Err(e) => rot_tools::ToolResult::error(format!("Tool error: {e}")),
                    }
                } else {
                    rot_tools::ToolResult::error(format!("Unknown tool: {}", tc.name))
                };

                let tool_msg = Message::tool_result_with_metadata(
                    tc.id.clone(),
                    result.output,
                    result.is_error,
                    result.metadata,
                );
                messages.push(tool_msg);
            }

            // Continue the loop — provider will see tool results
        }

        Err(AgentProcessError::MaxIterations(
            self.config.max_iterations,
        ))
    }

    /// Convert internal messages to provider format.
    fn convert_messages(&self, messages: &[Message]) -> Vec<ProviderMessage> {
        messages
            .iter()
            .filter(|m| m.role != Role::System) // System is handled separately
            .map(|msg| {
                let role = match msg.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "user", // Tool results sent as user messages
                    Role::System => "system",
                };

                let content = msg
                    .content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => ProviderContent::Text {
                            text: text.clone(),
                        },
                        ContentBlock::ToolCall {
                            id,
                            name,
                            arguments,
                        } => ProviderContent::ToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            arguments: arguments.clone(),
                        },
                        ContentBlock::ToolResult {
                            tool_call_id,
                            content,
                            is_error,
                            ..
                        } => ProviderContent::ToolResult {
                            tool_call_id: tool_call_id.clone(),
                            content: content.clone(),
                            is_error: *is_error,
                        },
                        ContentBlock::Image { data, mime_type } => ProviderContent::Image {
                            data: data.clone(),
                            mime_type: mime_type.clone(),
                        },
                        ContentBlock::Thinking { .. } => ProviderContent::Text {
                            text: String::new(),
                        },
                    })
                    .collect();

                ProviderMessage {
                    role: role.to_string(),
                    content,
                }
            })
            .collect()
    }

    /// Build tool definitions for the provider.
    fn build_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .all()
            .iter()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters_schema(),
            })
            .collect()
    }
}

/// Pending tool call being accumulated from streaming events.
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

/// Errors that can occur during agent processing.
#[derive(Debug, thiserror::Error)]
pub enum AgentProcessError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("Max iterations ({0}) reached")]
    MaxIterations(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_iterations, MAX_ITERATIONS);
        assert!(config.system_prompt.is_none());
    }

    #[test]
    fn test_convert_messages() {
        let agent = Agent::new(
            // We can't easily create a mock provider here without more infrastructure,
            // but we can test the conversion logic by creating a minimal test.
            // For now, just verify the config and types compile.
            Box::new(DummyProvider),
            ToolRegistry::new(),
            AgentConfig::default(),
            RuntimeSecurityConfig {
                approval_policy: crate::security::ApprovalPolicy::Never,
                ..RuntimeSecurityConfig::default()
            },
        );

        let messages = vec![
            Message::user("Hello"),
            Message::assistant(vec![ContentBlock::Text {
                text: "Hi!".to_string(),
            }]),
        ];

        let converted = agent.convert_messages(&messages);
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "user");
        assert_eq!(converted[1].role, "assistant");
    }

    // Minimal dummy provider for testing conversion logic
    struct DummyProvider;

    #[async_trait::async_trait]
    impl Provider for DummyProvider {
        fn name(&self) -> &str {
            "dummy"
        }
        fn models(&self) -> Vec<rot_provider::ModelInfo> {
            vec![]
        }
        fn current_model(&self) -> &str {
            "dummy"
        }
        fn set_model(&mut self, _: &str) -> Result<(), ProviderError> {
            Ok(())
        }
        async fn stream(
            &self,
            _: Request,
        ) -> Result<futures::stream::BoxStream<'_, Result<StreamEvent, ProviderError>>, ProviderError>
        {
            Ok(Box::pin(futures::stream::empty()))
        }
        async fn complete(
            &self,
            _: Request,
        ) -> Result<rot_provider::Response, ProviderError> {
            unimplemented!()
        }
    }
}
