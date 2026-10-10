use super::*;

pub(super) struct RuntimeNoncompactNextActionContext<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) request_model_name: Option<&'a str>,
    pub(super) preferred_profile: &'a str,
    pub(super) preferred_is_session: bool,
    pub(super) session_present: bool,
    pub(super) wait_affinity_owner: Option<&'a str>,
    pub(super) loop_state: &'a mut RuntimePrecommitLoopState<tiny_http::ResponseBox>,
}

pub(super) fn runtime_noncompact_next_action(
    context: RuntimeNoncompactNextActionContext<'_>,
) -> Result<RuntimePrecommitLoopAction<String, tiny_http::ResponseBox>> {
    let RuntimeNoncompactNextActionContext {
        request_id,
        shared,
        request_model_name,
        preferred_profile,
        preferred_is_session,
        session_present,
        wait_affinity_owner,
        loop_state,
    } = context;
    let release_revision = runtime_profile_inflight_release_revision(shared);
    let preferred_hard_limited = !preferred_is_session
        && runtime_profile_inflight_hard_limited_for_context(
            shared,
            preferred_profile,
            runtime_route_kind_inflight_context(RuntimeRouteKind::Standard),
        )?;
    let initial_action = prodex_mojo_core::runtime::noncompact_loop_action(
        prodex_mojo_core::runtime::NoncompactLoopInput {
            stage: prodex_mojo_core::runtime::NoncompactLoopStage::SelectPreferred,
            excluded_profiles_empty: loop_state.excluded_profiles.is_empty(),
            preferred_is_session,
            preferred_hard_limited,
            fresh_candidate_present: false,
            candidate_hard_affinity: false,
            candidate_hard_limited: false,
            inflight_relieved: false,
            session_present,
            cold_start_profiles_present: false,
            cold_start_probe_waited: loop_state.cold_start_probe_waited,
            transient_recovered: false,
        },
    )
    .map_err(|error| anyhow::anyhow!("Mojo noncompact loop policy failed: {error:?}"))?;
    match initial_action {
        prodex_mojo_core::runtime::NoncompactLoopAction::AttemptPreferred => {
            runtime_selection_trace_log_direct(
                shared,
                request_id,
                RuntimeSelectionTraceDirect {
                    requested_model: request_model_name,
                    route_kind: RuntimeRouteKind::Standard,
                    candidate_key: preferred_profile,
                    class: if preferred_is_session {
                        runtime_proxy_crate::RuntimeRouteCandidateClass::Affinity
                    } else {
                        runtime_proxy_crate::RuntimeRouteCandidateClass::Current
                    },
                    affinity_kind: preferred_is_session
                        .then_some(runtime_proxy_crate::RuntimeRouteAffinityKind::Session),
                    hard_affinity: preferred_is_session,
                },
            );
            return Ok(RuntimePrecommitLoopAction::Attempt(
                preferred_profile.to_string(),
            ));
        }
        prodex_mojo_core::runtime::NoncompactLoopAction::SelectFreshCandidate => {
            if preferred_hard_limited && loop_state.excluded_profiles.is_empty() {
                loop_state.record_inflight_saturation();
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "profile_inflight_saturated",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "http"),
                            runtime_proxy_log_field("profile", preferred_profile),
                            runtime_proxy_log_field(
                                "hard_limit",
                                shared
                                    .runtime_config
                                    .tuning
                                    .profile_inflight_hard_limit
                                    .to_string(),
                            ),
                        ],
                    ),
                );
            }
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Mojo noncompact loop returned an invalid initial action"
            ));
        }
    }

    let candidate_name = select_runtime_response_candidate_for_route_with_request(
        shared,
        RuntimeResponseCandidateSelection::fresh(
            &loop_state.excluded_profiles,
            RuntimeRouteKind::Standard,
        ),
        Some(request_id),
        request_model_name,
    )?;
    let candidate_action = prodex_mojo_core::runtime::noncompact_loop_action(
        prodex_mojo_core::runtime::NoncompactLoopInput {
            stage: prodex_mojo_core::runtime::NoncompactLoopStage::AfterFreshSelection,
            excluded_profiles_empty: false,
            preferred_is_session,
            preferred_hard_limited,
            fresh_candidate_present: candidate_name.is_some(),
            candidate_hard_affinity: false,
            candidate_hard_limited: false,
            inflight_relieved: false,
            session_present,
            cold_start_profiles_present: false,
            cold_start_probe_waited: loop_state.cold_start_probe_waited,
            transient_recovered: false,
        },
    )
    .map_err(|error| anyhow::anyhow!("Mojo noncompact loop policy failed: {error:?}"))?;
    match candidate_action {
        prodex_mojo_core::runtime::NoncompactLoopAction::AttemptCandidate => {
            let candidate_name = candidate_name.ok_or_else(|| {
                anyhow::anyhow!("Mojo selected a candidate action without a candidate")
            })?;
            return runtime_noncompact_candidate_admission(
                RuntimeNoncompactNextActionContext {
                    request_id,
                    shared,
                    request_model_name,
                    preferred_profile,
                    preferred_is_session,
                    session_present,
                    wait_affinity_owner,
                    loop_state,
                },
                candidate_name,
                preferred_hard_limited,
            );
        }
        prodex_mojo_core::runtime::NoncompactLoopAction::WaitInflight => {}
        _ => {
            return Err(anyhow::anyhow!(
                "Mojo noncompact loop returned an invalid fresh selection action"
            ));
        }
    }

    let inflight_relieved = matches!(
        runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
            observed_release_revision: Some(release_revision),
            request_id,
            shared,
            excluded_profiles: &loop_state.excluded_profiles,
            route_kind: RuntimeRouteKind::Standard,
            selection_started_at: &mut loop_state.selection_started_at,
            continuation: session_present,
            wait_affinity_owner,
            selected_profile: None,
        })?,
        RuntimeInflightReliefWaitResult::Relieved
    );
    let remaining_cold_start_profiles = runtime_remaining_sync_probe_cold_start_profiles_for_route(
        shared,
        &loop_state.excluded_profiles,
        RuntimeRouteKind::Standard,
    )?;
    let mut post_wait_action = prodex_mojo_core::runtime::noncompact_loop_action(
        prodex_mojo_core::runtime::NoncompactLoopInput {
            stage: prodex_mojo_core::runtime::NoncompactLoopStage::AfterInflightWait,
            excluded_profiles_empty: false,
            preferred_is_session,
            preferred_hard_limited,
            fresh_candidate_present: false,
            candidate_hard_affinity: false,
            candidate_hard_limited: false,
            inflight_relieved,
            session_present,
            cold_start_profiles_present: remaining_cold_start_profiles > 0,
            cold_start_probe_waited: loop_state.cold_start_probe_waited,
            transient_recovered: false,
        },
    )
    .map_err(|error| anyhow::anyhow!("Mojo noncompact loop policy failed: {error:?}"))?;
    loop {
        match post_wait_action {
            prodex_mojo_core::runtime::NoncompactLoopAction::Continue => {
                return Ok(RuntimePrecommitLoopAction::Continue);
            }
            prodex_mojo_core::runtime::NoncompactLoopAction::WaitColdStart => {
                if loop_state.claim_cold_start_probe_wait() {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http candidate_exhausted_continue route=standard remaining_cold_start_profiles={remaining_cold_start_profiles}"
                        ),
                    );
                    runtime_proxy_probe_refresh_pause(shared, RuntimeRouteKind::Standard);
                    return Ok(RuntimePrecommitLoopAction::Continue);
                }
                post_wait_action = prodex_mojo_core::runtime::noncompact_loop_action(
                    prodex_mojo_core::runtime::NoncompactLoopInput {
                        stage: prodex_mojo_core::runtime::NoncompactLoopStage::AfterInflightWait,
                        excluded_profiles_empty: false,
                        preferred_is_session,
                        preferred_hard_limited,
                        fresh_candidate_present: false,
                        candidate_hard_affinity: false,
                        candidate_hard_limited: false,
                        inflight_relieved,
                        session_present,
                        cold_start_profiles_present: remaining_cold_start_profiles > 0,
                        cold_start_probe_waited: true,
                        transient_recovered: false,
                    },
                )
                .map_err(|error| {
                    anyhow::anyhow!("Mojo noncompact loop policy failed: {error:?}")
                })?;
            }
            prodex_mojo_core::runtime::NoncompactLoopAction::WaitTransient => {
                let transient_recovered = loop_state.maybe_wait_for_transient_recovery(
                    request_id,
                    shared,
                    RuntimeRouteKind::Standard,
                )?;
                post_wait_action = prodex_mojo_core::runtime::noncompact_loop_action(
                    prodex_mojo_core::runtime::NoncompactLoopInput {
                        stage: prodex_mojo_core::runtime::NoncompactLoopStage::AfterTransientWait,
                        excluded_profiles_empty: false,
                        preferred_is_session,
                        preferred_hard_limited,
                        fresh_candidate_present: false,
                        candidate_hard_affinity: false,
                        candidate_hard_limited: false,
                        inflight_relieved,
                        session_present,
                        cold_start_profiles_present: remaining_cold_start_profiles > 0,
                        cold_start_probe_waited: loop_state.cold_start_probe_waited,
                        transient_recovered,
                    },
                )
                .map_err(|error| {
                    anyhow::anyhow!("Mojo noncompact loop policy failed: {error:?}")
                })?;
            }
            prodex_mojo_core::runtime::NoncompactLoopAction::Return => {
                return Ok(RuntimePrecommitLoopAction::Return(
                    runtime_proxy_final_retryable_http_failure_response(
                        loop_state.last_failure.take(),
                        loop_state.saw_inflight_saturation,
                        false,
                    )
                    .unwrap_or_else(|| {
                        build_runtime_proxy_text_response(
                            503,
                            runtime_proxy_local_selection_failure_message(),
                        )
                    }),
                ));
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Mojo noncompact loop returned an invalid recovery action"
                ));
            }
        }
    }
}

fn runtime_noncompact_candidate_admission(
    context: RuntimeNoncompactNextActionContext<'_>,
    candidate_name: String,
    preferred_hard_limited: bool,
) -> Result<RuntimePrecommitLoopAction<String, tiny_http::ResponseBox>> {
    let RuntimeNoncompactNextActionContext {
        request_id,
        shared,
        preferred_is_session,
        session_present,
        wait_affinity_owner,
        loop_state,
        ..
    } = context;
    let candidate_hard_limited = runtime_profile_inflight_hard_limited_for_context(
        shared,
        &candidate_name,
        "standard_http",
    )?;
    let admission_action = prodex_mojo_core::runtime::noncompact_loop_action(
        prodex_mojo_core::runtime::NoncompactLoopInput {
            stage: prodex_mojo_core::runtime::NoncompactLoopStage::CandidateAdmission,
            excluded_profiles_empty: false,
            preferred_is_session,
            preferred_hard_limited,
            fresh_candidate_present: true,
            candidate_hard_affinity: wait_affinity_owner == Some(candidate_name.as_str()),
            candidate_hard_limited,
            inflight_relieved: false,
            session_present,
            cold_start_profiles_present: false,
            cold_start_probe_waited: loop_state.cold_start_probe_waited,
            transient_recovered: false,
        },
    )
    .map_err(|error| anyhow::anyhow!("Mojo noncompact loop policy failed: {error:?}"))?;
    match admission_action {
        prodex_mojo_core::runtime::NoncompactLoopAction::AttemptCandidate => {
            Ok(RuntimePrecommitLoopAction::Attempt(candidate_name))
        }
        prodex_mojo_core::runtime::NoncompactLoopAction::WaitInflight => {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "profile_inflight_saturated",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("profile", &candidate_name),
                        runtime_proxy_log_field(
                            "hard_limit",
                            shared
                                .runtime_config
                                .tuning
                                .profile_inflight_hard_limit
                                .to_string(),
                        ),
                    ],
                ),
            );
            loop_state.record_inflight_saturation();
            let _ = runtime_proxy_maybe_wait_for_interactive_inflight_relief(
                RuntimeInflightReliefWait {
                    observed_release_revision: None,
                    request_id,
                    shared,
                    excluded_profiles: &loop_state.excluded_profiles,
                    route_kind: RuntimeRouteKind::Standard,
                    selection_started_at: &mut loop_state.selection_started_at,
                    continuation: session_present,
                    wait_affinity_owner,
                    selected_profile: None,
                },
            )?;
            Ok(RuntimePrecommitLoopAction::Continue)
        }
        _ => Err(anyhow::anyhow!(
            "Mojo noncompact loop returned an invalid candidate admission action"
        )),
    }
}
