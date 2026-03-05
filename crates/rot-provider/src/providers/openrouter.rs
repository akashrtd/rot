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
            ModelInfo {
                id: "deepseek/deepseek-coder".to_string(),
                name: "DeepSeek Coder V3".to_string(),
                context_window: 64_000,
                max_output_tokens: 8_192,
                supports_thinking: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "deepseek/deepseek-r1".to_string(),
                name: "DeepSeek R1 (Reasoning)".to_string(),
                context_window: 64_000,
                max_output_tokens: 8_192,
                supports_thinking: true,
                supports_tools: false,
            },
            ModelInfo {
                id: "meta-llama/llama-3.3-70b-instruct".to_string(),
                name: "Llama 3.3 70B Instruct".to_string(),
                context_window: 128_000,
                max_output_tokens: 8_192,
                supports_thinking: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "google/gemini-2.5-pro".to_string(),
                name: "Gemini 2.5 Pro".to_string(),
                context_window: 1_000_000,
                max_output_tokens: 8_192,
                supports_thinking: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "openai/o3-mini".to_string(),
                name: "OpenAI o3-mini".to_string(),
                context_window: 200_000,
                max_output_tokens: 100_000,
                supports_thinking: true,
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
