use std::collections::BTreeMap;

use crate::{
    LocalBridgeBearerTokenHash, local_bridge_authorization_bearer_token,
    runtime_gateway_request_model,
};
use serde::{Deserialize, Serialize};

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
        if value != value.trim() {
            return None;
        }
        match value.to_ascii_lowercase().as_str() {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGatewayVirtualKey {
    pub name: String,
    pub tenant_id: Option<String>,
    pub team_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub budget_id: Option<String>,
    pub token_hash: LocalBridgeBearerTokenHash,
    pub allowed_models: Vec<String>,
    pub budget_microusd: Option<u64>,
    pub request_budget: Option<u64>,
    pub rpm_limit: Option<u64>,
    pub tpm_limit: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeGatewayVirtualKeyUsage {
    pub minute_epoch: u64,
    pub requests_this_minute: u64,
    pub tokens_this_minute: u64,
    pub requests_total: u64,
    pub spend_microusd: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeGatewayVirtualKeyAdmission {
    pub key_name: String,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub estimated_cost_microusd: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGatewayVirtualKeyRejection {
    MissingOrInvalidToken,
    ModelNotAllowed,
    RequestBudgetExceeded,
    BudgetExceeded,
    RpmLimitExceeded,
    TpmLimitExceeded,
}

impl RuntimeGatewayVirtualKeyRejection {
    pub fn status(self) -> u16 {
        match self {
            Self::MissingOrInvalidToken => 401,
            Self::ModelNotAllowed | Self::RequestBudgetExceeded | Self::BudgetExceeded => 403,
            Self::RpmLimitExceeded | Self::TpmLimitExceeded => 429,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::MissingOrInvalidToken => "invalid_gateway_key",
            Self::ModelNotAllowed => "model_not_allowed",
            Self::RequestBudgetExceeded => "request_budget_exceeded",
            Self::BudgetExceeded => "budget_exceeded",
            Self::RpmLimitExceeded => "rpm_limit_exceeded",
            Self::TpmLimitExceeded => "tpm_limit_exceeded",
        }
    }
}

pub fn runtime_gateway_virtual_key_from_headers<'a>(
    headers: &[(String, String)],
    keys: &'a [RuntimeGatewayVirtualKey],
) -> Result<Option<&'a RuntimeGatewayVirtualKey>, RuntimeGatewayVirtualKeyRejection> {
    if keys.is_empty() {
        return Ok(None);
    }
    let Some(token) = headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("authorization")
            .then(|| local_bridge_authorization_bearer_token(value))
            .flatten()
    }) else {
        return Err(RuntimeGatewayVirtualKeyRejection::MissingOrInvalidToken);
    };
    keys.iter()
        .find(|key| key.token_hash.verify_bearer_token(token))
        .map(Some)
        .ok_or(RuntimeGatewayVirtualKeyRejection::MissingOrInvalidToken)
}

pub fn runtime_gateway_virtual_key_admission(
    key: &RuntimeGatewayVirtualKey,
    usage: Option<&RuntimeGatewayVirtualKeyUsage>,
    body: &[u8],
    estimated_cost_microusd: Option<u64>,
    minute_epoch: u64,
) -> Result<RuntimeGatewayVirtualKeyAdmission, RuntimeGatewayVirtualKeyRejection> {
    let model = runtime_gateway_request_model(body);
    if !key.allowed_models.is_empty()
        && model.as_ref().is_some_and(|model| {
            !key.allowed_models
                .iter()
                .any(|allowed| allowed.trim().eq_ignore_ascii_case(model))
        })
    {
        return Err(RuntimeGatewayVirtualKeyRejection::ModelNotAllowed);
    }

    let input_tokens = runtime_gateway_estimated_tokens(body);
    let usage = usage.cloned().unwrap_or_default();
    let same_minute = usage.minute_epoch == minute_epoch;
    let requests_this_minute = if same_minute {
        usage.requests_this_minute
    } else {
        0
    };
    let tokens_this_minute = if same_minute {
        usage.tokens_this_minute
    } else {
        0
    };

    if let Some(limit) = key.request_budget
        && usage.requests_total >= limit
    {
        return Err(RuntimeGatewayVirtualKeyRejection::RequestBudgetExceeded);
    }
    if let (Some(limit), Some(cost)) = (key.budget_microusd, estimated_cost_microusd)
        && usage.spend_microusd.saturating_add(cost) > limit
    {
        return Err(RuntimeGatewayVirtualKeyRejection::BudgetExceeded);
    }
    if let Some(limit) = key.rpm_limit
        && requests_this_minute.saturating_add(1) > limit
    {
        return Err(RuntimeGatewayVirtualKeyRejection::RpmLimitExceeded);
    }
    if let Some(limit) = key.tpm_limit
        && tokens_this_minute.saturating_add(input_tokens) > limit
    {
        return Err(RuntimeGatewayVirtualKeyRejection::TpmLimitExceeded);
    }

    Ok(RuntimeGatewayVirtualKeyAdmission {
        key_name: key.name.clone(),
        model,
        input_tokens,
        estimated_cost_microusd,
    })
}

pub fn runtime_gateway_record_virtual_key_usage(
    usage: &mut RuntimeGatewayVirtualKeyUsage,
    admission: &RuntimeGatewayVirtualKeyAdmission,
    minute_epoch: u64,
) {
    if usage.minute_epoch != minute_epoch {
        usage.minute_epoch = minute_epoch;
        usage.requests_this_minute = 0;
        usage.tokens_this_minute = 0;
    }
    usage.requests_this_minute = usage.requests_this_minute.saturating_add(1);
    usage.tokens_this_minute = usage
        .tokens_this_minute
        .saturating_add(admission.input_tokens);
    usage.requests_total = usage.requests_total.saturating_add(1);
    if let Some(cost) = admission.estimated_cost_microusd {
        usage.spend_microusd = usage.spend_microusd.saturating_add(cost);
    }
}

pub fn runtime_gateway_minute_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 60)
        .unwrap_or_default()
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

fn runtime_gateway_estimated_tokens(body: &[u8]) -> u64 {
    prodex_provider_core::estimate_request_input_tokens(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_strategy_parse_rejects_padded_values() {
        assert_eq!(
            RuntimeGatewayRouteStrategy::parse("lowest-cost"),
            Some(RuntimeGatewayRouteStrategy::LowestCost)
        );
        assert_eq!(RuntimeGatewayRouteStrategy::parse(" lowest-cost "), None);
    }

    #[test]
    fn rewrites_model_alias_to_combo_chain() {
        let aliases = vec![RuntimeGatewayRouteAlias {
            alias: "prodex-fast".to_string(),
            models: vec!["gpt-5-mini".to_string(), "gpt-5-nano".to_string()],
            strategy: RuntimeGatewayRouteStrategy::Fallback,
            model_metrics: BTreeMap::new(),
        }];
        let rewrite = runtime_gateway_rewrite_route_alias(
            br#"{"model":"prodex-fast","input":"hi"}"#,
            &aliases,
            1,
        )
        .expect("alias should rewrite");
        let value: serde_json::Value = serde_json::from_slice(&rewrite.body).unwrap();
        assert_eq!(rewrite.alias, "prodex-fast");
        assert_eq!(rewrite.strategy, RuntimeGatewayRouteStrategy::Fallback);
        assert_eq!(rewrite.model, "combo:gpt-5-mini,gpt-5-nano");
        assert_eq!(value["model"], "combo:gpt-5-mini,gpt-5-nano");
        assert_eq!(value["input"], "hi");
    }

    #[test]
    fn leaves_unknown_model_unchanged() {
        let aliases = vec![RuntimeGatewayRouteAlias {
            alias: "prodex-fast".to_string(),
            models: vec!["gpt-5-mini".to_string()],
            strategy: RuntimeGatewayRouteStrategy::Fallback,
            model_metrics: BTreeMap::new(),
        }];
        assert!(
            runtime_gateway_rewrite_route_alias(br#"{"model":"other"}"#, &aliases, 1).is_none()
        );
    }

    #[test]
    fn round_robin_strategy_picks_model_by_request_id() {
        let aliases = vec![RuntimeGatewayRouteAlias {
            alias: "prodex-fast".to_string(),
            models: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            strategy: RuntimeGatewayRouteStrategy::RoundRobin,
            model_metrics: BTreeMap::new(),
        }];

        let first = runtime_gateway_rewrite_route_alias(br#"{"model":"prodex-fast"}"#, &aliases, 1)
            .expect("first rewrite");
        let second =
            runtime_gateway_rewrite_route_alias(br#"{"model":"prodex-fast"}"#, &aliases, 2)
                .expect("second rewrite");
        let fourth =
            runtime_gateway_rewrite_route_alias(br#"{"model":"prodex-fast"}"#, &aliases, 4)
                .expect("fourth rewrite");

        assert_eq!(first.model, "a");
        assert_eq!(second.model, "b");
        assert_eq!(fourth.model, "a");
    }

    #[test]
    fn least_busy_strategy_picks_lowest_inflight_model() {
        let aliases = vec![RuntimeGatewayRouteAlias {
            alias: "prodex-fast".to_string(),
            models: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            strategy: RuntimeGatewayRouteStrategy::LeastBusy,
            model_metrics: BTreeMap::new(),
        }];
        let state = BTreeMap::from([
            (
                "a".to_string(),
                RuntimeGatewayRouteModelState {
                    in_flight: 3,
                    ..RuntimeGatewayRouteModelState::default()
                },
            ),
            (
                "b".to_string(),
                RuntimeGatewayRouteModelState {
                    in_flight: 1,
                    ..RuntimeGatewayRouteModelState::default()
                },
            ),
            (
                "c".to_string(),
                RuntimeGatewayRouteModelState {
                    in_flight: 2,
                    ..RuntimeGatewayRouteModelState::default()
                },
            ),
        ]);

        let rewrite = runtime_gateway_rewrite_route_alias_with_state(
            br#"{"model":"prodex-fast"}"#,
            &aliases,
            1,
            &state,
        )
        .expect("least busy rewrite");

        assert_eq!(rewrite.strategy, RuntimeGatewayRouteStrategy::LeastBusy);
        assert_eq!(rewrite.model, "b");
    }

    #[test]
    fn metric_strategies_pick_policy_or_runtime_best_model() {
        let metrics = BTreeMap::from([
            (
                "a".to_string(),
                RuntimeGatewayRouteModelMetrics {
                    input_cost_per_million_microusd: Some(20),
                    output_cost_per_million_microusd: Some(30),
                    latency_ms: Some(300),
                    rpm_limit: Some(100),
                    tpm_limit: Some(10_000),
                },
            ),
            (
                "b".to_string(),
                RuntimeGatewayRouteModelMetrics {
                    input_cost_per_million_microusd: Some(10),
                    output_cost_per_million_microusd: Some(15),
                    latency_ms: Some(500),
                    rpm_limit: Some(20),
                    tpm_limit: Some(100_000),
                },
            ),
        ]);
        let state = BTreeMap::from([(
            "a".to_string(),
            RuntimeGatewayRouteModelState {
                latency_ms_ewma: Some(80),
                requests_this_minute: 90,
                tokens_this_minute: 9_900,
                ..RuntimeGatewayRouteModelState::default()
            },
        )]);
        for (strategy, expected) in [
            (RuntimeGatewayRouteStrategy::LowestCost, "b"),
            (RuntimeGatewayRouteStrategy::LowestLatency, "a"),
            (RuntimeGatewayRouteStrategy::Rpm, "b"),
            (RuntimeGatewayRouteStrategy::Tpm, "b"),
        ] {
            let aliases = vec![RuntimeGatewayRouteAlias {
                alias: "prodex-fast".to_string(),
                models: vec!["a".to_string(), "b".to_string()],
                strategy,
                model_metrics: metrics.clone(),
            }];
            let rewrite = runtime_gateway_rewrite_route_alias_with_state(
                br#"{"model":"prodex-fast","input":"hello"}"#,
                &aliases,
                1,
                &state,
            )
            .expect("metric rewrite");
            assert_eq!(rewrite.model, expected, "{strategy:?}");
        }
    }

    #[test]
    fn virtual_key_authorizes_and_records_usage() {
        let key = RuntimeGatewayVirtualKey {
            name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: LocalBridgeBearerTokenHash::from_token("secret"),
            allowed_models: vec!["prodex-fast".to_string()],
            budget_microusd: Some(10_000),
            request_budget: Some(2),
            rpm_limit: Some(2),
            tpm_limit: Some(100),
        };
        let headers = vec![("Authorization".to_string(), "Bearer secret".to_string())];
        let keys = vec![key];
        let selected = runtime_gateway_virtual_key_from_headers(&headers, &keys)
            .expect("valid key")
            .expect("key enabled");
        assert_eq!(selected.name, "team-a");

        let mut usage = RuntimeGatewayVirtualKeyUsage::default();
        let admission = runtime_gateway_virtual_key_admission(
            selected,
            Some(&usage),
            br#"{"model":"prodex-fast","input":"hello from prodex"}"#,
            Some(500),
            10,
        )
        .expect("admission");
        runtime_gateway_record_virtual_key_usage(&mut usage, &admission, 10);

        assert_eq!(usage.minute_epoch, 10);
        assert_eq!(usage.requests_this_minute, 1);
        assert_eq!(usage.requests_total, 1);
        assert_eq!(usage.spend_microusd, 500);
    }

    #[test]
    fn virtual_key_rejects_model_and_rate_limits() {
        let key = RuntimeGatewayVirtualKey {
            name: "team-a".to_string(),
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            token_hash: LocalBridgeBearerTokenHash::from_token("secret"),
            allowed_models: vec!["prodex-fast".to_string()],
            budget_microusd: None,
            request_budget: None,
            rpm_limit: Some(1),
            tpm_limit: None,
        };
        let bad_model = runtime_gateway_virtual_key_admission(
            &key,
            None,
            br#"{"model":"other","input":"hello"}"#,
            None,
            10,
        )
        .unwrap_err();
        assert_eq!(
            bad_model,
            RuntimeGatewayVirtualKeyRejection::ModelNotAllowed
        );

        let usage = RuntimeGatewayVirtualKeyUsage {
            minute_epoch: 10,
            requests_this_minute: 1,
            ..RuntimeGatewayVirtualKeyUsage::default()
        };
        let rate_limit = runtime_gateway_virtual_key_admission(
            &key,
            Some(&usage),
            br#"{"model":"prodex-fast","input":"hello"}"#,
            None,
            10,
        )
        .unwrap_err();
        assert_eq!(
            rate_limit,
            RuntimeGatewayVirtualKeyRejection::RpmLimitExceeded
        );
    }
}
