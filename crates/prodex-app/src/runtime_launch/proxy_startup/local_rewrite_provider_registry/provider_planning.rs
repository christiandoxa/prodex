use super::RuntimeGatewayProviderModelCostArtifact;
use prodex_provider_core::{ProviderId, provider_model_catalog};
use std::collections::BTreeMap;

pub(super) struct RuntimeGatewayBuiltinModelCostPlan {
    pub(super) model_costs: BTreeMap<String, RuntimeGatewayProviderModelCostArtifact>,
    pub(super) pricing_known: bool,
}

#[cfg(feature = "mojo-core")]
fn runtime_gateway_builtin_model_cost_plan_impl(
    provider: ProviderId,
) -> RuntimeGatewayBuiltinModelCostPlan {
    let models = provider_model_catalog(provider);
    let mut names = Vec::new();
    let mut input_costs = Vec::new();
    let mut output_costs = Vec::new();
    for model in models {
        names.push(model.id);
        input_costs.push(model.input_cost_per_million_microusd);
        output_costs.push(model.output_cost_per_million_microusd);
        for alias in model.aliases {
            names.push(alias);
            input_costs.push(model.input_cost_per_million_microusd);
            output_costs.push(model.output_cost_per_million_microusd);
        }
    }
    let plan = prodex_mojo_core::rich::plan_provider_registry_model_costs(
        &names,
        &input_costs,
        &output_costs,
    )
    .expect("Mojo provider-registry cost plan returned invalid output");
    let mut model_costs = plan
        .entries
        .into_iter()
        .map(|entry| {
            (
                names[entry.name_index].to_string(),
                RuntimeGatewayProviderModelCostArtifact {
                    input_cost_per_million_microusd: Some(entry.input_cost_per_million_microusd),
                    output_cost_per_million_microusd: Some(entry.output_cost_per_million_microusd),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    model_costs.insert(
        "*".to_string(),
        RuntimeGatewayProviderModelCostArtifact {
            input_cost_per_million_microusd: Some(plan.fallback_input_cost_per_million_microusd),
            output_cost_per_million_microusd: Some(plan.fallback_output_cost_per_million_microusd),
        },
    );
    RuntimeGatewayBuiltinModelCostPlan {
        model_costs,
        pricing_known: plan.pricing_known,
    }
}

#[cfg(not(feature = "mojo-core"))]
fn runtime_gateway_builtin_model_cost_plan_impl(
    provider: ProviderId,
) -> RuntimeGatewayBuiltinModelCostPlan {
    let models = provider_model_catalog(provider);
    let fallback_input = models
        .iter()
        .filter_map(|model| model.input_cost_per_million_microusd)
        .max()
        .unwrap_or_default();
    let fallback_output = models
        .iter()
        .filter_map(|model| model.output_cost_per_million_microusd)
        .max()
        .unwrap_or_default();
    let mut model_costs = BTreeMap::new();
    for model in models {
        let cost = RuntimeGatewayProviderModelCostArtifact {
            input_cost_per_million_microusd: Some(
                model
                    .input_cost_per_million_microusd
                    .unwrap_or(fallback_input),
            ),
            output_cost_per_million_microusd: Some(
                model
                    .output_cost_per_million_microusd
                    .unwrap_or(fallback_output),
            ),
        };
        insert_case_insensitive_model_cost(&mut model_costs, model.id, cost);
        for alias in model.aliases {
            insert_case_insensitive_model_cost(&mut model_costs, alias, cost);
        }
    }
    model_costs.insert(
        "*".to_string(),
        RuntimeGatewayProviderModelCostArtifact {
            input_cost_per_million_microusd: Some(fallback_input),
            output_cost_per_million_microusd: Some(fallback_output),
        },
    );
    RuntimeGatewayBuiltinModelCostPlan {
        model_costs,
        pricing_known: models.iter().any(|model| {
            model.input_cost_per_million_microusd.is_some()
                || model.output_cost_per_million_microusd.is_some()
        }),
    }
}

#[cfg(not(feature = "mojo-core"))]
fn insert_case_insensitive_model_cost(
    model_costs: &mut BTreeMap<String, RuntimeGatewayProviderModelCostArtifact>,
    model: &str,
    cost: RuntimeGatewayProviderModelCostArtifact,
) {
    if !model_costs
        .keys()
        .any(|configured| configured.eq_ignore_ascii_case(model))
    {
        model_costs.insert(model.to_string(), cost);
    }
}

pub(super) fn runtime_gateway_builtin_model_cost_plan(
    provider: ProviderId,
) -> RuntimeGatewayBuiltinModelCostPlan {
    runtime_gateway_builtin_model_cost_plan_impl(provider)
}

#[cfg(all(test, feature = "mojo-core"))]
mod tests {
    use super::*;

    type RustModelCosts = BTreeMap<String, (Option<u64>, Option<u64>)>;

    fn rust_oracle(provider: ProviderId) -> (RustModelCosts, bool) {
        let models = provider_model_catalog(provider);
        let fallback_input = models
            .iter()
            .filter_map(|model| model.input_cost_per_million_microusd)
            .max()
            .unwrap_or_default();
        let fallback_output = models
            .iter()
            .filter_map(|model| model.output_cost_per_million_microusd)
            .max()
            .unwrap_or_default();
        let mut costs: BTreeMap<String, (Option<u64>, Option<u64>)> = BTreeMap::new();
        let mut insert = |name: &str, input: u64, output: u64| {
            if !costs
                .keys()
                .any(|configured| configured.eq_ignore_ascii_case(name))
            {
                costs.insert(name.to_string(), (Some(input), Some(output)));
            }
        };
        for model in models {
            let input = model
                .input_cost_per_million_microusd
                .unwrap_or(fallback_input);
            let output = model
                .output_cost_per_million_microusd
                .unwrap_or(fallback_output);
            insert(model.id, input, output);
            for alias in model.aliases {
                insert(alias, input, output);
            }
        }
        costs.insert(
            "*".to_string(),
            (Some(fallback_input), Some(fallback_output)),
        );
        (
            costs,
            models.iter().any(|model| {
                model.input_cost_per_million_microusd.is_some()
                    || model.output_cost_per_million_microusd.is_some()
            }),
        )
    }

    #[test]
    fn provider_registry_builtin_cost_plan_matches_rust_oracle_for_all_providers() {
        for &provider in prodex_provider_core::PROVIDER_IMPLEMENTATION_ORDER {
            let actual = runtime_gateway_builtin_model_cost_plan(provider);
            let actual_pricing_known = actual.pricing_known;
            let actual_costs = actual
                .model_costs
                .iter()
                .map(|(name, cost)| {
                    (
                        name.clone(),
                        (
                            cost.input_cost_per_million_microusd,
                            cost.output_cost_per_million_microusd,
                        ),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let (expected_costs, expected_pricing_known) = rust_oracle(provider);
            assert_eq!(
                actual_costs,
                expected_costs,
                "{} cost plan drift",
                provider.label()
            );
            assert_eq!(actual_pricing_known, expected_pricing_known);
        }
    }
}
