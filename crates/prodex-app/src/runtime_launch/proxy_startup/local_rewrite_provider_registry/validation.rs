use super::RuntimeGatewayProviderModelCostArtifact;
use std::collections::BTreeMap;

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
