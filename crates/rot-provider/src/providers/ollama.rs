//! Ollama provider built on the OpenAI-compatible transport.

use crate::providers::openai_compat::{OpenAiCompatConfig, OpenAiCompatProvider};
use crate::types::ModelInfo;

const OLLAMA_BASE_URL: &str = "http://localhost:11434/v1";

/// Create a new Ollama provider.
///
/// Uses the OpenAI-compatible endpoint exposed by Ollama.
pub fn new_ollama_provider(api_key: String) -> OpenAiCompatProvider {
    let config = OpenAiCompatConfig {
        base_url: std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| OLLAMA_BASE_URL.to_string()),
        api_key,
        provider_name: "ollama".to_string(),
        default_model: "llama3.1".to_string(),
        models: vec![
            ModelInfo {
                id: "llama3.1".to_string(),
                name: "Llama 3.1".to_string(),
                context_window: 128_000,
                max_output_tokens: 8_192,
                supports_thinking: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "qwen2.5-coder".to_string(),
                name: "Qwen 2.5 Coder".to_string(),
                context_window: 128_000,
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
    fn test_ollama_provider_name() {
        let p = new_ollama_provider(String::new());
        assert_eq!(p.name(), "ollama");
    }

    #[test]
    fn test_ollama_default_model() {
        let p = new_ollama_provider(String::new());
        assert_eq!(p.current_model(), "llama3.1");
    }

    #[test]
    fn test_ollama_set_model() {
        let mut p = new_ollama_provider(String::new());
        assert!(p.set_model("qwen2.5-coder").is_ok());
        assert_eq!(p.current_model(), "qwen2.5-coder");
    }
}
