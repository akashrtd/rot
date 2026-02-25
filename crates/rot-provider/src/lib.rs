//! rot-provider: LLM provider abstraction and implementations.

mod error;
pub mod traits;
pub mod types;

pub use error::ProviderError;
pub use traits::Provider;
pub use types::{
    ModelInfo, ProviderContent, ProviderMessage, Request, Response, StopReason, StreamEvent,
    ThinkingConfig, ToolDefinition, Usage,
};
