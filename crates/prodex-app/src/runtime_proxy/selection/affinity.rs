use anyhow::Result;
use chrono::Local;

use crate::{
    RuntimeContinuationBindingKind, RuntimeContinuationBindingLifecycle,
    runtime_continuation_status_map,
};

use super::{
    RuntimeAffinitySelectionKind, RuntimeQuotaSummary, RuntimeResponseCandidateSelection,
    RuntimeRotationProxyShared, runtime_affinity_selection_profile,
    runtime_profile_auth_failure_active_with_auth_cache, runtime_request_hard_binding_owner,
    runtime_selection_trace_affinity_kind, runtime_selection_trace_candidate,
    runtime_selection_trace_reject,
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
        return Ok(record_runtime_hard_binding_exhausted(
            trace,
            affinity_kind,
            selection,
            "hard_binding_conflict",
        ));
    }
    let Some(profile_name) = runtime_affinity_selection_profile(affinity_kind, selection) else {
        return Ok(RuntimeAffinitySelectionDecision::Continue);
    };
    let exact_binding = runtime_profile_has_exact_binding_identity(shared, profile_name)?;
    if exact_binding == Some(false) {
        return Ok(record_runtime_hard_binding_exhausted(
            trace,
            affinity_kind,
            selection,
            "binding_identity_mismatch",
        ));
    }
    if !runtime_profile_is_usable_for_hard_binding(shared, profile_name)? {
        return Ok(record_runtime_hard_binding_exhausted(
            trace,
            affinity_kind,
            selection,
            "hard_binding_unavailable",
        ));
    }
    if selection.excluded_profiles.contains(profile_name) {
        return Ok(record_runtime_hard_binding_exhausted(
            trace,
            affinity_kind,
            selection,
            "bound_profile_unavailable",
        ));
    }
    Ok(record_runtime_selected_affinity(
        trace,
        profile_name,
        true,
        None,
        runtime_selection_trace_affinity_kind(affinity_kind),
    ))
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
        selection.session_profile,
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

fn record_runtime_hard_binding_exhausted(
    trace: &mut runtime_proxy_crate::RuntimeRouteDecisionTraceBuilder,
    affinity_kind: RuntimeAffinitySelectionKind,
    selection: RuntimeResponseCandidateSelection<'_>,
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
    candidate.hard_affinity = true;
    runtime_selection_trace_reject(
        &mut candidate,
        reason,
        Some(runtime_proxy_crate::RuntimeRouteDecisionStage::Affinity),
    );
    trace.record_candidate(profile_name, candidate);
    trace.record_affinity(
        runtime_selection_trace_affinity_kind(affinity_kind),
        Some(profile_name),
        true,
        runtime_proxy_crate::RuntimeRouteAffinityOutcome::Exhausted,
    );
    RuntimeAffinitySelectionDecision::Exhausted
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
