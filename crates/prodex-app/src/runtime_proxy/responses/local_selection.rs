use super::*;

pub(super) enum RuntimeResponsesLocalSelectionAction {
    ReturnServiceUnavailable,
    Rotate,
}

pub(super) fn runtime_responses_local_selection_action(
    releasable: bool,
) -> RuntimeResponsesLocalSelectionAction {
    if releasable {
        RuntimeResponsesLocalSelectionAction::Rotate
    } else {
        RuntimeResponsesLocalSelectionAction::ReturnServiceUnavailable
    }
}

pub(super) fn runtime_responses_local_selection_failure_reply() -> RuntimeResponsesReply {
    RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
        503,
        "service_unavailable",
        runtime_proxy_local_selection_failure_message(),
    ))
}

pub(super) struct RuntimeResponsesLocalSelectionBlocked<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: String,
    pub(super) reason: &'static str,
    pub(super) previous_response_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_requires_previous_response_affinity: bool,
    pub(super) previous_response_fresh_fallback_shape:
        Option<RuntimePreviousResponseFreshFallbackShape>,
    pub(super) affinity_state: &'a mut RuntimeResponsesAffinityState,
    pub(super) excluded_profiles: &'a mut BTreeSet<String>,
}

pub(super) fn handle_runtime_responses_local_selection_blocked(
    blocked: RuntimeResponsesLocalSelectionBlocked<'_>,
) -> Result<Option<RuntimeResponsesReply>> {
    let RuntimeResponsesLocalSelectionBlocked {
        request_id,
        shared,
        profile_name,
        reason,
        previous_response_id,
        request_turn_state,
        request_session_id,
        request_requires_previous_response_affinity,
        previous_response_fresh_fallback_shape,
        affinity_state,
        excluded_profiles,
    } = blocked;

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http local_selection_blocked profile={profile_name} route=responses reason={reason}"
        ),
    );
    mark_runtime_profile_retry_backoff(shared, &profile_name)?;
    match runtime_responses_local_selection_action(
        affinity_state.quota_blocked_affinity_is_releasable(
            &profile_name,
            request_requires_previous_response_affinity,
            previous_response_fresh_fallback_shape,
        ),
    ) {
        RuntimeResponsesLocalSelectionAction::ReturnServiceUnavailable => {
            return Ok(Some(runtime_responses_local_selection_failure_reply()));
        }
        RuntimeResponsesLocalSelectionAction::Rotate => {}
    }
    let released_affinity = release_runtime_quota_blocked_affinity(
        shared,
        &profile_name,
        previous_response_id,
        request_turn_state,
        request_session_id,
    )?;
    affinity_state.clear_profile_affinity(&profile_name, true);
    if released_affinity {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason}"
            ),
        );
    }
    excluded_profiles.insert(profile_name);
    Ok(None)
}
