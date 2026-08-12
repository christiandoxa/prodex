use anyhow::Result;
use chrono::Local;

use crate::{
    RuntimeContinuationBindingKind, RuntimeContinuationBindingLifecycle,
    runtime_continuation_status_map,
};

use super::{
    RuntimeAffinitySelectionKind, RuntimeCandidateAffinity, RuntimeQuotaSource,
    RuntimeQuotaSummary, RuntimeResponseCandidateSelection, RuntimeRotationProxyShared,
    RuntimeRouteKind, prune_runtime_profile_selection_backoff,
    reserve_runtime_profile_route_circuit_half_open_probe, runtime_affinity_selection_profile,
    runtime_candidate_has_hard_affinity, runtime_has_route_eligible_quota_fallback,
    runtime_profile_auth_failure_active_with_auth_cache, runtime_profile_name_in_selection_backoff,
    runtime_profile_quota_summary_for_route, runtime_proxy_current_profile, runtime_proxy_log,
    runtime_proxy_log_field, runtime_proxy_responses_quota_critical_floor_percent,
    runtime_proxy_structured_log_message, runtime_request_hard_binding_owner,
    runtime_route_kind_label, runtime_selection_log_fields_with_quota,
    runtime_selection_quota_source_label, runtime_selection_trace_affinity_kind,
    runtime_selection_trace_candidate, runtime_selection_trace_reject,
};

pub(crate) fn runtime_previous_response_affinity_is_trusted(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
) -> Result<bool> {
    let Some(previous_response_id) = previous_response_id else {
        return Ok(false);
    };
    let Some(bound_profile) = bound_profile else {
        return Ok(false);
    };

    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let identity = prodex_runtime_state::RuntimeHardBindingIdentity::response(previous_response_id)
        .ok_or_else(|| anyhow::anyhow!("runtime hard-binding identity is invalid"))?;
    let now = Local::now().timestamp();
    let owner_is_trusted = matches!(
        prodex_runtime_store::runtime_hard_binding_owner(
            &identity,
            &runtime.state.response_profile_bindings,
            &runtime.turn_state_bindings,
            &runtime.session_id_bindings,
            &runtime.state.session_profile_bindings,
            &runtime.state.profiles,
        ),
        prodex_runtime_state::RuntimeHardBindingOwner::Owned(profile_name)
            if profile_name == bound_profile
                && !runtime_profile_auth_failure_active_with_auth_cache(
                    &runtime.profile_health,
                    &runtime.profile_usage_auth,
                    &profile_name,
                    now,
                )
    );
    if !owner_is_trusted {
        return Ok(false);
    }
    Ok(runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
    )
    .get(previous_response_id)
    .is_none_or(|status| {
        status.state == RuntimeContinuationBindingLifecycle::Verified
            || (status.state == RuntimeContinuationBindingLifecycle::Warm
                && status.last_verified_at.is_some())
    }))
}

pub(crate) fn runtime_previous_response_hard_binding_is_unusable(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
) -> Result<bool> {
    let Some(previous_response_id) = previous_response_id else {
        return Ok(false);
    };
    match runtime_request_hard_binding_owner(shared, Some(previous_response_id), None, None)? {
        prodex_runtime_state::RuntimeHardBindingOwner::Unbound => Ok(false),
        prodex_runtime_state::RuntimeHardBindingOwner::Conflict
        | prodex_runtime_state::RuntimeHardBindingOwner::Unavailable(_) => Ok(true),
        prodex_runtime_state::RuntimeHardBindingOwner::Owned(profile_name) => {
            if !runtime_profile_is_usable_for_hard_binding(shared, &profile_name)? {
                return Ok(true);
            }
            Ok(runtime_profile_has_exact_binding_identity(shared, &profile_name)? == Some(false))
        }
    }
}

fn runtime_previous_response_affinity_is_bound(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
) -> Result<bool> {
    let (Some(previous_response_id), Some(bound_profile)) = (previous_response_id, bound_profile)
    else {
        return Ok(false);
    };
    Ok(matches!(
        runtime_request_hard_binding_owner(shared, Some(previous_response_id), None, None)?,
        prodex_runtime_state::RuntimeHardBindingOwner::Owned(profile_name)
            if profile_name == bound_profile
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RuntimeAffinitySelectionDecision {
    Selected(String),
    Continue,
    Exhausted,
}

pub(super) fn runtime_affinity_selection_decision(
    shared: &RuntimeRotationProxyShared,
    selection: RuntimeResponseCandidateSelection<'_>,
    affinity_kind: RuntimeAffinitySelectionKind,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
) -> Result<RuntimeAffinitySelectionDecision> {
    if runtime_hard_binding_conflict(selection) {
        return Ok(record_runtime_unavailable_affinity(
            trace,
            affinity_kind,
            selection,
            true,
            "hard_binding_conflict",
        ));
    }
    let Some(profile_name) = runtime_affinity_selection_profile(affinity_kind, selection) else {
        return Ok(RuntimeAffinitySelectionDecision::Continue);
    };
    let hard_affinity = runtime_affinity_is_hard(shared, selection, affinity_kind, profile_name)?;
    let exact_binding = runtime_profile_has_exact_binding_identity(shared, profile_name)?;
    if exact_binding == Some(false) {
        return Ok(record_runtime_unavailable_affinity(
            trace,
            affinity_kind,
            selection,
            hard_affinity,
            "binding_identity_mismatch",
        ));
    }
    if !runtime_profile_is_usable_for_hard_binding(shared, profile_name)? {
        return Ok(record_runtime_unavailable_affinity(
            trace,
            affinity_kind,
            selection,
            hard_affinity,
            "hard_binding_unavailable",
        ));
    }
    if selection.excluded_profiles.contains(profile_name) {
        return Ok(record_runtime_unavailable_affinity(
            trace,
            affinity_kind,
            selection,
            hard_affinity,
            "bound_profile_unavailable",
        ));
    }
    if hard_affinity {
        return Ok(record_runtime_selected_affinity(
            trace,
            profile_name,
            true,
            None,
            runtime_selection_trace_affinity_kind(affinity_kind),
        ));
    }
    runtime_soft_affinity_selection_decision(shared, selection, affinity_kind, profile_name, trace)
}

fn runtime_soft_affinity_selection_decision(
    shared: &RuntimeRotationProxyShared,
    selection: RuntimeResponseCandidateSelection<'_>,
    affinity_kind: RuntimeAffinitySelectionKind,
    profile_name: &str,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
) -> Result<RuntimeAffinitySelectionDecision> {
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, selection.route_kind)?;
    let current_profile_matches_candidate = affinity_kind == RuntimeAffinitySelectionKind::Session
        && selection.route_kind == RuntimeRouteKind::Websocket
        && quota_source.is_none()
        && runtime_proxy_current_profile(shared)? == profile_name;
    let has_route_eligible_quota_fallback = if current_profile_matches_candidate {
        runtime_has_route_eligible_quota_fallback(
            shared,
            profile_name,
            selection.excluded_profiles,
            selection.route_kind,
        )?
    } else {
        false
    };
    let soft_policy = runtime_proxy_crate::RuntimeSoftAffinityPolicyInput {
        affinity_kind,
        route_kind: selection.route_kind,
        quota_summary: prodex_runtime_quota::runtime_selection_quota_summary_to_proxy(
            quota_summary,
        ),
        quota_source: prodex_runtime_quota::runtime_quota_source_option_to_proxy(quota_source),
        current_profile_matches_candidate,
        has_route_eligible_quota_fallback,
        responses_critical_floor_percent: runtime_proxy_responses_quota_critical_floor_percent(),
    };
    if runtime_proxy_crate::runtime_soft_affinity_allowed(soft_policy) {
        if let Some(reason) = runtime_soft_affinity_local_rejection_reason(
            shared,
            profile_name,
            selection.route_kind,
        )? {
            return Ok(record_runtime_unavailable_affinity(
                trace,
                affinity_kind,
                selection,
                false,
                reason,
            ));
        }
        return Ok(record_runtime_selected_affinity(
            trace,
            profile_name,
            false,
            Some(quota_summary),
            runtime_selection_trace_affinity_kind(affinity_kind),
        ));
    }

    record_runtime_rejected_affinity(
        shared,
        trace,
        affinity_kind,
        selection.route_kind,
        profile_name,
        runtime_proxy_crate::runtime_soft_affinity_rejection_reason(soft_policy),
        RuntimeRejectedAffinityQuota {
            source: quota_source,
            summary: quota_summary,
        },
    );
    Ok(RuntimeAffinitySelectionDecision::Continue)
}

fn runtime_affinity_is_hard(
    shared: &RuntimeRotationProxyShared,
    selection: RuntimeResponseCandidateSelection<'_>,
    affinity_kind: RuntimeAffinitySelectionKind,
    profile_name: &str,
) -> Result<bool> {
    let bound_previous_response_affinity = affinity_kind == RuntimeAffinitySelectionKind::Pinned
        && runtime_previous_response_affinity_is_bound(
            shared,
            selection.previous_response_id,
            selection.pinned_profile,
        )?;
    let trusted_previous_response_affinity = if affinity_kind
        == RuntimeAffinitySelectionKind::Pinned
        && !bound_previous_response_affinity
    {
        runtime_previous_response_affinity_is_trusted(
            shared,
            selection.previous_response_id,
            selection.pinned_profile,
        )?
    } else {
        false
    };
    Ok(bound_previous_response_affinity
        || runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind: selection.route_kind,
            candidate_name: profile_name,
            strict_affinity_profile: selection.strict_affinity_profile,
            pinned_profile: selection.pinned_profile,
            turn_state_profile: selection.turn_state_profile,
            session_profile: selection.session_profile,
            trusted_previous_response_affinity,
        }))
}

fn runtime_soft_affinity_local_rejection_reason(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<Option<&'static str>> {
    let now = Local::now().timestamp();
    let in_backoff = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        runtime_profile_name_in_selection_backoff(
            profile_name,
            &runtime.profile_retry_backoff_until,
            &runtime.profile_transport_backoff_until,
            &runtime.profile_route_circuit_open_until,
            route_kind,
            now,
        )
    };
    if in_backoff {
        return Ok(Some("selection_backoff"));
    }
    if !reserve_runtime_profile_route_circuit_half_open_probe(shared, profile_name, route_kind)? {
        return Ok(Some("route_circuit_half_open_probe_wait"));
    }
    Ok(None)
}

fn runtime_profile_is_usable_for_hard_binding(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<bool> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime.state.profiles.contains_key(profile_name)
        && !runtime_profile_auth_failure_active_with_auth_cache(
            &runtime.profile_health,
            &runtime.profile_usage_auth,
            profile_name,
            Local::now().timestamp(),
        ))
}

fn runtime_profile_has_exact_binding_identity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<Option<bool>> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let Some(current_identity) =
        crate::runtime_proxy::runtime_profile_binding_identity(&runtime, profile_name)
    else {
        return Ok(Some(false));
    };
    // ponytail: bounded lineage maps keep this scan cheap; add an owner index only if the limits grow.
    let bindings = runtime
        .state
        .response_profile_bindings
        .values()
        .chain(runtime.turn_state_bindings.values())
        .chain(runtime.session_id_bindings.values())
        .chain(runtime.state.session_profile_bindings.values());
    let mut found = false;
    let mut exact = false;
    for binding in bindings {
        if binding.profile_name != profile_name {
            continue;
        }
        if let Some(identity) = binding.binding_identity.as_ref() {
            found = true;
            exact |= identity == &current_identity;
        }
    }
    Ok(found.then_some(exact))
}

fn runtime_hard_binding_conflict(selection: RuntimeResponseCandidateSelection<'_>) -> bool {
    let mut owner = None;
    for candidate in [
        selection.strict_affinity_profile,
        selection.pinned_profile,
        selection.turn_state_profile,
        selection
            .session_profile
            .filter(|_| selection.route_kind == RuntimeRouteKind::Compact),
    ]
    .into_iter()
    .flatten()
    {
        if candidate == prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE {
            return true;
        }
        if owner.is_some_and(|current| current != candidate) {
            return true;
        }
        owner = Some(candidate);
    }
    false
}

fn record_runtime_unavailable_affinity(
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    affinity_kind: RuntimeAffinitySelectionKind,
    selection: RuntimeResponseCandidateSelection<'_>,
    hard: bool,
    reason: &'static str,
) -> RuntimeAffinitySelectionDecision {
    let profile_name = runtime_affinity_selection_profile(affinity_kind, selection)
        .or(selection.pinned_profile)
        .or(selection.strict_affinity_profile)
        .unwrap_or(prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE);
    let mut candidate = runtime_selection_trace_candidate(
        0,
        runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
        None,
        None,
        None,
        None,
    );
    candidate.hard_affinity = hard;
    runtime_selection_trace_reject(
        &mut candidate,
        reason,
        Some(runtime_proxy_crate::RuntimeRouteDecisionStage::Affinity),
    );
    trace.record_candidate(profile_name, candidate);
    trace.record_affinity(
        runtime_selection_trace_affinity_kind(affinity_kind),
        Some(profile_name),
        hard,
        if hard {
            runtime_proxy_crate::RuntimeRouteAffinityOutcome::Exhausted
        } else {
            runtime_proxy_crate::RuntimeRouteAffinityOutcome::Rejected
        },
    );
    if hard {
        RuntimeAffinitySelectionDecision::Exhausted
    } else {
        RuntimeAffinitySelectionDecision::Continue
    }
}

fn record_runtime_selected_affinity(
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    profile_name: &str,
    hard: bool,
    quota_summary: Option<RuntimeQuotaSummary>,
    trace_kind: runtime_proxy_crate::RuntimeRouteAffinityKind,
) -> RuntimeAffinitySelectionDecision {
    let mut candidate = runtime_selection_trace_candidate(
        0,
        runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
        quota_summary,
        None,
        None,
        None,
    );
    candidate.hard_affinity = hard;
    trace.record_candidate(profile_name, candidate);
    trace.record_affinity(
        trace_kind,
        Some(profile_name),
        hard,
        runtime_proxy_crate::RuntimeRouteAffinityOutcome::Retained,
    );
    RuntimeAffinitySelectionDecision::Selected(profile_name.to_string())
}

fn record_runtime_rejected_affinity(
    shared: &RuntimeRotationProxyShared,
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    affinity_kind: RuntimeAffinitySelectionKind,
    route_kind: RuntimeRouteKind,
    profile_name: &str,
    reason: &'static str,
    quota: RuntimeRejectedAffinityQuota,
) {
    let mut candidate = runtime_selection_trace_candidate(
        0,
        runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity,
        Some(quota.summary),
        None,
        None,
        None,
    );
    runtime_selection_trace_reject(
        &mut candidate,
        reason,
        Some(runtime_proxy_crate::RuntimeRouteDecisionStage::Quota),
    );
    trace.record_candidate(profile_name, candidate);
    trace.record_affinity(
        runtime_selection_trace_affinity_kind(affinity_kind),
        Some(profile_name),
        false,
        runtime_proxy_crate::RuntimeRouteAffinityOutcome::Rejected,
    );
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "selection_skip_affinity",
            runtime_selection_log_fields_with_quota(
                [
                    runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                    runtime_proxy_log_field("affinity", affinity_kind.skip_label()),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field(
                        "quota_source",
                        runtime_selection_quota_source_label(quota.source),
                    ),
                ],
                quota.summary,
            ),
        ),
    );
}

struct RuntimeRejectedAffinityQuota {
    source: Option<RuntimeQuotaSource>,
    summary: RuntimeQuotaSummary,
}
