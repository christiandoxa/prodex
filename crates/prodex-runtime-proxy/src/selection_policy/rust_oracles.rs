#[cfg(not(feature = "mojo"))]
mod rust_compat {
    use super::super::*;

    pub(crate) fn candidate_no_rotate_affinity(
        affinity: RuntimeCandidateAffinity<'_>,
    ) -> Option<RuntimeNoRotateAffinity> {
        if affinity.strict_affinity_profile == Some(affinity.candidate_name) {
            return Some(RuntimeNoRotateAffinity::Strict);
        }
        if affinity.turn_state_profile == Some(affinity.candidate_name) {
            return Some(RuntimeNoRotateAffinity::TurnState);
        }
        if affinity.trusted_previous_response_affinity
            && affinity.pinned_profile == Some(affinity.candidate_name)
        {
            return Some(RuntimeNoRotateAffinity::TrustedPreviousResponse);
        }
        if affinity.route_kind == RuntimeRouteKind::Compact
            && affinity.session_profile == Some(affinity.candidate_name)
        {
            return Some(RuntimeNoRotateAffinity::CompactSession);
        }
        None
    }

    pub(crate) fn quota_blocked_affinity_release_policy(
        request: RuntimeQuotaBlockedAffinityReleaseRequest<'_>,
    ) -> RuntimeQuotaBlockedAffinityReleasePolicy {
        if runtime_previous_response_fresh_fallback_policy(
            RuntimePreviousResponseFreshFallbackPolicyInput {
                has_previous_response_context: request.fresh_fallback_shape.is_some(),
                request_requires_locked_previous_response_affinity: false,
                fresh_fallback_shape: request.fresh_fallback_shape,
            },
        )
        .is_fail_closed()
            || request.affinity.strict_affinity_profile == Some(request.affinity.candidate_name)
            || request.affinity.turn_state_profile == Some(request.affinity.candidate_name)
            || (request.affinity.route_kind == RuntimeRouteKind::Compact
                && request.affinity.session_profile == Some(request.affinity.candidate_name))
        {
            RuntimeQuotaBlockedAffinityReleasePolicy::KeepAffinity
        } else {
            RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity
        }
    }

    pub(crate) fn websocket_previous_response_reuse_is_nonreplayable(
        previous_response_id: Option<&str>,
        previous_response_fresh_fallback_used: bool,
        turn_state_override: Option<&str>,
    ) -> bool {
        previous_response_id.is_some()
            && !previous_response_fresh_fallback_used
            && turn_state_override.is_none()
    }

    pub(crate) fn websocket_previous_response_reuse_is_stale_at(
        nonreplayable_previous_response_reuse: bool,
        reuse_terminal_idle: Option<Duration>,
        stale_after: Duration,
    ) -> bool {
        nonreplayable_previous_response_reuse
            && reuse_terminal_idle.is_some_and(|elapsed| elapsed >= stale_after)
    }

    pub(crate) fn has_continuation_priority(
        previous_response_id: Option<&str>,
        pinned_profile: Option<&str>,
        request_turn_state: Option<&str>,
        turn_state_profile: Option<&str>,
        session_profile: Option<&str>,
    ) -> bool {
        previous_response_id.is_some()
            || pinned_profile.is_some()
            || request_turn_state.is_some()
            || turn_state_profile.is_some()
            || session_profile.is_some()
    }

    pub(crate) fn wait_affinity_owner<'a>(
        strict_affinity_profile: Option<&'a str>,
        pinned_profile: Option<&'a str>,
        turn_state_profile: Option<&'a str>,
        session_profile: Option<&'a str>,
        trusted_previous_response_affinity: bool,
    ) -> Option<&'a str> {
        strict_affinity_profile
            .or(turn_state_profile)
            .or_else(|| {
                trusted_previous_response_affinity
                    .then_some(pinned_profile)
                    .flatten()
            })
            .or(session_profile)
    }

    pub(crate) fn noncompact_session_priority_profile<'a>(
        session_profile: Option<&'a str>,
        compact_session_profile: Option<&str>,
    ) -> Option<&'a str> {
        if compact_session_profile.is_some_and(|profile_name| session_profile == Some(profile_name))
        {
            None
        } else {
            session_profile
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn allows_direct_current_profile_fallback(
        previous_response_id: Option<&str>,
        pinned_profile: Option<&str>,
        request_turn_state: Option<&str>,
        turn_state_profile: Option<&str>,
        session_profile: Option<&str>,
        saw_inflight_saturation: bool,
        saw_upstream_failure: bool,
    ) -> bool {
        previous_response_id.is_none()
            && pinned_profile.is_none()
            && request_turn_state.is_none()
            && turn_state_profile.is_none()
            && session_profile.is_none()
            && !saw_inflight_saturation
            && !saw_upstream_failure
    }
}

#[cfg(not(feature = "mojo"))]
pub(crate) use rust_compat::*;
