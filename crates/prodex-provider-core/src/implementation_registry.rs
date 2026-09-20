//! Immutable built-in provider implementation registry.

use crate::models::builtin_model_catalog;
use crate::runtime_metadata::{
    ANTHROPIC_RUNTIME_METADATA, COPILOT_RUNTIME_METADATA, DEEPSEEK_RUNTIME_METADATA,
    GEMINI_RUNTIME_METADATA, KIRO_RUNTIME_METADATA, LOCAL_RUNTIME_METADATA,
};
use crate::translators::{
    AnthropicTranslator, DeepSeekTranslator, GeminiTranslator, KiroTranslator,
    PassthroughTranslator,
};
use crate::{
    ALL_PROVIDER_ENDPOINTS, ProviderCapabilityStatus, ProviderEndpoint, ProviderId,
    ProviderModelSpec, ProviderRuntimeMetadata, ProviderTranslator, ProviderWireFormat,
    StaticProviderAdapter,
};
use std::sync::LazyLock;

#[cfg(feature = "mojo")]
#[path = "implementation_registry/mojo.rs"]
mod mojo;
#[cfg(not(feature = "mojo"))]
#[path = "implementation_registry/rust.rs"]
mod mojo;

pub const PROVIDER_IMPLEMENTATION_ORDER: &[ProviderId] = &[
    ProviderId::OpenAi,
    ProviderId::Anthropic,
    ProviderId::Copilot,
    ProviderId::DeepSeek,
    ProviderId::Gemini,
    ProviderId::Kiro,
    ProviderId::Local,
];

static OPENAI_TRANSLATOR: PassthroughTranslator = PassthroughTranslator::new(ProviderId::OpenAi);
static ANTHROPIC_TRANSLATOR: AnthropicTranslator = AnthropicTranslator;
static COPILOT_TRANSLATOR: PassthroughTranslator = PassthroughTranslator::new(ProviderId::Copilot);
static DEEPSEEK_TRANSLATOR: DeepSeekTranslator = DeepSeekTranslator;
static GEMINI_TRANSLATOR: GeminiTranslator = GeminiTranslator;
static KIRO_TRANSLATOR: KiroTranslator = KiroTranslator;
static LOCAL_TRANSLATOR: PassthroughTranslator = PassthroughTranslator::new(ProviderId::Local);

#[derive(Clone, Copy)]
struct ProviderImplementationRegistration {
    provider: ProviderId,
    aliases: &'static [&'static str],
    adapter: StaticProviderAdapter,
    translator: &'static dyn ProviderTranslator,
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
        self.registration.provider.label()
    }

    pub const fn display_name(&self) -> &'static str {
        match self.registration.runtime_metadata {
            Some(metadata) => metadata.display_name,
            None if matches!(self.registration.provider, ProviderId::OpenAi) => "OpenAI",
            None => self.registration.provider.label(),
        }
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
    pub fn get(&self, provider: ProviderId) -> Option<&ProviderImplementationDescriptor> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.provider() == provider)
    }

    pub fn resolve_alias(&self, value: &str) -> Option<ProviderId> {
        mojo::resolve_alias(value)
    }

    pub fn resolve_model_provider_id(&self, value: &str) -> Option<ProviderId> {
        mojo::resolve_model_provider_id(value)
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
        let descriptors = mojo::builtin_registrations()
            .iter()
            .copied()
            .map(|registration| ProviderImplementationDescriptor {
                adapter: registration.adapter,
                translator: registration.translator,
                registration,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        ProviderImplementationRegistry { descriptors }
    });
    &REGISTRY
}

pub(crate) const fn builtin_provider_runtime_metadata(
    provider: ProviderId,
) -> Option<&'static ProviderRuntimeMetadata> {
    match provider {
        ProviderId::OpenAi => None,
        ProviderId::Anthropic => Some(&ANTHROPIC_RUNTIME_METADATA),
        ProviderId::Copilot => Some(&COPILOT_RUNTIME_METADATA),
        ProviderId::DeepSeek => Some(&DEEPSEEK_RUNTIME_METADATA),
        ProviderId::Gemini => Some(&GEMINI_RUNTIME_METADATA),
        ProviderId::Kiro => Some(&KIRO_RUNTIME_METADATA),
        ProviderId::Local => Some(&LOCAL_RUNTIME_METADATA),
    }
}
