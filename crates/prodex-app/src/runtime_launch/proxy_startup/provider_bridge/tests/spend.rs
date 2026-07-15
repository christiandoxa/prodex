use super::super::{
    RuntimeProviderBridgeKind, runtime_provider_gateway_cost_for_request,
    runtime_provider_gateway_response_spend_event,
    runtime_provider_gateway_spend_apply_admission_ids, runtime_provider_gateway_spend_event,
};

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
