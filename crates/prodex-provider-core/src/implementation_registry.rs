//! Immutable built-in provider implementation registry.

use std::{error::Error, fmt, sync::LazyLock};

use crate::models::{
    ANTHROPIC_MODELS, COPILOT_MODELS, DEEPSEEK_MODELS, GEMINI_MODELS, KIRO_MODELS, LOCAL_MODELS,
    OPENAI_MODELS,
};
use crate::runtime_metadata::{
    ANTHROPIC_RUNTIME_METADATA, COPILOT_RUNTIME_METADATA, DEEPSEEK_RUNTIME_METADATA,
    GEMINI_RUNTIME_METADATA, KIRO_RUNTIME_METADATA, LOCAL_RUNTIME_METADATA,
};
use crate::translators::{
    AnthropicTranslator, CopilotTranslator, DeepSeekTranslator, GeminiTranslator, KiroTranslator,
    PassthroughTranslator,
};
use crate::{
    ALL_PROVIDER_ENDPOINTS, COPILOT_TEXT_ENDPOINTS, CORE_TEXT_ENDPOINTS, GEMINI_ENDPOINTS,
    KIRO_ENDPOINTS, OPENAI_ENDPOINTS, ProviderCapabilityStatus, ProviderEndpoint, ProviderId,
    ProviderModelSpec, ProviderRuntimeMetadata, ProviderTranslator, ProviderWireFormat,
    StaticProviderAdapter,
};

pub const PROVIDER_IMPLEMENTATION_ORDER: &[ProviderId] = &[
    ProviderId::OpenAi,
    ProviderId::Anthropic,
    ProviderId::Copilot,
    ProviderId::DeepSeek,
    ProviderId::Gemini,
    ProviderId::Kiro,
    ProviderId::Local,
];

const OPENAI_CAPABILITIES: &[(ProviderEndpoint, ProviderCapabilityStatus)] = &[
    (
        ProviderEndpoint::Responses,
        ProviderCapabilityStatus::Native,
    ),
    (
        ProviderEndpoint::ChatCompletions,
        ProviderCapabilityStatus::Native,
    ),
    (ProviderEndpoint::Messages, ProviderCapabilityStatus::Native),
    (ProviderEndpoint::Models, ProviderCapabilityStatus::Native),
    (
        ProviderEndpoint::Embeddings,
        ProviderCapabilityStatus::Native,
    ),
    (ProviderEndpoint::Images, ProviderCapabilityStatus::Native),
    (ProviderEndpoint::Audio, ProviderCapabilityStatus::Native),
    (ProviderEndpoint::Batches, ProviderCapabilityStatus::Native),
    (ProviderEndpoint::Rerank, ProviderCapabilityStatus::Native),
    (ProviderEndpoint::A2a, ProviderCapabilityStatus::Native),
];
const CHAT_TRANSLATED_CAPABILITIES: &[(ProviderEndpoint, ProviderCapabilityStatus)] = &[
    (
        ProviderEndpoint::Responses,
        ProviderCapabilityStatus::Translated,
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
        ProviderCapabilityStatus::Translated,
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
const CHAT_PASSTHROUGH_ENDPOINTS: &[ProviderEndpoint] = &[
    ProviderEndpoint::ChatCompletions,
    ProviderEndpoint::Messages,
];
const COPILOT_PASSTHROUGH_ENDPOINTS: &[ProviderEndpoint] = &[
    ProviderEndpoint::ResponsesCompact,
    ProviderEndpoint::ChatCompletions,
    ProviderEndpoint::Messages,
];
const GEMINI_PASSTHROUGH_ENDPOINTS: &[ProviderEndpoint] = &[
    ProviderEndpoint::ChatCompletions,
    ProviderEndpoint::Messages,
    ProviderEndpoint::Embeddings,
];

static OPENAI_TRANSLATOR: PassthroughTranslator = PassthroughTranslator::new(ProviderId::OpenAi);
static ANTHROPIC_TRANSLATOR: AnthropicTranslator = AnthropicTranslator;
static COPILOT_TRANSLATOR: CopilotTranslator = CopilotTranslator;
static DEEPSEEK_TRANSLATOR: DeepSeekTranslator = DeepSeekTranslator;
static GEMINI_TRANSLATOR: GeminiTranslator = GeminiTranslator;
static KIRO_TRANSLATOR: KiroTranslator = KiroTranslator;
static LOCAL_TRANSLATOR: PassthroughTranslator = PassthroughTranslator::new(ProviderId::Local);

#[derive(Clone, Copy)]
struct ProviderImplementationRegistration {
    provider: ProviderId,
    canonical_label: &'static str,
    aliases: &'static [&'static str],
    adapter: Option<StaticProviderAdapter>,
    translator: Option<&'static dyn ProviderTranslator>,
    client_request_format: ProviderWireFormat,
    upstream_request_format: ProviderWireFormat,
    response_format: ProviderWireFormat,
    supports_streaming: bool,
    supports_model_fallback: bool,
    supported_endpoints: &'static [ProviderEndpoint],
    capabilities: &'static [(ProviderEndpoint, ProviderCapabilityStatus)],
    passthrough_endpoints: &'static [ProviderEndpoint],
    model_catalog: &'static [ProviderModelSpec],
    runtime_metadata: Option<&'static ProviderRuntimeMetadata>,
}

const BUILTIN_REGISTRATIONS: &[ProviderImplementationRegistration] = &[
    ProviderImplementationRegistration {
        provider: ProviderId::OpenAi,
        canonical_label: "openai",
        aliases: &["openai-responses", "openai_compatible", "openai-compatible"],
        adapter: Some(StaticProviderAdapter::new(ProviderId::OpenAi)),
        translator: Some(&OPENAI_TRANSLATOR),
        client_request_format: ProviderWireFormat::OpenAiResponses,
        upstream_request_format: ProviderWireFormat::OpenAiResponses,
        response_format: ProviderWireFormat::OpenAiResponses,
        supports_streaming: true,
        supports_model_fallback: false,
        supported_endpoints: OPENAI_ENDPOINTS,
        capabilities: OPENAI_CAPABILITIES,
        passthrough_endpoints: OPENAI_ENDPOINTS,
        model_catalog: OPENAI_MODELS,
        runtime_metadata: None,
    },
    ProviderImplementationRegistration {
        provider: ProviderId::Anthropic,
        canonical_label: "anthropic",
        aliases: &["claude"],
        adapter: Some(StaticProviderAdapter::new(ProviderId::Anthropic)),
        translator: Some(&ANTHROPIC_TRANSLATOR),
        client_request_format: ProviderWireFormat::OpenAiResponses,
        upstream_request_format: ProviderWireFormat::OpenAiChatCompletions,
        response_format: ProviderWireFormat::OpenAiResponses,
        supports_streaming: true,
        supports_model_fallback: true,
        supported_endpoints: CORE_TEXT_ENDPOINTS,
        capabilities: CHAT_TRANSLATED_CAPABILITIES,
        passthrough_endpoints: CHAT_PASSTHROUGH_ENDPOINTS,
        model_catalog: ANTHROPIC_MODELS,
        runtime_metadata: Some(&ANTHROPIC_RUNTIME_METADATA),
    },
    ProviderImplementationRegistration {
        provider: ProviderId::Copilot,
        canonical_label: "copilot",
        aliases: &["github-copilot", "github_copilot"],
        adapter: Some(StaticProviderAdapter::new(ProviderId::Copilot)),
        translator: Some(&COPILOT_TRANSLATOR),
        client_request_format: ProviderWireFormat::OpenAiResponses,
        upstream_request_format: ProviderWireFormat::OpenAiChatCompletions,
        response_format: ProviderWireFormat::OpenAiResponses,
        supports_streaming: true,
        supports_model_fallback: true,
        supported_endpoints: COPILOT_TEXT_ENDPOINTS,
        capabilities: COPILOT_CAPABILITIES,
        passthrough_endpoints: COPILOT_PASSTHROUGH_ENDPOINTS,
        model_catalog: COPILOT_MODELS,
        runtime_metadata: Some(&COPILOT_RUNTIME_METADATA),
    },
    ProviderImplementationRegistration {
        provider: ProviderId::DeepSeek,
        canonical_label: "deepseek",
        aliases: &[],
        adapter: Some(StaticProviderAdapter::new(ProviderId::DeepSeek)),
        translator: Some(&DEEPSEEK_TRANSLATOR),
        client_request_format: ProviderWireFormat::OpenAiResponses,
        upstream_request_format: ProviderWireFormat::OpenAiChatCompletions,
        response_format: ProviderWireFormat::OpenAiResponses,
        supports_streaming: true,
        supports_model_fallback: true,
        supported_endpoints: CORE_TEXT_ENDPOINTS,
        capabilities: CHAT_TRANSLATED_CAPABILITIES,
        passthrough_endpoints: CHAT_PASSTHROUGH_ENDPOINTS,
        model_catalog: DEEPSEEK_MODELS,
        runtime_metadata: Some(&DEEPSEEK_RUNTIME_METADATA),
    },
    ProviderImplementationRegistration {
        provider: ProviderId::Gemini,
        canonical_label: "gemini",
        aliases: &["google"],
        adapter: Some(StaticProviderAdapter::new(ProviderId::Gemini)),
        translator: Some(&GEMINI_TRANSLATOR),
        client_request_format: ProviderWireFormat::OpenAiResponses,
        upstream_request_format: ProviderWireFormat::GeminiGenerateContent,
        response_format: ProviderWireFormat::OpenAiResponses,
        supports_streaming: true,
        supports_model_fallback: true,
        supported_endpoints: GEMINI_ENDPOINTS,
        capabilities: GEMINI_CAPABILITIES,
        passthrough_endpoints: GEMINI_PASSTHROUGH_ENDPOINTS,
        model_catalog: GEMINI_MODELS,
        runtime_metadata: Some(&GEMINI_RUNTIME_METADATA),
    },
    ProviderImplementationRegistration {
        provider: ProviderId::Kiro,
        canonical_label: "kiro",
        aliases: &[],
        adapter: Some(StaticProviderAdapter::new(ProviderId::Kiro)),
        translator: Some(&KIRO_TRANSLATOR),
        client_request_format: ProviderWireFormat::OpenAiResponses,
        upstream_request_format: ProviderWireFormat::Passthrough,
        response_format: ProviderWireFormat::OpenAiResponses,
        supports_streaming: true,
        supports_model_fallback: false,
        supported_endpoints: KIRO_ENDPOINTS,
        capabilities: KIRO_CAPABILITIES,
        passthrough_endpoints: &[],
        model_catalog: KIRO_MODELS,
        runtime_metadata: Some(&KIRO_RUNTIME_METADATA),
    },
    ProviderImplementationRegistration {
        provider: ProviderId::Local,
        canonical_label: "local",
        aliases: &["local-openai", "local_openai"],
        adapter: Some(StaticProviderAdapter::new(ProviderId::Local)),
        translator: Some(&LOCAL_TRANSLATOR),
        client_request_format: ProviderWireFormat::OpenAiResponses,
        upstream_request_format: ProviderWireFormat::OpenAiResponses,
        response_format: ProviderWireFormat::OpenAiResponses,
        supports_streaming: true,
        supports_model_fallback: false,
        supported_endpoints: OPENAI_ENDPOINTS,
        capabilities: LOCAL_CAPABILITIES,
        passthrough_endpoints: OPENAI_ENDPOINTS,
        model_catalog: LOCAL_MODELS,
        runtime_metadata: Some(&LOCAL_RUNTIME_METADATA),
    },
];

pub struct ProviderImplementationDescriptor {
    registration: ProviderImplementationRegistration,
    adapter: StaticProviderAdapter,
    translator: &'static dyn ProviderTranslator,
}

impl ProviderImplementationDescriptor {
    pub const fn provider(&self) -> ProviderId {
        self.registration.provider
    }

    pub const fn canonical_label(&self) -> &'static str {
        self.registration.canonical_label
    }

    pub const fn accepted_aliases(&self) -> &'static [&'static str] {
        self.registration.aliases
    }

    pub const fn adapter(&self) -> StaticProviderAdapter {
        self.adapter
    }

    pub fn translator(&self) -> &'static dyn ProviderTranslator {
        self.translator
    }

    pub const fn client_request_format(&self) -> ProviderWireFormat {
        self.registration.client_request_format
    }

    pub const fn upstream_request_format(&self) -> ProviderWireFormat {
        self.registration.upstream_request_format
    }

    pub const fn response_format(&self) -> ProviderWireFormat {
        self.registration.response_format
    }

    pub const fn supports_streaming(&self) -> bool {
        self.registration.supports_streaming
    }

    pub const fn supports_model_fallback(&self) -> bool {
        self.registration.supports_model_fallback
    }

    pub const fn supported_endpoints(&self) -> &'static [ProviderEndpoint] {
        self.registration.supported_endpoints
    }

    pub const fn capabilities(&self) -> &'static [(ProviderEndpoint, ProviderCapabilityStatus)] {
        self.registration.capabilities
    }

    pub const fn passthrough_endpoints(&self) -> &'static [ProviderEndpoint] {
        self.registration.passthrough_endpoints
    }

    pub fn capability_status(&self, endpoint: ProviderEndpoint) -> ProviderCapabilityStatus {
        self.capabilities()
            .iter()
            .find_map(|(candidate, status)| (*candidate == endpoint).then_some(*status))
            .unwrap_or(ProviderCapabilityStatus::Unsupported)
    }

    pub const fn model_catalog(&self) -> &'static [ProviderModelSpec] {
        self.registration.model_catalog
    }

    pub const fn runtime_metadata(&self) -> Option<&'static ProviderRuntimeMetadata> {
        self.registration.runtime_metadata
    }

    pub fn declares_passthrough(
        &self,
        endpoint: ProviderEndpoint,
        from: ProviderWireFormat,
        to: ProviderWireFormat,
    ) -> bool {
        if !self.passthrough_endpoints().contains(&endpoint) {
            return false;
        }
        (from == self.client_request_format() && to == self.upstream_request_format())
            || (from == self.upstream_request_format() && to == self.response_format())
    }
}

pub struct ProviderImplementationRegistry {
    descriptors: Box<[ProviderImplementationDescriptor]>,
}

impl ProviderImplementationRegistry {
    fn from_registrations(
        registrations: &[ProviderImplementationRegistration],
    ) -> Result<Self, ProviderImplementationRegistryError> {
        validate_names(registrations)?;
        let mut descriptors = Vec::with_capacity(registrations.len());
        for (index, registration) in registrations.iter().copied().enumerate() {
            let adapter = registration
                .adapter
                .ok_or(ProviderImplementationRegistryError::MissingAdapter { index })?;
            let translator = registration
                .translator
                .ok_or(ProviderImplementationRegistryError::MissingTranslator { index })?;
            validate_registration(index, registration, adapter, translator)?;
            descriptors.push(ProviderImplementationDescriptor {
                registration,
                adapter,
                translator,
            });
        }
        validate_order(registrations)?;
        Ok(Self {
            descriptors: descriptors.into_boxed_slice(),
        })
    }

    pub fn get(&self, provider: ProviderId) -> Option<&ProviderImplementationDescriptor> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.provider() == provider)
    }

    pub fn resolve_alias(&self, value: &str) -> Option<ProviderId> {
        let value = value.trim();
        self.descriptors.iter().find_map(|descriptor| {
            (descriptor.canonical_label().eq_ignore_ascii_case(value)
                || descriptor
                    .accepted_aliases()
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(value)))
            .then_some(descriptor.provider())
        })
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ProviderImplementationDescriptor> {
        self.descriptors.iter()
    }

    pub fn descriptors(&self) -> &[ProviderImplementationDescriptor] {
        &self.descriptors
    }
}

pub fn provider_implementation_registry() -> &'static ProviderImplementationRegistry {
    static REGISTRY: LazyLock<ProviderImplementationRegistry> = LazyLock::new(|| {
        ProviderImplementationRegistry::from_registrations(BUILTIN_REGISTRATIONS)
            .expect("built-in provider implementation registry must be valid")
    });
    &REGISTRY
}

pub(crate) const fn builtin_provider_runtime_metadata(
    provider: ProviderId,
) -> Option<&'static ProviderRuntimeMetadata> {
    BUILTIN_REGISTRATIONS[provider as usize].runtime_metadata
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderImplementationRegistryError {
    DuplicateProviderId { first: usize, duplicate: usize },
    DuplicateCanonicalLabel { first: usize, duplicate: usize },
    DuplicateNormalizedAlias { first: usize, duplicate: usize },
    AliasCanonicalLabelCollision { alias: usize, label: usize },
    CanonicalLabelProviderMismatch { index: usize },
    MissingAdapter { index: usize },
    MissingTranslator { index: usize },
    AdapterProviderMismatch { index: usize },
    TranslatorProviderMismatch { index: usize },
    InconsistentWireFormatMetadata { index: usize },
    MissingEndpointMetadata { index: usize },
    InconsistentCapabilityMetadata { index: usize },
    MissingModelCatalog { index: usize },
    InconsistentModelCatalog { index: usize },
    MissingRuntimeMetadata { index: usize },
    UnexpectedRuntimeMetadata { index: usize },
    RuntimeMetadataProviderMismatch { index: usize },
    NonDeterministicRegistrationOrder { index: usize },
}

impl fmt::Display for ProviderImplementationRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid provider implementation registry: {self:?}"
        )
    }
}

impl Error for ProviderImplementationRegistryError {}

fn validate_names(
    registrations: &[ProviderImplementationRegistration],
) -> Result<(), ProviderImplementationRegistryError> {
    for (index, registration) in registrations.iter().enumerate() {
        for (previous, candidate) in registrations[..index].iter().enumerate() {
            if candidate.provider == registration.provider {
                return Err(ProviderImplementationRegistryError::DuplicateProviderId {
                    first: previous,
                    duplicate: index,
                });
            }
            if normalized(candidate.canonical_label) == normalized(registration.canonical_label) {
                return Err(
                    ProviderImplementationRegistryError::DuplicateCanonicalLabel {
                        first: previous,
                        duplicate: index,
                    },
                );
            }
        }
    }
    let mut seen_aliases = Vec::new();
    for (index, registration) in registrations.iter().enumerate() {
        for alias in registration.aliases {
            let normalized_alias = normalized(alias);
            if let Some((first, _)) = seen_aliases
                .iter()
                .find(|(_, candidate)| *candidate == normalized_alias)
            {
                return Err(
                    ProviderImplementationRegistryError::DuplicateNormalizedAlias {
                        first: *first,
                        duplicate: index,
                    },
                );
            }
            for (label_index, candidate) in registrations.iter().enumerate() {
                if normalized(candidate.canonical_label) == normalized_alias {
                    return Err(
                        ProviderImplementationRegistryError::AliasCanonicalLabelCollision {
                            alias: index,
                            label: label_index,
                        },
                    );
                }
            }
            seen_aliases.push((index, normalized_alias));
        }
    }
    Ok(())
}

fn validate_registration(
    index: usize,
    registration: ProviderImplementationRegistration,
    adapter: StaticProviderAdapter,
    translator: &'static dyn ProviderTranslator,
) -> Result<(), ProviderImplementationRegistryError> {
    if normalized(registration.canonical_label) != registration.provider.label() {
        return Err(ProviderImplementationRegistryError::CanonicalLabelProviderMismatch { index });
    }
    if adapter.provider() != registration.provider {
        return Err(ProviderImplementationRegistryError::AdapterProviderMismatch { index });
    }
    if translator.provider() != registration.provider {
        return Err(ProviderImplementationRegistryError::TranslatorProviderMismatch { index });
    }
    if translator.client_wire_format() != registration.client_request_format
        || translator.upstream_wire_format() != registration.upstream_request_format
        || registration.response_format != registration.client_request_format
    {
        return Err(ProviderImplementationRegistryError::InconsistentWireFormatMetadata { index });
    }
    if registration.supported_endpoints.is_empty() {
        return Err(ProviderImplementationRegistryError::MissingEndpointMetadata { index });
    }
    if !capabilities_are_coherent(registration) {
        return Err(ProviderImplementationRegistryError::InconsistentCapabilityMetadata { index });
    }
    if registration.model_catalog.is_empty() {
        return Err(ProviderImplementationRegistryError::MissingModelCatalog { index });
    }
    if registration.model_catalog.iter().any(|model| {
        model.provider != registration.provider
            || model.endpoints.is_empty()
            || model
                .endpoints
                .iter()
                .any(|endpoint| !registration.supported_endpoints.contains(endpoint))
    }) {
        return Err(ProviderImplementationRegistryError::InconsistentModelCatalog { index });
    }
    match (registration.provider, registration.runtime_metadata) {
        (ProviderId::OpenAi, Some(_)) => {
            return Err(ProviderImplementationRegistryError::UnexpectedRuntimeMetadata { index });
        }
        (ProviderId::OpenAi, None) => {}
        (_, None) => {
            return Err(ProviderImplementationRegistryError::MissingRuntimeMetadata { index });
        }
        (_, Some(metadata)) if metadata.provider != registration.provider => {
            return Err(
                ProviderImplementationRegistryError::RuntimeMetadataProviderMismatch { index },
            );
        }
        (_, Some(_)) => {}
    }
    Ok(())
}

fn capabilities_are_coherent(registration: ProviderImplementationRegistration) -> bool {
    if registration.capabilities.len() != registration.supported_endpoints.len() {
        return false;
    }
    for (index, endpoint) in registration.supported_endpoints.iter().enumerate() {
        if registration.supported_endpoints[..index].contains(endpoint) {
            return false;
        }
        let statuses: Vec<_> = registration
            .capabilities
            .iter()
            .filter_map(|(candidate, status)| (*candidate == *endpoint).then_some(*status))
            .collect();
        if statuses.len() != 1
            || matches!(
                statuses[0],
                ProviderCapabilityStatus::Unsupported | ProviderCapabilityStatus::Untested
            )
        {
            return false;
        }
    }
    let capabilities_valid = registration.capabilities.iter().all(|(endpoint, status)| {
        registration.supported_endpoints.contains(endpoint)
            && ALL_PROVIDER_ENDPOINTS.contains(endpoint)
            && (*status != ProviderCapabilityStatus::Passthrough
                || registration.passthrough_endpoints.contains(endpoint))
    });
    capabilities_valid
        && registration.passthrough_endpoints.iter().all(|endpoint| {
            registration.supported_endpoints.contains(endpoint)
                && matches!(
                    registration
                        .capabilities
                        .iter()
                        .find_map(|(candidate, status)| {
                            (*candidate == *endpoint).then_some(*status)
                        }),
                    Some(ProviderCapabilityStatus::Native | ProviderCapabilityStatus::Passthrough)
                )
        })
}

fn validate_order(
    registrations: &[ProviderImplementationRegistration],
) -> Result<(), ProviderImplementationRegistryError> {
    let count = registrations.len().max(PROVIDER_IMPLEMENTATION_ORDER.len());
    for index in 0..count {
        if registrations.get(index).map(|entry| entry.provider)
            != PROVIDER_IMPLEMENTATION_ORDER.get(index).copied()
        {
            return Err(
                ProviderImplementationRegistryError::NonDeterministicRegistrationOrder { index },
            );
        }
    }
    Ok(())
}

fn normalized(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
#[path = "implementation_registry/tests.rs"]
mod tests;
