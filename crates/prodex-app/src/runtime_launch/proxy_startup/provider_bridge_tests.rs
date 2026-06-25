#![cfg(test)]

use super::*;

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
        let contract = runtime_provider_openai_contract(kind);
        assert_eq!(
            contract.client_request_format,
            RuntimeProviderWireFormat::OpenAiResponses
        );
        assert_eq!(
            contract.response_format,
            RuntimeProviderWireFormat::OpenAiResponses
        );
        assert_eq!(contract.canonical_client_endpoint, "/v1/responses");
        assert_eq!(contract.model_list_endpoint, "/v1/models");
        assert!(contract.supports_streaming);
    }
}

#[test]
fn provider_bridge_contract_records_translation_boundary() {
    assert_eq!(
        runtime_provider_openai_contract(RuntimeProviderBridgeKind::OpenAiResponses)
            .upstream_request_format,
        RuntimeProviderWireFormat::OpenAiResponses
    );
    for kind in [
        RuntimeProviderBridgeKind::Anthropic,
        RuntimeProviderBridgeKind::Copilot,
        RuntimeProviderBridgeKind::DeepSeek,
    ] {
        assert_eq!(
            runtime_provider_openai_contract(kind).upstream_request_format,
            RuntimeProviderWireFormat::OpenAiChatCompletions
        );
        assert!(runtime_provider_openai_contract(kind).supports_model_fallback);
    }
    assert_eq!(
        runtime_provider_openai_contract(RuntimeProviderBridgeKind::Gemini).upstream_request_format,
        RuntimeProviderWireFormat::GeminiGenerateContent
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
fn provider_native_passthrough_is_explicit() {
    assert!(runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::OpenAiResponses,
        "/v1/responses"
    ));
    assert!(!runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::Gemini,
        "/v1/responses"
    ));
    assert!(runtime_provider_native_passthrough(
        RuntimeProviderBridgeKind::DeepSeek,
        "/v1/chat/completions"
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
        ProviderModelCost::default(),
    );
    let message = event.log_message();

    let fields = runtime_proxy_crate::runtime_proxy_log_fields(&message);
    assert_eq!(
        runtime_proxy_crate::runtime_proxy_log_event(&message),
        Some("gateway_spend")
    );
    assert_eq!(fields.get("phase").map(String::as_str), Some("request"));
    assert_eq!(fields.get("call_id").map(String::as_str), Some("prodex-42"));
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
    assert_eq!(json["call_id"], "prodex-42");
    assert_eq!(json["response_bytes"], serde_json::Value::Null);
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
        ProviderModelCost {
            input_cost_per_million_microusd: Some(1_000_000),
            output_cost_per_million_microusd: Some(2_000_000),
        },
    );

    assert_eq!(event.phase, "response");
    assert_eq!(event.response_bytes, Some(response_body.len()));
    assert_eq!(event.input_tokens, Some(7));
    assert_eq!(event.output_tokens, Some(11));
    assert_eq!(event.cost_usd, Some(0.000029));
}
