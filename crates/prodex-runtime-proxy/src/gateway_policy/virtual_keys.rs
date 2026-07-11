//! Virtual-key authentication, admission, limits, and usage accounting.

use super::runtime_gateway_estimated_tokens;
use crate::{
    LocalBridgeBearerTokenHash, local_bridge_authorization_bearer_token,
    runtime_gateway_request_model,
};
use serde::{Deserialize, Serialize};

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
