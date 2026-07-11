//! Smart-context rollout and environment flag helpers.

use super::*;
pub(super) fn runtime_smart_context_rollout_decision(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
) -> runtime_proxy_crate::SmartContextRolloutDecision {
    runtime_proxy_crate::smart_context_rollout_decision(
        runtime_proxy_crate::SmartContextRolloutDecisionInput {
            enabled: true,
            explicit_exact_mode: runtime_smart_context_exact_header(request),
            shadow_mode: shared.runtime_config.smart_context_shadow,
            canary_percent: shared.runtime_config.smart_context_canary_percent,
            stable_key: format!(
                "{}:{}:{}:{}:{}",
                shared.log_path.display(),
                profile_name.unwrap_or("-"),
                runtime_route_kind_label(route_kind),
                transport.label(),
                request_id
            ),
        },
    )
}
