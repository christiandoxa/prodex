use prodex_cli::SubAgentReasoningEffort;
use prodex_provider_core::{
    PROVIDER_IMPLEMENTATION_ORDER, ProviderId, ProviderModelChoice, ProviderReasoningEffort,
    provider_catalog_entry, provider_implementation_registry, provider_runtime_metadata,
    resolve_provider_model_choices,
};

pub(crate) fn canonical_sub_agent_providers() -> &'static [ProviderId] {
    PROVIDER_IMPLEMENTATION_ORDER
}

pub(crate) fn canonical_sub_agent_model_choices(
    provider: ProviderId,
    current_model: Option<&str>,
) -> Vec<ProviderModelChoice> {
    resolve_provider_model_choices(provider, &[], current_model)
}

pub(crate) fn canonical_sub_agent_efforts(
    provider: ProviderId,
    model: Option<&str>,
) -> Vec<SubAgentReasoningEffort> {
    let Some(catalog_efforts) = model
        .or_else(|| provider_runtime_metadata(provider).map(|metadata| metadata.default_model))
        .and_then(|model| provider_catalog_entry(provider, model))
        .and_then(|entry| entry.supported_reasoning_efforts.as_deref())
    else {
        return SubAgentReasoningEffort::ALL.to_vec();
    };

    catalog_efforts
        .iter()
        .filter_map(|effort| match effort {
            ProviderReasoningEffort::None => Some(SubAgentReasoningEffort::None),
            ProviderReasoningEffort::Minimal => Some(SubAgentReasoningEffort::Minimal),
            ProviderReasoningEffort::Low => Some(SubAgentReasoningEffort::Low),
            ProviderReasoningEffort::Medium => Some(SubAgentReasoningEffort::Medium),
            ProviderReasoningEffort::High => Some(SubAgentReasoningEffort::High),
            ProviderReasoningEffort::XHigh => Some(SubAgentReasoningEffort::XHigh),
            ProviderReasoningEffort::Max => Some(SubAgentReasoningEffort::Max),
            ProviderReasoningEffort::Unknown => None,
        })
        .collect()
}

pub(crate) fn provider_display_name(provider: ProviderId) -> &'static str {
    provider_implementation_registry()
        .get(provider)
        .map(|descriptor| descriptor.display_name())
        .unwrap_or(provider.label())
}
