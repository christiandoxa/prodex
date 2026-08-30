use super::super::super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode;
use super::*;

#[test]
fn native_web_search_route_is_selected_only_for_native_modes_with_options() {
    let body = br#"{"web_search_options":{}}"#;
    assert!(runtime_deepseek_uses_native_web_search(
        RuntimeDeepSeekWebSearchMode::Auto,
        body
    ));
    assert!(runtime_deepseek_uses_native_web_search(
        RuntimeDeepSeekWebSearchMode::Anthropic,
        body
    ));
    assert!(!runtime_deepseek_uses_native_web_search(
        RuntimeDeepSeekWebSearchMode::OpenAiChat,
        body
    ));
    assert!(!runtime_deepseek_uses_native_web_search(
        RuntimeDeepSeekWebSearchMode::Auto,
        br#"{}"#
    ));
}

#[test]
fn auto_native_translation_falls_back_without_dropping_other_chat_fields() {
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "deepseek-v4-flash",
        "messages": [{"role": "user", "content": "code"}],
        "web_search_options": {"search_context_size": "high"},
        "tools": [{"type": "function", "function": {"name": "shell"}}],
        "tool_choice": {"type": "function", "function": {"name": "shell"}},
        "temperature": 0.2,
        "metadata": {"request": "fixture"}
    }))
    .unwrap();

    let fallback =
        runtime_deepseek_auto_chat_fallback_body(&body, RuntimeDeepSeekWebSearchMode::Auto)
            .expect("auto mode should have a safe chat fallback");
    let fallback: serde_json::Value = serde_json::from_slice(&fallback).unwrap();
    assert!(fallback.get("web_search_options").is_none());
    assert_eq!(fallback["model"], "deepseek-v4-flash");
    assert_eq!(fallback["messages"][0]["content"], "code");
    assert_eq!(fallback["tools"][0]["function"]["name"], "shell");
    assert_eq!(fallback["tool_choice"]["function"]["name"], "shell");
    assert_eq!(fallback["temperature"], 0.2);
    assert_eq!(fallback["metadata"]["request"], "fixture");
}

#[test]
fn explicit_anthropic_mode_has_no_chat_fallback() {
    let body = br#"{"web_search_options":{}}"#;
    assert!(
        runtime_deepseek_auto_chat_fallback_body(body, RuntimeDeepSeekWebSearchMode::Anthropic,)
            .is_none()
    );
}

#[test]
fn only_known_native_capability_rejections_allow_auto_fallback() {
    let safe = prodex_provider_core::ProviderTransformResult::rejected(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        prodex_provider_core::ProviderWireFormat::OpenAiChatCompletions,
        prodex_provider_core::ProviderWireFormat::AnthropicMessages,
        "Anthropic Messages does not translate chat field `response_format`",
    );
    assert!(runtime_deepseek_native_translation_fallback_is_safe(&safe));

    let unsafe_result = prodex_provider_core::ProviderTransformResult::rejected(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        prodex_provider_core::ProviderWireFormat::OpenAiChatCompletions,
        prodex_provider_core::ProviderWireFormat::AnthropicMessages,
        "translated message must be an object",
    );
    assert!(!runtime_deepseek_native_translation_fallback_is_safe(
        &unsafe_result
    ));
}

#[test]
fn passthrough_url_selects_native_messages_without_changing_chat_completions() {
    assert_eq!(
        runtime_deepseek_passthrough_upstream_url(
            "https://api.deepseek.com/v1",
            "/v1",
            "/v1/messages",
            true,
        ),
        "https://api.deepseek.com/anthropic/v1/messages"
    );
    assert_eq!(
        runtime_deepseek_passthrough_upstream_url(
            "https://api.deepseek.com/v1",
            "/v1",
            "/v1/chat/completions",
            false,
        ),
        "https://api.deepseek.com/v1/chat/completions"
    );
}
