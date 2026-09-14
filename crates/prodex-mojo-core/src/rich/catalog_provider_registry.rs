use super::{
    RICH_ABI_VERSION, RichStringView, ensure_rich_abi, mojo_mut_pointer_address,
    mojo_pointer_address, status_error, view,
};
use crate::MojoError;

const MAX_PROVIDER_REGISTRY_MODEL_NAMES: usize = 65_536;
const MAX_PROVIDER_REGISTRY_MODEL_NAME_BYTES: usize = 4_096;

/// One deduplicated model or alias emitted by the provider-registry planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRegistryModelCostEntry {
    pub name_index: usize,
    pub input_cost_per_million_microusd: u64,
    pub output_cost_per_million_microusd: u64,
}

/// Bounded catalog cost normalization result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRegistryModelCostPlan {
    pub entries: Vec<ProviderRegistryModelCostEntry>,
    pub fallback_input_cost_per_million_microusd: u64,
    pub fallback_output_cost_per_million_microusd: u64,
    pub pricing_known: bool,
}

/// Borrowed structural facts for one provider-registry descriptor.
pub struct ProviderRegistryDescriptorValidationInput<'a> {
    pub revision: u64,
    pub pricing_revision: u64,
    pub provider_code: u8,
    pub credential_valid: bool,
    pub endpoint_codes: Vec<u8>,
    pub regions: &'a [String],
    pub cost: u16,
    pub latency: u16,
    pub risk: u16,
    pub priority: u16,
    pub model_cost_count: usize,
    pub pricing_authoritative: bool,
}

unsafe extern "C" {
    fn prodex_mojo_provider_registry_artifact_structure_v1(
        abi_version: i64,
        schema_version: u64,
        revision: u64,
        pricing_revision: u64,
        descriptor_fields: u64,
        descriptor_count: i64,
        regions: u64,
        region_count: i64,
        valid: u64,
    ) -> i64;
    fn prodex_mojo_provider_registry_pricing_authority_v1(
        abi_version: i64,
        names: u64,
        input_present: u64,
        output_present: u64,
        name_count: i64,
        authoritative: u64,
    ) -> i64;
    fn prodex_mojo_rich_catalog_provider_registry_costs_v1(
        abi_version: i64,
        names: u64,
        input_costs: u64,
        input_present: u64,
        output_costs: u64,
        output_present: u64,
        name_count: i64,
        accepted_indices: u64,
        normalized_input_costs: u64,
        normalized_output_costs: u64,
        output_capacity: i64,
        output_count: u64,
        fallback_input_cost: u64,
        fallback_output_cost: u64,
        pricing_present: u64,
    ) -> i64;
}

/// Validate the complete bounded structural contract of a provider-registry artifact.
pub fn provider_registry_artifact_is_valid(
    schema_version: u32,
    revision: u64,
    pricing_revision: u64,
    descriptors: &[ProviderRegistryDescriptorValidationInput<'_>],
) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    const WIDTH: usize = 14;
    let mut fields = Vec::with_capacity(descriptors.len() * WIDTH);
    let mut regions = Vec::new();
    for descriptor in descriptors {
        let region_offset = regions.len();
        regions.extend(descriptor.regions.iter().map(|region| view(region)));
        let endpoint_mask = descriptor
            .endpoint_codes
            .iter()
            .fold(0_u64, |mask, endpoint| {
                mask | 1_u64.checked_shl(u32::from(*endpoint)).unwrap_or_default()
            });
        fields.extend([
            descriptor.revision,
            descriptor.pricing_revision,
            u64::from(descriptor.provider_code),
            u64::from(descriptor.credential_valid),
            endpoint_mask,
            u64::try_from(descriptor.endpoint_codes.len()).unwrap_or(u64::MAX),
            u64::try_from(region_offset).unwrap_or(u64::MAX),
            u64::try_from(descriptor.regions.len()).unwrap_or(u64::MAX),
            u64::from(descriptor.cost),
            u64::from(descriptor.latency),
            u64::from(descriptor.risk),
            u64::from(descriptor.priority),
            u64::try_from(descriptor.model_cost_count).unwrap_or(u64::MAX),
            u64::from(descriptor.pricing_authoritative),
        ]);
    }
    let mut valid = -1_i64;
    let status = unsafe {
        prodex_mojo_provider_registry_artifact_structure_v1(
            RICH_ABI_VERSION,
            u64::from(schema_version),
            revision,
            pricing_revision,
            address(&fields),
            i64::try_from(descriptors.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&regions),
            i64::try_from(regions.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut valid),
        )
    };
    if status != 0 {
        return Err(status_error(status, 6, 0, 0, 0));
    }
    match valid {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Validate whether an artifact supplies complete, unambiguous model pricing.
pub fn provider_registry_pricing_is_authoritative(
    names: &[&str],
    input_present: &[bool],
    output_present: &[bool],
) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    if names.len() != input_present.len() || names.len() != output_present.len() {
        return Err(MojoError::InvalidInput);
    }
    let names = names.iter().map(|name| view(name)).collect::<Vec<_>>();
    let input_present = input_present
        .iter()
        .map(|present| i64::from(*present))
        .collect::<Vec<_>>();
    let output_present = output_present
        .iter()
        .map(|present| i64::from(*present))
        .collect::<Vec<_>>();
    let mut authoritative = -1_i64;
    let status = unsafe {
        prodex_mojo_provider_registry_pricing_authority_v1(
            RICH_ABI_VERSION,
            address(&names),
            address(&input_present),
            address(&output_present),
            i64::try_from(names.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut authoritative),
        )
    };
    if status != 0 {
        return Err(status_error(status, 6, 0, 0, 0));
    }
    match authoritative {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn address<T>(values: &[T]) -> u64 {
    if values.is_empty() {
        0
    } else {
        mojo_pointer_address(values.as_ptr())
    }
}

/// Normalize static provider model prices while keeping catalog strings in Rust.
pub fn plan_provider_registry_model_costs(
    names: &[&str],
    input_costs: &[Option<u64>],
    output_costs: &[Option<u64>],
) -> Result<ProviderRegistryModelCostPlan, MojoError> {
    ensure_rich_abi()?;
    if names.len() != input_costs.len()
        || names.len() != output_costs.len()
        || names.len() > MAX_PROVIDER_REGISTRY_MODEL_NAMES
        || names
            .iter()
            .any(|name| name.is_empty() || name.len() > MAX_PROVIDER_REGISTRY_MODEL_NAME_BYTES)
    {
        return Err(MojoError::InvalidInput);
    }

    let name_views = names
        .iter()
        .map(|name| view(name))
        .collect::<Vec<RichStringView>>();
    let input_values = input_costs
        .iter()
        .map(|cost| cost.unwrap_or_default())
        .collect::<Vec<_>>();
    let input_present = input_costs
        .iter()
        .map(|cost| i64::from(cost.is_some()))
        .collect::<Vec<_>>();
    let output_values = output_costs
        .iter()
        .map(|cost| cost.unwrap_or_default())
        .collect::<Vec<_>>();
    let output_present = output_costs
        .iter()
        .map(|cost| i64::from(cost.is_some()))
        .collect::<Vec<_>>();
    let mut accepted_indices = vec![-1_i64; names.len().max(1)];
    let mut normalized_input_costs = vec![0_u64; names.len().max(1)];
    let mut normalized_output_costs = vec![0_u64; names.len().max(1)];
    let mut output_count = 0_i64;
    let mut fallback_input_cost = 0_u64;
    let mut fallback_output_cost = 0_u64;
    let mut pricing_present = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_catalog_provider_registry_costs_v1(
            RICH_ABI_VERSION,
            address(&name_views),
            address(&input_values),
            address(&input_present),
            address(&output_values),
            address(&output_present),
            i64::try_from(names.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(accepted_indices.as_mut_ptr()),
            mojo_mut_pointer_address(normalized_input_costs.as_mut_ptr()),
            mojo_mut_pointer_address(normalized_output_costs.as_mut_ptr()),
            i64::try_from(names.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output_count),
            mojo_mut_pointer_address(&mut fallback_input_cost),
            mojo_mut_pointer_address(&mut fallback_output_cost),
            mojo_mut_pointer_address(&mut pricing_present),
        )
    };
    if status != 0 {
        return Err(status_error(status, 6, 0, 0, 0));
    }

    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if output_count > names.len() {
        return Err(MojoError::InvalidOutput);
    }
    let pricing_known = match pricing_present {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut seen = vec![false; names.len()];
    let entries = (0..output_count)
        .map(|index| {
            let name_index =
                usize::try_from(accepted_indices[index]).map_err(|_| MojoError::InvalidOutput)?;
            if name_index >= names.len() || seen[name_index] {
                return Err(MojoError::InvalidOutput);
            }
            seen[name_index] = true;
            Ok(ProviderRegistryModelCostEntry {
                name_index,
                input_cost_per_million_microusd: normalized_input_costs[index],
                output_cost_per_million_microusd: normalized_output_costs[index],
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProviderRegistryModelCostPlan {
        entries,
        fallback_input_cost_per_million_microusd: fallback_input_cost,
        fallback_output_cost_per_million_microusd: fallback_output_cost,
        pricing_known,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_registry_cost_plan_deduplicates_aliases_and_fills_missing_rates() {
        let plan = plan_provider_registry_model_costs(
            &["alpha", "ALPHA", "beta", "*"],
            &[Some(20), None, None, Some(90)],
            &[None, Some(30), Some(40), None],
        )
        .unwrap();

        assert_eq!(plan.fallback_input_cost_per_million_microusd, 90);
        assert_eq!(plan.fallback_output_cost_per_million_microusd, 40);
        assert!(plan.pricing_known);
        assert_eq!(
            plan.entries,
            [
                ProviderRegistryModelCostEntry {
                    name_index: 0,
                    input_cost_per_million_microusd: 20,
                    output_cost_per_million_microusd: 40,
                },
                ProviderRegistryModelCostEntry {
                    name_index: 2,
                    input_cost_per_million_microusd: 90,
                    output_cost_per_million_microusd: 40,
                },
                ProviderRegistryModelCostEntry {
                    name_index: 3,
                    input_cost_per_million_microusd: 90,
                    output_cost_per_million_microusd: 40,
                },
            ]
        );
    }

    #[test]
    fn provider_registry_pricing_authority_rejects_partial_and_ambiguous_maps() {
        assert_eq!(
            provider_registry_pricing_is_authoritative(
                &["*", "gpt-test"],
                &[true, true],
                &[true, true]
            ),
            Ok(true)
        );
        assert_eq!(
            provider_registry_pricing_is_authoritative(
                &["*", "GPT-test", "gpt-TEST"],
                &[true, true, true],
                &[true, true, true]
            ),
            Ok(false)
        );
        assert_eq!(
            provider_registry_pricing_is_authoritative(&["*"], &[true], &[false]),
            Ok(false)
        );
    }

    #[test]
    fn provider_registry_artifact_structure_owns_bounded_uniqueness_and_scores() {
        let regions = vec!["*".to_string(), "ap-southeast-1".to_string()];
        let descriptor = ProviderRegistryDescriptorValidationInput {
            revision: 1,
            pricing_revision: 2,
            provider_code: 0,
            credential_valid: true,
            endpoint_codes: vec![0, 1],
            regions: &regions,
            cost: 100,
            latency: 200,
            risk: 300,
            priority: 400,
            model_cost_count: 1,
            pricing_authoritative: true,
        };
        assert_eq!(
            provider_registry_artifact_is_valid(2, 1, 2, &[descriptor]),
            Ok(true)
        );

        let duplicate_regions = vec!["*".to_string(), "*".to_string()];
        let invalid = ProviderRegistryDescriptorValidationInput {
            regions: &duplicate_regions,
            revision: 1,
            pricing_revision: 2,
            provider_code: 0,
            credential_valid: true,
            endpoint_codes: vec![0, 1],
            cost: 100,
            latency: 200,
            risk: 300,
            priority: 400,
            model_cost_count: 1,
            pricing_authoritative: true,
        };
        assert_eq!(
            provider_registry_artifact_is_valid(2, 1, 2, &[invalid]),
            Ok(false)
        );
    }
}
