use super::*;
use std::collections::BTreeMap;

pub(crate) fn gateway_route_aliases_config(
    policy: &prodex_runtime_policy::RuntimePolicyGatewaySettings,
    provider: Option<SuperExternalProvider>,
) -> Vec<runtime_proxy_crate::RuntimeGatewayRouteAlias> {
    let provider_id = gateway_provider_catalog_id(provider);
    policy
        .route_aliases
        .iter()
        .map(|alias| {
            let strategy = alias
                .strategy
                .as_deref()
                .and_then(runtime_proxy_crate::RuntimeGatewayRouteStrategy::parse)
                .unwrap_or_default();
            let models = alias
                .models
                .iter()
                .map(|model| model.trim().to_string())
                .filter(|model| !model.is_empty())
                .collect::<Vec<_>>();
            runtime_proxy_crate::RuntimeGatewayRouteAlias {
                alias: alias.alias.trim().to_string(),
                model_metrics: gateway_route_alias_model_metrics(
                    provider_id,
                    &models,
                    &alias.model_metrics,
                ),
                models,
                strategy,
            }
        })
        .collect::<Vec<_>>()
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
) -> BTreeMap<String, runtime_proxy_crate::RuntimeGatewayRouteModelMetrics> {
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
        let model = metric.model.trim().to_string();
        if model.is_empty() {
            continue;
        }
        let entry = metrics.entry(model).or_default();
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
    metrics
}
