use super::*;
use std::collections::BTreeMap;

pub(crate) fn gateway_route_aliases_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    provider: Option<SuperExternalProvider>,
) -> Result<Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias>> {
    let provider_id = gateway_provider_catalog_id(provider);
    policy
        .route_aliases
        .iter()
        .map(|alias| -> Result<_> {
            if !gateway_exact_policy_identifier(&alias.alias) {
                bail!(
                    "gateway.route_aliases alias {:?} must be non-empty without whitespace",
                    alias.alias
                );
            }
            let strategy = match alias.strategy.as_deref() {
                Some(value) => runtime_proxy_crate::RuntimeGatewayRouteStrategy::parse(value)
                    .with_context(|| {
                        format!("gateway route alias '{}' strategy is invalid", alias.alias)
                    })?,
                None => Default::default(),
            };
            if alias.models.is_empty() {
                bail!("gateway.route_aliases {:?} models cannot be empty", alias.alias);
            }
            for model in &alias.models {
                if !gateway_exact_policy_identifier(model) {
                    bail!(
                        "gateway.route_aliases {:?} models must be non-empty strings without whitespace",
                        alias.alias
                    );
                }
            }
            let models = alias.models.clone();
            Ok(runtime_proxy_crate::RuntimeGatewayRouteAlias {
                alias: alias.alias.clone(),
                model_metrics: gateway_route_alias_model_metrics(
                    provider_id,
                    &models,
                    &alias.model_metrics,
                )?,
                models,
                strategy,
            })
        })
        .collect()
}

fn gateway_provider_catalog_id(
    provider: Option<SuperExternalProvider>,
) -> Option<prodex_provider_core::ProviderId> {
    match provider {
        Some(SuperExternalProvider::Anthropic) => Some(prodex_provider_core::ProviderId::Anthropic),
        Some(SuperExternalProvider::Copilot) => Some(prodex_provider_core::ProviderId::Copilot),
        Some(SuperExternalProvider::DeepSeek) => Some(prodex_provider_core::ProviderId::DeepSeek),
        Some(SuperExternalProvider::Gemini) => Some(prodex_provider_core::ProviderId::Gemini),
        Some(SuperExternalProvider::Kiro) => Some(prodex_provider_core::ProviderId::Kiro),
        None => Some(prodex_provider_core::ProviderId::OpenAi),
    }
}

pub(crate) fn gateway_route_alias_model_metrics(
    provider: Option<prodex_provider_core::ProviderId>,
    models: &[String],
    configured: &[prodex_runtime_policy::RuntimePolicyGatewayRouteModelMetrics],
) -> Result<BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelMetrics>> {
    let mut metrics = BTreeMap::new();
    for model in models {
        if let Some(provider) = provider {
            let cost = prodex_provider_core::provider_model_cost(provider, model);
            if cost.any() {
                metrics.insert(
                    model.clone(),
                    runtime_proxy_crate::RuntimeGatewayRouteModelMetrics {
                        input_cost_per_million_microusd: cost.input_cost_per_million_microusd,
                        output_cost_per_million_microusd: cost.output_cost_per_million_microusd,
                        ..runtime_proxy_crate::RuntimeGatewayRouteModelMetrics::default()
                    },
                );
            }
        }
    }
    for metric in configured {
        if !gateway_exact_policy_identifier(&metric.model) {
            bail!(
                "gateway.route_aliases model_metrics model {:?} must be non-empty without whitespace",
                metric.model
            );
        }
        if !models.iter().any(|model| model == &metric.model) {
            bail!(
                "gateway.route_aliases model_metrics model {:?} must match a configured model",
                metric.model
            );
        }
        let entry = metrics.entry(metric.model.clone()).or_default();
        if metric.input_cost_per_million_microusd.is_some() {
            entry.input_cost_per_million_microusd = metric.input_cost_per_million_microusd;
        }
        if metric.output_cost_per_million_microusd.is_some() {
            entry.output_cost_per_million_microusd = metric.output_cost_per_million_microusd;
        }
        if metric.latency_ms.is_some() {
            entry.latency_ms = metric.latency_ms;
        }
        if metric.rpm_limit.is_some() {
            entry.rpm_limit = metric.rpm_limit;
        }
        if metric.tpm_limit.is_some() {
            entry.tpm_limit = metric.tpm_limit;
        }
    }
    Ok(metrics)
}

fn gateway_exact_policy_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}
