#[cfg(not(feature = "mojo-core"))]
use super::{
    MAX_RUNTIME_GATEWAY_PROVIDER_PRICED_MODELS,
    RUNTIME_GATEWAY_PROVIDER_REGISTRY_LEGACY_SCHEMA_VERSION,
    RUNTIME_GATEWAY_PROVIDER_REGISTRY_SCHEMA_VERSION,
    RuntimeGatewayProviderRegistryDescriptorArtifact,
};
use super::{RuntimeGatewayProviderModelCostArtifact, RuntimeGatewayProviderRegistryArtifact};
use anyhow::Result;
use std::collections::BTreeMap;

#[cfg(feature = "mojo-core")]
pub(super) fn runtime_gateway_validate_provider_registry_structure(
    artifact: &RuntimeGatewayProviderRegistryArtifact,
) -> Result<()> {
    use prodex_mojo_core::rich::ProviderRegistryDescriptorValidationInput;
    let descriptors = artifact
        .descriptors
        .iter()
        .map(|descriptor| ProviderRegistryDescriptorValidationInput {
            revision: descriptor.revision,
            pricing_revision: descriptor.pricing_revision,
            provider_code: provider_code(descriptor.provider),
            credential_valid: descriptor.credential_ref.is_well_formed(),
            endpoint_codes: descriptor
                .endpoints
                .iter()
                .copied()
                .map(endpoint_code)
                .collect(),
            regions: &descriptor.regions,
            cost: descriptor.cost,
            latency: descriptor.latency,
            risk: descriptor.risk,
            priority: descriptor.priority,
            model_cost_count: descriptor.model_costs.len(),
            pricing_authoritative: runtime_gateway_model_costs_are_authoritative(
                &descriptor.model_costs,
            ),
        })
        .collect::<Vec<_>>();
    let valid = prodex_mojo_core::rich::provider_registry_artifact_is_valid(
        artifact.schema_version,
        artifact.revision,
        artifact.pricing_revision,
        &descriptors,
    )
    .unwrap_or(false);
    if !valid {
        anyhow::bail!("provider registry artifact structure is invalid");
    }
    Ok(())
}

#[cfg(feature = "mojo-core")]
fn provider_code(provider: prodex_provider_core::ProviderId) -> u8 {
    use prodex_provider_core::ProviderId;
    match provider {
        ProviderId::OpenAi => 0,
        ProviderId::Anthropic => 1,
        ProviderId::Copilot => 2,
        ProviderId::DeepSeek => 3,
        ProviderId::Gemini => 4,
        ProviderId::Kiro => 5,
        ProviderId::Local => 6,
    }
}

#[cfg(feature = "mojo-core")]
fn endpoint_code(endpoint: prodex_provider_core::ProviderEndpoint) -> u8 {
    use prodex_provider_core::ProviderEndpoint;
    match endpoint {
        ProviderEndpoint::Responses => 0,
        ProviderEndpoint::ResponsesCompact => 1,
        ProviderEndpoint::ChatCompletions => 2,
        ProviderEndpoint::Messages => 3,
        ProviderEndpoint::Models => 4,
        ProviderEndpoint::Embeddings => 5,
        ProviderEndpoint::Images => 6,
        ProviderEndpoint::Audio => 7,
        ProviderEndpoint::Batches => 8,
        ProviderEndpoint::Rerank => 9,
        ProviderEndpoint::A2a => 10,
    }
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn runtime_gateway_validate_provider_registry_structure(
    artifact: &RuntimeGatewayProviderRegistryArtifact,
) -> Result<()> {
    if !matches!(
        artifact.schema_version,
        RUNTIME_GATEWAY_PROVIDER_REGISTRY_LEGACY_SCHEMA_VERSION
            | RUNTIME_GATEWAY_PROVIDER_REGISTRY_SCHEMA_VERSION
    ) || artifact.revision == 0
        || artifact.pricing_revision == 0
        || artifact.descriptors.is_empty()
        || artifact.descriptors.len() > prodex_provider_spi::MAX_GOVERNED_ROUTING_CANDIDATES
    {
        anyhow::bail!("provider registry artifact structure is invalid");
    }
    for (index, descriptor) in artifact.descriptors.iter().enumerate() {
        if !runtime_gateway_provider_descriptor_shape_is_valid(
            descriptor,
            artifact.pricing_revision,
            artifact.schema_version == RUNTIME_GATEWAY_PROVIDER_REGISTRY_SCHEMA_VERSION,
        ) || artifact.descriptors[..index]
            .iter()
            .any(|previous| previous.provider == descriptor.provider)
        {
            anyhow::bail!("provider registry artifact structure is invalid");
        }
    }
    Ok(())
}

#[cfg(not(feature = "mojo-core"))]
fn runtime_gateway_provider_descriptor_shape_is_valid(
    descriptor: &RuntimeGatewayProviderRegistryDescriptorArtifact,
    pricing_revision: u64,
    authoritative_pricing: bool,
) -> bool {
    descriptor.revision != 0
        && descriptor.pricing_revision != 0
        && descriptor.pricing_revision == pricing_revision
        && descriptor.credential_ref.is_well_formed()
        && !descriptor.endpoints.is_empty()
        && descriptor.endpoints.len() <= prodex_provider_core::ALL_PROVIDER_ENDPOINTS.len()
        && !values_have_duplicate(&descriptor.endpoints)
        && !descriptor.regions.is_empty()
        && descriptor.regions.len() <= prodex_provider_spi::MAX_GOVERNED_PROVIDER_REGIONS
        && !values_have_duplicate(&descriptor.regions)
        && [
            descriptor.cost,
            descriptor.latency,
            descriptor.risk,
            descriptor.priority,
        ]
        .into_iter()
        .all(|value| value <= prodex_provider_spi::ROUTING_SCORE_SCALE)
        && descriptor.model_costs.len() <= MAX_RUNTIME_GATEWAY_PROVIDER_PRICED_MODELS
        && (!authoritative_pricing && descriptor.model_costs.is_empty()
            || runtime_gateway_model_costs_are_authoritative(&descriptor.model_costs))
}

#[cfg(not(feature = "mojo-core"))]
fn values_have_duplicate<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
}

#[cfg(feature = "mojo-core")]
pub(super) fn runtime_gateway_model_costs_are_authoritative(
    model_costs: &BTreeMap<String, RuntimeGatewayProviderModelCostArtifact>,
) -> bool {
    let names = model_costs.keys().map(String::as_str).collect::<Vec<_>>();
    let input_present = model_costs
        .values()
        .map(|cost| cost.input_cost_per_million_microusd.is_some())
        .collect::<Vec<_>>();
    let output_present = model_costs
        .values()
        .map(|cost| cost.output_cost_per_million_microusd.is_some())
        .collect::<Vec<_>>();
    prodex_mojo_core::rich::provider_registry_pricing_is_authoritative(
        &names,
        &input_present,
        &output_present,
    )
    .unwrap_or(false)
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn runtime_gateway_model_costs_are_authoritative(
    model_costs: &BTreeMap<String, RuntimeGatewayProviderModelCostArtifact>,
) -> bool {
    !model_costs.is_empty()
        && model_costs.contains_key("*")
        && model_costs.iter().all(|(model, cost)| {
            !model.trim().is_empty()
                && model.len() <= 128
                && cost.input_cost_per_million_microusd.is_some()
                && cost.output_cost_per_million_microusd.is_some()
        })
        && !model_costs.keys().enumerate().any(|(index, model)| {
            model_costs
                .keys()
                .take(index)
                .any(|previous| previous.eq_ignore_ascii_case(model))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_pricing_requires_both_rates() {
        let mut costs = BTreeMap::from([(
            "*".to_string(),
            RuntimeGatewayProviderModelCostArtifact {
                input_cost_per_million_microusd: Some(1),
                output_cost_per_million_microusd: Some(2),
            },
        )]);
        assert!(runtime_gateway_model_costs_are_authoritative(&costs));
        costs.get_mut("*").unwrap().output_cost_per_million_microusd = None;
        assert!(!runtime_gateway_model_costs_are_authoritative(&costs));
    }
}
