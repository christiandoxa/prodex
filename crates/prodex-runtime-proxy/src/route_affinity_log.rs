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

fn route_affinity_log_render_input<'a>(
    context: RuntimeResponseRouteAffinityLogContext<'a>,
    presence: RuntimeResponseRouteAffinityPresence,
    affinity: RuntimeResponseRouteAffinityLogState<'a>,
    debug_values: [&'a str; 5],
) -> prodex_mojo_core::log::RouteAffinityLogRenderInput<'a> {
    prodex_mojo_core::log::RouteAffinityLogRenderInput {
        request_id: context.request_id,
        websocket_session: context.websocket_session_id,
        reason: context.reason,
        previous_response_id_present: presence.previous_response_id_present,
        request_turn_state_present: presence.request_turn_state_present,
        request_session_id_present: presence.request_session_id_present,
        explicit_request_session_id_present: presence.explicit_request_session_id_present,
        bound_session_profile_debug: debug_values[0],
        compact_followup_profile_debug: debug_values[1],
        compact_session_profile_debug: debug_values[2],
        session_profile_debug: debug_values[3],
        pinned_profile_debug: debug_values[4],
        compact_followup_profile: affinity.compact_followup_profile,
        compact_session_profile: affinity.compact_session_profile,
    }
}

pub fn runtime_response_route_affinity_log_prefix(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
) -> String {
    prodex_mojo_core::log::render_route_affinity_log(
        prodex_mojo_core::log::ROUTE_AFFINITY_LOG_PREFIX,
        route_affinity_log_render_input(
            context,
            RuntimeResponseRouteAffinityPresence::default(),
            RuntimeResponseRouteAffinityLogState::default(),
            [""; 5],
        ),
    )
    .expect("Mojo route-affinity log prefix renderer returned invalid output")
}

pub fn runtime_response_route_affinity_recompute_log_message(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
    presence: RuntimeResponseRouteAffinityPresence,
) -> String {
    prodex_mojo_core::log::render_route_affinity_log(
        prodex_mojo_core::log::ROUTE_AFFINITY_LOG_RECOMPUTE,
        route_affinity_log_render_input(
            context,
            presence,
            RuntimeResponseRouteAffinityLogState::default(),
            [""; 5],
        ),
    )
    .expect("Mojo route-affinity recompute renderer returned invalid output")
}

pub fn runtime_response_route_affinity_recompute_result_log_message(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
    presence: RuntimeResponseRouteAffinityPresence,
    affinity: RuntimeResponseRouteAffinityLogState<'_>,
) -> String {
    let bound_session_profile_debug = format!("{:?}", affinity.bound_session_profile);
    let compact_followup_profile_debug = format!("{:?}", affinity.compact_followup_profile);
    let compact_session_profile_debug = format!("{:?}", affinity.compact_session_profile);
    let session_profile_debug = format!("{:?}", affinity.session_profile);
    let pinned_profile_debug = format!("{:?}", affinity.pinned_profile);
    prodex_mojo_core::log::render_route_affinity_log(
        prodex_mojo_core::log::ROUTE_AFFINITY_LOG_RESULT,
        route_affinity_log_render_input(
            context,
            presence,
            affinity,
            [
                &bound_session_profile_debug,
                &compact_followup_profile_debug,
                &compact_session_profile_debug,
                &session_profile_debug,
                &pinned_profile_debug,
            ],
        ),
    )
    .expect("Mojo route-affinity result renderer returned invalid output")
}

pub fn runtime_response_route_affinity_compact_followup_owner_log_messages(
    context: RuntimeResponseRouteAffinityLogContext<'_>,
    affinity: RuntimeResponseRouteAffinityLogState<'_>,
) -> Vec<String> {
    prodex_mojo_core::log::render_route_affinity_owner_logs(route_affinity_log_render_input(
        context,
        RuntimeResponseRouteAffinityPresence::default(),
        affinity,
        [""; 5],
    ))
    .expect("Mojo compact-followup owner renderer returned invalid output")
}

#[cfg(test)]
#[path = "../tests/src/route_affinity_log.rs"]
mod tests;
