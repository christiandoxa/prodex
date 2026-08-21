use super::validate_gateway_exact_identifier;
use crate::types::{RuntimePolicyFile, RuntimePolicyGatewayRouteAlias};
use crate::validate_helpers::validate_gateway_route_strategy;
#[cfg(any(not(feature = "mojo"), test))]
use crate::validate_helpers::{validate_optional_u64, validate_optional_usize};
use crate::validate_request_constraints::validate_gateway_request_constraints;
use anyhow::{Context, Result, bail};
use std::path::Path;

pub(super) fn validate_gateway_routing(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    validate_gateway_adaptive_numeric(policy, path)?;
    if let Some(rate) = policy.gateway.adaptive_routing.exploration_rate
        && !(0.0..=1.0).contains(&rate)
    {
        bail!(
            "gateway.adaptive_routing.exploration_rate in {} must be between 0.0 and 1.0",
            path.display()
        );
    }
    if policy.gateway.adaptive_routing.enabled == Some(true)
        && policy.gateway.route_aliases.is_empty()
    {
        bail!(
            "gateway.adaptive_routing.enabled in {} requires at least one gateway.route_aliases entry",
            path.display()
        );
    }
    validate_gateway_request_constraints(policy, path)?;
    for (index, alias) in policy.gateway.route_aliases.iter().enumerate() {
        validate_gateway_route_alias(alias, index, path)?;
    }
    Ok(())
}

#[cfg(any(not(feature = "mojo"), test))]
fn validate_gateway_adaptive_numeric_rust(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let adaptive = &policy.gateway.adaptive_routing;
    validate_optional_usize(
        adaptive.window_size,
        path,
        "gateway.adaptive_routing.window_size",
    )?;
    validate_optional_u64(
        adaptive.min_samples,
        path,
        "gateway.adaptive_routing.min_samples",
    )?;
    let window_size = adaptive.window_size.unwrap_or(128);
    let min_samples = adaptive.min_samples.unwrap_or(8);
    if window_size > 4_096 {
        bail!(
            "gateway.adaptive_routing.window_size in {} must be at most 4096",
            path.display()
        );
    }
    if min_samples > window_size as u64 {
        bail!(
            "gateway.adaptive_routing.min_samples in {} must not exceed gateway.adaptive_routing.window_size",
            path.display()
        );
    }
    Ok(())
}

#[cfg(feature = "mojo")]
#[derive(Debug, Clone, Copy)]
enum AdaptiveNumericTag {
    WindowNonZero,
    MinimumNonZero,
    WindowMaximum,
    MinimumRelation,
}

#[cfg(feature = "mojo")]
fn validate_gateway_adaptive_numeric_mojo(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let adaptive = &policy.gateway.adaptive_routing;
    let window_size = adaptive.window_size.unwrap_or(128);
    let min_samples = adaptive.min_samples.unwrap_or(8);
    let mut rules = Vec::new();
    let mut tags = Vec::new();

    if let Some(value) = adaptive.window_size {
        rules.push(prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_NON_ZERO,
            value: value as u64,
            minimum: 0,
            maximum: u64::MAX,
            related_value: 0,
        });
        tags.push(AdaptiveNumericTag::WindowNonZero);
    }
    if let Some(value) = adaptive.min_samples {
        rules.push(prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_NON_ZERO,
            value,
            minimum: 0,
            maximum: u64::MAX,
            related_value: 0,
        });
        tags.push(AdaptiveNumericTag::MinimumNonZero);
    }
    if adaptive.window_size.is_some() {
        rules.push(prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_RANGE,
            value: window_size as u64,
            minimum: 1,
            maximum: 4_096,
            related_value: 0,
        });
        tags.push(AdaptiveNumericTag::WindowMaximum);
    }
    rules.push(prodex_mojo_core::policy::NumericRule {
        kind: prodex_mojo_core::policy::POLICY_NUMERIC_RELATION_LE,
        value: min_samples,
        minimum: 0,
        maximum: 0,
        related_value: window_size as u64,
    });
    tags.push(AdaptiveNumericTag::MinimumRelation);

    let failed = prodex_mojo_core::policy::validate_numeric_rules(&rules).map_err(|_| {
        anyhow::anyhow!("gateway adaptive numeric validation returned invalid output")
    })?;
    if let Some(index) = failed.first() {
        match tags[*index] {
            AdaptiveNumericTag::WindowNonZero => bail!(
                "gateway.adaptive_routing.window_size in {} must be greater than 0",
                path.display()
            ),
            AdaptiveNumericTag::MinimumNonZero => bail!(
                "gateway.adaptive_routing.min_samples in {} must be greater than 0",
                path.display()
            ),
            AdaptiveNumericTag::WindowMaximum => bail!(
                "gateway.adaptive_routing.window_size in {} must be at most 4096",
                path.display()
            ),
            AdaptiveNumericTag::MinimumRelation => bail!(
                "gateway.adaptive_routing.min_samples in {} must not exceed gateway.adaptive_routing.window_size",
                path.display()
            ),
        }
    }
    Ok(())
}

fn validate_gateway_adaptive_numeric(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    #[cfg(feature = "mojo")]
    {
        validate_gateway_adaptive_numeric_mojo(policy, path)
    }
    #[cfg(not(feature = "mojo"))]
    {
        validate_gateway_adaptive_numeric_rust(policy, path)
    }
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_tests {
    use super::*;

    #[test]
    fn mojo_gateway_adaptive_numeric_validation_matches_rust_oracle() {
        for input in [
            "version = 1",
            "version = 1\n[gateway.adaptive_routing]\nwindow_size = 0",
            "version = 1\n[gateway.adaptive_routing]\nmin_samples = 0",
            "version = 1\n[gateway.adaptive_routing]\nwindow_size = 4097",
            "version = 1\n[gateway.adaptive_routing]\nwindow_size = 4\nmin_samples = 5",
            "version = 1\n[gateway.adaptive_routing]\nwindow_size = 64\nmin_samples = 8",
        ] {
            let policy = toml::from_str::<RuntimePolicyFile>(input).unwrap();
            let path = Path::new("policy.toml");
            assert_eq!(
                validate_gateway_adaptive_numeric_rust(&policy, path)
                    .map_err(|error| error.to_string()),
                validate_gateway_adaptive_numeric_mojo(&policy, path)
                    .map_err(|error| error.to_string()),
                "{input}"
            );
        }
    }
}

fn validate_gateway_route_alias(
    alias: &RuntimePolicyGatewayRouteAlias,
    index: usize,
    path: &Path,
) -> Result<()> {
    let field = format!("gateway.route_aliases[{index}]");
    validate_gateway_exact_identifier(&alias.alias, path, &format!("{field}.alias"))?;
    if alias.models.is_empty() {
        bail!("{field}.models in {} cannot be empty", path.display());
    }
    for (model_index, model) in alias.models.iter().enumerate() {
        validate_gateway_exact_identifier(model, path, &format!("{field}.models[{model_index}]"))?;
    }
    if let Some(strategy) = alias.strategy.as_deref() {
        validate_gateway_route_strategy(strategy)
            .with_context(|| format!("{field}.strategy in {} is invalid", path.display()))?;
    }
    for (metric_index, metric) in alias.model_metrics.iter().enumerate() {
        let metric_field = format!("{field}.model_metrics[{metric_index}]");
        validate_gateway_exact_identifier(&metric.model, path, &format!("{metric_field}.model"))?;
        if !alias.models.iter().any(|model| model == &metric.model) {
            bail!(
                "{metric_field}.model in {} must match one of {field}.models",
                path.display()
            );
        }
        validate_gateway_route_metric_numbers(
            [
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
            ],
            &metric_field,
            path,
        )?;
    }
    Ok(())
}

fn validate_gateway_route_metric_numbers(
    values: [(&'static str, Option<u64>); 5],
    field: &str,
    path: &Path,
) -> Result<()> {
    #[cfg(feature = "mojo")]
    {
        let default_rule = prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_NON_ZERO,
            value: 0,
            minimum: 0,
            maximum: u64::MAX,
            related_value: 0,
        };
        let mut rules = [default_rule; 5];
        let mut names = [""; 5];
        let mut count = 0;
        for (name, value) in values {
            if let Some(value) = value {
                rules[count].value = value;
                names[count] = name;
                count += 1;
            }
        }
        let failed =
            prodex_mojo_core::policy::validate_numeric_rules(&rules[..count]).map_err(|_| {
                anyhow::anyhow!("gateway route metric numeric validation returned invalid output")
            })?;
        if let Some(index) = failed.first() {
            bail!(
                "{field}.{} in {} must be greater than 0",
                names[*index],
                path.display()
            );
        }
        Ok(())
    }
    #[cfg(not(feature = "mojo"))]
    {
        for (name, value) in values {
            if matches!(value, Some(0)) {
                bail!(
                    "{field}.{name} in {} must be greater than 0",
                    path.display()
                );
            }
        }
        Ok(())
    }
}
