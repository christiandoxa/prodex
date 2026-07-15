use super::super::provider_bridge_conformance::runtime_provider_loss_log_fields;
use super::super::{
    RuntimeProviderBridgeKind, runtime_provider_stream_event_conformance_result,
    runtime_provider_stream_function_call_arguments_delta_event,
    runtime_provider_stream_reasoning_summary_text_delta_event,
    runtime_provider_stream_text_delta_event,
};
use prodex_provider_core::ProviderTransformLoss;

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
        runtime_provider_loss_log_fields(&degraded),
        Some(("degraded", "json schema downgraded"))
    );

    let rejected = ProviderTransformLoss::Rejected {
        reason: "tool choice unsupported".to_string(),
    };
    assert_eq!(
        runtime_provider_loss_log_fields(&rejected),
        Some(("rejected", "tool choice unsupported"))
    );

    let unsupported = ProviderTransformLoss::UnsupportedUpstream {
        reason: "multimodal input not translated".to_string(),
    };
    assert_eq!(
        runtime_provider_loss_log_fields(&unsupported),
        Some(("unsupported", "multimodal input not translated"))
    );

    assert_eq!(
        runtime_provider_loss_log_fields(&ProviderTransformLoss::Lossless),
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
