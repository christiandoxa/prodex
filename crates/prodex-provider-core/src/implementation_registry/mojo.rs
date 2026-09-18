use super::*;

fn provider_from_i64(value: i64) -> Option<ProviderId> {
    PROVIDER_IMPLEMENTATION_ORDER
        .get(usize::try_from(value).ok()?)
        .copied()
}

fn wire_format(value: i64) -> ProviderWireFormat {
    match value {
        0 => ProviderWireFormat::OpenAiResponses,
        1 => ProviderWireFormat::OpenAiChatCompletions,
        2 => ProviderWireFormat::AnthropicMessages,
        3 => ProviderWireFormat::GeminiGenerateContent,
        4 => ProviderWireFormat::Passthrough,
        _ => panic!("validated Mojo provider-registry wire format"),
    }
}

fn capability_status(value: i64) -> ProviderCapabilityStatus {
    match value {
        0 => ProviderCapabilityStatus::Native,
        1 => ProviderCapabilityStatus::Translated,
        2 => ProviderCapabilityStatus::Passthrough,
        3 => ProviderCapabilityStatus::Emulated,
        4 => ProviderCapabilityStatus::Partial,
        5 => ProviderCapabilityStatus::Unsupported,
        6 => ProviderCapabilityStatus::Untested,
        _ => panic!("validated Mojo provider-registry capability status"),
    }
}

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

fn model_catalog(provider: ProviderId) -> &'static [ProviderModelSpec] {
    builtin_model_catalog(provider)
}

fn runtime_metadata(provider: ProviderId) -> Option<&'static ProviderRuntimeMetadata> {
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

fn selected_endpoints(mask: u64) -> &'static [ProviderEndpoint] {
    let values = ALL_PROVIDER_ENDPOINTS
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, endpoint)| (mask & (1_u64 << index) != 0).then_some(endpoint))
        .collect::<Vec<_>>();
    Box::leak(values.into_boxed_slice())
}

fn capabilities(
    plan: prodex_mojo_core::provider_registry::ProviderRegistryPlan,
) -> &'static [(ProviderEndpoint, ProviderCapabilityStatus)] {
    let values = ALL_PROVIDER_ENDPOINTS
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, endpoint)| {
            (plan.endpoint_mask & (1_u64 << index) != 0)
                .then_some((endpoint, capability_status(plan.capability_statuses[index])))
        })
        .collect::<Vec<_>>();
    Box::leak(values.into_boxed_slice())
}

fn registration(provider: ProviderId) -> ProviderImplementationRegistration {
    let plan = prodex_mojo_core::provider_registry::provider_plan(provider as i64)
        .expect("Mojo provider implementation registry plan returned invalid output");
    ProviderImplementationRegistration {
        provider,
        aliases: aliases(provider),
        adapter: StaticProviderAdapter::new(provider),
        translator: translator(provider),
        client_request_format: wire_format(plan.client_wire),
        upstream_request_format: wire_format(plan.upstream_wire),
        response_format: wire_format(plan.response_wire),
        supports_streaming: plan.supports_streaming,
        supports_model_fallback: plan.supports_model_fallback,
        supported_endpoints: selected_endpoints(plan.endpoint_mask),
        capabilities: capabilities(plan),
        passthrough_endpoints: selected_endpoints(plan.passthrough_mask),
        model_catalog: model_catalog(provider),
        runtime_metadata: runtime_metadata(provider),
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
    prodex_mojo_core::provider_registry::resolve_alias(value)
        .expect("Mojo provider alias resolution returned invalid output")
        .and_then(provider_from_i64)
}

pub(super) fn resolve_model_provider_id(value: &str) -> Option<ProviderId> {
    prodex_mojo_core::provider_registry::resolve_model_provider_id(value)
        .expect("Mojo provider model-provider resolution returned invalid output")
        .and_then(provider_from_i64)
}
