use super::*;

#[test]
fn provider_transform_input_debug_redacts_headers_and_body() {
    let mut input = ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        b"provider-transform-body-secret".to_vec(),
    );
    input.headers.insert(
        "authorization".to_string(),
        "Bearer provider-transform-header-secret".to_string(),
    );

    let rendered = format!("{input:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("provider-transform-header-secret"));
    assert!(!rendered.contains("provider-transform-body-secret"));
}

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
            ProviderEndpoint::ResponsesCompact,
            ProviderEndpoint::ChatCompletions,
            ProviderEndpoint::Models,
            ProviderEndpoint::Embeddings,
            ProviderEndpoint::Images,
            ProviderEndpoint::Audio,
            ProviderEndpoint::Batches,
        ]
    );
    assert_eq!(
        provider_supported_endpoints(ProviderId::Local),
        ALL_PROVIDER_ENDPOINTS
    );
    assert_eq!(
        provider_supported_endpoints(ProviderId::Gemini),
        [
            ProviderEndpoint::Responses,
            ProviderEndpoint::ResponsesCompact,
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
                ProviderEndpoint::ResponsesCompact,
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
