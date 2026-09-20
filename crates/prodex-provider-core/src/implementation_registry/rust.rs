#[cfg(not(feature = "mojo"))]
mod implementation {
    //! Feature-off provider registry oracle.
    //!
    //! Production Mojo builds use implementation_registry/mojo.rs. This module
    //! preserves the same immutable contract when the Mojo feature is disabled.

    use super::super::*;

    const TEXT_ENDPOINTS: &[ProviderEndpoint] = &[
        ProviderEndpoint::Responses,
        ProviderEndpoint::ResponsesCompact,
        ProviderEndpoint::ChatCompletions,
        ProviderEndpoint::Messages,
        ProviderEndpoint::Models,
    ];
    const GEMINI_ENDPOINTS: &[ProviderEndpoint] = &[
        ProviderEndpoint::Responses,
        ProviderEndpoint::ResponsesCompact,
        ProviderEndpoint::ChatCompletions,
        ProviderEndpoint::Messages,
        ProviderEndpoint::Models,
        ProviderEndpoint::Embeddings,
    ];
    const OPENAI_ENDPOINTS: &[ProviderEndpoint] = &[
        ProviderEndpoint::Responses,
        ProviderEndpoint::ResponsesCompact,
        ProviderEndpoint::ChatCompletions,
        ProviderEndpoint::Models,
        ProviderEndpoint::Embeddings,
        ProviderEndpoint::Images,
        ProviderEndpoint::Audio,
        ProviderEndpoint::Batches,
    ];

    const OPENAI_CAPABILITIES: &[(ProviderEndpoint, ProviderCapabilityStatus)] = &[
        (
            ProviderEndpoint::Responses,
            ProviderCapabilityStatus::Native,
        ),
        (
            ProviderEndpoint::ResponsesCompact,
            ProviderCapabilityStatus::Emulated,
        ),
        (
            ProviderEndpoint::ChatCompletions,
            ProviderCapabilityStatus::Native,
        ),
        (ProviderEndpoint::Models, ProviderCapabilityStatus::Native),
        (
            ProviderEndpoint::Embeddings,
            ProviderCapabilityStatus::Native,
        ),
        (ProviderEndpoint::Images, ProviderCapabilityStatus::Native),
        (ProviderEndpoint::Audio, ProviderCapabilityStatus::Native),
        (ProviderEndpoint::Batches, ProviderCapabilityStatus::Native),
    ];
    const CHAT_TRANSLATED_CAPABILITIES: &[(ProviderEndpoint, ProviderCapabilityStatus)] = &[
        (
            ProviderEndpoint::Responses,
            ProviderCapabilityStatus::Translated,
        ),
        (
            ProviderEndpoint::ResponsesCompact,
            ProviderCapabilityStatus::Emulated,
        ),
        (
            ProviderEndpoint::ChatCompletions,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::Messages,
            ProviderCapabilityStatus::Passthrough,
        ),
        (ProviderEndpoint::Models, ProviderCapabilityStatus::Emulated),
    ];
    const COPILOT_CAPABILITIES: &[(ProviderEndpoint, ProviderCapabilityStatus)] = &[
        (
            ProviderEndpoint::Responses,
            ProviderCapabilityStatus::Native,
        ),
        (
            ProviderEndpoint::ResponsesCompact,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::ChatCompletions,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::Messages,
            ProviderCapabilityStatus::Passthrough,
        ),
        (ProviderEndpoint::Models, ProviderCapabilityStatus::Emulated),
    ];
    const GEMINI_CAPABILITIES: &[(ProviderEndpoint, ProviderCapabilityStatus)] = &[
        (
            ProviderEndpoint::Responses,
            ProviderCapabilityStatus::Translated,
        ),
        (
            ProviderEndpoint::ResponsesCompact,
            ProviderCapabilityStatus::Emulated,
        ),
        (
            ProviderEndpoint::ChatCompletions,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::Messages,
            ProviderCapabilityStatus::Passthrough,
        ),
        (ProviderEndpoint::Models, ProviderCapabilityStatus::Emulated),
        (
            ProviderEndpoint::Embeddings,
            ProviderCapabilityStatus::Passthrough,
        ),
    ];
    const KIRO_CAPABILITIES: &[(ProviderEndpoint, ProviderCapabilityStatus)] = &[
        (
            ProviderEndpoint::Responses,
            ProviderCapabilityStatus::Translated,
        ),
        (
            ProviderEndpoint::ResponsesCompact,
            ProviderCapabilityStatus::Emulated,
        ),
        (
            ProviderEndpoint::ChatCompletions,
            ProviderCapabilityStatus::Translated,
        ),
        (
            ProviderEndpoint::Messages,
            ProviderCapabilityStatus::Translated,
        ),
        (ProviderEndpoint::Models, ProviderCapabilityStatus::Emulated),
    ];
    const LOCAL_CAPABILITIES: &[(ProviderEndpoint, ProviderCapabilityStatus)] = &[
        (
            ProviderEndpoint::Responses,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::ResponsesCompact,
            ProviderCapabilityStatus::Emulated,
        ),
        (
            ProviderEndpoint::ChatCompletions,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::Messages,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::Models,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::Embeddings,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::Images,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::Audio,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::Batches,
            ProviderCapabilityStatus::Passthrough,
        ),
        (
            ProviderEndpoint::Rerank,
            ProviderCapabilityStatus::Passthrough,
        ),
        (ProviderEndpoint::A2a, ProviderCapabilityStatus::Passthrough),
    ];

    const OPENAI_PASSTHROUGH: &[ProviderEndpoint] = &[
        ProviderEndpoint::Responses,
        ProviderEndpoint::ChatCompletions,
        ProviderEndpoint::Models,
        ProviderEndpoint::Embeddings,
        ProviderEndpoint::Images,
        ProviderEndpoint::Audio,
        ProviderEndpoint::Batches,
    ];
    const CHAT_PASSTHROUGH: &[ProviderEndpoint] = &[
        ProviderEndpoint::ChatCompletions,
        ProviderEndpoint::Messages,
    ];
    const COPILOT_PASSTHROUGH: &[ProviderEndpoint] = &[
        ProviderEndpoint::Responses,
        ProviderEndpoint::ResponsesCompact,
        ProviderEndpoint::ChatCompletions,
        ProviderEndpoint::Messages,
    ];
    const GEMINI_PASSTHROUGH: &[ProviderEndpoint] = &[
        ProviderEndpoint::ChatCompletions,
        ProviderEndpoint::Messages,
        ProviderEndpoint::Embeddings,
    ];
    const LOCAL_PASSTHROUGH: &[ProviderEndpoint] = &[
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
    ];

    fn aliases(provider: ProviderId) -> &'static [&'static str] {
        match provider {
            ProviderId::OpenAi => &["openai-responses", "openai_compatible", "openai-compatible"],
            ProviderId::Anthropic => &["claude"],
            ProviderId::Copilot => &["github-copilot", "github_copilot"],
            ProviderId::DeepSeek | ProviderId::Kiro => &[],
            ProviderId::Gemini => &["google"],
            ProviderId::Local => &["local-openai", "local_openai"],
        }
    }

    fn translator(provider: ProviderId) -> &'static dyn ProviderTranslator {
        match provider {
            ProviderId::OpenAi => &OPENAI_TRANSLATOR,
            ProviderId::Anthropic => &ANTHROPIC_TRANSLATOR,
            ProviderId::Copilot => &COPILOT_TRANSLATOR,
            ProviderId::DeepSeek => &DEEPSEEK_TRANSLATOR,
            ProviderId::Gemini => &GEMINI_TRANSLATOR,
            ProviderId::Kiro => &KIRO_TRANSLATOR,
            ProviderId::Local => &LOCAL_TRANSLATOR,
        }
    }

    fn registration(provider: ProviderId) -> ProviderImplementationRegistration {
        let (
            upstream_request_format,
            supports_model_fallback,
            supported_endpoints,
            capabilities,
            passthrough_endpoints,
        ) = match provider {
            ProviderId::OpenAi => (
                ProviderWireFormat::OpenAiResponses,
                false,
                OPENAI_ENDPOINTS,
                OPENAI_CAPABILITIES,
                OPENAI_PASSTHROUGH,
            ),
            ProviderId::Anthropic | ProviderId::DeepSeek => (
                ProviderWireFormat::OpenAiChatCompletions,
                true,
                TEXT_ENDPOINTS,
                CHAT_TRANSLATED_CAPABILITIES,
                CHAT_PASSTHROUGH,
            ),
            ProviderId::Copilot => (
                ProviderWireFormat::OpenAiResponses,
                true,
                TEXT_ENDPOINTS,
                COPILOT_CAPABILITIES,
                COPILOT_PASSTHROUGH,
            ),
            ProviderId::Gemini => (
                ProviderWireFormat::GeminiGenerateContent,
                true,
                GEMINI_ENDPOINTS,
                GEMINI_CAPABILITIES,
                GEMINI_PASSTHROUGH,
            ),
            ProviderId::Kiro => (
                ProviderWireFormat::Passthrough,
                false,
                TEXT_ENDPOINTS,
                KIRO_CAPABILITIES,
                &[] as &[ProviderEndpoint],
            ),
            ProviderId::Local => (
                ProviderWireFormat::OpenAiResponses,
                false,
                ALL_PROVIDER_ENDPOINTS,
                LOCAL_CAPABILITIES,
                LOCAL_PASSTHROUGH,
            ),
        };

        ProviderImplementationRegistration {
            provider,
            aliases: aliases(provider),
            adapter: StaticProviderAdapter::new(provider),
            translator: translator(provider),
            client_request_format: ProviderWireFormat::OpenAiResponses,
            upstream_request_format,
            response_format: ProviderWireFormat::OpenAiResponses,
            supports_streaming: true,
            supports_model_fallback,
            supported_endpoints,
            capabilities,
            passthrough_endpoints,
            model_catalog: builtin_model_catalog(provider),
            runtime_metadata: builtin_provider_runtime_metadata(provider),
        }
    }

    pub(super) fn builtin_registrations() -> &'static [ProviderImplementationRegistration] {
        static REGISTRATIONS: LazyLock<Box<[ProviderImplementationRegistration]>> =
            LazyLock::new(|| {
                PROVIDER_IMPLEMENTATION_ORDER
                    .iter()
                    .copied()
                    .map(registration)
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            });
        REGISTRATIONS.as_ref()
    }

    pub(super) fn resolve_alias(value: &str) -> Option<ProviderId> {
        let value = value.trim();
        builtin_registrations().iter().find_map(|registration| {
            (registration.provider.label().eq_ignore_ascii_case(value)
                || registration
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(value)))
            .then_some(registration.provider)
        })
    }

    pub(super) fn resolve_model_provider_id(value: &str) -> Option<ProviderId> {
        let value = value.trim();
        resolve_alias(value).or_else(|| {
            builtin_registrations().iter().find_map(|registration| {
                registration
                    .runtime_metadata
                    .is_some_and(|metadata| metadata.model_provider_id.eq_ignore_ascii_case(value))
                    .then_some(registration.provider)
            })
        })
    }
}
#[cfg(not(feature = "mojo"))]
pub(super) fn builtin_registrations() -> &'static [super::ProviderImplementationRegistration] {
    implementation::builtin_registrations()
}

#[cfg(not(feature = "mojo"))]
pub(super) fn resolve_alias(value: &str) -> Option<crate::ProviderId> {
    implementation::resolve_alias(value)
}

#[cfg(not(feature = "mojo"))]
pub(super) fn resolve_model_provider_id(value: &str) -> Option<crate::ProviderId> {
    implementation::resolve_model_provider_id(value)
}
