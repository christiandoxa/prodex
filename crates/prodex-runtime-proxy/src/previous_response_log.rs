use crate::{
    RuntimePreviousResponseFreshFallbackShape, RuntimePreviousResponseNotFoundDecision,
    runtime_previous_response_fresh_fallback_shape_label,
    runtime_previous_response_not_found_observability_outcome,
};

#[derive(Clone, Copy)]
pub struct RuntimePreviousResponseLogContext<'a> {
    pub request_id: u64,
    pub transport: &'a str,
    pub route: &'a str,
    pub websocket_session: Option<u64>,
    pub via: Option<&'a str>,
}

fn render_previous_response_log(
    operation: i64,
    context: RuntimePreviousResponseLogContext<'_>,
    profile_name: &str,
    retry_index: usize,
    detail_one: &str,
    detail_two: &str,
    blocked: bool,
) -> String {
    prodex_mojo_core::log::render_previous_response_log(
        prodex_mojo_core::log::PreviousResponseLogRenderInput {
            operation,
            request_id: context.request_id,
            transport: context.transport,
            route: context.route,
            websocket_session: context.websocket_session,
            via: context.via,
            profile: profile_name,
            retry_index,
            detail_one,
            detail_two,
            blocked,
        },
    )
    .expect("Mojo previous-response log renderer returned invalid output")
}

pub fn runtime_previous_response_not_found_log_message(
    context: RuntimePreviousResponseLogContext<'_>,
    profile_name: &str,
    retry_index: usize,
    turn_state: Option<&str>,
) -> String {
    let turn_state_debug = format!("{turn_state:?}");
    render_previous_response_log(
        prodex_mojo_core::log::PREVIOUS_RESPONSE_LOG_NOT_FOUND,
        context,
        profile_name,
        retry_index,
        &turn_state_debug,
        "",
        false,
    )
}

pub fn runtime_previous_response_retry_immediate_log_message(
    context: RuntimePreviousResponseLogContext<'_>,
    profile_name: &str,
    delay_ms: u128,
    reason: &str,
) -> String {
    let delay_ms = delay_ms.to_string();
    render_previous_response_log(
        prodex_mojo_core::log::PREVIOUS_RESPONSE_LOG_RETRY_IMMEDIATE,
        context,
        profile_name,
        0,
        &delay_ms,
        reason,
        false,
    )
}

pub fn runtime_previous_response_stale_continuation_log_message(
    context: RuntimePreviousResponseLogContext<'_>,
    profile_name: &str,
) -> String {
    render_previous_response_log(
        prodex_mojo_core::log::PREVIOUS_RESPONSE_LOG_STALE_CONTINUATION,
        context,
        profile_name,
        0,
        "",
        "",
        false,
    )
}

pub fn runtime_previous_response_not_found_fresh_fallback_log_message(
    context: RuntimePreviousResponseLogContext<'_>,
    decision: RuntimePreviousResponseNotFoundDecision,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    profile_name: &str,
    blocked: bool,
) -> String {
    let outcome =
        runtime_previous_response_not_found_observability_outcome(decision, fresh_fallback_shape)
            .unwrap_or("-");
    render_previous_response_log(
        prodex_mojo_core::log::PREVIOUS_RESPONSE_LOG_FRESH_FALLBACK,
        context,
        profile_name,
        0,
        runtime_previous_response_fresh_fallback_shape_label(fresh_fallback_shape),
        outcome,
        blocked,
    )
}

pub fn runtime_previous_response_affinity_released_log_message(
    context: RuntimePreviousResponseLogContext<'_>,
    profile_name: &str,
) -> String {
    render_previous_response_log(
        prodex_mojo_core::log::PREVIOUS_RESPONSE_LOG_AFFINITY_RELEASED,
        context,
        profile_name,
        0,
        "",
        "",
        false,
    )
}

#[cfg(test)]
#[path = "../tests/src/previous_response_log.rs"]
mod tests;
