use super::super::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, RuntimeProviderRouteKind,
    runtime_provider_error_class, runtime_provider_error_cooldown_ms, runtime_provider_label,
    runtime_provider_model_fallback_chain, runtime_provider_models_buffered_response,
    runtime_provider_native_passthrough, runtime_provider_route_kind,
};
use prodex_provider_core::{ProviderWireFormat, provider_adapter};

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
