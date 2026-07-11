#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeResponseRouteAffinityPresence {
    pub previous_response_id_present: bool,
    pub request_turn_state_present: bool,
    pub request_session_id_present: bool,
    pub explicit_request_session_id_present: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeResponseRouteAffinityLogContext<'a> {
    pub request_id: u64,
    pub websocket_session_id: Option<u64>,
    pub reason: &'a str,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeResponseRouteAffinityLogState<'a> {
    pub bound_session_profile: Option<&'a str>,
    pub compact_followup_profile: Option<(&'a str, &'a str)>,
    pub compact_session_profile: Option<&'a str>,
    pub session_profile: Option<&'a str>,
    pub pinned_profile: Option<&'a str>,
}

pub fn runtime_response_route_affinity_log_prefix(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
) -> String {
    match context.websocket_session_id {
        Some(session_id) => {
            format!(
                "request={} websocket_session={session_id}",
                context.request_id
            )
        }
        None => format!("request={} transport=http", context.request_id),
    }
}

pub fn runtime_response_route_affinity_recompute_log_message(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
    presence: RuntimeResponseRouteAffinityPresence,
) -> String {
    format!(
        "{} route_affinity_recompute reason={} previous_response_id_present={} request_turn_state_present={} request_session_id_present={} explicit_session_id_present={}",
        runtime_response_route_affinity_log_prefix(context),
        context.reason,
        presence.previous_response_id_present,
        presence.request_turn_state_present,
        presence.request_session_id_present,
        presence.explicit_request_session_id_present,
    )
}

pub fn runtime_response_route_affinity_recompute_result_log_message(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
    presence: RuntimeResponseRouteAffinityPresence,
    affinity: RuntimeResponseRouteAffinityLogState<'_>,
) -> String {
    let prefix = runtime_response_route_affinity_log_prefix(context);
    format!(
        "{prefix} route_affinity_recompute_result reason={} previous_response_id_present={} request_turn_state_present={} request_session_id_present={} explicit_session_id_present={} bound_session_profile={:?} compact_followup_profile={:?} compact_session_profile={:?} session_profile={:?} pinned_profile={:?}",
        context.reason,
        presence.previous_response_id_present,
        presence.request_turn_state_present,
        presence.request_session_id_present,
        presence.explicit_request_session_id_present,
        affinity.bound_session_profile,
        affinity.compact_followup_profile,
        affinity.compact_session_profile,
        affinity.session_profile,
        affinity.pinned_profile,
    )
}

pub fn runtime_response_route_affinity_compact_followup_owner_log_messages(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
    affinity: RuntimeResponseRouteAffinityLogState<'_>,
) -> Vec<String> {
    let prefix = runtime_response_route_affinity_log_prefix(context);
    let mut messages = Vec::new();
    if let Some((profile_name, source)) = affinity.compact_followup_profile {
        messages.push(format!(
            "{prefix} compact_followup_owner profile={profile_name} source={source}"
        ));
    }
    if let Some(profile_name) = affinity.compact_session_profile {
        messages.push(format!(
            "{prefix} compact_followup_owner profile={profile_name} source=session_id"
        ));
    }
    messages
}

#[cfg(test)]
#[path = "../tests/src/route_affinity_log.rs"]
mod tests;
