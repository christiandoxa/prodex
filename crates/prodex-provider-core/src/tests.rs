use super::*;

#[test]
fn catalog_covers_gateway_providers() {
    for provider in [
        ProviderId::OpenAi,
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Kiro,
        ProviderId::Local,
    ] {
        assert!(!provider_model_catalog(provider).is_empty());
        assert!(provider_supported_endpoints(provider).contains(&ProviderEndpoint::Responses));
        assert!(provider_supported_endpoints(provider).contains(&ProviderEndpoint::Models));
    }
    assert!(provider_supported_endpoints(ProviderId::OpenAi).contains(&ProviderEndpoint::Images));
    assert!(provider_supported_endpoints(ProviderId::Local).contains(&ProviderEndpoint::A2a));
    assert!(
        provider_supported_endpoints(ProviderId::Gemini).contains(&ProviderEndpoint::Embeddings)
    );
    assert!(
        !provider_supported_endpoints(ProviderId::DeepSeek).contains(&ProviderEndpoint::Embeddings)
    );
    assert_eq!(
        provider_supported_endpoints(ProviderId::OpenAi),
        [
            ProviderEndpoint::Responses,
            ProviderEndpoint::ChatCompletions,
            ProviderEndpoint::Messages,
            ProviderEndpoint::Models,
            ProviderEndpoint::Embeddings,
            ProviderEndpoint::Images,
            ProviderEndpoint::Audio,
            ProviderEndpoint::Batches,
            ProviderEndpoint::Rerank,
            ProviderEndpoint::A2a,
        ]
    );
    assert_eq!(
        provider_supported_endpoints(ProviderId::Local),
        provider_supported_endpoints(ProviderId::OpenAi)
    );
    assert_eq!(
        provider_supported_endpoints(ProviderId::Gemini),
        [
            ProviderEndpoint::Responses,
            ProviderEndpoint::ChatCompletions,
            ProviderEndpoint::Messages,
            ProviderEndpoint::Models,
            ProviderEndpoint::Embeddings,
        ]
    );
    for provider in [ProviderId::Anthropic, ProviderId::DeepSeek] {
        assert_eq!(
            provider_supported_endpoints(provider),
            [
                ProviderEndpoint::Responses,
                ProviderEndpoint::ChatCompletions,
                ProviderEndpoint::Messages,
                ProviderEndpoint::Models,
            ]
        );
    }
    for provider in [ProviderId::Copilot, ProviderId::Kiro] {
        assert_eq!(
            provider_supported_endpoints(provider),
            [
                ProviderEndpoint::Responses,
                ProviderEndpoint::ResponsesCompact,
                ProviderEndpoint::ChatCompletions,
                ProviderEndpoint::Messages,
                ProviderEndpoint::Models,
            ]
        );
    }
}

#[test]
fn provider_adapter_contract_matches_fixture() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/provider_contracts.json"))
            .expect("provider contract fixture should parse");
    let contracts = fixture.as_array().expect("fixture should be an array");
    assert_eq!(contracts.len(), 7);
    for contract in contracts {
        let provider = ProviderId::parse(contract["provider"].as_str().unwrap())
            .expect("fixture provider should parse");
        let adapter = provider_adapter(provider);
        let spec = provider_adapter_contract_spec(provider);
        assert_eq!(adapter.provider().label(), contract["provider"]);
        assert_eq!(spec.provider, contract["provider"]);
        assert_eq!(
            spec.client_request_format,
            contract["client_request_format"]
        );
        assert_eq!(
            spec.upstream_request_format,
            contract["upstream_request_format"]
        );
        assert_eq!(spec.response_format, contract["response_format"]);
        assert_eq!(
            spec.canonical_client_endpoint,
            contract["canonical_client_endpoint"]
        );
        assert_eq!(spec.model_list_endpoint, contract["model_list_endpoint"]);
        assert_eq!(
            spec.supports_streaming,
            contract["supports_streaming"].as_bool().unwrap()
        );
        assert_eq!(
            spec.supports_model_fallback,
            contract["supports_model_fallback"].as_bool().unwrap()
        );
        assert_eq!(spec.transform_status, contract["transform_status"]);
        for required in contract["required_endpoints"].as_array().unwrap() {
            assert!(
                spec.supported_endpoints
                    .contains(&required.as_str().unwrap()),
                "{} missing endpoint {}",
                provider.label(),
                required
            );
        }
        assert!(spec.model_count > 0);
    }
}

#[test]
fn transform_result_exposes_explicit_status_outcome() {
    let lossless = ProviderTransformResult::lossless(
        ProviderId::Gemini,
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::GeminiGenerateContent,
        br#"{"ok":true}"#.to_vec(),
    );
    assert_eq!(lossless.status(), TransformStatus::Lossless);
    assert_eq!(lossless.outcome().value, Some(br#"{"ok":true}"#.to_vec()));

    let degraded = ProviderTransformResult::degraded(
        ProviderId::Gemini,
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::GeminiGenerateContent,
        Vec::new(),
        "dropped unsupported option",
        Default::default(),
    );
    assert_eq!(
        degraded.status(),
        TransformStatus::Degraded {
            reason: "dropped unsupported option".to_string()
        }
    );
    assert_eq!(
        degraded.status().reason(),
        Some("dropped unsupported option")
    );

    let rejected = ProviderTransformResult::rejected(
        ProviderId::Gemini,
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::GeminiGenerateContent,
        "bad request",
    );
    assert_eq!(
        rejected.outcome(),
        TransformOutcome {
            status: TransformStatus::Rejected {
                reason: "bad request".to_string()
            },
            value: None
        }
    );

    let unsupported = ProviderTransformResult::unsupported(
        ProviderId::Gemini,
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::GeminiGenerateContent,
        "stream event unsupported",
    );
    assert_eq!(
        unsupported.status(),
        TransformStatus::Unsupported {
            reason: "stream event unsupported".to_string()
        }
    );
    assert!(ProviderConformanceExpectedLoss::Unsupported.matches_status(&unsupported.status()));
    assert!(ProviderConformanceExpectedLoss::Unsupported.requires_reason());
    assert!(!ProviderConformanceExpectedLoss::Lossless.requires_reason());
    assert_eq!(
        ProviderConformanceExpectedLoss::Unsupported.label(),
        "unsupported"
    );
}

#[test]
fn provider_contract_matrix_is_stable_and_complete() {
    let matrix = provider_adapter_contract_matrix();
    assert_eq!(matrix.len(), PROVIDER_CONTRACT_PROVIDERS.len());
    assert_eq!(
        matrix
            .iter()
            .map(|contract| contract.provider)
            .collect::<Vec<_>>(),
        vec![
            "openai",
            "anthropic",
            "copilot",
            "deepseek",
            "gemini",
            "kiro",
            "local"
        ]
    );
    for contract in matrix {
        assert_eq!(contract.client_request_format, "openai-responses");
        assert_eq!(contract.canonical_client_endpoint, "/v1/responses");
        assert_eq!(contract.model_list_endpoint, "/v1/models");
        assert!(contract.supports_streaming);
        assert!(
            contract.transform_status == "passthrough" || contract.transform_status == "translated"
        );
        assert!(contract.supported_endpoints.contains(&"responses"));
        assert!(contract.supported_endpoints.contains(&"models"));
        if contract.provider == "copilot" {
            assert!(contract.supported_endpoints.contains(&"responses/compact"));
        }
        assert!(contract.model_count > 0);
        assert_eq!(contract.replay_case_count, 1);
    }
}

#[test]
fn fallback_chain_preserves_existing_gemini_aliases() {
    assert_eq!(
        provider_model_fallback_chain(ProviderId::Gemini, "flash")[0],
        "gemini-3-flash-preview"
    );
    assert_eq!(
        provider_model_fallback_chain(ProviderId::Anthropic, "opus"),
        vec!["claude-opus-4-8", "claude-sonnet-4-6"]
    );
    assert_eq!(
        provider_model_fallback_chain(ProviderId::Copilot, "codex"),
        vec!["gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"]
    );
    assert_eq!(
        provider_model_fallback_chain(ProviderId::Copilot, "gpt-5.4"),
        vec!["gpt-5.4", "gpt-5.3-codex", "gpt-5.1-codex", "gpt-4o"]
    );
    assert_eq!(
        provider_model_fallback_chain(ProviderId::DeepSeek, "flash"),
        vec!["deepseek-v4-flash", "deepseek-v4-pro"]
    );
    assert_eq!(
        provider_model_fallback_chain(ProviderId::DeepSeek, "auto"),
        vec!["deepseek-v4-pro", "deepseek-v4-flash"]
    );
    assert_eq!(
        provider_model_fallback_chain(ProviderId::OpenAi, "combo:gpt-5,gpt-5;gpt-4o"),
        vec!["gpt-5", "gpt-4o"]
    );
    assert_eq!(
        provider_model_fallback_chain(ProviderId::OpenAi, "combo:gpt-5|GPT-5>gpt-4o"),
        vec!["gpt-5", "gpt-4o"]
    );
    assert_eq!(
        provider_model_fallback_chain(ProviderId::OpenAi, " custom-model "),
        vec!["custom-model"]
    );
    assert_eq!(
        provider_model_fallback_chain(ProviderId::Local, " local-model "),
        vec!["local-model"]
    );
}

#[test]
fn gemini_code_assist_model_filter_preserves_existing_runtime_behavior() {
    let mut chain = vec![
        "gemini-3-pro-preview".to_string(),
        "gemini-3.1-pro-preview-customtools".to_string(),
        "gemini-3.5-flash".to_string(),
        "gemini-3-flash".to_string(),
        "gemini-2.5-flash".to_string(),
    ];

    provider_gemini_retain_code_assist_models(&mut chain);

    assert_eq!(
        chain,
        vec![
            "gemini-3-pro-preview".to_string(),
            "gemini-2.5-flash".to_string()
        ]
    );
}
