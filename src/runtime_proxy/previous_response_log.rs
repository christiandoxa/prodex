use super::*;

pub(crate) use runtime_proxy_crate::{
    RuntimePreviousResponseLogContext, runtime_previous_response_affinity_released_log_message,
    runtime_previous_response_not_found_fresh_fallback_log_message,
    runtime_previous_response_not_found_log_message,
    runtime_previous_response_retry_immediate_log_message,
    runtime_previous_response_stale_continuation_log_message,
};

pub(crate) fn runtime_proxy_log_previous_response_stale_continuation(
    shared: &RuntimeRotationProxyShared,
    context: RuntimePreviousResponseLogContext<'_>,
    profile_name: &str,
) {
    runtime_proxy_record_continuity_failure_reason(
        shared,
        "stale_continuation",
        "previous_response_not_found_locked_affinity",
    );
    runtime_proxy_log(
        shared,
        runtime_previous_response_stale_continuation_log_message(context, profile_name),
    );
}
