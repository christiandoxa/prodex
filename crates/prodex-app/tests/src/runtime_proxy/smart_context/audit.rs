use super::{
    RuntimeRouteKind, RuntimeSmartContextTransport, SMART_CONTEXT_SHADOW_SAMPLE_BASIS_POINTS,
    prepare_runtime_smart_context_http_body, register_runtime_smart_context_proxy_state,
    runtime_smart_context_rollout_decision, smart_context_observe_minimal_budget,
    smart_context_test_request, smart_context_test_shared, smart_context_test_state_snapshot,
};
use serde_json::json;
use std::borrow::Cow;
use std::sync::Arc;

#[test]
fn smart_context_audit_never_rewrites_tool_arguments_or_message_roles() {
    let text = "Exact repeated data, not permission to change a tool call. ".repeat(180);
    for item in [
        json!({"type":"function_call", "name":"execute", "call_id":"call_a", "arguments":text}),
        json!({"type":"custom_tool_call", "name":"execute", "call_id":"call_a", "input":text}),
        json!({"type":"message", "role":"user", "content":text}),
        json!({"type":"message", "role":"assistant", "content":[{"type":"output_text","text":text}]}),
        json!({"type":"reasoning", "encrypted_content":text}),
        json!({"type":"future_unknown_item", "output":text}),
    ] {
        let shared = smart_context_test_shared("audit-opaque-fields");
        register_runtime_smart_context_proxy_state(&shared, true, Some(32_000), None);
        smart_context_observe_minimal_budget(&shared);
        let request = smart_context_test_request(json!({"input":[item.clone(),item]}));
        let result = prepare_runtime_smart_context_http_body(
            601,
            &request,
            &shared,
            RuntimeRouteKind::Responses,
        )
        .unwrap();
        assert!(
            result.as_ref() == request.body.as_slice(),
            "non-output protocol data must stay byte-identical"
        );
    }
}

#[test]
fn smart_context_audit_shadow_static_observation_never_mutates_state() {
    for previous in [false, true] {
        let mut shared = smart_context_test_shared("audit-shadow-static");
        Arc::make_mut(&mut shared.runtime_config).smart_context_shadow = true;
        register_runtime_smart_context_proxy_state(&shared, true, Some(128_000), None);
        let mut body = json!({"instructions":"Stable developer instructions. ".repeat(80),"input":[{"role":"user","content":"hello"}]});
        if previous {
            body["previous_response_id"] = json!("resp_owned");
        }
        let mut request = smart_context_test_request(body);
        let sampled = (0..10000).any(|index| {
            request.headers = vec![("session_id".into(), format!("audit-shadow-{index}"))];
            runtime_smart_context_rollout_decision(
                602,
                &request,
                &shared,
                RuntimeRouteKind::Responses,
                RuntimeSmartContextTransport::Http,
                None,
            )
            .canary_bucket
                < SMART_CONTEXT_SHADOW_SAMPLE_BASIS_POINTS
        });
        assert!(sampled);
        let before = smart_context_test_state_snapshot(&shared);
        let result = prepare_runtime_smart_context_http_body(
            602,
            &request,
            &shared,
            RuntimeRouteKind::Responses,
        )
        .unwrap();
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(
            smart_context_test_state_snapshot(&shared),
            before,
            "sampled shadow must not commit static fingerprints"
        );
    }
}

#[test]
fn smart_context_audit_missing_artifact_fails_closed_for_unknown_tokenizer() {
    let shared = smart_context_test_shared("audit-unknown-ref");
    register_runtime_smart_context_proxy_state(&shared, true, Some(128_000), None);
    let request = smart_context_test_request(
        json!({"model":"unknown-tokenizer-model","input":[{"type":"function_call_output","call_id":"call_ref","output":"prodex-artifact:sc2:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}),
    );
    let result = prepare_runtime_smart_context_http_body(
        603,
        &request,
        &shared,
        RuntimeRouteKind::Responses,
    );
    assert!(
        result.is_err(),
        "tokenizer fallback must not send an unresolved mandatory reference upstream"
    );
}

#[test]
fn smart_context_audit_json_padding_is_not_token_savings() {
    let shared = smart_context_test_shared("audit-wire-padding");
    register_runtime_smart_context_proxy_state(&shared, true, Some(32_000), None);
    smart_context_observe_minimal_budget(&shared);
    let text = "x".repeat(1024);
    let value = json!({"input":[
        {"type":"function_call_output","call_id":"call_a","output":text},
        {"type":"function_call_output","call_id":"call_b","output":text}
    ]});
    let mut request = smart_context_test_request(value);
    let parsed: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
    let indent = vec![b' '; 1024];
    let formatter = serde_json::ser::PrettyFormatter::with_indent(&indent);
    let mut serializer = serde_json::Serializer::with_formatter(Vec::new(), formatter);
    serde::Serialize::serialize(&parsed, &mut serializer).unwrap();
    request.body = serializer.into_inner();
    let result = prepare_runtime_smart_context_http_body(
        604,
        &request,
        &shared,
        RuntimeRouteKind::Responses,
    )
    .unwrap();
    assert!(
        result.as_ref() == request.body.as_slice(),
        "JSON formatting alone is not model-context savings"
    );
}

#[test]
fn smart_context_audit_typed_output_parts_preserve_media_and_call_identity() {
    let text = "Unicode tool output 🧭 with exact quoted \"fields\" and newlines\n".repeat(130);
    let original = json!({"model":"gpt-4o","input":[
        {"type":"function_call_output","call_id":"call_first","output":[
            {"type":"input_text","text":text},
            {"type":"input_image","image_url":format!("data:image/png;base64,{text}")} ]},
        {"type":"custom_tool_call_output","call_id":"call_second","output":[
            {"type":"input_text","text":text},
            {"type":"future_part","text":text},
            {"type":"input_file","file_data":text} ]}
    ]});
    let mut candidate = original.clone();
    let mut stats = super::RuntimeSmartContextTransformStats::default();
    super::runtime_smart_context_dedupe_input_text_within_request(&mut candidate, &mut stats);
    assert_eq!(stats.duplicate_texts, 1);
    assert_eq!(candidate["input"][0], original["input"][0]);
    assert_eq!(
        candidate["input"][1]["call_id"],
        original["input"][1]["call_id"]
    );
    assert_eq!(
        candidate["input"][1]["output"][1],
        original["input"][1]["output"][1]
    );
    assert_eq!(
        candidate["input"][1]["output"][2],
        original["input"][1]["output"][2]
    );
    assert!(
        candidate["input"][1]["output"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("[prodex-context-ref ")
    );
    let restored =
        super::runtime_smart_context_expand_inline_references(&original, &candidate).unwrap();
    assert_eq!(restored, original);
}
