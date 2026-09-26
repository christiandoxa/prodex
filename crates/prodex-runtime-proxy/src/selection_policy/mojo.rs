use super::*;

unsafe extern "C" {
    #[link_name = "prodex_runtime_websocket_reuse_stale_v1"]
    fn mojo_websocket_reuse_stale(
        nonreplayable_previous_response_reuse: i64,
        reuse_terminal_idle_present: i64,
        reuse_terminal_idle_seconds: u64,
        reuse_terminal_idle_nanoseconds: u64,
        reuse_stale_after_seconds: u64,
        reuse_stale_after_nanoseconds: u64,
    ) -> i64;
}

fn route_kind_tag(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses => 0,
        RuntimeRouteKind::Compact => 1,
        RuntimeRouteKind::Websocket => 2,
        RuntimeRouteKind::Standard => 3,
    }
}

fn plan(
    input: prodex_mojo_core::runtime::AffinitySelectionInput,
) -> prodex_mojo_core::runtime::AffinitySelectionPlan {
    prodex_mojo_core::runtime::affinity_selection_plan(input)
        .expect("Mojo affinity selection planning returned an invalid result")
}

fn affinity_input(
    affinity: RuntimeCandidateAffinity<'_>,
) -> prodex_mojo_core::runtime::AffinitySelectionInput {
    prodex_mojo_core::runtime::AffinitySelectionInput {
        route_kind: route_kind_tag(affinity.route_kind),
        strict_candidate_match: affinity.strict_affinity_profile == Some(affinity.candidate_name),
        pinned_candidate_match: affinity.pinned_profile == Some(affinity.candidate_name),
        turn_state_candidate_match: affinity.turn_state_profile == Some(affinity.candidate_name),
        session_candidate_match: affinity.session_profile == Some(affinity.candidate_name),
        trusted_previous_response_affinity: affinity.trusted_previous_response_affinity,
        ..Default::default()
    }
}

pub(super) fn candidate_no_rotate_affinity(
    affinity: RuntimeCandidateAffinity<'_>,
) -> Option<RuntimeNoRotateAffinity> {
    match plan(affinity_input(affinity)).no_rotate_affinity {
        0 => None,
        1 => Some(RuntimeNoRotateAffinity::Strict),
        2 => Some(RuntimeNoRotateAffinity::TurnState),
        3 => Some(RuntimeNoRotateAffinity::TrustedPreviousResponse),
        4 => Some(RuntimeNoRotateAffinity::CompactSession),
        _ => unreachable!("validated Mojo no-rotate affinity"),
    }
}

pub(super) fn quota_blocked_affinity_release_policy(
    request: RuntimeQuotaBlockedAffinityReleaseRequest<'_>,
) -> RuntimeQuotaBlockedAffinityReleasePolicy {
    let mut input = affinity_input(request.affinity);
    input.fresh_fallback_shape_present = request.fresh_fallback_shape.is_some();
    if plan(input).release_quota_affinity {
        RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity
    } else {
        RuntimeQuotaBlockedAffinityReleasePolicy::KeepAffinity
    }
}

pub(super) fn websocket_previous_response_reuse_is_nonreplayable(
    previous_response_present: bool,
    previous_response_fresh_fallback_used: bool,
    request_turn_state_present: bool,
) -> bool {
    plan(prodex_mojo_core::runtime::AffinitySelectionInput {
        previous_response_present,
        request_turn_state_present,
        previous_response_fresh_fallback_used,
        ..Default::default()
    })
    .reuse_nonreplayable
}

pub(super) fn websocket_previous_response_reuse_is_stale_at(
    nonreplayable_previous_response_reuse: bool,
    reuse_terminal_idle: Option<Duration>,
    stale_after: Duration,
) -> bool {
    // The shared plan bridge only carries millisecond durations; this entrypoint preserves the
    // full precision of Rust's Duration comparison.
    let (reuse_terminal_idle_seconds, reuse_terminal_idle_nanoseconds) = reuse_terminal_idle
        .map(|duration| (duration.as_secs(), u64::from(duration.subsec_nanos())))
        .unwrap_or_default();
    let result = unsafe {
        mojo_websocket_reuse_stale(
            i64::from(nonreplayable_previous_response_reuse),
            i64::from(reuse_terminal_idle.is_some()),
            reuse_terminal_idle_seconds,
            reuse_terminal_idle_nanoseconds,
            stale_after.as_secs(),
            u64::from(stale_after.subsec_nanos()),
        )
    };
    match result {
        0 => false,
        1 => true,
        _ => unreachable!("validated Mojo websocket reuse stale result"),
    }
}

pub(super) fn has_continuation_priority(
    previous_response_present: bool,
    pinned_profile_present: bool,
    request_turn_state_present: bool,
    turn_state_profile_present: bool,
    session_profile_present: bool,
) -> bool {
    plan(prodex_mojo_core::runtime::AffinitySelectionInput {
        previous_response_present,
        pinned_profile_present,
        request_turn_state_present,
        turn_state_profile_present,
        session_profile_present,
        ..Default::default()
    })
    .continuation_priority
}

pub(super) fn wait_affinity_owner<'a>(
    strict_affinity_profile: Option<&'a str>,
    pinned_profile: Option<&'a str>,
    turn_state_profile: Option<&'a str>,
    session_profile: Option<&'a str>,
    trusted_previous_response_affinity: bool,
) -> Option<&'a str> {
    match plan(prodex_mojo_core::runtime::AffinitySelectionInput {
        strict_candidate_match: strict_affinity_profile.is_some(),
        pinned_profile_present: pinned_profile.is_some(),
        turn_state_profile_present: turn_state_profile.is_some(),
        session_profile_present: session_profile.is_some(),
        trusted_previous_response_affinity,
        ..Default::default()
    })
    .wait_owner
    {
        0 => None,
        1 => strict_affinity_profile,
        2 => turn_state_profile,
        3 => pinned_profile,
        4 => session_profile,
        _ => unreachable!("validated Mojo wait-affinity owner"),
    }
}

pub(super) fn noncompact_session_priority_profile<'a>(
    session_profile: Option<&'a str>,
    compact_session_profile: Option<&str>,
) -> Option<&'a str> {
    plan(prodex_mojo_core::runtime::AffinitySelectionInput {
        session_profile_present: session_profile.is_some(),
        compact_session_matches_session: compact_session_profile
            .is_some_and(|profile_name| session_profile == Some(profile_name)),
        ..Default::default()
    })
    .noncompact_session_priority
    .then_some(session_profile)
    .flatten()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn allows_direct_current_profile_fallback(
    previous_response_present: bool,
    pinned_profile_present: bool,
    request_turn_state_present: bool,
    turn_state_profile_present: bool,
    session_profile_present: bool,
    saw_inflight_saturation: bool,
    saw_upstream_failure: bool,
) -> bool {
    plan(prodex_mojo_core::runtime::AffinitySelectionInput {
        previous_response_present,
        pinned_profile_present,
        request_turn_state_present,
        turn_state_profile_present,
        session_profile_present,
        saw_inflight_saturation,
        saw_upstream_failure,
        ..Default::default()
    })
    .direct_current_fallback
}
