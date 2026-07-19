#![cfg(test)]

use super::{
    RuntimeProviderBridgeKind, runtime_anthropic_messages_upstream_url,
    runtime_deepseek_anthropic_messages_upstream_url,
    runtime_gemini_openai_compatible_upstream_url,
    runtime_local_rewrite_api_key_attempts_from_start, runtime_local_rewrite_log_url,
    runtime_local_rewrite_upstream_url, runtime_openai_standard_provider_upstream_url,
};

#[test]
fn openai_standard_provider_upstream_url_uses_contract_formats() {
    assert_eq!(
        runtime_openai_standard_provider_upstream_url(
            RuntimeProviderBridgeKind::OpenAiResponses,
            "https://upstream.test/v1",
            "/v1",
            "/v1/responses"
        ),
        "https://upstream.test/v1/responses"
    );
    assert_eq!(
        runtime_openai_standard_provider_upstream_url(
            RuntimeProviderBridgeKind::DeepSeek,
            "https://upstream.test/v1",
            "/v1",
            "/v1/responses"
        ),
        "https://upstream.test/v1/chat/completions"
    );
    assert_eq!(
        runtime_openai_standard_provider_upstream_url(
            RuntimeProviderBridgeKind::Copilot,
            "https://upstream.test/v1",
            "/v1",
            "/v1/chat/completions"
        ),
        "https://upstream.test/v1/chat/completions"
    );
    for path in [
        "/v1/embeddings",
        "/v1/images/generations",
        "/v1/audio/transcriptions",
        "/v1/batches",
        "/v1/rerank",
        "/v1/a2a",
        "/v1/messages",
    ] {
        assert_eq!(
            runtime_openai_standard_provider_upstream_url(
                RuntimeProviderBridgeKind::OpenAiResponses,
                "https://upstream.test/v1",
                "/v1",
                path
            ),
            format!("https://upstream.test{path}")
        );
    }
}

#[test]
fn anthropic_messages_upstream_url_uses_native_endpoint() {
    assert_eq!(
        runtime_anthropic_messages_upstream_url("https://api.anthropic.com/v1", "/v1"),
        "https://api.anthropic.com/v1/messages"
    );
}

#[test]
fn deepseek_anthropic_messages_upstream_url_uses_documented_native_endpoint() {
    for base_url in [
        "https://api.deepseek.com",
        "https://api.deepseek.com/v1",
        "https://api.deepseek.com/beta",
        "https://api.deepseek.com/anthropic",
        "https://api.deepseek.com/anthropic/v1",
    ] {
        assert_eq!(
            runtime_deepseek_anthropic_messages_upstream_url(base_url),
            "https://api.deepseek.com/anthropic/v1/messages"
        );
    }
}

#[test]
fn local_rewrite_upstream_url_neutralizes_dot_segments_before_url_parsing_can_escape_base() {
    assert_eq!(
        runtime_local_rewrite_upstream_url("https://upstream.test/v1", "/v1", "/v1/../admin?x=1"),
        "https://upstream.test/v1/%252e%252e/admin?x=1"
    );
    assert_eq!(
        runtime_local_rewrite_upstream_url("https://upstream.test/v1", "/v1", "/v1/%2e%2e/admin"),
        "https://upstream.test/v1/%252e%252e/admin"
    );
}

#[test]
fn gemini_openai_compatible_url_uses_documented_chat_completions_endpoint() {
    assert_eq!(
        runtime_gemini_openai_compatible_upstream_url(
            "https://generativelanguage.googleapis.com/v1beta"
        ),
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
    );
    assert_eq!(
        runtime_gemini_openai_compatible_upstream_url(
            "https://generativelanguage.googleapis.com/v1beta/openai/"
        ),
        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
    );
}

#[test]
fn api_key_attempts_rotate_start_and_include_all_keys() {
    let api_keys = vec![
        "first".to_string(),
        "second".to_string(),
        "third".to_string(),
    ];

    let attempts = runtime_local_rewrite_api_key_attempts_from_start(&api_keys, 1);

    assert_eq!(
        attempts,
        vec![
            ("api-key-2".to_string(), "second"),
            ("api-key-3".to_string(), "third"),
            ("api-key-1".to_string(), "first"),
        ]
    );
}

#[test]
fn single_api_key_attempt_uses_generic_label() {
    let api_keys = vec!["only".to_string()];

    let attempts = runtime_local_rewrite_api_key_attempts_from_start(&api_keys, 0);

    assert_eq!(attempts, vec![("api-key".to_string(), "only")]);
}

#[test]
fn log_url_strips_query_fragment_and_userinfo() {
    let url = runtime_local_rewrite_log_url(
        "https://user:secret@example.test/v1/responses?access_token=secret#frag",
    );
    assert_eq!(url, "https://example.test/v1/responses");
    assert_eq!(
        runtime_local_rewrite_log_url("/v1/responses?key=secret"),
        "/v1/responses"
    );
}
