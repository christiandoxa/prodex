use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_store_types::RuntimeGatewayVirtualKeyEntry;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use super::*;
use prodex_application::distributed_rate_limit::{
    ApplicationDistributedRateLimitRequest, plan_application_distributed_rate_limit,
};
use prodex_domain::{RateLimitBucketKey, TenantId};
use prodex_storage_redis::{RedisDualRateLimitDecision, RedisRateLimitDimension};

pub(super) fn runtime_gateway_durable_budget_enforced(
    shared: &RuntimeLocalRewriteProxyShared,
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
    entry: Option<&RuntimeGatewayVirtualKeyEntry>,
) -> bool {
    if !matches!(
        &shared.gateway_state_store,
        RuntimeGatewayStateStore::Sqlite { .. } | RuntimeGatewayStateStore::Postgres { .. }
    ) {
        return false;
    }
    if key.budget_microusd.is_none() || key.budget_id.as_deref().is_some_and(|id| !id.is_empty()) {
        return false;
    }
    entry.and_then(|entry| entry.virtual_key_id).is_some()
        && key
            .tenant_id
            .as_deref()
            .and_then(|tenant_id| tenant_id.parse::<TenantId>().ok())
            .is_some()
}

pub(super) fn runtime_gateway_local_admission_usage(
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
    usage: Option<&runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
    durable_budget_enforced: bool,
) -> runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
    let mut usage = usage.cloned().unwrap_or_default();
    if durable_budget_enforced && key.budget_microusd.is_some() {
        usage.spend_microusd = 0;
    }
    usage
}

pub(super) fn runtime_gateway_distributed_rate_limit_required(
    shared: &RuntimeLocalRewriteProxyShared,
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
) -> bool {
    shared.gateway_redis_rate_limit_executor.is_some()
        && (key.rpm_limit.is_some() || key.tpm_limit.is_some())
}

pub(super) fn runtime_gateway_distributed_rate_limit_admission(
    shared: &RuntimeLocalRewriteProxyShared,
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
    entry: Option<&RuntimeGatewayVirtualKeyEntry>,
    admission: &runtime_proxy_crate::RuntimeGatewayVirtualKeyAdmission,
    minute_epoch: u64,
) -> Result<(), runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection> {
    let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() else {
        return Ok(());
    };
    if key.rpm_limit.is_none() && key.tpm_limit.is_none() {
        return Ok(());
    }
    let tenant_id = key
        .tenant_id
        .as_deref()
        .and_then(|value| value.parse::<TenantId>().ok())
        .ok_or(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)?;
    let virtual_key_id = entry
        .and_then(|entry| entry.virtual_key_id)
        .ok_or(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)?;
    let window_start_unix_ms = minute_epoch
        .checked_mul(60_000)
        .ok_or(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)?;
    let plan = plan_application_distributed_rate_limit(ApplicationDistributedRateLimitRequest {
        bucket: RateLimitBucketKey::new(tenant_id, Some(virtual_key_id), window_start_unix_ms),
        window_seconds: 60,
        max_requests: key.rpm_limit,
        max_tokens: key.tpm_limit,
        increment_tokens: admission.reserved_tokens,
        now_unix_ms: runtime_gateway_unix_epoch_millis(),
    })
    .map_err(|_| runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable)?;
    let decision = shared
        .runtime_shared
        .async_runtime
        .handle()
        .block_on(executor.execute_dual(&plan.redis))
        .map_err(|_| {
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::PolicyStateUnavailable
        })?;
    match decision {
        RedisDualRateLimitDecision::Allowed { .. } => Ok(()),
        RedisDualRateLimitDecision::Limited {
            dimension: RedisRateLimitDimension::RequestsPerMinute,
            ..
        } => Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::RpmLimitExceeded),
        RedisDualRateLimitDecision::Limited {
            dimension: RedisRateLimitDimension::TokensPerMinute,
            ..
        } => Err(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::TpmLimitExceeded),
    }
}
