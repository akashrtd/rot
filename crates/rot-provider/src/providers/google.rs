//! Google provider built on the OpenAI-compatible transport endpoint.

use crate::providers::openai_compat::{OpenAiCompatConfig, OpenAiCompatProvider};
use crate::types::ModelInfo;

const GOOGLE_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/openai";

/// Create a new Google provider.
pub fn new_google_provider(api_key: String) -> OpenAiCompatProvider {
    let config = OpenAiCompatConfig {
        base_url: GOOGLE_BASE_URL.to_string(),
        api_key,
        provider_name: "google".to_string(),
        default_model: "gemini-2.5-flash".to_string(),
        models: vec![
            ModelInfo {
                id: "gemini-2.5-flash".to_string(),
                name: "Gemini 2.5 Flash".to_string(),
                context_window: 1_000_000,
                max_output_tokens: 8_192,
                supports_thinking: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "gemini-2.5-pro".to_string(),
                name: "Gemini 2.5 Pro".to_string(),
                context_window: 1_000_000,
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
    fn test_google_provider_name() {
        let p = new_google_provider("test-key".to_string());
        assert_eq!(p.name(), "google");
    }

    #[test]
    fn test_google_default_model() {
        let p = new_google_provider("test-key".to_string());
        assert_eq!(p.current_model(), "gemini-2.5-flash");
    }

    #[test]
    fn test_google_set_model() {
        let mut p = new_google_provider("test-key".to_string());
        assert!(p.set_model("gemini-2.5-pro").is_ok());
        assert_eq!(p.current_model(), "gemini-2.5-pro");
    }
}
