//! Integration tests for rot.
//!
//! These tests verify that the core components work together correctly
//! without requiring a live API key.

use rot_core::{Agent, AgentConfig, ContentBlock, Message, RuntimeSecurityConfig};
use rot_provider::*;
use rot_tools::ToolRegistry;

use async_trait::async_trait;
use futures::stream::{self, BoxStream, StreamExt};

// -- Mock provider for integration tests --

struct MockProvider;

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "mock-model".to_string(),
            name: "Mock Model".to_string(),
            context_window: 8192,
            max_output_tokens: 4096,
            supports_thinking: false,
            supports_tools: true,
        }]
    }

    fn current_model(&self) -> &str {
        "mock-model"
    }

    fn set_model(&mut self, _model: &str) -> Result<(), ProviderError> {
        Ok(())
    }

    async fn stream(
        &self,
        _request: Request,
    ) -> Result<BoxStream<'_, Result<StreamEvent, ProviderError>>, ProviderError> {
        let events = vec![
            Ok(StreamEvent::TextDelta {
                delta: "Hello from mock!".to_string(),
            }),
            Ok(StreamEvent::Done {
                reason: StopReason::EndTurn,
            }),
        ];
        Ok(stream::iter(events).boxed())
    }

    async fn complete(&self, _request: Request) -> Result<Response, ProviderError> {
        Ok(Response {
            content: vec![ProviderContent::Text {
                text: "Hello from mock!".to_string(),
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
        })
    }
}

// -- Integration tests --

#[tokio::test]
async fn test_agent_with_mock_provider() {
    let provider = Box::new(MockProvider);
    let tools = ToolRegistry::new();
    let config = AgentConfig {
        system_prompt: Some("You are a test assistant.".to_string()),
        max_tokens: Some(1024),
        ..Default::default()
    };

    let agent = std::sync::Arc::new(Agent::new(
        provider,
        tools,
        config,
        RuntimeSecurityConfig::default(),
    ));
    let mut messages: Vec<Message> = Vec::new();

    let response = agent.process(&mut messages, "hello").await;
    assert!(response.is_ok());

    let msg = response.unwrap();
    let text: String = msg
        .content
        .iter()
        .filter_map(|c| {
            if let ContentBlock::Text { text } = c {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("");
    assert!(!text.is_empty());
}

#[tokio::test]
async fn test_agent_with_tools_registered() {
    let provider = Box::new(MockProvider);
    let mut tools = ToolRegistry::new();
    rot_tools::register_all(&mut tools);

    let config = AgentConfig::default();
    let agent = std::sync::Arc::new(Agent::new(
        provider,
        tools,
        config,
        RuntimeSecurityConfig::default(),
    ));
    let mut messages: Vec<Message> = Vec::new();

    let response = agent.process(&mut messages, "test with tools").await;
    assert!(response.is_ok());
}

#[tokio::test]
async fn test_session_create_and_load() {
    use rot_session::SessionStore;

    let store = SessionStore::new();
    let dir = tempfile::tempdir().unwrap();

    let session = store
        .create(dir.path(), "mock-model", "mock")
        .await
        .unwrap();
    assert!(!session.id.is_empty());

    let sessions = store.list_recent(dir.path(), 10).await.unwrap();
    assert!(!sessions.is_empty());
}

#[tokio::test]
async fn test_tool_registry_has_all_builtin_tools() {
    let mut tools = ToolRegistry::new();
    rot_tools::register_all(&mut tools);

    let definitions = tools.tool_definitions();
    let names: Vec<&str> = definitions
        .iter()
        .filter_map(|d| d["name"].as_str())
        .collect();

    assert!(names.contains(&"read"), "Missing read tool");
    assert!(names.contains(&"write"), "Missing write tool");
    assert!(names.contains(&"edit"), "Missing edit tool");
    assert!(names.contains(&"bash"), "Missing bash tool");
    assert!(names.contains(&"glob"), "Missing glob tool");
    assert!(names.contains(&"grep"), "Missing grep tool");
    assert!(names.contains(&"task"), "Missing task tool");
    assert!(names.contains(&"webfetch"), "Missing webfetch tool");
}

#[tokio::test]
async fn test_rlm_context_manager() {
    use rot_rlm::ContextManager;

    let dir = tempfile::tempdir().unwrap();
    let mut ctx = ContextManager::with_dir(dir.path());

    let var = ctx
        .store("some large content here")
        .await
        .unwrap();
    assert!(!var.is_empty());

    let loaded = ctx.load(&var).await.unwrap();
    assert_eq!(loaded, "some large content here");
}

#[test]
fn test_provider_anthropic_creation() {
    let provider = AnthropicProvider::new("test-key".to_string());
    assert_eq!(provider.name(), "anthropic");
    assert_eq!(provider.current_model(), "claude-sonnet-4-20250514");
}

#[test]
fn test_provider_zai_creation() {
    let provider = new_zai_provider("test-key".to_string());
    assert_eq!(provider.name(), "zai");
    assert_eq!(provider.current_model(), "glm-5");
}

#[test]
fn test_all_providers_have_models() {
    let anthropic = AnthropicProvider::new("key".to_string());
    assert!(!anthropic.models().is_empty());

    let zai = new_zai_provider("key".to_string());
    assert!(!zai.models().is_empty());
}
