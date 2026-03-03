//! Scripted mock provider for deterministic integration tests.

use crate::error::ProviderError;
use crate::traits::Provider;
use crate::types::{
    ModelInfo, ProviderContent, Request, Response, StopReason, StreamEvent, Usage,
};
use async_trait::async_trait;
use futures::stream::{self, BoxStream, StreamExt};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A deterministic provider that emits scripted text responses.
#[derive(Clone)]
pub struct MockProvider {
    model: String,
    responses: Arc<Mutex<VecDeque<String>>>,
}

impl MockProvider {
    /// Create a mock provider with scripted responses.
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            model: "mock-model".to_string(),
            responses: Arc::new(Mutex::new(responses.into())),
        }
    }
}

/// Create a new mock provider instance.
pub fn new_mock_provider(responses: Vec<String>) -> MockProvider {
    MockProvider::new(responses)
}

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
        &self.model
    }

    fn set_model(&mut self, model: &str) -> Result<(), ProviderError> {
        self.model = model.to_string();
        Ok(())
    }

    async fn stream(
        &self,
        _request: Request,
    ) -> Result<BoxStream<'_, Result<StreamEvent, ProviderError>>, ProviderError> {
        let delta = self
            .responses
            .lock()
            .map_err(|e| ProviderError::ApiError(format!("mock provider lock poisoned: {e}")))?
            .pop_front()
            .unwrap_or_default();

        Ok(stream::iter(vec![
            Ok(StreamEvent::TextDelta { delta }),
            Ok(StreamEvent::Done {
                reason: StopReason::EndTurn,
            }),
        ])
        .boxed())
    }

    async fn complete(&self, _request: Request) -> Result<Response, ProviderError> {
        let text = self
            .responses
            .lock()
            .map_err(|e| ProviderError::ApiError(format!("mock provider lock poisoned: {e}")))?
            .pop_front()
            .unwrap_or_default();

        Ok(Response {
            content: vec![ProviderContent::Text { text }],
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
            },
        })
    }
}
