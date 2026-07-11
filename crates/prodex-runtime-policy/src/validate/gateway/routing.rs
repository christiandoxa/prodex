use super::validate_gateway_exact_identifier;
use crate::types::RuntimePolicyFile;
use crate::validate_helpers::{
    validate_gateway_route_strategy, validate_optional_u64, validate_optional_usize,
};
use crate::validate_request_constraints::validate_gateway_request_constraints;
use anyhow::{Context, Result, bail};
use std::path::Path;

pub(super) fn validate_gateway_routing(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    validate_optional_usize(
        policy.gateway.adaptive_routing.window_size,
        path,
        "gateway.adaptive_routing.window_size",
    )?;
    validate_optional_u64(
        policy.gateway.adaptive_routing.min_samples,
        path,
        "gateway.adaptive_routing.min_samples",
    )?;
    if let Some(rate) = policy.gateway.adaptive_routing.exploration_rate
        && !(0.0..=1.0).contains(&rate)
    {
        bail!(
            "gateway.adaptive_routing.exploration_rate in {} must be between 0.0 and 1.0",
            path.display()
        );
    }
    validate_gateway_request_constraints(policy, path)?;
    for (index, alias) in policy.gateway.route_aliases.iter().enumerate() {
        let field = format!("gateway.route_aliases[{index}]");
        validate_gateway_exact_identifier(&alias.alias, path, &format!("{field}.alias"))?;
        if alias.models.is_empty() {
            bail!("{field}.models in {} cannot be empty", path.display());
        }
        for (model_index, model) in alias.models.iter().enumerate() {
            validate_gateway_exact_identifier(
                model,
                path,
                &format!("{field}.models[{model_index}]"),
            )?;
        }
        if let Some(strategy) = alias.strategy.as_deref() {
            validate_gateway_route_strategy(strategy)
                .with_context(|| format!("{field}.strategy in {} is invalid", path.display()))?;
        }
        for (metric_index, metric) in alias.model_metrics.iter().enumerate() {
            let metric_field = format!("{field}.model_metrics[{metric_index}]");
            validate_gateway_exact_identifier(
                &metric.model,
                path,
                &format!("{metric_field}.model"),
            )?;
            if !alias.models.iter().any(|model| model == &metric.model) {
                bail!(
                    "{metric_field}.model in {} must match one of {field}.models",
                    path.display()
                );
            }
            for (name, value) in [
                (
                    "input_cost_per_million_microusd",
                    metric.input_cost_per_million_microusd,
                ),
                (
                    "output_cost_per_million_microusd",
                    metric.output_cost_per_million_microusd,
                ),
                ("latency_ms", metric.latency_ms),
                ("rpm_limit", metric.rpm_limit),
                ("tpm_limit", metric.tpm_limit),
            ] {
                validate_optional_u64(value, path, &format!("{metric_field}.{name}"))?;
            }
        }
    }
    Ok(())
}
