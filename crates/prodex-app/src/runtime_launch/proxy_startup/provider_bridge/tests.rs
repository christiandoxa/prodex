#![cfg(test)]

use super::*;
use prodex_provider_core::{ProviderAdapterContract, ProviderWireFormat, provider_adapter};

mod catalog_and_policy;
mod request;

#[test]
fn gateway_spend_message_includes_stable_call_fields() {
    let event = runtime_provider_gateway_spend_event(
        42,
        RuntimeProviderBridgeKind::Gemini,
        "/v1/responses?trace=1",
        Some("prodex-fast"),
        200,
        123,
        456,
        br#"{"model":"prodex-fast","input":"hello from prodex"}"#,
        prodex_provider_core::ProviderModelCost::default(),
    );
    let message = event.log_message();

    let fields = runtime_proxy_crate::runtime_proxy_log_fields(&message);
    assert_eq!(
        runtime_proxy_crate::runtime_proxy_log_event(&message),
        Some("gateway_spend")
    );
    assert_eq!(fields.get("phase").map(String::as_str), Some("request"));
    let request_id = fields
        .get("request_id")
        .and_then(|id| id.strip_prefix("prodex-"))
        .expect("gateway spend request id should keep prodex prefix");
    assert_eq!(
        request_id
            .parse::<prodex_domain::RequestId>()
            .unwrap()
            .as_uuid()
            .get_version_num(),
        7
    );
    assert_eq!(
        fields.get("legacy_request_sequence").map(String::as_str),
        Some("42")
    );
    let call_id = fields
        .get("call_id")
        .and_then(|id| id.strip_prefix("prodex-"))
        .expect("gateway spend call id should keep prodex prefix");
    assert_eq!(
        call_id
            .parse::<prodex_domain::CallId>()
            .unwrap()
            .as_uuid()
            .get_version_num(),
        7
    );
    assert_eq!(fields.get("provider").map(String::as_str), Some("gemini"));
    assert_eq!(
        fields.get("path").map(String::as_str),
        Some("/v1/responses")
    );
    assert_eq!(fields.get("model").map(String::as_str), Some("prodex-fast"));
    assert_eq!(fields.get("status").map(String::as_str), Some("200"));
    assert_eq!(fields.get("request_bytes").map(String::as_str), Some("456"));
    assert_ne!(
        fields.get("input_tokens").map(String::as_str),
        Some("unknown")
    );
    assert_eq!(fields.get("cost_usd").map(String::as_str), Some("unknown"));

    let json: serde_json::Value = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "gateway_spend");
    assert_eq!(json["phase"], "request");
    assert!(json.get("request").is_none());
    let request_id = json["request_id"]
        .as_str()
        .and_then(|id| id.strip_prefix("prodex-"))
        .expect("gateway spend request id should keep prodex prefix");
    assert_eq!(
        request_id
            .parse::<prodex_domain::RequestId>()
            .unwrap()
            .as_uuid()
            .get_version_num(),
        7
    );
    assert_eq!(json["legacy_request_sequence"], 42);
    assert_eq!(
        json["call_id"]
            .as_str()
            .and_then(|id| id.strip_prefix("prodex-")),
        Some(call_id)
    );
    assert_eq!(json["response_bytes"], serde_json::Value::Null);
}

#[test]
fn gateway_request_spend_uses_reserved_output_estimate_for_cost() {
    let event = runtime_provider_gateway_spend_event(
        42,
        RuntimeProviderBridgeKind::OpenAiResponses,
        "/v1/responses",
        Some("gpt-5.4"),
        200,
        123,
        456,
        br#"{"model":"gpt-5.4","input":"hello","max_output_tokens":17}"#,
        prodex_provider_core::ProviderModelCost {
            input_cost_per_million_microusd: Some(1_000_000),
            output_cost_per_million_microusd: Some(2_000_000),
        },
    );

    assert_eq!(event.input_tokens, Some(2));
    assert_eq!(event.output_tokens, Some(17));
    assert_eq!(event.cost_usd, Some(0.000036));
}

#[test]
fn gateway_response_spend_uses_response_usage_and_total_cost() {
    let response_body =
        br#"{"id":"resp","usage":{"input_tokens":7,"output_tokens":11,"total_tokens":18}}"#;
    let event = runtime_provider_gateway_response_spend_event(
        9,
        RuntimeProviderBridgeKind::OpenAiResponses,
        "/v1/responses",
        Some("gpt-5.4"),
        200,
        12,
        br#"{"model":"gpt-5.4","input":"hello"}"#,
        response_body,
        prodex_provider_core::ProviderModelCost {
            input_cost_per_million_microusd: Some(1_000_000),
            output_cost_per_million_microusd: Some(2_000_000),
        },
    );

    assert_eq!(event.phase, "response");
    let call_id = event
        .call_id
        .strip_prefix("prodex-")
        .expect("gateway response spend call id should keep prodex prefix");
    assert_eq!(
        call_id
            .parse::<prodex_domain::CallId>()
            .unwrap()
            .as_uuid()
            .get_version_num(),
        7
    );
    assert_eq!(event.response_bytes, Some(response_body.len()));
    assert_eq!(event.input_tokens, Some(7));
    assert_eq!(event.output_tokens, Some(11));
    assert_eq!(event.cost_usd, Some(0.000029));
}

#[test]
fn gateway_cost_lookup_accepts_selected_model_after_alias_rewrite() {
    let aliases = vec![runtime_proxy_crate::RuntimeGatewayRouteAlias {
        alias: "prodex-costly".to_string(),
        models: vec!["gpt-costly".to_string()],
        strategy: runtime_proxy_crate::RuntimeGatewayRouteStrategy::Fallback,
        model_metrics: std::collections::BTreeMap::from([(
            "gpt-costly".to_string(),
            runtime_proxy_crate::RuntimeGatewayRouteModelMetrics {
                input_cost_per_million_microusd: Some(1_000_000),
                output_cost_per_million_microusd: Some(2_000_000),
                latency_ms: None,
                rpm_limit: None,
                tpm_limit: None,
            },
        )]),
    }];

    let cost = runtime_provider_gateway_cost_for_request(
        RuntimeProviderBridgeKind::OpenAiResponses,
        &aliases,
        &std::collections::BTreeMap::new(),
        1,
        br#"{"model":"gpt-costly","input":"hello"}"#,
        "gpt-costly",
    );

    assert_eq!(cost.input_cost_per_million_microusd, Some(1_000_000));
    assert_eq!(cost.output_cost_per_million_microusd, Some(2_000_000));
}

#[test]
fn gateway_cost_lookup_accepts_combo_chain_after_fallback_rewrite() {
    let aliases = vec![runtime_proxy_crate::RuntimeGatewayRouteAlias {
        alias: "prodex-costly".to_string(),
        models: vec!["gpt-costly".to_string()],
        strategy: runtime_proxy_crate::RuntimeGatewayRouteStrategy::Fallback,
        model_metrics: std::collections::BTreeMap::from([(
            "gpt-costly".to_string(),
            runtime_proxy_crate::RuntimeGatewayRouteModelMetrics {
                input_cost_per_million_microusd: Some(1_000_000),
                output_cost_per_million_microusd: Some(2_000_000),
                latency_ms: None,
                rpm_limit: None,
                tpm_limit: None,
            },
        )]),
    }];

    let cost = runtime_provider_gateway_cost_for_request(
        RuntimeProviderBridgeKind::OpenAiResponses,
        &aliases,
        &std::collections::BTreeMap::new(),
        1,
        br#"{"model":"combo:gpt-costly","input":"hello"}"#,
        "combo:gpt-costly",
    );

    assert_eq!(cost.input_cost_per_million_microusd, Some(1_000_000));
    assert_eq!(cost.output_cost_per_million_microusd, Some(2_000_000));
}

#[test]
fn response_conformance_result_uses_provider_core_translator_for_deepseek_and_gemini() {
    let deepseek = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        200,
        br#"{"id":"chatcmpl_1","model":"deepseek-chat","choices":[{"message":{"role":"assistant","content":"hi"}}],"usage":{"prompt_tokens":11,"completion_tokens":7,"total_tokens":18}}"#,
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    let deepseek_body: serde_json::Value =
        serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(deepseek_body["object"], "response");

    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_1","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"text":"hello"}]}}],"usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":4,"totalTokenCount":7}}"#,
    )
    .unwrap();
    assert!(matches!(gemini.loss, ProviderTransformLoss::Lossless));
    let gemini_body: serde_json::Value =
        serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(gemini_body["object"], "response");
}

#[test]
fn deepseek_response_conformance_preserves_cache_and_tool_metadata() {
    let deepseek = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        200,
        br#"{"id":"chatcmpl_cache_1","model":"deepseek-chat","created":1700000000,"choices":[{"message":{"role":"assistant","content":"cached hello","reasoning_content":"think","tool_calls":[{"id":"call_1","function":{"name":"functions.exec_command","arguments":"{\"cmd\":\"echo hi\"}"},"extra_content":{"google":{"thought_signature":"sig_1"}}}]},"finish_reason":"tool_calls","logprobs":{"tokens":[]}}],"system_fingerprint":"fp_1","usage":{"prompt_tokens":11,"completion_tokens":7,"total_tokens":18,"prompt_cache_hit_tokens":5,"prompt_cache_miss_tokens":6,"completion_tokens_details":{"reasoning_tokens":2}}}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["created_at"], 1700000000);
    assert_eq!(body["output"][1]["namespace"], "functions");
    assert_eq!(body["output"][1]["name"], "exec_command");
    assert_eq!(body["output"][1]["arguments"], "{\"cmd\":\"rtk echo hi\"}");
    assert_eq!(body["output"][1]["gemini_thought_signature"], "sig_1");
    assert_eq!(
        body["usage"]["metadata"]["deepseek"]["prompt_cache_hit_tokens"],
        5
    );
    assert_eq!(body["metadata"]["deepseek"]["finish_reason"], "tool_calls");
}

#[test]
fn gemini_response_conformance_normalizes_wrapped_response_metadata() {
    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"traceId":"resp_wrapped_1","response":{"modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"text":"wrapped hi"}]}}],"usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":4,"totalTokenCount":7,"cachedContentTokenCount":1,"thoughtsTokenCount":2,"toolUsePromptTokenCount":1}}}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["id"], "resp_wrapped_1");
    assert_eq!(body["usage"]["input_tokens_details"]["cached_tokens"], 1);
    assert_eq!(
        body["usage"]["output_tokens_details"]["reasoning_tokens"],
        2
    );
    assert_eq!(
        body["metadata"]["gemini"]["usageMetadata"]["toolUsePromptTokenCount"],
        1
    );
}

#[test]
fn gemini_response_conformance_maps_prompt_feedback_and_incomplete_status() {
    let blocked = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_blocked","modelVersion":"gemini-2.5-pro","promptFeedback":{"blockReason":"SAFETY"}}"#,
    )
    .unwrap();
    let blocked_body: serde_json::Value =
        serde_json::from_slice(blocked.body.as_ref().unwrap()).unwrap();
    assert_eq!(blocked_body["status"], "failed");
    assert_eq!(blocked_body["error"]["code"], "gemini_prompt_blocked");

    let incomplete = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_truncated","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"text":"partial"}]},"finishReason":"MAX_TOKENS"}]}"#,
    )
    .unwrap();
    let incomplete_body: serde_json::Value =
        serde_json::from_slice(incomplete.body.as_ref().unwrap()).unwrap();
    assert_eq!(incomplete_body["status"], "incomplete");
    assert_eq!(
        incomplete_body["incomplete_details"]["reason"],
        "max_output_tokens"
    );
}

#[test]
fn gemini_response_conformance_maps_safe_function_call_only_response() {
    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_sqz_1","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"thoughtSignature":"sig-response-1","functionCall":{"id":"call_sqz_1","name":"mcp__prodex_sqz__compress","args":{"text":"large content"}}}]}}]}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["output"][0]["type"], "function_call");
    assert_eq!(body["output"][0]["namespace"], "mcp__prodex_sqz");
    assert_eq!(body["output"][0]["name"], "compress");
    assert_eq!(
        body["output"][0]["gemini_thought_signature"],
        "sig-response-1"
    );
}

#[test]
fn gemini_response_conformance_maps_tool_search_and_apply_patch_function_calls() {
    let tool_search = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_search_1","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"functionCall":{"id":"call_search_1","name":"tool_search","args":{"query":"provider translator"}}}]}}]}"#,
    )
    .unwrap();
    let tool_search_body: serde_json::Value =
        serde_json::from_slice(tool_search.body.as_ref().unwrap()).unwrap();
    assert_eq!(tool_search_body["output"][0]["type"], "tool_search_call");
    assert_eq!(tool_search_body["output"][0]["call_id"], "call_search_1");
    assert_eq!(
        tool_search_body["output"][0]["arguments"]["query"],
        "provider translator"
    );

    let apply_patch = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_patch_1","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"functionCall":{"id":"call_patch_1","name":"apply_patch","args":{"input":"*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch"}}}]}}]}"#,
    )
    .unwrap();
    let apply_patch_body: serde_json::Value =
        serde_json::from_slice(apply_patch.body.as_ref().unwrap()).unwrap();
    assert_eq!(apply_patch_body["output"][0]["type"], "custom_tool_call");
    assert_eq!(apply_patch_body["output"][0]["name"], "apply_patch");
    assert_eq!(
        apply_patch_body["output"][0]["input"],
        "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch"
    );
}

#[test]
fn gemini_response_conformance_ignores_thought_text_in_function_call_only_response() {
    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_reason_call","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"text":"internal summary","thought":true},{"thoughtSignature":"sig-response-1","functionCall":{"id":"call_sqz_1","name":"mcp__prodex_sqz__compress","args":{"text":"large content"}}}]}}]}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["output"].as_array().unwrap().len(), 1);
    assert_eq!(body["output"][0]["type"], "function_call");
    assert_eq!(
        body["output"][0]["gemini_thought_signature"],
        "sig-response-1"
    );
}

#[test]
fn gemini_response_conformance_maps_grounding_and_citation_outputs() {
    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_grounded","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"text":"grounded answer"}]},"finishReason":"STOP","citationMetadata":{"citations":[{"uri":"https://citation.example","title":"Citation"}]},"groundingMetadata":{"groundingChunks":[{"web":{"uri":"https://ground.example","title":"Ground"}}]},"urlContextMetadata":{"urlMetadata":[{"retrievedUrl":"https://context.example","urlRetrievalStatus":"URL_RETRIEVAL_STATUS_SUCCESS"}]}}]}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    let output = body["output"].as_array().unwrap();
    assert!(output.iter().any(|item| item["type"] == "web_search_call"));
    assert!(output.iter().any(|item| {
        item["type"] == "message"
            && item["content"][0]["text"]
                .as_str()
                .is_some_and(|text| text == "Citations:\n(Citation) https://citation.example")
    }));
}

#[test]
fn gemini_response_conformance_maps_media_and_special_parts() {
    let gemini = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        200,
        br#"{"responseId":"resp_media","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"parts":[{"executableCode":{"language":"PYTHON","code":"print(2 + 2)"}},{"codeExecutionResult":{"outcome":"OUTCOME_OK","output":"4"}},{"videoMetadata":{"startOffset":"1s","endOffset":"3s"}},{"inlineData":{"mimeType":"image/png","data":"aW1hZ2U="}},{"fileData":{"fileUri":"https://files.example/doc.pdf","mimeType":"application/pdf"}}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":20,"candidatesTokenCount":8,"totalTokenCount":32}}"#,
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    let output = body["output"].as_array().unwrap();
    let message = output
        .iter()
        .find(|item| item["type"] == "message")
        .unwrap();
    let content = message["content"].as_array().unwrap();

    assert!(content.iter().any(|item| {
        item["text"]
            .as_str()
            .is_some_and(|text| text.contains("Gemini executable code"))
    }));
    assert!(content.iter().any(|item| {
        item["text"]
            .as_str()
            .is_some_and(|text| text.contains("Gemini code execution result"))
    }));
    assert!(content.iter().any(|item| {
        item["text"]
            .as_str()
            .is_some_and(|text| text.contains("Gemini video metadata"))
    }));
    assert!(content.iter().any(|item| item["type"] == "input_image"));
    assert!(content.iter().any(|item| {
        item["text"]
            .as_str()
            .is_some_and(|text| text.contains("application/pdf"))
    }));
    assert!(
        output
            .iter()
            .any(|item| item["type"] == "image_generation_call")
    );
}

#[test]
fn response_conformance_result_skips_non_success_status() {
    assert!(
        runtime_provider_response_conformance_result(
            RuntimeProviderBridgeKind::DeepSeek,
            429,
            br#"{"error":{"message":"too many requests"}}"#,
        )
        .is_none()
    );
}

#[test]
fn stream_conformance_result_uses_provider_core_translator_for_deepseek_and_gemini() {
    let deepseek = runtime_provider_stream_event_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        br#"data: {"choices":[{"delta":{"content":"hi"}}]}

"#,
    );
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    assert_eq!(
        String::from_utf8_lossy(deepseek.body.as_ref().unwrap()),
        "event: response.output_text.delta\ndata: {\"delta\":\"hi\",\"type\":\"response.output_text.delta\"}\n\n"
    );

    let gemini = runtime_provider_stream_event_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        br#"data: {"candidates":[{"content":{"parts":[{"text":"gm"}]}}]}

"#,
    );
    assert!(matches!(gemini.loss, ProviderTransformLoss::Lossless));
    assert_eq!(
        String::from_utf8_lossy(gemini.body.as_ref().unwrap()),
        "event: response.output_text.delta\ndata: {\"delta\":\"gm\",\"type\":\"response.output_text.delta\"}\n\n"
    );
}

#[test]
fn stream_conformance_result_reports_unsupported_for_non_sse_payload() {
    let result = runtime_provider_stream_event_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        b"not-sse",
    );
    assert!(matches!(
        result.loss,
        ProviderTransformLoss::UnsupportedUpstream { .. }
    ));
}

#[test]
fn provider_loss_states_map_to_runtime_log_labels() {
    let degraded = ProviderTransformLoss::DegradedButSafe {
        reason: "json schema downgraded".to_string(),
        details: std::collections::BTreeMap::new(),
    };
    assert_eq!(
        super::provider_bridge_conformance::runtime_provider_loss_log_fields(&degraded),
        Some(("degraded", "json schema downgraded"))
    );

    let rejected = ProviderTransformLoss::Rejected {
        reason: "tool choice unsupported".to_string(),
    };
    assert_eq!(
        super::provider_bridge_conformance::runtime_provider_loss_log_fields(&rejected),
        Some(("rejected", "tool choice unsupported"))
    );

    let unsupported = ProviderTransformLoss::UnsupportedUpstream {
        reason: "multimodal input not translated".to_string(),
    };
    assert_eq!(
        super::provider_bridge_conformance::runtime_provider_loss_log_fields(&unsupported),
        Some(("unsupported", "multimodal input not translated"))
    );

    assert_eq!(
        super::provider_bridge_conformance::runtime_provider_loss_log_fields(
            &ProviderTransformLoss::Lossless
        ),
        None
    );
}

#[test]
fn stream_text_delta_event_uses_provider_core_for_gemini_and_deepseek() {
    let deepseek = runtime_provider_stream_text_delta_event(
        RuntimeProviderBridgeKind::DeepSeek,
        &serde_json::json!({
            "choices": [{
                "delta": {
                    "content": "hi"
                }
            }]
        }),
        7,
        123,
        "resp_ds",
    )
    .unwrap();
    assert_eq!(deepseek.0, "response.output_text.delta");
    assert_eq!(deepseek.1["type"], "response.output_text.delta");
    assert_eq!(deepseek.1["delta"], "hi");
    assert_eq!(deepseek.1["sequence_number"], 7);
    assert_eq!(deepseek.1["created_at"], 123);
    assert_eq!(deepseek.1["response_id"], "resp_ds");

    let gemini = runtime_provider_stream_text_delta_event(
        RuntimeProviderBridgeKind::Gemini,
        &serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": "gm"
                    }]
                }
            }]
        }),
        9,
        456,
        "resp_gm",
    )
    .unwrap();
    assert_eq!(gemini.0, "response.output_text.delta");
    assert_eq!(gemini.1["type"], "response.output_text.delta");
    assert_eq!(gemini.1["delta"], "gm");
    assert_eq!(gemini.1["sequence_number"], 9);
    assert_eq!(gemini.1["created_at"], 456);
    assert_eq!(gemini.1["response_id"], "resp_gm");
}

#[test]
fn stream_function_call_arguments_delta_event_uses_provider_core_for_gemini_and_deepseek() {
    let deepseek = runtime_provider_stream_function_call_arguments_delta_event(
        RuntimeProviderBridgeKind::DeepSeek,
        &serde_json::json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "id": "call_ds",
                        "function": {
                            "arguments": "{\"cmd\":\"rtk ls\"}"
                        }
                    }]
                }
            }]
        }),
        11,
    )
    .unwrap();
    assert_eq!(deepseek.0, "response.function_call_arguments.delta");
    assert_eq!(deepseek.1["type"], "response.function_call_arguments.delta");
    assert_eq!(deepseek.1["call_id"], "call_ds");
    assert_eq!(deepseek.1["delta"], "{\"cmd\":\"rtk ls\"}");
    assert_eq!(deepseek.1["sequence_number"], 11);

    let gemini = runtime_provider_stream_function_call_arguments_delta_event(
        RuntimeProviderBridgeKind::Gemini,
        &serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "id": "call_gm",
                            "name": "shell",
                            "args": {"cmd":"rtk ls"}
                        }
                    }]
                }
            }]
        }),
        12,
    )
    .unwrap();
    assert_eq!(gemini.0, "response.function_call_arguments.delta");
    assert_eq!(gemini.1["type"], "response.function_call_arguments.delta");
    assert_eq!(gemini.1["call_id"], "call_gm");
    assert_eq!(gemini.1["delta"], "{\"cmd\":\"rtk ls\"}");
    assert_eq!(gemini.1["sequence_number"], 12);
}

#[test]
fn stream_reasoning_summary_text_delta_event_uses_provider_core_for_gemini() {
    let gemini = runtime_provider_stream_reasoning_summary_text_delta_event(
        RuntimeProviderBridgeKind::Gemini,
        &serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": "internal summary",
                        "thought": true
                    }]
                }
            }]
        }),
        13,
        "resp_gm",
        0,
    )
    .unwrap();
    assert_eq!(gemini.0, "response.reasoning_summary_text.delta");
    assert_eq!(gemini.1["type"], "response.reasoning_summary_text.delta");
    assert_eq!(gemini.1["delta"], "internal summary");
    assert_eq!(gemini.1["sequence_number"], 13);
    assert_eq!(gemini.1["response_id"], "resp_gm");
    assert_eq!(gemini.1["summary_index"], 0);
}

#[test]
fn gateway_spend_events_reuse_admission_request_and_call_ids() {
    let typed_request_id = format!("prodex-{}", prodex_domain::RequestId::new());
    let call_id = format!("prodex-{}", prodex_domain::CallId::new());
    let mut request_event = runtime_provider_gateway_spend_event(
        42,
        RuntimeProviderBridgeKind::Gemini,
        "/v1/responses",
        Some("prodex-fast"),
        200,
        1,
        2,
        b"{}",
        prodex_provider_core::ProviderModelCost::default(),
    );
    let mut response_event = runtime_provider_gateway_response_spend_event(
        42,
        RuntimeProviderBridgeKind::Gemini,
        "/v1/responses",
        Some("prodex-fast"),
        200,
        1,
        b"{}",
        b"{}",
        prodex_provider_core::ProviderModelCost::default(),
    );

    runtime_provider_gateway_spend_apply_admission_ids(
        &mut request_event,
        Some(&typed_request_id),
        Some(&call_id),
    );
    runtime_provider_gateway_spend_apply_admission_ids(
        &mut response_event,
        Some(&typed_request_id),
        Some(&call_id),
    );

    assert_eq!(request_event.request_id, typed_request_id);
    assert_eq!(response_event.request_id, typed_request_id);
    assert_eq!(request_event.call_id, call_id);
    assert_eq!(response_event.call_id, call_id);
    assert_eq!(request_event.legacy_request_sequence, 42);
    assert_eq!(response_event.legacy_request_sequence, 42);
}
