mod body;
mod chains;

pub use self::body::{provider_model_from_request_body, provider_request_body_with_model};
pub use self::chains::{
    provider_canonical_model, provider_gemini_retain_code_assist_models,
    provider_model_allows_session_memory, provider_model_fallback_chain,
};

#[cfg(test)]
mod tests {
    use super::{
        provider_canonical_model, provider_model_allows_session_memory,
        provider_model_from_request_body, provider_request_body_with_model,
    };
    use crate::ProviderId;

    #[test]
    fn provider_model_body_helpers_preserve_invalid_body() {
        assert_eq!(provider_model_from_request_body(b"not json"), None);
        assert_eq!(
            provider_request_body_with_model(b"not json", "gpt-5"),
            b"not json"
        );
    }

    #[test]
    fn provider_model_body_helpers_extract_and_rewrite_model() {
        let body = br#"{"model":" auto ","input":"hi"}"#;

        assert_eq!(
            provider_model_from_request_body(body).as_deref(),
            Some("auto")
        );

        let rewritten = provider_request_body_with_model(body, "gpt-5.3-codex");
        let value: serde_json::Value = serde_json::from_slice(&rewritten).unwrap();
        assert_eq!(value["model"], "gpt-5.3-codex");
        assert_eq!(value["input"], "hi");
    }

    #[test]
    fn provider_canonical_model_uses_fallback_head() {
        assert_eq!(
            provider_canonical_model(ProviderId::Copilot, "codex"),
            "gpt-5.3-codex"
        );
        assert_eq!(
            provider_canonical_model(ProviderId::OpenAi, "custom-model"),
            "custom-model"
        );
    }

    #[test]
    fn provider_model_session_memory_only_allows_auto_like_models() {
        assert!(provider_model_allows_session_memory("auto"));
        assert!(provider_model_allows_session_memory(" default "));
        assert!(provider_model_allows_session_memory(""));
        assert!(!provider_model_allows_session_memory("claude-sonnet-4-6"));
    }
}
