#![cfg(test)]

use super::*;
use prodex_provider_core::{ProviderAdapterContract, ProviderWireFormat, provider_adapter};

#[test]
fn gemini_models_endpoint_exposes_catalog_from_gemini_cli() {
    let parts = runtime_provider_models_buffered_response(
        RuntimeProviderBridgeKind::Gemini,
        None,
        "GET",
        "/v1/models",
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
    let models = body["data"].as_array().unwrap();

    assert!(models.len() > 1);
    assert!(models.iter().any(|model| model["id"] == "auto"));
    assert!(models.iter().any(|model| model["id"] == "gemini-2.5-pro"));
    assert!(
        models
            .iter()
            .any(|model| model["id"] == "gemini-3.1-pro-preview")
    );
    assert!(models.iter().any(|model| model["id"] == "gemini-3.5-flash"));
    assert!(models.iter().any(|model| model["id"] == "flash"));
}

#[test]
fn deepseek_models_endpoint_exposes_current_and_compat_models() {
    let parts = runtime_provider_models_buffered_response(
        RuntimeProviderBridgeKind::DeepSeek,
        None,
        "GET",
        "/models",
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
    let models = body["data"].as_array().unwrap();

    assert!(models.iter().any(|model| model["id"] == "deepseek-v4-pro"));
    assert!(
        models
            .iter()
            .any(|model| model["id"] == "deepseek-v4-flash")
    );
    assert!(models.iter().any(|model| model["id"] == "deepseek-chat"));
    assert!(
        models
            .iter()
            .any(|model| model["id"] == "deepseek-reasoner")
    );
}

#[test]
fn anthropic_and_copilot_models_endpoint_expose_provider_catalogs() {
    let anthropic = runtime_provider_models_buffered_response(
        RuntimeProviderBridgeKind::Anthropic,
        None,
        "GET",
        "/v1/models",
    )
    .unwrap();
    let anthropic_body: serde_json::Value = serde_json::from_slice(&anthropic.body).unwrap();
    let anthropic_models = anthropic_body["data"].as_array().unwrap();

    assert!(
        anthropic_models
            .iter()
            .any(|model| model["id"] == prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL)
    );
    assert!(anthropic_models.iter().any(|model| model["id"] == "opus"));

    let copilot = runtime_provider_models_buffered_response(
        RuntimeProviderBridgeKind::Copilot,
        None,
        "GET",
        "/models",
    )
    .unwrap();
    let copilot_body: serde_json::Value = serde_json::from_slice(&copilot.body).unwrap();
    let copilot_models = copilot_body["data"].as_array().unwrap();

    assert!(
        copilot_models
            .iter()
            .any(|model| model["id"] == prodex_cli::SUPER_COPILOT_DEFAULT_MODEL)
    );
    assert!(copilot_models.iter().any(|model| model["id"] == "codex"));
}

#[test]
fn copilot_models_endpoint_prefers_dynamic_token_catalog() {
    let dynamic = vec![serde_json::json!({
        "id": "copilot-account-only-model",
        "object": "model",
        "owned_by": "github-copilot",
        "display_name": "Account Only Model"
    })];

    let parts = runtime_provider_models_buffered_response(
        RuntimeProviderBridgeKind::Copilot,
        Some(&dynamic),
        "GET",
        "/v1/models",
    )
    .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
    let models = body["data"].as_array().unwrap();

    assert_eq!(models.len(), 1);
    assert_eq!(models[0]["id"], "copilot-account-only-model");

    let single = runtime_provider_models_buffered_response(
        RuntimeProviderBridgeKind::Copilot,
        Some(&dynamic),
        "GET",
        "/v1/models/copilot-account-only-model",
    )
    .unwrap();
    assert_eq!(single.status, 200);
    let single_body: serde_json::Value = serde_json::from_slice(&single.body).unwrap();
    assert_eq!(single_body["display_name"], "Account Only Model");
}

#[test]
fn every_provider_bridge_exposes_openai_client_contract() {
    for kind in [
        RuntimeProviderBridgeKind::OpenAiResponses,
        RuntimeProviderBridgeKind::Anthropic,
        RuntimeProviderBridgeKind::Copilot,
        RuntimeProviderBridgeKind::DeepSeek,
        RuntimeProviderBridgeKind::Gemini,
    ] {
        let contract = provider_adapter(kind.provider_id());
        assert_eq!(
            contract.client_request_format(),
            ProviderWireFormat::OpenAiResponses
        );
        assert_eq!(
            contract.response_format(),
            ProviderWireFormat::OpenAiResponses
        );
        assert_eq!(contract.canonical_client_endpoint(), "/v1/responses");
        assert_eq!(contract.model_list_endpoint(), "/v1/models");
        assert!(contract.supports_streaming());
    }
}

#[test]
fn provider_bridge_contract_records_translation_boundary() {
    assert_eq!(
        provider_adapter(RuntimeProviderBridgeKind::OpenAiResponses.provider_id())
            .upstream_request_format(),
        ProviderWireFormat::OpenAiResponses
    );
    for kind in [
        RuntimeProviderBridgeKind::Anthropic,
        RuntimeProviderBridgeKind::Copilot,
        RuntimeProviderBridgeKind::DeepSeek,
    ] {
        assert_eq!(
            provider_adapter(kind.provider_id()).upstream_request_format(),
            ProviderWireFormat::OpenAiChatCompletions
        );
        assert!(provider_adapter(kind.provider_id()).supports_model_fallback());
    }
    assert_eq!(
        provider_adapter(RuntimeProviderBridgeKind::Gemini.provider_id()).upstream_request_format(),
        ProviderWireFormat::GeminiGenerateContent
    );
}

#[test]
fn provider_bridge_kind_exposes_stable_rate_limit_header_metadata() {
    assert_eq!(
        RuntimeProviderBridgeKind::DeepSeek.rate_limit_header_prefix(),
        "deepseek"
    );
    assert_eq!(
        RuntimeProviderBridgeKind::DeepSeek.rate_limit_header_label(),
        "DeepSeek"
    );
    assert_eq!(
        RuntimeProviderBridgeKind::Anthropic.rate_limit_header_prefix(),
        "anthropic"
    );
    assert_eq!(
        RuntimeProviderBridgeKind::Anthropic.rate_limit_header_label(),
        "Anthropic"
    );
    assert_eq!(
        RuntimeProviderBridgeKind::Gemini.rate_limit_header_prefix(),
        "gemini"
    );
    assert_eq!(
        RuntimeProviderBridgeKind::Gemini.rate_limit_header_label(),
        "Google Gemini"
    );
    assert_eq!(
        RuntimeProviderBridgeKind::Gemini.chat_compatible_adapter_label(),
        "Gemini OpenAI-compatible"
    );
    assert_eq!(
        RuntimeProviderBridgeKind::DeepSeek.chat_compatible_adapter_label(),
        "DeepSeek"
    );
}

#[test]
fn provider_model_fallback_supports_aliases_and_combo() {
    assert_eq!(
        runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Gemini, "auto"),
        vec![
            "gemini-3-pro-preview",
            "gemini-3.1-pro-preview",
            "gemini-2.5-pro",
            "gemini-3-flash-preview",
            "gemini-3.5-flash",
            "gemini-3-flash",
            "gemini-2.5-flash"
        ]
    );
    assert_eq!(
        runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Gemini, "flash"),
        vec![
            "gemini-3-flash-preview",
            "gemini-3.5-flash",
            "gemini-3-flash",
            "gemini-2.5-flash"
        ]
    );
    assert_eq!(
        runtime_provider_model_fallback_chain(
            RuntimeProviderBridgeKind::Gemini,
            "gemini-3.1-pro-preview-customtools"
        ),
        vec![
            "gemini-3.1-pro-preview-customtools",
            "gemini-3.1-pro-preview",
            "gemini-3-pro-preview",
            "gemini-2.5-pro",
            "gemini-3-flash-preview",
            "gemini-3-flash",
            "gemini-3.5-flash",
            "gemini-2.5-flash",
        ]
    );
    assert_eq!(
        runtime_provider_model_fallback_chain(
            RuntimeProviderBridgeKind::Gemini,
            "gemini-3-flash-preview"
        ),
        vec![
            "gemini-3-flash-preview",
            "gemini-3.5-flash",
            "gemini-3-flash",
            "gemini-2.5-flash"
        ]
    );
    assert_eq!(
        runtime_provider_model_fallback_chain(
            RuntimeProviderBridgeKind::DeepSeek,
            "combo:deepseek-v4-pro,deepseek-v4-flash,deepseek-v4-pro"
        ),
        vec!["deepseek-v4-pro", "deepseek-v4-flash"]
    );
    assert_eq!(
        runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Anthropic, "sonnet"),
        vec!["claude-sonnet-4-6", "claude-opus-4-8"]
    );
    assert_eq!(
        runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Copilot, "codex"),
        vec!["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"]
    );
    assert_eq!(
        runtime_provider_model_fallback_chain(RuntimeProviderBridgeKind::Copilot, "gpt-5.4"),
        vec!["gpt-5.4", "gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"]
    );
}

#[test]
fn provider_route_kind_normalizes_bridge_surface_paths() {
    assert_eq!(
        runtime_provider_route_kind("/v1/responses?trace=1"),
        Some(RuntimeProviderRouteKind::Responses)
    );
    assert_eq!(runtime_provider_route_kind("/v1/responses/compact/"), None);
    assert_eq!(
        runtime_provider_route_kind("/v1/chat/completions"),
        Some(RuntimeProviderRouteKind::ChatCompletions)
    );
    assert_eq!(
        runtime_provider_route_kind("/v1/messages"),
        Some(RuntimeProviderRouteKind::Messages)
    );
    assert_eq!(
        runtime_provider_route_kind("/v1/embeddings"),
        Some(RuntimeProviderRouteKind::Embeddings)
    );
    assert_eq!(
        runtime_provider_route_kind("/models/gpt-5.4"),
        Some(RuntimeProviderRouteKind::ModelsSingle("gpt-5.4"))
    );
    assert_eq!(
        runtime_provider_route_kind("/v1/models"),
        Some(RuntimeProviderRouteKind::ModelsList)
    );
    for path in [
        "/tenant/v1/responses",
        "/v1//responses",
        "/v1/%2e%2e/responses",
        "/v1/responses#fragment",
    ] {
        assert_eq!(runtime_provider_route_kind(path), None, "{path}");
    }
}

#[test]
fn provider_error_rules_do_not_treat_generic_429_as_quota() {
    assert_eq!(
        runtime_provider_error_class(RuntimeProviderBridgeKind::Gemini, 429, b"too many requests"),
        RuntimeProviderErrorClass::RateLimit
    );
    let body = serde_json::to_vec(&serde_json::json!({
        "error": {
            "status": "RESOURCE_EXHAUSTED",
            "message": "Quota exceeded."
        }
    }))
    .unwrap();
    assert_eq!(
        runtime_provider_error_class(RuntimeProviderBridgeKind::Gemini, 429, &body),
        RuntimeProviderErrorClass::Quota
    );
    assert_eq!(
        runtime_provider_error_class(RuntimeProviderBridgeKind::DeepSeek, 401, b"{}"),
        RuntimeProviderErrorClass::Auth
    );
    let unsupported_model_body = serde_json::to_vec(&serde_json::json!({
        "error": {
            "message": "The requested model is not supported.",
            "code": "model_not_supported",
            "param": "model",
            "type": "invalid_request_error"
        }
    }))
    .unwrap();
    assert_eq!(
        runtime_provider_error_class(
            RuntimeProviderBridgeKind::Copilot,
            400,
            &unsupported_model_body
        ),
        RuntimeProviderErrorClass::NotFound
    );
    let permission_body = serde_json::to_vec(&serde_json::json!({
        "error": {
            "code": "permission_denied",
            "details": [{
                "reason": "IAM_PERMISSION_DENIED"
            }]
        }
    }))
    .unwrap();
    assert_eq!(
        runtime_provider_error_class(RuntimeProviderBridgeKind::Gemini, 403, &permission_body),
        RuntimeProviderErrorClass::Auth
    );
}

#[test]
fn provider_error_cooldown_uses_provider_aware_classification() {
    let quota_body = serde_json::to_vec(&serde_json::json!({
        "error": {
            "code": "resource_exhausted"
        }
    }))
    .unwrap();
    assert_eq!(
        runtime_provider_error_cooldown_ms(RuntimeProviderBridgeKind::Gemini, 429, &quota_body,),
        300_000
    );

    let transient_body = serde_json::to_vec(&serde_json::json!({
        "error": {
            "message": "backend overloaded"
        }
    }))
    .unwrap();
    assert_eq!(
        runtime_provider_error_cooldown_ms(
            RuntimeProviderBridgeKind::DeepSeek,
            503,
            &transient_body,
        ),
        10_000
    );
}

#[test]
fn provider_native_passthrough_is_explicit() {
    assert!(runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::OpenAiResponses,
        "/v1/responses"
    ));
    assert_eq!(
        runtime_provider_label(RuntimeProviderBridgeKind::Kiro),
        "kiro"
    );
    assert!(!runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::Kiro,
        "/v1/responses"
    ));
    assert!(!runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::Kiro,
        "/v1/chat/completions"
    ));
    assert!(!runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::Gemini,
        "/v1/responses"
    ));
    assert!(runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::DeepSeek,
        "/v1/chat/completions"
    ));
    assert!(runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::DeepSeek,
        "/v1/messages"
    ));
    assert!(!runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::DeepSeek,
        "/v1/embeddings"
    ));
    assert!(!runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::Copilot,
        "/v1/responses/compact"
    ));
    assert!(!runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::Anthropic,
        "/v1/responses"
    ));
    assert!(!runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::Copilot,
        "/v1/responses"
    ));
    assert!(runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::Copilot,
        "/v1/chat/completions"
    ));
    assert!(!runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::Kiro,
        "/v1/messages"
    ));
    assert!(runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::Gemini,
        "/v1/embeddings"
    ));
}

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
fn request_conformance_result_uses_provider_core_translator_for_deepseek_and_gemini() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: vec![
            ("x-codex-turn-state".to_string(), "turn-1".to_string()),
            ("session_id".to_string(), "sess-1".to_string()),
        ],
        body: Vec::new(),
    };
    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        br#"{"model":"deepseek-chat","input":"hello"}"#,
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    assert_eq!(
        deepseek.metadata["continuation"]["x-codex-turn-state"],
        "turn-1"
    );

    let gemini = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        &request,
        br#"{"model":"gemini-2.5-pro","input":"hello","response_format":{"type":"json_schema","schema":{"type":"object","strict":true}}}"#,
    )
    .unwrap();
    assert!(matches!(gemini.loss, ProviderTransformLoss::Lossless));
    assert_eq!(
        gemini.metadata["continuation"]["x-codex-turn-state"],
        "turn-1"
    );
    assert_eq!(gemini.metadata["continuation"]["session_id"], "sess-1");
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        body["request"]["generationConfig"]["responseMimeType"],
        "application/json"
    );
}

#[test]
fn request_conformance_result_treats_compact_route_as_responses_translation_surface() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses/compact".to_string(),
        headers: vec![
            ("x-codex-turn-state".to_string(), "turn-compact".to_string()),
            ("session_id".to_string(), "sess-compact".to_string()),
        ],
        body: Vec::new(),
    };

    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        br#"{"model":"deepseek-chat","input":"hello","previous_response_id":"resp_compact"}"#,
    )
    .expect("deepseek compact conformance");
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    assert_eq!(
        deepseek.metadata["continuation"]["session_id"],
        "sess-compact"
    );
    assert_eq!(
        deepseek.metadata["continuation"]["previous_response_id"],
        "resp_compact"
    );

    let gemini = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        &request,
        br#"{"model":"gemini-2.5-pro","input":"hello","previous_response_id":"resp_compact"}"#,
    )
    .expect("gemini compact conformance");
    assert!(matches!(gemini.loss, ProviderTransformLoss::Lossless));
    assert_eq!(
        gemini.metadata["continuation"]["session_id"],
        "sess-compact"
    );
    assert_eq!(
        gemini.metadata["continuation"]["previous_response_id"],
        "resp_compact"
    );
}

#[test]
fn gemini_request_conformance_translates_message_content_arrays() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let gemini = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "# AGENTS.md instructions for /repo"},
                        {"type": "input_text", "text": "<environment_context><cwd>/repo</cwd></environment_context>"}
                    ]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "Run the update"}
                    ]
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(gemini.loss, ProviderTransformLoss::Lossless));
    let body: serde_json::Value = serde_json::from_slice(gemini.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        body["request"]["contents"][0]["parts"][0]["text"],
        "Run the update"
    );
    assert!(
        body["request"]["systemInstruction"]["parts"][0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("# AGENTS.md instructions for /repo"))
    );
}

#[test]
fn deepseek_request_conformance_translates_message_content_arrays() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "system",
                    "content": [{"type": "input_text", "text": "You are Codex."}]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "read commit history"}]
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    let body: serde_json::Value = serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "You are Codex.");
    assert_eq!(body["messages"][1]["content"], "read commit history");
}

#[test]
fn deepseek_request_conformance_preserves_assistant_tool_history() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "find the symbol"}]
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": ""}],
                    "tool_calls": [
                        {
                            "id":"call_1",
                            "type":"function",
                            "function":{
                                "name":"grep",
                                "arguments":"{\"pattern\":\"ProviderTranslator\"}"
                            }
                        }
                    ]
                },
                {
                    "type": "message",
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": [{"type": "output_text", "text": "{\"match_count\":1}"}]
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    let body: serde_json::Value = serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["messages"][1]["role"], "assistant");
    assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call_1");
    assert_eq!(
        body["messages"][1]["tool_calls"][0]["function"]["name"],
        "grep"
    );
    assert_eq!(body["messages"][2]["role"], "tool");
    assert_eq!(body["messages"][2]["tool_call_id"], "call_1");
}

#[test]
fn deepseek_request_conformance_translates_function_call_and_output_items() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "find the symbol"}]
                },
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "grep",
                    "arguments": {"pattern": "ProviderTranslator"}
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "{\"match_count\":1}"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    let body: serde_json::Value = serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(body["messages"][1]["role"], "assistant");
    assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call_1");
    assert_eq!(
        body["messages"][1]["tool_calls"][0]["function"]["arguments"],
        "{\"pattern\":\"ProviderTranslator\"}"
    );
    assert_eq!(body["messages"][2]["role"], "tool");
    assert_eq!(body["messages"][2]["tool_call_id"], "call_1");
}

#[test]
fn deepseek_request_conformance_translates_custom_and_shell_call_items() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let custom = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "custom_tool_call",
                    "call_id": "call_patch_1",
                    "name": "apply_patch",
                    "input": "*** Begin Patch\n*** End Patch"
                },
                {
                    "type": "custom_tool_call_output",
                    "call_id": "call_patch_1",
                    "output": "applied"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let custom_body: serde_json::Value =
        serde_json::from_slice(custom.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        custom_body["messages"][0]["tool_calls"][0]["function"]["name"],
        "apply_patch"
    );
    assert_eq!(
        custom_body["messages"][0]["tool_calls"][0]["function"]["arguments"],
        "{\"input\":\"*** Begin Patch\\n*** End Patch\"}"
    );
    assert_eq!(custom_body["messages"][1]["tool_call_id"], "call_patch_1");

    let shell = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "local_shell_call",
                    "call_id": "call_shell_1",
                    "action": {
                        "command": ["git", "status", "--short"]
                    },
                    "cwd": "/repo",
                    "timeout": 1000
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_shell_1",
                    "output": " M Cargo.toml"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let shell_body: serde_json::Value =
        serde_json::from_slice(shell.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        shell_body["messages"][0]["tool_calls"][0]["function"]["name"],
        "shell_command"
    );
    assert_eq!(
        shell_body["messages"][0]["tool_calls"][0]["function"]["arguments"],
        "{\"command\":\"git status --short\",\"cwd\":\"/repo\",\"timeout\":1000}"
    );
    assert_eq!(shell_body["messages"][1]["tool_call_id"], "call_shell_1");
}

#[test]
fn deepseek_request_conformance_translates_mcp_call_and_result_items() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/responses".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let deepseek = runtime_provider_request_conformance_result(
        RuntimeProviderBridgeKind::DeepSeek,
        &request,
        &serde_json::to_vec(&serde_json::json!({
            "model": "deepseek-chat",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "compress this text"}]
                },
                {
                    "type": "mcp_call",
                    "id": "call_sqz_1",
                    "name": "mcp__prodex_sqz__compress",
                    "arguments": {"text": "large repeated content"},
                    "output": "ref:abc123"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(deepseek.loss, ProviderTransformLoss::Lossless));
    let body: serde_json::Value = serde_json::from_slice(deepseek.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        body["messages"][1]["tool_calls"][0]["function"]["name"],
        "mcp__prodex_sqz__compress"
    );
    assert_eq!(
        body["messages"][1]["tool_calls"][0]["function"]["arguments"],
        "{\"text\":\"large repeated content\"}"
    );
    assert_eq!(body["messages"][2]["tool_call_id"], "call_sqz_1");
    assert_eq!(body["messages"][2]["content"], "ref:abc123");
}

#[test]
fn request_conformance_result_skips_non_responses_paths() {
    let request = crate::RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/chat/completions".to_string(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    assert!(
        runtime_provider_request_conformance_result(
            RuntimeProviderBridgeKind::DeepSeek,
            &request,
            br#"{"model":"deepseek-chat","input":"hello"}"#,
        )
        .is_none()
    );
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
