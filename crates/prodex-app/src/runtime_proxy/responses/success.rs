use super::*;

pub(super) fn handle_runtime_responses_success(
    context: &RuntimeResponsesRequestContext<'_>,
    affinity_state: &mut RuntimeResponsesAffinityState,
    profile_name: String,
    response: RuntimeResponsesReply,
) -> Result<Option<RuntimeResponsesReply>> {
    affinity_state.remember_successful_previous_response_owner(
        context.shared,
        &profile_name,
        context.previous_response_id,
    )?;
    commit_runtime_proxy_profile_selection_with_notice(
        context.shared,
        &profile_name,
        RuntimeRouteKind::Responses,
    )?;
    runtime_proxy_log(
        context.shared,
        format!(
            "request={} transport=http committed profile={profile_name}",
            context.request_id
        ),
    );
    Ok(Some(response))
}

pub(super) fn prepare_runtime_responses_success_attempt(
    context: RuntimeResponsesSuccessAttemptContext,
    response: reqwest::Response,
) -> Result<RuntimeResponsesAttempt> {
    let RuntimeResponsesSuccessAttemptContext {
        request_id,
        shared,
        profile_name,
        turn_state_override,
        request_previous_response_id,
        request_prompt_cache_key,
        request_session_id,
        request_thread_id,
        request_turn_id,
        request_turn_state,
        request_model_name,
        request_streaming,
        selection_attempt,
        retry_number,
        request_compaction_generation,
        inflight_guard,
    } = context;
    let Some(inflight_guard) = inflight_guard else {
        runtime_proxy_log(
            &shared,
            runtime_proxy_structured_log_message(
                "responses_inflight_guard_missing",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("profile", profile_name),
                ],
            ),
        );
        return Err(anyhow::anyhow!(
            "responses inflight guard missing before success forwarding"
        ));
    };
    let success_shared = shared.clone();
    let success_profile_name = profile_name.clone();
    await_runtime_proxy_async_task(&shared, "responses_prepare_success", async move {
        prepare_runtime_proxy_responses_success(
            RuntimeResponsesSuccessContext {
                request_id,
                request_model_name: request_model_name.as_deref(),
                request_streaming,
                request_previous_response_id: request_previous_response_id.as_deref(),
                request_prompt_cache_key: request_prompt_cache_key.as_deref(),
                request_session_id: request_session_id.as_deref(),
                request_thread_id: request_thread_id.as_deref(),
                request_turn_id: request_turn_id.as_deref(),
                request_turn_state: request_turn_state.as_deref(),
                turn_state_override: turn_state_override.as_deref(),
                selection_attempt,
                retry_number,
                request_compaction_generation,
                shared: &success_shared,
                profile_name: &success_profile_name,
                inflight_guard,
            },
            response,
        )
        .await
    })
    .inspect_err(|err| {
        note_runtime_profile_transport_failure(
            &shared,
            &profile_name,
            RuntimeRouteKind::Responses,
            "responses_prepare_success",
            err,
        );
    })
}

pub(super) struct RuntimeResponsesSuccessAttemptContext {
    pub(super) request_id: u64,
    pub(super) shared: RuntimeRotationProxyShared,
    pub(super) profile_name: String,
    pub(super) turn_state_override: Option<String>,
    pub(super) request_previous_response_id: Option<String>,
    pub(super) request_prompt_cache_key: Option<String>,
    pub(super) request_session_id: Option<String>,
    pub(super) request_thread_id: Option<String>,
    pub(super) request_turn_id: Option<String>,
    pub(super) request_turn_state: Option<String>,
    pub(super) request_model_name: Option<String>,
    pub(super) request_streaming: bool,
    pub(super) selection_attempt: usize,
    pub(super) retry_number: usize,
    pub(super) request_compaction_generation: Option<u64>,
    pub(super) inflight_guard: Option<RuntimeProfileInFlightGuard>,
}
