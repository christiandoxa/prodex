//! Compact-route transport failure handling.

use std::time::Instant;

use super::super::super::{
    build_runtime_proxy_json_error_response, runtime_proxy_local_selection_failure_message,
    runtime_proxy_log,
};
use super::{
    flow::RuntimeCompactFailureFlow,
    logging::{
        RuntimeProxyCompactAttemptFailureLog, log_runtime_proxy_compact_attempt_final_failure,
    },
};
use crate::runtime_state_shared::RuntimeRotationProxyShared;

pub(super) struct RuntimeProxyCompactTransportFailure<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: &'a str,
    pub(super) stage: &'static str,
    pub(super) hard_affinity: bool,
    pub(super) selection_attempts: usize,
    pub(super) selection_started_at: Instant,
    pub(super) pressure_mode: bool,
    pub(super) last_failure: Option<&'a (tiny_http::ResponseBox, bool)>,
    pub(super) saw_inflight_saturation: bool,
    pub(super) saw_transport_failure: bool,
}

pub(super) fn finish_runtime_proxy_compact_transport_failure(
    failure: RuntimeProxyCompactTransportFailure<'_>,
) -> RuntimeCompactFailureFlow {
    runtime_proxy_log(
        failure.shared,
        format!(
            "request={} transport=http compact_transport_failure profile={} route=compact stage={}",
            failure.request_id, failure.profile_name, failure.stage
        ),
    );
    if !failure.hard_affinity {
        return RuntimeCompactFailureFlow::Retry;
    }

    log_runtime_proxy_compact_attempt_final_failure(
        failure.shared,
        RuntimeProxyCompactAttemptFailureLog {
            request_id: failure.request_id,
            exit: "hard_affinity_transport_failure",
            reason: "transport",
            selection_attempts: failure.selection_attempts,
            selection_started_at: failure.selection_started_at,
            pressure_mode: failure.pressure_mode,
            last_failure: failure.last_failure,
            saw_inflight_saturation: failure.saw_inflight_saturation,
            saw_transport_failure: failure.saw_transport_failure,
            profile_name: failure.profile_name,
        },
    );
    RuntimeCompactFailureFlow::Return(build_runtime_proxy_json_error_response(
        503,
        "service_unavailable",
        runtime_proxy_local_selection_failure_message(),
    ))
}
