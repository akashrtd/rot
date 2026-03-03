//! OpenRouter provider built on the OpenAI-compatible transport.

use crate::providers::openai_compat::{OpenAiCompatConfig, OpenAiCompatProvider};
use crate::types::ModelInfo;

const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Create a new OpenRouter provider.
pub fn new_openrouter_provider(api_key: String) -> OpenAiCompatProvider {
    let config = OpenAiCompatConfig {
        base_url: OPENROUTER_BASE_URL.to_string(),
        api_key,
        provider_name: "openrouter".to_string(),
        default_model: "openai/gpt-4o-mini".to_string(),
        models: vec![
            ModelInfo {
                id: "openai/gpt-4o-mini".to_string(),
                name: "OpenAI GPT-4o mini (via OpenRouter)".to_string(),
                context_window: 128_000,
                max_output_tokens: 16_384,
                supports_thinking: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "anthropic/claude-3.5-sonnet".to_string(),
                name: "Anthropic Claude 3.5 Sonnet (via OpenRouter)".to_string(),
                context_window: 200_000,
                max_output_tokens: 8_192,
                supports_thinking: false,
                supports_tools: true,
            },
        ],
    };

    OpenAiCompatProvider::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Provider;

    #[test]
    fn test_openrouter_provider_name() {
        let p = new_openrouter_provider("test-key".to_string());
        assert_eq!(p.name(), "openrouter");
    }

    #[test]
    fn test_openrouter_default_model() {
        let p = new_openrouter_provider("test-key".to_string());
        assert_eq!(p.current_model(), "openai/gpt-4o-mini");
    }

    #[test]
    fn test_openrouter_set_model() {
        let mut p = new_openrouter_provider("test-key".to_string());
        assert!(p.set_model("anthropic/claude-3.5-sonnet").is_ok());
        assert_eq!(p.current_model(), "anthropic/claude-3.5-sonnet");
    }
}
