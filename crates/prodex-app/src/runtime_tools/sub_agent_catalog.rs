use prodex_cli::SubAgentReasoningEffort;
use prodex_provider_core::{
    PROVIDER_IMPLEMENTATION_ORDER, ProviderId, ProviderModelChoice,
    provider_implementation_registry, provider_model_reasoning_resolution,
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
    let resolution = provider_model_reasoning_resolution(provider, model, None)
        .expect("provider model reasoning resolution failed");
    if resolution.model_index.is_none() {
        return SubAgentReasoningEffort::ALL.to_vec();
    }

    resolution
        .supported_reasoning_efforts
        .iter()
        .filter_map(|effort| match effort {
            prodex_provider_core::ProviderReasoningEffort::None => {
                Some(SubAgentReasoningEffort::None)
            }
            prodex_provider_core::ProviderReasoningEffort::Minimal => {
                Some(SubAgentReasoningEffort::Minimal)
            }
            prodex_provider_core::ProviderReasoningEffort::Low => {
                Some(SubAgentReasoningEffort::Low)
            }
            prodex_provider_core::ProviderReasoningEffort::Medium => {
                Some(SubAgentReasoningEffort::Medium)
            }
            prodex_provider_core::ProviderReasoningEffort::High => {
                Some(SubAgentReasoningEffort::High)
            }
            prodex_provider_core::ProviderReasoningEffort::XHigh => {
                Some(SubAgentReasoningEffort::XHigh)
            }
            prodex_provider_core::ProviderReasoningEffort::Max => {
                Some(SubAgentReasoningEffort::Max)
            }
            prodex_provider_core::ProviderReasoningEffort::Ultra => {
                Some(SubAgentReasoningEffort::Ultra)
            }
            prodex_provider_core::ProviderReasoningEffort::Unknown => None,
        })
        .collect()
}

pub(crate) fn provider_display_name(provider: ProviderId) -> &'static str {
    provider_implementation_registry()
        .get(provider)
        .map(|descriptor| descriptor.display_name())
        .unwrap_or(provider.label())
}
