use prodex_provider_core::{
    ProviderAdapterContract, ProviderEndpoint, ProviderId, ProviderWireFormat, provider_adapter,
    provider_model_catalog, provider_model_fallback_chain,
};

const PROVIDERS: &[ProviderId] = &[
    ProviderId::OpenAi,
    ProviderId::Anthropic,
    ProviderId::Copilot,
    ProviderId::DeepSeek,
    ProviderId::Gemini,
    ProviderId::Local,
];

#[test]
fn adapters_publish_required_contract_surface() {
    for provider in PROVIDERS {
        let adapter = provider_adapter(*provider);
        assert_eq!(adapter.provider(), *provider);
        assert_eq!(
            adapter.client_request_format(),
            ProviderWireFormat::OpenAiResponses
        );
        assert!(adapter.supports_streaming());
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
    for provider in PROVIDERS {
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
    for provider in PROVIDERS {
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
    }
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
