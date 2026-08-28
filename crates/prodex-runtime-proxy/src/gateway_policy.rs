use std::collections::BTreeMap;

use crate::{
    LocalBridgeBearerTokenHash, local_bridge_authorization_bearer_token,
    runtime_gateway_request_model,
};
use prodex_gateway_core::{
    GatewayVirtualKeyAdmissionError, GatewayVirtualKeyAdmissionRequest, GatewayVirtualKeyPolicy,
    GatewayVirtualKeyUsageUpdate, apply_gateway_virtual_key_usage_update,
    plan_gateway_virtual_key_admission,
};

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeGatewayRouteModelState {
    pub in_flight: usize,
    pub latency_ms_ewma: Option<u64>,
    pub minute_epoch: u64,
    pub requests_this_minute: u64,
    pub tokens_this_minute: u64,
}

#[path = "gateway_policy/selection.rs"]
mod selection;
pub(crate) use selection::runtime_gateway_route_selected_model_from_models;
#[cfg(test)]
pub(super) use selection::runtime_gateway_route_selected_model_from_models_rust;
pub use selection::{
    runtime_gateway_rewrite_route_alias, runtime_gateway_rewrite_route_alias_with_state,
};

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

pub type RuntimeGatewayVirtualKeyUsage = prodex_gateway_core::GatewayVirtualKeyUsage;
pub type RuntimeGatewayVirtualKeyAdmission = prodex_gateway_core::GatewayVirtualKeyAdmission;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGatewayVirtualKeyRejection {
    MissingOrInvalidToken,
    ModelNotAllowed,
    RequestBudgetExceeded,
    BudgetExceeded,
    RpmLimitExceeded,
    TpmLimitExceeded,
    GovernanceDenied,
    GovernanceApprovalRequired,
    GovernanceSessionRequired,
    NoEligibleProvider,
    PolicyStateUnavailable,
}

impl RuntimeGatewayVirtualKeyRejection {
    pub fn status(self) -> u16 {
        match self {
            Self::MissingOrInvalidToken => 401,
            Self::ModelNotAllowed
            | Self::RequestBudgetExceeded
            | Self::BudgetExceeded
            | Self::GovernanceDenied
            | Self::GovernanceApprovalRequired
            | Self::GovernanceSessionRequired
            | Self::NoEligibleProvider => 403,
            Self::RpmLimitExceeded | Self::TpmLimitExceeded => 429,
            Self::PolicyStateUnavailable => 503,
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
            Self::GovernanceDenied => "governance_policy_denied",
            Self::GovernanceApprovalRequired => "governance_approval_required",
            Self::GovernanceSessionRequired => "governance_session_required",
            Self::NoEligibleProvider => "no_compliant_provider",
            Self::PolicyStateUnavailable => "gateway_policy_unavailable",
        }
    }
}

impl From<GatewayVirtualKeyAdmissionError> for RuntimeGatewayVirtualKeyRejection {
    fn from(error: GatewayVirtualKeyAdmissionError) -> Self {
        match error {
            GatewayVirtualKeyAdmissionError::ModelNotAllowed => Self::ModelNotAllowed,
            GatewayVirtualKeyAdmissionError::RequestBudgetExceeded => Self::RequestBudgetExceeded,
            GatewayVirtualKeyAdmissionError::BudgetExceeded => Self::BudgetExceeded,
            GatewayVirtualKeyAdmissionError::RpmLimitExceeded => Self::RpmLimitExceeded,
            GatewayVirtualKeyAdmissionError::TpmLimitExceeded => Self::TpmLimitExceeded,
            GatewayVirtualKeyAdmissionError::PolicyStateUnavailable => Self::PolicyStateUnavailable,
        }
    }
}

pub fn runtime_gateway_virtual_key_policy(
    key: &RuntimeGatewayVirtualKey,
) -> GatewayVirtualKeyPolicy {
    GatewayVirtualKeyPolicy {
        name: key.name.clone(),
        tenant_id: key.tenant_id.clone(),
        team_id: key.team_id.clone(),
        project_id: key.project_id.clone(),
        user_id: key.user_id.clone(),
        budget_id: key.budget_id.clone(),
        allowed_models: key.allowed_models.clone(),
        budget_microusd: key.budget_microusd,
        request_budget: key.request_budget,
        rpm_limit: key.rpm_limit,
        tpm_limit: key.tpm_limit,
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
    let input_tokens = prodex_provider_core::estimate_request_input_tokens(body);
    let reserved_tokens = runtime_gateway_estimated_tokens(body);
    plan_gateway_virtual_key_admission(GatewayVirtualKeyAdmissionRequest {
        policy: runtime_gateway_virtual_key_policy(key),
        usage: usage.cloned().unwrap_or_default(),
        grouped_usage: Vec::new(),
        model,
        input_tokens,
        reserved_tokens,
        estimated_cost_microusd,
        minute_epoch,
        reservation: None,
        distributed_rate_limit: false,
        now_unix_ms: 0,
    })
    .map(|plan| plan.admission)
    .map_err(Into::into)
}

pub fn runtime_gateway_record_virtual_key_usage(
    usage: &mut RuntimeGatewayVirtualKeyUsage,
    admission: &RuntimeGatewayVirtualKeyAdmission,
    minute_epoch: u64,
) {
    apply_gateway_virtual_key_usage_update(
        usage,
        GatewayVirtualKeyUsageUpdate::from_admission(admission, minute_epoch),
    );
}

pub fn runtime_gateway_minute_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 60)
        .unwrap_or_default()
}

pub fn runtime_gateway_estimated_tokens(body: &[u8]) -> u64 {
    let input_tokens = prodex_provider_core::estimate_request_input_tokens(body);
    input_tokens.saturating_add(runtime_gateway_requested_output_tokens(body, input_tokens))
}

fn runtime_gateway_requested_output_tokens(body: &[u8], input_tokens: u64) -> u64 {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return input_tokens;
    };
    prodex_provider_core::provider_requested_output_tokens_compat(&value)
        // ponytail: uncapped requests reserve one extra input-sized output budget; replace with
        // model-aware output estimation when clamp logs show it is still too loose.
        .unwrap_or(input_tokens)
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

    #[cfg(feature = "mojo")]
    #[test]
    fn mojo_route_policy_matches_rust_oracle_for_every_strategy() {
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
                in_flight: 3,
                latency_ms_ewma: Some(80),
                requests_this_minute: 90,
                tokens_this_minute: 9_900,
                ..RuntimeGatewayRouteModelState::default()
            },
        )]);
        for strategy in [
            RuntimeGatewayRouteStrategy::Fallback,
            RuntimeGatewayRouteStrategy::RoundRobin,
            RuntimeGatewayRouteStrategy::First,
            RuntimeGatewayRouteStrategy::LeastBusy,
            RuntimeGatewayRouteStrategy::LowestCost,
            RuntimeGatewayRouteStrategy::LowestLatency,
            RuntimeGatewayRouteStrategy::Rpm,
            RuntimeGatewayRouteStrategy::Tpm,
        ] {
            let alias = RuntimeGatewayRouteAlias {
                alias: "route".to_string(),
                models: vec!["a".to_string(), "b".to_string()],
                strategy,
                model_metrics: metrics.clone(),
            };
            let models = alias.models.clone();
            assert_eq!(
                runtime_gateway_route_selected_model_from_models(&alias, &models, 2, &state, 10),
                runtime_gateway_route_selected_model_from_models_rust(
                    &alias, &models, 2, &state, 10
                ),
                "{strategy:?}"
            );
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

        assert_eq!(admission.input_tokens, 5);
        assert_eq!(admission.reserved_tokens, 10);
        assert_eq!(usage.minute_epoch, 10);
        assert_eq!(usage.requests_this_minute, 1);
        assert_eq!(usage.tokens_this_minute, 10);
        assert_eq!(usage.requests_total, 1);
        assert_eq!(usage.spend_microusd, 500);
    }

    #[test]
    fn virtual_key_uncapped_requests_reserve_input_sized_output_fallback() {
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
            rpm_limit: None,
            tpm_limit: Some(10),
        };

        let admission = runtime_gateway_virtual_key_admission(
            &key,
            None,
            br#"{"model":"prodex-fast","input":"hello from prodex"}"#,
            None,
            10,
        )
        .expect("admission");
        assert_eq!(admission.input_tokens, 5);
        assert_eq!(admission.reserved_tokens, 10);

        let rejected = runtime_gateway_virtual_key_admission(
            &key,
            Some(&RuntimeGatewayVirtualKeyUsage {
                minute_epoch: 10,
                tokens_this_minute: 1,
                ..RuntimeGatewayVirtualKeyUsage::default()
            }),
            br#"{"model":"prodex-fast","input":"hello from prodex"}"#,
            None,
            10,
        )
        .unwrap_err();
        assert_eq!(
            rejected,
            RuntimeGatewayVirtualKeyRejection::TpmLimitExceeded
        );
    }

    #[test]
    fn virtual_key_tpm_uses_requested_output_tokens_in_reservation_estimate() {
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
            rpm_limit: None,
            tpm_limit: Some(25),
        };
        let admission = runtime_gateway_virtual_key_admission(
            &key,
            None,
            br#"{"model":"prodex-fast","input":"hello from prodex","max_output_tokens":17}"#,
            None,
            10,
        )
        .expect("admission");

        assert_eq!(admission.input_tokens, 5);
        assert_eq!(admission.reserved_tokens, 22);

        let rejected = runtime_gateway_virtual_key_admission(
            &key,
            Some(&RuntimeGatewayVirtualKeyUsage {
                minute_epoch: 10,
                tokens_this_minute: 1,
                ..RuntimeGatewayVirtualKeyUsage::default()
            }),
            br#"{"model":"prodex-fast","input":"hello from prodex","max_output_tokens":20}"#,
            None,
            10,
        )
        .unwrap_err();
        assert_eq!(
            rejected,
            RuntimeGatewayVirtualKeyRejection::TpmLimitExceeded
        );
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
