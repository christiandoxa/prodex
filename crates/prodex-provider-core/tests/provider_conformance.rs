use prodex_provider_core::{
    PROVIDER_CONTRACT_PROVIDERS, ProviderAdapterContract, ProviderCapabilityStatus,
    ProviderEndpoint, ProviderId, ProviderTransformPhase, ProviderWireFormat, extract_usage_tokens,
    provider_adapter, provider_adapter_contract_matrix, provider_model_catalog,
    provider_model_catalog_json, provider_model_fallback_chain, provider_replay_cases,
};

#[test]
fn adapters_publish_required_contract_surface() {
    for provider in PROVIDER_CONTRACT_PROVIDERS {
        let adapter = provider_adapter(*provider);
        assert_eq!(adapter.provider(), *provider);
        assert_eq!(
            adapter.client_request_format(),
            ProviderWireFormat::OpenAiResponses
        );
        assert!(adapter.supports_streaming());
        assert!(matches!(
            adapter.transform_status(),
            ProviderCapabilityStatus::Passthrough | ProviderCapabilityStatus::Translated
        ));
        assert_eq!(adapter.canonical_client_endpoint(), "/v1/responses");
        assert_eq!(adapter.model_list_endpoint(), "/v1/models");
        assert!(
            adapter
                .supported_endpoints()
                .contains(&ProviderEndpoint::Responses)
        );
        assert!(
            adapter
                .supported_endpoints()
                .contains(&ProviderEndpoint::Models)
        );
        assert!(!adapter.model_catalog().is_empty());
    }
}

#[test]
fn model_catalog_entries_are_provider_scoped_and_unique() {
    for provider in PROVIDER_CONTRACT_PROVIDERS {
        let mut seen = Vec::new();
        for model in provider_model_catalog(*provider) {
            assert_eq!(model.provider, *provider);
            assert!(!model.id.trim().is_empty());
            assert!(!model.owned_by.trim().is_empty());
            assert!(model.endpoints.contains(&ProviderEndpoint::Responses));
            if !matches!(provider, ProviderId::Local) {
                assert!(
                    model.context_window_tokens.is_some(),
                    "{provider:?} model {} should publish a context window",
                    model.id
                );
            }
            assert!(
                !seen
                    .iter()
                    .any(|existing: &String| existing.eq_ignore_ascii_case(model.id))
            );
            seen.push(model.id.to_string());
        }
    }
}

#[test]
fn adapter_transform_contract_preserves_body_and_declares_formats() {
    let body = br#"{"model":"auto","input":"hello"}"#;
    for provider in PROVIDER_CONTRACT_PROVIDERS {
        let adapter = provider_adapter(*provider);

        let request = adapter.transform_request_body(body);
        assert_eq!(request.provider, *provider);
        assert_eq!(request.from_format, adapter.client_request_format());
        assert_eq!(request.to_format, adapter.upstream_request_format());
        assert_eq!(request.body, body);
        assert!(!request.lossy);

        let response = adapter.transform_response_body(body);
        assert_eq!(response.provider, *provider);
        assert_eq!(response.from_format, adapter.upstream_request_format());
        assert_eq!(response.to_format, adapter.response_format());
        assert_eq!(response.body, body);
        assert!(!response.lossy);

        let stream_event = adapter.transform_stream_event(b"data: {}\n\n");
        assert_eq!(
            stream_event.phase,
            ProviderTransformPhase::UpstreamStreamEventToClient
        );
        assert_eq!(stream_event.provider, *provider);
        assert_eq!(stream_event.body, b"data: {}\n\n");
    }
}

#[test]
fn provider_replay_cases_exercise_transform_usage_and_fallback_contracts() {
    let cases = provider_replay_cases();
    assert_eq!(cases.len(), PROVIDER_CONTRACT_PROVIDERS.len());
    for case in cases {
        let adapter = provider_adapter(case.provider);

        let request = adapter.transform_request_body(case.request_body);
        assert_eq!(
            request.phase,
            ProviderTransformPhase::ClientRequestToUpstream
        );
        assert_eq!(request.provider, case.provider);
        assert_eq!(request.from_format, adapter.client_request_format());
        assert_eq!(request.to_format, adapter.upstream_request_format());
        assert_eq!(request.body, case.request_body);
        assert!(!request.lossy);
        assert!(adapter.estimate_input_tokens(case.request_body) > 0);

        let response = adapter.transform_response_body(case.response_body);
        assert_eq!(
            response.phase,
            ProviderTransformPhase::UpstreamResponseToClient
        );
        assert_eq!(response.provider, case.provider);
        assert_eq!(response.from_format, adapter.upstream_request_format());
        assert_eq!(response.to_format, adapter.response_format());
        assert_eq!(response.body, case.response_body);
        assert!(!response.lossy);

        let usage = extract_usage_tokens(case.response_body);
        assert_eq!(usage.input_tokens, case.expected_input_tokens);
        assert_eq!(usage.output_tokens, case.expected_output_tokens);

        let fallback = adapter.fallback_chain(case.model);
        if adapter.supports_model_fallback() {
            assert!(
                !fallback.is_empty(),
                "{:?} should have fallback",
                case.provider
            );
        } else {
            assert_eq!(fallback, vec![case.model.to_string()]);
        }
    }
}

#[test]
fn public_contract_matrix_is_machine_readable() {
    let matrix = provider_adapter_contract_matrix();
    assert_eq!(matrix.len(), PROVIDER_CONTRACT_PROVIDERS.len());
    let json = serde_json::to_value(&matrix).expect("contract matrix should serialize");
    assert_eq!(json[0]["provider"], "openai");
    assert!(
        json[0]["supported_endpoints"]
            .as_array()
            .unwrap()
            .iter()
            .any(|endpoint| endpoint == "responses")
    );
    assert!(json[0]["transform_status"].is_string());
    assert!(json[0]["endpoint_status"].as_array().unwrap().iter().any(
        |endpoint| endpoint["endpoint"] == "responses"
            && endpoint["status"].is_string()
            && endpoint["tested"].as_bool().unwrap()
    ));
}

#[test]
fn model_catalog_json_includes_machine_readable_contract_fields() {
    let models = provider_model_catalog_json(ProviderId::Gemini);
    assert!(!models.is_empty());
    assert!(models[0]["aliases"].is_array());
    assert!(models[0]["endpoints"].is_array());
    assert!(models.iter().all(|model| {
        model["endpoints"]
            .as_array()
            .unwrap()
            .iter()
            .any(|endpoint| endpoint == "responses")
    }));
}

#[test]
fn fallback_chains_resolve_to_catalog_or_explicit_passthrough() {
    for (provider, alias) in [
        (ProviderId::Anthropic, "auto"),
        (ProviderId::Anthropic, "opus"),
        (ProviderId::Copilot, "codex"),
        (ProviderId::DeepSeek, "flash"),
        (ProviderId::Gemini, "flash"),
    ] {
        let catalog = provider_model_catalog(provider);
        let chain = provider_model_fallback_chain(provider, alias);
        assert!(!chain.is_empty(), "{provider:?} {alias}");
        for model in chain {
            assert!(
                catalog
                    .iter()
                    .any(|spec| spec.id.eq_ignore_ascii_case(&model)),
                "{provider:?} alias {alias} returned uncataloged model {model}"
            );
        }
    }
}

#[test]
fn combo_fallback_deduplicates_user_supplied_chain() {
    assert_eq!(
        provider_model_fallback_chain(ProviderId::Gemini, "combo:a,b,a|c"),
        vec!["a", "b", "c"]
    );
}
