use super::*;

#[test]
fn catalog_covers_gateway_providers() {
    for provider in [
        ProviderId::OpenAi,
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Local,
    ] {
        assert!(!provider_model_catalog(provider).is_empty());
        assert!(provider_supported_endpoints(provider).contains(&ProviderEndpoint::Responses));
        assert!(provider_supported_endpoints(provider).contains(&ProviderEndpoint::Models));
    }
}

#[test]
fn provider_adapter_contract_matches_fixture() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/provider_contracts.json"))
            .expect("provider contract fixture should parse");
    let contracts = fixture.as_array().expect("fixture should be an array");
    assert_eq!(contracts.len(), 6);
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
            "local"
        ]
    );
    for contract in matrix {
        assert_eq!(contract.client_request_format, "openai-responses");
        assert_eq!(contract.canonical_client_endpoint, "/v1/responses");
        assert_eq!(contract.model_list_endpoint, "/v1/models");
        assert!(contract.supports_streaming);
        assert!(contract.supported_endpoints.contains(&"responses"));
        assert!(contract.supported_endpoints.contains(&"models"));
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
        vec!["gpt-5.1-codex", "gpt-5.3-codex", "gpt-5.4"]
    );
}
