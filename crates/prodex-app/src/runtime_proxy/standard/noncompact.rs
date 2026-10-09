use super::*;

mod failure_handlers;
mod failure_routing;
mod loop_policy;

use failure_handlers::*;
use failure_routing::*;
use loop_policy::{RuntimeNoncompactNextActionContext, runtime_noncompact_next_action};

pub(super) fn proxy_runtime_noncompact_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    let current_profile = runtime_proxy_current_profile(shared)?;
    if is_runtime_realtime_call_path(&request.path_and_query) {
        return proxy_runtime_noncompact_realtime_request(
            request_id,
            request,
            shared,
            &current_profile,
        );
    }
    proxy_runtime_noncompact_standard_request(request_id, request, shared, current_profile)
}

fn proxy_runtime_noncompact_realtime_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    current_profile: &str,
) -> Result<tiny_http::ResponseBox> {
    let request_model_name = runtime_smart_context_model_name_from_body(&request.body);
    runtime_selection_trace_log_direct(
        shared,
        request_id,
        RuntimeSelectionTraceDirect {
            requested_model: request_model_name.as_deref(),
            route_kind: RuntimeRouteKind::Standard,
            candidate_key: current_profile,
            class: runtime_proxy_crate::RuntimeRouteCandidateClass::Current,
            affinity_kind: Some(runtime_proxy_crate::RuntimeRouteAffinityKind::Strict),
            hard_affinity: true,
        },
    );
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http realtime_call_owner_pinned profile={current_profile} reason=sideband_auth_uses_current_profile"
        ),
    );
    match attempt_runtime_noncompact_standard_request_with_policy(
        request_id,
        request,
        shared,
        current_profile,
        false,
        true,
    )? {
        RuntimeStandardAttempt::Success {
            profile_name,
            response,
        } => {
            commit_runtime_proxy_profile_selection_with_notice(
                shared,
                &profile_name,
                RuntimeRouteKind::Standard,
            )?;
            Ok(response)
        }
        RuntimeStandardAttempt::StaleContinuation { response }
        | RuntimeStandardAttempt::RetryableFailure { response, .. }
        | RuntimeStandardAttempt::RateLimited { response, .. }
        | RuntimeStandardAttempt::ProfileUnavailable { response, .. }
        | RuntimeStandardAttempt::AuthFailed { response, .. } => Ok(response),
        RuntimeStandardAttempt::LocalSelectionBlocked { .. }
        | RuntimeStandardAttempt::TransportFailed { .. } => Ok(build_runtime_proxy_text_response(
            503,
            runtime_proxy_local_selection_failure_message(),
        )),
        RuntimeStandardAttempt::ProfileInflightSaturated { .. } => Ok(
            build_runtime_proxy_text_response(503, runtime_proxy_local_selection_failure_message()),
        ),
    }
}

fn proxy_runtime_noncompact_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    current_profile: String,
) -> Result<tiny_http::ResponseBox> {
    let request_model_name = runtime_smart_context_model_name_from_body(&request.body);
    let request_session_id = runtime_request_session_id(request);
    let mut session_profile = request_session_id
        .as_deref()
        .map(|session_id| runtime_session_bound_profile(shared, session_id))
        .transpose()?
        .flatten();
    let preferred_profile = session_profile
        .clone()
        .unwrap_or_else(|| current_profile.clone());
    let pressure_mode =
        runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Standard);
    let mut loop_state = RuntimePrecommitLoopState::<tiny_http::ResponseBox>::new();
    let (quota_summary, quota_source) = runtime_profile_quota_summary_for_route(
        shared,
        &preferred_profile,
        RuntimeRouteKind::Standard,
    )?;
    let preferred_is_session = session_profile.as_deref() == Some(preferred_profile.as_str());
    let preferred_profile_usable = if preferred_is_session {
        runtime_quota_summary_allows_soft_affinity(
            quota_summary,
            quota_source,
            RuntimeRouteKind::Standard,
        )
    } else {
        quota_summary.route_band != RuntimeQuotaPressureBand::Exhausted
    };
    if !preferred_profile_usable {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http {} profile={} reason={} quota_source={} {}",
                if preferred_is_session {
                    format!(
                        "selection_skip_affinity route={} affinity=session",
                        runtime_route_kind_label(RuntimeRouteKind::Standard)
                    )
                } else {
                    format!(
                        "selection_skip_current route={}",
                        runtime_route_kind_label(RuntimeRouteKind::Standard)
                    )
                },
                preferred_profile,
                if preferred_is_session {
                    runtime_quota_soft_affinity_rejection_reason(
                        quota_summary,
                        quota_source,
                        RuntimeRouteKind::Standard,
                    )
                } else {
                    runtime_quota_pressure_band_reason(quota_summary.route_band)
                },
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        loop_state
            .excluded_profiles
            .insert(preferred_profile.clone());
    }

    run_runtime_noncompact_standard_loop(RuntimeNoncompactStandardLoopContext {
        request_id,
        request,
        shared,
        request_model_name: request_model_name.as_deref(),
        request_session_id: request_session_id.as_deref(),
        preferred_profile: &preferred_profile,
        preferred_is_session,
        session_profile: &mut session_profile,
        pressure_mode,
        loop_state: &mut loop_state,
    })
}

struct RuntimeNoncompactStandardLoopContext<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeRotationProxyShared,
    request_model_name: Option<&'a str>,
    request_session_id: Option<&'a str>,
    preferred_profile: &'a str,
    preferred_is_session: bool,
    session_profile: &'a mut Option<String>,
    pressure_mode: bool,
    loop_state: &'a mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
}

enum RuntimeNoncompactBudgetAction {
    Proceed,
    Continue,
    Return(tiny_http::ResponseBox),
}

fn runtime_noncompact_budget_action(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    session_present: bool,
    pressure_mode: bool,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
) -> Result<RuntimeNoncompactBudgetAction> {
    let normal_budget_exhausted = runtime_proxy_precommit_budget_exhausted_for_route(
        shared,
        loop_state.selection_started_at,
        loop_state.selection_attempts,
        session_present,
        pressure_mode,
    )?;
    let saw_transient_failure = loop_state.saw_overload_failure
        || loop_state.saw_rate_limit_failure
        || loop_state.saw_transport_failure;
    let route_has_retryable_profile = saw_transient_failure
        && runtime_route_has_retryable_profile(shared, RuntimeRouteKind::Standard)?;
    let profile_count = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .state
        .profiles
        .len()
        .max(1);
    let attempt_limit = runtime_proxy_crate::runtime_proxy_precommit_budget_for_profile_count(
        session_present,
        pressure_mode,
        profile_count,
    )
    .0;
    let action = prodex_mojo_core::runtime::noncompact_precommit_action(
        prodex_mojo_core::runtime::NoncompactPrecommitInput {
            normal_budget_exhausted,
            continuation: session_present,
            saw_transient_failure,
            route_has_retryable_profile,
            recovery_sweeps: loop_state.recovery_sweeps,
            attempts: loop_state.selection_attempts,
            profile_count,
            attempt_limit,
        },
    )
    .map_err(|error| anyhow::anyhow!("Mojo noncompact precommit policy failed: {error:?}"))?;
    if matches!(
        action,
        prodex_mojo_core::runtime::NoncompactPrecommitAction::Proceed
    ) {
        return Ok(RuntimeNoncompactBudgetAction::Proceed);
    }
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http standard_precommit_budget_exhausted attempts={} elapsed_ms={} pressure_mode={pressure_mode}",
            loop_state.selection_attempts,
            loop_state.selection_started_at.elapsed().as_millis()
        ),
    );
    if matches!(
        action,
        prodex_mojo_core::runtime::NoncompactPrecommitAction::WaitTransient
    ) && loop_state.maybe_wait_for_transient_recovery(
        request_id,
        shared,
        RuntimeRouteKind::Standard,
    )? {
        return Ok(RuntimeNoncompactBudgetAction::Continue);
    }
    Ok(RuntimeNoncompactBudgetAction::Return(
        runtime_proxy_final_retryable_http_failure_response(
            loop_state.last_failure.take(),
            loop_state.saw_inflight_saturation,
            false,
        )
        .unwrap_or_else(|| {
            build_runtime_proxy_text_response(503, runtime_proxy_local_selection_failure_message())
        }),
    ))
}

fn wait_after_runtime_noncompact_inflight_saturation(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    session_profile: &Option<String>,
) -> Result<()> {
    loop_state.record_inflight_saturation();
    let _ = runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
        observed_release_revision: None,
        request_id,
        shared,
        excluded_profiles: &loop_state.excluded_profiles,
        route_kind: RuntimeRouteKind::Standard,
        selection_started_at: &mut loop_state.selection_started_at,
        continuation: session_profile.is_some(),
        wait_affinity_owner: session_profile.as_deref(),
        selected_profile: None,
    })?;
    Ok(())
}

fn run_runtime_noncompact_standard_loop(
    context: RuntimeNoncompactStandardLoopContext<'_>,
) -> Result<tiny_http::ResponseBox> {
    let RuntimeNoncompactStandardLoopContext {
        request_id,
        request,
        shared,
        request_model_name,
        request_session_id,
        preferred_profile,
        preferred_is_session,
        session_profile,
        pressure_mode,
        loop_state,
    } = context;
    loop {
        match runtime_noncompact_budget_action(
            request_id,
            shared,
            session_profile.is_some(),
            pressure_mode,
            &mut *loop_state,
        )? {
            RuntimeNoncompactBudgetAction::Continue => continue,
            RuntimeNoncompactBudgetAction::Return(response) => return Ok(response),
            RuntimeNoncompactBudgetAction::Proceed => {}
        }

        let action = runtime_noncompact_next_action(RuntimeNoncompactNextActionContext {
            request_id,
            shared,
            request_model_name,
            preferred_profile,
            preferred_is_session,
            session_present: session_profile.is_some(),
            wait_affinity_owner: session_profile.as_deref(),
            loop_state: &mut *loop_state,
        })?;
        let candidate_name = match action {
            RuntimePrecommitLoopAction::Continue => continue,
            RuntimePrecommitLoopAction::Attempt(candidate_name) => candidate_name,
            RuntimePrecommitLoopAction::Return(response) => return Ok(response),
        };
        loop_state.begin_attempt();
        let attempt = attempt_runtime_noncompact_standard_request(
            request_id,
            request,
            shared,
            &candidate_name,
            session_profile.as_deref() == Some(candidate_name.as_str()),
        )?;
        if matches!(
            &attempt,
            RuntimeStandardAttempt::ProfileInflightSaturated { .. }
        ) {
            wait_after_runtime_noncompact_inflight_saturation(
                request_id,
                shared,
                &mut *loop_state,
                session_profile,
            )?;
            continue;
        }
        if !matches!(
            &attempt,
            RuntimeStandardAttempt::LocalSelectionBlocked { .. }
                | RuntimeStandardAttempt::ProfileInflightSaturated { .. }
        ) {
            loop_state.record_attempt();
        }
        if let Some(response) = handle_runtime_noncompact_attempt(
            request_id,
            shared,
            request_session_id,
            !preferred_is_session,
            &mut *session_profile,
            &mut *loop_state,
            attempt,
        )? {
            return Ok(response);
        }
    }
}

fn handle_runtime_noncompact_attempt(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    request_session_id: Option<&str>,
    promote_committed_profile: bool,
    session_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
    attempt: RuntimeStandardAttempt,
) -> Result<Option<tiny_http::ResponseBox>> {
    match attempt {
        RuntimeStandardAttempt::Success {
            profile_name,
            response,
        } => {
            let _ = commit_runtime_proxy_profile_selection_with_policy(
                shared,
                &profile_name,
                RuntimeRouteKind::Standard,
                promote_committed_profile,
            )?;
            Ok(Some(response))
        }
        RuntimeStandardAttempt::StaleContinuation { response } => Ok(Some(response)),
        RuntimeStandardAttempt::RateLimited {
            profile_name,
            response,
            retry_after,
        } => handle_runtime_noncompact_rate_limited(RuntimeNoncompactRateLimitedContext {
            request_id,
            shared,
            request_session_id,
            session_profile,
            loop_state,
            profile_name,
            response,
            retry_after,
        }),
        RuntimeStandardAttempt::RetryableFailure {
            profile_name,
            response,
            overload,
        } => handle_runtime_noncompact_retryable(RuntimeNoncompactRetryableContext {
            request_id,
            shared,
            request_session_id,
            session_profile,
            loop_state,
            profile_name,
            response,
            overload,
        }),
        RuntimeStandardAttempt::ProfileUnavailable {
            profile_name,
            response,
        } => handle_runtime_noncompact_profile_unavailable(
            request_id,
            shared,
            request_session_id,
            session_profile,
            loop_state,
            profile_name,
            response,
        ),
        RuntimeStandardAttempt::AuthFailed {
            profile_name,
            response,
        } => handle_runtime_noncompact_auth_failed(
            request_id,
            shared,
            request_session_id,
            session_profile,
            loop_state,
            profile_name,
            response,
        ),
        RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
            handle_runtime_noncompact_local_selection_blocked(
                request_id,
                shared,
                request_session_id,
                session_profile,
                loop_state,
                profile_name,
            )
        }
        RuntimeStandardAttempt::ProfileInflightSaturated { profile_name } => {
            handle_runtime_noncompact_inflight_saturated(
                request_id,
                shared,
                loop_state,
                profile_name,
            )
        }
        RuntimeStandardAttempt::TransportFailed {
            profile_name,
            stage,
        } => handle_runtime_noncompact_transport_failed(
            request_id,
            shared,
            request_session_id,
            session_profile,
            loop_state,
            profile_name,
            stage,
        ),
    }
}
