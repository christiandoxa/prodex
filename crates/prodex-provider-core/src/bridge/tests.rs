//! Provider bridge helper tests.

use super::{
    provider_core_chat_compatible_created_at, provider_core_chat_compatible_responses_usage,
    provider_core_chat_compatible_responses_value_from_chat_value,
    provider_core_chat_compatible_responses_value_from_chat_value_with_fallback_ids,
    provider_core_chat_compatible_tool_call_thought_signature,
    provider_core_chat_compatible_validate_top_level_request_shape, provider_core_lossless_body,
    provider_core_rewritten_body, provider_core_rewritten_json_value,
    provider_core_split_flat_namespace_tool_name,
};
use crate::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformLoss,
    ProviderTransformResult, ProviderWireFormat, provider_translator,
};

#[test]
fn chat_compatible_response_uses_injected_fallback_ids() {
    let response = serde_json::json!({
        "choices": [{
            "message": {
                "tool_calls": [{
                    "type": "function",
                    "function": {"name": "shell", "arguments": "{}"}
                }]
            }
        }]
    });
    let mapped = provider_core_chat_compatible_responses_value_from_chat_value_with_fallback_ids(
        &response,
        "deepseek",
        "DeepSeek",
        "deepseek-chat",
        || "resp_deepseek_request-v7".to_string(),
        || "call_deepseek_call-v7".to_string(),
    );

    assert_eq!(mapped["id"], "resp_deepseek_request-v7");
    assert_eq!(mapped["output"][0]["call_id"], "call_deepseek_call-v7");
}

#[test]
fn provider_core_lossless_body_returns_only_lossless_payloads() {
    let lossless = ProviderTransformResult::lossless(
        ProviderId::Anthropic,
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::OpenAiChatCompletions,
        br#"{"messages":[]}"#.to_vec(),
    );
    assert_eq!(
        provider_core_lossless_body(Some(&lossless)),
        Some(br#"{"messages":[]}"#.to_vec())
    );

    let degraded = ProviderTransformResult {
        loss: ProviderTransformLoss::DegradedButSafe {
            reason: "degraded".to_string(),
            details: Default::default(),
        },
        ..lossless
    };
    assert!(provider_core_lossless_body(Some(&degraded)).is_none());
    assert!(provider_core_lossless_body(None).is_none());
}

#[test]
fn provider_core_chat_compatible_tool_call_thought_signature_reads_google_metadata() {
    let tool_call = serde_json::json!({
        "extra_content": {
            "google": {
                "thought_signature": "sig-1"
            }
        }
    });

    assert_eq!(
        provider_core_chat_compatible_tool_call_thought_signature(&tool_call),
        Some("sig-1".to_string())
    );

    let blank = serde_json::json!({
        "extra_content": {
            "google": {
                "thought_signature": " "
            }
        }
    });
    assert!(provider_core_chat_compatible_tool_call_thought_signature(&blank).is_none());
}

#[test]
fn provider_core_chat_compatible_responses_usage_maps_cache_and_reasoning_tokens() {
    let usage = serde_json::json!({
        "prompt_tokens": 11,
        "completion_tokens": 7,
        "total_tokens": 18,
        "prompt_cache_hit_tokens": 5,
        "prompt_cache_miss_tokens": 6,
        "completion_tokens_details": {
            "reasoning_tokens": 2
        }
    });

    let mapped = provider_core_chat_compatible_responses_usage(&usage, "deepseek").unwrap();

    assert_eq!(mapped["input_tokens"], 11);
    assert_eq!(mapped["output_tokens"], 7);
    assert_eq!(mapped["total_tokens"], 18);
    assert_eq!(mapped["input_tokens_details"]["cached_tokens"], 5);
    assert_eq!(mapped["output_tokens_details"]["reasoning_tokens"], 2);
    assert_eq!(
        mapped["metadata"]["deepseek"]["prompt_cache_miss_tokens"],
        6
    );
}

#[test]
fn provider_core_chat_compatible_created_at_returns_epoch_seconds() {
    assert!(provider_core_chat_compatible_created_at() > 0);
}

#[test]
fn provider_core_chat_compatible_request_shape_checks_public_fields() {
    assert!(provider_core_chat_compatible_validate_top_level_request_shape(
            &serde_json::json!({"model":"m","stream":true,"instructions":"i","previous_response_id":"r"}),
            "DeepSeek",
        )
        .is_ok());

    let error = provider_core_chat_compatible_validate_top_level_request_shape(
        &serde_json::json!({"previous_response_id": 7}),
        "DeepSeek",
    )
    .unwrap_err();
    assert_eq!(error, "DeepSeek previous_response_id must be a string");
}

#[test]
fn provider_core_split_flat_namespace_tool_name_handles_app_shapes() {
    assert_eq!(
        provider_core_split_flat_namespace_tool_name("mcp__prodex_sqz__sqz_read_file"),
        (
            Some("mcp__prodex_sqz".to_string()),
            "sqz_read_file".to_string()
        )
    );
    assert_eq!(
        provider_core_split_flat_namespace_tool_name("shell--exec"),
        (Some("shell".to_string()), "exec".to_string())
    );
    assert_eq!(
        provider_core_split_flat_namespace_tool_name("plain_tool"),
        (None, "plain_tool".to_string())
    );
}

#[test]
fn provider_core_chat_compatible_response_maps_tools_usage_and_metadata() {
    let response = serde_json::json!({
        "id": "chatcmpl_1",
        "model": "gemini-compat",
        "created": 1700000000u64,
        "choices": [{
            "message": {
                "content": "done",
                "reasoning_content": "thought",
                "tool_calls": [{
                    "id": "call_1",
                    "function": {
                        "name": "mcp__prodex_sqz__sqz_read_file",
                        "arguments": "{\"path\":\"README.md\"}"
                    },
                    "extra_content": {
                        "google": {
                            "thought_signature": "sig_1"
                        }
                    }
                }]
            },
            "finish_reason": "tool_calls",
            "logprobs": {"tokens": []}
        }],
        "system_fingerprint": "fp_1",
        "usage": {
            "prompt_tokens": 3,
            "completion_tokens": 4,
            "prompt_cache_hit_tokens": 2
        }
    });

    let mapped = provider_core_chat_compatible_responses_value_from_chat_value(
        &response,
        9,
        "gemini",
        "Gemini OpenAI-compatible",
        "deepseek-chat",
        "resp_deepseek",
    );

    assert_eq!(mapped["id"], "chatcmpl_1");
    assert_eq!(mapped["created_at"], 1700000000u64);
    assert_eq!(mapped["output"][0]["content"][0]["text"], "done");
    assert_eq!(mapped["output"][1]["namespace"], "mcp__prodex_sqz");
    assert_eq!(mapped["output"][1]["name"], "sqz_read_file");
    assert_eq!(mapped["output"][1]["gemini_thought_signature"], "sig_1");
    assert_eq!(mapped["usage"]["total_tokens"], 7);
    assert_eq!(mapped["metadata"]["gemini"]["reasoning_content"], "thought");
    assert_eq!(mapped["metadata"]["gemini"]["system_fingerprint"], "fp_1");
}

#[test]
fn anthropic_provider_core_request_stays_lossless_for_simple_responses_history() {
    let result =
        provider_translator(ProviderId::Anthropic).transform_request(ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            serde_json::to_vec(&serde_json::json!({
                "model": "claude-sonnet-4-6",
                "stream": true,
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "hello"}]
                }]
            }))
            .unwrap(),
        ));
    assert!(matches!(result.loss, ProviderTransformLoss::Lossless));
    assert!(provider_core_lossless_body(Some(&result)).is_some());
}

#[test]
fn provider_core_rewritten_body_accepts_degraded_but_safe_payloads() {
    let degraded = ProviderTransformResult {
        provider: ProviderId::DeepSeek,
        endpoint: ProviderEndpoint::Responses,
        from_format: ProviderWireFormat::OpenAiResponses,
        to_format: ProviderWireFormat::OpenAiChatCompletions,
        body: Some(br#"{"response_format":{"type":"json_object"}}"#.to_vec()),
        headers: Default::default(),
        metadata: Default::default(),
        loss: ProviderTransformLoss::DegradedButSafe {
            reason: "degraded".to_string(),
            details: Default::default(),
        },
    };
    assert_eq!(
        provider_core_rewritten_body(Some(&degraded)),
        Some(br#"{"response_format":{"type":"json_object"}}"#.to_vec())
    );
    assert!(provider_core_lossless_body(Some(&degraded)).is_none());
}

#[test]
fn provider_core_rewritten_json_value_parses_rewritten_payloads() {
    let lossless = ProviderTransformResult::lossless(
        ProviderId::Gemini,
        ProviderEndpoint::Responses,
        ProviderWireFormat::GeminiGenerateContent,
        ProviderWireFormat::OpenAiResponses,
        br#"{"status":"incomplete"}"#.to_vec(),
    );
    assert_eq!(
        provider_core_rewritten_json_value(Some(&lossless)),
        Some(serde_json::json!({"status":"incomplete"}))
    );
}
