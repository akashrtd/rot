//! Provider trait definition.

use crate::error::ProviderError;
use crate::types::{ModelInfo, Request, Response, StreamEvent};
use async_trait::async_trait;
use futures::stream::BoxStream;

/// Trait for LLM provider implementations.
///
/// Providers handle communication with different LLM APIs (Anthropic, OpenAI, etc.)
/// and normalize their responses into a common format.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Provider name (e.g., "anthropic", "openai").
    fn name(&self) -> &str;

    /// List of models supported by this provider.
    fn models(&self) -> Vec<ModelInfo>;

    /// Currently selected model identifier.
    fn current_model(&self) -> &str;

    /// Switch to a different model.
    fn set_model(&mut self, model: &str) -> Result<(), ProviderError>;

    /// Send a streaming request to the provider.
    ///
    /// Returns a stream of `StreamEvent`s as the model generates its response.
    async fn stream(
        &self,
        request: Request,
    ) -> Result<BoxStream<'_, Result<StreamEvent, ProviderError>>, ProviderError>;

    /// Send a non-streaming request to the provider.
    ///
    /// Collects the full response before returning.
    async fn complete(&self, request: Request) -> Result<Response, ProviderError>;
}

// Compile-time check: Provider must be object-safe
const _: () = {
    fn _assert_object_safe(_: &dyn Provider) {}
};
