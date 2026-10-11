#![cfg(test)]

use super::super::provider_bridge::{RuntimeProviderErrorClass, runtime_provider_error_class};
use super::{
    RuntimeProviderBridgeKind, runtime_anthropic_messages_upstream_url,
    runtime_deepseek_anthropic_messages_upstream_url,
    runtime_gemini_openai_compatible_upstream_url,
    runtime_local_rewrite_api_key_attempts_from_start,
    runtime_local_rewrite_apply_direct_api_key_auth, runtime_local_rewrite_copy_openai_headers,
    runtime_local_rewrite_log_url, runtime_local_rewrite_upstream_url,
    runtime_openai_standard_provider_upstream_url,
};
use crate::RuntimeProxyRequest;

#[test]
fn native_messages_api_key_auth_uses_x_api_key_and_chat_auth_uses_bearer() {
    let native = runtime_local_rewrite_apply_direct_api_key_auth(
        reqwest::Client::new().post("https://api.deepseek.com/anthropic/v1/messages"),
        "fixture-deepseek-key",
        true,
    )
    .build()
    .unwrap();
    assert_eq!(native.headers()["x-api-key"], "fixture-deepseek-key");
    assert!(
        !native
            .headers()
            .contains_key(reqwest::header::AUTHORIZATION)
    );

    let chat = runtime_local_rewrite_apply_direct_api_key_auth(
        reqwest::Client::new().post("https://api.deepseek.com/v1/chat/completions"),
        "fixture-deepseek-key",
        false,
    )
    .build()
    .unwrap();
    assert_eq!(
        chat.headers()[reqwest::header::AUTHORIZATION],
        "Bearer fixture-deepseek-key"
    );
    assert!(!chat.headers().contains_key("x-api-key"));
}

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
fn local_rewrite_header_consumer_preserves_metadata_and_filters_hops() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: vec![
            ("Connection".to_string(), "X-Local-Hop".to_string()),
            ("X-Local-Hop".to_string(), "strip-me".to_string()),
            ("Host".to_string(), "client.example".to_string()),
            (
                "X-Prodex-Internal-Trace".to_string(),
                "strip-me".to_string(),
            ),
            (
                "Authorization".to_string(),
                "Bearer caller-token".to_string(),
            ),
            ("x-codex-turn-state".to_string(), "turn-1".to_string()),
            ("User-Agent".to_string(), "codex-fixture".to_string()),
        ],
        body: Vec::new(),
    };
    let upstream = runtime_local_rewrite_copy_openai_headers(
        &request,
        reqwest::Client::new().post("https://upstream.example/v1/responses"),
        true,
    )
    .unwrap()
    .build()
    .unwrap();

    let mut checked = 0;
    for name in ["x-codex-turn-state", "user-agent"] {
        checked += 1;
        assert!(upstream.headers().contains_key(name));
    }
    assert!(checked > 0);
    for name in [
        "connection",
        "x-local-hop",
        "host",
        "x-prodex-internal-trace",
        "authorization",
    ] {
        assert!(!upstream.headers().contains_key(name), "header={name}");
    }
}

#[test]
fn local_rewrite_upstream_url_handles_empty_mount_and_endpoint_without_escape() {
    assert_eq!(runtime_local_rewrite_upstream_url("", "", ""), "");
    assert_eq!(
        runtime_local_rewrite_upstream_url("https://upstream.example", "", "/responses"),
        "https://upstream.example/responses"
    );
    assert_eq!(
        runtime_local_rewrite_upstream_url("https://upstream.example/", "/v1", "/v1"),
        "https://upstream.example"
    );
}

#[test]
fn local_rewrite_transport_keeps_generic_429_passthrough_provider_codes_rotatable() {
    let mut checked = 0;
    for provider in [
        RuntimeProviderBridgeKind::Anthropic,
        RuntimeProviderBridgeKind::Copilot,
        RuntimeProviderBridgeKind::OpenAiResponses,
        RuntimeProviderBridgeKind::DeepSeek,
        RuntimeProviderBridgeKind::Gemini,
        RuntimeProviderBridgeKind::Kiro,
    ] {
        checked += 1;
        assert_eq!(
            runtime_provider_error_class(provider, 429, b"too many requests"),
            RuntimeProviderErrorClass::Other,
            "generic 429 must pass through for {provider:?}"
        );
    }
    assert!(checked > 0);
    assert_eq!(
        runtime_provider_error_class(
            RuntimeProviderBridgeKind::OpenAiResponses,
            429,
            br#"{"error":{"code":"rate_limit_exceeded"}}"#,
        ),
        RuntimeProviderErrorClass::RateLimit
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
