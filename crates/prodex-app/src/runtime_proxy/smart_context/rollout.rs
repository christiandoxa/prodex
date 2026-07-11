//! Smart-context rollout and environment flag helpers.

use super::*;
use std::env;

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
            shadow_mode: runtime_smart_context_env_flag("PRODEX_SMART_CONTEXT_SHADOW"),
            canary_percent: runtime_smart_context_env_percent(
                "PRODEX_SMART_CONTEXT_CANARY_PERCENT",
                100,
            ),
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

pub(super) fn runtime_smart_context_env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub(super) fn runtime_smart_context_env_percent(name: &str, default: u8) -> u8 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u8>().ok())
        .map(|value| value.min(100))
        .unwrap_or(default)
}
