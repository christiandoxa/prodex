use super::*;

pub(crate) use runtime_proxy_crate::{
    RuntimeAffinitySelectionKind, runtime_websocket_previous_response_reuse_is_nonreplayable,
};
#[cfg(test)]
pub(crate) use runtime_proxy_crate::{
    RuntimePreviousResponseNotFoundFallbackRequest, RuntimePreviousResponseStaleContinuationPolicy,
    runtime_previous_response_not_found_fallback_policy,
    runtime_quota_blocked_previous_response_fresh_fallback_allowed,
    runtime_websocket_previous_response_not_found_requires_stale_continuation,
};
#[cfg(any(test, feature = "bench-support"))]
pub(crate) use runtime_proxy_crate::{
    RuntimeWebsocketReuseWatchdogPreviousResponseFallback,
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

#[cfg(test)]
impl<'a> RuntimeCandidateAffinity<'a> {
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
        route_kind: affinity.route_kind,
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
    pub(crate) requested_model: Option<&'a str>,
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
            requested_model: None,
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
