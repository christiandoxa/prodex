//! Route alias strategy and rewrite helpers for runtime gateway policy.

use std::collections::BTreeMap;

use super::runtime_gateway_estimated_tokens;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGatewayRouteAlias {
    pub alias: String,
    pub models: Vec<String>,
    pub strategy: RuntimeGatewayRouteStrategy,
    pub model_metrics: BTreeMap<String, RuntimeGatewayRouteModelMetrics>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimeGatewayRouteStrategy {
    #[default]
    Fallback,
    RoundRobin,
    First,
    LeastBusy,
    LowestCost,
    LowestLatency,
    Rpm,
    Tpm,
}

impl RuntimeGatewayRouteStrategy {
    pub const VALID_VALUES: &'static [&'static str] = &[
        "fallback",
        "round-robin",
        "first",
        "least-busy",
        "lowest-cost",
        "lowest-latency",
        "rpm",
        "tpm",
    ];

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "fallback" | "ordered-fallback" | "ordered_fallback" => Some(Self::Fallback),
            "round-robin" | "round_robin" | "rr" => Some(Self::RoundRobin),
            "first" | "first-available" | "first_available" | "ordered" => Some(Self::First),
            "least-busy" | "least_busy" | "least-busy-model" | "least_busy_model" => {
                Some(Self::LeastBusy)
            }
            "lowest-cost" | "lowest_cost" | "cost" | "cost-optimized" | "cost_optimized" => {
                Some(Self::LowestCost)
            }
            "lowest-latency" | "lowest_latency" | "latency" | "latency-optimized"
            | "latency_optimized" => Some(Self::LowestLatency),
            "rpm" | "rpm-headroom" | "rpm_headroom" => Some(Self::Rpm),
            "tpm" | "tpm-headroom" | "tpm_headroom" => Some(Self::Tpm),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fallback => "fallback",
            Self::RoundRobin => "round-robin",
            Self::First => "first",
            Self::LeastBusy => "least-busy",
            Self::LowestCost => "lowest-cost",
            Self::LowestLatency => "lowest-latency",
            Self::Rpm => "rpm",
            Self::Tpm => "tpm",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeGatewayRouteModelMetrics {
    pub input_cost_per_million_microusd: Option<u64>,
    pub output_cost_per_million_microusd: Option<u64>,
    pub latency_ms: Option<u64>,
    pub rpm_limit: Option<u64>,
    pub tpm_limit: Option<u64>,
}

impl RuntimeGatewayRouteModelMetrics {
    fn cost_score(&self) -> Option<u64> {
        match (
            self.input_cost_per_million_microusd,
            self.output_cost_per_million_microusd,
        ) {
            (Some(input), Some(output)) => Some(input.saturating_add(output)),
            (Some(input), None) => Some(input),
            (None, Some(output)) => Some(output),
            (None, None) => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeGatewayRouteModelState {
    pub in_flight: usize,
    pub latency_ms_ewma: Option<u64>,
    pub minute_epoch: u64,
    pub requests_this_minute: u64,
    pub tokens_this_minute: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGatewayRouteRewrite {
    pub alias: String,
    pub strategy: RuntimeGatewayRouteStrategy,
    pub model: String,
    pub body: Vec<u8>,
}

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
        .collect::<Vec<_>>();
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
                    .and_then(RuntimeGatewayRouteModelMetrics::cost_score)
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
