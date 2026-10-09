use super::*;
use prodex_mojo_core::runtime::{ResponsesLoopAction, ResponsesLoopInput, ResponsesLoopPhase};
mod affinity_state;
mod attempt;
mod fallback;
mod local_selection;
mod loop_control;
mod overloaded;
mod previous_response;
mod quota_blocked;
use self::affinity_state::{
    RuntimeResponsesAffinityState, RuntimeResponsesRefreshRouteAffinityInput,
};
pub(crate) use self::attempt::{
    RuntimeResponsesAttemptOptions, RuntimeResponsesContinuationTrace,
    attempt_runtime_responses_request, log_runtime_responses_continuation_trace,
    runtime_response_trace_provider_labels,
};
use self::attempt::{
    handle_runtime_responses_auth_failed, handle_runtime_responses_overloaded_attempt,
    handle_runtime_responses_success,
};
use self::fallback::{
    RuntimeResponsesDirectCurrentFallback, RuntimeResponsesDirectCurrentFallbackAction,
    RuntimeResponsesDirectCurrentFallbackReason,
    try_runtime_responses_direct_current_profile_fallback,
};
use self::local_selection::{
    RuntimeResponsesLocalSelectionBlocked, handle_runtime_responses_local_selection_blocked,
    runtime_responses_local_selection_failure_reply,
};
use self::loop_control::RuntimeResponsesRequestContext;
pub(crate) use self::loop_control::proxy_runtime_responses_request;
use self::overloaded::{RuntimeResponsesOverloaded, handle_runtime_responses_overloaded};
use self::previous_response::{
    RuntimeResponsesPreviousResponseNotFoundContextInput,
    handle_runtime_responses_previous_response_attempt,
    runtime_responses_previous_response_not_found_context,
};
use self::quota_blocked::{
    RuntimeResponsesFullContextRetry, handle_runtime_responses_quota_attempt,
    runtime_responses_full_context_retry_available, runtime_responses_full_context_retry_reply,
    try_signal_runtime_responses_full_context_retry,
};
use RuntimeResponsesDirectCurrentFallbackAction::*;
fn runtime_responses_stale_continuation_reply() -> RuntimeResponsesReply {
    RuntimeResponsesReply::Buffered(RuntimeHeapTrimmedBufferedResponseParts::from_crate_parts(
        runtime_proxy_crate::runtime_proxy_stale_continuation_http_parts(),
    ))
}
fn handle_runtime_responses_attempt(
    context: &mut RuntimeResponsesRequestContext<'_>,
    candidate_name: &str,
    turn_state_override: Option<&str>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    auto_redeemed_profiles: &mut BTreeSet<String>,
    quota_last_chance_profile: &mut Option<String>,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
) -> Result<Option<RuntimeResponsesReply>> {
    let hard_affinity = affinity_state.candidate_has_hard_affinity(candidate_name);
    loop_state.begin_attempt();
    let attempt = attempt_runtime_responses_request(
        context.request_id,
        &context.request,
        context.shared,
        candidate_name,
        RuntimeResponsesAttemptOptions {
            turn_state_override,
            prompt_cache_key: context.prompt_cache_key,
            hard_affinity,
            selection_attempt: loop_state.selection_attempts,
        },
    )?;
    if !matches!(
        &attempt,
        RuntimeResponsesAttempt::LocalSelectionBlocked { .. }
    ) {
        loop_state.record_attempt();
    }
    match attempt {
        RuntimeResponsesAttempt::Success {
            profile_name,
            response,
        } => handle_runtime_responses_success(context, affinity_state, profile_name, response),
        RuntimeResponsesAttempt::QuotaBlocked {
            profile_name,
            response,
        } => handle_runtime_responses_quota_attempt(
            context,
            affinity_state,
            auto_redeemed_profiles,
            quota_last_chance_profile,
            loop_state,
            profile_name,
            response,
        ),
        RuntimeResponsesAttempt::RateLimited {
            profile_name,
            response,
            retry_after,
        } => handle_runtime_responses_rate_limited_attempt(
            context,
            affinity_state,
            loop_state,
            profile_name,
            response,
            retry_after,
        ),
        RuntimeResponsesAttempt::Overloaded {
            profile_name,
            response,
            ..
        } => {
            loop_state.record_overload_failure();
            handle_runtime_responses_overloaded_attempt(
                context,
                affinity_state,
                loop_state,
                profile_name,
                response,
            )
        }
        RuntimeResponsesAttempt::AuthFailed {
            profile_name,
            response,
        } => handle_runtime_responses_auth_failed(
            context,
            affinity_state,
            loop_state,
            profile_name,
            response,
        ),
        RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name,
            reason,
        } => handle_runtime_responses_local_selection_attempt(
            context,
            affinity_state,
            loop_state,
            profile_name,
            reason,
        ),
        RuntimeResponsesAttempt::TransportFailed {
            profile_name,
            stage,
        } => {
            runtime_proxy_log(
                context.shared,
                format!(
                    "request={} transport=http responses_transport_failure profile={profile_name} stage={stage} hard_affinity={hard_affinity}",
                    context.request_id,
                ),
            );
            if hard_affinity {
                if runtime_responses_full_context_retry_available(
                    context.shared,
                    &profile_name,
                    context.prompt_cache_key,
                    context.previous_response_id,
                    context.request_session_id,
                    context.request_model_name.as_deref(),
                    &loop_state.excluded_profiles,
                )? {
                    let released_affinity = release_runtime_retryable_failure_affinity(
                        context.shared,
                        &profile_name,
                        context.previous_response_id,
                        context.request_turn_state,
                        context.request_session_id,
                        "transport_full_context_retry",
                    )?;
                    affinity_state.clear_profile_affinity(&profile_name, true);
                    runtime_proxy_log(
                        context.shared,
                        format!(
                            "request={} transport=http transport_failure_full_context_retry_signal profile={profile_name} stage={stage} affinity_released={released_affinity}",
                            context.request_id
                        ),
                    );
                    return Ok(Some(runtime_responses_full_context_retry_reply()));
                }
                return Ok(Some(runtime_responses_local_selection_failure_reply()));
            }
            loop_state.record_transport_failure_at(stage);
            loop_state.excluded_profiles.insert(profile_name);
            Ok(None)
        }
        RuntimeResponsesAttempt::PreviousResponseNotFound {
            profile_name,
            response,
            turn_state,
            invalid_previous_response_id,
        } => handle_runtime_responses_previous_response_attempt(
            context,
            affinity_state,
            loop_state,
            profile_name,
            response,
            turn_state,
            invalid_previous_response_id,
        ),
    }
}
fn handle_runtime_responses_rate_limited_attempt(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    profile_name: String,
    response: RuntimeResponsesReply,
    retry_after: Option<Duration>,
) -> Result<Option<RuntimeResponsesReply>> {
    runtime_proxy_log(
        context.shared,
        format!(
            "request={} transport=http rate_limited route=responses profile={} retry_after_ms={}",
            context.request_id,
            profile_name,
            retry_after.map_or(0, |delay| delay.as_millis()),
        ),
    );
    mark_runtime_profile_retry_backoff_for_delay(context.shared, &profile_name, retry_after)?;
    if affinity_state.candidate_has_hard_affinity(&profile_name) {
        if runtime_responses_full_context_retry_available(
            context.shared,
            &profile_name,
            context.prompt_cache_key,
            context.previous_response_id,
            context.request_session_id,
            context.request_model_name.as_deref(),
            &loop_state.excluded_profiles,
        )? {
            let released_affinity = release_runtime_retryable_failure_affinity(
                context.shared,
                &profile_name,
                context.previous_response_id,
                context.request_turn_state,
                context.request_session_id,
                "rate_limit_full_context_retry",
            )?;
            affinity_state.clear_profile_affinity(&profile_name, true);
            runtime_proxy_log(
                context.shared,
                format!(
                    "request={} transport=http rate_limit_full_context_retry_signal profile={profile_name} affinity_released={released_affinity}",
                    context.request_id
                ),
            );
            return Ok(Some(runtime_responses_full_context_retry_reply()));
        }
        return Ok(Some(response));
    }
    loop_state.record_rate_limit_failure();
    loop_state.excluded_profiles.insert(profile_name);
    loop_state.last_failure = Some((RuntimeUpstreamFailureResponse::Http(response), false));
    Ok(None)
}
fn handle_runtime_responses_local_selection_attempt(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    loop_state: &mut RuntimePrecommitLoopState<RuntimeUpstreamFailureResponse>,
    profile_name: String,
    reason: &'static str,
) -> Result<Option<RuntimeResponsesReply>> {
    handle_runtime_responses_local_selection_blocked(RuntimeResponsesLocalSelectionBlocked {
        request_id: context.request_id,
        shared: context.shared,
        selection_started_at: &mut loop_state.selection_started_at,
        profile_name,
        reason,
        previous_response_id: context.previous_response_id,
        request_turn_state: context.request_turn_state,
        request_session_id: context.request_session_id,
        prompt_cache_key: context.prompt_cache_key,
        request_model_name: context.request_model_name.as_deref(),
        request_requires_previous_response_affinity: context
            .request_requires_previous_response_affinity,
        previous_response_fresh_fallback_shape: context.previous_response_fresh_fallback_shape,
        affinity_state,
        excluded_profiles: &mut loop_state.excluded_profiles,
    })
}
