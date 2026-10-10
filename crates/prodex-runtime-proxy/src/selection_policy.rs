use std::time::Duration;

use crate::{
    RuntimePreviousResponseFreshFallbackPolicyInput, RuntimePreviousResponseFreshFallbackShape,
    RuntimeRouteKind, runtime_previous_response_fresh_fallback_policy,
};

mod mojo;

pub use prodex_mojo_core::runtime::{
    WebsocketChainReuseReason as RuntimeWebsocketChainReuseReason,
    WebsocketFailureAction as RuntimeWebsocketFailureAction,
    WebsocketFailureClass as RuntimeWebsocketFailureClass,
    WebsocketFailureDecisionInput as RuntimeWebsocketFailureDecisionInput,
    WebsocketFailureDecisionPlan as RuntimeWebsocketFailureDecisionPlan,
    WebsocketFailureDispositionPlan as RuntimeWebsocketFailureDispositionPlan,
    WebsocketFailureKind as RuntimeWebsocketFailureKind,
    WebsocketFailureStatePlan as RuntimeWebsocketFailureStatePlan,
    WebsocketInvalidPreviousResponseAction as RuntimeWebsocketInvalidPreviousResponseAction,
    WebsocketInvalidPreviousResponsePlan as RuntimeWebsocketInvalidPreviousResponsePlan,
    WebsocketQuotaFallbackPlan as RuntimeWebsocketQuotaFallbackPlan,
    WebsocketTransportFailurePlan as RuntimeWebsocketTransportFailurePlan,
};

pub fn runtime_websocket_failure_decision(
    input: RuntimeWebsocketFailureDecisionInput,
) -> Result<RuntimeWebsocketFailureDecisionPlan, prodex_mojo_core::MojoError> {
    prodex_mojo_core::runtime::websocket_failure_decision(input)
}

pub fn runtime_websocket_failure_frame_classification(
    http_class: i64,
    http_action: i64,
    connection_limit: bool,
    previous_response_not_found: bool,
    stream_committed: bool,
) -> Result<i64, prodex_mojo_core::MojoError> {
    prodex_mojo_core::runtime::websocket_failure_frame_classification(
        http_class,
        http_action,
        connection_limit,
        previous_response_not_found,
        stream_committed,
    )
}

pub fn runtime_websocket_invalid_previous_response_recovery_plan(
    previous_response_present: bool,
    session_present: bool,
    owner_matches: bool,
    owner_generation_present: bool,
    owner_generation_matches: bool,
    recovery_available: bool,
    stream_committed: bool,
) -> Result<RuntimeWebsocketInvalidPreviousResponsePlan, prodex_mojo_core::MojoError> {
    prodex_mojo_core::runtime::websocket_invalid_previous_response_recovery_plan(
        previous_response_present,
        session_present,
        owner_matches,
        owner_generation_present,
        owner_generation_matches,
        recovery_available,
        stream_committed,
    )
}

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

/// Reports conflicting hard binding owners before runtime candidate selection.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeHardBindingConflictInput<'a> {
    pub route_kind: RuntimeRouteKind,
    pub strict_affinity_profile: Option<&'a str>,
    pub pinned_profile: Option<&'a str>,
    pub turn_state_profile: Option<&'a str>,
    pub session_profile: Option<&'a str>,
}

pub fn runtime_hard_binding_conflict(
    input: RuntimeHardBindingConflictInput<'_>,
) -> Result<bool, prodex_mojo_core::MojoError> {
    prodex_mojo_core::runtime::affinity_binding_conflict(
        prodex_mojo_core::runtime::AffinityBindingConflictInput {
            route_kind: input.route_kind as i64,
            strict_affinity_profile: input.strict_affinity_profile,
            pinned_profile: input.pinned_profile,
            turn_state_profile: input.turn_state_profile,
            session_profile: input.session_profile,
            conflict_profile: prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE,
        },
    )
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
    mojo::candidate_no_rotate_affinity(affinity)
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
    mojo::quota_blocked_affinity_release_policy(request)
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
    mojo::websocket_previous_response_reuse_is_nonreplayable(
        previous_response_id.is_some(),
        previous_response_fresh_fallback_used,
        turn_state_override.is_some(),
    )
}

pub fn runtime_websocket_previous_response_reuse_is_stale_at(
    nonreplayable_previous_response_reuse: bool,
    reuse_terminal_idle: Option<Duration>,
    stale_after: Duration,
) -> bool {
    mojo::websocket_previous_response_reuse_is_stale_at(
        nonreplayable_previous_response_reuse,
        reuse_terminal_idle,
        stale_after,
    )
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
}

pub fn runtime_proxy_has_continuation_priority(
    previous_response_id: Option<&str>,
    pinned_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
) -> bool {
    mojo::has_continuation_priority(
        previous_response_id.is_some(),
        pinned_profile.is_some(),
        request_turn_state.is_some(),
        turn_state_profile.is_some(),
        session_profile.is_some(),
    )
}

pub fn runtime_wait_affinity_owner<'a>(
    strict_affinity_profile: Option<&'a str>,
    pinned_profile: Option<&'a str>,
    turn_state_profile: Option<&'a str>,
    session_profile: Option<&'a str>,
    trusted_previous_response_affinity: bool,
) -> Option<&'a str> {
    mojo::wait_affinity_owner(
        strict_affinity_profile,
        pinned_profile,
        turn_state_profile,
        session_profile,
        trusted_previous_response_affinity,
    )
}

pub fn runtime_noncompact_session_priority_profile<'a>(
    session_profile: Option<&'a str>,
    compact_session_profile: Option<&str>,
) -> Option<&'a str> {
    mojo::noncompact_session_priority_profile(session_profile, compact_session_profile)
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
    mojo::allows_direct_current_profile_fallback(
        previous_response_id.is_some(),
        pinned_profile.is_some(),
        request_turn_state.is_some(),
        turn_state_profile.is_some(),
        session_profile.is_some(),
        saw_inflight_saturation,
        saw_upstream_failure,
    )
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSelectionQuotaWindowStatus {
    Ready = 0,
    Thin = 1,
    Critical = 2,
    Exhausted = 3,
    Unknown = 4,
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

#[repr(i64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum RuntimeSelectionQuotaPressureBand {
    Healthy = 0,
    Thin = 1,
    Critical = 2,
    Exhausted = 3,
    Unknown = 4,
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSelectionQuotaSource {
    LiveProbe,
    PersistedSnapshot,
}

#[allow(clippy::too_many_arguments)]
fn runtime_quota_selection_policy_code(
    mode: i64,
    five_hour_status: RuntimeSelectionQuotaWindowStatus,
    weekly_status: RuntimeSelectionQuotaWindowStatus,
    band: RuntimeSelectionQuotaPressureBand,
    source_present: bool,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> i64 {
    prodex_mojo_core::runtime::quota_selection_policy(
        mode,
        prodex_mojo_core::runtime::QuotaSelectionPolicyInput {
            route_kind: route_kind as i64,
            five_hour_status: five_hour_status as i64,
            weekly_status: weekly_status as i64,
            quota_band: band as i64,
            quota_source_present: source_present,
            responses_critical_floor_percent,
        },
    )
    .expect("Mojo quota-selection policy returned an invalid result")
}

fn runtime_quota_summary_policy_code(
    mode: i64,
    summary: RuntimeSelectionQuotaSummary,
    source: Option<RuntimeSelectionQuotaSource>,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> i64 {
    runtime_quota_selection_policy_code(
        mode,
        summary.five_hour.status,
        summary.weekly.status,
        summary.route_band,
        source.is_some(),
        route_kind,
        responses_critical_floor_percent,
    )
}

fn runtime_quota_policy_reason(code: i64) -> Option<&'static str> {
    let label = prodex_mojo_core::observability::runtime_soft_affinity_policy_reason_label(code)
        .expect("Mojo soft-affinity reason label returned invalid output");
    (!label.is_empty()).then_some(label)
}

pub fn runtime_selection_quota_pressure_band_reason(
    band: RuntimeSelectionQuotaPressureBand,
) -> &'static str {
    let code = runtime_quota_selection_policy_code(
        prodex_mojo_core::runtime::QUOTA_SELECTION_MODE_BAND_REASON,
        RuntimeSelectionQuotaWindowStatus::Ready,
        RuntimeSelectionQuotaWindowStatus::Ready,
        band,
        false,
        RuntimeRouteKind::Responses,
        0,
    );
    runtime_quota_policy_reason(code).unwrap_or("quota_unknown")
}

pub fn runtime_quota_precommit_floor_percent_for_route(
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> i64 {
    runtime_quota_selection_policy_code(
        prodex_mojo_core::runtime::QUOTA_SELECTION_MODE_PRECOMMIT_FLOOR,
        RuntimeSelectionQuotaWindowStatus::Ready,
        RuntimeSelectionQuotaWindowStatus::Ready,
        RuntimeSelectionQuotaPressureBand::Healthy,
        false,
        route_kind,
        responses_critical_floor_percent,
    )
}

pub fn runtime_quota_window_precommit_guard(
    window: RuntimeSelectionQuotaWindowSummary,
    floor_percent: i64,
) -> bool {
    runtime_quota_selection_policy_code(
        prodex_mojo_core::runtime::QUOTA_SELECTION_MODE_WINDOW_GUARD,
        window.status,
        RuntimeSelectionQuotaWindowStatus::Ready,
        RuntimeSelectionQuotaPressureBand::Healthy,
        false,
        RuntimeRouteKind::Responses,
        floor_percent,
    ) == 1
}

pub fn runtime_quota_precommit_guard_reason(
    summary: RuntimeSelectionQuotaSummary,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> Option<&'static str> {
    runtime_quota_policy_reason(runtime_quota_summary_policy_code(
        prodex_mojo_core::runtime::QUOTA_SELECTION_MODE_PRECOMMIT_REASON,
        summary,
        None,
        route_kind,
        responses_critical_floor_percent,
    ))
}

pub fn runtime_quota_window_usable_for_auto_rotate(
    status: RuntimeSelectionQuotaWindowStatus,
) -> bool {
    runtime_quota_selection_policy_code(
        prodex_mojo_core::runtime::QUOTA_SELECTION_MODE_WINDOW_USABLE,
        status,
        RuntimeSelectionQuotaWindowStatus::Ready,
        RuntimeSelectionQuotaPressureBand::Healthy,
        false,
        RuntimeRouteKind::Responses,
        0,
    ) == 1
}

pub fn runtime_quota_summary_allows_soft_affinity(
    summary: RuntimeSelectionQuotaSummary,
    source: Option<RuntimeSelectionQuotaSource>,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> bool {
    runtime_quota_summary_policy_code(
        prodex_mojo_core::runtime::QUOTA_SELECTION_MODE_SUMMARY_ALLOWS,
        summary,
        source,
        route_kind,
        responses_critical_floor_percent,
    ) == 1
}

pub fn runtime_quota_soft_affinity_rejection_reason(
    summary: RuntimeSelectionQuotaSummary,
    source: Option<RuntimeSelectionQuotaSource>,
    route_kind: RuntimeRouteKind,
    responses_critical_floor_percent: i64,
) -> &'static str {
    runtime_quota_policy_reason(runtime_quota_summary_policy_code(
        prodex_mojo_core::runtime::QUOTA_SELECTION_MODE_REJECTION_REASON,
        summary,
        source,
        route_kind,
        responses_critical_floor_percent,
    ))
    .unwrap_or("quota_unknown")
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAffinityLocalRejection {
    None,
    SelectionBackoff,
    RouteCircuitHalfOpenProbeWait,
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeAffinityOutcomeInput {
    pub hard_binding_conflict: bool,
    pub exact_binding_mismatch: bool,
    pub profile_usable: bool,
    pub excluded: bool,
    pub hard_affinity: bool,
    pub soft_policy_allowed: bool,
    pub local_rejection: RuntimeAffinityLocalRejection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAffinityOutcome {
    Unavailable { reason: &'static str, hard: bool },
    SelectHard,
    SelectSoft,
    RejectSoftQuota,
}

pub fn runtime_affinity_outcome(input: RuntimeAffinityOutcomeInput) -> RuntimeAffinityOutcome {
    runtime_affinity_outcome_checked(input)
        .expect("Mojo affinity outcome planning returned an invalid result")
}

pub fn runtime_affinity_outcome_checked(
    input: RuntimeAffinityOutcomeInput,
) -> Result<RuntimeAffinityOutcome, prodex_mojo_core::MojoError> {
    let local_rejection = input.local_rejection as i64;
    let plan = prodex_mojo_core::runtime::affinity_outcome_plan(
        prodex_mojo_core::runtime::AffinityOutcomeInput {
            hard_binding_conflict: input.hard_binding_conflict,
            exact_binding_mismatch: input.exact_binding_mismatch,
            profile_usable: input.profile_usable,
            excluded: input.excluded,
            hard_affinity: input.hard_affinity,
            soft_policy_allowed: input.soft_policy_allowed,
            local_rejection,
        },
    )?;
    match plan.action {
        0 => Ok(RuntimeAffinityOutcome::Unavailable {
            reason: prodex_mojo_core::observability::runtime_affinity_unavailable_reason_label(
                plan.reason,
            )?,
            hard: plan.unavailable_hard,
        }),
        1 => Ok(RuntimeAffinityOutcome::SelectHard),
        2 => Ok(RuntimeAffinityOutcome::SelectSoft),
        3 => Ok(RuntimeAffinityOutcome::RejectSoftQuota),
        _ => Err(prodex_mojo_core::MojoError::InvalidOutput),
    }
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAffinitySelectionKind {
    Strict,
    Pinned,
    TurnState,
    Session,
}

impl RuntimeAffinitySelectionKind {
    pub fn skip_label(self) -> &'static str {
        prodex_mojo_core::observability::runtime_affinity_selection_kind_label(self as i64)
            .expect("Mojo affinity selection-kind label returned invalid output")
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
    runtime_soft_affinity_allowed_checked(input)
        .expect("Mojo soft affinity policy returned an invalid result")
}

pub fn runtime_soft_affinity_allowed_checked(
    input: RuntimeSoftAffinityPolicyInput,
) -> Result<bool, prodex_mojo_core::MojoError> {
    let result = prodex_mojo_core::runtime::soft_affinity_policy(
        prodex_mojo_core::runtime::SoftAffinityPolicyInput {
            affinity_kind: input.affinity_kind as i64,
            route_kind: input.route_kind as i64,
            five_hour_status: input.quota_summary.five_hour.status as i64,
            weekly_status: input.quota_summary.weekly.status as i64,
            quota_band: input.quota_summary.route_band as i64,
            quota_source_present: input.quota_source.is_some(),
            current_profile_matches_candidate: input.current_profile_matches_candidate,
            has_route_eligible_quota_fallback: input.has_route_eligible_quota_fallback,
        },
    )?;
    match result {
        prodex_mojo_core::runtime::SOFT_AFFINITY_POLICY_ALLOWED => Ok(true),
        prodex_mojo_core::runtime::SOFT_AFFINITY_POLICY_QUOTA_WINDOWS_UNAVAILABLE
        | prodex_mojo_core::runtime::SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED_BEFORE_SEND
        | prodex_mojo_core::runtime::SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED
        | prodex_mojo_core::runtime::SOFT_AFFINITY_POLICY_QUOTA_HEALTHY
        | prodex_mojo_core::runtime::SOFT_AFFINITY_POLICY_QUOTA_THIN
        | prodex_mojo_core::runtime::SOFT_AFFINITY_POLICY_QUOTA_CRITICAL
        | prodex_mojo_core::runtime::SOFT_AFFINITY_POLICY_QUOTA_UNKNOWN => Ok(false),
        _ => Err(prodex_mojo_core::MojoError::InvalidOutput),
    }
}

pub fn runtime_soft_affinity_rejection_reason(
    input: RuntimeSoftAffinityPolicyInput,
) -> &'static str {
    runtime_soft_affinity_rejection_reason_checked(input)
        .expect("Mojo soft affinity reason returned an invalid result")
}

pub fn runtime_soft_affinity_rejection_reason_checked(
    input: RuntimeSoftAffinityPolicyInput,
) -> Result<&'static str, prodex_mojo_core::MojoError> {
    let result = prodex_mojo_core::runtime::soft_affinity_policy(
        prodex_mojo_core::runtime::SoftAffinityPolicyInput {
            affinity_kind: input.affinity_kind as i64,
            route_kind: input.route_kind as i64,
            five_hour_status: input.quota_summary.five_hour.status as i64,
            weekly_status: input.quota_summary.weekly.status as i64,
            quota_band: input.quota_summary.route_band as i64,
            quota_source_present: input.quota_source.is_some(),
            current_profile_matches_candidate: input.current_profile_matches_candidate,
            has_route_eligible_quota_fallback: input.has_route_eligible_quota_fallback,
        },
    )?;
    runtime_quota_policy_reason(result).ok_or(prodex_mojo_core::MojoError::InvalidOutput)
}

pub fn runtime_websocket_transport_failure_plan(
    committed: bool,
    reuse_existing_session: bool,
    precommit_transport_retry_allowed: bool,
) -> RuntimeWebsocketTransportFailurePlan {
    prodex_mojo_core::runtime::websocket_transport_failure_plan(
        committed,
        reuse_existing_session,
        precommit_transport_retry_allowed,
    )
    .expect("Mojo websocket transport-failure planner returned an invalid result")
}

pub fn runtime_websocket_failure_disposition(
    affinity_releasable: bool,
    inflight_saturated: bool,
) -> RuntimeWebsocketFailureDispositionPlan {
    prodex_mojo_core::runtime::websocket_failure_disposition_plan(
        affinity_releasable,
        inflight_saturated,
    )
    .expect("Mojo websocket failure disposition returned an invalid result")
}

pub fn runtime_websocket_failure_state_plan(
    failure_kind: RuntimeWebsocketFailureKind,
    affinity_releasable: bool,
) -> RuntimeWebsocketFailureStatePlan {
    prodex_mojo_core::runtime::websocket_failure_state_plan(failure_kind, affinity_releasable)
        .expect("Mojo websocket failure state plan returned an invalid result")
}

pub fn runtime_websocket_invalid_previous_response_plan(
    previous_response_present: bool,
    session_present: bool,
    owner_matches: bool,
    owner_generation_present: bool,
    owner_generation_matches: bool,
) -> RuntimeWebsocketInvalidPreviousResponsePlan {
    prodex_mojo_core::runtime::websocket_invalid_previous_response_plan(
        previous_response_present,
        session_present,
        owner_matches,
        owner_generation_present,
        owner_generation_matches,
    )
    .expect("Mojo websocket invalid-previous-response policy returned invalid output")
}

pub fn runtime_full_context_retry_signal_eligible(
    previous_response_present: bool,
    session_present: bool,
    owner_matches: bool,
) -> bool {
    prodex_mojo_core::runtime::websocket_full_context_signal_eligible(
        previous_response_present,
        session_present,
        owner_matches,
    )
    .expect("Mojo full-context retry policy returned an invalid result")
}

pub fn runtime_websocket_full_context_signal_eligible(
    previous_response_present: bool,
    session_present: bool,
    owner_matches: bool,
) -> bool {
    runtime_full_context_retry_signal_eligible(
        previous_response_present,
        session_present,
        owner_matches,
    )
}

pub fn runtime_websocket_quota_fallback_plan(
    route_eligible_fallback: bool,
    has_context_constraint: bool,
) -> RuntimeWebsocketQuotaFallbackPlan {
    prodex_mojo_core::runtime::websocket_quota_fallback_plan(
        route_eligible_fallback,
        has_context_constraint,
    )
    .expect("Mojo websocket quota-fallback policy returned an invalid result")
}

#[cfg(test)]
#[path = "../tests/src/selection_policy.rs"]
mod tests;
