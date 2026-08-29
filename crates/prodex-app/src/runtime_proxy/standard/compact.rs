use super::super::{
    RuntimeInflightReliefWait, RuntimeInflightReliefWaitResult, RuntimeResponseCandidateSelection,
    await_runtime_proxy_async_task, build_runtime_proxy_json_error_response,
    clear_runtime_recovered_profiles, mark_runtime_profile_retry_backoff,
    runtime_compact_route_followup_bound_profile, runtime_profile_recovery_wait_for_route,
    runtime_proxy_current_profile, runtime_proxy_local_capacity_timeout_message, runtime_proxy_log,
    runtime_proxy_maybe_wait_for_interactive_inflight_relief,
    runtime_proxy_precommit_budget_exhausted_for_route,
    runtime_proxy_pressure_mode_active_for_route, runtime_proxy_probe_refresh_pause,
    runtime_proxy_should_shed_fresh_compact_request,
    runtime_remaining_sync_probe_cold_start_profiles_for_route,
    runtime_request_previous_response_id, runtime_request_session_id, runtime_request_turn_state,
    runtime_response_bound_profile, runtime_session_bound_profile,
    runtime_smart_context_model_name_from_body, runtime_turn_state_affinity_profile,
    select_runtime_response_candidate_for_route_with_request,
};
use super::attempt_runtime_standard_request;
use crate::runtime_proxy_shared::RuntimeStandardAttempt;
use crate::runtime_state_shared::{RuntimeRotationProxyShared, RuntimeRouteKind};
use crate::shared_types::RuntimeProxyRequest;
use anyhow::Result;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
mod admission;
mod affinity;
mod auth;
mod commit;
mod fallback;
mod flow;
mod logging;
mod retryable;
mod transport;
use admission::{
    build_runtime_fresh_compact_pressure_response, log_runtime_compact_inflight_saturated,
    log_runtime_compact_local_capacity_timeout, log_runtime_compact_local_selection_blocked,
    runtime_compact_candidate_inflight_saturated,
};
use affinity::runtime_compact_route_candidate_has_hard_affinity;
use auth::{RuntimeProxyCompactAuthFailure, handle_runtime_proxy_compact_auth_failure};
use commit::commit_runtime_proxy_compact_success;
use fallback::RuntimeProxyCompactSelectionExhausted;
use fallback::finish_runtime_proxy_compact_selection_exhausted;
use flow::RuntimeCompactFailureFlow;
use logging::log_runtime_proxy_compact_candidate;
use retryable::RuntimeProxyCompactRetryableFailure;
use retryable::handle_runtime_proxy_compact_retryable_failure;
use transport::RuntimeProxyCompactTransportFailure;
use transport::finish_runtime_proxy_compact_transport_failure;

pub(super) fn proxy_runtime_compact_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    let request_model_name = runtime_smart_context_model_name_from_body(&request.body);
    let request_previous_response_id = runtime_request_previous_response_id(request);
    let request_session_id = runtime_request_session_id(request);
    let request_turn_state = runtime_request_turn_state(request);
    let previous_response_profile = request_previous_response_id
        .as_deref()
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Compact)
        })
        .transpose()?
        .flatten();
    let session_profile = request_session_id
        .as_deref()
        .map(|session_id| runtime_session_bound_profile(shared, session_id))
        .transpose()?
        .flatten();
    let current_profile = runtime_proxy_current_profile(shared)?;
    let mut compact_followup_profile = runtime_compact_route_followup_bound_profile(
        shared,
        request_turn_state.as_deref(),
        request_session_id.as_deref(),
    )?;
    if compact_followup_profile.is_none() {
        compact_followup_profile = runtime_turn_state_affinity_profile(
            shared,
            request_turn_state.as_deref(),
            previous_response_profile
                .as_deref()
                .or(session_profile.as_deref()),
        )?
        .map(|profile_name| (profile_name, "turn_state"));
    }
    logging::log_runtime_proxy_compact_followup_owner(
        request_id,
        shared,
        compact_followup_profile.as_ref(),
    );
    let initial_compact_affinity_profile = previous_response_profile
        .as_deref()
        .or(compact_followup_profile
            .as_ref()
            .map(|(profile_name, _)| profile_name.as_str()))
        .or(session_profile.as_deref());
    let compact_owner_profile = previous_response_profile
        .clone()
        .or_else(|| {
            compact_followup_profile
                .as_ref()
                .map(|(profile_name, _)| profile_name.clone())
        })
        .or_else(|| session_profile.clone())
        .unwrap_or_else(|| current_profile.clone());
    let pressure_mode =
        runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Compact);
    let selection_started_at = Instant::now();
    let selection_attempts = 0usize;
    if runtime_proxy_should_shed_fresh_compact_request(
        pressure_mode,
        initial_compact_affinity_profile,
    ) {
        return Ok(build_runtime_fresh_compact_pressure_response(
            request_id,
            shared,
            selection_attempts,
            selection_started_at,
            pressure_mode,
        ));
    }
    let excluded_profiles = BTreeSet::new();
    let auto_redeemed_profiles = BTreeSet::new();
    let conservative_overload_retried_profiles = BTreeSet::new();
    let last_failure: Option<(tiny_http::ResponseBox, bool)> = None;
    let saw_inflight_saturation = false;
    let saw_transport_failure = false;
    run_runtime_compact_selection(RuntimeCompactSelectionContext {
        request_id,
        request,
        shared,
        request_model_name,
        request_previous_response_id,
        request_session_id,
        request_turn_state,
        previous_response_profile,
        current_profile,
        compact_followup_profile,
        session_profile,
        compact_owner_profile,
        pressure_mode,
        selection_started_at,
        selection_attempts,
        excluded_profiles,
        auto_redeemed_profiles,
        conservative_overload_retried_profiles,
        last_failure,
        saw_inflight_saturation,
        saw_transport_failure,
        saw_overload_failure: false,
        recovery_sweeps: 0,
        recovery_started_at: None,
    })
}

struct RuntimeCompactSelectionContext<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeRotationProxyShared,
    request_model_name: Option<String>,
    request_previous_response_id: Option<String>,
    request_session_id: Option<String>,
    request_turn_state: Option<String>,
    previous_response_profile: Option<String>,
    current_profile: String,
    compact_followup_profile: Option<(String, &'static str)>,
    session_profile: Option<String>,
    compact_owner_profile: String,
    pressure_mode: bool,
    selection_started_at: Instant,
    selection_attempts: usize,
    excluded_profiles: BTreeSet<String>,
    auto_redeemed_profiles: BTreeSet<String>,
    conservative_overload_retried_profiles: BTreeSet<String>,
    last_failure: Option<(tiny_http::ResponseBox, bool)>,
    saw_inflight_saturation: bool,
    saw_transport_failure: bool,
    saw_overload_failure: bool,
    recovery_sweeps: usize,
    recovery_started_at: Option<Instant>,
}

enum RuntimeCompactLoopAction {
    Continue,
    Attempt(String),
    Return(tiny_http::ResponseBox),
}

fn run_runtime_compact_selection(
    mut context: RuntimeCompactSelectionContext<'_>,
) -> Result<tiny_http::ResponseBox> {
    loop {
        if let Some(action) = context.budget_action()? {
            match action {
                RuntimeCompactLoopAction::Continue => continue,
                RuntimeCompactLoopAction::Attempt(_) => unreachable!(),
                RuntimeCompactLoopAction::Return(response) => return Ok(response),
            }
        }
        let action = context.next_action()?;
        let RuntimeCompactLoopAction::Attempt(candidate_name) = action else {
            return match action {
                RuntimeCompactLoopAction::Continue => continue,
                RuntimeCompactLoopAction::Return(response) => Ok(response),
                RuntimeCompactLoopAction::Attempt(_) => unreachable!(),
            };
        };
        if let Some(response) = context.attempt_candidate(candidate_name)? {
            return Ok(response);
        }
    }
}

impl RuntimeCompactSelectionContext<'_> {
    fn budget_action(&mut self) -> Result<Option<RuntimeCompactLoopAction>> {
        if !self.budget_exhausted()? {
            return Ok(None);
        }
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} transport=http compact_precommit_budget_exhausted attempts={} elapsed_ms={} pressure_mode={}",
                self.request_id,
                self.selection_attempts,
                self.selection_started_at.elapsed().as_millis(),
                self.pressure_mode
            ),
        );
        if self.can_wait_for_overload_recovery() && self.wait_for_overload_recovery()? {
            return Ok(Some(RuntimeCompactLoopAction::Continue));
        }
        Ok(Some(RuntimeCompactLoopAction::Return(self.finish(
            "precommit_budget_exhausted",
            "precommit_budget_exhausted_fallback",
        )?)))
    }

    fn budget_exhausted(&self) -> Result<bool> {
        let continuation = self.request_previous_response_id.is_some()
            || self.request_turn_state.is_some()
            || self.compact_followup_profile.is_some()
            || self.session_profile.is_some();
        let normal_budget_exhausted = runtime_proxy_precommit_budget_exhausted_for_route(
            self.shared,
            self.selection_started_at,
            self.selection_attempts,
            continuation,
            self.pressure_mode,
        )?;
        if self.recovery_sweeps == 0 {
            // Complete one bounded attempt/retry opportunity for every profile before the
            // elapsed-time budget can route to the current-profile last chance.
            let initial_sweep_attempt_limit = self
                .profile_count()?
                .saturating_mul(runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_ATTEMPTS_PER_PROFILE);
            if self.selection_attempts < initial_sweep_attempt_limit {
                return Ok(false);
            }
            return Ok(normal_budget_exhausted);
        }
        let (attempt_limit, _) =
            runtime_proxy_crate::runtime_proxy_precommit_budget_for_profile_count(
                continuation,
                self.pressure_mode,
                self.profile_count()?,
            );
        Ok(self.selection_attempts >= attempt_limit || self.recovery_budget_exhausted())
    }

    fn profile_count(&self) -> Result<usize> {
        Ok(self
            .shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
            .state
            .profiles
            .len()
            .max(1))
    }

    fn recovery_budget_exhausted(&self) -> bool {
        self.recovery_sweeps >= runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_SWEEP_LIMIT
            || self.recovery_started_at.is_some_and(|started_at| {
                started_at.elapsed()
                    >= Duration::from_millis(
                        runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS,
                    )
            })
    }

    fn next_action(&mut self) -> Result<RuntimeCompactLoopAction> {
        let candidate = select_runtime_response_candidate_for_route_with_request(
            self.shared,
            RuntimeResponseCandidateSelection {
                strict_affinity_profile: self
                    .compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                pinned_profile: self.previous_response_profile.as_deref(),
                session_profile: self.session_profile.as_deref(),
                discover_previous_response_owner: self.request_previous_response_id.is_some(),
                previous_response_id: self.request_previous_response_id.as_deref(),
                ..RuntimeResponseCandidateSelection::fresh(
                    &self.excluded_profiles,
                    RuntimeRouteKind::Compact,
                )
            },
            Some(self.request_id),
            self.request_model_name.as_deref(),
        )?;
        let Some(candidate) = candidate else {
            return self.no_candidate_action();
        };
        if self.excluded_profiles.contains(&candidate) {
            return Ok(RuntimeCompactLoopAction::Continue);
        }
        Ok(RuntimeCompactLoopAction::Attempt(candidate))
    }

    fn no_candidate_action(&mut self) -> Result<RuntimeCompactLoopAction> {
        runtime_proxy_log(
            self.shared,
            format!(
                "request={} transport=http compact_candidate_exhausted last_failure={}",
                self.request_id,
                if self.last_failure.is_some() {
                    "http"
                } else {
                    "none"
                }
            ),
        );
        let remaining_cold_start_profiles =
            runtime_remaining_sync_probe_cold_start_profiles_for_route(
                self.shared,
                &self.excluded_profiles,
                RuntimeRouteKind::Compact,
            )?;
        if remaining_cold_start_profiles > 0 && self.is_fresh_request() {
            runtime_proxy_log(
                self.shared,
                format!(
                    "request={} transport=http candidate_exhausted_continue route=compact remaining_cold_start_profiles={remaining_cold_start_profiles}",
                    self.request_id
                ),
            );
            runtime_proxy_probe_refresh_pause(self.shared, RuntimeRouteKind::Compact);
            return Ok(RuntimeCompactLoopAction::Continue);
        }
        if self.can_wait_for_overload_recovery() && self.wait_for_overload_recovery()? {
            return Ok(RuntimeCompactLoopAction::Continue);
        }
        Ok(RuntimeCompactLoopAction::Return(self.finish(
            "candidate_exhausted",
            "candidate_exhausted_fallback",
        )?))
    }

    fn attempt_candidate(
        &mut self,
        candidate_name: String,
    ) -> Result<Option<tiny_http::ResponseBox>> {
        log_runtime_proxy_compact_candidate(
            self.request_id,
            self.shared,
            &candidate_name,
            self.excluded_profiles.len(),
        );
        let candidate_has_hard_affinity = runtime_compact_route_candidate_has_hard_affinity(
            &candidate_name,
            &self.compact_followup_profile,
            self.previous_response_profile.as_deref(),
            self.session_profile.as_deref(),
        );
        if runtime_compact_candidate_inflight_saturated(
            self.request_id,
            self.shared,
            &candidate_name,
            candidate_has_hard_affinity,
        )? {
            self.saw_inflight_saturation = true;
            return match self
                .wait_for_inflight_relief(&candidate_name, candidate_has_hard_affinity)?
            {
                RuntimeInflightReliefWaitResult::Relieved
                | RuntimeInflightReliefWaitResult::NotWaitable => Ok(None),
                RuntimeInflightReliefWaitResult::DeadlineExpired => {
                    log_runtime_compact_local_capacity_timeout(
                        self.request_id,
                        self.shared,
                        self.selection_attempts,
                        self.selection_started_at,
                        self.pressure_mode,
                    );
                    Ok(Some(build_runtime_proxy_json_error_response(
                        503,
                        "local_capacity_timeout",
                        runtime_proxy_local_capacity_timeout_message(),
                    )))
                }
            };
        }
        let attempt = attempt_runtime_standard_request(
            self.request_id,
            self.request,
            self.shared,
            &candidate_name,
            candidate_has_hard_affinity,
            candidate_has_hard_affinity,
        )?;
        if matches!(
            &attempt,
            RuntimeStandardAttempt::ProfileInflightSaturated { .. }
        ) {
            self.saw_inflight_saturation = true;
            return match self
                .wait_for_inflight_relief(&candidate_name, candidate_has_hard_affinity)?
            {
                RuntimeInflightReliefWaitResult::Relieved
                | RuntimeInflightReliefWaitResult::NotWaitable => Ok(None),
                RuntimeInflightReliefWaitResult::DeadlineExpired => {
                    log_runtime_compact_local_capacity_timeout(
                        self.request_id,
                        self.shared,
                        self.selection_attempts,
                        self.selection_started_at,
                        self.pressure_mode,
                    );
                    Ok(Some(build_runtime_proxy_json_error_response(
                        503,
                        "local_capacity_timeout",
                        runtime_proxy_local_capacity_timeout_message(),
                    )))
                }
            };
        }
        if !matches!(
            &attempt,
            RuntimeStandardAttempt::LocalSelectionBlocked { .. }
        ) {
            self.selection_attempts = self.selection_attempts.saturating_add(1);
        }
        handle_runtime_compact_attempt(
            RuntimeCompactAttemptContext {
                request_id: self.request_id,
                shared: self.shared,
                candidate_has_hard_affinity,
                previous_response_profile: self.previous_response_profile.as_deref(),
                request_session_id: self.request_session_id.as_deref(),
                request_turn_state: self.request_turn_state.as_deref(),
                current_profile: &self.current_profile,
                compact_followup_profile: &mut self.compact_followup_profile,
                session_profile: &mut self.session_profile,
                auto_redeemed_profiles: &mut self.auto_redeemed_profiles,
                conservative_overload_retried_profiles: &mut self
                    .conservative_overload_retried_profiles,
                excluded_profiles: &mut self.excluded_profiles,
                last_failure: &mut self.last_failure,
                selection_attempts: self.selection_attempts,
                selection_started_at: self.selection_started_at,
                pressure_mode: self.pressure_mode,
                saw_inflight_saturation: &mut self.saw_inflight_saturation,
                saw_transport_failure: &mut self.saw_transport_failure,
                saw_overload_failure: &mut self.saw_overload_failure,
            },
            attempt,
        )
    }

    fn wait_for_inflight_relief(
        &self,
        candidate_name: &str,
        hard_affinity: bool,
    ) -> Result<RuntimeInflightReliefWaitResult> {
        runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait {
            request_id: self.request_id,
            request: self.request,
            shared: self.shared,
            excluded_profiles: &self.excluded_profiles,
            route_kind: RuntimeRouteKind::Compact,
            selection_started_at: self.selection_started_at,
            continuation: !self.is_fresh_request(),
            wait_affinity_owner: hard_affinity.then_some(candidate_name),
            selected_profile: None,
        })
    }

    fn is_fresh_request(&self) -> bool {
        self.request_previous_response_id.is_none()
            && self.request_turn_state.is_none()
            && self.compact_followup_profile.is_none()
            && self.session_profile.is_none()
    }

    fn can_wait_for_overload_recovery(&self) -> bool {
        self.request_previous_response_id.is_none()
            && self.request_turn_state.is_none()
            && self.request_session_id.is_none()
            && (self.saw_overload_failure || self.saw_transport_failure)
    }

    fn wait_for_overload_recovery(&mut self) -> Result<bool> {
        wait_for_compact_overload_recovery(
            self.request_id,
            self.shared,
            &mut self.excluded_profiles,
            &mut self.recovery_sweeps,
            &mut self.recovery_started_at,
        )
    }

    fn finish(
        &mut self,
        exit: &'static str,
        fallback_exit: &'static str,
    ) -> Result<tiny_http::ResponseBox> {
        finish_runtime_proxy_compact_selection_exhausted(
            RuntimeProxyCompactSelectionExhausted {
                request_id: self.request_id,
                request: self.request,
                shared: self.shared,
                compact_owner_profile: &self.compact_owner_profile,
                previous_response_id: self.request_previous_response_id.as_deref(),
                previous_response_profile: self.previous_response_profile.as_deref(),
                strict_affinity_profile: self
                    .compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                session_profile: self.session_profile.as_deref(),
                selection_attempts: self.selection_attempts,
                selection_started_at: self.selection_started_at,
                pressure_mode: self.pressure_mode,
                exit,
                fallback_exit,
                saw_transport_failure: self.saw_transport_failure,
            },
            self.last_failure.take(),
            self.saw_inflight_saturation,
        )
    }
}

struct RuntimeCompactAttemptContext<'a> {
    request_id: u64,
    shared: &'a RuntimeRotationProxyShared,
    candidate_has_hard_affinity: bool,
    previous_response_profile: Option<&'a str>,
    request_session_id: Option<&'a str>,
    request_turn_state: Option<&'a str>,
    current_profile: &'a str,
    compact_followup_profile: &'a mut Option<(String, &'static str)>,
    session_profile: &'a mut Option<String>,
    auto_redeemed_profiles: &'a mut BTreeSet<String>,
    conservative_overload_retried_profiles: &'a mut BTreeSet<String>,
    excluded_profiles: &'a mut BTreeSet<String>,
    last_failure: &'a mut Option<(tiny_http::ResponseBox, bool)>,
    selection_attempts: usize,
    selection_started_at: Instant,
    pressure_mode: bool,
    saw_inflight_saturation: &'a mut bool,
    saw_transport_failure: &'a mut bool,
    saw_overload_failure: &'a mut bool,
}

fn handle_runtime_compact_attempt(
    context: RuntimeCompactAttemptContext<'_>,
    attempt: RuntimeStandardAttempt,
) -> Result<Option<tiny_http::ResponseBox>> {
    let RuntimeCompactAttemptContext {
        request_id,
        shared,
        candidate_has_hard_affinity,
        previous_response_profile,
        request_session_id,
        request_turn_state,
        current_profile,
        compact_followup_profile,
        session_profile,
        auto_redeemed_profiles,
        conservative_overload_retried_profiles,
        excluded_profiles,
        last_failure,
        selection_attempts,
        selection_started_at,
        pressure_mode,
        saw_inflight_saturation,
        saw_transport_failure,
        saw_overload_failure,
    } = context;
    match attempt {
        RuntimeStandardAttempt::Success {
            profile_name,
            response,
        } => Ok(Some(commit_runtime_proxy_compact_success(
            request_id,
            shared,
            profile_name,
            response,
        )?)),
        RuntimeStandardAttempt::StaleContinuation { response } => Ok(Some(response)),
        RuntimeStandardAttempt::TransportFailed {
            profile_name,
            stage,
        } => {
            *saw_transport_failure = true;
            match finish_runtime_proxy_compact_transport_failure(
                RuntimeProxyCompactTransportFailure {
                    request_id,
                    shared,
                    profile_name: &profile_name,
                    stage,
                    hard_affinity: candidate_has_hard_affinity,
                    selection_attempts,
                    selection_started_at,
                    pressure_mode,
                    last_failure: last_failure.as_ref(),
                    saw_inflight_saturation: *saw_inflight_saturation,
                    saw_transport_failure: *saw_transport_failure,
                },
            ) {
                RuntimeCompactFailureFlow::Retry => {
                    excluded_profiles.insert(profile_name);
                    Ok(None)
                }
                RuntimeCompactFailureFlow::Return(response) => Ok(Some(response)),
            }
        }
        RuntimeStandardAttempt::RetryableFailure {
            profile_name,
            response,
            overload,
        } => {
            if overload {
                *saw_overload_failure = true;
            }
            match handle_runtime_proxy_compact_retryable_failure(
                RuntimeProxyCompactRetryableFailure {
                    request_id,
                    shared,
                    profile_name,
                    response,
                    overload,
                    previous_response_profile,
                    request_session_id,
                    request_turn_state,
                    current_profile,
                    compact_followup_profile,
                    session_profile,
                    auto_redeemed_profiles,
                    conservative_overload_retried_profiles,
                    excluded_profiles,
                    last_failure,
                    selection_attempts,
                    selection_started_at,
                    pressure_mode,
                    saw_inflight_saturation: *saw_inflight_saturation,
                    saw_transport_failure: *saw_transport_failure,
                },
            )? {
                RuntimeCompactFailureFlow::Retry => Ok(None),
                RuntimeCompactFailureFlow::Return(response) => Ok(Some(response)),
            }
        }
        RuntimeStandardAttempt::ProfileUnavailable {
            profile_name,
            response,
        } => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_profile_unavailable profile={profile_name}"
                ),
            );
            if candidate_has_hard_affinity {
                return Ok(Some(response));
            }
            mark_runtime_profile_retry_backoff(shared, &profile_name)?;
            excluded_profiles.insert(profile_name);
            *last_failure = Some((response, false));
            *saw_transport_failure = true;
            Ok(None)
        }
        RuntimeStandardAttempt::AuthFailed {
            profile_name,
            response,
        } => match handle_runtime_proxy_compact_auth_failure(RuntimeProxyCompactAuthFailure {
            request_id,
            shared,
            profile_name,
            response,
            hard_affinity: candidate_has_hard_affinity,
            request_session_id,
            request_turn_state,
            compact_followup_profile,
            session_profile,
            excluded_profiles,
            last_failure,
            selection_attempts,
            selection_started_at,
            pressure_mode,
            saw_inflight_saturation: *saw_inflight_saturation,
            saw_transport_failure: *saw_transport_failure,
        })? {
            RuntimeCompactFailureFlow::Retry => Ok(None),
            RuntimeCompactFailureFlow::Return(response) => Ok(Some(response)),
        },
        RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
            log_runtime_compact_local_selection_blocked(request_id, shared, &profile_name);
            excluded_profiles.insert(profile_name);
            Ok(None)
        }
        RuntimeStandardAttempt::ProfileInflightSaturated { profile_name } => {
            log_runtime_compact_inflight_saturated(request_id, shared, &profile_name);
            *saw_inflight_saturation = true;
            Ok(None)
        }
    }
}

fn wait_for_compact_overload_recovery(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &mut BTreeSet<String>,
    recovery_sweeps: &mut usize,
    recovery_started_at: &mut Option<Instant>,
) -> Result<bool> {
    if *recovery_sweeps >= runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_SWEEP_LIMIT {
        return Ok(false);
    }
    let recovery_started_at = *recovery_started_at.get_or_insert_with(Instant::now);
    if recovery_started_at.elapsed()
        >= Duration::from_millis(runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS)
    {
        return Ok(false);
    }
    let profile_count = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .state
        .profiles
        .len();
    if profile_count < 2 {
        return Ok(false);
    }
    let recovered = clear_runtime_recovered_profiles(
        shared,
        excluded_profiles,
        RuntimeRouteKind::Compact,
        true,
    )?;
    if recovered > 0 {
        *recovery_sweeps = recovery_sweeps.saturating_add(1);
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http rotation_sweep_start route=compact recovered_profiles={recovered} sweep={recovery_sweeps}"
            ),
        );
        return Ok(true);
    }
    let Some(until) =
        runtime_profile_recovery_wait_for_route(shared, RuntimeRouteKind::Compact, true)?
    else {
        return Ok(false);
    };
    let now = chrono::Local::now().timestamp();
    let remaining_budget =
        Duration::from_millis(runtime_proxy_crate::RUNTIME_PROXY_PRECOMMIT_RECOVERY_BUDGET_MS)
            .saturating_sub(recovery_started_at.elapsed());
    let wait = Duration::from_secs(u64::try_from(until.saturating_sub(now)).unwrap_or(0))
        .saturating_add(Duration::from_secs(1))
        .min(remaining_budget);
    if wait.is_zero() {
        return Ok(false);
    }
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http rotation_waiting_for_recovery route=compact wait_ms={} sweep={}",
            wait.as_millis(),
            recovery_sweeps.saturating_add(1)
        ),
    );
    await_runtime_proxy_async_task(shared, "profile_recovery_wait", async move {
        tokio::time::sleep(wait).await;
        Ok(())
    })?;
    let recovered = clear_runtime_recovered_profiles(
        shared,
        excluded_profiles,
        RuntimeRouteKind::Compact,
        true,
    )?;
    *recovery_sweeps = recovery_sweeps.saturating_add(1);
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http rotation_sweep_start route=compact recovered_profiles={recovered} sweep={recovery_sweeps}"
        ),
    );
    Ok(recovered > 0)
}
