use std::time::Duration;

use crate::{
    RuntimePreviousResponseFreshFallbackPolicyInput, RuntimePreviousResponseFreshFallbackShape,
    RuntimeRouteKind, runtime_previous_response_fresh_fallback_policy,
};

#[derive(Clone, Copy, Debug)]
pub struct RuntimeCandidateAffinity<'a> {
    pub route_kind: RuntimeRouteKind,
    pub candidate_name: &'a str,
    pub strict_affinity_profile: Option<&'a str>,
    pub pinned_profile: Option<&'a str>,
    pub turn_state_profile: Option<&'a str>,
    pub session_profile: Option<&'a str>,
    pub trusted_previous_response_affinity: bool,
}

impl<'a> RuntimeCandidateAffinity<'a> {
    pub fn new(
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

pub fn runtime_candidate_has_hard_affinity(affinity: RuntimeCandidateAffinity<'_>) -> bool {
    runtime_candidate_no_rotate_affinity(affinity).is_some()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeNoRotateAffinity {
    Strict,
    TurnState,
    TrustedPreviousResponse,
    CompactSession,
}

pub fn runtime_candidate_no_rotate_affinity(
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
pub enum RuntimeQuotaBlockedAffinityReleasePolicy {
    KeepAffinity,
    ReleaseAffinity,
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeQuotaBlockedAffinityReleaseRequest<'a> {
    pub affinity: RuntimeCandidateAffinity<'a>,
    pub fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

pub fn runtime_quota_blocked_affinity_release_policy(
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

pub fn runtime_quota_blocked_affinity_is_releasable(
    affinity: RuntimeCandidateAffinity<'_>,
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

pub fn runtime_quota_blocked_previous_response_fresh_fallback_allowed(
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

pub fn runtime_websocket_previous_response_reuse_is_nonreplayable(
    previous_response_id: Option<&str>,
    previous_response_fresh_fallback_used: bool,
    turn_state_override: Option<&str>,
) -> bool {
    previous_response_id.is_some()
        && !previous_response_fresh_fallback_used
        && turn_state_override.is_none()
}

pub fn runtime_websocket_previous_response_reuse_is_stale_at(
    nonreplayable_previous_response_reuse: bool,
    reuse_terminal_idle: Option<Duration>,
    stale_after: Duration,
) -> bool {
    nonreplayable_previous_response_reuse
        && reuse_terminal_idle.is_some_and(|elapsed| elapsed >= stale_after)
}

pub fn runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed(
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

#[derive(Clone, Copy)]
pub struct RuntimeWebsocketReuseWatchdogPreviousResponseFallback<'a> {
    pub profile_name: &'a str,
    pub previous_response_id: Option<&'a str>,
    pub previous_response_fresh_fallback_used: bool,
    pub bound_profile: Option<&'a str>,
    pub pinned_profile: Option<&'a str>,
    pub request_requires_previous_response_affinity: bool,
    pub trusted_previous_response_affinity: bool,
    pub request_turn_state: Option<&'a str>,
    pub fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
}

pub fn runtime_websocket_previous_response_not_found_requires_stale_continuation(
    previous_response_id: Option<&str>,
    has_turn_state_retry: bool,
    request_requires_locked_previous_response_affinity: bool,
    fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
) -> bool {
    crate::runtime_previous_response_not_found_fallback_policy(
        crate::RuntimePreviousResponseNotFoundFallbackRequest {
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

pub fn runtime_proxy_has_continuation_priority(
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

pub fn runtime_wait_affinity_owner<'a>(
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

pub fn runtime_noncompact_session_priority_profile<'a>(
    session_profile: Option<&'a str>,
    compact_session_profile: Option<&str>,
) -> Option<&'a str> {
    if compact_session_profile.is_some_and(|profile_name| session_profile == Some(profile_name)) {
        None
    } else {
        session_profile
    }
}

pub fn runtime_proxy_allows_direct_current_profile_fallback(
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSelectionQuotaWindowStatus {
    Ready,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSelectionQuotaWindowSummary {
    pub status: RuntimeSelectionQuotaWindowStatus,
    pub remaining_percent: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSelectionQuotaSummary {
    pub five_hour: RuntimeSelectionQuotaWindowSummary,
    pub weekly: RuntimeSelectionQuotaWindowSummary,
    pub route_band: RuntimeSelectionQuotaPressureBand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum RuntimeSelectionQuotaPressureBand {
    Healthy,
    Thin,
    Critical,
    Exhausted,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSelectionQuotaSource {
    LiveProbe,
    PersistedSnapshot,
}

pub fn runtime_selection_quota_pressure_band_reason(
    band: RuntimeSelectionQuotaPressureBand,
) -> &'static str {
    match band {
        RuntimeSelectionQuotaPressureBand::Healthy => "quota_healthy",
        RuntimeSelectionQuotaPressureBand::Thin => "quota_thin",
        RuntimeSelectionQuotaPressureBand::Critical => "quota_critical",
        RuntimeSelectionQuotaPressureBand::Exhausted => "quota_exhausted",
        RuntimeSelectionQuotaPressureBand::Unknown => "quota_unknown",
    }
}

pub fn runtime_quota_precommit_floor_percent_for_route(
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => {
            responses_critical_floor_percent
        }
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 1,
    }
}

pub fn runtime_quota_window_precommit_guard(
    window: RuntimeSelectionQuotaWindowSummary,
    floor_percent: i64,
) -> bool {
    matches!(
        window.status,
        RuntimeSelectionQuotaWindowStatus::Critical | RuntimeSelectionQuotaWindowStatus::Exhausted
    ) && window.remaining_percent <= floor_percent
}

pub fn runtime_quota_precommit_guard_reason(
    summary: RuntimeSelectionQuotaSummary,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> Option<&'static str> {
    let floor_percent = runtime_quota_precommit_floor_percent_for_route(
        route_kind,
        responses_critical_floor_percent,
    );
    if matches!(
        summary.five_hour.status,
        RuntimeSelectionQuotaWindowStatus::Exhausted
    ) {
        return Some("quota_exhausted_before_send");
    }

    if matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && runtime_quota_window_precommit_guard(summary.five_hour, floor_percent)
    {
        return Some("quota_critical_floor_before_send");
    }

    None
}

pub fn runtime_quota_window_usable_for_auto_rotate(
    status: RuntimeSelectionQuotaWindowStatus,
) -> bool {
    matches!(
        status,
        RuntimeSelectionQuotaWindowStatus::Ready
            | RuntimeSelectionQuotaWindowStatus::Thin
            | RuntimeSelectionQuotaWindowStatus::Critical
    )
}

pub fn runtime_quota_summary_allows_soft_affinity(
    summary: RuntimeSelectionQuotaSummary,
    source: Option<RuntimeSelectionQuotaSource>,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> bool {
    source.is_some()
        && runtime_quota_window_usable_for_auto_rotate(summary.five_hour.status)
        && runtime_quota_window_usable_for_auto_rotate(summary.weekly.status)
        && runtime_quota_precommit_guard_reason(
            summary,
            route_kind,
            responses_critical_floor_percent,
        )
        .is_none()
}

pub fn runtime_quota_soft_affinity_rejection_reason(
    summary: RuntimeSelectionQuotaSummary,
    source: Option<RuntimeSelectionQuotaSource>,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> &'static str {
    if source.is_none()
        || matches!(
            summary.five_hour.status,
            RuntimeSelectionQuotaWindowStatus::Unknown
        )
        || matches!(
            summary.weekly.status,
            RuntimeSelectionQuotaWindowStatus::Unknown
        )
    {
        "quota_windows_unavailable"
    } else if let Some(reason) =
        runtime_quota_precommit_guard_reason(summary, route_kind, responses_critical_floor_percent)
    {
        reason
    } else if matches!(
        summary.five_hour.status,
        RuntimeSelectionQuotaWindowStatus::Exhausted
    ) || matches!(
        summary.weekly.status,
        RuntimeSelectionQuotaWindowStatus::Exhausted
    ) {
        "quota_exhausted"
    } else {
        runtime_selection_quota_pressure_band_reason(summary.route_band)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAffinitySelectionKind {
    Strict,
    Pinned,
    TurnState,
    Session,
}

impl RuntimeAffinitySelectionKind {
    pub fn skip_label(self) -> &'static str {
        match self {
            Self::Strict => "compact_followup",
            Self::Pinned => "pinned",
            Self::TurnState => "turn_state",
            Self::Session => "session",
        }
    }

    pub fn excluded_is_terminal(self) -> bool {
        matches!(self, Self::Strict)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeSoftAffinityPolicyInput {
    pub affinity_kind: RuntimeAffinitySelectionKind,
    pub route_kind: RuntimeRouteKind,
    pub quota_summary: RuntimeSelectionQuotaSummary,
    pub quota_source: Option<RuntimeSelectionQuotaSource>,
    pub current_profile_matches_candidate: bool,
    pub has_route_eligible_quota_fallback: bool,
    pub responses_critical_floor_percent: i64,
}

pub fn runtime_soft_affinity_allowed(input: RuntimeSoftAffinityPolicyInput) -> bool {
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
                input.responses_critical_floor_percent,
            ) || compact_followup_owner_without_probe
        }
        RuntimeAffinitySelectionKind::Pinned | RuntimeAffinitySelectionKind::TurnState => {
            input.quota_summary.route_band <= RuntimeSelectionQuotaPressureBand::Critical
                && runtime_quota_precommit_guard_reason(
                    input.quota_summary,
                    input.route_kind,
                    input.responses_critical_floor_percent,
                )
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
                input.responses_critical_floor_percent,
            ) || compact_session_owner_without_probe
                || websocket_unknown_current_profile_without_pool_fallback
        }
    }
}

pub fn runtime_soft_affinity_rejection_reason(
    input: RuntimeSoftAffinityPolicyInput,
) -> &'static str {
    match input.affinity_kind {
        RuntimeAffinitySelectionKind::Pinned | RuntimeAffinitySelectionKind::TurnState => {
            runtime_selection_quota_pressure_band_reason(input.quota_summary.route_band)
        }
        RuntimeAffinitySelectionKind::Strict | RuntimeAffinitySelectionKind::Session => {
            runtime_quota_soft_affinity_rejection_reason(
                input.quota_summary,
                input.quota_source,
                input.route_kind,
                input.responses_critical_floor_percent,
            )
        }
    }
}

#[cfg(test)]
#[path = "../tests/src/selection_policy.rs"]
mod tests;
