#[derive(Clone, Copy)]
pub(crate) struct RuntimePreviousResponseLogContext<'a> {
    pub(crate) request_id: u64,
    pub(crate) transport: &'a str,
    pub(crate) route: &'a str,
    pub(crate) websocket_session: Option<u64>,
    pub(crate) via: Option<&'a str>,
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

pub(crate) fn runtime_previous_response_not_found_log_message(
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

pub(crate) fn runtime_previous_response_retry_immediate_log_message(
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

pub(crate) fn runtime_previous_response_stale_continuation_log_message(
    context: RuntimePreviousResponseLogContext<'_>,
    profile_name: &str,
) -> String {
    format!(
        "{} stale_continuation reason=previous_response_not_found_locked_affinity profile={profile_name}{}",
        runtime_previous_response_event_prefix(context),
        runtime_previous_response_log_suffix(context),
    )
}

pub(crate) fn runtime_previous_response_not_found_fresh_fallback_log_message(
    context: RuntimePreviousResponseLogContext<'_>,
    decision: crate::runtime_proxy::RuntimePreviousResponseNotFoundDecision,
    fresh_fallback_shape: Option<crate::runtime_proxy::RuntimePreviousResponseFreshFallbackShape>,
    profile_name: &str,
    blocked: bool,
) -> String {
    let outcome = crate::runtime_proxy::runtime_previous_response_not_found_observability_outcome(
        decision,
        fresh_fallback_shape,
    )
    .unwrap_or("-");
    let event = if blocked {
        "previous_response_fresh_fallback_blocked"
    } else {
        "previous_response_fresh_fallback"
    };
    format!(
        "{} {event} reason=previous_response_not_found request_shape={} outcome={outcome} profile={profile_name}{}",
        runtime_previous_response_event_prefix(context),
        crate::runtime_proxy::runtime_previous_response_fresh_fallback_shape_label(
            fresh_fallback_shape,
        ),
        runtime_previous_response_log_suffix(context),
    )
}

pub(crate) fn runtime_previous_response_affinity_released_log_message(
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
mod tests {
    use super::*;

    #[test]
    fn websocket_previous_response_logs_keep_existing_fields() {
        let context = RuntimePreviousResponseLogContext {
            request_id: 17,
            transport: "websocket",
            route: "websocket",
            websocket_session: Some(23),
            via: Some("direct_current_profile_fallback"),
        };

        assert_eq!(
            runtime_previous_response_not_found_log_message(
                context,
                "alpha",
                2,
                Some("turn-state"),
            ),
            "request=17 transport=websocket route=websocket websocket_session=23 previous_response_not_found profile=alpha retry_index=2 replay_turn_state=Some(\"turn-state\") via=direct_current_profile_fallback"
        );
        assert_eq!(
            runtime_previous_response_retry_immediate_log_message(
                context,
                "alpha",
                250,
                "non_blocking_retry",
            ),
            "request=17 websocket_session=23 previous_response_retry_immediate profile=alpha delay_ms=250 reason=non_blocking_retry via=direct_current_profile_fallback"
        );
        assert_eq!(
            runtime_previous_response_stale_continuation_log_message(context, "alpha"),
            "request=17 websocket_session=23 stale_continuation reason=previous_response_not_found_locked_affinity profile=alpha via=direct_current_profile_fallback"
        );
        assert_eq!(
            runtime_previous_response_affinity_released_log_message(context, "alpha"),
            "request=17 websocket_session=23 previous_response_affinity_released profile=alpha via=direct_current_profile_fallback"
        );
    }

    #[test]
    fn http_previous_response_logs_keep_existing_fields() {
        let context = RuntimePreviousResponseLogContext {
            request_id: 41,
            transport: "http",
            route: "responses",
            websocket_session: None,
            via: None,
        };

        assert_eq!(
            runtime_previous_response_not_found_log_message(context, "beta", 1, None),
            "request=41 transport=http route=responses previous_response_not_found profile=beta retry_index=1 replay_turn_state=None"
        );
        assert_eq!(
            runtime_previous_response_retry_immediate_log_message(context, "beta", 500, "-",),
            "request=41 transport=http previous_response_retry_immediate profile=beta delay_ms=500 reason=-"
        );
        assert_eq!(
            runtime_previous_response_affinity_released_log_message(context, "beta"),
            "request=41 transport=http previous_response_affinity_released profile=beta"
        );
        assert_eq!(
            runtime_previous_response_not_found_fresh_fallback_log_message(
                context,
                crate::runtime_proxy::RuntimePreviousResponseNotFoundDecision {
                    retry_delay: None,
                    retry_reason: None,
                    chain_retry_reason: None,
                    request_requires_locked_previous_response_affinity: false,
                    stale_continuation: false,
                    fresh_fallback_allowed: false,
                    fresh_fallback_blocked_without_affinity: true,
                },
                Some(
                    crate::runtime_proxy::RuntimePreviousResponseFreshFallbackShape::ContinuationOnly
                ),
                "beta",
                true,
            ),
            "request=41 transport=http previous_response_fresh_fallback_blocked reason=previous_response_not_found request_shape=continuation_only outcome=blocked_nonreplayable_without_affinity profile=beta"
        );
    }
}
