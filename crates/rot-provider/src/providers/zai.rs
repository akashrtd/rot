//! z.ai (Zhipu AI) provider — GLM-5 and GLM-4.7 models.
//!
//! Uses the OpenAI-compatible chat completions API at `https://api.z.ai/api/coding/paas/v4`.

use crate::providers::openai_compat::{OpenAiCompatConfig, OpenAiCompatProvider};
use crate::types::ModelInfo;

const ZAI_BASE_URL: &str = "https://api.z.ai/api/coding/paas/v4";

/// Create a new z.ai provider.
///
/// Uses the GLM Coding Plan endpoint. Set the `ZAI_API_KEY` environment variable.
pub fn new_zai_provider(api_key: String) -> OpenAiCompatProvider {
    let config = OpenAiCompatConfig {
        base_url: ZAI_BASE_URL.to_string(),
        api_key,
        provider_name: "zai".to_string(),
        default_model: "glm-5".to_string(),
        models: vec![
            ModelInfo {
                id: "glm-5".to_string(),
                name: "GLM-5".to_string(),
                context_window: 128_000,
                max_output_tokens: 16_384,
                supports_thinking: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "glm-4.7".to_string(),
                name: "GLM-4.7".to_string(),
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
    fn test_zai_provider_name() {
        let p = new_zai_provider("test-key".to_string());
        assert_eq!(p.name(), "zai");
    }

    #[test]
    fn test_zai_default_model() {
        let p = new_zai_provider("test-key".to_string());
        assert_eq!(p.current_model(), "glm-5");
    }

    #[test]
    fn test_zai_models() {
        let p = new_zai_provider("test-key".to_string());
        let models = p.models();
        assert_eq!(models.len(), 2);
        assert!(models.iter().any(|m| m.id == "glm-5"));
        assert!(models.iter().any(|m| m.id == "glm-4.7"));
    }

    #[test]
    fn test_zai_set_model() {
        let mut p = new_zai_provider("test-key".to_string());
        assert!(p.set_model("glm-4.7").is_ok());
        assert_eq!(p.current_model(), "glm-4.7");
    }
}
