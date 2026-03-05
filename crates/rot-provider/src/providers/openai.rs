//! OpenAI provider built on the OpenAI-compatible transport.

use crate::providers::openai_compat::{OpenAiCompatConfig, OpenAiCompatProvider};
use crate::types::ModelInfo;

const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// Create a new OpenAI provider.
///
/// Set the `OPENAI_API_KEY` environment variable.
pub fn new_openai_provider(api_key: String, custom_models: Vec<ModelInfo>) -> OpenAiCompatProvider {
    let mut models = vec![
            ModelInfo {
                id: "gpt-4o".to_string(),
                name: "GPT-4o".to_string(),
                context_window: 128_000,
                max_output_tokens: 16_384,
                supports_thinking: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "gpt-4o-mini".to_string(),
                name: "GPT-4o mini".to_string(),
                context_window: 128_000,
                max_output_tokens: 16_384,
                supports_thinking: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "o1".to_string(),
                name: "o1".to_string(),
                context_window: 200_000,
                max_output_tokens: 32_768,
                supports_thinking: true, // Internal thinking handled by API
                supports_tools: true, // Only recently supported via function calling
            },
            ModelInfo {
                id: "o1-mini".to_string(),
                name: "o1-mini".to_string(),
                context_window: 128_000,
                max_output_tokens: 65_536,
                supports_thinking: true, // Internal thinking
                supports_tools: false,
            },
            ModelInfo {
                id: "o3-mini".to_string(),
                name: "o3-mini".to_string(),
                context_window: 200_000,
                max_output_tokens: 100_000,
                supports_thinking: true, // Internal thinking
                supports_tools: true,
            },
        ];
    
    models.extend(custom_models);

    let config = OpenAiCompatConfig {
        base_url: OPENAI_BASE_URL.to_string(),
        api_key,
        provider_name: "openai".to_string(),
        default_model: "gpt-4o".to_string(),
        models,
    };

    OpenAiCompatProvider::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Provider;

    #[test]
    fn test_openai_provider_name() {
        let p = new_openai_provider("test-key".to_string(), vec![]);
        assert_eq!(p.name(), "openai");
    }

    #[test]
    fn test_openai_default_model() {
        let p = new_openai_provider("test-key".to_string(), vec![]);
        assert_eq!(p.current_model(), "gpt-4o");
    }

    #[test]
    fn test_openai_models() {
        let p = new_openai_provider("test-key".to_string(), vec![]);
        let models = p.models();
        assert!(models.len() >= 2);
        assert!(models.iter().any(|m| m.id == "gpt-4o"));
        assert!(models.iter().any(|m| m.id == "gpt-4o-mini"));
        assert!(models.iter().any(|m| m.id == "o3-mini"));
    }

    #[test]
    fn test_openai_set_model() {
        let mut p = new_openai_provider("test-key".to_string(), vec![]);
        assert!(p.set_model("gpt-4o-mini").is_ok());
        assert_eq!(p.current_model(), "gpt-4o-mini");
    }
}
