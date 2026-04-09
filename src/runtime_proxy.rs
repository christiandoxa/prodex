use super::*;

impl Drop for RuntimeRotationProxy {
    fn drop(&mut self) {
        unregister_runtime_proxy_persistence_mode(&self.log_path);
        unregister_runtime_broker_metadata(&self.log_path);
        self.shutdown.store(true, Ordering::SeqCst);
        for _ in 0..self.accept_worker_count {
            self.server.unblock();
        }
        while let Some(worker) = self.worker_threads.pop() {
            let _ = worker.join();
        }
        let _ = self.owner_lock.take();
    }
}

pub(super) fn reject_runtime_proxy_overloaded_request(
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
    reason: &str,
) {
    let path = request.url().to_string();
    let websocket = is_tiny_http_websocket_upgrade(&request);
    runtime_proxy_log(
        shared,
        format!(
            "runtime_proxy_queue_overloaded transport={} path={} reason={reason}",
            if websocket { "websocket" } else { "http" },
            path
        ),
    );
    let response = if websocket {
        build_runtime_proxy_text_response(
            503,
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        )
    } else if is_runtime_anthropic_messages_path(&path) {
        build_runtime_proxy_response_from_parts(build_runtime_anthropic_error_parts(
            503,
            runtime_anthropic_error_type_for_status(503),
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        ))
    } else if is_runtime_responses_path(&path) || is_runtime_compact_path(&path) {
        build_runtime_proxy_json_error_response(
            503,
            "service_unavailable",
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        )
    } else {
        build_runtime_proxy_text_response(
            503,
            "Runtime auto-rotate proxy is temporarily saturated. Retry the request.",
        )
    };
    let _ = request.respond(response);
}

pub(super) fn mark_runtime_proxy_local_overload(shared: &RuntimeRotationProxyShared, reason: &str) {
    let now = Local::now().timestamp().max(0) as u64;
    let until = now.saturating_add(RUNTIME_PROXY_LOCAL_OVERLOAD_BACKOFF_SECONDS.max(1) as u64);
    let current = shared.local_overload_backoff_until.load(Ordering::SeqCst);
    if until > current {
        shared
            .local_overload_backoff_until
            .store(until, Ordering::SeqCst);
    }
    runtime_proxy_log(
        shared,
        format!("runtime_proxy_overload_backoff until={until} reason={reason}"),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeProxyAdmissionRejection {
    GlobalLimit,
    LaneLimit(RuntimeRouteKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeProxyQueueRejection {
    Full,
    Disconnected,
}

pub(super) fn runtime_proxy_request_lane(path: &str, websocket: bool) -> RuntimeRouteKind {
    if websocket {
        RuntimeRouteKind::Websocket
    } else if is_runtime_compact_path(path) {
        RuntimeRouteKind::Compact
    } else if is_runtime_responses_path(path) || is_runtime_anthropic_messages_path(path) {
        RuntimeRouteKind::Responses
    } else {
        RuntimeRouteKind::Standard
    }
}

pub(super) fn runtime_proxy_request_origin(headers: &[(String, String)]) -> Option<&str> {
    runtime_proxy_request_header_value(headers, PRODEX_INTERNAL_REQUEST_ORIGIN_HEADER)
}

pub(super) fn runtime_proxy_request_prefers_interactive_inflight_wait(
    request: &RuntimeProxyRequest,
) -> bool {
    runtime_proxy_request_origin(&request.headers).is_some_and(|origin| {
        origin.eq_ignore_ascii_case(PRODEX_INTERNAL_REQUEST_ORIGIN_ANTHROPIC_MESSAGES)
    })
}

pub(super) fn runtime_proxy_request_prefers_inflight_wait(request: &RuntimeProxyRequest) -> bool {
    is_runtime_responses_path(&request.path_and_query)
        || runtime_proxy_request_prefers_interactive_inflight_wait(request)
}

pub(super) fn runtime_proxy_request_inflight_wait_budget(
    request: &RuntimeProxyRequest,
    pressure_mode: bool,
) -> Duration {
    if runtime_proxy_request_prefers_interactive_inflight_wait(request) {
        runtime_proxy_admission_wait_budget(RUNTIME_PROXY_ANTHROPIC_MESSAGES_PATH, pressure_mode)
    } else if runtime_proxy_request_prefers_inflight_wait(request) {
        runtime_proxy_admission_wait_budget(&request.path_and_query, pressure_mode)
    } else {
        Duration::ZERO
    }
}

pub(super) fn runtime_proxy_request_is_long_lived(path: &str, websocket: bool) -> bool {
    websocket || is_runtime_responses_path(path) || is_runtime_anthropic_messages_path(path)
}

pub(super) fn runtime_proxy_interactive_wait_budget_ms(path: &str, base_budget_ms: u64) -> u64 {
    if is_runtime_anthropic_messages_path(path) {
        base_budget_ms.saturating_mul(RUNTIME_PROXY_INTERACTIVE_WAIT_MULTIPLIER)
    } else {
        base_budget_ms
    }
}

pub(super) fn runtime_proxy_admission_wait_budget(path: &str, pressure_mode: bool) -> Duration {
    let base_budget_ms = if pressure_mode {
        runtime_proxy_pressure_admission_wait_budget_ms()
    } else {
        runtime_proxy_admission_wait_budget_ms()
    };
    Duration::from_millis(runtime_proxy_interactive_wait_budget_ms(
        path,
        base_budget_ms,
    ))
}

pub(super) fn runtime_proxy_long_lived_queue_wait_budget(
    path: &str,
    pressure_mode: bool,
) -> Duration {
    let base_budget_ms = if pressure_mode {
        runtime_proxy_pressure_long_lived_queue_wait_budget_ms()
    } else {
        runtime_proxy_long_lived_queue_wait_budget_ms()
    };
    Duration::from_millis(runtime_proxy_interactive_wait_budget_ms(
        path,
        base_budget_ms,
    ))
}

pub(super) fn runtime_proxy_local_overload_pressure_active(
    shared: &RuntimeRotationProxyShared,
) -> bool {
    let now = Local::now().timestamp().max(0) as u64;
    shared.local_overload_backoff_until.load(Ordering::SeqCst) > now
}

pub(super) fn runtime_proxy_background_queue_pressure_active() -> bool {
    runtime_proxy_queue_pressure_active(
        runtime_state_save_queue_backlog(),
        runtime_continuation_journal_queue_backlog(),
        runtime_probe_refresh_queue_backlog(),
    )
}

pub(super) fn runtime_proxy_pressure_mode_active(shared: &RuntimeRotationProxyShared) -> bool {
    runtime_proxy_local_overload_pressure_active(shared)
        || runtime_proxy_background_queue_pressure_active()
}

pub(super) fn runtime_proxy_background_queue_pressure_affects_route(
    route_kind: RuntimeRouteKind,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard
    )
}

pub(super) fn runtime_proxy_pressure_mode_for_route(
    route_kind: RuntimeRouteKind,
    local_overload_pressure: bool,
    background_queue_pressure: bool,
) -> bool {
    local_overload_pressure
        || (background_queue_pressure
            && runtime_proxy_background_queue_pressure_affects_route(route_kind))
}

pub(super) fn runtime_proxy_pressure_mode_active_for_route(
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime_proxy_pressure_mode_for_route(
        route_kind,
        runtime_proxy_local_overload_pressure_active(shared),
        runtime_proxy_background_queue_pressure_active(),
    )
}

pub(super) fn runtime_proxy_pressure_mode_active_for_request_path(
    shared: &RuntimeRotationProxyShared,
    path: &str,
    websocket: bool,
) -> bool {
    runtime_proxy_pressure_mode_active_for_route(
        shared,
        runtime_proxy_request_lane(path, websocket),
    )
}

pub(super) fn runtime_proxy_sync_probe_pressure_mode_active(
    shared: &RuntimeRotationProxyShared,
) -> bool {
    runtime_proxy_local_overload_pressure_active(shared)
        || runtime_proxy_background_queue_pressure_active()
}

pub(super) fn runtime_proxy_lane_limit_marks_global_overload(lane: RuntimeRouteKind) -> bool {
    lane == RuntimeRouteKind::Responses
}

pub(super) fn runtime_proxy_should_shed_fresh_compact_request(
    pressure_mode: bool,
    session_profile: Option<&str>,
) -> bool {
    pressure_mode && session_profile.is_none()
}

pub(super) fn audit_runtime_proxy_startup_state(shared: &RuntimeRotationProxyShared) {
    let Ok(mut runtime) = shared.runtime.lock() else {
        return;
    };
    let now = Local::now().timestamp();
    let orphan_managed_dirs = collect_orphan_managed_profile_dirs(&runtime.paths, &runtime.state);
    let missing_managed_dirs = runtime
        .state
        .profiles
        .values()
        .filter(|profile| profile.managed && !profile.codex_home.exists())
        .count();
    let valid_profiles = runtime
        .state
        .profiles
        .iter()
        .filter(|(_, profile)| !profile.managed || profile.codex_home.exists())
        .map(|(name, _)| name.clone())
        .collect::<BTreeSet<_>>();
    let stale_response_bindings = runtime
        .state
        .response_profile_bindings
        .values()
        .filter(|binding| !valid_profiles.contains(&binding.profile_name))
        .count();
    let stale_session_bindings = runtime
        .state
        .session_profile_bindings
        .values()
        .filter(|binding| !valid_profiles.contains(&binding.profile_name))
        .count();
    let stale_probe_cache = runtime
        .profile_probe_cache
        .keys()
        .filter(|profile_name| !valid_profiles.contains(*profile_name))
        .count();
    let stale_usage_snapshots = runtime
        .profile_usage_snapshots
        .keys()
        .filter(|profile_name| !valid_profiles.contains(*profile_name))
        .count();
    let stale_retry_backoffs = runtime
        .profile_retry_backoff_until
        .keys()
        .filter(|profile_name| !valid_profiles.contains(*profile_name))
        .count();
    let stale_transport_backoffs = runtime
        .profile_transport_backoff_until
        .keys()
        .filter(|key| !runtime_profile_transport_backoff_key_valid(key, &valid_profiles))
        .count();
    let stale_route_circuits = runtime
        .profile_route_circuit_open_until
        .keys()
        .filter(|key| !valid_profiles.contains(runtime_profile_route_circuit_profile_name(key)))
        .count();
    let stale_health_scores = runtime
        .profile_health
        .keys()
        .filter(|key| !valid_profiles.contains(runtime_profile_score_profile_name(key)))
        .count();
    let active_profile_missing_dir = runtime
        .state
        .active_profile
        .as_deref()
        .and_then(|name| runtime.state.profiles.get(name))
        .is_some_and(|profile| profile.managed && !profile.codex_home.exists());

    runtime
        .state
        .response_profile_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .state
        .session_profile_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .turn_state_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .session_id_bindings
        .retain(|_, binding| valid_profiles.contains(&binding.profile_name));
    runtime
        .profile_probe_cache
        .retain(|profile_name, _| valid_profiles.contains(profile_name));
    runtime
        .profile_usage_snapshots
        .retain(|profile_name, _| valid_profiles.contains(profile_name));
    runtime
        .profile_retry_backoff_until
        .retain(|profile_name, _| valid_profiles.contains(profile_name));
    runtime
        .profile_transport_backoff_until
        .retain(|key, _| runtime_profile_transport_backoff_key_valid(key, &valid_profiles));
    runtime
        .profile_route_circuit_open_until
        .retain(|key, _| valid_profiles.contains(runtime_profile_route_circuit_profile_name(key)));
    runtime
        .profile_health
        .retain(|key, _| valid_profiles.contains(runtime_profile_score_profile_name(key)));
    let route_circuit_count_after_profile_prune = runtime.profile_route_circuit_open_until.len();
    prune_runtime_profile_route_circuits(&mut runtime, now);
    let expired_route_circuits = route_circuit_count_after_profile_prune
        .saturating_sub(runtime.profile_route_circuit_open_until.len());
    let changed = stale_response_bindings > 0
        || stale_session_bindings > 0
        || stale_probe_cache > 0
        || stale_usage_snapshots > 0
        || stale_retry_backoffs > 0
        || stale_transport_backoffs > 0
        || stale_route_circuits > 0
        || expired_route_circuits > 0
        || stale_health_scores > 0;
    runtime_proxy_log(
        shared,
        format!(
            "runtime_proxy_startup_audit missing_managed_dirs={missing_managed_dirs} orphan_managed_dirs={} stale_response_bindings={stale_response_bindings} stale_session_bindings={stale_session_bindings} stale_probe_cache={stale_probe_cache} stale_usage_snapshots={stale_usage_snapshots} stale_retry_backoffs={stale_retry_backoffs} stale_transport_backoffs={stale_transport_backoffs} stale_route_circuits={stale_route_circuits} expired_route_circuits={expired_route_circuits} stale_health_scores={stale_health_scores} active_profile_missing_dir={active_profile_missing_dir}",
            orphan_managed_dirs.len(),
        ),
    );
    if changed {
        schedule_runtime_state_save_from_runtime(shared, &runtime, "startup_audit");
    }
    drop(runtime);
}

pub(super) fn try_acquire_runtime_proxy_active_request_slot(
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
) -> Result<RuntimeProxyActiveRequestGuard, RuntimeProxyAdmissionRejection> {
    let lane = runtime_proxy_request_lane(path, transport == "websocket");
    let lane_active_count = shared.lane_admission.active_counter(lane);
    let lane_limit = shared.lane_admission.limit(lane);
    loop {
        let active = shared.active_request_count.load(Ordering::SeqCst);
        if active >= shared.active_request_limit {
            runtime_proxy_log(
                shared,
                format!(
                    "runtime_proxy_active_limit_reached transport={transport} path={path} active={active} limit={}",
                    shared.active_request_limit
                ),
            );
            return Err(RuntimeProxyAdmissionRejection::GlobalLimit);
        }
        let lane_active = lane_active_count.load(Ordering::SeqCst);
        if lane_active >= lane_limit {
            runtime_proxy_log(
                shared,
                format!(
                    "runtime_proxy_lane_limit_reached transport={transport} path={path} lane={} active={lane_active} limit={lane_limit}",
                    runtime_route_kind_label(lane)
                ),
            );
            return Err(RuntimeProxyAdmissionRejection::LaneLimit(lane));
        }
        if shared
            .active_request_count
            .compare_exchange(
                active,
                active.saturating_add(1),
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
        {
            if lane_active_count
                .compare_exchange(
                    lane_active,
                    lane_active.saturating_add(1),
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_ok()
            {
                return Ok(RuntimeProxyActiveRequestGuard {
                    active_request_count: Arc::clone(&shared.active_request_count),
                    lane_active_count,
                    wait: Arc::clone(&shared.lane_admission.wait),
                });
            }
            shared.active_request_count.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

pub(super) fn acquire_runtime_proxy_active_request_slot_with_wait(
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
) -> Result<RuntimeProxyActiveRequestGuard, RuntimeProxyAdmissionRejection> {
    let started_at = Instant::now();
    let pressure_mode =
        runtime_proxy_pressure_mode_active_for_request_path(shared, path, transport == "websocket");
    let budget = runtime_proxy_admission_wait_budget(path, pressure_mode);
    let mut waited = false;
    loop {
        match try_acquire_runtime_proxy_active_request_slot(shared, transport, path) {
            Ok(guard) => {
                if waited {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_admission_recovered transport={transport} path={path} waited_ms={}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                return Ok(guard);
            }
            Err(rejection) => {
                let elapsed = started_at.elapsed();
                if elapsed >= budget {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_admission_wait_exhausted transport={transport} path={path} waited_ms={} reason={} pressure_mode={pressure_mode}",
                            elapsed.as_millis(),
                            match rejection {
                                RuntimeProxyAdmissionRejection::GlobalLimit =>
                                    "active_request_limit",
                                RuntimeProxyAdmissionRejection::LaneLimit(lane) =>
                                    runtime_route_kind_label(lane),
                            }
                        ),
                    );
                    return Err(rejection);
                }
                if !waited {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_admission_wait_started transport={transport} path={path} budget_ms={} wait_timeout_ms={} reason={} pressure_mode={pressure_mode}",
                            budget.as_millis(),
                            budget.saturating_sub(elapsed).as_millis(),
                            match rejection {
                                RuntimeProxyAdmissionRejection::GlobalLimit =>
                                    "active_request_limit",
                                RuntimeProxyAdmissionRejection::LaneLimit(lane) =>
                                    runtime_route_kind_label(lane),
                            }
                        ),
                    );
                }
                waited = true;
                let (mutex, condvar) = &*shared.lane_admission.wait;
                let wait_guard = mutex
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if let Ok(guard) =
                    try_acquire_runtime_proxy_active_request_slot(shared, transport, path)
                {
                    return Ok(guard);
                }
                let wait_for = budget.saturating_sub(elapsed);
                if !wait_for.is_zero() {
                    let _ = condvar
                        .wait_timeout(wait_guard, wait_for)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
            }
        }
    }
}

pub(super) fn wait_for_runtime_proxy_queue_capacity<T, F>(
    mut item: T,
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
    mut try_enqueue: F,
) -> Result<(), (RuntimeProxyQueueRejection, T)>
where
    F: FnMut(T) -> Result<(), (RuntimeProxyQueueRejection, T)>,
{
    let started_at = Instant::now();
    let pressure_mode =
        runtime_proxy_pressure_mode_active_for_request_path(shared, path, transport == "websocket");
    let budget = runtime_proxy_long_lived_queue_wait_budget(path, pressure_mode);
    let mut waited = false;
    loop {
        match try_enqueue(item) {
            Ok(()) => {
                if waited {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_queue_recovered transport={transport} path={path} waited_ms={}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                return Ok(());
            }
            Err((RuntimeProxyQueueRejection::Full, returned_item)) => {
                item = returned_item;
                let elapsed = started_at.elapsed();
                if elapsed >= budget {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_queue_wait_exhausted transport={transport} path={path} waited_ms={} reason=long_lived_queue_full pressure_mode={pressure_mode}",
                            elapsed.as_millis()
                        ),
                    );
                    return Err((RuntimeProxyQueueRejection::Full, item));
                }
                if !waited {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "runtime_proxy_queue_wait_started transport={transport} path={path} budget_ms={} wait_timeout_ms={} reason=long_lived_queue_full pressure_mode={pressure_mode}",
                            budget.as_millis(),
                            budget.saturating_sub(elapsed).as_millis()
                        ),
                    );
                }
                waited = true;
                let (mutex, condvar) = &*shared.lane_admission.wait;
                let wait_guard = mutex
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                match try_enqueue(item) {
                    Ok(()) => return Ok(()),
                    Err((RuntimeProxyQueueRejection::Full, returned_item)) => {
                        item = returned_item;
                    }
                    Err((RuntimeProxyQueueRejection::Disconnected, returned_item)) => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "runtime_proxy_queue_wait_exhausted transport={transport} path={path} waited_ms={} reason=long_lived_queue_disconnected",
                                started_at.elapsed().as_millis()
                            ),
                        );
                        return Err((RuntimeProxyQueueRejection::Disconnected, returned_item));
                    }
                }
                let wait_for = budget.saturating_sub(elapsed);
                if !wait_for.is_zero() {
                    let _ = condvar
                        .wait_timeout(wait_guard, wait_for)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
                continue;
            }
            Err((RuntimeProxyQueueRejection::Disconnected, returned_item)) => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "runtime_proxy_queue_wait_exhausted transport={transport} path={path} waited_ms={} reason=long_lived_queue_disconnected",
                        started_at.elapsed().as_millis()
                    ),
                );
                return Err((RuntimeProxyQueueRejection::Disconnected, returned_item));
            }
        }
    }
}

pub(super) fn runtime_profile_inflight_release_revision(
    shared: &RuntimeRotationProxyShared,
) -> u64 {
    shared
        .lane_admission
        .inflight_release_revision
        .load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeProfileInFlightWaitOutcome {
    InflightRelease,
    OtherNotify,
    Timeout,
}

pub(super) fn runtime_profile_inflight_wait_outcome_label(
    outcome: RuntimeProfileInFlightWaitOutcome,
) -> &'static str {
    match outcome {
        RuntimeProfileInFlightWaitOutcome::InflightRelease => "inflight_release",
        RuntimeProfileInFlightWaitOutcome::OtherNotify => "other_notify",
        RuntimeProfileInFlightWaitOutcome::Timeout => "timeout",
    }
}

pub(super) fn runtime_profile_inflight_wait_outcome_since(
    shared: &RuntimeRotationProxyShared,
    timeout: Duration,
    observed_revision: u64,
) -> RuntimeProfileInFlightWaitOutcome {
    if timeout.is_zero() {
        return RuntimeProfileInFlightWaitOutcome::Timeout;
    }
    if runtime_profile_inflight_release_revision(shared) != observed_revision {
        return RuntimeProfileInFlightWaitOutcome::InflightRelease;
    }
    let (mutex, condvar) = &*shared.lane_admission.wait;
    let guard = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if runtime_profile_inflight_release_revision(shared) != observed_revision {
        return RuntimeProfileInFlightWaitOutcome::InflightRelease;
    }
    let (_guard, result) = condvar
        .wait_timeout(guard, timeout)
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if runtime_profile_inflight_release_revision(shared) != observed_revision {
        RuntimeProfileInFlightWaitOutcome::InflightRelease
    } else if result.timed_out() {
        RuntimeProfileInFlightWaitOutcome::Timeout
    } else {
        RuntimeProfileInFlightWaitOutcome::OtherNotify
    }
}

#[cfg(test)]
pub(super) fn wait_for_runtime_profile_inflight_relief_since(
    shared: &RuntimeRotationProxyShared,
    timeout: Duration,
    observed_revision: u64,
) -> bool {
    matches!(
        runtime_profile_inflight_wait_outcome_since(shared, timeout, observed_revision),
        RuntimeProfileInFlightWaitOutcome::InflightRelease
    )
}

#[allow(clippy::result_large_err)]
pub(super) fn enqueue_runtime_proxy_long_lived_request_with_wait(
    sender: &mpsc::SyncSender<tiny_http::Request>,
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) -> Result<(), (RuntimeProxyQueueRejection, tiny_http::Request)> {
    let path = request.url().to_string();
    let transport = if is_tiny_http_websocket_upgrade(&request) {
        "websocket"
    } else {
        "http"
    };
    wait_for_runtime_proxy_queue_capacity(request, shared, transport, &path, |request| match sender
        .try_send(request)
    {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(returned_request)) => {
            Err((RuntimeProxyQueueRejection::Full, returned_request))
        }
        Err(TrySendError::Disconnected(returned_request)) => {
            Err((RuntimeProxyQueueRejection::Disconnected, returned_request))
        }
    })
}

pub(super) fn handle_runtime_rotation_proxy_request(
    mut request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) {
    if let Some(response) = handle_runtime_proxy_admin_request(&mut request, shared) {
        let _ = request.respond(response);
        return;
    }
    if let Some(response) = handle_runtime_proxy_anthropic_compat_request(&request) {
        let _ = request.respond(response);
        return;
    }
    let request_path = request.url().to_string();
    let request_transport = if is_tiny_http_websocket_upgrade(&request) {
        "websocket"
    } else {
        "http"
    };
    let _active_request_guard = match acquire_runtime_proxy_active_request_slot_with_wait(
        shared,
        request_transport,
        &request_path,
    ) {
        Ok(guard) => guard,
        Err(RuntimeProxyAdmissionRejection::GlobalLimit) => {
            mark_runtime_proxy_local_overload(shared, "active_request_limit");
            reject_runtime_proxy_overloaded_request(request, shared, "active_request_limit");
            return;
        }
        Err(RuntimeProxyAdmissionRejection::LaneLimit(lane)) => {
            let reason = format!("lane_limit:{}", runtime_route_kind_label(lane));
            if runtime_proxy_lane_limit_marks_global_overload(lane) {
                mark_runtime_proxy_local_overload(shared, &reason);
            }
            reject_runtime_proxy_overloaded_request(request, shared, &reason);
            return;
        }
    };
    let request_id = runtime_proxy_next_request_id(shared);
    if is_tiny_http_websocket_upgrade(&request) {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket upgrade path={}",
                request.url()
            ),
        );
        proxy_runtime_responses_websocket_request(request_id, request, shared);
        return;
    }

    let captured = match capture_runtime_proxy_request(&mut request) {
        Ok(captured) => captured,
        Err(err) => {
            runtime_proxy_log(
                shared,
                format!("request={request_id} transport=http capture_error={err}"),
            );
            let _ = request.respond(build_runtime_proxy_text_response(502, &err.to_string()));
            return;
        }
    };
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http path={} previous_response_id={:?} turn_state={:?} body_bytes={}",
            captured.path_and_query,
            runtime_request_previous_response_id(&captured),
            runtime_request_turn_state(&captured),
            captured.body.len()
        ),
    );
    if is_runtime_anthropic_messages_path(&captured.path_and_query)
        && std::env::var_os("PRODEX_DEBUG_ANTHROPIC_COMPAT").is_some()
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http anthropic_compat headers={:?} body_snippet={}",
                captured.headers,
                runtime_proxy_body_snippet(&captured.body, 1024),
            ),
        );
    }

    if is_runtime_anthropic_messages_path(&captured.path_and_query) {
        let response = match proxy_runtime_anthropic_messages_request(request_id, &captured, shared)
        {
            Ok(response) => response,
            Err(err) => {
                if is_runtime_proxy_transport_failure(&err) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http anthropic_transport_failure={err:#}"
                        ),
                    );
                    return;
                } else {
                    runtime_proxy_log(
                        shared,
                        format!("request={request_id} transport=http anthropic_error={err:#}"),
                    );
                    RuntimeResponsesReply::Buffered(build_runtime_anthropic_error_parts(
                        502,
                        "api_error",
                        &err.to_string(),
                    ))
                }
            }
        };
        respond_runtime_responses_reply(request, response);
        return;
    }

    if is_runtime_responses_path(&captured.path_and_query) {
        let response = match proxy_runtime_responses_request(request_id, &captured, shared) {
            Ok(response) => response,
            Err(err) => {
                if is_runtime_proxy_transport_failure(&err) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http responses_transport_failure={err:#}"
                        ),
                    );
                    return;
                } else {
                    runtime_proxy_log(
                        shared,
                        format!("request={request_id} transport=http responses_error={err:#}"),
                    );
                    RuntimeResponsesReply::Buffered(build_runtime_proxy_text_response_parts(
                        502,
                        &err.to_string(),
                    ))
                }
            }
        };
        respond_runtime_responses_reply(request, response);
        return;
    }

    let response = match proxy_runtime_standard_request(request_id, &captured, shared) {
        Ok(response) => response,
        Err(err) => {
            if is_runtime_proxy_transport_failure(&err) {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http standard_transport_failure={err:#}"
                    ),
                );
                return;
            } else {
                runtime_proxy_log(
                    shared,
                    format!("request={request_id} transport=http standard_error={err:#}"),
                );
                build_runtime_proxy_text_response(502, &err.to_string())
            }
        }
    };
    let _ = request.respond(response);
}

pub(super) fn respond_runtime_responses_reply(
    request: tiny_http::Request,
    response: RuntimeResponsesReply,
) {
    match response {
        RuntimeResponsesReply::Buffered(parts) => {
            let _ = request.respond(build_runtime_proxy_response_from_parts(parts));
        }
        RuntimeResponsesReply::Streaming(response) => {
            let writer = request.into_writer();
            let _ = write_runtime_streaming_response(writer, response);
        }
    }
}

pub(super) fn is_tiny_http_websocket_upgrade(request: &tiny_http::Request) -> bool {
    request.headers().iter().any(|header| {
        header.field.equiv("Upgrade") && header.value.as_str().eq_ignore_ascii_case("websocket")
    })
}

pub(super) fn capture_runtime_proxy_request(
    request: &mut tiny_http::Request,
) -> Result<RuntimeProxyRequest> {
    let mut body = Vec::new();
    request
        .as_reader()
        .read_to_end(&mut body)
        .context("failed to read proxied Codex request body")?;

    Ok(RuntimeProxyRequest {
        method: request.method().as_str().to_string(),
        path_and_query: request.url().to_string(),
        headers: runtime_proxy_request_headers(request),
        body,
    })
}

pub(super) fn capture_runtime_proxy_websocket_request(
    request: &tiny_http::Request,
) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: request.method().as_str().to_string(),
        path_and_query: request.url().to_string(),
        headers: runtime_proxy_request_headers(request),
        body: Vec::new(),
    }
}

pub(super) fn runtime_proxy_request_headers(request: &tiny_http::Request) -> Vec<(String, String)> {
    request
        .headers()
        .iter()
        .map(|header| {
            (
                header.field.as_str().as_str().to_string(),
                header.value.as_str().to_string(),
            )
        })
        .collect()
}

pub(super) fn runtime_request_previous_response_id(
    request: &RuntimeProxyRequest,
) -> Option<String> {
    runtime_request_previous_response_id_from_bytes(&request.body)
}

#[derive(Clone, Default)]
pub(super) struct RuntimeWebsocketRequestMetadata {
    pub(super) previous_response_id: Option<String>,
    pub(super) session_id: Option<String>,
    pub(super) requires_previous_response_affinity: bool,
}

pub(super) fn parse_runtime_websocket_request_metadata(
    request_text: &str,
) -> RuntimeWebsocketRequestMetadata {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(request_text) else {
        return RuntimeWebsocketRequestMetadata::default();
    };
    RuntimeWebsocketRequestMetadata {
        previous_response_id: runtime_request_previous_response_id_from_value(&value),
        session_id: runtime_request_session_id_from_value(&value),
        requires_previous_response_affinity:
            runtime_request_value_requires_previous_response_affinity(&value),
    }
}

pub(super) fn runtime_request_previous_response_id_from_bytes(body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }

    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    runtime_request_previous_response_id_from_value(&value)
}

#[cfg(test)]
#[cfg(test)]
#[cfg(test)]
pub(super) fn runtime_request_previous_response_id_from_text(request_text: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(request_text).ok()?;
    runtime_request_previous_response_id_from_value(&value)
}

pub(super) fn runtime_request_previous_response_id_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    value
        .get("previous_response_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn runtime_request_without_previous_response_id(
    request: &RuntimeProxyRequest,
) -> Option<RuntimeProxyRequest> {
    let mut value = serde_json::from_slice::<serde_json::Value>(&request.body).ok()?;
    let object = value.as_object_mut()?;
    let removed = object.remove("previous_response_id")?;
    if removed.as_str().map(str::trim).is_none_or(str::is_empty) {
        return None;
    }
    let body = serde_json::to_vec(&value).ok()?;
    Some(RuntimeProxyRequest {
        method: request.method.clone(),
        path_and_query: request.path_and_query.clone(),
        headers: request.headers.clone(),
        body,
    })
}

pub(super) fn runtime_request_without_turn_state_header(
    request: &RuntimeProxyRequest,
) -> RuntimeProxyRequest {
    RuntimeProxyRequest {
        method: request.method.clone(),
        path_and_query: request.path_and_query.clone(),
        headers: request
            .headers
            .iter()
            .filter(|(name, _)| !name.eq_ignore_ascii_case("x-codex-turn-state"))
            .cloned()
            .collect(),
        body: request.body.clone(),
    }
}

pub(super) fn runtime_request_without_previous_response_affinity(
    request: &RuntimeProxyRequest,
) -> Option<RuntimeProxyRequest> {
    let request = runtime_request_without_previous_response_id(request)?;
    Some(runtime_request_without_turn_state_header(&request))
}

pub(super) fn runtime_request_value_requires_previous_response_affinity(
    value: &serde_json::Value,
) -> bool {
    value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                let Some(object) = item.as_object() else {
                    return false;
                };
                let item_type = object
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let has_call_id = object
                    .get("call_id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|call_id| !call_id.trim().is_empty());
                has_call_id && item_type.ends_with("_call_output")
            })
        })
}

pub(super) fn runtime_request_requires_previous_response_affinity(
    request: &RuntimeProxyRequest,
) -> bool {
    serde_json::from_slice::<serde_json::Value>(&request.body)
        .map(|value| runtime_request_value_requires_previous_response_affinity(&value))
        .unwrap_or(false)
}

pub(super) fn runtime_request_text_without_previous_response_id(
    request_text: &str,
) -> Option<String> {
    let mut value = serde_json::from_str::<serde_json::Value>(request_text).ok()?;
    let object = value.as_object_mut()?;
    let removed = object.remove("previous_response_id")?;
    if removed.as_str().map(str::trim).is_none_or(str::is_empty) {
        return None;
    }
    serde_json::to_string(&value).ok()
}

pub(super) fn runtime_request_turn_state(request: &RuntimeProxyRequest) -> Option<String> {
    request.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("x-codex-turn-state")
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn runtime_request_session_id_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("client_metadata")
                .and_then(|metadata| metadata.get("session_id"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn runtime_request_session_id_from_turn_metadata(
    request: &RuntimeProxyRequest,
) -> Option<String> {
    request
        .headers
        .iter()
        .find_map(|(name, value)| {
            name.eq_ignore_ascii_case("x-codex-turn-metadata")
                .then_some(value.as_str())
        })
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .and_then(|value| runtime_request_session_id_from_value(&value))
}

pub(super) fn runtime_request_session_id(request: &RuntimeProxyRequest) -> Option<String> {
    request
        .headers
        .iter()
        .find_map(|(name, value)| {
            name.eq_ignore_ascii_case("session_id")
                .then(|| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| runtime_request_session_id_from_turn_metadata(request))
        .or_else(|| {
            serde_json::from_slice::<serde_json::Value>(&request.body)
                .ok()
                .and_then(|value| runtime_request_session_id_from_value(&value))
        })
}

pub(super) fn runtime_binding_touch_should_persist(bound_at: i64, now: i64) -> bool {
    // These timestamps are stored with second precision. Require strictly more
    // than the interval so a boundary-crossing lookup does not persist nearly a
    // second early.
    now.saturating_sub(bound_at) > RUNTIME_BINDING_TOUCH_PERSIST_INTERVAL_SECONDS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeContinuationBindingKind {
    Response,
    TurnState,
    SessionId,
}

pub(super) fn runtime_continuation_status_map(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
) -> &BTreeMap<String, RuntimeContinuationBindingStatus> {
    match kind {
        RuntimeContinuationBindingKind::Response => &statuses.response,
        RuntimeContinuationBindingKind::TurnState => &statuses.turn_state,
        RuntimeContinuationBindingKind::SessionId => &statuses.session_id,
    }
}

pub(super) fn runtime_continuation_status_map_mut(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
) -> &mut BTreeMap<String, RuntimeContinuationBindingStatus> {
    match kind {
        RuntimeContinuationBindingKind::Response => &mut statuses.response,
        RuntimeContinuationBindingKind::TurnState => &mut statuses.turn_state,
        RuntimeContinuationBindingKind::SessionId => &mut statuses.session_id,
    }
}

pub(super) fn runtime_continuation_next_event_at(
    status: &RuntimeContinuationBindingStatus,
    now: i64,
) -> i64 {
    runtime_continuation_status_last_event_at(status)
        .filter(|last| *last >= now)
        .map_or(now, |last| last.saturating_add(1))
}

pub(super) fn runtime_continuation_status_touches(
    status: &mut RuntimeContinuationBindingStatus,
    now: i64,
) -> bool {
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.last_touched_at = Some(event_at);
    if status.state == RuntimeContinuationBindingLifecycle::Suspect {
        if status.last_not_found_at.is_some_and(|last| {
            event_at.saturating_sub(last) >= RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS
        }) {
            status.state = RuntimeContinuationBindingLifecycle::Warm;
            status.not_found_streak = 0;
            status.last_not_found_at = None;
        }
        status.confidence = status
            .confidence
            .saturating_add(RUNTIME_CONTINUATION_TOUCH_CONFIDENCE_BONUS)
            .min(RUNTIME_CONTINUATION_CONFIDENCE_MAX);
    } else if status.state != RuntimeContinuationBindingLifecycle::Dead {
        status.confidence = status
            .confidence
            .saturating_add(RUNTIME_CONTINUATION_TOUCH_CONFIDENCE_BONUS)
            .min(RUNTIME_CONTINUATION_CONFIDENCE_MAX);
    }
    *status != previous
}

pub(super) fn runtime_mark_continuation_status_touched(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    runtime_continuation_status_touches(status, now)
}

pub(super) fn runtime_continuation_status_should_refresh_verified(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    verified_route: Option<RuntimeRouteKind>,
) -> bool {
    let Some(status) = runtime_continuation_status_map(statuses, kind).get(key) else {
        return true;
    };

    if status.state != RuntimeContinuationBindingLifecycle::Verified {
        return true;
    }

    let verified_route_label = verified_route.map(runtime_route_kind_label);
    if status.last_verified_route.as_deref() != verified_route_label {
        return true;
    }

    status
        .last_verified_at
        .is_none_or(|last_verified_at| runtime_binding_touch_should_persist(last_verified_at, now))
}

pub(super) fn runtime_continuation_status_should_persist_touch(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    let Some(status) = runtime_continuation_status_map(statuses, kind).get(key) else {
        return true;
    };

    if status.state == RuntimeContinuationBindingLifecycle::Suspect
        && status.last_not_found_at.is_some_and(|last_not_found_at| {
            now.saturating_sub(last_not_found_at) >= RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS
        })
    {
        return true;
    }

    status
        .last_touched_at
        .is_none_or(|last_touched_at| runtime_binding_touch_should_persist(last_touched_at, now))
}

pub(super) fn runtime_mark_continuation_status_verified(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
    verified_route: Option<RuntimeRouteKind>,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.state = RuntimeContinuationBindingLifecycle::Verified;
    status.last_touched_at = Some(event_at);
    status.last_verified_at = Some(event_at);
    status.last_verified_route =
        verified_route.map(|route_kind| runtime_route_kind_label(route_kind).to_string());
    status.last_not_found_at = None;
    status.not_found_streak = 0;
    status.success_count = status.success_count.saturating_add(1);
    status.failure_count = 0;
    status.confidence = status
        .confidence
        .saturating_add(RUNTIME_CONTINUATION_VERIFIED_CONFIDENCE_BONUS)
        .min(RUNTIME_CONTINUATION_CONFIDENCE_MAX);
    *status != previous
}

pub(super) fn runtime_mark_continuation_status_suspect(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.not_found_streak = status.not_found_streak.saturating_add(1);
    status.last_touched_at = Some(event_at);
    status.last_not_found_at = Some(event_at);
    status.failure_count = status.failure_count.saturating_add(1);
    let previous_confidence = status.confidence;
    status.confidence = status
        .confidence
        .saturating_sub(RUNTIME_CONTINUATION_SUSPECT_CONFIDENCE_PENALTY);
    if previous_confidence == 0 {
        status.confidence = 1;
    }
    status.state = if status.not_found_streak >= RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT
        || (previous_confidence > 0 && status.confidence == 0)
    {
        RuntimeContinuationBindingLifecycle::Dead
    } else {
        RuntimeContinuationBindingLifecycle::Suspect
    };
    *status != previous
}

pub(super) fn runtime_mark_continuation_status_dead(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    let status = runtime_continuation_status_map_mut(statuses, kind)
        .entry(key.to_string())
        .or_default();
    let previous = status.clone();
    let event_at = runtime_continuation_next_event_at(&previous, now);
    status.state = RuntimeContinuationBindingLifecycle::Dead;
    status.confidence = 0;
    status.last_touched_at = Some(event_at);
    status.last_not_found_at = Some(event_at);
    status.not_found_streak = status
        .not_found_streak
        .max(RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT);
    status.failure_count = status.failure_count.saturating_add(1);
    *status != previous
}

pub(super) fn runtime_continuation_status_recently_suspect(
    statuses: &RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    runtime_continuation_status_map(statuses, kind)
        .get(key)
        .is_some_and(|status| {
            status.state == RuntimeContinuationBindingLifecycle::Suspect
                && !runtime_continuation_status_is_terminal(status)
                && status.last_not_found_at.is_some_and(|last| {
                    now.saturating_sub(last) < RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS
                })
        })
}

pub(super) fn runtime_continuation_status_label(
    status: &RuntimeContinuationBindingStatus,
) -> &'static str {
    match status.state {
        RuntimeContinuationBindingLifecycle::Warm => "warm",
        RuntimeContinuationBindingLifecycle::Verified => "verified",
        RuntimeContinuationBindingLifecycle::Suspect => "suspect",
        RuntimeContinuationBindingLifecycle::Dead => "dead",
    }
}

pub(super) fn runtime_compact_session_lineage_key(session_id: &str) -> String {
    format!("{RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX}{session_id}")
}

pub(super) fn runtime_compact_turn_state_lineage_key(turn_state: &str) -> String {
    format!("{RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX}{turn_state}")
}

pub(super) fn runtime_is_compact_session_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX)
}

pub(super) fn runtime_external_session_id_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    bindings
        .iter()
        .filter(|(key, _)| !runtime_is_compact_session_lineage_key(key))
        .map(|(key, binding)| (key.clone(), binding.clone()))
        .collect()
}

pub(super) fn runtime_touch_compact_lineage_binding(
    shared: &RuntimeRotationProxyShared,
    runtime: &mut RuntimeRotationState,
    key: &str,
    reason: &str,
    session_binding: bool,
) -> Option<String> {
    let now = Local::now().timestamp();
    let status_kind = if session_binding {
        RuntimeContinuationBindingKind::SessionId
    } else {
        RuntimeContinuationBindingKind::TurnState
    };
    if runtime_age_stale_verified_continuation_status(
        &mut runtime.continuation_statuses,
        status_kind,
        key,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity={} profile=- reason=continuation_stale key={key}",
                if session_binding {
                    "compact_session"
                } else {
                    "compact_turn_state"
                }
            ),
        );
        schedule_runtime_binding_touch_save(shared, runtime, &format!("continuation_stale:{key}"));
        return None;
    }
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        status_kind,
        key,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity={} profile=- reason=continuation_recent_suspect key={key}",
                if session_binding {
                    "compact_session"
                } else {
                    "compact_turn_state"
                }
            ),
        );
        return None;
    }
    if runtime_continuation_status_map(&runtime.continuation_statuses, status_kind)
        .get(key)
        .is_some_and(runtime_continuation_status_is_terminal)
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity={} profile=- reason=continuation_dead key={key}",
                if session_binding {
                    "compact_session"
                } else {
                    "compact_turn_state"
                }
            ),
        );
        return None;
    }
    let bindings = if session_binding {
        &mut runtime.session_id_bindings
    } else {
        &mut runtime.turn_state_bindings
    };
    let profile_name = bindings
        .get(key)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name));
    let mut persist_touch = false;
    if let Some(profile_name) = profile_name.as_deref()
        && let Some(binding) = bindings.get_mut(key)
        && binding.profile_name == profile_name
    {
        if runtime_binding_touch_should_persist(binding.bound_at, now) {
            persist_touch = true;
        }
        if binding.bound_at < now {
            binding.bound_at = now;
        }
        persist_touch = runtime_continuation_status_should_persist_touch(
            &runtime.continuation_statuses,
            status_kind,
            key,
            now,
        ) || persist_touch;
        let _ = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            status_kind,
            key,
            now,
        );
    }
    if persist_touch {
        schedule_runtime_binding_touch_save(shared, runtime, reason);
    }
    profile_name
}

pub(super) fn runtime_compact_followup_bound_profile(
    shared: &RuntimeRotationProxyShared,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<Option<(String, &'static str)>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    if let Some(turn_state) = turn_state.map(str::trim).filter(|value| !value.is_empty()) {
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        if let Some(profile_name) = runtime_touch_compact_lineage_binding(
            shared,
            &mut runtime,
            &key,
            &format!("compact_turn_state_touch:{turn_state}"),
            false,
        ) {
            return Ok(Some((profile_name, "turn_state")));
        }
    }
    if let Some(session_id) = session_id.map(str::trim).filter(|value| !value.is_empty()) {
        let key = runtime_compact_session_lineage_key(session_id);
        if let Some(profile_name) = runtime_touch_compact_lineage_binding(
            shared,
            &mut runtime,
            &key,
            &format!("compact_session_touch:{session_id}"),
            true,
        ) {
            return Ok(Some((profile_name, "session_id")));
        }
    }
    Ok(None)
}

pub(super) fn runtime_previous_response_negative_cache_key(
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__previous_response_not_found__:{}:{}:{profile_name}",
        runtime_route_kind_label(route_kind),
        previous_response_id
    )
}

pub(super) fn runtime_previous_response_negative_cache_failures(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> u32 {
    runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_previous_response_negative_cache_key(
            previous_response_id,
            profile_name,
            route_kind,
        ),
        now,
        RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS,
    )
}

pub(super) fn runtime_previous_response_negative_cache_active(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    previous_response_id: &str,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    runtime_previous_response_negative_cache_failures(
        profile_health,
        previous_response_id,
        profile_name,
        route_kind,
        now,
    ) > 0
}

pub(super) fn clear_runtime_previous_response_negative_cache(
    runtime: &mut RuntimeRotationState,
    previous_response_id: &str,
    profile_name: &str,
) -> bool {
    let mut changed = false;
    for route_kind in [
        RuntimeRouteKind::Responses,
        RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Compact,
        RuntimeRouteKind::Standard,
    ] {
        changed = runtime
            .profile_health
            .remove(&runtime_previous_response_negative_cache_key(
                previous_response_id,
                profile_name,
                route_kind,
            ))
            .is_some()
            || changed;
    }
    changed
}

pub(super) fn note_runtime_previous_response_not_found(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<u32> {
    let Some(previous_response_id) = previous_response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(0);
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_previous_response_negative_cache_key(
        previous_response_id,
        profile_name,
        route_kind,
    );
    let next_failures = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &key,
        now,
        RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_SECONDS,
    )
    .saturating_add(1)
    .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_failures,
            updated_at: now,
        },
    );
    let _ = runtime_mark_continuation_status_suspect(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        now,
    );
    runtime_proxy_log(
        shared,
        format!(
            "previous_response_negative_cache profile={profile_name} route={} response_id={} failures={next_failures}",
            runtime_route_kind_label(route_kind),
            previous_response_id,
        ),
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!(
            "previous_response_negative_cache:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    drop(runtime);
    if next_failures >= RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD {
        let _ = bump_runtime_profile_bad_pairing_score(
            shared,
            profile_name,
            route_kind,
            1,
            "previous_response_not_found",
        );
    }
    Ok(next_failures)
}

pub(super) fn schedule_runtime_binding_touch_save(
    shared: &RuntimeRotationProxyShared,
    runtime: &RuntimeRotationState,
    reason: &str,
) {
    schedule_runtime_state_save_from_runtime(shared, runtime, reason);
}

pub(super) fn runtime_response_bound_profile(
    shared: &RuntimeRotationProxyShared,
    previous_response_id: &str,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let profile_name = runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name));
    if runtime_age_stale_verified_continuation_status(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=previous_response profile={} reason=continuation_stale response_id={previous_response_id}",
                runtime_route_kind_label(route_kind),
                profile_name.as_deref().unwrap_or("-"),
            ),
        );
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            &format!("continuation_stale:{previous_response_id}"),
        );
        return Ok(None);
    }
    if runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
    )
    .get(previous_response_id)
    .is_some_and(runtime_continuation_status_is_terminal)
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=previous_response profile=- reason=continuation_dead response_id={previous_response_id}",
                runtime_route_kind_label(route_kind),
            ),
        );
        return Ok(None);
    }
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::Response,
        previous_response_id,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=previous_response profile=- reason=continuation_suspect response_id={previous_response_id}",
                runtime_route_kind_label(route_kind),
            ),
        );
        return Ok(None);
    }
    if let Some(profile_name) = profile_name.as_deref()
        && runtime_previous_response_negative_cache_active(
            &runtime.profile_health,
            previous_response_id,
            profile_name,
            route_kind,
            now,
        )
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=previous_response profile={} reason=negative_cache response_id={}",
                runtime_route_kind_label(route_kind),
                profile_name,
                previous_response_id,
            ),
        );
        return Ok(None);
    }
    let mut persist_touch = false;
    if let Some(profile_name) = profile_name.as_deref()
        && let Some(binding) = runtime
            .state
            .response_profile_bindings
            .get_mut(previous_response_id)
        && binding.profile_name == profile_name
    {
        if runtime_binding_touch_should_persist(binding.bound_at, now) {
            persist_touch = true;
        }
        if binding.bound_at < now {
            binding.bound_at = now;
        }
        persist_touch = runtime_continuation_status_should_persist_touch(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        ) || persist_touch;
        let _ = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        );
    }
    if persist_touch {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            &format!("response_touch:{previous_response_id}"),
        );
    }
    Ok(profile_name)
}

pub(super) fn runtime_turn_state_bound_profile(
    shared: &RuntimeRotationProxyShared,
    turn_state: &str,
) -> Result<Option<String>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let profile_name = runtime
        .turn_state_bindings
        .get(turn_state)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name));
    if runtime_age_stale_verified_continuation_status(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::TurnState,
        turn_state,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=responses affinity=turn_state profile={} reason=continuation_stale turn_state={turn_state}",
                profile_name.as_deref().unwrap_or("-"),
            ),
        );
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            &format!("continuation_stale:{turn_state}"),
        );
        return Ok(None);
    }
    if runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::TurnState,
    )
    .get(turn_state)
    .is_some_and(runtime_continuation_status_is_terminal)
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=responses affinity=turn_state profile=- reason=continuation_dead turn_state={turn_state}",
            ),
        );
        return Ok(None);
    }
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::TurnState,
        turn_state,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=responses affinity=turn_state profile=- reason=continuation_suspect turn_state={turn_state}",
            ),
        );
        return Ok(None);
    }
    let mut persist_touch = false;
    if let Some(profile_name) = profile_name.as_deref()
        && let Some(binding) = runtime.turn_state_bindings.get_mut(turn_state)
        && binding.profile_name == profile_name
    {
        if runtime_binding_touch_should_persist(binding.bound_at, now) {
            persist_touch = true;
        }
        if binding.bound_at < now {
            binding.bound_at = now;
        }
        persist_touch = runtime_continuation_status_should_persist_touch(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        ) || persist_touch;
        let _ = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        );
    }
    if persist_touch {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            &format!("turn_state_touch:{turn_state}"),
        );
    }
    Ok(profile_name)
}

pub(super) fn runtime_session_bound_profile(
    shared: &RuntimeRotationProxyShared,
    session_id: &str,
) -> Result<Option<String>> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let profile_name = runtime
        .session_id_bindings
        .get(session_id)
        .map(|binding| binding.profile_name.clone())
        .filter(|profile_name| runtime.state.profiles.contains_key(profile_name));
    if runtime_age_stale_verified_continuation_status(
        &mut runtime.continuation_statuses,
        RuntimeContinuationBindingKind::SessionId,
        session_id,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity=session_id profile={} reason=continuation_stale session_id={session_id}",
                profile_name.as_deref().unwrap_or("-"),
            ),
        );
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            &format!("continuation_stale:{session_id}"),
        );
        return Ok(None);
    }
    if runtime_continuation_status_map(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::SessionId,
    )
    .get(session_id)
    .is_some_and(runtime_continuation_status_is_terminal)
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity=session_id profile=- reason=continuation_dead session_id={session_id}",
            ),
        );
        return Ok(None);
    }
    if runtime_continuation_status_recently_suspect(
        &runtime.continuation_statuses,
        RuntimeContinuationBindingKind::SessionId,
        session_id,
        now,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route=compact affinity=session_id profile=- reason=continuation_suspect session_id={session_id}",
            ),
        );
        return Ok(None);
    }
    let mut persist_touch = false;
    if let Some(profile_name) = profile_name.as_deref() {
        if let Some(binding) = runtime.session_id_bindings.get_mut(session_id)
            && binding.profile_name == profile_name
        {
            if runtime_binding_touch_should_persist(binding.bound_at, now) {
                persist_touch = true;
            }
            if binding.bound_at < now {
                binding.bound_at = now;
            }
        }
        if let Some(binding) = runtime.state.session_profile_bindings.get_mut(session_id)
            && binding.profile_name == profile_name
        {
            if runtime_binding_touch_should_persist(binding.bound_at, now) {
                persist_touch = true;
            }
            if binding.bound_at < now {
                binding.bound_at = now;
            }
        }
        persist_touch = runtime_continuation_status_should_persist_touch(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        ) || persist_touch;
        let _ = runtime_mark_continuation_status_touched(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        );
    }
    if persist_touch {
        schedule_runtime_binding_touch_save(
            shared,
            &runtime,
            &format!("session_touch:{session_id}"),
        );
    }
    Ok(profile_name)
}

pub(super) fn remember_runtime_turn_state(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let Some(turn_state) = turn_state.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;
    let should_refresh_binding = match runtime.turn_state_bindings.get_mut(turn_state) {
        Some(binding) if binding.profile_name == profile_name => {
            if binding.bound_at < bound_at {
                binding.bound_at = bound_at;
            }
            false
        }
        Some(binding) => {
            binding.profile_name = profile_name.to_string();
            binding.bound_at = bound_at;
            changed = true;
            true
        }
        None => {
            runtime.turn_state_bindings.insert(
                turn_state.to_string(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                },
            );
            changed = true;
            true
        }
    };
    if should_refresh_binding
        || runtime_continuation_status_should_refresh_verified(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            bound_at,
            Some(verified_route),
        )
    {
        changed = runtime_mark_continuation_status_verified(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            bound_at,
            Some(verified_route),
        ) || changed;
    }
    if changed {
        prune_profile_bindings(
            &mut runtime.turn_state_bindings,
            TURN_STATE_PROFILE_BINDING_LIMIT,
        );
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("turn_state:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!("binding turn_state profile={profile_name} value={turn_state}"),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

pub(super) fn remember_runtime_session_id(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let Some(session_id) = session_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;
    let mut should_refresh_binding = false;
    match runtime.session_id_bindings.get_mut(session_id) {
        Some(binding) if binding.profile_name == profile_name => {
            if binding.bound_at < bound_at {
                binding.bound_at = bound_at;
            }
        }
        Some(binding) => {
            binding.profile_name = profile_name.to_string();
            binding.bound_at = bound_at;
            changed = true;
            should_refresh_binding = true;
        }
        None => {
            runtime.session_id_bindings.insert(
                session_id.to_string(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                },
            );
            changed = true;
            should_refresh_binding = true;
        }
    }
    match runtime.state.session_profile_bindings.get_mut(session_id) {
        Some(binding) if binding.profile_name == profile_name => {
            if binding.bound_at < bound_at {
                binding.bound_at = bound_at;
            }
        }
        Some(binding) => {
            binding.profile_name = profile_name.to_string();
            binding.bound_at = bound_at;
            changed = true;
            should_refresh_binding = true;
        }
        None => {
            runtime.state.session_profile_bindings.insert(
                session_id.to_string(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                },
            );
            changed = true;
            should_refresh_binding = true;
        }
    }
    if should_refresh_binding
        || runtime_continuation_status_should_refresh_verified(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            bound_at,
            Some(verified_route),
        )
    {
        changed = runtime_mark_continuation_status_verified(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            bound_at,
            Some(verified_route),
        ) || changed;
    }
    if changed {
        prune_profile_bindings(
            &mut runtime.session_id_bindings,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        );
        prune_profile_bindings(
            &mut runtime.state.session_profile_bindings,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        );
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("session_id:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!("binding session_id profile={profile_name} value={session_id}"),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

pub(super) fn remember_runtime_compact_lineage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
    turn_state: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let session_id = session_id.map(str::trim).filter(|value| !value.is_empty());
    let turn_state = turn_state.map(str::trim).filter(|value| !value.is_empty());
    if session_id.is_none() && turn_state.is_none() {
        return Ok(());
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;

    if let Some(session_id) = session_id {
        let key = runtime_compact_session_lineage_key(session_id);
        let should_refresh_binding = match runtime.session_id_bindings.get_mut(&key) {
            Some(binding) if binding.profile_name == profile_name => {
                if binding.bound_at < bound_at {
                    binding.bound_at = bound_at;
                }
                false
            }
            Some(binding) => {
                binding.profile_name = profile_name.to_string();
                binding.bound_at = bound_at;
                changed = true;
                true
            }
            None => {
                runtime.session_id_bindings.insert(
                    key.clone(),
                    ResponseProfileBinding {
                        profile_name: profile_name.to_string(),
                        bound_at,
                    },
                );
                changed = true;
                true
            }
        };
        if should_refresh_binding
            || runtime_continuation_status_should_refresh_verified(
                &runtime.continuation_statuses,
                RuntimeContinuationBindingKind::SessionId,
                &key,
                bound_at,
                Some(verified_route),
            )
        {
            changed = runtime_mark_continuation_status_verified(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::SessionId,
                &key,
                bound_at,
                Some(verified_route),
            ) || changed;
        }
    }

    if let Some(turn_state) = turn_state {
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        let should_refresh_binding = match runtime.turn_state_bindings.get_mut(&key) {
            Some(binding) if binding.profile_name == profile_name => {
                if binding.bound_at < bound_at {
                    binding.bound_at = bound_at;
                }
                false
            }
            Some(binding) => {
                binding.profile_name = profile_name.to_string();
                binding.bound_at = bound_at;
                changed = true;
                true
            }
            None => {
                runtime.turn_state_bindings.insert(
                    key.clone(),
                    ResponseProfileBinding {
                        profile_name: profile_name.to_string(),
                        bound_at,
                    },
                );
                changed = true;
                true
            }
        };
        if should_refresh_binding
            || runtime_continuation_status_should_refresh_verified(
                &runtime.continuation_statuses,
                RuntimeContinuationBindingKind::TurnState,
                &key,
                bound_at,
                Some(verified_route),
            )
        {
            changed = runtime_mark_continuation_status_verified(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::TurnState,
                &key,
                bound_at,
                Some(verified_route),
            ) || changed;
        }
    }

    if changed {
        prune_profile_bindings(
            &mut runtime.turn_state_bindings,
            TURN_STATE_PROFILE_BINDING_LIMIT,
        );
        prune_profile_bindings(
            &mut runtime.session_id_bindings,
            SESSION_ID_PROFILE_BINDING_LIMIT,
        );
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("compact_lineage:{profile_name}"),
        );
        drop(runtime);
    } else {
        drop(runtime);
    }
    Ok(())
}

pub(super) fn release_runtime_compact_lineage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    session_id: Option<&str>,
    turn_state: Option<&str>,
    reason: &str,
) -> Result<bool> {
    let session_id = session_id.map(str::trim).filter(|value| !value.is_empty());
    let turn_state = turn_state.map(str::trim).filter(|value| !value.is_empty());
    if session_id.is_none() && turn_state.is_none() {
        return Ok(false);
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut changed = false;
    let now = Local::now().timestamp();

    if let Some(session_id) = session_id {
        let key = runtime_compact_session_lineage_key(session_id);
        if runtime
            .session_id_bindings
            .get(&key)
            .is_some_and(|binding| binding.profile_name == profile_name)
        {
            runtime.session_id_bindings.remove(&key);
            let _ = runtime_mark_continuation_status_dead(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::SessionId,
                &key,
                now,
            );
            changed = true;
        }
    }

    if let Some(turn_state) = turn_state {
        let key = runtime_compact_turn_state_lineage_key(turn_state);
        if runtime
            .turn_state_bindings
            .get(&key)
            .is_some_and(|binding| binding.profile_name == profile_name)
        {
            runtime.turn_state_bindings.remove(&key);
            let _ = runtime_mark_continuation_status_dead(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::TurnState,
                &key,
                now,
            );
            changed = true;
        }
    }

    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("compact_lineage_release:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "compact_lineage_released profile={profile_name} reason={reason} session={} turn_state={}",
                session_id.unwrap_or("-"),
                turn_state.unwrap_or("-"),
            ),
        );
    } else {
        drop(runtime);
    }
    Ok(changed)
}

pub(super) fn remember_runtime_response_ids(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    response_ids: &[String],
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    if response_ids.is_empty() {
        return Ok(());
    }

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = false;
    for response_id in response_ids {
        changed =
            clear_runtime_previous_response_negative_cache(&mut runtime, response_id, profile_name)
                || changed;
        let should_refresh_binding =
            match runtime.state.response_profile_bindings.get_mut(response_id) {
                Some(binding) if binding.profile_name == profile_name => {
                    if binding.bound_at < bound_at {
                        binding.bound_at = bound_at;
                    }
                    false
                }
                Some(binding) => {
                    binding.profile_name = profile_name.to_string();
                    binding.bound_at = bound_at;
                    changed = true;
                    true
                }
                None => {
                    runtime.state.response_profile_bindings.insert(
                        response_id.clone(),
                        ResponseProfileBinding {
                            profile_name: profile_name.to_string(),
                            bound_at,
                        },
                    );
                    changed = true;
                    true
                }
            };
        if should_refresh_binding
            || runtime_continuation_status_should_refresh_verified(
                &runtime.continuation_statuses,
                RuntimeContinuationBindingKind::Response,
                response_id,
                bound_at,
                Some(verified_route),
            )
        {
            changed = runtime_mark_continuation_status_verified(
                &mut runtime.continuation_statuses,
                RuntimeContinuationBindingKind::Response,
                response_id,
                bound_at,
                Some(verified_route),
            ) || changed;
        }
    }
    if changed {
        prune_profile_bindings(
            &mut runtime.state.response_profile_bindings,
            RESPONSE_PROFILE_BINDING_LIMIT,
        );
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("response_ids:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "binding response_ids profile={profile_name} count={} first={:?}",
                response_ids.len(),
                response_ids.first()
            ),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

pub(super) fn remember_runtime_successful_previous_response_owner(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    verified_route: RuntimeRouteKind,
) -> Result<()> {
    let Some(previous_response_id) = previous_response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let bound_at = Local::now().timestamp();
    let mut changed = clear_runtime_previous_response_negative_cache(
        &mut runtime,
        previous_response_id,
        profile_name,
    );
    let should_refresh_binding = match runtime
        .state
        .response_profile_bindings
        .get_mut(previous_response_id)
    {
        Some(binding) if binding.profile_name == profile_name => {
            if binding.bound_at < bound_at {
                binding.bound_at = bound_at;
            }
            false
        }
        Some(binding) => {
            binding.profile_name = profile_name.to_string();
            binding.bound_at = bound_at;
            changed = true;
            true
        }
        None => {
            runtime.state.response_profile_bindings.insert(
                previous_response_id.to_string(),
                ResponseProfileBinding {
                    profile_name: profile_name.to_string(),
                    bound_at,
                },
            );
            changed = true;
            true
        }
    };
    if should_refresh_binding
        || runtime_continuation_status_should_refresh_verified(
            &runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            bound_at,
            Some(verified_route),
        )
    {
        changed = runtime_mark_continuation_status_verified(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            bound_at,
            Some(verified_route),
        ) || changed;
    }
    if changed {
        prune_profile_bindings(
            &mut runtime.state.response_profile_bindings,
            RESPONSE_PROFILE_BINDING_LIMIT,
        );
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("previous_response_owner:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "binding previous_response_owner profile={profile_name} response_id={previous_response_id}"
            ),
        );
    } else {
        drop(runtime);
    }
    Ok(())
}

pub(super) fn clear_runtime_stale_previous_response_binding(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
) -> Result<bool> {
    let Some(previous_response_id) = previous_response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(false);
    };

    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    if runtime
        .state
        .response_profile_bindings
        .get(previous_response_id)
        .is_none_or(|binding| binding.profile_name != profile_name)
    {
        drop(runtime);
        return Ok(false);
    }

    runtime
        .state
        .response_profile_bindings
        .remove(previous_response_id);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("previous_response_binding_clear:{profile_name}"),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "previous_response_binding_cleared profile={profile_name} response_id={previous_response_id}"
        ),
    );
    Ok(true)
}

pub(super) fn release_runtime_quota_blocked_affinity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut changed = false;
    let now = Local::now().timestamp();

    if let Some(previous_response_id) = previous_response_id
        && runtime
            .state
            .response_profile_bindings
            .get(previous_response_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime
            .state
            .response_profile_bindings
            .remove(previous_response_id);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        );
        changed = true;
    }

    if let Some(turn_state) = turn_state
        && runtime
            .turn_state_bindings
            .get(turn_state)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.turn_state_bindings.remove(turn_state);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        );
        changed = true;
    }

    if let Some(session_id) = session_id
        && runtime
            .session_id_bindings
            .get(session_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.session_id_bindings.remove(session_id);
        runtime.state.session_profile_bindings.remove(session_id);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        );
        changed = true;
    }

    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("quota_release:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "quota_release_affinity profile={profile_name} previous_response_id={:?} turn_state={:?} session_id={:?}",
                previous_response_id, turn_state, session_id
            ),
        );
    } else {
        drop(runtime);
    }

    Ok(changed)
}

pub(super) fn release_runtime_previous_response_affinity(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let previous_response_failures = note_runtime_previous_response_not_found(
        shared,
        profile_name,
        previous_response_id,
        route_kind,
    )?;
    if previous_response_failures < RUNTIME_PREVIOUS_RESPONSE_NEGATIVE_CACHE_FAILURE_THRESHOLD {
        runtime_proxy_log(
            shared,
            format!(
                "previous_response_release_deferred profile={profile_name} route={} previous_response_id={:?} failures={previous_response_failures}",
                runtime_route_kind_label(route_kind),
                previous_response_id,
            ),
        );
        return Ok(false);
    }
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut changed = false;
    let now = Local::now().timestamp();

    if let Some(previous_response_id) = previous_response_id
        && runtime
            .state
            .response_profile_bindings
            .get(previous_response_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime
            .state
            .response_profile_bindings
            .remove(previous_response_id);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::Response,
            previous_response_id,
            now,
        );
        changed = true;
    }

    if let Some(turn_state) = turn_state
        && runtime
            .turn_state_bindings
            .get(turn_state)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.turn_state_bindings.remove(turn_state);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::TurnState,
            turn_state,
            now,
        );
        changed = true;
    }

    if let Some(session_id) = session_id
        && runtime
            .session_id_bindings
            .get(session_id)
            .is_some_and(|binding| binding.profile_name == profile_name)
    {
        runtime.session_id_bindings.remove(session_id);
        runtime.state.session_profile_bindings.remove(session_id);
        let _ = runtime_mark_continuation_status_dead(
            &mut runtime.continuation_statuses,
            RuntimeContinuationBindingKind::SessionId,
            session_id,
            now,
        );
        changed = true;
    }

    if changed {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("previous_response_release:{profile_name}"),
        );
        drop(runtime);
        runtime_proxy_log(
            shared,
            format!(
                "previous_response_release_affinity profile={profile_name} previous_response_id={:?} turn_state={:?} session_id={:?}",
                previous_response_id, turn_state, session_id
            ),
        );
    } else {
        drop(runtime);
    }

    Ok(changed)
}

pub(super) fn prune_profile_bindings(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    max_entries: usize,
) {
    if bindings.len() <= max_entries {
        return;
    }

    let excess = bindings.len() - max_entries;
    let mut oldest = bindings
        .iter()
        .map(|(response_id, binding)| (response_id.clone(), binding.bound_at))
        .collect::<Vec<_>>();
    oldest.sort_by_key(|(_, bound_at)| *bound_at);

    for (response_id, _) in oldest.into_iter().take(excess) {
        bindings.remove(&response_id);
    }
}

pub(super) fn runtime_previous_response_retry_delay(retry_index: usize) -> Option<Duration> {
    RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS
        .get(retry_index)
        .copied()
        .map(Duration::from_millis)
}

pub(super) fn runtime_proxy_precommit_budget_exhausted(
    started_at: Instant,
    attempts: usize,
    continuation: bool,
    pressure_mode: bool,
) -> bool {
    let (attempt_limit, budget) = runtime_proxy_precommit_budget(continuation, pressure_mode);

    attempts >= attempt_limit || started_at.elapsed() >= budget
}

pub(super) fn runtime_proxy_precommit_budget(
    continuation: bool,
    pressure_mode: bool,
) -> (usize, Duration) {
    if continuation {
        (
            RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS),
        )
    } else if pressure_mode {
        (
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS),
        )
    } else {
        (
            RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_BUDGET_MS),
        )
    }
}

pub(super) fn runtime_proxy_has_continuation_priority(
    previous_response_id: Option<&str>,
    pinned_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
) -> bool {
    previous_response_id.is_some()
        || pinned_profile.is_some()
        || request_turn_state.is_some()
        || turn_state_profile.is_some()
        || session_profile.is_some()
}

pub(super) fn runtime_wait_affinity_owner<'a>(
    strict_affinity_profile: Option<&'a str>,
    pinned_profile: Option<&'a str>,
    turn_state_profile: Option<&'a str>,
    session_profile: Option<&'a str>,
    trusted_previous_response_affinity: bool,
) -> Option<&'a str> {
    strict_affinity_profile
        .or(turn_state_profile)
        .or_else(|| {
            trusted_previous_response_affinity
                .then_some(pinned_profile)
                .flatten()
        })
        .or(session_profile)
}

pub(super) fn runtime_proxy_allows_direct_current_profile_fallback(
    previous_response_id: Option<&str>,
    pinned_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    saw_inflight_saturation: bool,
    saw_upstream_failure: bool,
) -> bool {
    previous_response_id.is_none()
        && pinned_profile.is_none()
        && request_turn_state.is_none()
        && turn_state_profile.is_none()
        && session_profile.is_none()
        && !saw_inflight_saturation
        && !saw_upstream_failure
}

pub(super) fn runtime_profile_codex_home(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<Option<PathBuf>> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime
        .state
        .profiles
        .get(profile_name)
        .map(|profile| profile.codex_home.clone()))
}

pub(super) fn runtime_has_alternative_quota_compatible_profile(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<bool> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime.state.profiles.iter().any(|(name, profile)| {
        name != profile_name && read_auth_summary(&profile.codex_home).quota_compatible
    }))
}

pub(super) fn refresh_runtime_profile_quota_inline(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &str,
) -> Result<()> {
    let Some(codex_home) = runtime_profile_codex_home(shared, profile_name)? else {
        return Ok(());
    };
    runtime_proxy_log(shared, format!("{context}_start profile={profile_name}"));
    run_runtime_probe_jobs_inline(
        shared,
        vec![(profile_name.to_string(), codex_home)],
        context,
    );
    Ok(())
}

pub(super) fn runtime_quota_summary_requires_precommit_live_probe(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && !matches!(source, Some(RuntimeQuotaSource::LiveProbe))
        && (matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Critical)
            || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Critical)
            || matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
            || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown))
}

pub(super) fn runtime_quota_summary_requires_live_source_after_probe(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && !matches!(source, Some(RuntimeQuotaSource::LiveProbe))
        && (matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
            || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown))
}

pub(super) fn ensure_runtime_profile_precommit_quota_ready(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
) -> Result<(RuntimeQuotaSummary, Option<RuntimeQuotaSource>)> {
    let (mut quota_summary, mut quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
    if runtime_quota_summary_requires_precommit_live_probe(quota_summary, quota_source, route_kind)
    {
        refresh_runtime_profile_quota_inline(shared, profile_name, context)?;
        (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
    }
    Ok((quota_summary, quota_source))
}

pub(super) fn runtime_proxy_direct_current_fallback_profile(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let (profile_name, codex_home, auth_failure_active) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let profile_name = runtime.current_profile.clone();
        let Some(profile) = runtime.state.profiles.get(&profile_name) else {
            return Ok(None);
        };
        let now = Local::now().timestamp();
        (
            profile_name.clone(),
            profile.codex_home.clone(),
            runtime_profile_auth_failure_active(&runtime, &profile_name, now),
        )
    };
    if excluded_profiles.contains(&profile_name) {
        return Ok(None);
    }
    if auth_failure_active {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=auth_failure_backoff",
                runtime_route_kind_label(route_kind),
                profile_name,
            ),
        );
        return Ok(None);
    }
    if !read_auth_summary(&codex_home).quota_compatible {
        return Ok(None);
    }
    if runtime_profile_inflight_hard_limited_for_context(
        shared,
        &profile_name,
        runtime_route_kind_inflight_context(route_kind),
    )? {
        return Ok(None);
    }
    if matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) {
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, &profile_name, route_kind)?;
        if quota_source.is_none()
            || runtime_quota_summary_requires_live_source_after_probe(
                quota_summary,
                quota_source,
                route_kind,
            )
            || runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some()
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    profile_name,
                    runtime_quota_precommit_guard_reason(quota_summary, route_kind).unwrap_or_else(
                        || {
                            runtime_quota_soft_affinity_rejection_reason(
                                quota_summary,
                                quota_source,
                                route_kind,
                            )
                        }
                    ),
                    quota_source
                        .map(runtime_quota_source_label)
                        .unwrap_or("unknown"),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            return Ok(None);
        }
    }
    Ok(Some(profile_name))
}

pub(super) fn runtime_proxy_local_selection_failure_message() -> &'static str {
    "Runtime proxy could not secure a healthy upstream profile before the pre-commit retry budget was exhausted. Retry the request."
}

pub(super) fn runtime_quota_window_usable_for_auto_rotate(
    status: RuntimeQuotaWindowStatus,
) -> bool {
    matches!(
        status,
        RuntimeQuotaWindowStatus::Ready
            | RuntimeQuotaWindowStatus::Thin
            | RuntimeQuotaWindowStatus::Critical
    )
}

pub(super) fn runtime_quota_summary_allows_soft_affinity(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> bool {
    source.is_some()
        && runtime_quota_window_usable_for_auto_rotate(summary.five_hour.status)
        && runtime_quota_window_usable_for_auto_rotate(summary.weekly.status)
        && runtime_quota_precommit_guard_reason(summary, route_kind).is_none()
}

pub(super) fn runtime_quota_soft_affinity_rejection_reason(
    summary: RuntimeQuotaSummary,
    source: Option<RuntimeQuotaSource>,
    route_kind: RuntimeRouteKind,
) -> &'static str {
    if source.is_none()
        || matches!(summary.five_hour.status, RuntimeQuotaWindowStatus::Unknown)
        || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Unknown)
    {
        "quota_windows_unavailable"
    } else if let Some(reason) = runtime_quota_precommit_guard_reason(summary, route_kind) {
        reason
    } else if matches!(
        summary.five_hour.status,
        RuntimeQuotaWindowStatus::Exhausted
    ) || matches!(summary.weekly.status, RuntimeQuotaWindowStatus::Exhausted)
    {
        "quota_exhausted"
    } else {
        runtime_quota_pressure_band_reason(summary.route_band)
    }
}

pub(super) fn runtime_quota_source_sort_key(
    route_kind: RuntimeRouteKind,
    source: RuntimeQuotaSource,
) -> usize {
    match (route_kind, source) {
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeQuotaSource::LiveProbe,
        ) => 0,
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeQuotaSource::PersistedSnapshot,
        ) => 1,
        _ => 0,
    }
}

pub(super) fn runtime_profile_quota_summary_for_route(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<(RuntimeQuotaSummary, Option<RuntimeQuotaSource>)> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    Ok(runtime
        .profile_probe_cache
        .get(profile_name)
        .filter(|entry| runtime_profile_usage_cache_is_fresh(entry, now))
        .and_then(|entry| entry.result.as_ref().ok())
        .map(|usage| {
            (
                runtime_quota_summary_for_route(usage, route_kind),
                Some(RuntimeQuotaSource::LiveProbe),
            )
        })
        .or_else(|| {
            runtime
                .profile_usage_snapshots
                .get(profile_name)
                .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
                .map(|snapshot| {
                    (
                        runtime_quota_summary_from_usage_snapshot(snapshot, route_kind),
                        Some(RuntimeQuotaSource::PersistedSnapshot),
                    )
                })
        })
        .unwrap_or((
            RuntimeQuotaSummary {
                five_hour: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                weekly: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                route_band: RuntimeQuotaPressureBand::Unknown,
            },
            None,
        )))
}

pub(super) fn runtime_previous_response_affinity_is_trusted(
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
    .is_none_or(|status| status.state == RuntimeContinuationBindingLifecycle::Verified))
}

pub(super) fn runtime_previous_response_affinity_is_bound(
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

pub(super) fn runtime_candidate_has_hard_affinity(
    route_kind: RuntimeRouteKind,
    candidate_name: &str,
    strict_affinity_profile: Option<&str>,
    pinned_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    trusted_previous_response_affinity: bool,
) -> bool {
    strict_affinity_profile.is_some_and(|profile_name| profile_name == candidate_name)
        || turn_state_profile.is_some_and(|profile_name| profile_name == candidate_name)
        || (trusted_previous_response_affinity
            && pinned_profile.is_some_and(|profile_name| profile_name == candidate_name))
        || (route_kind == RuntimeRouteKind::Compact
            && session_profile.is_some_and(|profile_name| profile_name == candidate_name))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_quota_blocked_affinity_is_releasable(
    route_kind: RuntimeRouteKind,
    candidate_name: &str,
    strict_affinity_profile: Option<&str>,
    pinned_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    trusted_previous_response_affinity: bool,
    request_requires_previous_response_affinity: bool,
) -> bool {
    if strict_affinity_profile.is_some_and(|profile_name| profile_name == candidate_name)
        || turn_state_profile.is_some_and(|profile_name| profile_name == candidate_name)
        || (route_kind == RuntimeRouteKind::Compact
            && session_profile.is_some_and(|profile_name| profile_name == candidate_name))
    {
        return false;
    }

    if trusted_previous_response_affinity
        && pinned_profile.is_some_and(|profile_name| profile_name == candidate_name)
    {
        return !request_requires_previous_response_affinity;
    }

    true
}

pub(super) fn runtime_quota_precommit_floor_percent(route_kind: RuntimeRouteKind) -> i64 {
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => {
            runtime_proxy_responses_quota_critical_floor_percent()
        }
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 1,
    }
}

pub(super) fn runtime_quota_window_precommit_guard(
    window: RuntimeQuotaWindowSummary,
    floor_percent: i64,
) -> bool {
    matches!(
        window.status,
        RuntimeQuotaWindowStatus::Critical | RuntimeQuotaWindowStatus::Exhausted
    ) && window.remaining_percent <= floor_percent
}

pub(super) fn runtime_quota_precommit_guard_reason(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<&'static str> {
    let floor_percent = runtime_quota_precommit_floor_percent(route_kind);
    if summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        return Some("quota_exhausted_before_send");
    }

    if matches!(
        route_kind,
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
    ) && (runtime_quota_window_precommit_guard(summary.five_hour, floor_percent)
        || runtime_quota_window_precommit_guard(summary.weekly, floor_percent))
    {
        return Some("quota_critical_floor_before_send");
    }

    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    strict_affinity_profile: Option<&str>,
    pinned_profile: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    discover_previous_response_owner: bool,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    if let Some(profile_name) = strict_affinity_profile {
        if excluded_profiles.contains(profile_name) {
            return Ok(None);
        }
        if runtime_candidate_has_hard_affinity(
            route_kind,
            profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            false,
        ) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        let compact_followup_owner_without_probe = matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        ) && quota_source.is_none();
        if runtime_quota_summary_allows_soft_affinity(quota_summary, quota_source, route_kind)
            || compact_followup_owner_without_probe
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=compact_followup profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_soft_affinity_rejection_reason(
                    quota_summary,
                    quota_source,
                    route_kind,
                ),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }

    if let Some(profile_name) = pinned_profile.filter(|name| !excluded_profiles.contains(*name)) {
        if runtime_previous_response_affinity_is_bound(
            shared,
            previous_response_id,
            pinned_profile,
        )? {
            return Ok(Some(profile_name.to_string()));
        }
        if runtime_candidate_has_hard_affinity(
            route_kind,
            profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            runtime_previous_response_affinity_is_trusted(
                shared,
                previous_response_id,
                pinned_profile,
            )?,
        ) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        if quota_summary.route_band <= RuntimeQuotaPressureBand::Critical
            && runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_none()
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=pinned profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_pressure_band_reason(quota_summary.route_band),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if let Some(profile_name) = turn_state_profile.filter(|name| !excluded_profiles.contains(*name))
    {
        if runtime_candidate_has_hard_affinity(
            route_kind,
            profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            false,
        ) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        if quota_summary.route_band <= RuntimeQuotaPressureBand::Critical
            && runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_none()
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=turn_state profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_pressure_band_reason(quota_summary.route_band),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if discover_previous_response_owner {
        return next_runtime_previous_response_candidate(
            shared,
            excluded_profiles,
            previous_response_id,
            route_kind,
        );
    }

    if let Some(profile_name) = session_profile.filter(|name| !excluded_profiles.contains(*name)) {
        if runtime_candidate_has_hard_affinity(
            route_kind,
            profile_name,
            strict_affinity_profile,
            pinned_profile,
            turn_state_profile,
            session_profile,
            false,
        ) {
            return Ok(Some(profile_name.to_string()));
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, profile_name, route_kind)?;
        let compact_session_owner_without_probe =
            route_kind == RuntimeRouteKind::Compact && quota_source.is_none();
        let websocket_reuse_current_profile = route_kind == RuntimeRouteKind::Websocket
            && quota_source.is_none()
            && runtime_proxy_current_profile(shared)? == profile_name;
        if runtime_quota_summary_allows_soft_affinity(quota_summary, quota_source, route_kind)
            || compact_session_owner_without_probe
            || websocket_reuse_current_profile
        {
            return Ok(Some(profile_name.to_string()));
        }
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_affinity route={} affinity=session profile={} reason={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                profile_name,
                runtime_quota_soft_affinity_rejection_reason(
                    quota_summary,
                    quota_source,
                    route_kind,
                ),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }

    if let Some(profile_name) =
        runtime_proxy_optimistic_current_candidate_for_route(shared, excluded_profiles, route_kind)?
    {
        return Ok(Some(profile_name));
    }

    next_runtime_response_candidate_for_route(shared, excluded_profiles, route_kind)
}

pub(super) fn next_runtime_previous_response_candidate(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    previous_response_id: Option<&str>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let (state, current_profile, profile_health, profile_usage_auth) = {
        let runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.profile_health.clone(),
            runtime.profile_usage_auth.clone(),
        )
    };
    let now = Local::now().timestamp();
    if let Some(previous_response_id) = previous_response_id
        && let Some(binding) = state.response_profile_bindings.get(previous_response_id)
    {
        let owner = binding.profile_name.as_str();
        if !excluded_profiles.contains(owner)
            && state.profiles.contains_key(owner)
            && !runtime_previous_response_negative_cache_active(
                &profile_health,
                previous_response_id,
                owner,
                route_kind,
                now,
            )
        {
            return Ok(Some(owner.to_string()));
        }
    }

    for name in active_profile_selection_order(&state, &current_profile) {
        if excluded_profiles.contains(&name) {
            continue;
        }
        if let Some(previous_response_id) = previous_response_id
            && runtime_previous_response_negative_cache_active(
                &profile_health,
                previous_response_id,
                &name,
                route_kind,
                now,
            )
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason=negative_cache response_id={}",
                    runtime_route_kind_label(route_kind),
                    name,
                    previous_response_id,
                ),
            );
            continue;
        }
        let Some(profile) = state.profiles.get(&name) else {
            continue;
        };
        if !read_auth_summary(&profile.codex_home).quota_compatible {
            continue;
        }
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &name,
            now,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason=auth_failure_backoff",
                    runtime_route_kind_label(route_kind),
                    name,
                ),
            );
            continue;
        }
        let (quota_summary, quota_source) =
            runtime_profile_quota_summary_for_route(shared, &name, route_kind)?;
        if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted
            || runtime_quota_precommit_guard_reason(quota_summary, route_kind).is_some()
        {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_affinity route={} affinity=previous_response_discovery profile={} reason={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    name,
                    runtime_quota_precommit_guard_reason(quota_summary, route_kind).unwrap_or_else(
                        || runtime_quota_pressure_band_reason(quota_summary.route_band)
                    ),
                    quota_source
                        .map(runtime_quota_source_label)
                        .unwrap_or("unknown"),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        return Ok(Some(name));
    }
    Ok(None)
}

pub(super) fn runtime_proxy_optimistic_current_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let pressure_mode = runtime_proxy_pressure_mode_active(shared);
    let (
        current_profile,
        codex_home,
        has_alternative_quota_compatible_profile,
        in_selection_backoff,
        circuit_open_until,
        inflight_count,
        health_score,
        performance_score,
        auth_failure_active,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let now = Local::now().timestamp();
        prune_runtime_profile_selection_backoff(&mut runtime, now);

        if excluded_profiles.contains(&runtime.current_profile) {
            return Ok(None);
        }

        let Some(profile) = runtime.state.profiles.get(&runtime.current_profile) else {
            return Ok(None);
        };
        (
            runtime.current_profile.clone(),
            profile.codex_home.clone(),
            runtime.state.profiles.iter().any(|(name, profile)| {
                name != &runtime.current_profile
                    && read_auth_summary(&profile.codex_home).quota_compatible
            }),
            runtime_profile_in_selection_backoff(
                &runtime,
                &runtime.current_profile,
                route_kind,
                now,
            ),
            runtime_profile_route_circuit_open_until(
                &runtime,
                &runtime.current_profile,
                route_kind,
                now,
            ),
            runtime_profile_inflight_count(&runtime, &runtime.current_profile),
            runtime_profile_health_score(&runtime, &runtime.current_profile, now, route_kind),
            runtime_profile_route_performance_score(
                &runtime.profile_health,
                &runtime.current_profile,
                now,
                route_kind,
            ),
            runtime_profile_auth_failure_active_with_auth_cache(
                &runtime.profile_health,
                &runtime.profile_usage_auth,
                &runtime.current_profile,
                now,
            ),
        )
    };
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, &current_profile, route_kind)?;
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let quota_evidence_required =
        has_alternative_quota_compatible_profile && quota_source.is_none();
    let live_quota_probe_required = has_alternative_quota_compatible_profile
        && matches!(
            route_kind,
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket
        )
        && !matches!(quota_source, Some(RuntimeQuotaSource::LiveProbe));
    let unknown_quota_allowed = quota_summary.route_band == RuntimeQuotaPressureBand::Unknown
        && !has_alternative_quota_compatible_profile;
    let quota_band_blocks_current =
        quota_summary.route_band > RuntimeQuotaPressureBand::Healthy && !unknown_quota_allowed;

    if auth_failure_active
        || in_selection_backoff
        || circuit_open_until.is_some()
        || health_score > 0
        || performance_score > 0
        || quota_evidence_required
        || live_quota_probe_required
        || inflight_count >= inflight_soft_limit
        || quota_band_blocks_current
    {
        let reason = if auth_failure_active {
            "auth_failure_backoff"
        } else if in_selection_backoff {
            "selection_backoff"
        } else if circuit_open_until.is_some() {
            "route_circuit_open"
        } else if health_score > 0 {
            "profile_health"
        } else if performance_score > 0 {
            "profile_performance"
        } else if quota_evidence_required {
            "quota_probe_unavailable"
        } else if live_quota_probe_required {
            if matches!(quota_source, Some(RuntimeQuotaSource::PersistedSnapshot)) {
                "stale_persisted_quota"
            } else {
                "quota_probe_unavailable"
            }
        } else if quota_band_blocks_current {
            runtime_quota_pressure_band_reason(quota_summary.route_band)
        } else {
            "profile_inflight_soft_limit"
        };
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason={} inflight={} health={} performance={} soft_limit={} circuit_until={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                current_profile,
                reason,
                inflight_count,
                health_score,
                performance_score,
                inflight_soft_limit,
                circuit_open_until.unwrap_or_default(),
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }
    if !read_auth_summary(&codex_home).quota_compatible {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=auth_not_quota_compatible",
                runtime_route_kind_label(route_kind),
                current_profile
            ),
        );
        return Ok(None);
    }

    runtime_proxy_log(
        shared,
        format!(
            "selection_keep_current route={} profile={} inflight={} health={} performance={} quota_source={} {}",
            runtime_route_kind_label(route_kind),
            current_profile,
            inflight_count,
            health_score,
            performance_score,
            quota_source
                .map(runtime_quota_source_label)
                .unwrap_or("unknown"),
            runtime_quota_summary_log_fields(quota_summary),
        ),
    );
    if !reserve_runtime_profile_route_circuit_half_open_probe(shared, &current_profile, route_kind)?
    {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} performance={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                current_profile,
                inflight_count,
                health_score,
                performance_score,
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(None);
    }
    Ok(Some(current_profile))
}

pub(super) fn proxy_runtime_responses_websocket_request(
    request_id: u64,
    request: tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) {
    if !is_runtime_responses_path(request.url()) {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket unsupported_path={}",
                request.url()
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            404,
            "Runtime websocket proxy only supports Codex responses endpoints.",
        ));
        return;
    }

    let handshake_request = capture_runtime_proxy_websocket_request(&request);
    let Some(websocket_key) = runtime_proxy_websocket_key(&handshake_request) else {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket missing_sec_websocket_key path={}",
                handshake_request.path_and_query
            ),
        );
        let _ = request.respond(build_runtime_proxy_text_response(
            400,
            "Missing Sec-WebSocket-Key header for runtime auto-rotate websocket proxy.",
        ));
        return;
    };

    let response = build_runtime_proxy_websocket_upgrade_response(&websocket_key);
    let upgraded = request.upgrade("websocket", response);
    let mut local_socket = WsSocket::from_raw_socket(upgraded, WsRole::Server, None);
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=websocket upgraded path={} previous_response_id={:?} turn_state={:?}",
            handshake_request.path_and_query,
            runtime_request_previous_response_id(&handshake_request),
            runtime_request_turn_state(&handshake_request)
        ),
    );
    if let Err(err) = run_runtime_proxy_websocket_session(
        request_id,
        &mut local_socket,
        &handshake_request,
        shared,
    ) {
        runtime_proxy_log(
            shared,
            format!("request={request_id} transport=websocket session_error={err:#}"),
        );
        if !is_runtime_proxy_transport_failure(&err) {
            let _ = local_socket.close(None);
        }
    }
}

pub(super) fn runtime_proxy_websocket_key(request: &RuntimeProxyRequest) -> Option<String> {
    request.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("Sec-WebSocket-Key")
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn build_runtime_proxy_websocket_upgrade_response(
    key: &str,
) -> TinyResponse<std::io::Empty> {
    let accept = derive_accept_key(key.as_bytes());
    TinyResponse::new_empty(TinyStatusCode(101))
        .with_header(TinyHeader::from_bytes("Upgrade", "websocket").expect("upgrade header"))
        .with_header(TinyHeader::from_bytes("Connection", "Upgrade").expect("connection header"))
        .with_header(
            TinyHeader::from_bytes("Sec-WebSocket-Accept", accept.as_bytes())
                .expect("accept header"),
        )
}

#[derive(Default)]
pub(super) struct RuntimeWebsocketSessionState {
    upstream_socket: Option<RuntimeUpstreamWebSocket>,
    profile_name: Option<String>,
    turn_state: Option<String>,
    inflight_guard: Option<RuntimeProfileInFlightGuard>,
    last_terminal_at: Option<Instant>,
}

impl RuntimeWebsocketSessionState {
    fn can_reuse(&self, profile_name: &str, turn_state_override: Option<&str>) -> bool {
        self.upstream_socket.is_some()
            && self.profile_name.as_deref() == Some(profile_name)
            && turn_state_override.is_none_or(|value| self.turn_state.as_deref() == Some(value))
    }

    fn take_socket(&mut self) -> Option<RuntimeUpstreamWebSocket> {
        self.upstream_socket.take()
    }

    fn last_terminal_elapsed(&self) -> Option<Duration> {
        self.last_terminal_at.map(|timestamp| timestamp.elapsed())
    }

    fn store(
        &mut self,
        socket: RuntimeUpstreamWebSocket,
        profile_name: &str,
        turn_state: Option<String>,
        inflight_guard: Option<RuntimeProfileInFlightGuard>,
    ) {
        self.upstream_socket = Some(socket);
        self.profile_name = Some(profile_name.to_string());
        self.turn_state = turn_state;
        self.last_terminal_at = Some(Instant::now());
        if let Some(inflight_guard) = inflight_guard {
            self.inflight_guard = Some(inflight_guard);
        }
    }

    fn reset(&mut self) {
        self.upstream_socket = None;
        self.profile_name = None;
        self.turn_state = None;
        self.inflight_guard = None;
    }

    fn close(&mut self) {
        if let Some(mut socket) = self.upstream_socket.take() {
            let _ = socket.close(None);
        }
        self.profile_name = None;
        self.turn_state = None;
        self.inflight_guard = None;
    }
}

pub(super) fn acquire_runtime_profile_inflight_guard(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &'static str,
) -> Result<RuntimeProfileInFlightGuard> {
    let weight = runtime_profile_inflight_weight(context);
    let count = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        let count = runtime
            .profile_inflight
            .entry(profile_name.to_string())
            .or_insert(0);
        *count = count.saturating_add(weight);
        *count
    };
    runtime_proxy_log(
        shared,
        format!(
            "profile_inflight profile={profile_name} count={count} weight={weight} context={context} event=acquire"
        ),
    );
    Ok(RuntimeProfileInFlightGuard {
        shared: shared.clone(),
        profile_name: profile_name.to_string(),
        context,
        weight,
    })
}

pub(super) fn run_runtime_proxy_websocket_session(
    session_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<()> {
    let mut websocket_session = RuntimeWebsocketSessionState::default();
    loop {
        match local_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let message_id = runtime_proxy_next_request_id(shared);
                let request_metadata = parse_runtime_websocket_request_metadata(text.as_ref());
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={message_id} websocket_session={session_id} inbound_text previous_response_id={:?} turn_state={:?} bytes={}",
                        request_metadata.previous_response_id,
                        runtime_request_turn_state(handshake_request),
                        text.len()
                    ),
                );
                proxy_runtime_websocket_text_message(
                    session_id,
                    message_id,
                    local_socket,
                    handshake_request,
                    text.as_ref(),
                    &request_metadata,
                    shared,
                    &mut websocket_session,
                )?;
            }
            Ok(WsMessage::Binary(_)) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} inbound_binary_rejected"),
                );
                send_runtime_proxy_websocket_error(
                    local_socket,
                    400,
                    "invalid_request_error",
                    "Binary websocket messages are not supported by the runtime auto-rotate proxy.",
                )?;
            }
            Ok(WsMessage::Ping(payload)) => {
                local_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to runtime websocket ping")?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {}
            Ok(WsMessage::Close(frame)) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} local_close"),
                );
                websocket_session.close();
                let _ = local_socket.close(frame);
                break;
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} local_connection_closed"),
                );
                websocket_session.close();
                break;
            }
            Err(err) => {
                runtime_proxy_log(
                    shared,
                    format!("websocket_session={session_id} local_read_error={err}"),
                );
                websocket_session.close();
                return Err(anyhow::anyhow!(
                    "runtime websocket session ended unexpectedly: {err}"
                ));
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn proxy_runtime_websocket_text_message(
    session_id: u64,
    request_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    request_text: &str,
    request_metadata: &RuntimeWebsocketRequestMetadata,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
) -> Result<()> {
    let mut handshake_request = handshake_request.clone();
    let mut request_text = request_text.to_string();
    let request_requires_previous_response_affinity =
        request_metadata.requires_previous_response_affinity;
    let mut previous_response_id = request_metadata.previous_response_id.clone();
    let mut request_turn_state = runtime_request_turn_state(&handshake_request);
    let request_session_id = runtime_request_session_id(&handshake_request)
        .or_else(|| request_metadata.session_id.clone());
    let mut bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Websocket)
        })
        .transpose()?
        .flatten();
    let mut trusted_previous_response_affinity = runtime_previous_response_affinity_is_trusted(
        shared,
        previous_response_id.as_deref(),
        bound_profile.as_deref(),
    )?;
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut compact_followup_profile = if previous_response_id.is_none()
        && bound_profile.is_none()
        && turn_state_profile.is_none()
    {
        runtime_compact_followup_bound_profile(
            shared,
            request_turn_state.as_deref(),
            request_session_id.as_deref(),
        )?
    } else {
        None
    };
    if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} websocket_session={session_id} compact_followup_owner profile={profile_name} source={source}"
            ),
        );
    }
    let mut session_profile = if previous_response_id.is_none()
        && bound_profile.is_none()
        && turn_state_profile.is_none()
        && compact_followup_profile.is_none()
    {
        websocket_session.profile_name.clone().or(request_session_id
            .as_deref()
            .map(|session_id| runtime_session_bound_profile(shared, session_id))
            .transpose()?
            .flatten())
    } else {
        None
    };
    let mut pinned_profile = bound_profile.clone().or(compact_followup_profile
        .as_ref()
        .map(|(profile_name, _)| profile_name.clone()));
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;
    let mut saw_inflight_saturation = false;
    let mut selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    let mut previous_response_fresh_fallback_used = false;
    let mut saw_previous_response_not_found = false;
    let mut websocket_reuse_fresh_retry_profiles = BTreeSet::new();

    loop {
        let pressure_mode =
            runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Websocket);
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            runtime_proxy_has_continuation_priority(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
            ),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !request_requires_previous_response_affinity
                && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=precommit_budget_exhausted"
                    ),
                );
                request_text = fresh_request_text;
                handshake_request = runtime_request_without_turn_state_header(&handshake_request);
                previous_response_id = None;
                request_turn_state = None;
                previous_response_fresh_fallback_used = true;
                saw_previous_response_not_found = false;
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                candidate_turn_state_retry_profile = None;
                candidate_turn_state_retry_value = None;
                trusted_previous_response_affinity = false;
                bound_profile = None;
                pinned_profile = None;
                turn_state_profile = None;
                session_profile = None;
                websocket_reuse_fresh_retry_profiles.clear();
                excluded_profiles.clear();
                last_failure = None;
                selection_started_at = Instant::now();
                selection_attempts = 0;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} compact_fresh_fallback_blocked profile={profile_name} source={source} reason=precommit_budget_exhausted"
                    ),
                );
                match last_failure {
                    Some(RuntimeUpstreamFailureResponse::Websocket(payload)) => {
                        forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    }
                    _ if saw_inflight_saturation => {
                        send_runtime_proxy_websocket_error(
                            local_socket,
                            503,
                            "service_unavailable",
                            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                        )?;
                    }
                    _ => {
                        send_runtime_proxy_websocket_error(
                            local_socket,
                            503,
                            "service_unavailable",
                            runtime_proxy_local_selection_failure_message(),
                        )?;
                    }
                }
                return Ok(());
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Websocket,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} direct_current_profile_fallback profile={current_profile} reason=precommit_budget_exhausted"
                    ),
                );
                match attempt_runtime_websocket_request(
                    request_id,
                    local_socket,
                    &handshake_request,
                    &request_text,
                    previous_response_id.as_deref(),
                    request_session_id.as_deref(),
                    request_turn_state.as_deref(),
                    shared,
                    websocket_session,
                    &current_profile,
                    request_turn_state.as_deref(),
                )? {
                    RuntimeWebsocketAttempt::Delivered => return Ok(()),
                    RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name,
                        payload,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            RuntimeRouteKind::Websocket,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                            pinned_profile.as_deref(),
                            turn_state_profile.as_deref(),
                            session_profile.as_deref(),
                            trusted_previous_response_affinity,
                            request_requires_previous_response_affinity,
                        ) {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                            previous_response_retry_index = 0;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if previous_response_id.is_some()
                            && trusted_previous_response_affinity
                            && !previous_response_fresh_fallback_used
                            && !request_requires_previous_response_affinity
                            && let Some(fresh_request_text) =
                                runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            request_text = fresh_request_text;
                            handshake_request =
                                runtime_request_without_turn_state_header(&handshake_request);
                            previous_response_id = None;
                            request_turn_state = None;
                            previous_response_fresh_fallback_used = true;
                            saw_previous_response_not_found = false;
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                            trusted_previous_response_affinity = false;
                            bound_profile = None;
                            session_profile = None;
                            pinned_profile = None;
                            turn_state_profile = None;
                            websocket_reuse_fresh_retry_profiles.clear();
                            excluded_profiles.clear();
                            last_failure = None;
                            selection_started_at = Instant::now();
                            selection_attempts = 0;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                        continue;
                    }
                    RuntimeWebsocketAttempt::Overloaded {
                        profile_name,
                        payload,
                    } => {
                        let overload_message =
                            extract_runtime_proxy_overload_message_from_websocket_payload(&payload);
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} upstream_overloaded route=websocket profile={profile_name} via=direct_current_profile_fallback message={}",
                                overload_message.as_deref().unwrap_or("-"),
                            ),
                        );
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        let _ = bump_runtime_profile_health_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                            "websocket_overload",
                        );
                        let _ = bump_runtime_profile_bad_pairing_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                            "websocket_overload",
                        );
                        if !runtime_quota_blocked_affinity_is_releasable(
                            RuntimeRouteKind::Websocket,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                            pinned_profile.as_deref(),
                            turn_state_profile.as_deref(),
                            session_profile.as_deref(),
                            trusted_previous_response_affinity,
                            request_requires_previous_response_affinity,
                        ) {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} upstream_overload_passthrough route=websocket profile={profile_name} reason=hard_affinity via=direct_current_profile_fallback"
                                ),
                            );
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        if previous_response_id.is_some()
                            && trusted_previous_response_affinity
                            && !previous_response_fresh_fallback_used
                            && !request_requires_previous_response_affinity
                            && let Some(fresh_request_text) =
                                runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=upstream_overloaded via=direct_current_profile_fallback"
                                ),
                            );
                            request_text = fresh_request_text;
                            handshake_request =
                                runtime_request_without_turn_state_header(&handshake_request);
                            previous_response_id = None;
                            request_turn_state = None;
                            previous_response_fresh_fallback_used = true;
                            saw_previous_response_not_found = false;
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                            trusted_previous_response_affinity = false;
                            bound_profile = None;
                            session_profile = None;
                            pinned_profile = None;
                            turn_state_profile = None;
                            websocket_reuse_fresh_retry_profiles.clear();
                            excluded_profiles.clear();
                            last_failure = None;
                            selection_started_at = Instant::now();
                            selection_attempts = 0;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                        continue;
                    }
                    RuntimeWebsocketAttempt::PreviousResponseNotFound {
                        profile_name,
                        payload,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry && !request_requires_previous_response_affinity {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Websocket,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                            previous_response_retry_index = 0;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if compact_followup_profile
                            .as_ref()
                            .is_some_and(|(owner, _)| owner == &profile_name)
                        {
                            compact_followup_profile = None;
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                        continue;
                    }
                    RuntimeWebsocketAttempt::ReuseWatchdogTripped { profile_name, .. } => {
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                    RuntimeWebsocketAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            RuntimeRouteKind::Websocket,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                            pinned_profile.as_deref(),
                            turn_state_profile.as_deref(),
                            session_profile.as_deref(),
                            trusted_previous_response_affinity,
                            request_requires_previous_response_affinity,
                        ) {
                            send_runtime_proxy_websocket_error(
                                local_socket,
                                503,
                                "service_unavailable",
                                runtime_proxy_local_selection_failure_message(),
                            )?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                            previous_response_retry_index = 0;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if previous_response_id.is_some()
                            && trusted_previous_response_affinity
                            && !previous_response_fresh_fallback_used
                            && !request_requires_previous_response_affinity
                            && let Some(fresh_request_text) =
                                runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            request_text = fresh_request_text;
                            handshake_request =
                                runtime_request_without_turn_state_header(&handshake_request);
                            previous_response_id = None;
                            request_turn_state = None;
                            previous_response_fresh_fallback_used = true;
                            saw_previous_response_not_found = false;
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                            trusted_previous_response_affinity = false;
                            bound_profile = None;
                            session_profile = None;
                            pinned_profile = None;
                            turn_state_profile = None;
                            websocket_reuse_fresh_retry_profiles.clear();
                            excluded_profiles.clear();
                            last_failure = None;
                            selection_started_at = Instant::now();
                            selection_attempts = 0;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            match last_failure {
                Some(RuntimeUpstreamFailureResponse::Websocket(payload)) => {
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                }
                _ if saw_inflight_saturation => {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        503,
                        "service_unavailable",
                        "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                    )?;
                }
                _ => {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    )?;
                }
            }
            return Ok(());
        }

        let Some(candidate_name) = select_runtime_response_candidate_for_route(
            shared,
            &excluded_profiles,
            compact_followup_profile
                .as_ref()
                .map(|(profile_name, _)| profile_name.as_str()),
            pinned_profile.as_deref(),
            turn_state_profile.as_deref(),
            session_profile.as_deref(),
            previous_response_id.is_some(),
            previous_response_id.as_deref(),
            RuntimeRouteKind::Websocket,
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} candidate_exhausted last_failure={}",
                    match &last_failure {
                        Some(RuntimeUpstreamFailureResponse::Websocket(_)) => "websocket",
                        Some(RuntimeUpstreamFailureResponse::Http(_)) => "http",
                        None => "none",
                    }
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !request_requires_previous_response_affinity
                && let Some(fresh_request_text) =
                    runtime_request_text_without_previous_response_id(&request_text)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=candidate_exhausted"
                    ),
                );
                request_text = fresh_request_text;
                handshake_request = runtime_request_without_turn_state_header(&handshake_request);
                previous_response_id = None;
                request_turn_state = None;
                previous_response_fresh_fallback_used = true;
                saw_previous_response_not_found = false;
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                candidate_turn_state_retry_profile = None;
                candidate_turn_state_retry_value = None;
                trusted_previous_response_affinity = false;
                bound_profile = None;
                pinned_profile = None;
                turn_state_profile = None;
                session_profile = None;
                websocket_reuse_fresh_retry_profiles.clear();
                excluded_profiles.clear();
                last_failure = None;
                selection_started_at = Instant::now();
                selection_attempts = 0;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} compact_fresh_fallback_blocked profile={profile_name} source={source} reason=candidate_exhausted"
                    ),
                );
                match last_failure {
                    Some(RuntimeUpstreamFailureResponse::Websocket(payload)) => {
                        forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    }
                    _ if saw_inflight_saturation => {
                        send_runtime_proxy_websocket_error(
                            local_socket,
                            503,
                            "service_unavailable",
                            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                        )?;
                    }
                    _ => {
                        send_runtime_proxy_websocket_error(
                            local_socket,
                            503,
                            "service_unavailable",
                            runtime_proxy_local_selection_failure_message(),
                        )?;
                    }
                }
                return Ok(());
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Websocket,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} direct_current_profile_fallback profile={current_profile} reason=candidate_exhausted"
                    ),
                );
                match attempt_runtime_websocket_request(
                    request_id,
                    local_socket,
                    &handshake_request,
                    &request_text,
                    previous_response_id.as_deref(),
                    request_session_id.as_deref(),
                    request_turn_state.as_deref(),
                    shared,
                    websocket_session,
                    &current_profile,
                    request_turn_state.as_deref(),
                )? {
                    RuntimeWebsocketAttempt::Delivered => return Ok(()),
                    RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name,
                        payload,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            RuntimeRouteKind::Websocket,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                            pinned_profile.as_deref(),
                            turn_state_profile.as_deref(),
                            session_profile.as_deref(),
                            trusted_previous_response_affinity,
                            request_requires_previous_response_affinity,
                        ) {
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                            previous_response_retry_index = 0;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if previous_response_id.is_some()
                            && trusted_previous_response_affinity
                            && !previous_response_fresh_fallback_used
                            && !request_requires_previous_response_affinity
                            && let Some(fresh_request_text) =
                                runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            request_text = fresh_request_text;
                            handshake_request =
                                runtime_request_without_turn_state_header(&handshake_request);
                            previous_response_id = None;
                            request_turn_state = None;
                            previous_response_fresh_fallback_used = true;
                            saw_previous_response_not_found = false;
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                            trusted_previous_response_affinity = false;
                            bound_profile = None;
                            session_profile = None;
                            pinned_profile = None;
                            turn_state_profile = None;
                            websocket_reuse_fresh_retry_profiles.clear();
                            excluded_profiles.clear();
                            last_failure = None;
                            selection_started_at = Instant::now();
                            selection_attempts = 0;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                        continue;
                    }
                    RuntimeWebsocketAttempt::Overloaded {
                        profile_name,
                        payload,
                    } => {
                        let overload_message =
                            extract_runtime_proxy_overload_message_from_websocket_payload(&payload);
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} upstream_overloaded route=websocket profile={profile_name} via=direct_current_profile_fallback message={}",
                                overload_message.as_deref().unwrap_or("-"),
                            ),
                        );
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        let _ = bump_runtime_profile_health_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                            "websocket_overload",
                        );
                        let _ = bump_runtime_profile_bad_pairing_score(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Websocket,
                            RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                            "websocket_overload",
                        );
                        if !runtime_quota_blocked_affinity_is_releasable(
                            RuntimeRouteKind::Websocket,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                            pinned_profile.as_deref(),
                            turn_state_profile.as_deref(),
                            session_profile.as_deref(),
                            trusted_previous_response_affinity,
                            request_requires_previous_response_affinity,
                        ) {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} upstream_overload_passthrough route=websocket profile={profile_name} reason=hard_affinity via=direct_current_profile_fallback"
                                ),
                            );
                            forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                            return Ok(());
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                        continue;
                    }
                    RuntimeWebsocketAttempt::PreviousResponseNotFound {
                        profile_name,
                        payload,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry && !request_requires_previous_response_affinity {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Websocket,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if compact_followup_profile
                            .as_ref()
                            .is_some_and(|(owner, _)| owner == &profile_name)
                        {
                            compact_followup_profile = None;
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                        continue;
                    }
                    RuntimeWebsocketAttempt::ReuseWatchdogTripped { profile_name, .. } => {
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                    RuntimeWebsocketAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            RuntimeRouteKind::Websocket,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                            pinned_profile.as_deref(),
                            turn_state_profile.as_deref(),
                            session_profile.as_deref(),
                            trusted_previous_response_affinity,
                            request_requires_previous_response_affinity,
                        ) {
                            send_runtime_proxy_websocket_error(
                                local_socket,
                                503,
                                "service_unavailable",
                                runtime_proxy_local_selection_failure_message(),
                            )?;
                            return Ok(());
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if previous_response_id.is_some()
                            && trusted_previous_response_affinity
                            && !previous_response_fresh_fallback_used
                            && !request_requires_previous_response_affinity
                            && let Some(fresh_request_text) =
                                runtime_request_text_without_previous_response_id(&request_text)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            request_text = fresh_request_text;
                            handshake_request =
                                runtime_request_without_turn_state_header(&handshake_request);
                            previous_response_id = None;
                            request_turn_state = None;
                            previous_response_fresh_fallback_used = true;
                            saw_previous_response_not_found = false;
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                            trusted_previous_response_affinity = false;
                            bound_profile = None;
                            session_profile = None;
                            pinned_profile = None;
                            turn_state_profile = None;
                            websocket_reuse_fresh_retry_profiles.clear();
                            excluded_profiles.clear();
                            last_failure = None;
                            selection_started_at = Instant::now();
                            selection_attempts = 0;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            match last_failure {
                Some(RuntimeUpstreamFailureResponse::Websocket(payload)) => {
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                }
                _ if saw_inflight_saturation => {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        503,
                        "service_unavailable",
                        "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                    )?;
                }
                _ => {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    )?;
                }
            }
            return Ok(());
        };
        selection_attempts = selection_attempts.saturating_add(1);
        let turn_state_override =
            if candidate_turn_state_retry_profile.as_deref() == Some(candidate_name.as_str()) {
                candidate_turn_state_retry_value.as_deref()
            } else {
                request_turn_state.as_deref()
            };
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} websocket_session={session_id} candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                candidate_name,
                pinned_profile,
                turn_state_profile,
                turn_state_override,
                excluded_profiles.len()
            ),
        );
        let session_affinity_candidate =
            session_profile.as_deref() == Some(candidate_name.as_str());
        if previous_response_id.is_none()
            && pinned_profile.is_none()
            && turn_state_profile.is_none()
            && !session_affinity_candidate
            && runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate_name,
                "websocket_session",
            )?
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} websocket_session={session_id} profile_inflight_saturated profile={candidate_name} hard_limit={}",
                    runtime_proxy_profile_inflight_hard_limit(),
                ),
            );
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
            continue;
        }

        match attempt_runtime_websocket_request(
            request_id,
            local_socket,
            &handshake_request,
            &request_text,
            previous_response_id.as_deref(),
            request_session_id.as_deref(),
            request_turn_state.as_deref(),
            shared,
            websocket_session,
            &candidate_name,
            turn_state_override,
        )? {
            RuntimeWebsocketAttempt::Delivered => return Ok(()),
            RuntimeWebsocketAttempt::QuotaBlocked {
                profile_name,
                payload,
            } => {
                let quota_message =
                    extract_runtime_proxy_quota_message_from_websocket_payload(&payload);
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} quota_blocked profile={profile_name}"
                    ),
                );
                mark_runtime_profile_quota_quarantine(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Websocket,
                    quota_message.as_deref(),
                )?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    RuntimeRouteKind::Websocket,
                    &profile_name,
                    compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    pinned_profile.as_deref(),
                    turn_state_profile.as_deref(),
                    session_profile.as_deref(),
                    trusted_previous_response_affinity,
                    request_requires_previous_response_affinity,
                ) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} upstream_usage_limit_passthrough route=websocket profile={profile_name} reason=hard_affinity"
                        ),
                    );
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    return Ok(());
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                    previous_response_retry_index = 0;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name}"
                        ),
                    );
                }
                if previous_response_id.is_some()
                    && trusted_previous_response_affinity
                    && !previous_response_fresh_fallback_used
                    && !request_requires_previous_response_affinity
                    && let Some(fresh_request_text) =
                        runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=quota_blocked"
                        ),
                    );
                    request_text = fresh_request_text;
                    handshake_request =
                        runtime_request_without_turn_state_header(&handshake_request);
                    previous_response_id = None;
                    request_turn_state = None;
                    previous_response_fresh_fallback_used = true;
                    saw_previous_response_not_found = false;
                    previous_response_retry_candidate = None;
                    previous_response_retry_index = 0;
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                    trusted_previous_response_affinity = false;
                    bound_profile = None;
                    session_profile = None;
                    pinned_profile = None;
                    turn_state_profile = None;
                    websocket_reuse_fresh_retry_profiles.clear();
                    excluded_profiles.clear();
                    last_failure = None;
                    selection_started_at = Instant::now();
                    selection_attempts = 0;
                    continue;
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
            }
            RuntimeWebsocketAttempt::Overloaded {
                profile_name,
                payload,
            } => {
                let overload_message =
                    extract_runtime_proxy_overload_message_from_websocket_payload(&payload);
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} upstream_overloaded route=websocket profile={profile_name} message={}",
                        overload_message.as_deref().unwrap_or("-"),
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                let _ = bump_runtime_profile_health_score(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Websocket,
                    RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                    "websocket_overload",
                );
                let _ = bump_runtime_profile_bad_pairing_score(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Websocket,
                    RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                    "websocket_overload",
                );
                if !runtime_quota_blocked_affinity_is_releasable(
                    RuntimeRouteKind::Websocket,
                    &profile_name,
                    compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    pinned_profile.as_deref(),
                    turn_state_profile.as_deref(),
                    session_profile.as_deref(),
                    trusted_previous_response_affinity,
                    request_requires_previous_response_affinity,
                ) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} upstream_overload_passthrough route=websocket profile={profile_name} reason=hard_affinity"
                        ),
                    );
                    forward_runtime_proxy_websocket_error(local_socket, &payload)?;
                    return Ok(());
                }
                if previous_response_id.is_some()
                    && trusted_previous_response_affinity
                    && !previous_response_fresh_fallback_used
                    && !request_requires_previous_response_affinity
                    && let Some(fresh_request_text) =
                        runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=upstream_overloaded"
                        ),
                    );
                    request_text = fresh_request_text;
                    handshake_request =
                        runtime_request_without_turn_state_header(&handshake_request);
                    previous_response_id = None;
                    request_turn_state = None;
                    previous_response_fresh_fallback_used = true;
                    saw_previous_response_not_found = false;
                    previous_response_retry_candidate = None;
                    previous_response_retry_index = 0;
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                    trusted_previous_response_affinity = false;
                    bound_profile = None;
                    session_profile = None;
                    pinned_profile = None;
                    turn_state_profile = None;
                    websocket_reuse_fresh_retry_profiles.clear();
                    excluded_profiles.clear();
                    last_failure = None;
                    selection_started_at = Instant::now();
                    selection_attempts = 0;
                    continue;
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
            }
            RuntimeWebsocketAttempt::LocalSelectionBlocked {
                profile_name,
                reason,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} local_selection_blocked profile={profile_name} reason={reason}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    RuntimeRouteKind::Websocket,
                    &profile_name,
                    compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    pinned_profile.as_deref(),
                    turn_state_profile.as_deref(),
                    session_profile.as_deref(),
                    trusted_previous_response_affinity,
                    request_requires_previous_response_affinity,
                ) {
                    send_runtime_proxy_websocket_error(
                        local_socket,
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    )?;
                    return Ok(());
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                    previous_response_retry_index = 0;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} quota_blocked_affinity_released profile={profile_name} reason={reason}"
                        ),
                    );
                }
                if previous_response_id.is_some()
                    && trusted_previous_response_affinity
                    && !previous_response_fresh_fallback_used
                    && !request_requires_previous_response_affinity
                    && let Some(fresh_request_text) =
                        runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason={reason}"
                        ),
                    );
                    request_text = fresh_request_text;
                    handshake_request =
                        runtime_request_without_turn_state_header(&handshake_request);
                    previous_response_id = None;
                    request_turn_state = None;
                    previous_response_fresh_fallback_used = true;
                    saw_previous_response_not_found = false;
                    previous_response_retry_candidate = None;
                    previous_response_retry_index = 0;
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                    trusted_previous_response_affinity = false;
                    bound_profile = None;
                    session_profile = None;
                    pinned_profile = None;
                    turn_state_profile = None;
                    websocket_reuse_fresh_retry_profiles.clear();
                    excluded_profiles.clear();
                    last_failure = None;
                    selection_started_at = Instant::now();
                    selection_attempts = 0;
                    continue;
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name,
                event,
            } => {
                let reuse_terminal_idle = websocket_session.last_terminal_elapsed();
                let retry_same_profile_with_fresh_connect = !websocket_reuse_fresh_retry_profiles
                    .contains(&profile_name)
                    && (bound_profile.as_deref() == Some(profile_name.as_str())
                        || turn_state_profile.as_deref() == Some(profile_name.as_str())
                        || compact_followup_profile
                            .as_ref()
                            .is_some_and(|(owner, _)| owner == &profile_name)
                        || (request_session_id.is_some()
                            && session_profile.as_deref() == Some(profile_name.as_str())));
                let reuse_failed_bound_previous_response = previous_response_id.is_some()
                    && !previous_response_fresh_fallback_used
                    && (bound_profile.as_deref() == Some(profile_name.as_str())
                        || pinned_profile.as_deref() == Some(profile_name.as_str()));
                let nonreplayable_previous_response_reuse = previous_response_id.is_some()
                    && !previous_response_fresh_fallback_used
                    && turn_state_override.is_none();
                let stale_previous_response_reuse = nonreplayable_previous_response_reuse
                    && turn_state_override.is_none()
                    && reuse_terminal_idle.is_some_and(|elapsed| {
                        elapsed
                            >= Duration::from_millis(
                                runtime_proxy_websocket_previous_response_reuse_stale_ms(),
                            )
                    });
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} websocket_session={session_id} websocket_reuse_watchdog_timeout profile={profile_name} event={event}"
                    ),
                );
                if nonreplayable_previous_response_reuse {
                    if stale_previous_response_reuse {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} websocket_reuse_stale_previous_response_blocked profile={profile_name} event={event} elapsed_ms={} threshold_ms={}",
                                reuse_terminal_idle
                                    .map(|elapsed| elapsed.as_millis())
                                    .unwrap_or(0),
                                runtime_proxy_websocket_previous_response_reuse_stale_ms(),
                            ),
                        );
                    } else {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} websocket_session={session_id} websocket_reuse_previous_response_blocked profile={profile_name} event={event} reason=missing_turn_state elapsed_ms={}",
                                reuse_terminal_idle
                                    .map(|elapsed| elapsed.as_millis())
                                    .unwrap_or(0),
                            ),
                        );
                    }
                    return Err(anyhow::anyhow!(
                        "runtime websocket upstream closed before response.completed for previous_response_id continuation without replayable turn_state: profile={profile_name} event={event}"
                    ));
                }
                if retry_same_profile_with_fresh_connect {
                    websocket_reuse_fresh_retry_profiles.insert(profile_name.clone());
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} websocket_reuse_owner_fresh_retry profile={profile_name} event={event}"
                        ),
                    );
                    continue;
                }
                if reuse_failed_bound_previous_response
                    && !request_requires_previous_response_affinity
                    && let Some(fresh_request_text) =
                        runtime_request_text_without_previous_response_id(&request_text)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_fresh_fallback reason=websocket_reuse_watchdog"
                        ),
                    );
                    request_text = fresh_request_text;
                    handshake_request =
                        runtime_request_without_turn_state_header(&handshake_request);
                    previous_response_id = None;
                    request_turn_state = None;
                    previous_response_fresh_fallback_used = true;
                    saw_previous_response_not_found = false;
                    previous_response_retry_candidate = None;
                    previous_response_retry_index = 0;
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                    trusted_previous_response_affinity = false;
                    bound_profile = None;
                    session_profile = None;
                    pinned_profile = None;
                    turn_state_profile = None;
                    websocket_reuse_fresh_retry_profiles.clear();
                    excluded_profiles.clear();
                    last_failure = None;
                    selection_started_at = Instant::now();
                    selection_attempts = 0;
                    continue;
                }
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                    previous_response_retry_index = 0;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeWebsocketAttempt::PreviousResponseNotFound {
                profile_name,
                payload,
                turn_state,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket route=websocket websocket_session={session_id} previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?}",
                        turn_state
                    ),
                );
                saw_previous_response_not_found = true;
                if previous_response_retry_candidate.as_deref() != Some(profile_name.as_str()) {
                    previous_response_retry_candidate = Some(profile_name.clone());
                    previous_response_retry_index = 0;
                }
                let has_turn_state_retry = turn_state.is_some();
                if has_turn_state_retry {
                    candidate_turn_state_retry_profile = Some(profile_name.clone());
                    candidate_turn_state_retry_value = turn_state;
                }
                if has_turn_state_retry
                    && let Some(delay) =
                        runtime_previous_response_retry_delay(previous_response_retry_index)
                {
                    previous_response_retry_index += 1;
                    last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry",
                            delay.as_millis()
                        ),
                    );
                    continue;
                }
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                if !has_turn_state_retry && !request_requires_previous_response_affinity {
                    let _ = clear_runtime_stale_previous_response_binding(
                        shared,
                        &profile_name,
                        previous_response_id.as_deref(),
                    )?;
                }
                let released_affinity = release_runtime_previous_response_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                    RuntimeRouteKind::Websocket,
                )?;
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} websocket_session={session_id} previous_response_affinity_released profile={profile_name}"
                        ),
                    );
                }
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                }
                trusted_previous_response_affinity = false;
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                if compact_followup_profile
                    .as_ref()
                    .is_some_and(|(owner, _)| owner == &profile_name)
                {
                    compact_followup_profile = None;
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some(RuntimeUpstreamFailureResponse::Websocket(payload));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn attempt_runtime_websocket_request(
    request_id: u64,
    local_socket: &mut RuntimeLocalWebSocket,
    handshake_request: &RuntimeProxyRequest,
    request_text: &str,
    request_previous_response_id: Option<&str>,
    request_session_id: Option<&str>,
    request_turn_state: Option<&str>,
    shared: &RuntimeRotationProxyShared,
    websocket_session: &mut RuntimeWebsocketSessionState,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<RuntimeWebsocketAttempt> {
    let promote_committed_profile = request_previous_response_id.is_none()
        && request_session_id.is_none()
        && request_turn_state.is_none();
    let (initial_quota_summary, initial_quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Websocket)?;
    if (request_previous_response_id.is_some()
        || request_session_id.is_some()
        || request_turn_state.is_some())
        && matches!(
            initial_quota_source,
            Some(RuntimeQuotaSource::PersistedSnapshot)
        )
        && let Some(reason) =
            runtime_quota_precommit_guard_reason(initial_quota_summary, RuntimeRouteKind::Websocket)
    {
        websocket_session.close();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket websocket_pre_send_skip profile={profile_name} reason={reason} quota_source={} {}",
                initial_quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(initial_quota_summary),
            ),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }
    let has_alternative_quota_profile =
        runtime_has_alternative_quota_compatible_profile(shared, profile_name)?;
    let (quota_summary, quota_source) = ensure_runtime_profile_precommit_quota_ready(
        shared,
        profile_name,
        RuntimeRouteKind::Websocket,
        "websocket_precommit_reprobe",
    )?;
    if runtime_quota_summary_requires_live_source_after_probe(
        quota_summary,
        quota_source,
        RuntimeRouteKind::Websocket,
    ) && has_alternative_quota_profile
    {
        websocket_session.close();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket websocket_pre_send_skip profile={profile_name} reason=quota_windows_unavailable_after_reprobe quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: "quota_windows_unavailable_after_reprobe",
        });
    }
    if let Some(reason) =
        runtime_quota_precommit_guard_reason(quota_summary, RuntimeRouteKind::Websocket)
    {
        websocket_session.close();
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket websocket_pre_send_skip profile={profile_name} reason={reason} quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeWebsocketAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }

    let reuse_existing_session = websocket_session.can_reuse(profile_name, turn_state_override);
    let reuse_started_at = reuse_existing_session.then(Instant::now);
    let precommit_started_at = Instant::now();
    let (mut upstream_socket, mut upstream_turn_state, mut inflight_guard) =
        if reuse_existing_session {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket websocket_reuse_start profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=reuse profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            (
                websocket_session
                    .take_socket()
                    .expect("runtime websocket session should keep its upstream socket"),
                websocket_session.turn_state.clone(),
                None,
            )
        } else {
            websocket_session.close();
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_session=connect profile={profile_name} turn_state_override={:?}",
                    turn_state_override
                ),
            );
            match connect_runtime_proxy_upstream_websocket(
                request_id,
                handshake_request,
                shared,
                profile_name,
                turn_state_override,
            )? {
                RuntimeWebsocketConnectResult::Connected { socket, turn_state } => (
                    socket,
                    turn_state,
                    Some(acquire_runtime_profile_inflight_guard(
                        shared,
                        profile_name,
                        "websocket_session",
                    )?),
                ),
                RuntimeWebsocketConnectResult::QuotaBlocked(payload) => {
                    return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                        profile_name: profile_name.to_string(),
                        payload,
                    });
                }
                RuntimeWebsocketConnectResult::Overloaded(payload) => {
                    return Ok(RuntimeWebsocketAttempt::Overloaded {
                        profile_name: profile_name.to_string(),
                        payload,
                    });
                }
            }
        };
    runtime_set_upstream_websocket_io_timeout(
        &mut upstream_socket,
        Some(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms(),
        )),
    )
    .context("failed to configure runtime websocket pre-commit timeout")?;

    if let Err(err) = upstream_socket.send(WsMessage::Text(request_text.to_string().into())) {
        let _ = upstream_socket.close(None);
        websocket_session.reset();
        let transport_error =
            anyhow::anyhow!("failed to send runtime websocket request upstream: {err}");
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            "websocket_upstream_send",
            &transport_error,
        );
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=websocket upstream_send_error profile={profile_name} error={err}"
            ),
        );
        if reuse_existing_session {
            return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                profile_name: profile_name.to_string(),
                event: "upstream_send_error",
            });
        }
        return Err(transport_error);
    }

    let mut committed = false;
    let mut first_upstream_frame_seen = false;
    let mut buffered_precommit_text_frames = Vec::new();
    let mut previous_response_owner_recorded = false;
    let mut precommit_hold_count = 0usize;
    loop {
        match upstream_socket.read() {
            Ok(WsMessage::Text(text)) => {
                let text = text.to_string();
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                }

                let inspected = inspect_runtime_websocket_text_frame(text.as_str());
                if let Some(turn_state) = inspected.turn_state.as_deref() {
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        Some(turn_state),
                        RuntimeRouteKind::Websocket,
                    )?;
                    upstream_turn_state = Some(turn_state.to_string());
                }

                if !committed {
                    match inspected.retry_kind {
                        Some(RuntimeWebsocketRetryInspectionKind::QuotaBlocked) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::QuotaBlocked {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                            });
                        }
                        Some(RuntimeWebsocketRetryInspectionKind::Overloaded) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::Overloaded {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                            });
                        }
                        Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound) => {
                            let _ = upstream_socket.close(None);
                            websocket_session.reset();
                            return Ok(RuntimeWebsocketAttempt::PreviousResponseNotFound {
                                profile_name: profile_name.to_string(),
                                payload: RuntimeWebsocketErrorPayload::Text(text),
                                turn_state: upstream_turn_state.clone(),
                            });
                        }
                        None => {}
                    }
                }

                if reuse_existing_session && !committed && inspected.precommit_hold {
                    if precommit_hold_count == 0 {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=websocket precommit_hold profile={profile_name} event_type={}",
                                inspected.event_type.as_deref().unwrap_or("-")
                            ),
                        );
                    }
                    precommit_hold_count = precommit_hold_count.saturating_add(1);
                    buffered_precommit_text_frames.push(RuntimeBufferedWebsocketTextFrame {
                        text,
                        response_ids: inspected.response_ids,
                    });
                    continue;
                }

                if !committed {
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id,
                        RuntimeRouteKind::Websocket,
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    let _ = commit_runtime_proxy_profile_selection_with_policy(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        promote_committed_profile,
                    )?;
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket committed profile={profile_name}"
                        ),
                    );
                    committed = true;
                    forward_runtime_proxy_buffered_websocket_text_frames(
                        local_socket,
                        &mut buffered_precommit_text_frames,
                        shared,
                        profile_name,
                        request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        &mut previous_response_owner_recorded,
                    )?;
                }

                remember_runtime_websocket_response_ids(
                    shared,
                    profile_name,
                    request_previous_response_id,
                    request_session_id,
                    request_turn_state,
                    &inspected.response_ids,
                    &mut previous_response_owner_recorded,
                )?;
                local_socket
                    .send(WsMessage::Text(text.into()))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket text frame"
                    })?;
                if inspected.terminal_event {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket terminal_event profile={profile_name} event_type={} precommit_hold_count={precommit_hold_count}",
                            inspected.event_type.as_deref().unwrap_or("-"),
                        ),
                    );
                    websocket_session.store(
                        upstream_socket,
                        profile_name,
                        upstream_turn_state,
                        inflight_guard.take(),
                    );
                    return Ok(RuntimeWebsocketAttempt::Delivered);
                }
            }
            Ok(WsMessage::Binary(payload)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                }
                if !committed {
                    remember_runtime_session_id(
                        shared,
                        profile_name,
                        request_session_id,
                        RuntimeRouteKind::Websocket,
                    )?;
                    remember_runtime_turn_state(
                        shared,
                        profile_name,
                        upstream_turn_state.as_deref(),
                        RuntimeRouteKind::Websocket,
                    )?;
                    let _ = commit_runtime_proxy_profile_selection_with_policy(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        promote_committed_profile,
                    )?;
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket committed_binary profile={profile_name}"
                        ),
                    );
                    committed = true;
                    forward_runtime_proxy_buffered_websocket_text_frames(
                        local_socket,
                        &mut buffered_precommit_text_frames,
                        shared,
                        profile_name,
                        request_previous_response_id,
                        request_session_id,
                        request_turn_state,
                        &mut previous_response_owner_recorded,
                    )?;
                }
                local_socket
                    .send(WsMessage::Binary(payload))
                    .with_context(|| {
                        websocket_session.reset();
                        "failed to forward runtime websocket binary frame"
                    })?;
            }
            Ok(WsMessage::Ping(payload)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                }
                upstream_socket
                    .send(WsMessage::Pong(payload))
                    .context("failed to respond to upstream websocket ping")?;
            }
            Ok(WsMessage::Pong(_)) | Ok(WsMessage::Frame(_)) => {
                if !first_upstream_frame_seen {
                    first_upstream_frame_seen = true;
                    runtime_set_upstream_websocket_io_timeout(
                        &mut upstream_socket,
                        Some(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms())),
                    )
                    .context("failed to restore runtime websocket idle timeout")?;
                }
            }
            Ok(WsMessage::Close(frame)) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=upstream_close_before_terminal elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_close_before_completed profile={profile_name}"
                    ),
                );
                let _ = frame;
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_close",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_close_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(WsError::ConnectionClosed) | Err(WsError::AlreadyClosed) => {
                websocket_session.reset();
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=connection_closed elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_connection_closed profile={profile_name}"
                    ),
                );
                let transport_error =
                    anyhow::anyhow!("runtime websocket upstream closed before response.completed");
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_connection_closed",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "connection_closed_before_commit",
                    });
                }
                return Err(transport_error);
            }
            Err(err) => {
                websocket_session.reset();
                if !committed && !first_upstream_frame_seen && runtime_websocket_timeout_error(&err)
                {
                    let elapsed_ms = precommit_started_at.elapsed().as_millis();
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_precommit_frame_timeout profile={profile_name} event=no_first_upstream_frame_before_deadline elapsed_ms={elapsed_ms} reuse={reuse_existing_session}"
                        ),
                    );
                    let transport_error = anyhow::anyhow!(
                        "runtime websocket upstream produced no first frame before the pre-commit deadline: {err}"
                    );
                    note_runtime_profile_transport_failure(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        "websocket_first_frame_timeout",
                        &transport_error,
                    );
                    if reuse_existing_session {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "websocket_reuse_watchdog profile={profile_name} event=no_first_upstream_frame_before_deadline elapsed_ms={elapsed_ms} committed={committed}"
                            ),
                        );
                        return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                            profile_name: profile_name.to_string(),
                            event: "no_first_upstream_frame_before_deadline",
                        });
                    }
                    return Err(transport_error);
                }
                if let Some(started_at) = reuse_started_at {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "websocket_reuse_watchdog profile={profile_name} event=read_error elapsed_ms={} committed={committed}",
                            started_at.elapsed().as_millis()
                        ),
                    );
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=websocket upstream_read_error profile={profile_name} error={err}"
                    ),
                );
                let transport_error = anyhow::anyhow!(
                    "runtime websocket upstream failed before response.completed: {err}"
                );
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    "websocket_upstream_read",
                    &transport_error,
                );
                if reuse_existing_session && !committed {
                    return Ok(RuntimeWebsocketAttempt::ReuseWatchdogTripped {
                        profile_name: profile_name.to_string(),
                        event: "upstream_read_error",
                    });
                }
                return Err(transport_error);
            }
        }
    }
}

pub(super) fn connect_runtime_proxy_upstream_websocket(
    request_id: u64,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<RuntimeWebsocketConnectResult> {
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .clone();
    let auth = runtime_profile_usage_auth(shared, profile_name)?;
    let upstream_url = runtime_proxy_upstream_websocket_url(
        &runtime.upstream_base_url,
        &handshake_request.path_and_query,
    )?;
    let mut request = upstream_url
        .as_str()
        .into_client_request()
        .with_context(|| format!("failed to build runtime websocket request for {upstream_url}"))?;

    for (name, value) in &handshake_request.headers {
        if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
            continue;
        }
        if should_skip_runtime_request_header(name) {
            continue;
        }
        let Ok(header_name) = WsHeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        let Ok(header_value) = WsHeaderValue::from_str(value) else {
            continue;
        };
        request.headers_mut().insert(header_name, header_value);
    }
    if let Some(turn_state) = turn_state_override {
        request.headers_mut().insert(
            WsHeaderName::from_static("x-codex-turn-state"),
            WsHeaderValue::from_str(turn_state)
                .context("failed to encode websocket turn-state header")?,
        );
    }

    request.headers_mut().insert(
        WsHeaderName::from_static("authorization"),
        WsHeaderValue::from_str(&format!("Bearer {}", auth.access_token))
            .context("failed to encode websocket authorization header")?,
    );
    let user_agent =
        runtime_proxy_effective_user_agent(&handshake_request.headers).unwrap_or("codex-cli");
    request.headers_mut().insert(
        WsHeaderName::from_static("user-agent"),
        WsHeaderValue::from_str(user_agent).context("failed to encode websocket user-agent")?,
    );
    if let Some(account_id) = auth.account_id.as_deref() {
        request.headers_mut().insert(
            WsHeaderName::from_static("chatgpt-account-id"),
            WsHeaderValue::from_str(account_id)
                .context("failed to encode websocket account header")?,
        );
    }

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=websocket upstream_connect_start profile={profile_name} url={upstream_url} turn_state_override={:?}",
            turn_state_override
        ),
    );
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE") {
        let transport_error = anyhow::anyhow!("injected runtime websocket connect failure");
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Websocket,
            "websocket_connect",
            &transport_error,
        );
        return Err(transport_error);
    }
    let started_at = Instant::now();
    match connect_runtime_proxy_upstream_websocket_with_timeout(request) {
        Ok((socket, response, selected_addr, resolved_addrs, attempted_addrs)) => {
            Ok(RuntimeWebsocketConnectResult::Connected {
                socket,
                turn_state: {
                    let turn_state = runtime_proxy_tungstenite_header_value(
                        response.headers(),
                        "x-codex-turn-state",
                    );
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=websocket upstream_connect_ok profile={profile_name} status={} addr={} resolved_addrs={} attempted_addrs={} turn_state={:?}",
                            response.status().as_u16(),
                            selected_addr,
                            resolved_addrs,
                            attempted_addrs,
                            turn_state
                        ),
                    );
                    note_runtime_profile_latency_observation(
                        shared,
                        profile_name,
                        RuntimeRouteKind::Websocket,
                        "connect",
                        started_at.elapsed().as_millis() as u64,
                    );
                    turn_state
                },
            })
        }
        Err(WsError::Http(response)) => {
            let status = response.status().as_u16();
            let body = response.body().clone().unwrap_or_default();
            if matches!(status, 401 | 403)
                && (status == 401 || extract_runtime_proxy_quota_message(&body).is_none())
            {
                note_runtime_profile_auth_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Websocket,
                    status,
                );
            }
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=websocket upstream_connect_http profile={profile_name} status={status} body_bytes={}",
                    body.len()
                ),
            );
            if matches!(status, 403 | 429) && extract_runtime_proxy_quota_message(&body).is_some() {
                return Ok(RuntimeWebsocketConnectResult::QuotaBlocked(
                    runtime_websocket_error_payload_from_http_body(&body),
                ));
            }
            if extract_runtime_proxy_overload_message(status, &body).is_some() {
                return Ok(RuntimeWebsocketConnectResult::Overloaded(
                    runtime_websocket_error_payload_from_http_body(&body),
                ));
            }
            bail!("runtime websocket upstream rejected the handshake with HTTP {status}");
        }
        Err(err) => {
            let failure_kind = runtime_transport_failure_kind_from_ws(&err);
            log_runtime_upstream_connect_failure(
                shared,
                request_id,
                "websocket",
                profile_name,
                failure_kind,
                &err,
            );
            let transport_error =
                anyhow::anyhow!("failed to connect runtime websocket upstream: {err}");
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Websocket,
                "websocket_connect",
                &transport_error,
            );
            Err(transport_error)
        }
    }
}

pub(super) fn runtime_websocket_error_payload_from_http_body(
    body: &[u8],
) -> RuntimeWebsocketErrorPayload {
    if body.is_empty() {
        return RuntimeWebsocketErrorPayload::Empty;
    }

    match std::str::from_utf8(body) {
        Ok(text) => RuntimeWebsocketErrorPayload::Text(text.to_string()),
        Err(_) => RuntimeWebsocketErrorPayload::Binary(body.to_vec()),
    }
}

pub(super) fn connect_runtime_proxy_upstream_websocket_with_timeout(
    request: tungstenite::http::Request<()>,
) -> std::result::Result<
    (
        RuntimeUpstreamWebSocket,
        tungstenite::handshake::client::Response,
        SocketAddr,
        usize,
        usize,
    ),
    WsError,
> {
    let stream = connect_runtime_proxy_upstream_tcp_stream(request.uri())?;
    let selected_addr = stream.selected_addr;
    let resolved_addrs = stream.resolved_addrs;
    let attempted_addrs = stream.attempted_addrs;
    match client_tls_with_config(request, stream.stream, None, None) {
        Ok((socket, response)) => Ok((
            socket,
            response,
            selected_addr,
            resolved_addrs,
            attempted_addrs,
        )),
        Err(WsHandshakeError::Failure(err)) => Err(err),
        Err(WsHandshakeError::Interrupted(_)) => {
            unreachable!("blocking upstream websocket handshake should not interrupt")
        }
    }
}

pub(super) fn runtime_interleave_socket_addrs(addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    let (mut primary, mut secondary): (VecDeque<_>, VecDeque<_>) =
        addrs.into_iter().partition(|addr| addr.is_ipv6());
    let prefer_ipv6 = primary.front().is_some();
    if !prefer_ipv6 {
        std::mem::swap(&mut primary, &mut secondary);
    }

    let mut ordered = Vec::with_capacity(primary.len().saturating_add(secondary.len()));
    loop {
        let mut progressed = false;
        if let Some(addr) = primary.pop_front() {
            ordered.push(addr);
            progressed = true;
        }
        if let Some(addr) = secondary.pop_front() {
            ordered.push(addr);
            progressed = true;
        }
        if !progressed {
            break;
        }
    }
    ordered
}

pub(super) fn runtime_configure_upstream_tcp_stream(
    stream: &TcpStream,
    io_timeout: Duration,
) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(io_timeout))?;
    stream.set_write_timeout(Some(io_timeout))?;
    Ok(())
}

pub(super) fn runtime_launch_websocket_tcp_connect_attempt(
    sender: mpsc::Sender<RuntimeWebsocketTcpAttemptResult>,
    addr: SocketAddr,
    connect_timeout: Duration,
) {
    thread::spawn(move || {
        let result = TcpStream::connect_timeout(&addr, connect_timeout);
        let _ = sender.send(RuntimeWebsocketTcpAttemptResult { addr, result });
    });
}

pub(super) fn connect_runtime_proxy_upstream_tcp_stream(
    uri: &tungstenite::http::Uri,
) -> std::result::Result<RuntimeWebsocketTcpConnectSuccess, WsError> {
    let host = uri.host().ok_or(WsError::Url(WsUrlError::NoHostName))?;
    let host = if host.starts_with('[') && host.ends_with(']') {
        &host[1..host.len() - 1]
    } else {
        host
    };
    let port = uri.port_u16().unwrap_or(match uri.scheme_str() {
        Some("wss") => 443,
        _ => 80,
    });
    let connect_timeout = Duration::from_millis(runtime_proxy_websocket_connect_timeout_ms());
    let io_timeout = Duration::from_millis(runtime_proxy_websocket_precommit_progress_timeout_ms());
    let happy_eyeballs_delay =
        Duration::from_millis(runtime_proxy_websocket_happy_eyeballs_delay_ms());
    let addrs = runtime_interleave_socket_addrs(
        (host, port)
            .to_socket_addrs()
            .map_err(WsError::Io)?
            .collect(),
    );
    if addrs.is_empty() {
        return Err(WsError::Url(WsUrlError::UnableToConnect(uri.to_string())));
    }

    let resolved_addrs = addrs.len();
    let (sender, receiver) = mpsc::channel::<RuntimeWebsocketTcpAttemptResult>();
    let mut next_index = 0usize;
    let mut attempted_addrs = 0usize;
    let mut in_flight = 0usize;
    let mut last_error = None;

    while next_index < addrs.len() || in_flight > 0 {
        if in_flight == 0 && next_index < addrs.len() {
            runtime_launch_websocket_tcp_connect_attempt(
                sender.clone(),
                addrs[next_index],
                connect_timeout,
            );
            next_index += 1;
            attempted_addrs += 1;
            in_flight += 1;
        }

        let next = if in_flight == 1 && next_index < addrs.len() && !happy_eyeballs_delay.is_zero()
        {
            match receiver.recv_timeout(happy_eyeballs_delay) {
                Ok(result) => Some(result),
                Err(RecvTimeoutError::Timeout) => {
                    runtime_launch_websocket_tcp_connect_attempt(
                        sender.clone(),
                        addrs[next_index],
                        connect_timeout,
                    );
                    next_index += 1;
                    attempted_addrs += 1;
                    in_flight += 1;
                    receiver.recv().ok()
                }
                Err(RecvTimeoutError::Disconnected) => None,
            }
        } else {
            receiver.recv().ok()
        };

        let Some(result) = next else {
            break;
        };
        in_flight = in_flight.saturating_sub(1);
        match result.result {
            Ok(stream) => {
                runtime_configure_upstream_tcp_stream(&stream, io_timeout).map_err(WsError::Io)?;
                return Ok(RuntimeWebsocketTcpConnectSuccess {
                    stream,
                    selected_addr: result.addr,
                    resolved_addrs,
                    attempted_addrs,
                });
            }
            Err(err) => {
                last_error = Some(err);
            }
        }
    }

    match last_error {
        Some(err) => Err(WsError::Io(err)),
        None => Err(WsError::Url(WsUrlError::UnableToConnect(uri.to_string()))),
    }
}

pub(super) fn send_runtime_proxy_websocket_error(
    local_socket: &mut RuntimeLocalWebSocket,
    status: u16,
    code: &str,
    message: &str,
) -> Result<()> {
    let payload = serde_json::json!({
        "type": "error",
        "status": status,
        "error": {
            "code": code,
            "message": message,
        }
    })
    .to_string();
    local_socket
        .send(WsMessage::Text(payload.into()))
        .context("failed to send runtime websocket error frame")
}

pub(super) fn forward_runtime_proxy_websocket_error(
    local_socket: &mut RuntimeLocalWebSocket,
    payload: &RuntimeWebsocketErrorPayload,
) -> Result<()> {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => local_socket
            .send(WsMessage::Text(text.clone().into()))
            .context("failed to forward runtime websocket text error frame"),
        RuntimeWebsocketErrorPayload::Binary(bytes) => local_socket
            .send(WsMessage::Binary(bytes.clone().into()))
            .context("failed to forward runtime websocket binary error frame"),
        RuntimeWebsocketErrorPayload::Empty => Ok(()),
    }
}

pub(super) fn remember_runtime_websocket_response_ids(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    request_previous_response_id: Option<&str>,
    request_session_id: Option<&str>,
    request_turn_state: Option<&str>,
    response_ids: &[String],
    previous_response_owner_recorded: &mut bool,
) -> Result<()> {
    if !*previous_response_owner_recorded {
        remember_runtime_successful_previous_response_owner(
            shared,
            profile_name,
            request_previous_response_id,
            RuntimeRouteKind::Websocket,
        )?;
        *previous_response_owner_recorded = true;
    }
    remember_runtime_response_ids(
        shared,
        profile_name,
        response_ids,
        RuntimeRouteKind::Websocket,
    )?;
    if !response_ids.is_empty() {
        let _ = release_runtime_compact_lineage(
            shared,
            profile_name,
            request_session_id,
            request_turn_state,
            "response_committed",
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn forward_runtime_proxy_buffered_websocket_text_frames(
    local_socket: &mut RuntimeLocalWebSocket,
    buffered_frames: &mut Vec<RuntimeBufferedWebsocketTextFrame>,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    request_previous_response_id: Option<&str>,
    request_session_id: Option<&str>,
    request_turn_state: Option<&str>,
    previous_response_owner_recorded: &mut bool,
) -> Result<()> {
    for frame in buffered_frames.drain(..) {
        remember_runtime_websocket_response_ids(
            shared,
            profile_name,
            request_previous_response_id,
            request_session_id,
            request_turn_state,
            &frame.response_ids,
            previous_response_owner_recorded,
        )?;
        local_socket
            .send(WsMessage::Text(frame.text.into()))
            .context("failed to forward buffered runtime websocket text frame")?;
    }
    Ok(())
}

pub(super) fn inspect_runtime_websocket_text_frame(
    payload: &str,
) -> RuntimeInspectedWebsocketTextFrame {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return RuntimeInspectedWebsocketTextFrame::default();
    };

    let event_type = runtime_response_event_type_from_value(&value);
    let retry_kind = if extract_runtime_proxy_previous_response_message_from_value(&value).is_some()
    {
        Some(RuntimeWebsocketRetryInspectionKind::PreviousResponseNotFound)
    } else if extract_runtime_proxy_overload_message_from_value(&value).is_some() {
        Some(RuntimeWebsocketRetryInspectionKind::Overloaded)
    } else if extract_runtime_proxy_quota_message_from_value(&value).is_some() {
        Some(RuntimeWebsocketRetryInspectionKind::QuotaBlocked)
    } else {
        None
    };
    let precommit_hold = event_type
        .as_deref()
        .is_some_and(runtime_proxy_precommit_hold_event_kind);
    let terminal_event = event_type
        .as_deref()
        .is_some_and(|kind| matches!(kind, "response.completed" | "response.failed"));

    RuntimeInspectedWebsocketTextFrame {
        event_type,
        turn_state: extract_runtime_turn_state_from_value(&value),
        response_ids: extract_runtime_response_ids_from_value(&value),
        retry_kind,
        precommit_hold,
        terminal_event,
    }
}

pub(super) fn runtime_response_event_type_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
pub(super) fn runtime_response_event_type(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|value| runtime_response_event_type_from_value(&value))
}

pub(super) fn runtime_proxy_precommit_hold_event_kind(kind: &str) -> bool {
    matches!(
        kind,
        "response.created"
            | "response.in_progress"
            | "response.queued"
            | "response.output_item.added"
            | "response.content_part.added"
            | "response.reasoning_summary_part.added"
    )
}

#[cfg(test)]
pub(super) fn is_runtime_terminal_event(payload: &str) -> bool {
    runtime_response_event_type(payload)
        .is_some_and(|kind| matches!(kind.as_str(), "response.completed" | "response.failed"))
}

pub(super) fn proxy_runtime_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    let request_session_id = runtime_request_session_id(request);
    let request_turn_state = runtime_request_turn_state(request);
    let mut session_profile = request_session_id
        .as_deref()
        .map(|session_id| runtime_session_bound_profile(shared, session_id))
        .transpose()?
        .flatten();
    if !is_runtime_compact_path(&request.path_and_query) {
        let current_profile = runtime_proxy_current_profile(shared)?;
        let preferred_profile = session_profile
            .clone()
            .unwrap_or_else(|| current_profile.clone());
        let pressure_mode =
            runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Standard);
        let selection_started_at = Instant::now();
        let mut selection_attempts = 0usize;
        let mut excluded_profiles = BTreeSet::new();
        let mut last_failure = None;
        let mut saw_inflight_saturation = false;
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
            excluded_profiles.insert(preferred_profile.clone());
        }

        loop {
            if runtime_proxy_precommit_budget_exhausted(
                selection_started_at,
                selection_attempts,
                session_profile.is_some(),
                pressure_mode,
            ) {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http standard_precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                        selection_started_at.elapsed().as_millis()
                    ),
                );
                return match last_failure {
                    Some(response) => Ok(response),
                    None if saw_inflight_saturation => Ok(build_runtime_proxy_json_error_response(
                        503,
                        "service_unavailable",
                        "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                    )),
                    None => Ok(build_runtime_proxy_text_response(
                        503,
                        runtime_proxy_local_selection_failure_message(),
                    )),
                };
            }

            let candidate_name = if excluded_profiles.is_empty() {
                preferred_profile.clone()
            } else if let Some(candidate_name) = select_runtime_response_candidate_for_route(
                shared,
                &excluded_profiles,
                None,
                None,
                None,
                None,
                false,
                None,
                RuntimeRouteKind::Standard,
            )? {
                candidate_name
            } else {
                return match last_failure {
                    Some(response) => Ok(response),
                    None if saw_inflight_saturation => Ok(build_runtime_proxy_json_error_response(
                        503,
                        "service_unavailable",
                        "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                    )),
                    None => Ok(build_runtime_proxy_text_response(
                        503,
                        runtime_proxy_local_selection_failure_message(),
                    )),
                };
            };
            selection_attempts = selection_attempts.saturating_add(1);

            if runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate_name,
                "standard_http",
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http profile_inflight_saturated profile={candidate_name} hard_limit={}",
                        runtime_proxy_profile_inflight_hard_limit(),
                    ),
                );
                excluded_profiles.insert(candidate_name);
                saw_inflight_saturation = true;
                continue;
            }

            match attempt_runtime_noncompact_standard_request(
                request_id,
                request,
                shared,
                &candidate_name,
            )? {
                RuntimeStandardAttempt::Success {
                    profile_name: _,
                    response,
                } => return Ok(response),
                RuntimeStandardAttempt::RetryableFailure {
                    profile_name,
                    response,
                    overload: _,
                } => {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http standard_retryable_failure profile={profile_name}"
                        ),
                    );
                    mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                    let released_affinity = release_runtime_quota_blocked_affinity(
                        shared,
                        &profile_name,
                        None,
                        None,
                        request_session_id.as_deref(),
                    )?;
                    if session_profile.as_deref() == Some(profile_name.as_str()) {
                        session_profile = None;
                    }
                    if released_affinity {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} route=standard"
                            ),
                        );
                    }
                    excluded_profiles.insert(profile_name);
                    last_failure = Some(response);
                }
                RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http local_selection_blocked profile={profile_name} route=standard reason=quota_exhausted_before_send"
                        ),
                    );
                    if session_profile.as_deref() == Some(profile_name.as_str()) {
                        session_profile = None;
                    }
                    excluded_profiles.insert(profile_name);
                }
            }
        }
    }

    let current_profile = runtime_proxy_current_profile(shared)?;
    let mut compact_followup_profile = runtime_compact_followup_bound_profile(
        shared,
        request_turn_state.as_deref(),
        request_session_id.as_deref(),
    )?;
    if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_followup_owner profile={profile_name} source={source}"
            ),
        );
    }
    let initial_compact_affinity_profile = compact_followup_profile
        .as_ref()
        .map(|(profile_name, _)| profile_name.as_str())
        .or(session_profile.as_deref());
    let compact_owner_profile = compact_followup_profile
        .as_ref()
        .map(|(profile_name, _)| profile_name.clone())
        .or(session_profile.clone())
        .unwrap_or_else(|| current_profile.clone());
    let pressure_mode =
        runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Compact);
    if runtime_proxy_should_shed_fresh_compact_request(
        pressure_mode,
        initial_compact_affinity_profile,
    ) {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_pressure_shed reason=fresh_request pressure_mode={pressure_mode}"
            ),
        );
        return Ok(build_runtime_proxy_json_error_response(
            503,
            "service_unavailable",
            "Fresh compact requests are temporarily deferred while the runtime proxy is under pressure. Retry the request.",
        ));
    }
    let mut excluded_profiles = BTreeSet::new();
    let mut conservative_overload_retried_profiles = BTreeSet::new();
    let mut last_failure = None;
    let mut saw_inflight_saturation = false;
    let selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;

    loop {
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            compact_followup_profile.is_some() || session_profile.is_some(),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            return match last_failure {
                Some(response) => Ok(response),
                None if saw_inflight_saturation => Ok(build_runtime_proxy_json_error_response(
                    503,
                    "service_unavailable",
                    "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                )),
                None if compact_followup_profile.is_some() || session_profile.is_some() => {
                    Ok(build_runtime_proxy_json_error_response(
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    ))
                }
                None => match attempt_runtime_standard_request(
                    request_id,
                    request,
                    shared,
                    &compact_owner_profile,
                    runtime_candidate_has_hard_affinity(
                        RuntimeRouteKind::Compact,
                        &compact_owner_profile,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        None,
                        None,
                        session_profile.as_deref(),
                        false,
                    ),
                )? {
                    RuntimeStandardAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Compact,
                        )?;
                        Ok(response)
                    }
                    RuntimeStandardAttempt::RetryableFailure { response, .. } => Ok(response),
                    RuntimeStandardAttempt::LocalSelectionBlocked { .. } => {
                        Ok(build_runtime_proxy_json_error_response(
                            503,
                            "service_unavailable",
                            runtime_proxy_local_selection_failure_message(),
                        ))
                    }
                },
            };
        }
        selection_attempts = selection_attempts.saturating_add(1);

        let Some(candidate_name) = select_runtime_response_candidate_for_route(
            shared,
            &excluded_profiles,
            compact_followup_profile
                .as_ref()
                .map(|(profile_name, _)| profile_name.as_str()),
            None,
            None,
            session_profile.as_deref(),
            false,
            None,
            RuntimeRouteKind::Compact,
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_candidate_exhausted last_failure={}",
                    if last_failure.is_some() {
                        "http"
                    } else {
                        "none"
                    }
                ),
            );
            return match last_failure {
                Some(response) => Ok(response),
                None if saw_inflight_saturation => Ok(build_runtime_proxy_json_error_response(
                    503,
                    "service_unavailable",
                    "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                )),
                None if compact_followup_profile.is_some() || session_profile.is_some() => {
                    Ok(build_runtime_proxy_json_error_response(
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    ))
                }
                None => match attempt_runtime_standard_request(
                    request_id,
                    request,
                    shared,
                    &compact_owner_profile,
                    runtime_candidate_has_hard_affinity(
                        RuntimeRouteKind::Compact,
                        &compact_owner_profile,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        None,
                        None,
                        session_profile.as_deref(),
                        false,
                    ),
                )? {
                    RuntimeStandardAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Compact,
                        )?;
                        Ok(response)
                    }
                    RuntimeStandardAttempt::RetryableFailure { response, .. } => Ok(response),
                    RuntimeStandardAttempt::LocalSelectionBlocked { .. } => {
                        Ok(build_runtime_proxy_json_error_response(
                            503,
                            "service_unavailable",
                            runtime_proxy_local_selection_failure_message(),
                        ))
                    }
                },
            };
        };

        if excluded_profiles.contains(&candidate_name) {
            continue;
        }

        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_candidate={} excluded_count={}",
                candidate_name,
                excluded_profiles.len()
            ),
        );
        let session_affinity_candidate = compact_followup_profile
            .as_ref()
            .is_some_and(|(owner, _)| owner == &candidate_name)
            || session_profile.as_deref() == Some(candidate_name.as_str());
        if runtime_profile_inflight_hard_limited_for_context(
            shared,
            &candidate_name,
            "compact_http",
        )? && !session_affinity_candidate
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http profile_inflight_saturated profile={candidate_name} hard_limit={}",
                    runtime_proxy_profile_inflight_hard_limit(),
                ),
            );
            excluded_profiles.insert(candidate_name);
            saw_inflight_saturation = true;
            continue;
        }

        match attempt_runtime_standard_request(
            request_id,
            request,
            shared,
            &candidate_name,
            runtime_candidate_has_hard_affinity(
                RuntimeRouteKind::Compact,
                &candidate_name,
                compact_followup_profile
                    .as_ref()
                    .map(|(profile_name, _)| profile_name.as_str()),
                None,
                None,
                session_profile.as_deref(),
                false,
            ),
        )? {
            RuntimeStandardAttempt::Success {
                profile_name,
                response,
            } => {
                commit_runtime_proxy_profile_selection_with_notice(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Compact,
                )?;
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_committed profile={profile_name}"
                    ),
                );
                return Ok(response);
            }
            RuntimeStandardAttempt::RetryableFailure {
                profile_name,
                response,
                overload,
            } => {
                let mut released_affinity = false;
                let mut released_compact_lineage = false;
                if !overload {
                    released_affinity = release_runtime_quota_blocked_affinity(
                        shared,
                        &profile_name,
                        None,
                        None,
                        request_session_id.as_deref(),
                    )?;
                    released_compact_lineage = release_runtime_compact_lineage(
                        shared,
                        &profile_name,
                        request_session_id.as_deref(),
                        None,
                        "quota_blocked",
                    )?;
                    if session_profile.as_deref() == Some(profile_name.as_str()) {
                        session_profile = None;
                    }
                    if compact_followup_profile
                        .as_ref()
                        .is_some_and(|(owner, _)| owner == &profile_name)
                    {
                        compact_followup_profile = None;
                    }
                }
                let should_retry_same_profile = overload
                    && !conservative_overload_retried_profiles.contains(&profile_name)
                    && (compact_followup_profile
                        .as_ref()
                        .is_some_and(|(owner, _)| owner == &profile_name)
                        || session_profile.as_deref() == Some(profile_name.as_str())
                        || current_profile == profile_name);
                if should_retry_same_profile {
                    conservative_overload_retried_profiles.insert(profile_name.clone());
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http compact_overload_conservative_retry profile={profile_name} delay_ms={RUNTIME_PROXY_COMPACT_OWNER_RETRY_DELAY_MS} reason=non_blocking_retry"
                        ),
                    );
                    last_failure = Some(response);
                    continue;
                }
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_retryable_failure profile={profile_name}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                if !overload
                    && runtime_candidate_has_hard_affinity(
                        RuntimeRouteKind::Compact,
                        &profile_name,
                        compact_followup_profile
                            .as_ref()
                            .map(|(profile_name, _)| profile_name.as_str()),
                        None,
                        None,
                        session_profile.as_deref(),
                        false,
                    )
                {
                    return Ok(response);
                }
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} route=compact"
                        ),
                    );
                }
                if released_compact_lineage {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http compact_lineage_released profile={profile_name} reason=quota_blocked"
                        ),
                    );
                }
                if overload {
                    let _ = bump_runtime_profile_health_score(
                        shared,
                        &profile_name,
                        RuntimeRouteKind::Compact,
                        RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                        "compact_overload",
                    );
                    let _ = bump_runtime_profile_bad_pairing_score(
                        shared,
                        &profile_name,
                        RuntimeRouteKind::Compact,
                        RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
                        "compact_overload",
                    );
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some(response);
            }
            RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http local_selection_blocked profile={profile_name} route=compact reason=quota_exhausted_before_send"
                    ),
                );
                excluded_profiles.insert(profile_name);
            }
        }
    }
}

pub(super) fn attempt_runtime_noncompact_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<RuntimeStandardAttempt> {
    let request_session_id = runtime_request_session_id(request);
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Standard)?;
    if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http standard_pre_send_skip profile={profile_name} route=standard quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeStandardAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
        });
    }
    let _inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "standard_http")?;
    let response =
        send_runtime_proxy_upstream_request(request_id, request, shared, profile_name, None)
            .inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                    "standard_upstream_request",
                    err,
                );
            })?;
    if request.path_and_query.ends_with("/backend-api/wham/usage") {
        let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
            .inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                    "standard_buffer_usage_response",
                    err,
                );
            })?;
        if let Ok(usage) = serde_json::from_slice::<UsageResponse>(&parts.body) {
            update_runtime_profile_probe_cache_with_usage(shared, profile_name, usage)?;
        }
        remember_runtime_session_id(
            shared,
            profile_name,
            request_session_id.as_deref(),
            RuntimeRouteKind::Standard,
        )?;
        return Ok(RuntimeStandardAttempt::Success {
            profile_name: profile_name.to_string(),
            response: build_runtime_proxy_response_from_parts(parts),
        });
    }
    if response.status().is_success() {
        remember_runtime_session_id(
            shared,
            profile_name,
            request_session_id.as_deref(),
            RuntimeRouteKind::Standard,
        )?;
        let response =
            forward_runtime_proxy_response(shared, response, Vec::new()).inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Standard,
                    "standard_forward_response",
                    err,
                );
            })?;
        return Ok(RuntimeStandardAttempt::Success {
            profile_name: profile_name.to_string(),
            response,
        });
    }

    let status = response.status().as_u16();
    let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
        .inspect_err(|err| {
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Standard,
                "standard_buffer_response",
                err,
            );
        })?;
    let retryable_quota =
        matches!(status, 403 | 429) && extract_runtime_proxy_quota_message(&parts.body).is_some();
    if matches!(status, 403 | 429) && !retryable_quota {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http standard_quota_unclassified profile={profile_name} status={status} body_snippet={}",
                runtime_proxy_body_snippet(&parts.body, 240),
            ),
        );
    }
    let response = build_runtime_proxy_response_from_parts(parts);

    if retryable_quota {
        return Ok(RuntimeStandardAttempt::RetryableFailure {
            profile_name: profile_name.to_string(),
            response,
            overload: false,
        });
    }

    if matches!(status, 401 | 403) {
        note_runtime_profile_auth_failure(shared, profile_name, RuntimeRouteKind::Standard, status);
    }

    remember_runtime_session_id(
        shared,
        profile_name,
        request_session_id.as_deref(),
        RuntimeRouteKind::Standard,
    )?;
    Ok(RuntimeStandardAttempt::Success {
        profile_name: profile_name.to_string(),
        response,
    })
}

pub(super) fn attempt_runtime_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    allow_quota_exhausted_send: bool,
) -> Result<RuntimeStandardAttempt> {
    let request_session_id = runtime_request_session_id(request);
    let (quota_summary, quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Compact)?;
    if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted
        && !allow_quota_exhausted_send
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http standard_pre_send_skip profile={profile_name} route=compact quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeStandardAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
        });
    } else if quota_summary.route_band == RuntimeQuotaPressureBand::Exhausted {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_pre_send_allow_quota_exhausted profile={profile_name} quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
    }
    let _inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "compact_http")?;
    let response =
        send_runtime_proxy_upstream_request(request_id, request, shared, profile_name, None)
            .inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Compact,
                    "compact_upstream_request",
                    err,
                );
            })?;
    if !is_runtime_compact_path(&request.path_and_query) || response.status().is_success() {
        let response_turn_state = is_runtime_compact_path(&request.path_and_query)
            .then(|| runtime_proxy_header_value(response.headers(), "x-codex-turn-state"))
            .flatten();
        let response =
            forward_runtime_proxy_response(shared, response, Vec::new()).inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Compact,
                    "compact_forward_response",
                    err,
                );
            })?;
        remember_runtime_session_id(
            shared,
            profile_name,
            request_session_id.as_deref(),
            if is_runtime_compact_path(&request.path_and_query) {
                RuntimeRouteKind::Compact
            } else {
                RuntimeRouteKind::Standard
            },
        )?;
        if is_runtime_compact_path(&request.path_and_query) {
            remember_runtime_compact_lineage(
                shared,
                profile_name,
                request_session_id.as_deref(),
                response_turn_state.as_deref(),
                RuntimeRouteKind::Compact,
            )?;
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http compact_committed_owner profile={profile_name} session={} turn_state={}",
                    request_session_id.as_deref().unwrap_or("-"),
                    response_turn_state.as_deref().unwrap_or("-"),
                ),
            );
        }
        return Ok(RuntimeStandardAttempt::Success {
            profile_name: profile_name.to_string(),
            response,
        });
    }

    let status = response.status().as_u16();
    let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
        .inspect_err(|err| {
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Compact,
                "compact_buffer_response",
                err,
            );
        })?;
    let retryable_quota =
        matches!(status, 403 | 429) && extract_runtime_proxy_quota_message(&parts.body).is_some();
    let retryable_overload = extract_runtime_proxy_overload_message(status, &parts.body).is_some();
    if matches!(status, 403 | 429) && !retryable_quota {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_quota_unclassified profile={profile_name} status={status} body_snippet={}",
                runtime_proxy_body_snippet(&parts.body, 240),
            ),
        );
    }
    let response = build_runtime_proxy_response_from_parts(parts);

    if retryable_quota || retryable_overload {
        return Ok(RuntimeStandardAttempt::RetryableFailure {
            profile_name: profile_name.to_string(),
            response,
            overload: retryable_overload,
        });
    }

    if matches!(status, 401 | 403) {
        note_runtime_profile_auth_failure(shared, profile_name, RuntimeRouteKind::Compact, status);
    }

    Ok(RuntimeStandardAttempt::Success {
        profile_name: profile_name.to_string(),
        response,
    })
}

pub(super) fn proxy_runtime_anthropic_messages_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    let translated_request = match translate_runtime_anthropic_messages_request(request) {
        Ok(translated_request) => translated_request,
        Err(err) => {
            return Ok(RuntimeResponsesReply::Buffered(
                build_runtime_anthropic_error_parts(400, "invalid_request_error", &err.to_string()),
            ));
        }
    };
    if std::env::var_os("PRODEX_DEBUG_ANTHROPIC_COMPAT").is_some() {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http anthropic_translated path={} headers={:?} body_snippet={}",
                translated_request.translated_request.path_and_query,
                translated_request.translated_request.headers,
                runtime_proxy_body_snippet(&translated_request.translated_request.body, 2048),
            ),
        );
    }
    let response = proxy_runtime_responses_request(
        request_id,
        &translated_request.translated_request,
        shared,
    )?;
    translate_runtime_responses_reply_to_anthropic(
        response,
        &translated_request,
        request_id,
        shared,
    )
}

pub(super) fn proxy_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    let mut request = request.clone();
    let request_requires_previous_response_affinity =
        runtime_request_requires_previous_response_affinity(&request);
    let mut previous_response_id = runtime_request_previous_response_id(&request);
    let mut request_turn_state = runtime_request_turn_state(&request);
    let request_session_id = runtime_request_session_id(&request);
    let mut bound_profile = previous_response_id
        .as_deref()
        .map(|response_id| {
            runtime_response_bound_profile(shared, response_id, RuntimeRouteKind::Responses)
        })
        .transpose()?
        .flatten();
    let mut trusted_previous_response_affinity = runtime_previous_response_affinity_is_trusted(
        shared,
        previous_response_id.as_deref(),
        bound_profile.as_deref(),
    )?;
    let mut turn_state_profile = request_turn_state
        .as_deref()
        .map(|value| runtime_turn_state_bound_profile(shared, value))
        .transpose()?
        .flatten();
    let mut compact_followup_profile = if previous_response_id.is_none()
        && bound_profile.is_none()
        && turn_state_profile.is_none()
    {
        runtime_compact_followup_bound_profile(
            shared,
            request_turn_state.as_deref(),
            request_session_id.as_deref(),
        )?
    } else {
        None
    };
    if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_followup_owner profile={profile_name} source={source}"
            ),
        );
    }
    let mut session_profile = if previous_response_id.is_none()
        && bound_profile.is_none()
        && turn_state_profile.is_none()
        && compact_followup_profile.is_none()
    {
        request_session_id
            .as_deref()
            .map(|session_id| runtime_session_bound_profile(shared, session_id))
            .transpose()?
            .flatten()
    } else {
        None
    };
    let mut pinned_profile = bound_profile.clone().or(compact_followup_profile
        .as_ref()
        .map(|(profile_name, _)| profile_name.clone()));
    let mut excluded_profiles = BTreeSet::new();
    let mut last_failure = None;
    let mut previous_response_retry_candidate: Option<String> = None;
    let mut previous_response_retry_index = 0usize;
    let mut candidate_turn_state_retry_profile: Option<String> = None;
    let mut candidate_turn_state_retry_value: Option<String> = None;
    let mut saw_inflight_saturation = false;
    let mut selection_started_at = Instant::now();
    let mut selection_attempts = 0usize;
    let mut previous_response_fresh_fallback_used = false;
    let mut saw_previous_response_not_found = false;

    loop {
        let pressure_mode =
            runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Responses);
        if runtime_proxy_precommit_budget_exhausted(
            selection_started_at,
            selection_attempts,
            runtime_proxy_has_continuation_priority(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
            ),
            pressure_mode,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http precommit_budget_exhausted attempts={selection_attempts} elapsed_ms={} pressure_mode={pressure_mode}",
                    selection_started_at.elapsed().as_millis()
                ),
            );
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_request_requires_previous_response_affinity(&request)
                && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http previous_response_fresh_fallback reason=precommit_budget_exhausted"
                    ),
                );
                request = fresh_request;
                previous_response_id = None;
                request_turn_state = None;
                previous_response_fresh_fallback_used = true;
                saw_previous_response_not_found = false;
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                candidate_turn_state_retry_profile = None;
                candidate_turn_state_retry_value = None;
                trusted_previous_response_affinity = false;
                bound_profile = None;
                pinned_profile = None;
                turn_state_profile = None;
                session_profile = None;
                excluded_profiles.clear();
                last_failure = None;
                selection_started_at = Instant::now();
                selection_attempts = 0;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=precommit_budget_exhausted"
                    ),
                );
                return Ok(match last_failure {
                    Some(RuntimeUpstreamFailureResponse::Http(response)) => response,
                    _ if saw_inflight_saturation => {
                        RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                            503,
                            "service_unavailable",
                            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                        ))
                    }
                    _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    )),
                });
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Responses,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http direct_current_profile_fallback profile={current_profile} reason=precommit_budget_exhausted"
                    ),
                );
                match attempt_runtime_responses_request(
                    request_id,
                    &request,
                    shared,
                    &current_profile,
                    request_turn_state.as_deref(),
                )? {
                    RuntimeResponsesAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        if saw_previous_response_not_found {
                            remember_runtime_successful_previous_response_owner(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                RuntimeRouteKind::Responses,
                            )?;
                        }
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Responses,
                        )?;
                        let _ = release_runtime_compact_lineage(
                            shared,
                            &profile_name,
                            request_session_id.as_deref(),
                            request_turn_state.as_deref(),
                            "response_committed_post_commit",
                        );
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http committed profile={profile_name} via=direct_current_profile_fallback"
                            ),
                        );
                        return Ok(response);
                    }
                    RuntimeResponsesAttempt::QuotaBlocked {
                        profile_name,
                        response,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            RuntimeRouteKind::Responses,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                            pinned_profile.as_deref(),
                            turn_state_profile.as_deref(),
                            session_profile.as_deref(),
                            trusted_previous_response_affinity,
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(response);
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                            previous_response_retry_index = 0;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if previous_response_id.is_some()
                            && trusted_previous_response_affinity
                            && !previous_response_fresh_fallback_used
                            && !request_requires_previous_response_affinity
                            && let Some(fresh_request) =
                                runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            request = fresh_request;
                            previous_response_id = None;
                            request_turn_state = None;
                            previous_response_fresh_fallback_used = true;
                            saw_previous_response_not_found = false;
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                            trusted_previous_response_affinity = false;
                            bound_profile = None;
                            pinned_profile = None;
                            turn_state_profile = None;
                            session_profile = None;
                            excluded_profiles.clear();
                            last_failure = None;
                            selection_started_at = Instant::now();
                            selection_attempts = 0;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                        continue;
                    }
                    RuntimeResponsesAttempt::PreviousResponseNotFound {
                        profile_name,
                        response,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry && !request_requires_previous_response_affinity {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Responses,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if compact_followup_profile
                            .as_ref()
                            .is_some_and(|(owner, _)| owner == &profile_name)
                        {
                            compact_followup_profile = None;
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                        continue;
                    }
                    RuntimeResponsesAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            RuntimeRouteKind::Responses,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                            pinned_profile.as_deref(),
                            turn_state_profile.as_deref(),
                            session_profile.as_deref(),
                            trusted_previous_response_affinity,
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(RuntimeResponsesReply::Buffered(
                                build_runtime_proxy_json_error_parts(
                                    503,
                                    "service_unavailable",
                                    runtime_proxy_local_selection_failure_message(),
                                ),
                            ));
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                            previous_response_retry_index = 0;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if previous_response_id.is_some()
                            && trusted_previous_response_affinity
                            && !previous_response_fresh_fallback_used
                            && !request_requires_previous_response_affinity
                            && let Some(fresh_request) =
                                runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            request = fresh_request;
                            previous_response_id = None;
                            request_turn_state = None;
                            previous_response_fresh_fallback_used = true;
                            saw_previous_response_not_found = false;
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                            trusted_previous_response_affinity = false;
                            bound_profile = None;
                            session_profile = None;
                            pinned_profile = None;
                            turn_state_profile = None;
                            excluded_profiles.clear();
                            last_failure = None;
                            selection_started_at = Instant::now();
                            selection_attempts = 0;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            return Ok(match last_failure {
                Some(RuntimeUpstreamFailureResponse::Http(response)) => response,
                _ if saw_inflight_saturation => {
                    RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                        503,
                        "service_unavailable",
                        "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                    ))
                }
                _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                )),
            });
        }

        let Some(candidate_name) = select_runtime_response_candidate_for_route(
            shared,
            &excluded_profiles,
            compact_followup_profile
                .as_ref()
                .map(|(profile_name, _)| profile_name.as_str()),
            pinned_profile.as_deref(),
            turn_state_profile.as_deref(),
            session_profile.as_deref(),
            previous_response_id.is_some(),
            previous_response_id.as_deref(),
            RuntimeRouteKind::Responses,
        )?
        else {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http candidate_exhausted last_failure={}",
                    match &last_failure {
                        Some(RuntimeUpstreamFailureResponse::Http(_)) => "http",
                        Some(RuntimeUpstreamFailureResponse::Websocket(_)) => "websocket",
                        None => "none",
                    }
                ),
            );
            if runtime_proxy_maybe_wait_for_interactive_inflight_relief(
                request_id,
                &request,
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Responses,
                selection_started_at,
                runtime_proxy_has_continuation_priority(
                    previous_response_id.as_deref(),
                    pinned_profile.as_deref(),
                    request_turn_state.as_deref(),
                    turn_state_profile.as_deref(),
                    session_profile.as_deref(),
                ),
                runtime_wait_affinity_owner(
                    compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    pinned_profile.as_deref(),
                    turn_state_profile.as_deref(),
                    session_profile.as_deref(),
                    trusted_previous_response_affinity,
                ),
            )? {
                continue;
            }
            if previous_response_id.is_some()
                && saw_previous_response_not_found
                && !previous_response_fresh_fallback_used
                && !runtime_request_requires_previous_response_affinity(&request)
                && let Some(fresh_request) =
                    runtime_request_without_previous_response_affinity(&request)
            {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http previous_response_fresh_fallback reason=candidate_exhausted"
                    ),
                );
                request = fresh_request;
                previous_response_id = None;
                request_turn_state = None;
                previous_response_fresh_fallback_used = true;
                saw_previous_response_not_found = false;
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                candidate_turn_state_retry_profile = None;
                candidate_turn_state_retry_value = None;
                trusted_previous_response_affinity = false;
                bound_profile = None;
                pinned_profile = None;
                turn_state_profile = None;
                session_profile = None;
                excluded_profiles.clear();
                last_failure = None;
                selection_started_at = Instant::now();
                selection_attempts = 0;
                continue;
            }
            if let Some((profile_name, source)) = compact_followup_profile.as_ref() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http compact_fresh_fallback_blocked profile={profile_name} source={source} reason=candidate_exhausted"
                    ),
                );
                return Ok(match last_failure {
                    Some(RuntimeUpstreamFailureResponse::Http(response)) => response,
                    _ if saw_inflight_saturation => {
                        RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                            503,
                            "service_unavailable",
                            "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                        ))
                    }
                    _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                        503,
                        "service_unavailable",
                        runtime_proxy_local_selection_failure_message(),
                    )),
                });
            }
            if runtime_proxy_allows_direct_current_profile_fallback(
                previous_response_id.as_deref(),
                pinned_profile.as_deref(),
                request_turn_state.as_deref(),
                turn_state_profile.as_deref(),
                session_profile.as_deref(),
                saw_inflight_saturation,
                last_failure.is_some(),
            ) && let Some(current_profile) = runtime_proxy_direct_current_fallback_profile(
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Responses,
            )? {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http direct_current_profile_fallback profile={current_profile} reason=candidate_exhausted"
                    ),
                );
                match attempt_runtime_responses_request(
                    request_id,
                    &request,
                    shared,
                    &current_profile,
                    request_turn_state.as_deref(),
                )? {
                    RuntimeResponsesAttempt::Success {
                        profile_name,
                        response,
                    } => {
                        if saw_previous_response_not_found {
                            remember_runtime_successful_previous_response_owner(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                                RuntimeRouteKind::Responses,
                            )?;
                        }
                        commit_runtime_proxy_profile_selection_with_notice(
                            shared,
                            &profile_name,
                            RuntimeRouteKind::Responses,
                        )?;
                        let _ = release_runtime_compact_lineage(
                            shared,
                            &profile_name,
                            request_session_id.as_deref(),
                            request_turn_state.as_deref(),
                            "response_committed_post_commit",
                        );
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http committed profile={profile_name} via=direct_current_profile_fallback"
                            ),
                        );
                        return Ok(response);
                    }
                    RuntimeResponsesAttempt::QuotaBlocked {
                        profile_name,
                        response,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            RuntimeRouteKind::Responses,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                            pinned_profile.as_deref(),
                            turn_state_profile.as_deref(),
                            session_profile.as_deref(),
                            trusted_previous_response_affinity,
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(response);
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                            previous_response_retry_index = 0;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if previous_response_id.is_some()
                            && trusted_previous_response_affinity
                            && !previous_response_fresh_fallback_used
                            && !request_requires_previous_response_affinity
                            && let Some(fresh_request) =
                                runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason=quota_blocked via=direct_current_profile_fallback"
                                ),
                            );
                            request = fresh_request;
                            previous_response_id = None;
                            request_turn_state = None;
                            previous_response_fresh_fallback_used = true;
                            saw_previous_response_not_found = false;
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                            trusted_previous_response_affinity = false;
                            bound_profile = None;
                            pinned_profile = None;
                            turn_state_profile = None;
                            session_profile = None;
                            excluded_profiles.clear();
                            last_failure = None;
                            selection_started_at = Instant::now();
                            selection_attempts = 0;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                        continue;
                    }
                    RuntimeResponsesAttempt::PreviousResponseNotFound {
                        profile_name,
                        response,
                        turn_state,
                    } => {
                        runtime_proxy_log(
                            shared,
                            format!(
                                "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?} via=direct_current_profile_fallback",
                                turn_state
                            ),
                        );
                        saw_previous_response_not_found = true;
                        if previous_response_retry_candidate.as_deref()
                            != Some(profile_name.as_str())
                        {
                            previous_response_retry_candidate = Some(profile_name.clone());
                            previous_response_retry_index = 0;
                        }
                        let has_turn_state_retry = turn_state.is_some();
                        if has_turn_state_retry {
                            candidate_turn_state_retry_profile = Some(profile_name.clone());
                            candidate_turn_state_retry_value = turn_state;
                        }
                        if has_turn_state_retry
                            && let Some(delay) =
                                runtime_previous_response_retry_delay(previous_response_retry_index)
                        {
                            previous_response_retry_index += 1;
                            last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry via=direct_current_profile_fallback",
                                    delay.as_millis()
                                ),
                            );
                            continue;
                        }
                        previous_response_retry_candidate = None;
                        previous_response_retry_index = 0;
                        if !has_turn_state_retry && !request_requires_previous_response_affinity {
                            let _ = clear_runtime_stale_previous_response_binding(
                                shared,
                                &profile_name,
                                previous_response_id.as_deref(),
                            )?;
                        }
                        let released_affinity = release_runtime_previous_response_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                            RuntimeRouteKind::Responses,
                        )?;
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_affinity_released profile={profile_name} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if compact_followup_profile
                            .as_ref()
                            .is_some_and(|(owner, _)| owner == &profile_name)
                        {
                            compact_followup_profile = None;
                        }
                        excluded_profiles.insert(profile_name);
                        last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                        continue;
                    }
                    RuntimeResponsesAttempt::LocalSelectionBlocked {
                        profile_name,
                        reason,
                    } => {
                        mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                        if !runtime_quota_blocked_affinity_is_releasable(
                            RuntimeRouteKind::Responses,
                            &profile_name,
                            compact_followup_profile
                                .as_ref()
                                .map(|(profile_name, _)| profile_name.as_str()),
                            pinned_profile.as_deref(),
                            turn_state_profile.as_deref(),
                            session_profile.as_deref(),
                            trusted_previous_response_affinity,
                            request_requires_previous_response_affinity,
                        ) {
                            return Ok(RuntimeResponsesReply::Buffered(
                                build_runtime_proxy_json_error_parts(
                                    503,
                                    "service_unavailable",
                                    runtime_proxy_local_selection_failure_message(),
                                ),
                            ));
                        }
                        let released_affinity = release_runtime_quota_blocked_affinity(
                            shared,
                            &profile_name,
                            previous_response_id.as_deref(),
                            request_turn_state.as_deref(),
                            request_session_id.as_deref(),
                        )?;
                        if bound_profile.as_deref() == Some(profile_name.as_str()) {
                            bound_profile = None;
                        }
                        if session_profile.as_deref() == Some(profile_name.as_str()) {
                            session_profile = None;
                        }
                        if candidate_turn_state_retry_profile.as_deref()
                            == Some(profile_name.as_str())
                        {
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                        }
                        if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                            pinned_profile = None;
                            previous_response_retry_index = 0;
                        }
                        if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                            turn_state_profile = None;
                        }
                        if released_affinity {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                        }
                        if previous_response_id.is_some()
                            && trusted_previous_response_affinity
                            && !previous_response_fresh_fallback_used
                            && !request_requires_previous_response_affinity
                            && let Some(fresh_request) =
                                runtime_request_without_previous_response_affinity(&request)
                        {
                            runtime_proxy_log(
                                shared,
                                format!(
                                    "request={request_id} transport=http previous_response_fresh_fallback reason={reason} via=direct_current_profile_fallback"
                                ),
                            );
                            request = fresh_request;
                            previous_response_id = None;
                            request_turn_state = None;
                            previous_response_fresh_fallback_used = true;
                            saw_previous_response_not_found = false;
                            previous_response_retry_candidate = None;
                            previous_response_retry_index = 0;
                            candidate_turn_state_retry_profile = None;
                            candidate_turn_state_retry_value = None;
                            trusted_previous_response_affinity = false;
                            bound_profile = None;
                            session_profile = None;
                            pinned_profile = None;
                            turn_state_profile = None;
                            excluded_profiles.clear();
                            last_failure = None;
                            selection_started_at = Instant::now();
                            selection_attempts = 0;
                            continue;
                        }
                        excluded_profiles.insert(profile_name);
                        continue;
                    }
                }
            }
            return Ok(match last_failure {
                Some(RuntimeUpstreamFailureResponse::Http(response)) => response,
                _ if saw_inflight_saturation => {
                    RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                        503,
                        "service_unavailable",
                        "All runtime auto-rotate candidates are temporarily saturated. Retry the request.",
                    ))
                }
                _ => RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
                    503,
                    "service_unavailable",
                    runtime_proxy_local_selection_failure_message(),
                )),
            });
        };
        selection_attempts = selection_attempts.saturating_add(1);
        let turn_state_override =
            if candidate_turn_state_retry_profile.as_deref() == Some(candidate_name.as_str()) {
                candidate_turn_state_retry_value.as_deref()
            } else {
                request_turn_state.as_deref()
            };
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http candidate={} pinned={:?} turn_state_profile={:?} turn_state_override={:?} excluded_count={}",
                candidate_name,
                pinned_profile,
                turn_state_profile,
                turn_state_override,
                excluded_profiles.len()
            ),
        );
        if previous_response_id.is_none()
            && pinned_profile.is_none()
            && turn_state_profile.is_none()
            && runtime_profile_inflight_hard_limited_for_context(
                shared,
                &candidate_name,
                "responses_http",
            )?
        {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http profile_inflight_saturated profile={candidate_name} hard_limit={}",
                    runtime_proxy_profile_inflight_hard_limit(),
                ),
            );
            saw_inflight_saturation = true;
            if runtime_proxy_maybe_wait_for_interactive_inflight_relief(
                request_id,
                &request,
                shared,
                &excluded_profiles,
                RuntimeRouteKind::Responses,
                selection_started_at,
                runtime_proxy_has_continuation_priority(
                    previous_response_id.as_deref(),
                    pinned_profile.as_deref(),
                    request_turn_state.as_deref(),
                    turn_state_profile.as_deref(),
                    session_profile.as_deref(),
                ),
                runtime_wait_affinity_owner(
                    compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    pinned_profile.as_deref(),
                    turn_state_profile.as_deref(),
                    session_profile.as_deref(),
                    trusted_previous_response_affinity,
                ),
            )? {
                continue;
            }
            excluded_profiles.insert(candidate_name);
            continue;
        }

        match attempt_runtime_responses_request(
            request_id,
            &request,
            shared,
            &candidate_name,
            turn_state_override,
        )? {
            RuntimeResponsesAttempt::Success {
                profile_name,
                response,
            } => {
                if saw_previous_response_not_found {
                    remember_runtime_successful_previous_response_owner(
                        shared,
                        &profile_name,
                        previous_response_id.as_deref(),
                        RuntimeRouteKind::Responses,
                    )?;
                }
                commit_runtime_proxy_profile_selection_with_notice(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Responses,
                )?;
                let _ = release_runtime_compact_lineage(
                    shared,
                    &profile_name,
                    request_session_id.as_deref(),
                    request_turn_state.as_deref(),
                    "response_committed_post_commit",
                );
                runtime_proxy_log(
                    shared,
                    format!("request={request_id} transport=http committed profile={profile_name}"),
                );
                return Ok(response);
            }
            RuntimeResponsesAttempt::QuotaBlocked {
                profile_name,
                response,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http quota_blocked profile={profile_name}"
                    ),
                );
                let quota_message =
                    extract_runtime_proxy_quota_message_from_response_reply(&response);
                mark_runtime_profile_quota_quarantine(
                    shared,
                    &profile_name,
                    RuntimeRouteKind::Responses,
                    quota_message.as_deref(),
                )?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    RuntimeRouteKind::Responses,
                    &profile_name,
                    compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    pinned_profile.as_deref(),
                    turn_state_profile.as_deref(),
                    session_profile.as_deref(),
                    trusted_previous_response_affinity,
                    request_requires_previous_response_affinity,
                ) {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http upstream_usage_limit_passthrough route=responses profile={profile_name} reason=hard_affinity"
                        ),
                    );
                    return Ok(response);
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                    previous_response_retry_index = 0;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name}"
                        ),
                    );
                }
                if previous_response_id.is_some()
                    && trusted_previous_response_affinity
                    && !previous_response_fresh_fallback_used
                    && !request_requires_previous_response_affinity
                    && let Some(fresh_request) =
                        runtime_request_without_previous_response_affinity(&request)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_fresh_fallback reason=quota_blocked"
                        ),
                    );
                    request = fresh_request;
                    previous_response_id = None;
                    request_turn_state = None;
                    previous_response_fresh_fallback_used = true;
                    saw_previous_response_not_found = false;
                    previous_response_retry_candidate = None;
                    previous_response_retry_index = 0;
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                    trusted_previous_response_affinity = false;
                    bound_profile = None;
                    pinned_profile = None;
                    turn_state_profile = None;
                    session_profile = None;
                    excluded_profiles.clear();
                    last_failure = None;
                    selection_started_at = Instant::now();
                    selection_attempts = 0;
                    continue;
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
            }
            RuntimeResponsesAttempt::LocalSelectionBlocked {
                profile_name,
                reason,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http local_selection_blocked profile={profile_name} route=responses reason={reason}"
                    ),
                );
                mark_runtime_profile_retry_backoff(shared, &profile_name)?;
                if !runtime_quota_blocked_affinity_is_releasable(
                    RuntimeRouteKind::Responses,
                    &profile_name,
                    compact_followup_profile
                        .as_ref()
                        .map(|(profile_name, _)| profile_name.as_str()),
                    pinned_profile.as_deref(),
                    turn_state_profile.as_deref(),
                    session_profile.as_deref(),
                    trusted_previous_response_affinity,
                    request_requires_previous_response_affinity,
                ) {
                    return Ok(RuntimeResponsesReply::Buffered(
                        build_runtime_proxy_json_error_parts(
                            503,
                            "service_unavailable",
                            runtime_proxy_local_selection_failure_message(),
                        ),
                    ));
                }
                let released_affinity = release_runtime_quota_blocked_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                )?;
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                    previous_response_retry_index = 0;
                }
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http quota_blocked_affinity_released profile={profile_name} reason={reason}"
                        ),
                    );
                }
                if previous_response_id.is_some()
                    && trusted_previous_response_affinity
                    && !previous_response_fresh_fallback_used
                    && !request_requires_previous_response_affinity
                    && let Some(fresh_request) =
                        runtime_request_without_previous_response_affinity(&request)
                {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_fresh_fallback reason={reason}"
                        ),
                    );
                    request = fresh_request;
                    previous_response_id = None;
                    request_turn_state = None;
                    previous_response_fresh_fallback_used = true;
                    saw_previous_response_not_found = false;
                    previous_response_retry_candidate = None;
                    previous_response_retry_index = 0;
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                    trusted_previous_response_affinity = false;
                    bound_profile = None;
                    pinned_profile = None;
                    turn_state_profile = None;
                    session_profile = None;
                    excluded_profiles.clear();
                    last_failure = None;
                    selection_started_at = Instant::now();
                    selection_attempts = 0;
                    continue;
                }
                excluded_profiles.insert(profile_name);
            }
            RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name,
                response,
                turn_state,
            } => {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} retry_index={previous_response_retry_index} replay_turn_state={:?}",
                        turn_state
                    ),
                );
                saw_previous_response_not_found = true;
                if previous_response_retry_candidate.as_deref() != Some(profile_name.as_str()) {
                    previous_response_retry_candidate = Some(profile_name.clone());
                    previous_response_retry_index = 0;
                }
                let has_turn_state_retry = turn_state.is_some();
                if has_turn_state_retry {
                    candidate_turn_state_retry_profile = Some(profile_name.clone());
                    candidate_turn_state_retry_value = turn_state;
                }
                if has_turn_state_retry
                    && let Some(delay) =
                        runtime_previous_response_retry_delay(previous_response_retry_index)
                {
                    previous_response_retry_index += 1;
                    last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_retry_immediate profile={profile_name} delay_ms={} reason=non_blocking_retry",
                            delay.as_millis()
                        ),
                    );
                    continue;
                }
                previous_response_retry_candidate = None;
                previous_response_retry_index = 0;
                if !has_turn_state_retry && !request_requires_previous_response_affinity {
                    let _ = clear_runtime_stale_previous_response_binding(
                        shared,
                        &profile_name,
                        previous_response_id.as_deref(),
                    )?;
                }
                let released_affinity = release_runtime_previous_response_affinity(
                    shared,
                    &profile_name,
                    previous_response_id.as_deref(),
                    request_turn_state.as_deref(),
                    request_session_id.as_deref(),
                    RuntimeRouteKind::Responses,
                )?;
                if released_affinity {
                    runtime_proxy_log(
                        shared,
                        format!(
                            "request={request_id} transport=http previous_response_affinity_released profile={profile_name}"
                        ),
                    );
                }
                if bound_profile.as_deref() == Some(profile_name.as_str()) {
                    bound_profile = None;
                }
                if session_profile.as_deref() == Some(profile_name.as_str()) {
                    session_profile = None;
                }
                if candidate_turn_state_retry_profile.as_deref() == Some(profile_name.as_str()) {
                    candidate_turn_state_retry_profile = None;
                    candidate_turn_state_retry_value = None;
                }
                if pinned_profile.as_deref() == Some(profile_name.as_str()) {
                    pinned_profile = None;
                }
                trusted_previous_response_affinity = false;
                if turn_state_profile.as_deref() == Some(profile_name.as_str()) {
                    turn_state_profile = None;
                }
                if compact_followup_profile
                    .as_ref()
                    .is_some_and(|(owner, _)| owner == &profile_name)
                {
                    compact_followup_profile = None;
                }
                excluded_profiles.insert(profile_name);
                last_failure = Some(RuntimeUpstreamFailureResponse::Http(response));
            }
        }
    }
}

pub(super) fn attempt_runtime_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<RuntimeResponsesAttempt> {
    let request_session_id = runtime_request_session_id(request);
    let request_previous_response_id = runtime_request_previous_response_id(request);
    let request_turn_state = runtime_request_turn_state(request);
    let (initial_quota_summary, initial_quota_source) =
        runtime_profile_quota_summary_for_route(shared, profile_name, RuntimeRouteKind::Responses)?;
    if (request_previous_response_id.is_some()
        || request_session_id.is_some()
        || request_turn_state.is_some())
        && matches!(
            initial_quota_source,
            Some(RuntimeQuotaSource::PersistedSnapshot)
        )
        && let Some(reason) =
            runtime_quota_precommit_guard_reason(initial_quota_summary, RuntimeRouteKind::Responses)
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason={reason} quota_source={} {}",
                initial_quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(initial_quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }
    let has_alternative_quota_profile =
        runtime_has_alternative_quota_compatible_profile(shared, profile_name)?;
    let (quota_summary, quota_source) = ensure_runtime_profile_precommit_quota_ready(
        shared,
        profile_name,
        RuntimeRouteKind::Responses,
        "responses_precommit_reprobe",
    )?;
    if runtime_quota_summary_requires_live_source_after_probe(
        quota_summary,
        quota_source,
        RuntimeRouteKind::Responses,
    ) && has_alternative_quota_profile
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason=quota_windows_unavailable_after_reprobe quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason: "quota_windows_unavailable_after_reprobe",
        });
    }
    if let Some(reason) =
        runtime_quota_precommit_guard_reason(quota_summary, RuntimeRouteKind::Responses)
    {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http responses_pre_send_skip profile={profile_name} route=responses reason={reason} quota_source={} {}",
                quota_source
                    .map(runtime_quota_source_label)
                    .unwrap_or("unknown"),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        return Ok(RuntimeResponsesAttempt::LocalSelectionBlocked {
            profile_name: profile_name.to_string(),
            reason,
        });
    }
    let inflight_guard =
        acquire_runtime_profile_inflight_guard(shared, profile_name, "responses_http")?;
    let response = send_runtime_proxy_upstream_responses_request(
        request_id,
        request,
        shared,
        profile_name,
        turn_state_override,
    )
    .inspect_err(|err| {
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Responses,
            "responses_upstream_request",
            err,
        );
    })?;
    let response_turn_state = runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())
            .inspect_err(|err| {
                note_runtime_profile_transport_failure(
                    shared,
                    profile_name,
                    RuntimeRouteKind::Responses,
                    "responses_buffer_response",
                    err,
                );
            })?;
        let retryable_quota = matches!(status, 403 | 429)
            && extract_runtime_proxy_quota_message(&parts.body).is_some();
        let retryable_previous =
            status == 400 && extract_runtime_proxy_previous_response_message(&parts.body).is_some();
        let response = RuntimeResponsesReply::Buffered(parts);

        if retryable_quota {
            return Ok(RuntimeResponsesAttempt::QuotaBlocked {
                profile_name: profile_name.to_string(),
                response,
            });
        }
        if retryable_previous {
            return Ok(RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name: profile_name.to_string(),
                response,
                turn_state: response_turn_state,
            });
        }
        if matches!(status, 401 | 403) {
            note_runtime_profile_auth_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Responses,
                status,
            );
        }

        return Ok(RuntimeResponsesAttempt::Success {
            profile_name: profile_name.to_string(),
            response,
        });
    }
    prepare_runtime_proxy_responses_success(
        request_id,
        runtime_request_previous_response_id(request).as_deref(),
        request_session_id.as_deref(),
        runtime_request_turn_state(request).as_deref(),
        response,
        shared,
        profile_name,
        inflight_guard,
    )
    .inspect_err(|err| {
        note_runtime_profile_transport_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Responses,
            "responses_prepare_success",
            err,
        );
    })
}

pub(super) fn next_runtime_response_candidate_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<Option<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let sync_probe_pressure_mode = runtime_proxy_sync_probe_pressure_mode_active(shared);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let (
        state,
        current_profile,
        include_code_review,
        upstream_base_url,
        cached_reports,
        mut cached_usage_snapshots,
        profile_usage_auth,
        mut retry_backoff_until,
        mut transport_backoff_until,
        mut route_circuit_open_until,
        profile_inflight,
        profile_health,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.include_code_review,
            runtime.upstream_base_url.clone(),
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
        )
    };

    let mut reports = Vec::new();
    let mut cold_start_probe_jobs = Vec::new();
    for (order_index, name) in active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .enumerate()
    {
        if excluded_profiles.contains(&name) {
            continue;
        }
        let Some(profile) = state.profiles.get(&name) else {
            continue;
        };
        if let Some(entry) = cached_reports.get(&name) {
            reports.push(RunProfileProbeReport {
                name: name.clone(),
                order_index,
                auth: entry.auth.clone(),
                result: entry.result.clone(),
            });
            if runtime_profile_probe_cache_freshness(entry, now)
                != RuntimeProbeCacheFreshness::Fresh
            {
                schedule_runtime_probe_refresh(shared, &name, &profile.codex_home);
            }
        } else {
            let auth = read_auth_summary(&profile.codex_home);
            reports.push(RunProfileProbeReport {
                name: name.clone(),
                order_index,
                auth,
                result: Err("runtime quota snapshot unavailable".to_string()),
            });
            cold_start_probe_jobs.push(RunProfileProbeJob {
                name,
                order_index,
                codex_home: profile.codex_home.clone(),
            });
        }
    }

    cold_start_probe_jobs.sort_by_key(|job| {
        let quota_summary = cached_usage_snapshots
            .get(&job.name)
            .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
            .map(|snapshot| runtime_quota_summary_from_usage_snapshot(snapshot, route_kind))
            .unwrap_or(RuntimeQuotaSummary {
                five_hour: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                weekly: RuntimeQuotaWindowSummary {
                    status: RuntimeQuotaWindowStatus::Unknown,
                    remaining_percent: 0,
                    reset_at: i64::MAX,
                },
                route_band: RuntimeQuotaPressureBand::Unknown,
            });
        (
            runtime_quota_pressure_sort_key_for_route_from_summary(quota_summary),
            job.order_index,
        )
    });

    reports.sort_by_key(|report| report.order_index);
    let mut candidates = ready_profile_candidates(
        &reports,
        include_code_review,
        Some(current_profile.as_str()),
        &state,
        Some(&cached_usage_snapshots),
    );

    let best_candidate_order_index = candidates
        .iter()
        .filter(|candidate| !excluded_profiles.contains(&candidate.name))
        .filter(|candidate| {
            !runtime_profile_name_in_selection_backoff(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                &route_circuit_open_until,
                route_kind,
                now,
            )
        })
        .filter(|candidate| {
            !runtime_profile_auth_failure_active_with_auth_cache(
                &profile_health,
                &profile_usage_auth,
                &candidate.name,
                now,
            )
        })
        .filter(|candidate| {
            runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight)
                < inflight_soft_limit
        })
        .map(|candidate| candidate.order_index)
        .min();
    let should_sync_probe_cold_start = !sync_probe_pressure_mode
        && !cold_start_probe_jobs.is_empty()
        && (candidates.is_empty()
            || best_candidate_order_index.is_none()
            || best_candidate_order_index.is_some_and(|best_order_index| {
                cold_start_probe_jobs
                    .iter()
                    .any(|job| job.order_index < best_order_index)
            }));
    if sync_probe_pressure_mode && !cold_start_probe_jobs.is_empty() {
        runtime_proxy_log(
            shared,
            format!(
                "selection_skip_sync_probe route={} reason=pressure_mode cold_start_jobs={}",
                runtime_route_kind_label(route_kind),
                cold_start_probe_jobs.len(),
            ),
        );
    }

    if should_sync_probe_cold_start {
        let base_url = Some(upstream_base_url.clone());
        let sync_jobs = cold_start_probe_jobs
            .iter()
            .filter(|job| {
                candidates.is_empty()
                    || best_candidate_order_index.is_none()
                    || best_candidate_order_index
                        .is_some_and(|best_order_index| job.order_index < best_order_index)
            })
            .take(RUNTIME_PROFILE_SYNC_PROBE_FALLBACK_LIMIT)
            .map(|job| RunProfileProbeJob {
                name: job.name.clone(),
                order_index: job.order_index,
                codex_home: job.codex_home.clone(),
            })
            .collect::<Vec<_>>();
        let probed_names = sync_jobs
            .iter()
            .map(|job| job.name.clone())
            .collect::<BTreeSet<_>>();
        let fresh_reports = map_parallel(sync_jobs, |job| {
            let auth = read_auth_summary(&job.codex_home);
            let result = if auth.quota_compatible {
                fetch_usage(&job.codex_home, base_url.as_deref()).map_err(|err| err.to_string())
            } else {
                Err("auth mode is not quota-compatible".to_string())
            };

            RunProfileProbeReport {
                name: job.name,
                order_index: job.order_index,
                auth,
                result,
            }
        });

        for report in &fresh_reports {
            apply_runtime_profile_probe_result(
                shared,
                &report.name,
                report.auth.clone(),
                report.result.clone(),
            )?;
        }
        {
            let runtime = shared
                .runtime
                .lock()
                .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
            cached_usage_snapshots = runtime.profile_usage_snapshots.clone();
            retry_backoff_until = runtime.profile_retry_backoff_until.clone();
            transport_backoff_until = runtime.profile_transport_backoff_until.clone();
            route_circuit_open_until = runtime.profile_route_circuit_open_until.clone();
        }

        for fresh_report in fresh_reports {
            if let Some(existing) = reports
                .iter_mut()
                .find(|report| report.name == fresh_report.name)
            {
                *existing = fresh_report;
            }
        }
        reports.sort_by_key(|report| report.order_index);
        candidates = ready_profile_candidates(
            &reports,
            include_code_review,
            Some(current_profile.as_str()),
            &state,
            Some(&cached_usage_snapshots),
        );
        for job in cold_start_probe_jobs
            .into_iter()
            .filter(|job| !probed_names.contains(&job.name))
        {
            schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
        }
    } else {
        if sync_probe_pressure_mode && !cold_start_probe_jobs.is_empty() {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_sync_probe route={} reason=pressure_mode cold_start_profiles={}",
                    runtime_route_kind_label(route_kind),
                    cold_start_probe_jobs.len()
                ),
            );
        }
        for job in cold_start_probe_jobs {
            schedule_runtime_probe_refresh(shared, &job.name, &job.codex_home);
        }
    }
    let available_candidates = candidates
        .into_iter()
        .enumerate()
        .filter(|(_, candidate)| !excluded_profiles.contains(&candidate.name))
        .collect::<Vec<_>>();

    let mut ready_candidates = available_candidates
        .iter()
        .filter(|(_, candidate)| {
            !runtime_profile_name_in_selection_backoff(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                &route_circuit_open_until,
                route_kind,
                now,
            )
        })
        .collect::<Vec<_>>();
    ready_candidates.sort_by_key(|(index, candidate)| {
        (
            runtime_quota_pressure_sort_key_for_route(&candidate.usage, route_kind),
            runtime_quota_source_sort_key(route_kind, candidate.quota_source),
            runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
            runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
            *index,
            runtime_profile_selection_jitter(shared, &candidate.name, route_kind),
        )
    });
    for (index, candidate) in ready_candidates {
        let inflight = runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight);
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=auth_failure_backoff inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    inflight,
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(runtime_quota_summary_for_route(
                        &candidate.usage,
                        route_kind
                    )),
                ),
            );
            continue;
        }
        if inflight >= inflight_soft_limit {
            let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=profile_inflight_soft_limit inflight={} soft_limit={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    inflight,
                    inflight_soft_limit,
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        if !reserve_runtime_profile_route_circuit_half_open_probe(
            shared,
            &candidate.name,
            route_kind,
        )? {
            let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    inflight,
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile={} mode=ready inflight={} health={} order={} {}",
                runtime_route_kind_label(route_kind),
                candidate.name,
                inflight,
                runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
                index,
                format_args!(
                    "quota_source={} {}",
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary)
                ),
            ),
        );
        return Ok(Some(candidate.name.clone()));
    }

    let mut fallback_candidates = available_candidates.into_iter().collect::<Vec<_>>();
    fallback_candidates.sort_by_key(|(index, candidate)| {
        (
            runtime_profile_backoff_sort_key(
                &candidate.name,
                &retry_backoff_until,
                &transport_backoff_until,
                &route_circuit_open_until,
                route_kind,
                now,
            ),
            runtime_quota_pressure_sort_key_for_route(&candidate.usage, route_kind),
            runtime_quota_source_sort_key(route_kind, candidate.quota_source),
            runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
            runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
            *index,
            runtime_profile_selection_jitter(shared, &candidate.name, route_kind),
        )
    });
    let mut fallback = None;
    for (index, candidate) in fallback_candidates {
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=auth_failure_backoff inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(runtime_quota_summary_for_route(
                        &candidate.usage,
                        route_kind
                    )),
                ),
            );
            continue;
        }
        if !reserve_runtime_profile_route_circuit_half_open_probe(
            shared,
            &candidate.name,
            route_kind,
        )? {
            let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
            runtime_proxy_log(
                shared,
                format!(
                    "selection_skip_current route={} profile={} reason=route_circuit_half_open_probe_wait inflight={} health={} quota_source={} {}",
                    runtime_route_kind_label(route_kind),
                    candidate.name,
                    runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                    runtime_profile_health_sort_key(
                        &candidate.name,
                        &profile_health,
                        now,
                        route_kind
                    ),
                    runtime_quota_source_label(candidate.quota_source),
                    runtime_quota_summary_log_fields(quota_summary),
                ),
            );
            continue;
        }
        let quota_summary = runtime_quota_summary_for_route(&candidate.usage, route_kind);
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile={} mode=backoff inflight={} health={} backoff={:?} order={} quota_source={} {}",
                runtime_route_kind_label(route_kind),
                candidate.name,
                runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight),
                runtime_profile_health_sort_key(&candidate.name, &profile_health, now, route_kind),
                runtime_profile_backoff_sort_key(
                    &candidate.name,
                    &retry_backoff_until,
                    &transport_backoff_until,
                    &route_circuit_open_until,
                    route_kind,
                    now,
                ),
                index,
                runtime_quota_source_label(candidate.quota_source),
                runtime_quota_summary_log_fields(quota_summary),
            ),
        );
        fallback = Some(candidate.name);
        break;
    }

    if fallback.is_none() {
        runtime_proxy_log(
            shared,
            format!(
                "selection_pick route={} profile=none mode=exhausted excluded_count={}",
                runtime_route_kind_label(route_kind),
                excluded_profiles.len()
            ),
        );
    }

    Ok(fallback)
}

pub(super) fn runtime_waitable_inflight_candidates_for_route(
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    wait_affinity_owner: Option<&str>,
) -> Result<BTreeSet<String>> {
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let (
        state,
        current_profile,
        include_code_review,
        cached_reports,
        cached_usage_snapshots,
        profile_usage_auth,
        retry_backoff_until,
        transport_backoff_until,
        route_circuit_open_until,
        profile_inflight,
        profile_health,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.include_code_review,
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
        )
    };

    let mut waitable_profiles = BTreeSet::new();
    let mut reports = Vec::new();
    for (order_index, name) in active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .enumerate()
    {
        if excluded_profiles.contains(&name) {
            continue;
        }
        if wait_affinity_owner.is_some_and(|owner| owner != name) {
            continue;
        }
        let Some(entry) = cached_reports.get(&name) else {
            continue;
        };
        reports.push(RunProfileProbeReport {
            name,
            order_index,
            auth: entry.auth.clone(),
            result: entry.result.clone(),
        });
    }

    for candidate in ready_profile_candidates(
        &reports,
        include_code_review,
        Some(current_profile.as_str()),
        &state,
        Some(&cached_usage_snapshots),
    ) {
        if excluded_profiles.contains(&candidate.name) {
            continue;
        }
        if runtime_profile_name_in_selection_backoff(
            &candidate.name,
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            route_kind,
            now,
        ) {
            continue;
        }
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
            continue;
        }
        if runtime_quota_precommit_guard_reason(
            runtime_quota_summary_for_route(&candidate.usage, route_kind),
            route_kind,
        )
        .is_some()
        {
            continue;
        }
        if runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight)
            >= inflight_soft_limit
        {
            waitable_profiles.insert(candidate.name.clone());
        }
    }

    Ok(waitable_profiles)
}

pub(super) fn runtime_any_waited_candidate_relieved(
    shared: &RuntimeRotationProxyShared,
    waited_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    if waited_profiles.is_empty() {
        return Ok(false);
    }
    let now = Local::now().timestamp();
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let inflight_soft_limit = runtime_profile_inflight_soft_limit(route_kind, pressure_mode);
    let (
        state,
        current_profile,
        include_code_review,
        cached_reports,
        cached_usage_snapshots,
        profile_usage_auth,
        retry_backoff_until,
        transport_backoff_until,
        route_circuit_open_until,
        profile_inflight,
        profile_health,
    ) = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        prune_runtime_profile_selection_backoff(&mut runtime, now);
        (
            runtime.state.clone(),
            runtime.current_profile.clone(),
            runtime.include_code_review,
            runtime.profile_probe_cache.clone(),
            runtime.profile_usage_snapshots.clone(),
            runtime.profile_usage_auth.clone(),
            runtime.profile_retry_backoff_until.clone(),
            runtime.profile_transport_backoff_until.clone(),
            runtime.profile_route_circuit_open_until.clone(),
            runtime.profile_inflight.clone(),
            runtime.profile_health.clone(),
        )
    };

    let mut reports = Vec::new();
    for (order_index, name) in active_profile_selection_order(&state, &current_profile)
        .into_iter()
        .enumerate()
    {
        if !waited_profiles.contains(&name) {
            continue;
        }
        let Some(entry) = cached_reports.get(&name) else {
            continue;
        };
        reports.push(RunProfileProbeReport {
            name,
            order_index,
            auth: entry.auth.clone(),
            result: entry.result.clone(),
        });
    }

    for candidate in ready_profile_candidates(
        &reports,
        include_code_review,
        Some(current_profile.as_str()),
        &state,
        Some(&cached_usage_snapshots),
    ) {
        if !waited_profiles.contains(&candidate.name) {
            continue;
        }
        if runtime_profile_name_in_selection_backoff(
            &candidate.name,
            &retry_backoff_until,
            &transport_backoff_until,
            &route_circuit_open_until,
            route_kind,
            now,
        ) {
            continue;
        }
        if runtime_profile_auth_failure_active_with_auth_cache(
            &profile_health,
            &profile_usage_auth,
            &candidate.name,
            now,
        ) {
            continue;
        }
        if runtime_quota_precommit_guard_reason(
            runtime_quota_summary_for_route(&candidate.usage, route_kind),
            route_kind,
        )
        .is_some()
        {
            continue;
        }
        if runtime_profile_inflight_sort_key(&candidate.name, &profile_inflight)
            < inflight_soft_limit
        {
            return Ok(true);
        }
    }

    Ok(false)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_proxy_maybe_wait_for_interactive_inflight_relief(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    excluded_profiles: &BTreeSet<String>,
    route_kind: RuntimeRouteKind,
    selection_started_at: Instant,
    continuation: bool,
    wait_affinity_owner: Option<&str>,
) -> Result<bool> {
    let pressure_mode = runtime_proxy_pressure_mode_active_for_route(shared, route_kind);
    let wait_budget = runtime_proxy_request_inflight_wait_budget(request, pressure_mode);
    if wait_budget.is_zero() {
        return Ok(false);
    }
    let waited_profiles = runtime_waitable_inflight_candidates_for_route(
        shared,
        excluded_profiles,
        route_kind,
        wait_affinity_owner,
    )?;
    if waited_profiles.is_empty() {
        return Ok(false);
    }

    let (_, precommit_budget) = runtime_proxy_precommit_budget(continuation, pressure_mode);
    let remaining_budget = precommit_budget.saturating_sub(selection_started_at.elapsed());
    let total_wait_budget = wait_budget.min(remaining_budget);
    if total_wait_budget.is_zero() {
        return Ok(false);
    }
    let wait_deadline = Instant::now() + total_wait_budget;

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http inflight_wait_started route={} wait_ms={}",
            runtime_route_kind_label(route_kind),
            total_wait_budget.as_millis()
        ),
    );
    let started_at = Instant::now();
    let mut observed_revision = runtime_profile_inflight_release_revision(shared);
    let mut signaled = false;
    let mut useful_relief = false;
    let mut wake_source = RuntimeProfileInFlightWaitOutcome::Timeout;
    loop {
        let remaining_wait = wait_deadline.saturating_duration_since(Instant::now());
        if remaining_wait.is_zero() {
            break;
        }
        match runtime_profile_inflight_wait_outcome_since(shared, remaining_wait, observed_revision)
        {
            RuntimeProfileInFlightWaitOutcome::InflightRelease => {
                signaled = true;
                wake_source = RuntimeProfileInFlightWaitOutcome::InflightRelease;
                observed_revision = runtime_profile_inflight_release_revision(shared);
                useful_relief =
                    runtime_any_waited_candidate_relieved(shared, &waited_profiles, route_kind)?;
                if useful_relief {
                    break;
                }
            }
            RuntimeProfileInFlightWaitOutcome::OtherNotify => {
                signaled = true;
                wake_source = RuntimeProfileInFlightWaitOutcome::OtherNotify;
                observed_revision = runtime_profile_inflight_release_revision(shared);
            }
            RuntimeProfileInFlightWaitOutcome::Timeout => {
                if !signaled {
                    wake_source = RuntimeProfileInFlightWaitOutcome::Timeout;
                }
                break;
            }
        }
    }
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http inflight_wait_finished route={} waited_ms={} signaled={signaled} useful={} wake_source={}",
            runtime_route_kind_label(route_kind),
            started_at.elapsed().as_millis(),
            useful_relief,
            runtime_profile_inflight_wait_outcome_label(wake_source),
        ),
    );
    Ok(useful_relief)
}

pub(super) fn runtime_profile_usage_cache_is_fresh(
    entry: &RuntimeProfileProbeCacheEntry,
    now: i64,
) -> bool {
    now.saturating_sub(entry.checked_at) <= RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS
}

pub(super) fn runtime_profile_probe_cache_freshness(
    entry: &RuntimeProfileProbeCacheEntry,
    now: i64,
) -> RuntimeProbeCacheFreshness {
    let age = now.saturating_sub(entry.checked_at);
    if age <= RUNTIME_PROFILE_USAGE_CACHE_FRESH_SECONDS {
        RuntimeProbeCacheFreshness::Fresh
    } else if age <= RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS {
        RuntimeProbeCacheFreshness::StaleUsable
    } else {
        RuntimeProbeCacheFreshness::Expired
    }
}

pub(super) fn update_runtime_profile_probe_cache_with_usage(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    usage: UsageResponse,
) -> Result<()> {
    let auth = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .state
        .profiles
        .get(profile_name)
        .map(|profile| read_auth_summary(&profile.codex_home))
        .unwrap_or(AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        });
    apply_runtime_profile_probe_result(shared, profile_name, auth, Ok(usage))
}

pub(super) fn runtime_proxy_current_profile(shared: &RuntimeRotationProxyShared) -> Result<String> {
    Ok(shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .current_profile
        .clone())
}

pub(super) fn runtime_profile_in_retry_backoff(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
) -> bool {
    runtime
        .profile_retry_backoff_until
        .get(profile_name)
        .copied()
        .is_some_and(|until| until > now)
}

pub(super) fn runtime_profile_in_transport_backoff(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    runtime_profile_transport_backoff_until_from_map(
        &runtime.profile_transport_backoff_until,
        profile_name,
        route_kind,
        now,
    )
    .is_some()
}

pub(super) fn runtime_profile_inflight_count(
    runtime: &RuntimeRotationState,
    profile_name: &str,
) -> usize {
    runtime
        .profile_inflight
        .get(profile_name)
        .copied()
        .unwrap_or(0)
}

pub(super) fn runtime_profile_inflight_hard_limit_context(context: &str) -> usize {
    runtime_profile_inflight_weight(context)
}

pub(super) fn runtime_profile_inflight_hard_limited_for_context(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &str,
) -> Result<bool> {
    let hard_limit = runtime_proxy_profile_inflight_hard_limit();
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    Ok(runtime_profile_inflight_count(&runtime, profile_name)
        .saturating_add(runtime_profile_inflight_hard_limit_context(context))
        > hard_limit)
}

pub(super) fn runtime_profile_in_selection_backoff(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    runtime_profile_in_retry_backoff(runtime, profile_name, now)
        || runtime_profile_in_transport_backoff(runtime, profile_name, route_kind, now)
}

pub(super) fn runtime_profile_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_global_health_score(runtime, profile_name, now)
        .saturating_add(runtime_profile_route_health_score(
            runtime,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_coupling_score(
            runtime,
            profile_name,
            now,
            route_kind,
        ))
}

pub(super) fn runtime_route_coupled_kinds(
    route_kind: RuntimeRouteKind,
) -> &'static [RuntimeRouteKind] {
    match route_kind {
        RuntimeRouteKind::Responses => &[RuntimeRouteKind::Websocket],
        RuntimeRouteKind::Websocket => &[RuntimeRouteKind::Responses],
        RuntimeRouteKind::Compact => &[RuntimeRouteKind::Standard],
        RuntimeRouteKind::Standard => &[RuntimeRouteKind::Compact],
    }
}

pub(super) fn runtime_profile_route_coupling_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_route_coupling_score_from_map(
        &runtime.profile_health,
        profile_name,
        now,
        route_kind,
    )
}

pub(super) fn runtime_profile_route_coupling_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            let route_score = runtime_profile_effective_health_score_from_map(
                profile_health,
                &runtime_profile_route_health_key(profile_name, coupled_kind),
                now,
            );
            let bad_pairing_score = runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_bad_pairing_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
            );
            route_score
                .saturating_add(bad_pairing_score)
                .saturating_div(2)
        })
        .fold(0, u32::saturating_add)
}

pub(super) fn runtime_profile_selection_jitter(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    shared
        .request_sequence
        .load(Ordering::Relaxed)
        .hash(&mut hasher);
    profile_name.hash(&mut hasher);
    runtime_route_kind_label(route_kind).hash(&mut hasher);
    hasher.finish()
}

pub(super) fn prune_runtime_profile_retry_backoff(runtime: &mut RuntimeRotationState, now: i64) {
    runtime
        .profile_retry_backoff_until
        .retain(|_, until| *until > now);
}

pub(super) fn prune_runtime_profile_transport_backoff(
    runtime: &mut RuntimeRotationState,
    now: i64,
) {
    runtime
        .profile_transport_backoff_until
        .retain(|_, until| *until > now);
}

pub(super) fn prune_runtime_profile_route_circuits(runtime: &mut RuntimeRotationState, now: i64) {
    runtime
        .profile_route_circuit_open_until
        .retain(|key, until| {
            if *until > now {
                return true;
            }
            let health_key = runtime_profile_route_circuit_health_key(key);
            runtime_profile_effective_health_score_from_map(
                &runtime.profile_health,
                &health_key,
                now,
            ) > 0
        });
}

pub(super) fn prune_runtime_profile_selection_backoff(
    runtime: &mut RuntimeRotationState,
    now: i64,
) {
    prune_runtime_profile_retry_backoff(runtime, now);
    prune_runtime_profile_transport_backoff(runtime, now);
    prune_runtime_profile_route_circuits(runtime, now);
}

pub(super) fn runtime_profile_name_in_selection_backoff(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    retry_backoff_until
        .get(profile_name)
        .copied()
        .is_some_and(|until| until > now)
        || runtime_profile_transport_backoff_until_from_map(
            transport_backoff_until,
            profile_name,
            route_kind,
            now,
        )
        .is_some()
        || route_circuit_open_until
            .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
            .copied()
            .is_some_and(|until| until > now)
}

pub(super) fn runtime_profile_backoff_sort_key(
    profile_name: &str,
    retry_backoff_until: &BTreeMap<String, i64>,
    transport_backoff_until: &BTreeMap<String, i64>,
    route_circuit_open_until: &BTreeMap<String, i64>,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> (usize, i64, i64, i64) {
    let retry_until = retry_backoff_until
        .get(profile_name)
        .copied()
        .filter(|until| *until > now);
    let transport_until = runtime_profile_transport_backoff_until_from_map(
        transport_backoff_until,
        profile_name,
        route_kind,
        now,
    );
    let circuit_until = route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .copied()
        .filter(|until| *until > now);

    match (circuit_until, transport_until, retry_until) {
        (None, None, None) => (0, 0, 0, 0),
        (Some(circuit_until), None, None) => (1, circuit_until, 0, 0),
        (None, Some(transport_until), None) => (2, transport_until, 0, 0),
        (None, None, Some(retry_until)) => (3, retry_until, 0, 0),
        (Some(circuit_until), Some(transport_until), None) => (
            4,
            circuit_until.min(transport_until),
            circuit_until.max(transport_until),
            0,
        ),
        (Some(circuit_until), None, Some(retry_until)) => (
            5,
            circuit_until.min(retry_until),
            circuit_until.max(retry_until),
            0,
        ),
        (None, Some(transport_until), Some(retry_until)) => (
            6,
            transport_until.min(retry_until),
            transport_until.max(retry_until),
            0,
        ),
        (Some(circuit_until), Some(transport_until), Some(retry_until)) => (
            7,
            circuit_until.min(transport_until.min(retry_until)),
            circuit_until.max(transport_until.max(retry_until)),
            retry_until,
        ),
    }
}

pub(super) fn runtime_profile_health_sort_key(
    profile_name: &str,
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_effective_health_score_from_map(profile_health, profile_name, now)
        .saturating_add(runtime_profile_effective_health_score_from_map(
            profile_health,
            &runtime_profile_route_health_key(profile_name, route_kind),
            now,
        ))
        .saturating_add(runtime_profile_effective_score_from_map(
            profile_health,
            &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
            now,
            RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
        ))
        .saturating_add(runtime_profile_route_coupling_score_from_map(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
        .saturating_add(runtime_profile_route_performance_score(
            profile_health,
            profile_name,
            now,
            route_kind,
        ))
}

pub(super) fn runtime_profile_inflight_sort_key(
    profile_name: &str,
    profile_inflight: &BTreeMap<String, usize>,
) -> usize {
    profile_inflight.get(profile_name).copied().unwrap_or(0)
}

pub(super) fn runtime_profile_inflight_weight(context: &str) -> usize {
    match context {
        "websocket_session" | "responses_http" => 2,
        _ => 1,
    }
}

pub(super) fn runtime_route_kind_inflight_context(route_kind: RuntimeRouteKind) -> &'static str {
    match route_kind {
        RuntimeRouteKind::Responses => "responses_http",
        RuntimeRouteKind::Compact => "compact_http",
        RuntimeRouteKind::Websocket => "websocket_session",
        RuntimeRouteKind::Standard => "standard_http",
    }
}

pub(super) fn runtime_profile_inflight_soft_limit(
    route_kind: RuntimeRouteKind,
    pressure_mode: bool,
) -> usize {
    let base = runtime_proxy_profile_inflight_soft_limit().max(1);
    if !pressure_mode {
        return base;
    }
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => base.saturating_sub(1).max(1),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => base.saturating_sub(2).max(1),
    }
}

pub(super) fn runtime_quota_pressure_band_reason(band: RuntimeQuotaPressureBand) -> &'static str {
    match band {
        RuntimeQuotaPressureBand::Healthy => "quota_healthy",
        RuntimeQuotaPressureBand::Thin => "quota_thin",
        RuntimeQuotaPressureBand::Critical => "quota_critical",
        RuntimeQuotaPressureBand::Exhausted => "quota_exhausted",
        RuntimeQuotaPressureBand::Unknown => "quota_unknown",
    }
}

pub(super) fn runtime_quota_window_status_reason(status: RuntimeQuotaWindowStatus) -> &'static str {
    match status {
        RuntimeQuotaWindowStatus::Ready => "ready",
        RuntimeQuotaWindowStatus::Thin => "thin",
        RuntimeQuotaWindowStatus::Critical => "critical",
        RuntimeQuotaWindowStatus::Exhausted => "exhausted",
        RuntimeQuotaWindowStatus::Unknown => "unknown",
    }
}

pub(super) fn runtime_quota_window_summary(
    usage: &UsageResponse,
    label: &str,
) -> RuntimeQuotaWindowSummary {
    let Some(window) = required_main_window_snapshot(usage, label) else {
        return RuntimeQuotaWindowSummary {
            status: RuntimeQuotaWindowStatus::Unknown,
            remaining_percent: 0,
            reset_at: i64::MAX,
        };
    };
    let status = if window.remaining_percent == 0 {
        RuntimeQuotaWindowStatus::Exhausted
    } else if window.remaining_percent <= 5 {
        RuntimeQuotaWindowStatus::Critical
    } else if window.remaining_percent <= 15 {
        RuntimeQuotaWindowStatus::Thin
    } else {
        RuntimeQuotaWindowStatus::Ready
    };
    RuntimeQuotaWindowSummary {
        status,
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

pub(super) fn runtime_quota_summary_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    RuntimeQuotaSummary {
        five_hour: runtime_quota_window_summary(usage, "5h"),
        weekly: runtime_quota_window_summary(usage, "weekly"),
        route_band: runtime_quota_pressure_band_for_route(usage, route_kind),
    }
}

pub(super) fn runtime_quota_summary_blocking_reset_at(
    summary: RuntimeQuotaSummary,
    route_kind: RuntimeRouteKind,
) -> Option<i64> {
    let floor_percent = runtime_quota_precommit_floor_percent(route_kind);
    [summary.five_hour, summary.weekly]
        .into_iter()
        .filter(|window| runtime_quota_window_precommit_guard(*window, floor_percent))
        .map(|window| window.reset_at)
        .filter(|reset_at| *reset_at != i64::MAX)
        .max()
}

pub(super) fn runtime_profile_usage_snapshot_from_usage(
    usage: &UsageResponse,
) -> RuntimeProfileUsageSnapshot {
    let five_hour = runtime_quota_window_summary(usage, "5h");
    let weekly = runtime_quota_window_summary(usage, "weekly");
    RuntimeProfileUsageSnapshot {
        checked_at: Local::now().timestamp(),
        five_hour_status: five_hour.status,
        five_hour_remaining_percent: five_hour.remaining_percent,
        five_hour_reset_at: five_hour.reset_at,
        weekly_status: weekly.status,
        weekly_remaining_percent: weekly.remaining_percent,
        weekly_reset_at: weekly.reset_at,
    }
}

pub(super) fn runtime_quota_summary_from_usage_snapshot(
    snapshot: &RuntimeProfileUsageSnapshot,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaSummary {
    let five_hour = RuntimeQuotaWindowSummary {
        status: snapshot.five_hour_status,
        remaining_percent: snapshot.five_hour_remaining_percent,
        reset_at: snapshot.five_hour_reset_at,
    };
    let weekly = RuntimeQuotaWindowSummary {
        status: snapshot.weekly_status,
        remaining_percent: snapshot.weekly_remaining_percent,
        reset_at: snapshot.weekly_reset_at,
    };
    let route_band = [
        five_hour.status,
        weekly.status,
        match route_kind {
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => weekly.status,
            RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => five_hour.status,
        },
    ]
    .into_iter()
    .fold(RuntimeQuotaPressureBand::Healthy, |band, status| {
        band.max(match status {
            RuntimeQuotaWindowStatus::Ready => RuntimeQuotaPressureBand::Healthy,
            RuntimeQuotaWindowStatus::Thin => RuntimeQuotaPressureBand::Thin,
            RuntimeQuotaWindowStatus::Critical => RuntimeQuotaPressureBand::Critical,
            RuntimeQuotaWindowStatus::Exhausted => RuntimeQuotaPressureBand::Exhausted,
            RuntimeQuotaWindowStatus::Unknown => RuntimeQuotaPressureBand::Unknown,
        })
    });
    RuntimeQuotaSummary {
        five_hour,
        weekly,
        route_band,
    }
}

pub(super) fn runtime_profile_usage_snapshot_hold_active(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at > now
    })
}

pub(super) fn runtime_profile_usage_snapshot_hold_expired(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at <= now
    })
}

pub(super) fn runtime_proxy_quota_reset_at_from_message(message: &str) -> Option<i64> {
    let marker = message.to_ascii_lowercase().find("try again at ")?;
    let candidate = message
        .get(marker + "try again at ".len()..)?
        .trim()
        .trim_end_matches('.');
    let now = Local::now();
    if let Some((time_text, meridiem)) = candidate
        .split_whitespace()
        .collect::<Vec<_>>()
        .get(..2)
        .and_then(|parts| {
            if parts.len() == 2 {
                Some((parts[0], parts[1]))
            } else {
                None
            }
        })
        && let Ok(time) =
            chrono::NaiveTime::parse_from_str(&format!("{time_text} {meridiem}"), "%I:%M %p")
    {
        let mut naive = now.date_naive().and_time(time);
        let mut parsed = Local
            .from_local_datetime(&naive)
            .single()
            .or_else(|| Local.from_local_datetime(&naive).earliest())?;
        if parsed.timestamp() <= now.timestamp() {
            naive = naive.checked_add_signed(chrono::Duration::days(1))?;
            parsed = Local
                .from_local_datetime(&naive)
                .single()
                .or_else(|| Local.from_local_datetime(&naive).earliest())?;
        }
        return Some(parsed.timestamp());
    }
    let mut parts = candidate
        .split_whitespace()
        .map(|part| part.to_string())
        .collect::<Vec<_>>();
    if parts.len() < 5 {
        return None;
    }
    let day_digits = parts[1]
        .trim_end_matches(',')
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if day_digits.is_empty() {
        return None;
    }
    parts[1] = format!("{day_digits},");
    let normalized = parts[..5].join(" ");
    let naive = chrono::NaiveDateTime::parse_from_str(&normalized, "%b %d, %Y %I:%M %p").ok()?;
    Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .map(|datetime| datetime.timestamp())
}

pub(super) fn runtime_profile_known_quota_reset_at(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Option<i64> {
    let now = Local::now().timestamp();
    runtime
        .profile_probe_cache
        .get(profile_name)
        .and_then(|entry| entry.result.as_ref().ok())
        .map(|usage| runtime_quota_summary_for_route(usage, route_kind))
        .and_then(|summary| runtime_quota_summary_blocking_reset_at(summary, route_kind))
        .filter(|reset_at| *reset_at > now)
        .or_else(|| {
            runtime
                .profile_usage_snapshots
                .get(profile_name)
                .and_then(|snapshot| {
                    runtime_quota_summary_blocking_reset_at(
                        runtime_quota_summary_from_usage_snapshot(snapshot, route_kind),
                        route_kind,
                    )
                })
                .filter(|reset_at| *reset_at > now)
        })
}

pub(super) fn mark_runtime_profile_quota_quarantine(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    quota_message: Option<&str>,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    let resolved_reset_at = quota_message
        .and_then(runtime_proxy_quota_reset_at_from_message)
        .or_else(|| runtime_profile_known_quota_reset_at(&runtime, profile_name, route_kind))
        .filter(|reset_at| *reset_at > now);
    let until = resolved_reset_at
        .unwrap_or_else(|| now.saturating_add(RUNTIME_PROFILE_QUOTA_QUARANTINE_FALLBACK_SECONDS));
    runtime.profile_probe_cache.remove(profile_name);
    let snapshot = runtime
        .profile_usage_snapshots
        .entry(profile_name.to_string())
        .or_insert(RuntimeProfileUsageSnapshot {
            checked_at: now,
            five_hour_status: RuntimeQuotaWindowStatus::Unknown,
            five_hour_remaining_percent: 0,
            five_hour_reset_at: i64::MAX,
            weekly_status: RuntimeQuotaWindowStatus::Unknown,
            weekly_remaining_percent: 0,
            weekly_reset_at: i64::MAX,
        });
    snapshot.checked_at = now;
    snapshot.five_hour_status = RuntimeQuotaWindowStatus::Exhausted;
    snapshot.five_hour_remaining_percent = 0;
    snapshot.five_hour_reset_at = if snapshot.five_hour_reset_at == i64::MAX {
        until
    } else {
        snapshot.five_hour_reset_at.max(until)
    };
    snapshot.weekly_status = RuntimeQuotaWindowStatus::Exhausted;
    snapshot.weekly_remaining_percent = 0;
    snapshot.weekly_reset_at = if snapshot.weekly_reset_at == i64::MAX {
        until
    } else {
        snapshot.weekly_reset_at.max(until)
    };
    runtime
        .profile_retry_backoff_until
        .entry(profile_name.to_string())
        .and_modify(|current| *current = (*current).max(until))
        .or_insert(until);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("profile_retry_backoff:{profile_name}"),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_quota_quarantine profile={profile_name} route={} until={} reset_at={} message={}",
            runtime_route_kind_label(route_kind),
            until,
            resolved_reset_at.unwrap_or(i64::MAX),
            quota_message.unwrap_or("-"),
        ),
    );
    runtime_proxy_log(
        shared,
        format!("profile_retry_backoff profile={profile_name} until={until}"),
    );
    Ok(())
}

pub(super) fn usage_from_runtime_usage_snapshot(
    snapshot: &RuntimeProfileUsageSnapshot,
) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.five_hour_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.five_hour_reset_at != i64::MAX)
                    .then_some(snapshot.five_hour_reset_at),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.weekly_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.weekly_reset_at != i64::MAX)
                    .then_some(snapshot.weekly_reset_at),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

pub(super) fn runtime_profile_backoffs_snapshot(
    runtime: &RuntimeRotationState,
) -> RuntimeProfileBackoffs {
    RuntimeProfileBackoffs {
        retry_backoff_until: runtime.profile_retry_backoff_until.clone(),
        transport_backoff_until: runtime.profile_transport_backoff_until.clone(),
        route_circuit_open_until: runtime.profile_route_circuit_open_until.clone(),
    }
}

pub(super) fn runtime_continuation_store_snapshot(
    runtime: &RuntimeRotationState,
) -> RuntimeContinuationStore {
    RuntimeContinuationStore {
        response_profile_bindings: runtime.state.response_profile_bindings.clone(),
        session_profile_bindings: runtime.state.session_profile_bindings.clone(),
        turn_state_bindings: runtime.turn_state_bindings.clone(),
        session_id_bindings: runtime.session_id_bindings.clone(),
        statuses: runtime.continuation_statuses.clone(),
    }
}

pub(super) fn runtime_soften_persisted_backoff_map_for_startup(
    backoffs: &mut BTreeMap<String, i64>,
    now: i64,
    max_future_seconds: i64,
) -> bool {
    let max_until = now.saturating_add(max_future_seconds.max(0));
    let mut changed = false;
    backoffs.retain(|_, until| {
        if *until <= now {
            changed = true;
            return false;
        }
        let next_until = (*until).min(max_until);
        if next_until != *until {
            changed = true;
        }
        *until = next_until;
        true
    });
    changed
}

pub(super) fn runtime_route_kind_from_label(label: &str) -> Option<RuntimeRouteKind> {
    match label {
        "responses" => Some(RuntimeRouteKind::Responses),
        "compact" => Some(RuntimeRouteKind::Compact),
        "websocket" => Some(RuntimeRouteKind::Websocket),
        "standard" => Some(RuntimeRouteKind::Standard),
        _ => None,
    }
}

pub(super) fn runtime_profile_route_circuit_probe_seconds(
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    route_profile_key: &str,
    now: i64,
) -> i64 {
    let Some((route_label, profile_name)) =
        runtime_profile_route_key_parts(route_profile_key, "__route_circuit__:")
    else {
        return RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let Some(route_kind) = runtime_route_kind_from_label(route_label) else {
        return RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS;
    };
    let score = runtime_profile_effective_health_score_from_map(
        profile_scores,
        &runtime_profile_route_health_key(profile_name, route_kind),
        now,
    );
    runtime_profile_circuit_half_open_probe_seconds(score)
}

pub(super) fn runtime_soften_persisted_route_circuits_for_startup(
    route_circuit_open_until: &mut BTreeMap<String, i64>,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> bool {
    let mut changed = false;
    route_circuit_open_until.retain(|route_profile_key, until| {
        if *until <= now {
            changed = true;
            return false;
        }
        let max_until = now.saturating_add(runtime_profile_route_circuit_probe_seconds(
            profile_scores,
            route_profile_key,
            now,
        ));
        let next_until = (*until).min(max_until);
        if next_until != *until {
            changed = true;
        }
        *until = next_until;
        true
    });
    changed
}

pub(super) fn runtime_soften_persisted_backoffs_for_startup(
    backoffs: &mut RuntimeProfileBackoffs,
    profile_scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> bool {
    let mut changed = runtime_soften_persisted_backoff_map_for_startup(
        &mut backoffs.transport_backoff_until,
        now,
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
    );
    changed = runtime_soften_persisted_route_circuits_for_startup(
        &mut backoffs.route_circuit_open_until,
        profile_scores,
        now,
    ) || changed;
    changed
}

pub(super) const RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD: u32 = 4;
pub(super) const RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS: i64 = 20;
pub(super) const RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS: i64 = if cfg!(test) { 320 } else { 600 };
pub(super) const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS: i64 = 5;
pub(super) const RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS: i64 =
    if cfg!(test) { 20 } else { 60 };
pub(super) const RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS: i64 =
    if cfg!(test) { 12 } else { 1_800 };
pub(super) const RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE: u32 = 4;

pub(super) fn runtime_profile_circuit_open_seconds(score: u32, reopen_stage: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3)
                .saturating_add(reopen_stage.min(RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE)),
        )
        .unwrap_or(i64::MAX);
    RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS
        .saturating_mul(multiplier)
        .min(RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS)
}

pub(super) fn runtime_profile_circuit_half_open_probe_seconds(score: u32) -> i64 {
    let multiplier = 1_i64
        .checked_shl(
            score
                .saturating_sub(RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD)
                .min(3),
        )
        .unwrap_or(i64::MAX);
    RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_SECONDS
        .saturating_mul(multiplier)
        .min(RUNTIME_PROFILE_CIRCUIT_HALF_OPEN_PROBE_MAX_SECONDS)
}

pub(super) fn runtime_profile_route_circuit_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_circuit_profile_name(key: &str) -> &str {
    key.rsplit(':').next().unwrap_or(key)
}

pub(super) fn runtime_profile_route_circuit_health_key(key: &str) -> String {
    key.replacen("__route_circuit__", "__route_health__", 1)
}

pub(super) fn runtime_profile_route_circuit_reopen_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_circuit_reopen__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_circuit_open_until(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Option<i64> {
    runtime
        .profile_route_circuit_open_until
        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .copied()
        .filter(|until| *until > now)
}

pub(super) fn reserve_runtime_profile_route_circuit_half_open_probe(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_circuit_key(profile_name, route_kind);
    let route_label = runtime_route_kind_label(route_kind);
    let Some(until) = runtime.profile_route_circuit_open_until.get(&key).copied() else {
        return Ok(true);
    };
    if until > now {
        return Ok(false);
    }
    let health_key = runtime_profile_route_circuit_health_key(&key);
    let reopen_key = runtime_profile_route_circuit_reopen_key(profile_name, route_kind);
    let health_score =
        runtime_profile_effective_health_score_from_map(&runtime.profile_health, &health_key, now);
    if health_score == 0 {
        runtime.profile_route_circuit_open_until.remove(&key);
        runtime.profile_health.remove(&reopen_key);
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("profile_circuit_clear:{profile_name}:{route_label}"),
        );
        return Ok(true);
    }

    let probe_seconds = runtime_profile_circuit_half_open_probe_seconds(health_score);
    let reserve_until = now.saturating_add(probe_seconds);
    runtime
        .profile_route_circuit_open_until
        .insert(key, reserve_until);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("profile_circuit_half_open_probe:{profile_name}:{route_label}"),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_circuit_half_open_probe profile={profile_name} route={} until={reserve_until} health={health_score} probe_seconds={probe_seconds}",
            route_label
        ),
    );
    Ok(true)
}

pub(super) fn clear_runtime_profile_circuit_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> bool {
    runtime
        .profile_route_circuit_open_until
        .remove(&runtime_profile_route_circuit_key(profile_name, route_kind))
        .is_some()
}

pub(super) fn runtime_quota_source_label(source: RuntimeQuotaSource) -> &'static str {
    match source {
        RuntimeQuotaSource::LiveProbe => "probe_cache",
        RuntimeQuotaSource::PersistedSnapshot => "persisted_snapshot",
    }
}

pub(super) fn runtime_usage_snapshot_is_usable(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    if runtime_profile_usage_snapshot_hold_active(snapshot, now) {
        return true;
    }
    if runtime_profile_usage_snapshot_hold_expired(snapshot, now) {
        return false;
    }
    now.saturating_sub(snapshot.checked_at) <= RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS
}

pub(super) fn runtime_quota_summary_log_fields(summary: RuntimeQuotaSummary) -> String {
    format!(
        "quota_band={} five_hour_status={} five_hour_remaining={} five_hour_reset_at={} weekly_status={} weekly_remaining={} weekly_reset_at={}",
        runtime_quota_pressure_band_reason(summary.route_band),
        runtime_quota_window_status_reason(summary.five_hour.status),
        summary.five_hour.remaining_percent,
        summary.five_hour.reset_at,
        runtime_quota_window_status_reason(summary.weekly.status),
        summary.weekly.remaining_percent,
        summary.weekly.reset_at,
    )
}

pub(super) type RuntimeQuotaPressureSortKey = (
    RuntimeQuotaPressureBand,
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
);

pub(super) fn runtime_quota_pressure_sort_key_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureSortKey {
    let score = ready_profile_score_for_route(usage, route_kind);
    (
        runtime_quota_pressure_band_for_route(usage, route_kind),
        score.total_pressure,
        score.weekly_pressure,
        score.five_hour_pressure,
        Reverse(score.reserve_floor),
        Reverse(score.weekly_remaining),
        Reverse(score.five_hour_remaining),
        score.weekly_reset_at,
        score.five_hour_reset_at,
    )
}

pub(super) fn runtime_quota_pressure_sort_key_for_route_from_summary(
    summary: RuntimeQuotaSummary,
) -> RuntimeQuotaPressureSortKey {
    (
        summary.route_band,
        match summary.route_band {
            RuntimeQuotaPressureBand::Healthy => 0,
            RuntimeQuotaPressureBand::Thin => 1,
            RuntimeQuotaPressureBand::Critical => 2,
            RuntimeQuotaPressureBand::Exhausted => 3,
            RuntimeQuotaPressureBand::Unknown => 4,
        },
        match summary.weekly.status {
            RuntimeQuotaWindowStatus::Ready => 0,
            RuntimeQuotaWindowStatus::Thin => 1,
            RuntimeQuotaWindowStatus::Critical => 2,
            RuntimeQuotaWindowStatus::Exhausted => 3,
            RuntimeQuotaWindowStatus::Unknown => 4,
        },
        match summary.five_hour.status {
            RuntimeQuotaWindowStatus::Ready => 0,
            RuntimeQuotaWindowStatus::Thin => 1,
            RuntimeQuotaWindowStatus::Critical => 2,
            RuntimeQuotaWindowStatus::Exhausted => 3,
            RuntimeQuotaWindowStatus::Unknown => 4,
        },
        Reverse(
            summary
                .weekly
                .remaining_percent
                .min(summary.five_hour.remaining_percent),
        ),
        Reverse(summary.weekly.remaining_percent),
        Reverse(summary.five_hour.remaining_percent),
        summary.weekly.reset_at,
        summary.five_hour.reset_at,
    )
}

pub(super) fn runtime_quota_pressure_band_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureBand {
    let Some(weekly) = required_main_window_snapshot(usage, "weekly") else {
        return RuntimeQuotaPressureBand::Unknown;
    };
    let Some(five_hour) = required_main_window_snapshot(usage, "5h") else {
        return RuntimeQuotaPressureBand::Unknown;
    };

    let weekly_remaining = weekly.remaining_percent;
    let five_hour_remaining = five_hour.remaining_percent;
    if weekly_remaining == 0 || five_hour_remaining == 0 {
        return RuntimeQuotaPressureBand::Exhausted;
    }

    let (thin_weekly, thin_five_hour, critical_weekly, critical_five_hour) = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => (20, 10, 10, 5),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => (10, 5, 5, 3),
    };

    if weekly_remaining <= critical_weekly || five_hour_remaining <= critical_five_hour {
        RuntimeQuotaPressureBand::Critical
    } else if weekly_remaining <= thin_weekly || five_hour_remaining <= thin_five_hour {
        RuntimeQuotaPressureBand::Thin
    } else {
        RuntimeQuotaPressureBand::Healthy
    }
}

pub(super) fn runtime_profile_effective_health_score(
    entry: &RuntimeProfileHealth,
    now: i64,
) -> u32 {
    runtime_profile_effective_score(entry, now, RUNTIME_PROFILE_HEALTH_DECAY_SECONDS)
}

pub(super) fn runtime_profile_effective_score(
    entry: &RuntimeProfileHealth,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    let decay = now
        .saturating_sub(entry.updated_at)
        .saturating_div(decay_seconds.max(1))
        .clamp(0, i64::from(u32::MAX)) as u32;
    entry.score.saturating_sub(decay)
}

pub(super) fn runtime_profile_effective_health_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
}

pub(super) fn runtime_profile_effective_score_from_map(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    profile_health
        .get(key)
        .map(|entry| runtime_profile_effective_score(entry, now, decay_seconds))
        .unwrap_or(0)
}

pub(super) fn runtime_profile_global_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
) -> u32 {
    runtime_profile_effective_health_score_from_map(&runtime.profile_health, profile_name, now)
}

pub(super) fn runtime_profile_route_health_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_health__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_bad_pairing__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_success_streak_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_success__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    format!(
        "__route_performance__:{}:{profile_name}",
        runtime_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_profile_route_health_score(
    runtime: &RuntimeRotationState,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    runtime_profile_effective_health_score_from_map(
        &runtime.profile_health,
        &runtime_profile_route_health_key(profile_name, route_kind),
        now,
    )
}

pub(super) fn runtime_profile_route_performance_score(
    profile_health: &BTreeMap<String, RuntimeProfileHealth>,
    profile_name: &str,
    now: i64,
    route_kind: RuntimeRouteKind,
) -> u32 {
    let route_score = runtime_profile_effective_score_from_map(
        profile_health,
        &runtime_profile_route_performance_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
    );
    let coupled_score = runtime_route_coupled_kinds(route_kind)
        .iter()
        .copied()
        .map(|coupled_kind| {
            runtime_profile_effective_score_from_map(
                profile_health,
                &runtime_profile_route_performance_key(profile_name, coupled_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
            .saturating_div(2)
        })
        .fold(0, u32::saturating_add);
    route_score.saturating_add(coupled_score)
}

pub(super) fn runtime_profile_latency_penalty(
    elapsed_ms: u64,
    route_kind: RuntimeRouteKind,
    stage: &str,
) -> u32 {
    let (good_ms, warn_ms, poor_ms, severe_ms) = match (route_kind, stage) {
        (RuntimeRouteKind::Responses, "ttfb") | (RuntimeRouteKind::Websocket, "connect") => {
            (120, 300, 700, 1_500)
        }
        (RuntimeRouteKind::Compact, _) | (RuntimeRouteKind::Standard, _) => (80, 180, 400, 900),
        _ => (100, 250, 600, 1_200),
    };
    match elapsed_ms {
        elapsed if elapsed <= good_ms => 0,
        elapsed if elapsed <= warn_ms => 2,
        elapsed if elapsed <= poor_ms => 4,
        elapsed if elapsed <= severe_ms => 7,
        _ => RUNTIME_PROFILE_LATENCY_PENALTY_MAX,
    }
}

pub(super) fn update_runtime_profile_route_performance(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    next_score: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_performance_key(profile_name, route_kind);
    if next_score == 0 {
        runtime.profile_health.remove(&key);
    } else {
        runtime.profile_health.insert(
            key,
            RuntimeProfileHealth {
                score: next_score.min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX),
                updated_at: now,
            },
        );
    }
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_latency profile={profile_name} route={} score={} reason={reason}",
            runtime_route_kind_label(route_kind),
            next_score.min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX),
        ),
    );
    Ok(())
}

pub(super) fn note_runtime_profile_latency_observation(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    stage: &str,
    elapsed_ms: u64,
) {
    let current_score = shared
        .runtime
        .lock()
        .ok()
        .map(|runtime| {
            let now = Local::now().timestamp();
            runtime_profile_effective_score_from_map(
                &runtime.profile_health,
                &runtime_profile_route_performance_key(profile_name, route_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
        })
        .unwrap_or(0);
    let observed = runtime_profile_latency_penalty(elapsed_ms, route_kind, stage);
    let next_score = if observed == 0 {
        current_score.saturating_sub(2)
    } else {
        (((current_score as u64) * 2) + (observed as u64)).div_ceil(3) as u32
    };
    let _ = update_runtime_profile_route_performance(
        shared,
        profile_name,
        route_kind,
        next_score,
        &format!("{stage}_{elapsed_ms}ms"),
    );
}

pub(super) fn note_runtime_profile_latency_failure(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    stage: &str,
) {
    let current_score = shared
        .runtime
        .lock()
        .ok()
        .map(|runtime| {
            let now = Local::now().timestamp();
            runtime_profile_effective_score_from_map(
                &runtime.profile_health,
                &runtime_profile_route_performance_key(profile_name, route_kind),
                now,
                RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
            )
        })
        .unwrap_or(0);
    let next_score = current_score
        .saturating_add(RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY)
        .min(RUNTIME_PROFILE_LATENCY_PENALTY_MAX);
    let _ = update_runtime_profile_route_performance(
        shared,
        profile_name,
        route_kind,
        next_score,
        stage,
    );
}

pub(super) fn runtime_route_kind_label(route_kind: RuntimeRouteKind) -> &'static str {
    match route_kind {
        RuntimeRouteKind::Responses => "responses",
        RuntimeRouteKind::Compact => "compact",
        RuntimeRouteKind::Websocket => "websocket",
        RuntimeRouteKind::Standard => "standard",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeTransportFailureKind {
    Dns,
    ConnectTimeout,
    ConnectRefused,
    ConnectReset,
    TlsHandshake,
    ConnectionAborted,
    BrokenPipe,
    UnexpectedEof,
    ReadTimeout,
    UpstreamClosedBeforeCommit,
    Other,
}

pub(super) fn runtime_transport_failure_kind_label(
    kind: RuntimeTransportFailureKind,
) -> &'static str {
    match kind {
        RuntimeTransportFailureKind::Dns => "dns",
        RuntimeTransportFailureKind::ConnectTimeout => "connect_timeout",
        RuntimeTransportFailureKind::ConnectRefused => "connection_refused",
        RuntimeTransportFailureKind::ConnectReset => "connection_reset",
        RuntimeTransportFailureKind::TlsHandshake => "tls_handshake",
        RuntimeTransportFailureKind::ConnectionAborted => "connection_aborted",
        RuntimeTransportFailureKind::BrokenPipe => "broken_pipe",
        RuntimeTransportFailureKind::UnexpectedEof => "unexpected_eof",
        RuntimeTransportFailureKind::ReadTimeout => "read_timeout",
        RuntimeTransportFailureKind::UpstreamClosedBeforeCommit => "upstream_closed_before_commit",
        RuntimeTransportFailureKind::Other => "other",
    }
}

pub(super) fn runtime_upstream_connect_failure_marker(
    failure_kind: Option<RuntimeTransportFailureKind>,
) -> &'static str {
    match failure_kind {
        Some(RuntimeTransportFailureKind::ConnectTimeout)
        | Some(RuntimeTransportFailureKind::ReadTimeout) => "upstream_connect_timeout",
        Some(RuntimeTransportFailureKind::Dns) => "upstream_connect_dns_error",
        Some(RuntimeTransportFailureKind::TlsHandshake) => "upstream_tls_handshake_error",
        _ => "upstream_connect_error",
    }
}

pub(super) fn log_runtime_upstream_connect_failure(
    shared: &RuntimeRotationProxyShared,
    request_id: u64,
    transport: &str,
    profile_name: &str,
    failure_kind: Option<RuntimeTransportFailureKind>,
    error: &impl std::fmt::Display,
) {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport={transport} {} profile={profile_name} class={} error={error}",
            runtime_upstream_connect_failure_marker(failure_kind),
            failure_kind
                .map(runtime_transport_failure_kind_label)
                .unwrap_or("unknown"),
        ),
    );
}

pub(super) fn runtime_transport_failure_kind_from_message(
    message: &str,
) -> Option<RuntimeTransportFailureKind> {
    let message = message.to_ascii_lowercase();
    if message.contains("dns")
        || message.contains("failed to lookup address information")
        || message.contains("no such host")
        || message.contains("name or service not known")
    {
        Some(RuntimeTransportFailureKind::Dns)
    } else if message.contains("tls")
        || message.contains("handshake")
        || message.contains("certificate")
    {
        Some(RuntimeTransportFailureKind::TlsHandshake)
    } else if message.contains("connection refused") {
        Some(RuntimeTransportFailureKind::ConnectRefused)
    } else if message.contains("timed out") || message.contains("timeout") {
        Some(RuntimeTransportFailureKind::ConnectTimeout)
    } else if message.contains("connection reset") {
        Some(RuntimeTransportFailureKind::ConnectReset)
    } else if message.contains("broken pipe") {
        Some(RuntimeTransportFailureKind::BrokenPipe)
    } else if message.contains("unexpected eof") {
        Some(RuntimeTransportFailureKind::UnexpectedEof)
    } else if message.contains("connection aborted") {
        Some(RuntimeTransportFailureKind::ConnectionAborted)
    } else if message.contains("stream closed before response.completed")
        || message.contains("closed before response.completed")
    {
        Some(RuntimeTransportFailureKind::UpstreamClosedBeforeCommit)
    } else if message.contains("unable to connect") {
        Some(RuntimeTransportFailureKind::Other)
    } else {
        None
    }
}

pub(super) fn runtime_transport_failure_kind_from_io_error(
    err: &io::Error,
) -> Option<RuntimeTransportFailureKind> {
    match err.kind() {
        io::ErrorKind::TimedOut => Some(RuntimeTransportFailureKind::ConnectTimeout),
        io::ErrorKind::ConnectionRefused => Some(RuntimeTransportFailureKind::ConnectRefused),
        io::ErrorKind::ConnectionReset => Some(RuntimeTransportFailureKind::ConnectReset),
        io::ErrorKind::ConnectionAborted => Some(RuntimeTransportFailureKind::ConnectionAborted),
        io::ErrorKind::BrokenPipe => Some(RuntimeTransportFailureKind::BrokenPipe),
        io::ErrorKind::UnexpectedEof => Some(RuntimeTransportFailureKind::UnexpectedEof),
        _ => runtime_transport_failure_kind_from_message(&err.to_string()),
    }
}

pub(super) fn runtime_transport_failure_kind_from_reqwest(
    err: &reqwest::Error,
) -> Option<RuntimeTransportFailureKind> {
    if err.is_timeout() {
        return Some(RuntimeTransportFailureKind::ReadTimeout);
    }
    std::error::Error::source(err)
        .and_then(|source| source.downcast_ref::<io::Error>())
        .and_then(runtime_transport_failure_kind_from_io_error)
        .or_else(|| runtime_transport_failure_kind_from_message(&err.to_string()))
}

pub(super) fn runtime_transport_failure_kind_from_ws(
    err: &WsError,
) -> Option<RuntimeTransportFailureKind> {
    match err {
        WsError::Io(io) => runtime_transport_failure_kind_from_io_error(io),
        WsError::Tls(_) => Some(RuntimeTransportFailureKind::TlsHandshake),
        WsError::ConnectionClosed | WsError::AlreadyClosed => {
            Some(RuntimeTransportFailureKind::UpstreamClosedBeforeCommit)
        }
        _ => runtime_transport_failure_kind_from_message(&err.to_string()),
    }
}

pub(super) fn runtime_proxy_transport_failure_kind(
    err: &anyhow::Error,
) -> Option<RuntimeTransportFailureKind> {
    for cause in err.chain() {
        if let Some(reqwest_error) = cause.downcast_ref::<reqwest::Error>()
            && let Some(kind) = runtime_transport_failure_kind_from_reqwest(reqwest_error)
        {
            return Some(kind);
        }
        if let Some(ws_error) = cause.downcast_ref::<WsError>()
            && let Some(kind) = runtime_transport_failure_kind_from_ws(ws_error)
        {
            return Some(kind);
        }
        if let Some(io_error) = cause.downcast_ref::<io::Error>()
            && let Some(kind) = runtime_transport_failure_kind_from_io_error(io_error)
        {
            return Some(kind);
        }
        if let Some(kind) = runtime_transport_failure_kind_from_message(&cause.to_string()) {
            return Some(kind);
        }
    }
    None
}

pub(super) fn runtime_profile_transport_health_penalty(kind: RuntimeTransportFailureKind) -> u32 {
    match kind {
        RuntimeTransportFailureKind::Dns
        | RuntimeTransportFailureKind::ConnectTimeout
        | RuntimeTransportFailureKind::ConnectRefused
        | RuntimeTransportFailureKind::ConnectReset
        | RuntimeTransportFailureKind::TlsHandshake => {
            RUNTIME_PROFILE_CONNECT_FAILURE_HEALTH_PENALTY
        }
        RuntimeTransportFailureKind::BrokenPipe
        | RuntimeTransportFailureKind::ConnectionAborted
        | RuntimeTransportFailureKind::UnexpectedEof
        | RuntimeTransportFailureKind::ReadTimeout
        | RuntimeTransportFailureKind::UpstreamClosedBeforeCommit
        | RuntimeTransportFailureKind::Other => RUNTIME_PROFILE_TRANSPORT_FAILURE_HEALTH_PENALTY,
    }
}

pub(super) fn reset_runtime_profile_success_streak(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) {
    runtime
        .profile_health
        .remove(&runtime_profile_route_success_streak_key(
            profile_name,
            route_kind,
        ));
}

pub(super) fn bump_runtime_profile_bad_pairing_score(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    delta: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_bad_pairing_key(profile_name, route_kind);
    let next_score = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &key,
        now,
        RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
    )
    .saturating_add(delta)
    .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);
    reset_runtime_profile_success_streak(&mut runtime, profile_name, route_kind);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_score,
            updated_at: now,
        },
    );
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!(
            "profile_bad_pairing:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_bad_pairing profile={profile_name} route={} score={next_score} delta={delta} reason={reason}",
            runtime_route_kind_label(route_kind)
        ),
    );
    Ok(())
}

pub(super) fn bump_runtime_profile_health_score(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    delta: u32,
    reason: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    let key = runtime_profile_route_health_key(profile_name, route_kind);
    let next_score = runtime
        .profile_health
        .get(&key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
        .unwrap_or(0)
        .saturating_add(delta)
        .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);
    reset_runtime_profile_success_streak(&mut runtime, profile_name, route_kind);
    runtime.profile_health.insert(
        key,
        RuntimeProfileHealth {
            score: next_score,
            updated_at: now,
        },
    );
    let circuit_until = if next_score >= RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD {
        let circuit_key = runtime_profile_route_circuit_key(profile_name, route_kind);
        let reopen_stage = if runtime
            .profile_route_circuit_open_until
            .contains_key(&circuit_key)
        {
            runtime_profile_effective_score_from_map(
                &runtime.profile_health,
                &runtime_profile_route_circuit_reopen_key(profile_name, route_kind),
                now,
                RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS,
            )
            .saturating_add(1)
            .min(RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE)
        } else {
            0
        };
        if reopen_stage == 0 {
            runtime
                .profile_health
                .remove(&runtime_profile_route_circuit_reopen_key(
                    profile_name,
                    route_kind,
                ));
        } else {
            runtime.profile_health.insert(
                runtime_profile_route_circuit_reopen_key(profile_name, route_kind),
                RuntimeProfileHealth {
                    score: reopen_stage,
                    updated_at: now,
                },
            );
        }
        let until = now.saturating_add(runtime_profile_circuit_open_seconds(
            next_score,
            reopen_stage,
        ));
        runtime
            .profile_route_circuit_open_until
            .entry(circuit_key)
            .and_modify(|current| *current = (*current).max(until))
            .or_insert(until);
        Some((until, reopen_stage))
    } else {
        None
    };
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!(
            "profile_health:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_health profile={profile_name} route={} score={next_score} delta={delta} reason={reason}",
            runtime_route_kind_label(route_kind)
        ),
    );
    if let Some((until, reopen_stage)) = circuit_until {
        runtime_proxy_log(
            shared,
            format!(
                "profile_circuit_open profile={profile_name} route={} until={until} reopen_stage={reopen_stage} reason={reason} score={next_score}",
                runtime_route_kind_label(route_kind)
            ),
        );
    }
    Ok(())
}

pub(super) fn recover_runtime_profile_health_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) {
    let key = runtime_profile_route_health_key(profile_name, route_kind);
    let streak_key = runtime_profile_route_success_streak_key(profile_name, route_kind);
    let Some(current_score) = runtime
        .profile_health
        .get(&key)
        .map(|entry| runtime_profile_effective_health_score(entry, now))
    else {
        runtime.profile_health.remove(&streak_key);
        return;
    };

    let next_streak = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &streak_key,
        now,
        RUNTIME_PROFILE_SUCCESS_STREAK_DECAY_SECONDS,
    )
    .saturating_add(1)
    .min(RUNTIME_PROFILE_SUCCESS_STREAK_MAX);
    let recovery = RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE
        .saturating_add(next_streak.saturating_sub(1).min(1));
    let next_score = current_score.saturating_sub(recovery);
    if next_score == 0 {
        runtime.profile_health.remove(&key);
        runtime.profile_health.remove(&streak_key);
    } else {
        runtime.profile_health.insert(
            key,
            RuntimeProfileHealth {
                score: next_score,
                updated_at: now,
            },
        );
        runtime.profile_health.insert(
            streak_key,
            RuntimeProfileHealth {
                score: next_streak,
                updated_at: now,
            },
        );
    }
}

pub(super) fn mark_runtime_profile_retry_backoff(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    runtime.profile_probe_cache.remove(profile_name);
    let until = now.saturating_add(RUNTIME_PROFILE_RETRY_BACKOFF_SECONDS);
    runtime
        .profile_retry_backoff_until
        .insert(profile_name.to_string(), until);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!("profile_retry_backoff:{profile_name}"),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!("profile_retry_backoff profile={profile_name} until={until}"),
    );
    Ok(())
}

pub(super) fn mark_runtime_profile_transport_backoff(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
) -> Result<()> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let now = Local::now().timestamp();
    prune_runtime_profile_selection_backoff(&mut runtime, now);
    runtime.profile_probe_cache.remove(profile_name);
    let route_key = runtime_profile_transport_backoff_key(profile_name, route_kind);
    let existing_remaining = runtime_profile_transport_backoff_until_from_map(
        &runtime.profile_transport_backoff_until,
        profile_name,
        route_kind,
        now,
    )
    .unwrap_or(now)
    .saturating_sub(now);
    let next_backoff_seconds = if existing_remaining > 0 {
        existing_remaining.saturating_mul(2).clamp(
            RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS,
            RUNTIME_PROFILE_TRANSPORT_BACKOFF_MAX_SECONDS,
        )
    } else {
        RUNTIME_PROFILE_TRANSPORT_BACKOFF_SECONDS
    };
    let until = now.saturating_add(next_backoff_seconds);
    runtime
        .profile_transport_backoff_until
        .entry(route_key)
        .and_modify(|current| *current = (*current).max(until))
        .or_insert(until);
    schedule_runtime_state_save_from_runtime(
        shared,
        &runtime,
        &format!(
            "profile_transport_backoff:{profile_name}:{}",
            runtime_route_kind_label(route_kind)
        ),
    );
    drop(runtime);
    runtime_proxy_log(
        shared,
        format!(
            "profile_transport_backoff profile={profile_name} route={} until={until} seconds={next_backoff_seconds} context={context}",
            runtime_route_kind_label(route_kind)
        ),
    );
    Ok(())
}

pub(super) fn note_runtime_profile_transport_failure(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    context: &str,
    err: &anyhow::Error,
) {
    let Some(failure_kind) = runtime_proxy_transport_failure_kind(err) else {
        return;
    };
    runtime_proxy_log(
        shared,
        format!(
            "profile_transport_failure profile={profile_name} route={} class={} context={context}",
            runtime_route_kind_label(route_kind),
            runtime_transport_failure_kind_label(failure_kind),
        ),
    );
    let _ = bump_runtime_profile_health_score(
        shared,
        profile_name,
        route_kind,
        runtime_profile_transport_health_penalty(failure_kind),
        context,
    );
    let _ = bump_runtime_profile_bad_pairing_score(
        shared,
        profile_name,
        route_kind,
        RUNTIME_PROFILE_BAD_PAIRING_PENALTY,
        context,
    );
    note_runtime_profile_latency_failure(shared, profile_name, route_kind, context);
    let _ = mark_runtime_profile_transport_backoff(shared, profile_name, route_kind, context);
}

pub(super) fn clear_runtime_profile_transport_backoff_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> bool {
    let mut changed = runtime
        .profile_transport_backoff_until
        .remove(&runtime_profile_transport_backoff_key(
            profile_name,
            route_kind,
        ))
        .is_some();
    changed = runtime
        .profile_transport_backoff_until
        .remove(profile_name)
        .is_some()
        || changed;
    changed
}

pub(super) fn commit_runtime_proxy_profile_selection(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<bool> {
    commit_runtime_proxy_profile_selection_with_policy(shared, profile_name, route_kind, true)
}

pub(super) fn commit_runtime_proxy_profile_selection_with_policy(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    track_current_profile: bool,
) -> Result<bool> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let switch_runtime_profile = track_current_profile && runtime.current_profile != profile_name;
    let switch_global_profile =
        track_current_profile && !matches!(route_kind, RuntimeRouteKind::Compact);
    let switched = switch_runtime_profile;
    let now = Local::now().timestamp();
    let cleared_retry_backoff = runtime
        .profile_retry_backoff_until
        .remove(profile_name)
        .is_some();
    let cleared_transport_backoff =
        clear_runtime_profile_transport_backoff_for_route(&mut runtime, profile_name, route_kind);
    let cleared_route_circuit =
        clear_runtime_profile_circuit_for_route(&mut runtime, profile_name, route_kind);
    let cleared_health =
        clear_runtime_profile_health_for_route(&mut runtime, profile_name, route_kind, now);
    if switch_runtime_profile {
        runtime.current_profile = profile_name.to_string();
    }
    let state_changed =
        switch_global_profile && runtime.state.active_profile.as_deref() != Some(profile_name);
    if switch_global_profile {
        runtime.state.active_profile = Some(profile_name.to_string());
        record_run_selection(&mut runtime.state, profile_name);
    }
    let should_persist = switched
        || state_changed
        || cleared_retry_backoff
        || cleared_transport_backoff
        || cleared_route_circuit
        || cleared_health;
    if should_persist {
        schedule_runtime_state_save_from_runtime(
            shared,
            &runtime,
            &format!("profile_commit:{profile_name}"),
        );
    }
    drop(runtime);
    if switch_runtime_profile {
        update_runtime_broker_current_profile(&shared.log_path, profile_name);
    }
    runtime_proxy_log(
        shared,
        format!(
            "profile_commit profile={profile_name} route={} switched={switched} persisted={should_persist} track_current_profile={track_current_profile} cleared_route_circuit={cleared_route_circuit}",
            runtime_route_kind_label(route_kind),
        ),
    );
    Ok(switched)
}

pub(super) fn clear_runtime_profile_health_for_route(
    runtime: &mut RuntimeRotationState,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> bool {
    let mut changed = runtime.profile_health.remove(profile_name).is_some();
    let previous_route_score =
        runtime_profile_route_health_score(runtime, profile_name, now, route_kind);
    let previous_bad_pairing = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
    );
    let previous_circuit_reopen = runtime_profile_effective_score_from_map(
        &runtime.profile_health,
        &runtime_profile_route_circuit_reopen_key(profile_name, route_kind),
        now,
        RUNTIME_PROFILE_CIRCUIT_REOPEN_DECAY_SECONDS,
    );
    recover_runtime_profile_health_for_route(runtime, profile_name, route_kind, now);
    changed = changed || previous_route_score > 0;
    changed = changed || previous_bad_pairing > 0;
    changed = changed || previous_circuit_reopen > 0;
    changed = runtime
        .profile_health
        .remove(&runtime_profile_route_bad_pairing_key(
            profile_name,
            route_kind,
        ))
        .is_some()
        || changed;
    changed = runtime
        .profile_health
        .remove(&runtime_profile_route_circuit_reopen_key(
            profile_name,
            route_kind,
        ))
        .is_some()
        || changed;
    changed
}

pub(super) fn commit_runtime_proxy_profile_selection_with_notice(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> Result<()> {
    let _ = commit_runtime_proxy_profile_selection(shared, profile_name, route_kind)?;
    Ok(())
}

pub(super) fn send_runtime_proxy_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<reqwest::Response> {
    let started_at = Instant::now();
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .clone();
    let auth = runtime_profile_usage_auth(shared, profile_name)?;
    let upstream_url =
        runtime_proxy_upstream_url(&runtime.upstream_base_url, &request.path_and_query);
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime auto-rotate",
            request.method
        )
    })?;

    let mut upstream_request = shared.async_client.request(method, &upstream_url);
    for (name, value) in &request.headers {
        if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
            continue;
        }
        if should_skip_runtime_request_header(name) {
            continue;
        }
        upstream_request = upstream_request.header(name.as_str(), value.as_str());
    }
    if let Some(turn_state) = turn_state_override {
        upstream_request = upstream_request.header("x-codex-turn-state", turn_state);
    }

    upstream_request = upstream_request
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .body(request.body.clone());

    if let Some(user_agent) = runtime_proxy_effective_user_agent(&request.headers) {
        upstream_request = upstream_request.header("User-Agent", user_agent);
    } else {
        upstream_request = upstream_request.header("User-Agent", "codex-cli");
    }

    if let Some(account_id) = auth.account_id.as_deref() {
        upstream_request = upstream_request.header("ChatGPT-Account-Id", account_id);
    }

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_start profile={profile_name} method={} url={} turn_state_override={:?} previous_response_id={:?}",
            request.method,
            upstream_url,
            turn_state_override,
            runtime_request_previous_response_id(request)
        ),
    );
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE") {
        bail!("injected runtime upstream connect failure");
    }
    let response = match shared
        .async_runtime
        .block_on(async move { upstream_request.send().await })
    {
        Ok(response) => response,
        Err(err) => {
            log_runtime_upstream_connect_failure(
                shared,
                request_id,
                "http",
                profile_name,
                runtime_transport_failure_kind_from_reqwest(&err),
                &err,
            );
            return Err(anyhow::Error::new(err).context(format!(
                "failed to proxy runtime request for profile '{}' to {}",
                profile_name, upstream_url
            )));
        }
    };
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_response profile={profile_name} status={} content_type={:?} turn_state={:?}",
            response.status().as_u16(),
            response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            runtime_proxy_header_value(response.headers(), "x-codex-turn-state")
        ),
    );
    if matches!(response.status().as_u16(), 401 | 403) && response.status().as_u16() == 401 {
        note_runtime_profile_auth_failure(
            shared,
            profile_name,
            runtime_proxy_request_lane(&request.path_and_query, false),
            response.status().as_u16(),
        );
    }
    note_runtime_profile_latency_observation(
        shared,
        profile_name,
        runtime_proxy_request_lane(&request.path_and_query, false),
        "connect",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(response)
}

pub(super) fn send_runtime_proxy_upstream_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    turn_state_override: Option<&str>,
) -> Result<reqwest::Response> {
    let started_at = Instant::now();
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?
        .clone();
    let auth = runtime_profile_usage_auth(shared, profile_name)?;
    let upstream_url =
        runtime_proxy_upstream_url(&runtime.upstream_base_url, &request.path_and_query);
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).with_context(|| {
        format!(
            "failed to proxy unsupported HTTP method '{}' for runtime auto-rotate",
            request.method
        )
    })?;

    let mut upstream_request = shared.async_client.request(method, &upstream_url);
    for (name, value) in &request.headers {
        if turn_state_override.is_some() && name.eq_ignore_ascii_case("x-codex-turn-state") {
            continue;
        }
        if should_skip_runtime_request_header(name) {
            continue;
        }
        upstream_request = upstream_request.header(name.as_str(), value.as_str());
    }
    if let Some(turn_state) = turn_state_override {
        upstream_request = upstream_request.header("x-codex-turn-state", turn_state);
    }

    upstream_request = upstream_request
        .header("Authorization", format!("Bearer {}", auth.access_token))
        .body(request.body.clone());

    if let Some(user_agent) = runtime_proxy_effective_user_agent(&request.headers) {
        upstream_request = upstream_request.header("User-Agent", user_agent);
    } else {
        upstream_request = upstream_request.header("User-Agent", "codex-cli");
    }

    if let Some(account_id) = auth.account_id.as_deref() {
        upstream_request = upstream_request.header("ChatGPT-Account-Id", account_id);
    }

    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_async_start profile={profile_name} method={} url={} turn_state_override={:?} previous_response_id={:?}",
            request.method,
            upstream_url,
            turn_state_override,
            runtime_request_previous_response_id(request)
        ),
    );
    if runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE") {
        bail!("injected runtime upstream connect failure");
    }
    let response = match shared
        .async_runtime
        .block_on(async move { upstream_request.send().await })
    {
        Ok(response) => response,
        Err(err) => {
            log_runtime_upstream_connect_failure(
                shared,
                request_id,
                "http",
                profile_name,
                runtime_transport_failure_kind_from_reqwest(&err),
                &err,
            );
            return Err(anyhow::Error::new(err).context(format!(
                "failed to proxy runtime request for profile '{}' to {}",
                profile_name, upstream_url
            )));
        }
    };
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http upstream_async_response profile={profile_name} status={} content_type={:?} turn_state={:?}",
            response.status().as_u16(),
            response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            runtime_proxy_header_value(response.headers(), "x-codex-turn-state")
        ),
    );
    if matches!(response.status().as_u16(), 401 | 403) && response.status().as_u16() == 401 {
        note_runtime_profile_auth_failure(
            shared,
            profile_name,
            RuntimeRouteKind::Responses,
            response.status().as_u16(),
        );
    }
    note_runtime_profile_latency_observation(
        shared,
        profile_name,
        RuntimeRouteKind::Responses,
        "connect",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(response)
}

pub(super) fn runtime_proxy_upstream_url(base_url: &str, path_and_query: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    if base_url.contains("/backend-api")
        && let Some(suffix) = normalized_path_and_query
            .as_ref()
            .strip_prefix("/backend-api")
    {
        return format!("{base_url}{suffix}");
    }
    if normalized_path_and_query.starts_with('/') {
        return format!("{base_url}{normalized_path_and_query}");
    }
    format!("{base_url}/{normalized_path_and_query}")
}

pub(super) fn runtime_proxy_upstream_websocket_url(
    base_url: &str,
    path_and_query: &str,
) -> Result<String> {
    let upstream_url = runtime_proxy_upstream_url(base_url, path_and_query);
    let mut url = reqwest::Url::parse(&upstream_url)
        .with_context(|| format!("failed to parse upstream websocket URL {}", upstream_url))?;
    match url.scheme() {
        "http" => {
            url.set_scheme("ws").map_err(|_| {
                anyhow::anyhow!("failed to set websocket scheme for {upstream_url}")
            })?;
        }
        "https" => {
            url.set_scheme("wss").map_err(|_| {
                anyhow::anyhow!("failed to set websocket scheme for {upstream_url}")
            })?;
        }
        "ws" | "wss" => {}
        scheme => bail!(
            "unsupported upstream websocket scheme '{scheme}' in {}",
            upstream_url
        ),
    }
    Ok(url.to_string())
}

pub(super) fn should_skip_runtime_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "authorization"
            | "chatgpt-account-id"
            | "connection"
            | "content-length"
            | "host"
            | "transfer-encoding"
            | "upgrade"
    ) || lower.starts_with("sec-websocket-")
        || lower.starts_with("x-prodex-internal-")
}

pub(super) fn runtime_proxy_effective_user_agent(headers: &[(String, String)]) -> Option<&str> {
    headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("user-agent")
            .then_some(value.as_str())
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn should_skip_runtime_response_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "content-encoding"
            | "content-length"
            | "date"
            | "server"
            | "transfer-encoding"
    )
}

pub(super) fn forward_runtime_proxy_response(
    shared: &RuntimeRotationProxyShared,
    response: reqwest::Response,
    prelude: Vec<u8>,
) -> Result<tiny_http::ResponseBox> {
    let parts = buffer_runtime_proxy_async_response_parts(shared, response, prelude)?;
    Ok(build_runtime_proxy_response_from_parts(parts))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_runtime_proxy_responses_success(
    request_id: u64,
    request_previous_response_id: Option<&str>,
    request_session_id: Option<&str>,
    request_turn_state: Option<&str>,
    response: reqwest::Response,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    inflight_guard: RuntimeProfileInFlightGuard,
) -> Result<RuntimeResponsesAttempt> {
    let turn_state = runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
    remember_runtime_successful_previous_response_owner(
        shared,
        profile_name,
        request_previous_response_id,
        RuntimeRouteKind::Responses,
    )?;
    remember_runtime_session_id(
        shared,
        profile_name,
        request_session_id,
        RuntimeRouteKind::Responses,
    )?;
    remember_runtime_turn_state(
        shared,
        profile_name,
        turn_state.as_deref(),
        RuntimeRouteKind::Responses,
    )?;
    let is_sse = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/event-stream"));
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http prepare_success profile={profile_name} sse={is_sse} turn_state={:?}",
            turn_state
        ),
    );
    if !is_sse {
        let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())?;
        let response_ids = extract_runtime_response_ids_from_body_bytes(&parts.body);
        if !response_ids.is_empty() {
            remember_runtime_response_ids(
                shared,
                profile_name,
                &response_ids,
                RuntimeRouteKind::Responses,
            )?;
            let _ = release_runtime_compact_lineage(
                shared,
                profile_name,
                request_session_id,
                request_turn_state,
                "response_committed",
            );
        }
        return Ok(RuntimeResponsesAttempt::Success {
            profile_name: profile_name.to_string(),
            response: RuntimeResponsesReply::Buffered(parts),
        });
    }

    let status = response.status().as_u16();
    let mut headers = Vec::new();
    for (name, value) in response.headers() {
        if should_skip_runtime_response_header(name.as_str()) {
            continue;
        }
        if let Ok(value) = value.to_str() {
            headers.push((name.to_string(), value.to_string()));
        }
    }

    let mut prefetch = RuntimePrefetchStream::spawn(
        response,
        Arc::clone(&shared.async_runtime),
        shared.log_path.clone(),
        request_id,
    );
    let lookahead = inspect_runtime_sse_lookahead(&mut prefetch, &shared.log_path, request_id)?;

    let (prelude, response_ids) = match lookahead {
        RuntimeSseInspection::Commit {
            prelude,
            response_ids,
        } => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http sse_commit profile={profile_name} prelude_bytes={} response_ids={}",
                    prelude.len(),
                    response_ids.len()
                ),
            );
            (prelude, response_ids)
        }
        RuntimeSseInspection::QuotaBlocked(prelude) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http sse_quota_blocked profile={profile_name} prelude_bytes={}",
                    prelude.len()
                ),
            );
            return Ok(RuntimeResponsesAttempt::QuotaBlocked {
                profile_name: profile_name.to_string(),
                response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
                    status,
                    headers: headers.clone(),
                    body: Box::new(prefetch.into_reader(prelude)),
                    request_id,
                    profile_name: profile_name.to_string(),
                    log_path: shared.log_path.clone(),
                    shared: shared.clone(),
                    _inflight_guard: Some(inflight_guard),
                }),
            });
        }
        RuntimeSseInspection::PreviousResponseNotFound(prelude) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} stage=sse_prelude prelude_bytes={}",
                    prelude.len()
                ),
            );
            return Ok(RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name: profile_name.to_string(),
                response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
                    status,
                    headers: headers.clone(),
                    body: Box::new(prefetch.into_reader(prelude)),
                    request_id,
                    profile_name: profile_name.to_string(),
                    log_path: shared.log_path.clone(),
                    shared: shared.clone(),
                    _inflight_guard: Some(inflight_guard),
                }),
                turn_state,
            });
        }
    };
    remember_runtime_response_ids(
        shared,
        profile_name,
        &response_ids,
        RuntimeRouteKind::Responses,
    )?;
    if !response_ids.is_empty() {
        let _ = release_runtime_compact_lineage(
            shared,
            profile_name,
            request_session_id,
            request_turn_state,
            "response_committed",
        );
    }

    Ok(RuntimeResponsesAttempt::Success {
        profile_name: profile_name.to_string(),
        response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
            status,
            headers,
            body: Box::new(RuntimeSseTapReader::new(
                prefetch.into_reader(prelude.clone()),
                shared.clone(),
                profile_name.to_string(),
                &prelude,
                &response_ids,
            )),
            request_id,
            profile_name: profile_name.to_string(),
            log_path: shared.log_path.clone(),
            shared: shared.clone(),
            _inflight_guard: Some(inflight_guard),
        }),
    })
}

impl RuntimeSseTapState {
    fn observe(&mut self, shared: &RuntimeRotationProxyShared, profile_name: &str, chunk: &[u8]) {
        for byte in chunk {
            self.line.push(*byte);
            if *byte != b'\n' {
                continue;
            }

            let line_text = String::from_utf8_lossy(&self.line);
            let trimmed = line_text.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                self.remember_response_ids(shared, profile_name, RuntimeRouteKind::Responses);
                self.data_lines.clear();
                self.line.clear();
                continue;
            }

            if let Some(payload) = trimmed.strip_prefix("data:") {
                self.data_lines.push(payload.trim_start().to_string());
            }
            self.line.clear();
        }
    }

    fn finish(&mut self, shared: &RuntimeRotationProxyShared, profile_name: &str) {
        self.remember_response_ids(shared, profile_name, RuntimeRouteKind::Responses);
    }

    fn remember_response_ids(
        &mut self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        verified_route: RuntimeRouteKind,
    ) {
        let fresh_ids = extract_runtime_response_ids_from_sse(&self.data_lines)
            .into_iter()
            .filter(|response_id| self.remembered_response_ids.insert(response_id.clone()))
            .collect::<Vec<_>>();
        if fresh_ids.is_empty() {
            return;
        }
        let _ = remember_runtime_response_ids(shared, profile_name, &fresh_ids, verified_route);
    }
}

pub(super) struct RuntimeSseTapReader {
    inner: Box<dyn Read + Send>,
    shared: RuntimeRotationProxyShared,
    profile_name: String,
    state: RuntimeSseTapState,
}

impl Read for RuntimePrefetchReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.finished {
            return Ok(0);
        }

        loop {
            let read = self.pending.read(buf)?;
            if read > 0 {
                return Ok(read);
            }

            let next = if let Some(chunk) = self.backlog.pop_front() {
                Some(chunk)
            } else {
                match self
                    .receiver
                    .recv_timeout(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms()))
                {
                    Ok(chunk) => {
                        if let RuntimePrefetchChunk::Data(bytes) = &chunk {
                            runtime_prefetch_release_queued_bytes(&self.shared, bytes.len());
                        }
                        Some(chunk)
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        self.finished = true;
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "runtime upstream stream idle timed out",
                        ));
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        if let Some((kind, message)) = runtime_prefetch_terminal_error(&self.shared)
                        {
                            self.finished = true;
                            return Err(io::Error::new(kind, message));
                        }
                        None
                    }
                }
            };

            match next {
                Some(RuntimePrefetchChunk::Data(chunk)) => {
                    self.pending = Cursor::new(chunk);
                }
                Some(RuntimePrefetchChunk::End) | None => {
                    self.finished = true;
                    return Ok(0);
                }
                Some(RuntimePrefetchChunk::Error(kind, message)) => {
                    self.finished = true;
                    return Err(io::Error::new(kind, message));
                }
            }
        }
    }
}

impl Drop for RuntimePrefetchReader {
    fn drop(&mut self) {
        self.worker_abort.abort();
    }
}

impl RuntimeSseTapReader {
    pub(super) fn new(
        inner: impl Read + Send + 'static,
        shared: RuntimeRotationProxyShared,
        profile_name: String,
        prelude: &[u8],
        remembered_response_ids: &[String],
    ) -> Self {
        let mut state = RuntimeSseTapState {
            remembered_response_ids: remembered_response_ids.iter().cloned().collect(),
            ..RuntimeSseTapState::default()
        };
        state.observe(&shared, &profile_name, prelude);
        Self {
            inner: Box::new(inner),
            shared,
            profile_name,
            state,
        }
    }
}

impl Read for RuntimeSseTapReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = match self.inner.read(buf) {
            Ok(read) => read,
            Err(err) => {
                let transport_error =
                    anyhow::Error::new(io::Error::new(err.kind(), err.to_string()));
                note_runtime_profile_transport_failure(
                    &self.shared,
                    &self.profile_name,
                    RuntimeRouteKind::Responses,
                    "sse_read",
                    &transport_error,
                );
                return Err(err);
            }
        };
        if read == 0 {
            self.state.finish(&self.shared, &self.profile_name);
            return Ok(0);
        }
        self.state
            .observe(&self.shared, &self.profile_name, &buf[..read]);
        Ok(read)
    }
}

pub(super) fn write_runtime_streaming_response(
    writer: Box<dyn Write + Send + 'static>,
    mut response: RuntimeStreamingResponse,
) -> io::Result<()> {
    let mut writer = writer;
    let flush_each_chunk = response.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            && value.to_ascii_lowercase().contains("text/event-stream")
    });
    let started_at = Instant::now();
    let log_writer_error = |stage: &str,
                            chunk_count: usize,
                            total_bytes: usize,
                            err: &io::Error| {
        runtime_proxy_log_to_path(
            &response.log_path,
            &format!(
                "local_writer_error request={} transport=http profile={} stage={} chunks={} bytes={} elapsed_ms={} error={}",
                response.request_id,
                response.profile_name,
                stage,
                chunk_count,
                total_bytes,
                started_at.elapsed().as_millis(),
                err
            ),
        );
    };
    runtime_proxy_log_to_path(
        &response.log_path,
        &format!(
            "request={} transport=http stream_start profile={} status={}",
            response.request_id, response.profile_name, response.status
        ),
    );
    let status = reqwest::StatusCode::from_u16(response.status)
        .ok()
        .and_then(|status| status.canonical_reason().map(str::to_string))
        .unwrap_or_else(|| "OK".to_string());
    write!(
        writer,
        "HTTP/1.1 {} {}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n",
        response.status, status
    )
    .map_err(|err| {
        log_writer_error("headers_start", 0, 0, &err);
        err
    })?;
    for (name, value) in response.headers {
        write!(writer, "{name}: {value}\r\n").map_err(|err| {
            log_writer_error("header_line", 0, 0, &err);
            err
        })?;
    }
    writer.write_all(b"\r\n").inspect_err(|err| {
        log_writer_error("headers_end", 0, 0, err);
    })?;
    writer.flush().inspect_err(|err| {
        log_writer_error("headers_flush", 0, 0, err);
    })?;

    let mut buffer = [0_u8; 8192];
    let mut total_bytes = 0usize;
    let mut chunk_count = 0usize;
    loop {
        let read = match response.body.read(&mut buffer) {
            Ok(read) => read,
            Err(err) => {
                runtime_proxy_log_to_path(
                    &response.log_path,
                    &format!(
                        "request={} transport=http stream_read_error profile={} chunks={} bytes={} elapsed_ms={} error={}",
                        response.request_id,
                        response.profile_name,
                        chunk_count,
                        total_bytes,
                        started_at.elapsed().as_millis(),
                        err
                    ),
                );
                let transport_error =
                    anyhow::Error::new(io::Error::new(err.kind(), err.to_string()));
                if is_runtime_proxy_transport_failure(&transport_error) {
                    note_runtime_profile_latency_failure(
                        &response.shared,
                        &response.profile_name,
                        RuntimeRouteKind::Responses,
                        "stream_read_error",
                    );
                }
                return Err(err);
            }
        };
        if read == 0 {
            break;
        }
        if chunk_count == 0
            && runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_STREAM_READ_ERROR_ONCE")
        {
            let err = io::Error::new(
                io::ErrorKind::ConnectionReset,
                "injected runtime stream read failure",
            );
            runtime_proxy_log_to_path(
                &response.log_path,
                &format!(
                    "request={} transport=http stream_read_error profile={} chunks={} bytes={} elapsed_ms={} error={}",
                    response.request_id,
                    response.profile_name,
                    chunk_count,
                    total_bytes,
                    started_at.elapsed().as_millis(),
                    err
                ),
            );
            note_runtime_profile_latency_failure(
                &response.shared,
                &response.profile_name,
                RuntimeRouteKind::Responses,
                "stream_read_error",
            );
            return Err(err);
        }
        chunk_count += 1;
        total_bytes += read;
        if chunk_count == 1 {
            runtime_proxy_log_to_path(
                &response.log_path,
                &format!(
                    "request={} transport=http first_local_chunk profile={} bytes={} elapsed_ms={}",
                    response.request_id,
                    response.profile_name,
                    read,
                    started_at.elapsed().as_millis()
                ),
            );
            note_runtime_profile_latency_observation(
                &response.shared,
                &response.profile_name,
                RuntimeRouteKind::Responses,
                "ttfb",
                started_at.elapsed().as_millis() as u64,
            );
        }
        write!(writer, "{:X}\r\n", read).map_err(|err| {
            log_writer_error("chunk_size", chunk_count, total_bytes, &err);
            err
        })?;
        writer.write_all(&buffer[..read]).inspect_err(|err| {
            log_writer_error("chunk_body", chunk_count, total_bytes, err);
        })?;
        writer.write_all(b"\r\n").inspect_err(|err| {
            log_writer_error("chunk_suffix", chunk_count, total_bytes, err);
        })?;
        if flush_each_chunk || chunk_count == 1 {
            writer.flush().inspect_err(|err| {
                log_writer_error("chunk_flush", chunk_count, total_bytes, err);
            })?;
        }
    }
    writer.write_all(b"0\r\n\r\n").inspect_err(|err| {
        log_writer_error("trailer", chunk_count, total_bytes, err);
    })?;
    writer.flush().inspect_err(|err| {
        log_writer_error("trailer_flush", chunk_count, total_bytes, err);
    })?;
    runtime_proxy_log_to_path(
        &response.log_path,
        &format!(
            "request={} transport=http stream_complete profile={} chunks={} bytes={} elapsed_ms={}",
            response.request_id,
            response.profile_name,
            chunk_count,
            total_bytes,
            started_at.elapsed().as_millis()
        ),
    );
    note_runtime_profile_latency_observation(
        &response.shared,
        &response.profile_name,
        RuntimeRouteKind::Responses,
        "stream_complete",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(())
}

pub(super) fn build_runtime_proxy_text_response_parts(
    status: u16,
    message: &str,
) -> RuntimeBufferedResponseParts {
    RuntimeBufferedResponseParts {
        status,
        headers: vec![(
            "Content-Type".to_string(),
            b"text/plain; charset=utf-8".to_vec(),
        )],
        body: message.as_bytes().to_vec(),
    }
}

pub(super) fn build_runtime_proxy_text_response(
    status: u16,
    message: &str,
) -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(build_runtime_proxy_text_response_parts(
        status, message,
    ))
}

pub(super) fn runtime_proxy_header_value(
    headers: &reqwest::header::HeaderMap,
    name: &str,
) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn runtime_proxy_tungstenite_header_value(
    headers: &tungstenite::http::HeaderMap,
    name: &str,
) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn is_runtime_proxy_transport_failure(err: &anyhow::Error) -> bool {
    runtime_proxy_transport_failure_kind(err).is_some()
}

pub(super) fn build_runtime_proxy_json_error_parts(
    status: u16,
    code: &str,
    message: &str,
) -> RuntimeBufferedResponseParts {
    let body = serde_json::json!({
        "error": {
            "code": code,
            "message": message,
        }
    })
    .to_string();
    RuntimeBufferedResponseParts {
        status,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: body.into_bytes(),
    }
}

pub(super) fn build_runtime_proxy_json_error_response(
    status: u16,
    code: &str,
    message: &str,
) -> tiny_http::ResponseBox {
    build_runtime_proxy_response_from_parts(build_runtime_proxy_json_error_parts(
        status, code, message,
    ))
}

pub(super) struct RuntimeBufferedResponseParts {
    pub(super) status: u16,
    pub(super) headers: Vec<(String, Vec<u8>)>,
    pub(super) body: Vec<u8>,
}

pub(super) fn is_runtime_responses_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/codex/responses")
}

pub(super) fn is_runtime_anthropic_messages_path(path_and_query: &str) -> bool {
    path_without_query(path_and_query).ends_with(RUNTIME_PROXY_ANTHROPIC_MESSAGES_PATH)
}

pub(super) fn is_runtime_compact_path(path_and_query: &str) -> bool {
    let normalized_path_and_query = runtime_proxy_normalize_openai_path(path_and_query);
    path_without_query(normalized_path_and_query.as_ref()).ends_with("/responses/compact")
}

pub(super) fn path_without_query(path_and_query: &str) -> &str {
    path_and_query
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(path_and_query)
}

pub(super) fn runtime_proxy_openai_suffix(path: &str) -> Option<&str> {
    if let Some(suffix) = path.strip_prefix(LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX)
        && let Some(version_suffix_index) = suffix.find('/')
    {
        return Some(&suffix[version_suffix_index..]);
    }

    if let Some(suffix) = path.strip_prefix(RUNTIME_PROXY_OPENAI_MOUNT_PATH)
        && (suffix.is_empty() || suffix.starts_with('/'))
    {
        return Some(suffix);
    }

    None
}

pub(super) fn runtime_proxy_normalize_openai_path(path_and_query: &str) -> Cow<'_, str> {
    let (path, query) = match path_and_query.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (path_and_query, None),
    };
    let Some(suffix) = runtime_proxy_openai_suffix(path) else {
        return Cow::Borrowed(path_and_query);
    };

    let mut normalized =
        String::with_capacity(path_and_query.len() + RUNTIME_PROXY_OPENAI_UPSTREAM_PATH.len());
    normalized.push_str(RUNTIME_PROXY_OPENAI_UPSTREAM_PATH);
    normalized.push_str(suffix);
    if let Some(query) = query {
        normalized.push('?');
        normalized.push_str(query);
    }
    Cow::Owned(normalized)
}

impl RuntimePrefetchStream {
    fn spawn(
        response: reqwest::Response,
        async_runtime: Arc<TokioRuntime>,
        log_path: PathBuf,
        request_id: u64,
    ) -> Self {
        let (sender, receiver) =
            mpsc::sync_channel::<RuntimePrefetchChunk>(RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY);
        let shared = Arc::new(RuntimePrefetchSharedState::default());
        let worker_shared = Arc::clone(&shared);
        let worker = async_runtime.spawn(async move {
            runtime_prefetch_response_chunks(response, sender, worker_shared, log_path, request_id)
                .await;
        });
        let worker_abort = worker.abort_handle();
        Self {
            receiver: Some(receiver),
            shared,
            backlog: VecDeque::new(),
            worker_abort: Some(worker_abort),
        }
    }

    fn recv_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::result::Result<RuntimePrefetchChunk, RecvTimeoutError> {
        if let Some(chunk) = self.backlog.pop_front() {
            return Ok(chunk);
        }
        let chunk = self
            .receiver
            .as_ref()
            .expect("runtime prefetch receiver should remain available")
            .recv_timeout(timeout)?;
        if let RuntimePrefetchChunk::Data(bytes) = &chunk {
            runtime_prefetch_release_queued_bytes(&self.shared, bytes.len());
        }
        Ok(chunk)
    }

    fn push_backlog(&mut self, chunk: RuntimePrefetchChunk) {
        self.backlog.push_back(chunk);
    }

    fn into_reader(mut self, prelude: Vec<u8>) -> RuntimePrefetchReader {
        RuntimePrefetchReader {
            receiver: self
                .receiver
                .take()
                .expect("runtime prefetch receiver should remain available"),
            shared: Arc::clone(&self.shared),
            backlog: std::mem::take(&mut self.backlog),
            pending: Cursor::new(prelude),
            finished: false,
            worker_abort: self
                .worker_abort
                .take()
                .expect("runtime prefetch abort handle should remain available"),
        }
    }
}

impl Drop for RuntimePrefetchStream {
    fn drop(&mut self) {
        if let Some(worker_abort) = self.worker_abort.take() {
            worker_abort.abort();
        }
    }
}

pub(super) fn runtime_prefetch_set_terminal_error(
    shared: &RuntimePrefetchSharedState,
    kind: io::ErrorKind,
    message: impl Into<String>,
) {
    let mut terminal_error = shared
        .terminal_error
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if terminal_error.is_none() {
        *terminal_error = Some((kind, message.into()));
    }
}

pub(super) fn runtime_prefetch_terminal_error(
    shared: &RuntimePrefetchSharedState,
) -> Option<(io::ErrorKind, String)> {
    shared
        .terminal_error
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(super) fn runtime_prefetch_release_queued_bytes(
    shared: &RuntimePrefetchSharedState,
    bytes: usize,
) {
    if bytes > 0 {
        shared.queued_bytes.fetch_sub(bytes, Ordering::SeqCst);
    }
}

pub(super) async fn runtime_prefetch_send_with_wait(
    sender: &SyncSender<RuntimePrefetchChunk>,
    shared: &RuntimePrefetchSharedState,
    chunk: Vec<u8>,
) -> RuntimePrefetchSendOutcome {
    let started_at = Instant::now();
    let retry_delay = Duration::from_millis(runtime_proxy_prefetch_backpressure_retry_ms());
    let timeout = Duration::from_millis(runtime_proxy_prefetch_backpressure_timeout_ms());
    let buffered_limit = runtime_proxy_prefetch_max_buffered_bytes().max(1);
    let mut pending = RuntimePrefetchChunk::Data(chunk);
    let mut retries = 0usize;
    loop {
        let chunk_bytes = match &pending {
            RuntimePrefetchChunk::Data(bytes) => bytes.len(),
            RuntimePrefetchChunk::End | RuntimePrefetchChunk::Error(_, _) => 0,
        };
        let queued_bytes = shared.queued_bytes.load(Ordering::SeqCst);
        if queued_bytes.saturating_add(chunk_bytes) > buffered_limit {
            if started_at.elapsed() >= timeout {
                return RuntimePrefetchSendOutcome::TimedOut {
                    message: format!(
                        "runtime prefetch buffered bytes exceeded safe limit ({} > {})",
                        queued_bytes.saturating_add(chunk_bytes),
                        buffered_limit
                    ),
                };
            }
            retries = retries.saturating_add(1);
            let remaining = timeout.saturating_sub(started_at.elapsed());
            let sleep_for = retry_delay.min(remaining);
            if !sleep_for.is_zero() {
                tokio::time::sleep(sleep_for).await;
            }
            continue;
        }
        match sender.try_send(pending) {
            Ok(()) => {
                if chunk_bytes > 0 {
                    shared.queued_bytes.fetch_add(chunk_bytes, Ordering::SeqCst);
                }
                return RuntimePrefetchSendOutcome::Sent {
                    wait_ms: started_at.elapsed().as_millis(),
                    retries,
                };
            }
            Err(TrySendError::Disconnected(_)) => {
                return RuntimePrefetchSendOutcome::Disconnected;
            }
            Err(TrySendError::Full(returned)) => {
                if started_at.elapsed() >= timeout {
                    return RuntimePrefetchSendOutcome::TimedOut {
                        message: format!(
                            "runtime prefetch backlog exceeded bounded capacity ({})",
                            RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY
                        ),
                    };
                }
                pending = returned;
                retries = retries.saturating_add(1);
                let remaining = timeout.saturating_sub(started_at.elapsed());
                let sleep_for = retry_delay.min(remaining);
                if !sleep_for.is_zero() {
                    tokio::time::sleep(sleep_for).await;
                }
            }
        }
    }
}

pub(super) async fn runtime_prefetch_response_chunks(
    mut response: reqwest::Response,
    sender: SyncSender<RuntimePrefetchChunk>,
    shared: Arc<RuntimePrefetchSharedState>,
    log_path: PathBuf,
    request_id: u64,
) {
    let mut saw_data = false;
    loop {
        match response.chunk().await {
            Ok(None) => {
                runtime_proxy_log_to_path(
                    &log_path,
                    &format!(
                        "request={request_id} transport=http upstream_stream_end saw_data={saw_data}"
                    ),
                );
                let _ = sender.try_send(RuntimePrefetchChunk::End);
                break;
            }
            Ok(Some(chunk)) => {
                if !saw_data {
                    saw_data = true;
                    runtime_proxy_log_to_path(
                        &log_path,
                        &format!(
                            "request={request_id} transport=http first_upstream_chunk bytes={}",
                            chunk.len()
                        ),
                    );
                }
                if chunk.len() > RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES {
                    let message = format!(
                        "runtime upstream chunk exceeded prefetch limit ({} > {})",
                        chunk.len(),
                        RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES
                    );
                    runtime_prefetch_set_terminal_error(
                        &shared,
                        io::ErrorKind::InvalidData,
                        message.clone(),
                    );
                    runtime_proxy_log_to_path(
                        &log_path,
                        &format!(
                            "request={request_id} transport=http prefetch_chunk_too_large bytes={} limit={} error={message}",
                            chunk.len(),
                            RUNTIME_PROXY_PREFETCH_MAX_CHUNK_BYTES,
                        ),
                    );
                    let _ = sender.try_send(RuntimePrefetchChunk::Error(
                        io::ErrorKind::InvalidData,
                        message,
                    ));
                    break;
                }
                let chunk_bytes = chunk.len();
                match runtime_prefetch_send_with_wait(&sender, &shared, chunk.to_vec()).await {
                    RuntimePrefetchSendOutcome::Sent { wait_ms, retries } => {
                        if retries > 0 {
                            runtime_proxy_log_to_path(
                                &log_path,
                                &format!(
                                    "request={request_id} transport=http prefetch_backpressure_recovered bytes={chunk_bytes} retries={retries} wait_ms={wait_ms}",
                                ),
                            );
                        }
                    }
                    RuntimePrefetchSendOutcome::TimedOut { message } => {
                        runtime_prefetch_set_terminal_error(
                            &shared,
                            io::ErrorKind::WouldBlock,
                            message.clone(),
                        );
                        runtime_proxy_log_to_path(
                            &log_path,
                            &format!(
                                "request={request_id} transport=http prefetch_backpressure_timeout bytes={chunk_bytes} capacity={} error={message}",
                                RUNTIME_PROXY_PREFETCH_QUEUE_CAPACITY,
                            ),
                        );
                        break;
                    }
                    RuntimePrefetchSendOutcome::Disconnected => {
                        runtime_proxy_log_to_path(
                            &log_path,
                            &format!(
                                "request={request_id} transport=http prefetch_receiver_disconnected"
                            ),
                        );
                        break;
                    }
                }
            }
            Err(err) => {
                let kind = runtime_reqwest_error_kind(&err);
                runtime_prefetch_set_terminal_error(&shared, kind, err.to_string());
                runtime_proxy_log_to_path(
                    &log_path,
                    &format!(
                        "request={request_id} transport=http upstream_stream_error kind={kind:?} error={err}"
                    ),
                );
                let _ = sender.try_send(RuntimePrefetchChunk::Error(kind, err.to_string()));
                break;
            }
        }
    }
}

pub(super) fn inspect_runtime_sse_lookahead(
    prefetch: &mut RuntimePrefetchStream,
    log_path: &Path,
    request_id: u64,
) -> Result<RuntimeSseInspection> {
    let deadline = Instant::now() + Duration::from_millis(runtime_proxy_sse_lookahead_timeout_ms());
    let mut buffered = Vec::new();

    loop {
        if buffered.len() >= RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES {
            break;
        }
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline.saturating_duration_since(now);
        match prefetch.recv_timeout(remaining) {
            Ok(RuntimePrefetchChunk::Data(chunk)) => {
                buffered.extend_from_slice(&chunk);
                match inspect_runtime_sse_buffer(&buffered)? {
                    RuntimeSseInspectionProgress::Commit { response_ids } => {
                        runtime_proxy_log_to_path(
                            log_path,
                            &format!(
                                "request={request_id} transport=http lookahead_commit bytes={} response_ids={}",
                                buffered.len(),
                                response_ids.len()
                            ),
                        );
                        return Ok(RuntimeSseInspection::Commit {
                            prelude: buffered,
                            response_ids,
                        });
                    }
                    RuntimeSseInspectionProgress::Hold { .. } => {}
                    RuntimeSseInspectionProgress::QuotaBlocked => {
                        runtime_proxy_log_to_path(
                            log_path,
                            &format!(
                                "request={request_id} transport=http lookahead_retryable_signal bytes={}",
                                buffered.len()
                            ),
                        );
                        return Ok(RuntimeSseInspection::QuotaBlocked(buffered));
                    }
                    RuntimeSseInspectionProgress::PreviousResponseNotFound => {
                        runtime_proxy_log_to_path(
                            log_path,
                            &format!(
                                "request={request_id} transport=http lookahead_retryable_signal bytes={}",
                                buffered.len()
                            ),
                        );
                        return Ok(RuntimeSseInspection::PreviousResponseNotFound(buffered));
                    }
                }
            }
            Ok(RuntimePrefetchChunk::End) => break,
            Ok(RuntimePrefetchChunk::Error(kind, message)) => {
                if buffered.is_empty() {
                    runtime_proxy_log_to_path(
                        log_path,
                        &format!(
                            "request={request_id} transport=http lookahead_error_before_bytes kind={kind:?} error={message}"
                        ),
                    );
                    return Err(anyhow::Error::new(io::Error::new(kind, message))
                        .context("failed to inspect runtime auto-rotate SSE stream"));
                }
                prefetch.push_backlog(RuntimePrefetchChunk::Error(kind, message));
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_timeout bytes={}",
                        buffered.len()
                    ),
                );
                break;
            }
            Err(RecvTimeoutError::Disconnected) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_channel_disconnected bytes={}",
                        buffered.len()
                    ),
                );
                break;
            }
        }
    }

    match inspect_runtime_sse_buffer(&buffered)? {
        RuntimeSseInspectionProgress::Commit { response_ids }
        | RuntimeSseInspectionProgress::Hold { response_ids } => {
            if !buffered.is_empty() {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_budget_exhausted bytes={} response_ids={}",
                        buffered.len(),
                        response_ids.len()
                    ),
                );
            }
            Ok(RuntimeSseInspection::Commit {
                prelude: buffered,
                response_ids,
            })
        }
        RuntimeSseInspectionProgress::QuotaBlocked => {
            Ok(RuntimeSseInspection::QuotaBlocked(buffered))
        }
        RuntimeSseInspectionProgress::PreviousResponseNotFound => {
            Ok(RuntimeSseInspection::PreviousResponseNotFound(buffered))
        }
    }
}

pub(super) fn inspect_runtime_sse_buffer(buffered: &[u8]) -> Result<RuntimeSseInspectionProgress> {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut response_ids = BTreeSet::new();
    let mut saw_commit_ready_event = false;

    for byte in buffered {
        line.push(*byte);
        if *byte != b'\n' {
            continue;
        }

        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            let event = parse_runtime_sse_event(&data_lines);
            if event.quota_blocked {
                return Ok(RuntimeSseInspectionProgress::QuotaBlocked);
            }
            if event.previous_response_not_found {
                return Ok(RuntimeSseInspectionProgress::PreviousResponseNotFound);
            }
            response_ids.extend(event.response_ids);
            if !data_lines.is_empty()
                && !event
                    .event_type
                    .as_deref()
                    .is_some_and(runtime_proxy_precommit_hold_event_kind)
            {
                saw_commit_ready_event = true;
            }
            data_lines.clear();
            line.clear();
            continue;
        }

        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
        line.clear();
    }

    if saw_commit_ready_event {
        Ok(RuntimeSseInspectionProgress::Commit {
            response_ids: response_ids.into_iter().collect(),
        })
    } else {
        Ok(RuntimeSseInspectionProgress::Hold {
            response_ids: response_ids.into_iter().collect(),
        })
    }
}

pub(super) fn buffer_runtime_proxy_async_response_parts(
    shared: &RuntimeRotationProxyShared,
    mut response: reqwest::Response,
    prelude: Vec<u8>,
) -> Result<RuntimeBufferedResponseParts> {
    let status = response.status().as_u16();
    let mut headers = Vec::new();
    for (name, value) in response.headers() {
        if should_skip_runtime_response_header(name.as_str()) {
            continue;
        }
        headers.push((name.as_str().to_string(), value.as_bytes().to_vec()));
    }
    let body = shared.async_runtime.block_on(async move {
        let mut body = prelude;
        loop {
            let next = response
                .chunk()
                .await
                .context("failed to read upstream runtime response body chunk")?;
            let Some(chunk) = next else {
                break;
            };
            if body.len().saturating_add(chunk.len()) > RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES {
                return Err(anyhow::Error::new(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "runtime buffered response exceeded safe size limit ({})",
                        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES
                    ),
                )));
            }
            body.extend_from_slice(&chunk);
        }
        Ok::<Vec<u8>, anyhow::Error>(body)
    })?;
    Ok(RuntimeBufferedResponseParts {
        status,
        headers,
        body,
    })
}

pub(super) fn runtime_reqwest_error_kind(err: &reqwest::Error) -> io::ErrorKind {
    match runtime_transport_failure_kind_from_reqwest(err) {
        Some(
            RuntimeTransportFailureKind::ConnectTimeout | RuntimeTransportFailureKind::ReadTimeout,
        ) => io::ErrorKind::TimedOut,
        Some(RuntimeTransportFailureKind::ConnectRefused) => io::ErrorKind::ConnectionRefused,
        Some(RuntimeTransportFailureKind::ConnectReset) => io::ErrorKind::ConnectionReset,
        Some(RuntimeTransportFailureKind::ConnectionAborted) => io::ErrorKind::ConnectionAborted,
        Some(RuntimeTransportFailureKind::BrokenPipe) => io::ErrorKind::BrokenPipe,
        Some(RuntimeTransportFailureKind::UnexpectedEof) => io::ErrorKind::UnexpectedEof,
        _ => io::ErrorKind::Other,
    }
}

pub(super) fn build_runtime_proxy_response_from_parts(
    parts: RuntimeBufferedResponseParts,
) -> tiny_http::ResponseBox {
    let status = TinyStatusCode(parts.status);
    let headers = parts
        .headers
        .into_iter()
        .filter_map(|(name, value)| TinyHeader::from_bytes(name.as_bytes(), value).ok())
        .collect::<Vec<_>>();
    let body_len = parts.body.len();
    TinyResponse::new(
        status,
        headers,
        Box::new(Cursor::new(parts.body)),
        Some(body_len),
        None,
    )
    .boxed()
}

pub(super) fn runtime_buffered_response_content_type(
    parts: &RuntimeBufferedResponseParts,
) -> Option<&str> {
    parts.headers.iter().find_map(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            .then(|| std::str::from_utf8(value).ok())
            .flatten()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn parse_runtime_sse_payload(data_lines: &[String]) -> Option<serde_json::Value> {
    if data_lines.is_empty() {
        return None;
    }

    let payload = data_lines.join("\n");
    serde_json::from_str::<serde_json::Value>(&payload).ok()
}

pub(super) fn parse_runtime_sse_event(data_lines: &[String]) -> RuntimeParsedSseEvent {
    let Some(value) = parse_runtime_sse_payload(data_lines) else {
        return RuntimeParsedSseEvent::default();
    };

    RuntimeParsedSseEvent {
        quota_blocked: extract_runtime_proxy_quota_message_from_value(&value).is_some(),
        previous_response_not_found: extract_runtime_proxy_previous_response_message_from_value(
            &value,
        )
        .is_some(),
        response_ids: extract_runtime_response_ids_from_value(&value),
        event_type: runtime_response_event_type_from_value(&value),
    }
}

pub(super) fn extract_runtime_response_ids_from_sse(data_lines: &[String]) -> Vec<String> {
    parse_runtime_sse_payload(data_lines)
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub(super) fn extract_runtime_proxy_quota_message(body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = extract_runtime_proxy_quota_message_from_value(&value)
    {
        return Some(message);
    }

    extract_runtime_proxy_quota_message_from_text(&String::from_utf8_lossy(body))
}

pub(super) fn extract_runtime_proxy_quota_message_from_response_reply(
    response: &RuntimeResponsesReply,
) -> Option<String> {
    match response {
        RuntimeResponsesReply::Buffered(parts) => extract_runtime_proxy_quota_message(&parts.body),
        RuntimeResponsesReply::Streaming(_) => None,
    }
}

pub(super) fn extract_runtime_proxy_quota_message_from_websocket_payload(
    payload: &RuntimeWebsocketErrorPayload,
) -> Option<String> {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => {
            extract_runtime_proxy_quota_message_from_text(text)
        }
        RuntimeWebsocketErrorPayload::Binary(bytes) => extract_runtime_proxy_quota_message(bytes),
        RuntimeWebsocketErrorPayload::Empty => None,
    }
}

pub(super) fn extract_runtime_proxy_overload_message_from_websocket_payload(
    payload: &RuntimeWebsocketErrorPayload,
) -> Option<String> {
    match payload {
        RuntimeWebsocketErrorPayload::Text(text) => {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(text)
                && let Some(message) = extract_runtime_proxy_overload_message_from_value(&value)
            {
                return Some(message);
            }
            extract_runtime_proxy_overload_message_from_text(text)
        }
        RuntimeWebsocketErrorPayload::Binary(bytes) => {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes)
                && let Some(message) = extract_runtime_proxy_overload_message_from_value(&value)
            {
                return Some(message);
            }
            extract_runtime_proxy_overload_message_from_text(&String::from_utf8_lossy(bytes))
        }
        RuntimeWebsocketErrorPayload::Empty => None,
    }
}

pub(super) fn extract_runtime_proxy_previous_response_message(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|value| extract_runtime_proxy_previous_response_message_from_value(&value))
}

pub(super) fn extract_runtime_proxy_overload_message(status: u16, body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body)
        && let Some(message) = extract_runtime_proxy_overload_message_from_value(&value)
    {
        return Some(message);
    }

    let body_text = String::from_utf8_lossy(body).trim().to_string();
    if matches!(status, 429 | 500 | 502 | 503 | 504 | 529)
        && let Some(message) = extract_runtime_proxy_overload_message_from_text(&body_text)
    {
        return Some(message);
    }

    (status == 500).then(|| {
        if body_text.is_empty() {
            "Upstream Codex backend is currently experiencing high demand.".to_string()
        } else {
            body_text
        }
    })
}

pub(super) fn extract_runtime_proxy_overload_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    let direct_error = value.get("error");
    let response_error = value
        .get("response")
        .and_then(|response| response.get("error"));
    for error in [direct_error, response_error].into_iter().flatten() {
        let code = error.get("code").and_then(serde_json::Value::as_str);
        let message = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .or_else(|| error.get("detail").and_then(serde_json::Value::as_str));
        if matches!(code, Some("server_is_overloaded" | "slow_down")) {
            return Some(
                message
                    .unwrap_or("Upstream Codex backend is currently overloaded.")
                    .to_string(),
            );
        }
        if let Some(message) = message.filter(|message| runtime_proxy_overload_message(message)) {
            return Some(message.to_string());
        }
    }

    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(extract_runtime_proxy_overload_message_from_value),
        serde_json::Value::Object(map) => map
            .values()
            .find_map(extract_runtime_proxy_overload_message_from_value),
        _ => None,
    }
}

pub(super) fn extract_runtime_proxy_overload_message_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    runtime_proxy_overload_message(trimmed).then(|| trimmed.to_string())
}

pub(super) fn extract_runtime_proxy_quota_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    if let Some(message) = extract_runtime_proxy_quota_message_candidate(value) {
        return Some(message);
    }

    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(extract_runtime_proxy_quota_message_from_value),
        serde_json::Value::Object(map) => map
            .values()
            .find_map(extract_runtime_proxy_quota_message_from_value),
        _ => None,
    }
}

pub(super) fn extract_runtime_proxy_quota_message_candidate(
    value: &serde_json::Value,
) -> Option<String> {
    match value {
        serde_json::Value::String(message) => {
            runtime_proxy_usage_limit_message(message).then(|| message.to_string())
        }
        serde_json::Value::Object(map) => {
            let message = map
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| map.get("detail").and_then(serde_json::Value::as_str))
                .or_else(|| map.get("error").and_then(serde_json::Value::as_str));
            let code = map.get("code").and_then(serde_json::Value::as_str);
            let error_type = map.get("type").and_then(serde_json::Value::as_str);
            let code_matches = matches!(code, Some("insufficient_quota" | "rate_limit_exceeded"));
            let type_matches = matches!(error_type, Some("usage_limit_reached"));
            let message_matches = message.is_some_and(runtime_proxy_usage_limit_message);
            if !(code_matches || type_matches || message_matches) {
                return None;
            }

            Some(
                message
                    .unwrap_or("Upstream Codex account quota was exhausted.")
                    .to_string(),
            )
        }
        _ => None,
    }
}

pub(super) fn extract_runtime_proxy_quota_message_from_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if runtime_proxy_usage_limit_message(trimmed)
        || lower.contains("usage_limit_reached")
        || lower.contains("insufficient_quota")
        || lower.contains("rate_limit_exceeded")
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

pub(super) fn runtime_proxy_usage_limit_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("you've hit your usage limit")
        || lower.contains("you have hit your usage limit")
        || lower.contains("the usage limit has been reached")
        || lower.contains("usage limit has been reached")
        || lower.contains("usage limit")
            && (lower.contains("try again at")
                || lower.contains("request to your admin")
                || lower.contains("more access now"))
}

pub(super) fn runtime_proxy_overload_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("selected model is at capacity")
        || (lower.contains("model is at capacity")
            && (lower.contains("try a different model") || lower.contains("please try again")))
        || lower.contains("backend under high demand")
        || lower.contains("experiencing high demand")
        || lower.contains("server is overloaded")
        || lower.contains("currently overloaded")
}

pub(super) fn runtime_proxy_body_snippet(body: &[u8], max_chars: usize) -> String {
    let normalized = String::from_utf8_lossy(body)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return "-".to_string();
    }

    let snippet = normalized.chars().take(max_chars).collect::<String>();
    if normalized.chars().count() > max_chars {
        format!("{snippet}...")
    } else {
        snippet
    }
}

pub(super) fn extract_runtime_proxy_previous_response_message_from_value(
    value: &serde_json::Value,
) -> Option<String> {
    let direct_error = value.get("error");
    let response_error = value
        .get("response")
        .and_then(|response| response.get("error"));
    for error in [direct_error, response_error].into_iter().flatten() {
        let code = error.get("code").and_then(serde_json::Value::as_str)?;
        if code != "previous_response_not_found" {
            continue;
        }
        return Some(
            error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Previous response could not be found on the selected Codex account.")
                .to_string(),
        );
    }
    None
}

#[cfg(test)]
pub(super) fn extract_runtime_response_ids_from_payload(payload: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub(super) fn extract_runtime_response_ids_from_body_bytes(body: &[u8]) -> Vec<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .map(|value| extract_runtime_response_ids_from_value(&value))
        .unwrap_or_default()
}

pub(super) fn push_runtime_response_id(response_ids: &mut Vec<String>, id: Option<&str>) {
    if let Some(id) = id
        && !response_ids.iter().any(|existing| existing == id)
    {
        response_ids.push(id.to_string());
    }
}

pub(super) fn extract_runtime_response_ids_from_value(value: &serde_json::Value) -> Vec<String> {
    let mut response_ids = Vec::new();

    push_runtime_response_id(
        &mut response_ids,
        value
            .get("response")
            .and_then(|response| response.get("id"))
            .and_then(serde_json::Value::as_str),
    );
    push_runtime_response_id(
        &mut response_ids,
        value.get("response_id").and_then(serde_json::Value::as_str),
    );

    if value
        .get("object")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|object| object == "response" || object.ends_with(".response"))
    {
        push_runtime_response_id(
            &mut response_ids,
            value.get("id").and_then(serde_json::Value::as_str),
        );
    }

    response_ids
}

pub(super) fn extract_runtime_turn_state_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("response")
        .and_then(|response| response.get("headers"))
        .and_then(extract_runtime_turn_state_from_headers_value)
        .or_else(|| {
            value
                .get("headers")
                .and_then(extract_runtime_turn_state_from_headers_value)
        })
}

pub(super) fn extract_runtime_turn_state_from_headers_value(
    value: &serde_json::Value,
) -> Option<String> {
    let headers = value.as_object()?;
    headers.iter().find_map(|(name, value)| {
        if name.eq_ignore_ascii_case("x-codex-turn-state") {
            match value {
                serde_json::Value::String(value) => Some(value.clone()),
                serde_json::Value::Array(items) => items.iter().find_map(|item| match item {
                    serde_json::Value::String(value) => Some(value.clone()),
                    _ => None,
                }),
                _ => None,
            }
        } else {
            None
        }
    })
}
