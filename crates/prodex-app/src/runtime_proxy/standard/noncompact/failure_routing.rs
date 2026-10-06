use super::*;

pub(super) fn handle_runtime_noncompact_local_selection_blocked(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    request_session_id: Option<&str>,
    session_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    profile_name: String,
) -> Result<Option<tiny_http::ResponseBox>> {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http local_selection_blocked profile={profile_name} route=standard reason=quota_exhausted_before_send"
        ),
    );
    let session_owned = session_profile.as_deref() == Some(profile_name.as_str());
    let plan = runtime_noncompact_failure_plan(
        prodex_mojo_core::runtime::NoncompactFailureKind::LocalBlocked,
        session_profile,
        &profile_name,
        false,
        false,
    );
    if plan.clear_session {
        release_noncompact_local_blocked_session(
            shared,
            request_session_id,
            session_owned,
            &profile_name,
        )?;
        clear_noncompact_session_profile(session_profile, &profile_name);
    }
    if plan.exclude_profile {
        loop_state.excluded_profiles.insert(profile_name);
    }
    Ok(None)
}

fn release_noncompact_local_blocked_session(
    shared: &RuntimeRotationProxyShared,
    request_session_id: Option<&str>,
    session_owned: bool,
    profile_name: &str,
) -> Result<()> {
    if !session_owned {
        return Ok(());
    }
    let Some(session_id) = request_session_id else {
        return Ok(());
    };
    let _ =
        release_runtime_quota_blocked_affinity(shared, profile_name, None, None, Some(session_id))?;
    Ok(())
}

pub(super) fn handle_runtime_noncompact_inflight_saturated(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    profile_name: String,
) -> Result<Option<tiny_http::ResponseBox>> {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http local_selection_blocked profile={profile_name} route=standard reason=profile_inflight_saturated"
        ),
    );
    loop_state.record_inflight_saturation();
    Ok(None)
}

pub(super) fn handle_runtime_noncompact_transport_failed(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    request_session_id: Option<&str>,
    session_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    profile_name: String,
    stage: &'static str,
) -> Result<Option<tiny_http::ResponseBox>> {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http standard_transport_failure profile={profile_name} stage={stage}"
        ),
    );
    let session_owned = session_profile.as_deref() == Some(profile_name.as_str());
    let plan = runtime_noncompact_failure_plan(
        prodex_mojo_core::runtime::NoncompactFailureKind::Transport,
        session_profile,
        &profile_name,
        false,
        false,
    );
    if plan.terminal {
        return Ok(Some(build_runtime_proxy_text_response(
            503,
            runtime_proxy_local_selection_failure_message(),
        )));
    }
    if plan.clear_session {
        release_noncompact_transport_session(
            shared,
            request_session_id,
            session_owned,
            &profile_name,
        )?;
        clear_noncompact_session_profile(session_profile, &profile_name);
    }
    if plan.record_transport_failure {
        loop_state.record_transport_failure_at(stage);
    }
    if plan.exclude_profile {
        loop_state.excluded_profiles.insert(profile_name);
    }
    Ok(None)
}

fn release_noncompact_transport_session(
    shared: &RuntimeRotationProxyShared,
    request_session_id: Option<&str>,
    session_owned: bool,
    profile_name: &str,
) -> Result<()> {
    if !session_owned {
        return Ok(());
    }
    let Some(session_id) = request_session_id else {
        return Ok(());
    };
    let _ = release_runtime_retryable_failure_affinity(
        shared,
        profile_name,
        None,
        None,
        Some(session_id),
        "standard_transport",
    )?;
    Ok(())
}
