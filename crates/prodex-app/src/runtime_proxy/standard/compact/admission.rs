//! Compact-route local admission pressure helpers.

use super::super::super::{
    build_runtime_proxy_json_error_response, runtime_profile_inflight_hard_limited_for_context,
    runtime_proxy_log, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use super::logging::{RuntimeProxyCompactFinalFailureLog, log_runtime_proxy_compact_final_failure};
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use std::time::Instant;

pub(super) fn build_runtime_fresh_compact_pressure_response(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    selection_attempts: usize,
    selection_started_at: Instant,
    pressure_mode: bool,
) -> tiny_http::ResponseBox {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http compact_pressure_shed reason=fresh_request pressure_mode={pressure_mode}"
        ),
    );
    log_runtime_proxy_compact_final_failure(
        shared,
        RuntimeProxyCompactFinalFailureLog {
            request_id,
            exit: "pressure",
            reason: "pressure",
            selection_attempts,
            selection_started_at,
            pressure_mode,
            last_failure_kind: "none",
            saw_inflight_saturation: false,
            saw_transport_failure: false,
            profile_name: None,
        },
    );
    build_runtime_proxy_json_error_response(
        503,
        "service_unavailable",
        "Fresh compact requests are temporarily deferred while the runtime proxy is under pressure. Retry the request.",
    )
}

fn log_runtime_compact_inflight_saturated(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "profile_inflight_saturated",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("profile", profile_name),
                runtime_proxy_log_field(
                    "hard_limit",
                    shared
                        .runtime_config
                        .tuning
                        .profile_inflight_hard_limit
                        .to_string(),
                ),
            ],
        ),
    );
}

pub(super) fn runtime_compact_candidate_inflight_saturated(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    hard_affinity: bool,
) -> anyhow::Result<bool> {
    if hard_affinity
        || !runtime_profile_inflight_hard_limited_for_context(shared, profile_name, "compact_http")?
    {
        return Ok(false);
    }
    log_runtime_compact_inflight_saturated(request_id, shared, profile_name);
    Ok(true)
}

pub(super) fn log_runtime_compact_local_selection_blocked(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http local_selection_blocked profile={profile_name} route=compact reason=quota_exhausted_before_send"
        ),
    );
}
