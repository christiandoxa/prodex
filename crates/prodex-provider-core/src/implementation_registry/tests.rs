use super::*;
use crate::{
    ProviderConformanceOperation, ProviderTransformInput, ProviderTransformLoss,
    provider_adapter_contract_matrix, provider_catalog_entries_for, provider_conformance_cases,
};

fn registrations() -> Vec<ProviderImplementationRegistration> {
    BUILTIN_REGISTRATIONS.to_vec()
}

#[test]
fn registry_contains_every_builtin_once_in_stable_order() {
    let registry = provider_implementation_registry();
    let providers: Vec<_> = registry.iter().map(|entry| entry.provider()).collect();
    assert_eq!(providers, PROVIDER_IMPLEMENTATION_ORDER);
    for provider in PROVIDER_IMPLEMENTATION_ORDER {
        assert_eq!(
            providers
                .iter()
                .filter(|candidate| *candidate == provider)
                .count(),
            1
        );
    }
}

#[test]
fn historical_aliases_resolve_identically() {
    let cases = [
        ("openai", ProviderId::OpenAi),
        ("openai-responses", ProviderId::OpenAi),
        ("openai_compatible", ProviderId::OpenAi),
        ("openai-compatible", ProviderId::OpenAi),
        ("anthropic", ProviderId::Anthropic),
        ("claude", ProviderId::Anthropic),
        ("copilot", ProviderId::Copilot),
        ("github-copilot", ProviderId::Copilot),
        ("github_copilot", ProviderId::Copilot),
        ("deepseek", ProviderId::DeepSeek),
        ("gemini", ProviderId::Gemini),
        ("google", ProviderId::Gemini),
        ("kiro", ProviderId::Kiro),
        ("local", ProviderId::Local),
        ("local-openai", ProviderId::Local),
        ("local_openai", ProviderId::Local),
    ];
    for (alias, provider) in cases {
        assert_eq!(ProviderId::parse(alias), Some(provider));
        assert_eq!(
            ProviderId::parse(&format!("  {}  ", alias.to_uppercase())),
            Some(provider)
        );
    }
    assert_eq!(ProviderId::parse("unknown"), None);
}

#[test]
fn provider_id_serde_output_is_unchanged() {
    let cases = [
        (ProviderId::OpenAi, "\"openai\""),
        (ProviderId::Anthropic, "\"anthropic\""),
        (ProviderId::Copilot, "\"copilot\""),
        (ProviderId::DeepSeek, "\"deepseek\""),
        (ProviderId::Gemini, "\"gemini\""),
        (ProviderId::Kiro, "\"kiro\""),
        (ProviderId::Local, "\"local\""),
    ];
    for (provider, expected) in cases {
        assert_eq!(serde_json::to_string(&provider).unwrap(), expected);
        assert_eq!(
            serde_json::from_str::<ProviderId>(expected).unwrap(),
            provider
        );
    }
}

#[test]
fn duplicate_provider_ids_are_rejected() {
    let mut entries = registrations();
    entries[1].provider = ProviderId::OpenAi;
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&entries),
        Err(ProviderImplementationRegistryError::DuplicateProviderId { .. })
    ));
}

#[test]
fn duplicate_labels_and_aliases_are_rejected() {
    let mut labels = registrations();
    labels[1].canonical_label = " OPENAI ";
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&labels),
        Err(ProviderImplementationRegistryError::DuplicateCanonicalLabel { .. })
    ));

    let mut aliases = registrations();
    aliases[1].aliases = &[" OPENAI-COMPATIBLE "];
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&aliases),
        Err(ProviderImplementationRegistryError::DuplicateNormalizedAlias { .. })
    ));

    let mut collision = registrations();
    collision[1].aliases = &[" OPENAI "];
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&collision),
        Err(ProviderImplementationRegistryError::AliasCanonicalLabelCollision { .. })
    ));
}

#[test]
fn missing_or_mismatched_implementations_are_rejected() {
    let mut missing_adapter = registrations();
    missing_adapter[0].adapter = None;
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&missing_adapter),
        Err(ProviderImplementationRegistryError::MissingAdapter { .. })
    ));

    let mut missing_translator = registrations();
    missing_translator[0].translator = None;
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&missing_translator),
        Err(ProviderImplementationRegistryError::MissingTranslator { .. })
    ));

    let mut adapter_mismatch = registrations();
    adapter_mismatch[1].adapter = Some(StaticProviderAdapter::new(ProviderId::OpenAi));
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&adapter_mismatch),
        Err(ProviderImplementationRegistryError::AdapterProviderMismatch { .. })
    ));

    let mut translator_mismatch = registrations();
    translator_mismatch[1].translator = Some(&OPENAI_TRANSLATOR);
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&translator_mismatch),
        Err(ProviderImplementationRegistryError::TranslatorProviderMismatch { .. })
    ));
}

#[test]
fn incoherent_metadata_and_registration_order_are_rejected() {
    let mut endpoints = registrations();
    endpoints[0].supported_endpoints = &[];
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&endpoints),
        Err(ProviderImplementationRegistryError::MissingEndpointMetadata { .. })
    ));

    let mut capabilities = registrations();
    capabilities[0].capabilities = &[];
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&capabilities),
        Err(ProviderImplementationRegistryError::InconsistentCapabilityMetadata { .. })
    ));

    let mut models = registrations();
    models[0].model_catalog = &[];
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&models),
        Err(ProviderImplementationRegistryError::MissingModelCatalog { .. })
    ));

    let mut order = registrations();
    order.swap(0, 1);
    assert!(matches!(
        ProviderImplementationRegistry::from_registrations(&order),
        Err(ProviderImplementationRegistryError::NonDeterministicRegistrationOrder { .. })
    ));
}

#[test]
fn descriptors_have_coherent_adapter_translator_endpoint_and_runtime_metadata() {
    for descriptor in provider_implementation_registry().iter() {
        assert_eq!(descriptor.adapter().provider(), descriptor.provider());
        assert_eq!(descriptor.translator().provider(), descriptor.provider());
        assert_eq!(
            descriptor.translator().client_wire_format(),
            descriptor.client_request_format()
        );
        assert_eq!(
            descriptor.translator().upstream_wire_format(),
            descriptor.upstream_request_format()
        );
        assert!(!descriptor.supported_endpoints().is_empty());
        assert!(!descriptor.model_catalog().is_empty());
        for endpoint in ALL_PROVIDER_ENDPOINTS {
            assert_eq!(
                descriptor.supported_endpoints().contains(endpoint),
                descriptor.capability_status(*endpoint) != ProviderCapabilityStatus::Unsupported
            );
        }
        if let Some(metadata) = descriptor.runtime_metadata() {
            assert_eq!(metadata.provider, descriptor.provider());
        } else {
            assert_eq!(descriptor.provider(), ProviderId::OpenAi);
        }
    }
}

#[test]
fn contracts_catalogs_and_conformance_cover_registry_entries() {
    let contracts = provider_adapter_contract_matrix();
    assert_eq!(
        contracts.len(),
        provider_implementation_registry().iter().len()
    );
    for descriptor in provider_implementation_registry().iter() {
        assert!(
            contracts
                .iter()
                .any(|contract| contract.provider == descriptor.canonical_label())
        );
        assert!(!provider_catalog_entries_for(descriptor.provider()).is_empty());
        if descriptor
            .capabilities()
            .iter()
            .any(|(_, status)| *status == ProviderCapabilityStatus::Translated)
        {
            let coverage: Vec<_> = provider_conformance_cases()
                .iter()
                .filter(|case| case.provider == descriptor.provider())
                .collect();
            assert!(!coverage.is_empty());
            assert!(
                coverage
                    .iter()
                    .any(|case| { case.operation == ProviderConformanceOperation::Request })
            );
            assert!(
                coverage
                    .iter()
                    .any(|case| { case.operation == ProviderConformanceOperation::Response })
            );
        }
    }
}

#[test]
fn missing_translation_is_explicitly_unsupported_not_passthrough() {
    let original = br#"{"authorization":"Bearer test-only-value"}"#.to_vec();
    let result = provider_implementation_registry()
        .get(ProviderId::OpenAi)
        .unwrap()
        .translator()
        .transform_request(ProviderTransformInput::new(
            ProviderEndpoint::ResponsesCompact,
            original,
        ));
    assert!(matches!(
        result.loss,
        ProviderTransformLoss::UnsupportedUpstream { .. }
    ));
    assert_eq!(result.body, None);

    let emulated = provider_implementation_registry()
        .get(ProviderId::Anthropic)
        .unwrap()
        .translator()
        .transform_request(ProviderTransformInput::new(
            ProviderEndpoint::Models,
            br#"{"authorization":"Bearer test-only-value"}"#.to_vec(),
        ));
    assert!(matches!(
        emulated.loss,
        ProviderTransformLoss::UnsupportedUpstream { .. }
    ));
    assert_eq!(emulated.body, None);
}

#[test]
fn passthrough_is_declared_for_endpoint_and_wire_pair() {
    let local = provider_implementation_registry()
        .get(ProviderId::Local)
        .unwrap();
    assert!(local.declares_passthrough(
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::OpenAiResponses,
    ));
    assert!(!local.declares_passthrough(
        ProviderEndpoint::ResponsesCompact,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::OpenAiResponses,
    ));
    let openai = provider_implementation_registry()
        .get(ProviderId::OpenAi)
        .unwrap();
    assert!(openai.declares_passthrough(
        ProviderEndpoint::Responses,
        ProviderWireFormat::OpenAiResponses,
        ProviderWireFormat::OpenAiResponses,
    ));
}

#[test]
fn validation_errors_do_not_expose_registration_values() {
    const SECRET_LIKE: &str = "sk-test-only-super-secret-value";
    let mut entries = registrations();
    entries[0].aliases = &[SECRET_LIKE];
    entries[1].aliases = &[SECRET_LIKE];
    let error = match ProviderImplementationRegistry::from_registrations(&entries) {
        Ok(_) => panic!("duplicate secret-like aliases should be rejected"),
        Err(error) => error,
    };
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains(SECRET_LIKE));
}
