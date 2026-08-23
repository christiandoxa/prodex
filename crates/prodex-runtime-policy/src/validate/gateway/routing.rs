use super::validate_gateway_exact_identifier;
use crate::types::{
    RuntimePolicyFile, RuntimePolicyGatewayRouteAlias, RuntimePolicyGatewayRouteModelMetrics,
};
use crate::validate_helpers::{NumericRule, failed_numeric_rules, validate_gateway_route_strategy};
use crate::validate_request_constraints::validate_gateway_request_constraints;
use anyhow::{Context, Result, bail};
use std::path::Path;

pub(super) fn validate_gateway_routing(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let numeric = gateway_routing_numeric_failures(policy)?;
    validate_gateway_adaptive_numeric(numeric.adaptive, path)?;
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
    validate_gateway_request_constraints(policy, path, numeric.safe_window)?;
    for ((index, alias), metric_failures) in policy
        .gateway
        .route_aliases
        .iter()
        .enumerate()
        .zip(&numeric.route_metrics)
    {
        validate_gateway_route_alias(alias, index, metric_failures, path)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum AdaptiveNumericTag {
    WindowNonZero,
    MinimumNonZero,
    WindowMaximum,
    MinimumRelation,
}

#[derive(Clone, Copy)]
enum RoutingNumericLocation {
    Adaptive(AdaptiveNumericTag),
    SafeWindow,
    RouteMetric {
        alias: usize,
        metric: usize,
        name: &'static str,
    },
}

struct RoutingNumericFailures {
    adaptive: Option<AdaptiveNumericTag>,
    safe_window: bool,
    route_metrics: Vec<Vec<Option<&'static str>>>,
}

fn append_route_metric_numeric_rules(
    alias: usize,
    metric: usize,
    values: &RuntimePolicyGatewayRouteModelMetrics,
    rules: &mut Vec<NumericRule>,
    locations: &mut Vec<RoutingNumericLocation>,
) {
    for (name, value) in [
        (
            "input_cost_per_million_microusd",
            values.input_cost_per_million_microusd,
        ),
        (
            "output_cost_per_million_microusd",
            values.output_cost_per_million_microusd,
        ),
        ("latency_ms", values.latency_ms),
        ("rpm_limit", values.rpm_limit),
        ("tpm_limit", values.tpm_limit),
    ] {
        let Some(value) = value else { continue };
        rules.push(NumericRule::NonZero(value));
        locations.push(RoutingNumericLocation::RouteMetric {
            alias,
            metric,
            name,
        });
    }
}

fn gateway_routing_numeric_failures(policy: &RuntimePolicyFile) -> Result<RoutingNumericFailures> {
    let adaptive = &policy.gateway.adaptive_routing;
    let window_size = adaptive.window_size.unwrap_or(128) as u64;
    let min_samples = adaptive.min_samples.unwrap_or(8);
    let mut rules = Vec::new();
    let mut locations = Vec::new();

    if let Some(value) = adaptive.window_size {
        rules.push(NumericRule::NonZero(value as u64));
        locations.push(RoutingNumericLocation::Adaptive(
            AdaptiveNumericTag::WindowNonZero,
        ));
    }
    if let Some(value) = adaptive.min_samples {
        rules.push(NumericRule::NonZero(value));
        locations.push(RoutingNumericLocation::Adaptive(
            AdaptiveNumericTag::MinimumNonZero,
        ));
    }
    if adaptive.window_size.is_some() {
        rules.push(NumericRule::Range {
            value: window_size,
            minimum: 1,
            maximum: 4_096,
        });
        locations.push(RoutingNumericLocation::Adaptive(
            AdaptiveNumericTag::WindowMaximum,
        ));
    }
    rules.push(NumericRule::LessOrEqual {
        value: min_samples,
        maximum: window_size,
    });
    locations.push(RoutingNumericLocation::Adaptive(
        AdaptiveNumericTag::MinimumRelation,
    ));

    if let Some(value) = policy.gateway.request_constraints.safe_window_tokens {
        rules.push(NumericRule::NonZero(value));
        locations.push(RoutingNumericLocation::SafeWindow);
    }
    for (alias_index, alias) in policy.gateway.route_aliases.iter().enumerate() {
        for (metric_index, metric) in alias.model_metrics.iter().enumerate() {
            append_route_metric_numeric_rules(
                alias_index,
                metric_index,
                metric,
                &mut rules,
                &mut locations,
            );
        }
    }

    let mut failures = RoutingNumericFailures {
        adaptive: None,
        safe_window: false,
        route_metrics: policy
            .gateway
            .route_aliases
            .iter()
            .map(|alias| vec![None; alias.model_metrics.len()])
            .collect(),
    };
    for index in failed_numeric_rules(&rules)? {
        match locations[index] {
            RoutingNumericLocation::Adaptive(tag) => {
                failures.adaptive.get_or_insert(tag);
            }
            RoutingNumericLocation::SafeWindow => failures.safe_window = true,
            RoutingNumericLocation::RouteMetric {
                alias,
                metric,
                name,
            } => {
                failures.route_metrics[alias][metric].get_or_insert(name);
            }
        }
    }
    Ok(failures)
}

fn validate_gateway_adaptive_numeric(
    failure: Option<AdaptiveNumericTag>,
    path: &Path,
) -> Result<()> {
    match failure {
        Some(AdaptiveNumericTag::WindowNonZero) => bail!(
            "gateway.adaptive_routing.window_size in {} must be greater than 0",
            path.display()
        ),
        Some(AdaptiveNumericTag::MinimumNonZero) => bail!(
            "gateway.adaptive_routing.min_samples in {} must be greater than 0",
            path.display()
        ),
        Some(AdaptiveNumericTag::WindowMaximum) => bail!(
            "gateway.adaptive_routing.window_size in {} must be at most 4096",
            path.display()
        ),
        Some(AdaptiveNumericTag::MinimumRelation) => bail!(
            "gateway.adaptive_routing.min_samples in {} must not exceed gateway.adaptive_routing.window_size",
            path.display()
        ),
        None => Ok(()),
    }
}

fn validate_gateway_route_alias(
    alias: &RuntimePolicyGatewayRouteAlias,
    index: usize,
    numeric_failures: &[Option<&'static str>],
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
        if let Some(name) = numeric_failures[metric_index] {
            bail!(
                "{metric_field}.{name} in {} must be greater than 0",
                path.display()
            );
        }
    }
    Ok(())
}
