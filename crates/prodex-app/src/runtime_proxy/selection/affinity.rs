use anyhow::Result;

use crate::{
    RuntimeContinuationBindingKind, RuntimeContinuationBindingLifecycle,
    runtime_continuation_status_map,
};

use super::{
    RuntimeAffinitySelectionKind, RuntimeCandidateAffinity, RuntimeResponseCandidateSelection,
    RuntimeRotationProxyShared, RuntimeRouteKind, RuntimeSoftAffinityPolicyInput,
    runtime_affinity_selection_profile, runtime_candidate_has_hard_affinity,
    runtime_has_route_eligible_quota_fallback, runtime_profile_quota_summary_for_route,
    runtime_proxy_current_profile, runtime_proxy_log, runtime_proxy_log_field,
    runtime_proxy_structured_log_message, runtime_route_kind_label,
    runtime_selection_log_fields_with_quota, runtime_selection_quota_source_label,
    runtime_soft_affinity_allowed, runtime_soft_affinity_rejection_reason,
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
    let Some(binding) = runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
    else {
        return Ok(false);
    };
    if binding.profile_name != bound_profile {
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

pub(crate) fn runtime_previous_response_affinity_is_bound(
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
    Ok(runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .is_some_and(|binding| binding.profile_name == bound_profile))
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
) -> Result<RuntimeAffinitySelectionDecision> {
    let Some(profile_name) = runtime_affinity_selection_profile(affinity_kind, selection) else {
        return Ok(RuntimeAffinitySelectionDecision::Continue);
    };
    if selection.excluded_profiles.contains(profile_name) {
        return Ok(if affinity_kind.excluded_is_terminal() {
            RuntimeAffinitySelectionDecision::Exhausted
        } else {
            RuntimeAffinitySelectionDecision::Continue
        });
    }

    if affinity_kind == RuntimeAffinitySelectionKind::Pinned
        && runtime_previous_response_affinity_is_bound(
            shared,
            selection.previous_response_id,
            selection.pinned_profile,
        )?
    {
        return Ok(RuntimeAffinitySelectionDecision::Selected(
            profile_name.to_string(),
        ));
    }

    let trusted_previous_response_affinity =
        if affinity_kind == RuntimeAffinitySelectionKind::Pinned {
            runtime_previous_response_affinity_is_trusted(
                shared,
                selection.previous_response_id,
                selection.pinned_profile,
            )?
        } else {
            false
        };
    if runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
        route_kind: selection.route_kind,
        candidate_name: profile_name,
        strict_affinity_profile: selection.strict_affinity_profile,
        pinned_profile: selection.pinned_profile,
        turn_state_profile: selection.turn_state_profile,
        session_profile: selection.session_profile,
        trusted_previous_response_affinity,
    }) {
        return Ok(RuntimeAffinitySelectionDecision::Selected(
            profile_name.to_string(),
        ));
    }

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
    let soft_policy = RuntimeSoftAffinityPolicyInput {
        affinity_kind,
        route_kind: selection.route_kind,
        quota_summary,
        quota_source,
        current_profile_matches_candidate,
        has_route_eligible_quota_fallback,
    };
    if runtime_soft_affinity_allowed(soft_policy) {
        return Ok(RuntimeAffinitySelectionDecision::Selected(
            profile_name.to_string(),
        ));
    }

    let reason = runtime_soft_affinity_rejection_reason(soft_policy);
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "selection_skip_affinity",
            runtime_selection_log_fields_with_quota(
                [
                    runtime_proxy_log_field(
                        "route",
                        runtime_route_kind_label(selection.route_kind),
                    ),
                    runtime_proxy_log_field("affinity", affinity_kind.skip_label()),
                    runtime_proxy_log_field("profile", profile_name),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field(
                        "quota_source",
                        runtime_selection_quota_source_label(quota_source),
                    ),
                ],
                quota_summary,
            ),
        ),
    );
    Ok(RuntimeAffinitySelectionDecision::Continue)
}
