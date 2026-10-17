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
fn compiled_namespace_split_preserves_deepseek_anthropic_and_mcp_shapes() {
    for (input, namespace, name) in [
        ("functions__lookup", None, "functions__lookup"),
        ("mcp__repo__東京🧭", Some("mcp__repo"), "東京🧭"),
        ("mcp____tool", Some("mcp__"), "tool"),
        ("mcp__repo__tool--exec", Some("mcp__repo__tool"), "exec"),
        ("mcp__repo__tool--", Some("mcp__repo"), "tool--"),
        ("mcp__repo__\u{2003}", None, "mcp__repo__\u{2003}"),
        ("namespace--sub--tool", Some("namespace--sub"), "tool"),
        ("日本語--🔥", Some("日本語"), "🔥"),
        ("--empty_ns", None, "--empty_ns"),
        ("namespace--\u{3000}", None, "namespace--\u{3000}"),
        ("shell", None, "shell"),
    ] {
        let (actual_namespace, actual_name) = provider_core_split_flat_namespace_tool_name(input);
        assert_eq!(actual_namespace.as_deref(), namespace, "{input:?}");
        assert_eq!(actual_name, name, "{input:?}");
    }
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
fn provider_core_chat_compatible_response_mojo_preserves_special_tools_and_unicode() {
    let response = serde_json::json!({
        "created": u64::MAX,
        "choices": [{"message": {
            "content": "完成 ✅",
            "tool_calls": [
                {"function": {"name": "mcp__prodex__读取", "arguments": "{\"path\":\"東京.txt\"}"}},
                {"id": "search-1", "function": {"name": "tool_search", "arguments": "{\"query\":\"é\"}"}},
                {"id": "patch-1", "function": {"name": "apply_patch", "arguments": "{\"input\":\"*** Begin Patch\\n*** End Patch\"}"}}
            ]
        }}]
    });

    let mapped = provider_core_chat_compatible_responses_value_from_chat_value_with_fallback_ids(
        &response,
        "東京",
        "Gemini OpenAI-compatible",
        "gemini-default",
        || "resp_fallback_é".to_string(),
        || "call_fallback_é".to_string(),
    );

    assert_eq!(mapped["id"], "resp_fallback_é");
    assert_eq!(mapped["created_at"], u64::MAX);
    assert_eq!(mapped["model"], "gemini-default");
    assert_eq!(mapped["output"][0]["content"][0]["text"], "完成 ✅");
    assert_eq!(mapped["output"][1]["call_id"], "call_fallback_é");
    assert_eq!(mapped["output"][1]["namespace"], "mcp__prodex");
    assert_eq!(mapped["output"][1]["name"], "读取");
    assert_eq!(
        mapped["output"][2],
        serde_json::json!({
            "type": "tool_search_call",
            "call_id": "search-1",
            "execution": "client",
            "arguments": {"query": "é"}
        })
    );
    assert_eq!(mapped["output"][3]["type"], "custom_tool_call");
    assert_eq!(
        mapped["output"][3]["input"],
        "*** Begin Patch\n*** End Patch"
    );
}

#[test]
fn compiled_chat_tool_call_classifier_preserves_special_names_and_rejects_blank_unicode() {
    let response = serde_json::json!({
        "id": "chat-functions",
        "model": "model",
        "created": 7,
        "choices": [{"message": {"tool_calls": [
            {"id":"plain", "function":{"name":"tool_search ", "arguments":"{}"}},
            {"id":"search", "function":{"name":"tool_search", "arguments":"{}"}},
            {"id":"patch", "function":{"name":"apply_patch", "arguments":"{\"input\":\"patch\"}"}},
            {"id":"unicode", "function":{"name":"mcp__repo__東京🔥", "arguments":"{}"}}
        ]}}]
    });
    let mapped = provider_core_chat_compatible_responses_value_from_chat_value(
        &response, 7, "deepseek", "DeepSeek", "model", "resp",
    );
    let output = mapped["output"].as_array().unwrap();
    assert_eq!(output.len(), 4);
    assert_eq!(output[0]["type"], "function_call");
    assert_eq!(output[0]["name"], "tool_search ");
    assert_eq!(output[1]["type"], "tool_search_call");
    assert_eq!(output[2]["type"], "custom_tool_call");
    assert_eq!(output[2]["input"], "patch");
    assert_eq!(output[3]["name"], "東京🔥");
    assert_eq!(output[3]["namespace"], "mcp__repo");

    let invalid = serde_json::json!({
        "id": "chat-invalid", "choices": [{"message": {"tool_calls": [
            {"id":"bad", "function":{"name":"\u{2003}\t", "arguments":"{}"}}
        ]}}]
    });
    let failed = provider_core_chat_compatible_responses_value_from_chat_value(
        &invalid, 7, "deepseek", "DeepSeek", "model", "resp",
    );
    assert_eq!(failed["status"], "failed");
    assert!(
        failed["error"]["message"]
            .as_str()
            .unwrap()
            .contains("without a function name")
    );
}

#[test]
fn provider_core_chat_compatible_response_mojo_keeps_partial_output_on_tool_error() {
    let response = serde_json::json!({
        "id": "chat-error",
        "model": "compat",
        "created": 7,
        "choices": [{"message": {
            "content": "before",
            "tool_calls": [
                {"id": "good", "function": {"name": "shell", "arguments": "{}"}},
                {"id": "bad", "function": {"name": "shell", "arguments": "{\"cmd\":"}}
            ]
        }}]
    });

    let mapped = provider_core_chat_compatible_responses_value_from_chat_value(
        &response,
        1,
        "deepseek",
        "DeepSeek",
        "deepseek-chat",
        "resp_deepseek",
    );

    assert_eq!(mapped["status"], "failed");
    assert_eq!(mapped["error"]["code"], "invalid_tool_call_arguments");
    assert_eq!(mapped["output"].as_array().map(Vec::len), Some(2));
    assert_eq!(mapped["output"][1]["call_id"], "good");
    assert!(
        mapped["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("DeepSeek returned malformed JSON arguments"))
    );
}

#[test]
fn provider_core_chat_compatible_response_mojo_operation_is_directly_reachable() {
    let context = serde_json::json!({
        "response": {
            "id": "direct",
            "model": "compat",
            "created": 1,
            "choices": [{"message": {"content": "ok"}}]
        },
        "fallback_response_id": "fallback",
        "fallback_created_at": 0,
        "default_model": "default",
        "provider_metadata_key": "provider",
        "tool_plans": []
    });
    let mut document = crate::mojo_json::Document::default();
    document.root(&context);
    let raw = std::str::from_utf8(&document.raw).unwrap();
    let body = prodex_mojo_core::json::transform_chat_bridge_response(&document.nodes, raw)
        .expect("compiled Mojo operation 5 must accept a valid bridge context");
    let mapped: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(mapped["id"], "direct");
    assert_eq!(mapped["output"][0]["content"][0]["text"], "ok");
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
fn anthropic_chat_compat_response_and_stream_use_mojo_shape_at_provider_boundary() {
    let response =
        provider_translator(ProviderId::Anthropic).transform_response(ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            serde_json::to_vec(&serde_json::json!({
                "id": "chatcmpl_boundary",
                "model": "compat-model",
                "created": 1_700_000_001u64,
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "hello",
                        "tool_calls": [{
                            "id": "call_boundary",
                            "function": {
                                "name": "functions.exec_command",
                                "arguments": "{\"cmd\":\"ls\"}"
                            }
                        }]
                    }
                }],
                "usage": {"prompt_tokens": 2, "completion_tokens": 3}
            }))
            .expect("chat response fixture serializes"),
        ));
    assert_eq!(response.loss, ProviderTransformLoss::Lossless);
    assert_eq!(
        response.from_format,
        ProviderWireFormat::OpenAiChatCompletions
    );
    assert_eq!(response.to_format, ProviderWireFormat::OpenAiResponses);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            response.body.as_deref().expect("translated response body"),
        )
        .expect("translated response parses"),
        serde_json::json!({
            "id": "chatcmpl_boundary",
            "object": "response",
            "created_at": 1_700_000_001u64,
            "model": "compat-model",
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "hello"}]
                },
                {
                    "type": "function_call",
                    "call_id": "call_boundary",
                    "name": "exec_command",
                    "namespace": "functions",
                    "arguments": "{\"cmd\":\"rtk ls\"}"
                }
            ],
            "usage": {"input_tokens": 2, "output_tokens": 3, "total_tokens": 5}
        })
    );

    let stream = provider_translator(ProviderId::Anthropic).transform_stream_event(
        ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            br#"data: {"choices":[{"delta":{"content":"ignored","tool_calls":[{"id":"call_boundary","function":{"name":"functions.exec_command","arguments":"{\"cmd\":\"ls\"}"}}]}}]}

"#
                .to_vec(),
        ),
    );
    assert_eq!(stream.loss, ProviderTransformLoss::Lossless);
    assert_eq!(
        stream.from_format,
        ProviderWireFormat::OpenAiChatCompletions
    );
    assert_eq!(stream.to_format, ProviderWireFormat::OpenAiResponses);
    let stream_body = String::from_utf8(stream.body.expect("translated stream body"))
        .expect("translated stream is UTF-8");
    assert_eq!(
        stream_body.lines().next(),
        Some("event: response.function_call_arguments.delta")
    );
    let payload = stream_body
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .expect("translated stream payload");
    assert_eq!(
        payload,
        serde_json::json!({
            "type": "response.function_call_arguments.delta",
            "call_id": "call_boundary",
            "delta": "{\"cmd\":\"rtk ls\"}"
        })
    );
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
