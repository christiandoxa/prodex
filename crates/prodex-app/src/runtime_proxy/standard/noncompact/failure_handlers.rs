use super::*;

pub(super) fn runtime_noncompact_failure_plan(
    kind: prodex_mojo_core::runtime::NoncompactFailureKind,
    session_profile: &Option<String>,
    profile_name: &str,
    overload: bool,
    quota_fallback_available: bool,
) -> prodex_mojo_core::runtime::NoncompactFailurePlan {
    prodex_mojo_core::runtime::noncompact_failure_plan(
        kind,
        session_profile.as_deref() == Some(profile_name),
        overload,
        quota_fallback_available,
    )
    .expect("Mojo noncompact failure policy returned invalid output")
}

pub(super) fn handle_runtime_noncompact_rate_limited(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    session_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    profile_name: String,
    response: tiny_http::ResponseBox,
    retry_after: Option<Duration>,
) -> Result<Option<tiny_http::ResponseBox>> {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http standard_rate_limited profile={profile_name} retry_after_ms={}",
            retry_after.map_or(0, |delay| delay.as_millis()),
        ),
    );
    let plan = runtime_noncompact_failure_plan(
        prodex_mojo_core::runtime::NoncompactFailureKind::RateLimited,
        session_profile,
        &profile_name,
        false,
        false,
    );
    if plan.mark_backoff {
        mark_runtime_profile_retry_backoff_for_delay(shared, &profile_name, retry_after)?;
    }
    if plan.terminal {
        return Ok(Some(response));
    }
    if plan.clear_session {
        clear_noncompact_session_profile(session_profile, &profile_name);
    }
    if plan.exclude_profile {
        loop_state.excluded_profiles.insert(profile_name.clone());
    }
    if plan.store_last_failure {
        loop_state.last_failure = Some((response, plan.last_failure_retryable));
    }
    Ok(None)
}

pub(super) struct RuntimeNoncompactRetryableContext<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) session_profile: &'a mut Option<String>,
    pub(super) loop_state: &'a mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    pub(super) profile_name: String,
    pub(super) response: tiny_http::ResponseBox,
    pub(super) overload: bool,
}

pub(super) fn handle_runtime_noncompact_profile_unavailable(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    session_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    profile_name: String,
    response: tiny_http::ResponseBox,
) -> Result<Option<tiny_http::ResponseBox>> {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http standard_profile_unavailable profile={profile_name}"
        ),
    );
    let plan = runtime_noncompact_failure_plan(
        prodex_mojo_core::runtime::NoncompactFailureKind::Unavailable,
        session_profile,
        &profile_name,
        false,
        false,
    );
    if plan.terminal {
        return Ok(Some(response));
    }
    if plan.mark_backoff {
        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
    }
    if plan.clear_session {
        clear_noncompact_session_profile(session_profile, &profile_name);
    }
    if plan.exclude_profile {
        loop_state.excluded_profiles.insert(profile_name.clone());
    }
    if plan.store_last_failure {
        loop_state.last_failure = Some((response, plan.last_failure_retryable));
    }
    Ok(None)
}

pub(super) fn handle_runtime_noncompact_retryable(
    context: RuntimeNoncompactRetryableContext<'_>,
) -> Result<Option<tiny_http::ResponseBox>> {
    let RuntimeNoncompactRetryableContext {
        request_id,
        shared,
        request_session_id,
        session_profile,
        loop_state,
        profile_name,
        response,
        overload,
    } = context;
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http standard_retryable_failure profile={profile_name} reason={}",
            if overload { "overload" } else { "quota" }
        ),
    );
    if overload {
        loop_state.record_overload_failure();
    }
    mark_runtime_profile_retry_backoff(shared, &profile_name)?;
    let released_affinity = if overload {
        let _ = bump_runtime_profile_health_score(
            shared,
            &profile_name,
            RuntimeRouteKind::Standard,
            RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
            "standard_overload",
        );
        let _ = bump_runtime_profile_bad_pairing_score(
            shared,
            &profile_name,
            RuntimeRouteKind::Standard,
            RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
            "standard_overload",
        );
        false
    } else {
        release_runtime_quota_blocked_affinity(
            shared,
            &profile_name,
            None,
            None,
            request_session_id,
        )?
    };
    clear_noncompact_session_profile(session_profile, &profile_name);
    if released_affinity {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} route=standard"
            ),
        );
    }
    let quota_fallback_available = if overload {
        false
    } else {
        runtime_has_route_eligible_quota_fallback(
            shared,
            &profile_name,
            &BTreeSet::new(),
            RuntimeRouteKind::Standard,
        )?
    };
    let plan = runtime_noncompact_failure_plan(
        prodex_mojo_core::runtime::NoncompactFailureKind::Retryable,
        session_profile,
        &profile_name,
        overload,
        quota_fallback_available,
    );
    if plan.terminal {
        return Ok(Some(response));
    }
    if plan.exclude_profile {
        loop_state.excluded_profiles.insert(profile_name.clone());
    }
    if plan.store_last_failure {
        loop_state.last_failure = Some((response, plan.last_failure_retryable));
    }
    Ok(None)
}

pub(super) fn handle_runtime_noncompact_auth_failed(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    request_session_id: Option<&str>,
    session_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    profile_name: String,
    response: tiny_http::ResponseBox,
) -> Result<Option<tiny_http::ResponseBox>> {
    runtime_proxy_log(
        shared,
        format!("request={request_id} transport=http standard_auth_failed profile={profile_name}"),
    );
    let released_affinity = release_runtime_auth_failed_affinity(
        shared,
        &profile_name,
        None,
        None,
        request_session_id,
    )?;
    let plan = runtime_noncompact_failure_plan(
        prodex_mojo_core::runtime::NoncompactFailureKind::AuthFailed,
        session_profile,
        &profile_name,
        false,
        false,
    );
    if plan.clear_session {
        clear_noncompact_session_profile(session_profile, &profile_name);
    }
    if released_affinity {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http auth_failed_affinity_released profile={profile_name} route=standard"
            ),
        );
    }
    if plan.exclude_profile {
        loop_state.excluded_profiles.insert(profile_name.clone());
    }
    if plan.store_last_failure {
        loop_state.last_failure = Some((response, plan.last_failure_retryable));
    } else if plan.terminal {
        return Ok(Some(response));
    }
    Ok(None)
}

pub(super) fn clear_noncompact_session_profile(
    session_profile: &mut Option<String>,
    profile_name: &str,
) {
    if session_profile.as_deref() == Some(profile_name) {
        *session_profile = None;
    }
}
