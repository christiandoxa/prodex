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

fn runtime_previous_response_log_suffix(context: RuntimePreviousResponseLogContext<'_>) -> String {
    context
        .via
        .map(|via| format!(" via={via}"))
        .unwrap_or_default()
}

fn runtime_previous_response_not_found_prefix(
    context: RuntimePreviousResponseLogContext<'_>,
) -> String {
    match context.websocket_session {
        Some(session_id) => format!(
            "request={} transport={} route={} websocket_session={session_id}",
            context.request_id, context.transport, context.route
        ),
        None => format!(
            "request={} transport={} route={}",
            context.request_id, context.transport, context.route
        ),
    }
}

fn runtime_previous_response_event_prefix(
    context: RuntimePreviousResponseLogContext<'_>,
) -> String {
    match context.websocket_session {
        Some(session_id) => format!(
            "request={} websocket_session={session_id}",
            context.request_id
        ),
        None => format!(
            "request={} transport={}",
            context.request_id, context.transport
        ),
    }
}

pub fn runtime_previous_response_not_found_log_message(
    context: RuntimePreviousResponseLogContext<'_>,
    profile_name: &str,
    retry_index: usize,
    turn_state: Option<&str>,
) -> String {
    format!(
        "{} previous_response_not_found profile={profile_name} retry_index={retry_index} replay_turn_state={turn_state:?}{}",
        runtime_previous_response_not_found_prefix(context),
        runtime_previous_response_log_suffix(context),
    )
}

pub fn runtime_previous_response_retry_immediate_log_message(
    context: RuntimePreviousResponseLogContext<'_>,
    profile_name: &str,
    delay_ms: u128,
    reason: &str,
) -> String {
    format!(
        "{} previous_response_retry_immediate profile={profile_name} delay_ms={delay_ms} reason={reason}{}",
        runtime_previous_response_event_prefix(context),
        runtime_previous_response_log_suffix(context),
    )
}

pub fn runtime_previous_response_stale_continuation_log_message(
    context: RuntimePreviousResponseLogContext<'_>,
    profile_name: &str,
) -> String {
    format!(
        "{} stale_continuation reason=previous_response_not_found_locked_affinity profile={profile_name}{}",
        runtime_previous_response_event_prefix(context),
        runtime_previous_response_log_suffix(context),
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
    let event = if blocked {
        "previous_response_fresh_fallback_blocked"
    } else {
        "previous_response_fresh_fallback"
    };
    format!(
        "{} {event} reason=previous_response_not_found request_shape={} outcome={outcome} profile={profile_name}{}",
        runtime_previous_response_event_prefix(context),
        runtime_previous_response_fresh_fallback_shape_label(fresh_fallback_shape),
        runtime_previous_response_log_suffix(context),
    )
}

pub fn runtime_previous_response_affinity_released_log_message(
    context: RuntimePreviousResponseLogContext<'_>,
    profile_name: &str,
) -> String {
    format!(
        "{} previous_response_affinity_released profile={profile_name}{}",
        runtime_previous_response_event_prefix(context),
        runtime_previous_response_log_suffix(context),
    )
}

#[cfg(test)]
#[path = "../../../tests/unit/crates/prodex-runtime-proxy/src/previous_response_log.rs"]
mod tests;
