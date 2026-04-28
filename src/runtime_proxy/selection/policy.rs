use super::*;

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

pub(crate) fn runtime_candidate_has_hard_affinity(affinity: RuntimeCandidateAffinity<'_>) -> bool {
    runtime_candidate_no_rotate_affinity(affinity).is_some()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeNoRotateAffinity {
    Strict,
    TurnState,
    TrustedPreviousResponse,
    CompactSession,
}

pub(crate) fn runtime_candidate_no_rotate_affinity(
    affinity: RuntimeCandidateAffinity<'_>,
) -> Option<RuntimeNoRotateAffinity> {
    if affinity
        .strict_affinity_profile
        .is_some_and(|profile_name| profile_name == affinity.candidate_name)
    {
        return Some(RuntimeNoRotateAffinity::Strict);
    }
    if affinity
        .turn_state_profile
        .is_some_and(|profile_name| profile_name == affinity.candidate_name)
    {
        return Some(RuntimeNoRotateAffinity::TurnState);
    }
    if affinity.trusted_previous_response_affinity
        && affinity
            .pinned_profile
            .is_some_and(|profile_name| profile_name == affinity.candidate_name)
    {
        return Some(RuntimeNoRotateAffinity::TrustedPreviousResponse);
    }
    if affinity.route_kind == RuntimeRouteKind::Compact
        && affinity
            .session_profile
            .is_some_and(|profile_name| profile_name == affinity.candidate_name)
    {
        return Some(RuntimeNoRotateAffinity::CompactSession);
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeQuotaBlockedAffinityReleasePolicy {
    KeepAffinity,
    ReleaseAffinity,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeQuotaBlockedAffinityReleaseRequest<'a> {
    pub(crate) affinity: RuntimeCandidateAffinity<'a>,
    pub(crate) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

pub(crate) fn runtime_quota_blocked_affinity_release_policy(
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
    {
        // Fail closed: classified previous_response continuations stay chained to the owner.
        return RuntimeQuotaBlockedAffinityReleasePolicy::KeepAffinity;
    }

    if request
        .affinity
        .strict_affinity_profile
        .is_some_and(|profile_name| profile_name == request.affinity.candidate_name)
        || request
            .affinity
            .turn_state_profile
            .is_some_and(|profile_name| profile_name == request.affinity.candidate_name)
        || (request.affinity.route_kind == RuntimeRouteKind::Compact
            && request
                .affinity
                .session_profile
                .is_some_and(|profile_name| profile_name == request.affinity.candidate_name))
    {
        return RuntimeQuotaBlockedAffinityReleasePolicy::KeepAffinity;
    }

    RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity
}

pub(crate) fn runtime_quota_blocked_affinity_is_releasable(
    affinity: RuntimeCandidateAffinity<'_>,
    _request_requires_previous_response_affinity: bool,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    matches!(
        runtime_quota_blocked_affinity_release_policy(RuntimeQuotaBlockedAffinityReleaseRequest {
            affinity,
            fresh_fallback_shape,
        }),
        RuntimeQuotaBlockedAffinityReleasePolicy::ReleaseAffinity
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimePreviousResponseNotFoundFallbackPolicy {
    pub(crate) stale_continuation: RuntimePreviousResponseStaleContinuationPolicy,
    pub(crate) fresh_fallback: RuntimePreviousResponseFreshFallbackPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimePreviousResponseStaleContinuationPolicy {
    NotApplicable,
    RetryWithTurnState,
    FailClosed,
}

impl RuntimePreviousResponseStaleContinuationPolicy {
    pub(crate) fn requires_stale_continuation(self) -> bool {
        matches!(self, Self::FailClosed)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimePreviousResponseNotFoundFallbackRequest<'a> {
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) has_turn_state_retry: bool,
    pub(crate) request_requires_locked_previous_response_affinity: bool,
    pub(crate) previous_response_fresh_fallback_used: bool,
    pub(crate) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

pub(crate) fn runtime_previous_response_not_found_fallback_policy(
    request: RuntimePreviousResponseNotFoundFallbackRequest<'_>,
) -> RuntimePreviousResponseNotFoundFallbackPolicy {
    let stale_continuation = match (request.previous_response_id, request.has_turn_state_retry) {
        (Some(_), false) => RuntimePreviousResponseStaleContinuationPolicy::FailClosed,
        (Some(_), true) => RuntimePreviousResponseStaleContinuationPolicy::RetryWithTurnState,
        (None, _) => RuntimePreviousResponseStaleContinuationPolicy::NotApplicable,
    };
    let fresh_fallback = runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: request.previous_response_id.is_some()
                || request.previous_response_fresh_fallback_used,
            request_requires_locked_previous_response_affinity: request
                .request_requires_locked_previous_response_affinity,
            fresh_fallback_shape: request.fresh_fallback_shape,
        },
    );

    RuntimePreviousResponseNotFoundFallbackPolicy {
        stale_continuation,
        fresh_fallback,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_quota_blocked_previous_response_fresh_fallback_allowed(
    previous_response_id: Option<&str>,
    trusted_previous_response_affinity: bool,
    previous_response_fresh_fallback_used: bool,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: previous_response_id.is_some()
                || trusted_previous_response_affinity
                || previous_response_fresh_fallback_used,
            request_requires_locked_previous_response_affinity: false,
            fresh_fallback_shape,
        },
    )
    .allows_fresh_fallback()
}

pub(crate) fn runtime_websocket_previous_response_reuse_is_nonreplayable(
    previous_response_id: Option<&str>,
    previous_response_fresh_fallback_used: bool,
    turn_state_override: Option<&str>,
) -> bool {
    previous_response_id.is_some()
        && !previous_response_fresh_fallback_used
        && turn_state_override.is_none()
}

pub(crate) fn runtime_websocket_previous_response_reuse_is_stale(
    nonreplayable_previous_response_reuse: bool,
    reuse_terminal_idle: Option<Duration>,
) -> bool {
    nonreplayable_previous_response_reuse
        && reuse_terminal_idle.is_some_and(|elapsed| {
            elapsed
                >= Duration::from_millis(runtime_proxy_websocket_previous_response_reuse_stale_ms())
        })
}

#[allow(dead_code)]
pub(crate) fn runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed(
    request: RuntimeWebsocketReuseWatchdogPreviousResponseFallback<'_>,
) -> bool {
    let profile_matches_bound_owner = request.bound_profile == Some(request.profile_name);
    let profile_matches_pinned_owner = request.pinned_profile == Some(request.profile_name);

    runtime_previous_response_fresh_fallback_policy(
        RuntimePreviousResponseFreshFallbackPolicyInput {
            has_previous_response_context: request.previous_response_id.is_some()
                || request.previous_response_fresh_fallback_used
                || request.trusted_previous_response_affinity
                || profile_matches_bound_owner
                || profile_matches_pinned_owner
                || request.request_turn_state.is_some(),
            request_requires_locked_previous_response_affinity: request
                .request_requires_previous_response_affinity,
            fresh_fallback_shape: request.fresh_fallback_shape,
        },
    )
    .allows_fresh_fallback()
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(crate) struct RuntimeWebsocketReuseWatchdogPreviousResponseFallback<'a> {
    pub(crate) profile_name: &'a str,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) previous_response_fresh_fallback_used: bool,
    pub(crate) bound_profile: Option<&'a str>,
    pub(crate) pinned_profile: Option<&'a str>,
    pub(crate) request_requires_previous_response_affinity: bool,
    pub(crate) trusted_previous_response_affinity: bool,
    pub(crate) request_turn_state: Option<&'a str>,
    pub(crate) fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn runtime_websocket_previous_response_not_found_requires_stale_continuation(
    previous_response_id: Option<&str>,
    has_turn_state_retry: bool,
    request_requires_locked_previous_response_affinity: bool,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    runtime_previous_response_not_found_fallback_policy(
        RuntimePreviousResponseNotFoundFallbackRequest {
            previous_response_id,
            has_turn_state_retry,
            request_requires_locked_previous_response_affinity,
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape,
        },
    )
    .stale_continuation
    .requires_stale_continuation()
}

pub(crate) fn runtime_quota_precommit_floor_percent(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => {
            runtime_proxy_responses_quota_critical_floor_percent()
        }
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 1,
    }
}

pub(crate) fn runtime_quota_window_precommit_guard(
    window: RuntimeQuotaWindowSummary,
    floor_percent: i64,
) -> bool {
    matches!(
        window.status,
        RuntimeQuotaWindowStatus::Critical | RuntimeQuotaWindowStatus::Exhausted
    ) && window.remaining_percent <= floor_percent
}

pub(crate) fn runtime_quota_precommit_guard_reason(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<&'static str> {
    let floor_percent = runtime_quota_precommit_floor_percent(route_kind);
    if summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        return Some("quota_exhausted_before_send");
    }

    if matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && (runtime_quota_window_precommit_guard(summary.five_hour, floor_percent)
        || runtime_quota_window_precommit_guard(summary.weekly, floor_percent))
    {
        return Some("quota_critical_floor_before_send");
    }

    None
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuntimeResponseCandidateSelection<'a> {
    pub(crate) excluded_profiles: &'a BTreeSet<String>,
    pub(crate) strict_affinity_profile: Option<&'a str>,
    pub(crate) pinned_profile: Option<&'a str>,
    pub(crate) turn_state_profile: Option<&'a str>,
    pub(crate) session_profile: Option<&'a str>,
    pub(crate) discover_previous_response_owner: bool,
    pub(crate) previous_response_id: Option<&'a str>,
    pub(crate) route_kind: RuntimeRouteKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeAffinitySelectionKind {
    Strict,
    Pinned,
    TurnState,
    Session,
}

impl RuntimeAffinitySelectionKind {
    pub(super) fn profile<'a>(
        self,
        selection: RuntimeResponseCandidateSelection<'a>,
    ) -> Option<&'a str> {
        match self {
            Self::Strict => selection.strict_affinity_profile,
            Self::Pinned => selection.pinned_profile,
            Self::TurnState => selection.turn_state_profile,
            Self::Session => selection.session_profile,
        }
    }

    pub(super) fn skip_label(self) -> &'static str {
        match self {
            Self::Strict => "compact_followup",
            Self::Pinned => "pinned",
            Self::TurnState => "turn_state",
            Self::Session => "session",
        }
    }

    pub(super) fn excluded_is_terminal(self) -> bool {
        matches!(self, Self::Strict)
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

pub(super) fn runtime_soft_affinity_allowed(input: RuntimeSoftAffinityPolicyInput) -> bool {
    match input.affinity_kind {
        RuntimeAffinitySelectionKind::Strict => {
            let compact_followup_owner_without_probe = matches!(
                input.route_kind,
                RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
            ) && input.quota_source.is_none();
            runtime_quota_summary_allows_soft_affinity(
                input.quota_summary,
                input.quota_source,
                input.route_kind,
            ) || compact_followup_owner_without_probe
        }
        RuntimeAffinitySelectionKind::Pinned | RuntimeAffinitySelectionKind::TurnState => {
            input.quota_summary.route_band <= RuntimeQuotaPressureBand::Critical
                && runtime_quota_precommit_guard_reason(input.quota_summary, input.route_kind)
                    .is_none()
        }
        RuntimeAffinitySelectionKind::Session => {
            let compact_session_owner_without_probe =
                input.route_kind == RuntimeRouteKind::Compact && input.quota_source.is_none();
            let websocket_unknown_current_profile_without_pool_fallback = input.route_kind
                == RuntimeRouteKind::Websocket
                && input.quota_source.is_none()
                && input.current_profile_matches_candidate
                && !input.has_route_eligible_quota_fallback;
            runtime_quota_summary_allows_soft_affinity(
                input.quota_summary,
                input.quota_source,
                input.route_kind,
            ) || compact_session_owner_without_probe
                || websocket_unknown_current_profile_without_pool_fallback
        }
    }
}

pub(super) fn runtime_soft_affinity_rejection_reason(
    input: RuntimeSoftAffinityPolicyInput,
) -> &'static str {
    match input.affinity_kind {
        RuntimeAffinitySelectionKind::Pinned | RuntimeAffinitySelectionKind::TurnState => {
            runtime_quota_pressure_band_reason(input.quota_summary.route_band)
        }
        RuntimeAffinitySelectionKind::Strict | RuntimeAffinitySelectionKind::Session => {
            runtime_quota_soft_affinity_rejection_reason(
                input.quota_summary,
                input.quota_source,
                input.route_kind,
            )
        }
    }
}
