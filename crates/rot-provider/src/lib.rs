//! rot-provider: LLM provider abstraction and implementations.

mod error;
pub mod providers;
pub mod traits;
pub mod types;

pub use error::ProviderError;
pub use providers::anthropic::AnthropicProvider;
pub use providers::openai::new_openai_provider;
pub use providers::openai_compat::{OpenAiCompatConfig, OpenAiCompatProvider};
pub use providers::zai::new_zai_provider;
pub use traits::Provider;
pub use types::{
    ModelInfo, ProviderContent, ProviderMessage, Request, Response, StopReason, StreamEvent,
    ThinkingConfig, ToolDefinition, Usage,
};
