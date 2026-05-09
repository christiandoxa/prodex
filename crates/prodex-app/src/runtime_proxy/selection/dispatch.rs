use super::*;

#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn select_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    strict_affinity_profile: Option<&str>,
    pinned_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    discover_previous_response_owner: bool,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    select_runtime_response_candidate_for_route_with_selection(
        shared,
        RuntimeResponseCandidateSelection {
            excluded_profiles,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            prompt_cache_key: None,
            discover_previous_response_owner,
            previous_response_id,
            route_kind,
        },
    )
}

pub(crate) fn select_runtime_response_candidate_for_route_with_selection(
    shared: &RuntimeRotationProxyShared,
    selection: RuntimeResponseCandidateSelection<'_>,
) -> Result<Option<String>> {
    for affinity_kind in [
        RuntimeAffinitySelectionKind::Strict,
        RuntimeAffinitySelectionKind::Pinned,
        RuntimeAffinitySelectionKind::TurnState,
    ] {
        match runtime_affinity_selection_decision(shared, selection, affinity_kind)? {
            RuntimeAffinitySelectionDecision::Selected(profile_name) => {
                return Ok(Some(profile_name));
            }
            RuntimeAffinitySelectionDecision::Continue => {}
            RuntimeAffinitySelectionDecision::Exhausted => return Ok(None),
        }
    }

    if selection.discover_previous_response_owner {
        return next_runtime_previous_response_candidate(
            shared,
            selection.excluded_profiles,
            selection.previous_response_id,
            selection.route_kind,
        );
    }

    match runtime_affinity_selection_decision(
        shared,
        selection,
        RuntimeAffinitySelectionKind::Session,
    )? {
        RuntimeAffinitySelectionDecision::Selected(profile_name) => return Ok(Some(profile_name)),
        RuntimeAffinitySelectionDecision::Continue => {}
        RuntimeAffinitySelectionDecision::Exhausted => return Ok(None),
    }

    if let Some(profile_name) = runtime_proxy_optimistic_current_candidate_for_route_with_selection(
        shared,
        selection.excluded_profiles,
        selection.route_kind,
        selection.prompt_cache_key,
    )? {
        return Ok(Some(profile_name));
    }

    next_runtime_response_candidate_for_route_with_prompt_cache_key(
        shared,
        selection.excluded_profiles,
        selection.route_kind,
        selection.prompt_cache_key,
    )
}
