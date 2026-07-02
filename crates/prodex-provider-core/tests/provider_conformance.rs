use prodex_provider_core::{
    PROVIDER_CONTRACT_PROVIDERS, ProviderAdapterContract, ProviderCapabilityStatus,
    ProviderEndpoint, ProviderId, ProviderTransformPhase, ProviderWireFormat, extract_usage_tokens,
    provider_adapter, provider_adapter_contract_matrix, provider_model_catalog,
    provider_model_catalog_json, provider_model_fallback_chain, provider_replay_cases,
    provider_translator,
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
fn non_responses_supported_endpoint_statuses_follow_runtime_surface_more_closely() {
    let matrix = provider_adapter_contract_matrix();

    let anthropic = matrix
        .iter()
        .find(|spec| spec.provider == "anthropic")
        .expect("anthropic contract");
    let chat = anthropic
        .endpoint_status
        .iter()
        .find(|endpoint| endpoint.endpoint == "chat-completions")
        .expect("chat-completions endpoint");
    assert_eq!(chat.status, "passthrough");
    assert!(chat.tested);

    let copilot = matrix
        .iter()
        .find(|spec| spec.provider == "copilot")
        .expect("copilot contract");
    let compact = copilot
        .endpoint_status
        .iter()
        .find(|endpoint| endpoint.endpoint == "responses/compact")
        .expect("responses/compact endpoint");
    assert_eq!(compact.status, "passthrough");
    assert!(compact.tested);
    assert!(!compact.streaming);

    let kiro = matrix
        .iter()
        .find(|spec| spec.provider == "kiro")
        .expect("kiro contract");
    let chat = kiro
        .endpoint_status
        .iter()
        .find(|endpoint| endpoint.endpoint == "chat-completions")
        .expect("kiro chat-completions endpoint");
    assert!(
        chat.unsupported_params
            .iter()
            .any(|field| field == "parallel_tool_calls")
    );
    assert!(
        chat.unsupported_params
            .iter()
            .any(|field| field == "max_output_tokens/max_tokens/max_completion_tokens")
    );
}

#[test]
fn translated_responses_contract_surface_exposes_known_parameter_limitations() {
    let matrix = provider_adapter_contract_matrix();

    let deepseek = matrix
        .iter()
        .find(|spec| spec.provider == "deepseek")
        .expect("deepseek contract");
    let deepseek_responses = deepseek
        .endpoint_status
        .iter()
        .find(|endpoint| endpoint.endpoint == "responses")
        .expect("deepseek responses endpoint");
    assert!(
        deepseek_responses
            .unsupported_params
            .iter()
            .any(|field| field == "parallel_tool_calls=false")
    );
    assert!(
        deepseek_responses
            .unsupported_params
            .iter()
            .any(|field| field == "tools[type!=function]")
    );

    let gemini = matrix
        .iter()
        .find(|spec| spec.provider == "gemini")
        .expect("gemini contract");
    let gemini_responses = gemini
        .endpoint_status
        .iter()
        .find(|endpoint| endpoint.endpoint == "responses")
        .expect("gemini responses endpoint");
    assert!(
        gemini_responses
            .unsupported_params
            .iter()
            .any(|field| field == "input[*].content[type!=text]")
    );
    assert!(
        gemini_responses
            .unsupported_params
            .iter()
            .any(|field| field == "response_format.type")
    );
}

#[test]
fn covered_endpoint_statuses_do_not_overclaim_beyond_request_response_conformance() {
    for contract in provider_adapter_contract_matrix() {
        for endpoint in contract.endpoint_status {
            match endpoint.status {
                "native" | "passthrough" | "translated" => {
                    assert!(
                        endpoint.tested,
                        "{} {} should be tested before claiming {}",
                        contract.provider, endpoint.endpoint, endpoint.status
                    );
                }
                "emulated" | "partial" | "untested" | "unsupported" => {}
                other => panic!("unexpected endpoint status {other}"),
            }
        }
    }
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

#[test]
fn claimed_endpoint_statuses_have_request_and_response_conformance_evidence() {
    let matrix = provider_adapter_contract_matrix();
    for contract in matrix {
        for endpoint in contract.endpoint_status {
            if !matches!(endpoint.status, "native" | "passthrough" | "translated") {
                continue;
            }
            let provider = ProviderId::parse(contract.provider).expect("provider should parse");
            let endpoint_id = match endpoint.endpoint {
                "responses" => ProviderEndpoint::Responses,
                "responses/compact" => ProviderEndpoint::ResponsesCompact,
                "chat-completions" => ProviderEndpoint::ChatCompletions,
                "messages" => ProviderEndpoint::Messages,
                "models" => ProviderEndpoint::Models,
                "embeddings" => ProviderEndpoint::Embeddings,
                "images" => ProviderEndpoint::Images,
                "audio" => ProviderEndpoint::Audio,
                "batches" => ProviderEndpoint::Batches,
                "rerank" => ProviderEndpoint::Rerank,
                "a2a" => ProviderEndpoint::A2a,
                other => panic!("unexpected endpoint {other}"),
            };
            let coverage: Vec<_> = prodex_provider_core::provider_conformance_cases()
                .iter()
                .filter(|case| case.provider == provider && case.endpoint == endpoint_id)
                .collect();
            assert!(
                coverage.iter().any(|case| case.operation
                    == prodex_provider_core::ProviderConformanceOperation::Request),
                "{} {} missing request fixture for claimed status {}",
                contract.provider,
                endpoint.endpoint,
                endpoint.status
            );
            assert!(
                coverage.iter().any(|case| case.operation
                    == prodex_provider_core::ProviderConformanceOperation::Response),
                "{} {} missing response fixture for claimed status {}",
                contract.provider,
                endpoint.endpoint,
                endpoint.status
            );
        }
    }
}

#[test]
fn translator_supported_params_do_not_overclaim_unsupported_endpoints() {
    for provider in PROVIDER_CONTRACT_PROVIDERS {
        let adapter = provider_adapter(*provider);
        let translator = provider_translator(*provider);
        for endpoint in [
            ProviderEndpoint::Responses,
            ProviderEndpoint::ChatCompletions,
            ProviderEndpoint::Messages,
            ProviderEndpoint::Embeddings,
            ProviderEndpoint::Images,
            ProviderEndpoint::Audio,
            ProviderEndpoint::Batches,
            ProviderEndpoint::Rerank,
            ProviderEndpoint::A2a,
        ] {
            let support = translator.supported_params(endpoint, "test-model");
            let claimed = matches!(
                adapter.capability_status(endpoint),
                ProviderCapabilityStatus::Native
                    | ProviderCapabilityStatus::Passthrough
                    | ProviderCapabilityStatus::Translated
            );
            assert_eq!(
                support.supported, claimed,
                "{provider:?} {endpoint:?} supported_params drifted from capability status"
            );
            if !claimed {
                assert!(
                    !support.unsupported.is_empty(),
                    "{provider:?} {endpoint:?} should explain unsupported status"
                );
            }
        }
    }
}
