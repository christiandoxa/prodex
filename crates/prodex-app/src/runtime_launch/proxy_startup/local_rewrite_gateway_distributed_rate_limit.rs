use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use prodex_application::distributed_rate_limit::ApplicationDistributedRateLimitPlan;
use prodex_storage_redis::{RedisDualRateLimitDecision, RedisRateLimitDimension};

pub(super) fn runtime_gateway_distributed_rate_limit_admission(
    shared: &RuntimeLocalRewriteProxyShared,
    plan: &ApplicationDistributedRateLimitPlan,
) -> Result<(), runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection> {
    let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() else {
        return Ok(());
    };
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
