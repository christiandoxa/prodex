use super::*;

pub(crate) use runtime_proxy_crate::{
    RuntimeAffinitySelectionKind, RuntimeNoRotateAffinity,
    RuntimePreviousResponseNotFoundFallbackRequest, RuntimePreviousResponseStaleContinuationPolicy,
    RuntimeQuotaBlockedAffinityReleasePolicy,
    RuntimeWebsocketReuseWatchdogPreviousResponseFallback,
    runtime_previous_response_not_found_fallback_policy,
    runtime_quota_blocked_previous_response_fresh_fallback_allowed,
    runtime_websocket_previous_response_not_found_requires_stale_continuation,
    runtime_websocket_previous_response_reuse_is_nonreplayable,
    runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeCandidateAffinity<'a> {
    pub(crate) route_kind: RuntimeRouteKind,
    pub(crate) candidate_name: &'a str,
    pub(crate) strict_affinity_profile: Option<&'a str>,
    pub(crate) pinned_profile: Option<&'a str>,
    pub(crate) turn_state_profile: Option<&'a str>,
    pub(crate) session_profile: Option<&'a str>,
    pub(crate) trusted_previous_response_affinity: bool,
}

impl<'a> RuntimeCandidateAffinity<'a> {
    #[allow(dead_code)]
    pub(crate) fn new(
        route_kind: RuntimeRouteKind,
        candidate_name: &'a str,
        strict_affinity_profile: Option<&'a str>,
        pinned_profile: Option<&'a str>,
        turn_state_profile: Option<&'a str>,
        session_profile: Option<&'a str>,
        trusted_previous_response_affinity: bool,
    ) -> Self {
        Self {
            route_kind,
            candidate_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            trusted_previous_response_affinity,
        }
    }
}

fn runtime_candidate_affinity_to_proxy(
    affinity: RuntimeCandidateAffinity<'_>,
) -> runtime_proxy_crate::RuntimeCandidateAffinity<'_> {
    runtime_proxy_crate::RuntimeCandidateAffinity {
        route_kind: prodex_runtime_quota::runtime_route_kind_to_proxy(affinity.route_kind),
        candidate_name: affinity.candidate_name,
        strict_affinity_profile: affinity.strict_affinity_profile,
        pinned_profile: affinity.pinned_profile,
        turn_state_profile: affinity.turn_state_profile,
        session_profile: affinity.session_profile,
        trusted_previous_response_affinity: affinity.trusted_previous_response_affinity,
    }
}

pub(crate) fn runtime_candidate_has_hard_affinity(affinity: RuntimeCandidateAffinity<'_>) -> bool {
    runtime_proxy_crate::runtime_candidate_has_hard_affinity(runtime_candidate_affinity_to_proxy(
        affinity,
    ))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_candidate_no_rotate_affinity(
    affinity: RuntimeCandidateAffinity<'_>,
) -> Option<RuntimeNoRotateAffinity> {
    runtime_proxy_crate::runtime_candidate_no_rotate_affinity(runtime_candidate_affinity_to_proxy(
        affinity,
    ))
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeQuotaBlockedAffinityReleaseRequest<'a> {
    pub(crate) affinity: RuntimeCandidateAffinity<'a>,
    pub(crate) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_quota_blocked_affinity_release_policy(
    request: RuntimeQuotaBlockedAffinityReleaseRequest<'_>,
) -> RuntimeQuotaBlockedAffinityReleasePolicy {
    runtime_proxy_crate::runtime_quota_blocked_affinity_release_policy(
        runtime_proxy_crate::RuntimeQuotaBlockedAffinityReleaseRequest {
            affinity: runtime_candidate_affinity_to_proxy(request.affinity),
            fresh_fallback_shape: request.fresh_fallback_shape,
        },
    )
}

pub(crate) fn runtime_quota_blocked_affinity_is_releasable(
    affinity: RuntimeCandidateAffinity<'_>,
    _request_requires_previous_response_affinity: bool,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    runtime_proxy_crate::runtime_quota_blocked_affinity_is_releasable(
        runtime_candidate_affinity_to_proxy(affinity),
        fresh_fallback_shape,
    )
}

pub(crate) fn runtime_websocket_previous_response_reuse_is_stale(
    nonreplayable_previous_response_reuse: bool,
    reuse_terminal_idle: Option<Duration>,
    stale_after_ms: u64,
) -> bool {
    runtime_proxy_crate::runtime_websocket_previous_response_reuse_is_stale_at(
        nonreplayable_previous_response_reuse,
        reuse_terminal_idle,
        Duration::from_millis(stale_after_ms),
    )
}

pub(crate) fn runtime_quota_precommit_guard_reason(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<&'static str> {
    prodex_runtime_quota::runtime_quota_precommit_guard_reason(
        summary,
        route_kind,
        runtime_proxy_responses_quota_critical_floor_percent(),
    )
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeResponseCandidateSelection<'a> {
    pub(crate) excluded_profiles: &'a BTreeSet<String>,
    pub(crate) strict_affinity_profile: Option<&'a str>,
    pub(crate) pinned_profile: Option<&'a str>,
    pub(crate) turn_state_profile: Option<&'a str>,
    pub(crate) session_profile: Option<&'a str>,
    pub(crate) prompt_cache_key: Option<&'a str>,
    pub(crate) discover_previous_response_owner: bool,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) route_kind: RuntimeRouteKind,
}

impl<'a> RuntimeResponseCandidateSelection<'a> {
    pub(crate) fn fresh(
        excluded_profiles: &'a BTreeSet<String>,
        route_kind: RuntimeRouteKind,
    ) -> Self {
        Self {
            excluded_profiles,
            strict_affinity_profile: None,
            pinned_profile: None,
            turn_state_profile: None,
            session_profile: None,
            prompt_cache_key: None,
            discover_previous_response_owner: false,
            previous_response_id: None,
            route_kind,
        }
    }
}

pub(super) fn runtime_affinity_selection_profile<'a>(
    affinity_kind: RuntimeAffinitySelectionKind,
    selection: RuntimeResponseCandidateSelection<'a>,
) -> Option<&'a str> {
    match affinity_kind {
        RuntimeAffinitySelectionKind::Strict => selection.strict_affinity_profile,
        RuntimeAffinitySelectionKind::Pinned => selection.pinned_profile,
        RuntimeAffinitySelectionKind::TurnState => selection.turn_state_profile,
        RuntimeAffinitySelectionKind::Session => selection.session_profile,
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RuntimeSoftAffinityPolicyInput {
    pub(super) affinity_kind: RuntimeAffinitySelectionKind,
    pub(super) route_kind: RuntimeRouteKind,
    pub(super) quota_summary: RuntimeQuotaSummary,
    pub(super) quota_source: Option<RuntimeQuotaSource>,
    pub(super) current_profile_matches_candidate: bool,
    pub(super) has_route_eligible_quota_fallback: bool,
}

fn runtime_soft_affinity_input_to_proxy(
    input: RuntimeSoftAffinityPolicyInput,
) -> runtime_proxy_crate::RuntimeSoftAffinityPolicyInput {
    runtime_proxy_crate::RuntimeSoftAffinityPolicyInput {
        affinity_kind: input.affinity_kind,
        route_kind: prodex_runtime_quota::runtime_route_kind_to_proxy(input.route_kind),
        quota_summary: prodex_runtime_quota::runtime_selection_quota_summary_to_proxy(
            input.quota_summary,
        ),
        quota_source: prodex_runtime_quota::runtime_quota_source_option_to_proxy(
            input.quota_source,
        ),
        current_profile_matches_candidate: input.current_profile_matches_candidate,
        has_route_eligible_quota_fallback: input.has_route_eligible_quota_fallback,
        responses_critical_floor_percent: runtime_proxy_responses_quota_critical_floor_percent(),
    }
}

pub(super) fn runtime_soft_affinity_allowed(input: RuntimeSoftAffinityPolicyInput) -> bool {
    runtime_proxy_crate::runtime_soft_affinity_allowed(runtime_soft_affinity_input_to_proxy(input))
}

pub(super) fn runtime_soft_affinity_rejection_reason(
    input: RuntimeSoftAffinityPolicyInput,
) -> &'static str {
    runtime_proxy_crate::runtime_soft_affinity_rejection_reason(
        runtime_soft_affinity_input_to_proxy(input),
    )
}
