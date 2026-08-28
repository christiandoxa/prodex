use super::{
    RuntimeGatewayRouteAlias, RuntimeGatewayRouteModelState, RuntimeGatewayRouteRewrite,
    RuntimeGatewayRouteStrategy,
};
use crate::runtime_gateway_estimated_tokens;
use std::collections::BTreeMap;

pub fn runtime_gateway_rewrite_route_alias(
    body: &[u8],
    aliases: &[RuntimeGatewayRouteAlias],
    request_id: u64,
) -> Option<RuntimeGatewayRouteRewrite> {
    runtime_gateway_rewrite_route_alias_with_state(body, aliases, request_id, &BTreeMap::new())
}

pub fn runtime_gateway_rewrite_route_alias_with_state(
    body: &[u8],
    aliases: &[RuntimeGatewayRouteAlias],
    request_id: u64,
    model_state: &BTreeMap<String, RuntimeGatewayRouteModelState>,
) -> Option<RuntimeGatewayRouteRewrite> {
    if aliases.is_empty() {
        return None;
    }
    let mut value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    let object = value.as_object_mut()?;
    let requested_model = object
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())?;
    let alias = aliases
        .iter()
        .find(|alias| alias.alias == requested_model && !alias.models.is_empty())?;
    let estimated_tokens = runtime_gateway_estimated_tokens(body);
    let model =
        runtime_gateway_route_selected_model(alias, request_id, model_state, estimated_tokens)?;
    object.insert(
        "model".to_string(),
        serde_json::Value::String(model.clone()),
    );
    let body = serde_json::to_vec(&value).ok()?;
    Some(RuntimeGatewayRouteRewrite {
        alias: alias.alias.clone(),
        strategy: alias.strategy,
        model,
        body,
    })
}

fn runtime_gateway_route_selected_model(
    alias: &RuntimeGatewayRouteAlias,
    request_id: u64,
    model_state: &BTreeMap<String, RuntimeGatewayRouteModelState>,
    estimated_tokens: u64,
) -> Option<String> {
    let models = alias
        .models
        .iter()
        .map(|model| model.trim())
        .filter(|model| !model.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    runtime_gateway_route_selected_model_from_models(
        alias,
        &models,
        request_id,
        model_state,
        estimated_tokens,
    )
}

#[cfg(feature = "mojo")]
pub(crate) fn runtime_gateway_route_selected_model_from_models(
    alias: &RuntimeGatewayRouteAlias,
    models: &[String],
    request_id: u64,
    model_state: &BTreeMap<String, RuntimeGatewayRouteModelState>,
    estimated_tokens: u64,
) -> Option<String> {
    if models.len() > 256 {
        return None;
    }
    let inputs = models
        .iter()
        .map(|model| {
            let metrics = alias.model_metrics.get(model);
            let state = model_state.get(model);
            prodex_mojo_core::rich::PolicyRouteModel {
                model,
                input_cost: metrics.and_then(|metrics| metrics.input_cost_per_million_microusd),
                output_cost: metrics.and_then(|metrics| metrics.output_cost_per_million_microusd),
                policy_latency: metrics.and_then(|metrics| metrics.latency_ms),
                state_latency: state.and_then(|state| state.latency_ms_ewma),
                in_flight: state
                    .map(|state| u64::try_from(state.in_flight).unwrap_or(u64::MAX))
                    .unwrap_or_default(),
                rpm_limit: metrics.and_then(|metrics| metrics.rpm_limit),
                rpm_used: state
                    .map(|state| state.requests_this_minute)
                    .unwrap_or_default(),
                tpm_limit: metrics.and_then(|metrics| metrics.tpm_limit),
                tpm_used: state
                    .map(|state| state.tokens_this_minute)
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::rich::plan_route_policy(
        alias.strategy.as_str(),
        request_id,
        estimated_tokens,
        &inputs,
    )
    .expect("Mojo route policy planning returned an invalid structured result");
    if alias.strategy == RuntimeGatewayRouteStrategy::Fallback {
        return (!plan.ordered_indices.is_empty()).then(|| {
            format!(
                "combo:{}",
                plan.ordered_indices
                    .iter()
                    .filter_map(|index| models.get(*index))
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        });
    }
    plan.selected_index
        .and_then(|index| models.get(index))
        .cloned()
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn runtime_gateway_route_selected_model_from_models_rust(
    alias: &RuntimeGatewayRouteAlias,
    models: &[String],
    request_id: u64,
    model_state: &BTreeMap<String, RuntimeGatewayRouteModelState>,
    estimated_tokens: u64,
) -> Option<String> {
    let models = models.iter().map(String::as_str).collect::<Vec<_>>();
    if models.is_empty() {
        return None;
    }
    Some(match alias.strategy {
        RuntimeGatewayRouteStrategy::Fallback => format!("combo:{}", models.join(",")),
        RuntimeGatewayRouteStrategy::RoundRobin => {
            let index = (request_id as usize).saturating_sub(1) % models.len();
            models[index].to_string()
        }
        RuntimeGatewayRouteStrategy::First => models[0].to_string(),
        RuntimeGatewayRouteStrategy::LeastBusy => models
            .iter()
            .min_by_key(|&model| {
                model_state
                    .get::<str>(model)
                    .map(|state| state.in_flight)
                    .unwrap_or_default()
            })
            .copied()
            .unwrap_or(models[0])
            .to_string(),
        RuntimeGatewayRouteStrategy::LowestCost => models
            .iter()
            .min_by_key(|&model| {
                alias
                    .model_metrics
                    .get::<str>(model)
                    .map(|metrics| {
                        metrics
                            .input_cost_per_million_microusd
                            .unwrap_or_default()
                            .saturating_add(
                                metrics.output_cost_per_million_microusd.unwrap_or_default(),
                            )
                    })
                    .unwrap_or(u64::MAX)
            })
            .copied()
            .unwrap_or(models[0])
            .to_string(),
        RuntimeGatewayRouteStrategy::LowestLatency => models
            .iter()
            .min_by_key(|&model| {
                let state_latency = model_state
                    .get::<str>(model)
                    .and_then(|state| state.latency_ms_ewma);
                let policy_latency = alias
                    .model_metrics
                    .get::<str>(model)
                    .and_then(|metrics| metrics.latency_ms);
                state_latency.or(policy_latency).unwrap_or(u64::MAX)
            })
            .copied()
            .unwrap_or(models[0])
            .to_string(),
        RuntimeGatewayRouteStrategy::Rpm => models
            .iter()
            .max_by_key(|&model| {
                let limit = alias
                    .model_metrics
                    .get::<str>(model)
                    .and_then(|metrics| metrics.rpm_limit)
                    .unwrap_or(u64::MAX / 2);
                let used = model_state
                    .get::<str>(model)
                    .map(|state| state.requests_this_minute)
                    .unwrap_or_default();
                limit.saturating_sub(used)
            })
            .copied()
            .unwrap_or(models[0])
            .to_string(),
        RuntimeGatewayRouteStrategy::Tpm => models
            .iter()
            .max_by_key(|&model| {
                let limit = alias
                    .model_metrics
                    .get::<str>(model)
                    .and_then(|metrics| metrics.tpm_limit)
                    .unwrap_or(u64::MAX / 2);
                let used = model_state
                    .get::<str>(model)
                    .map(|state| state.tokens_this_minute)
                    .unwrap_or_default()
                    .saturating_add(estimated_tokens);
                limit.saturating_sub(used)
            })
            .copied()
            .unwrap_or(models[0])
            .to_string(),
    })
}

#[cfg(not(feature = "mojo"))]
pub(crate) fn runtime_gateway_route_selected_model_from_models(
    alias: &RuntimeGatewayRouteAlias,
    models: &[String],
    request_id: u64,
    model_state: &BTreeMap<String, RuntimeGatewayRouteModelState>,
    estimated_tokens: u64,
) -> Option<String> {
    runtime_gateway_route_selected_model_from_models_rust(
        alias,
        models,
        request_id,
        model_state,
        estimated_tokens,
    )
}
