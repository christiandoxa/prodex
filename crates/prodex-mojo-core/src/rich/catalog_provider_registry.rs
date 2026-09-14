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

unsafe extern "C" {
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
}
