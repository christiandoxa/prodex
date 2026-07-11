use super::*;

#[path = "lifecycle/audit.rs"]
mod audit;
#[path = "lifecycle/drain.rs"]
mod drain;
#[path = "lifecycle/wait.rs"]
mod wait;
pub(crate) use audit::audit_runtime_proxy_startup_state;
pub(crate) use wait::{
    RuntimeProfileInFlightWaitOutcome, runtime_probe_refresh_wait_outcome_since,
    runtime_profile_inflight_release_revision, runtime_profile_inflight_wait_outcome_label,
    runtime_profile_inflight_wait_outcome_since, runtime_profile_wait_outcome_label,
};
#[cfg(test)]
pub(crate) use wait::{
    wait_for_runtime_probe_refresh_since, wait_for_runtime_profile_inflight_relief_since,
};

impl Drop for RuntimeRotationProxy {
    fn drop(&mut self) {
        clear_runtime_proxy_continuity_failure_reason_metrics(&self.log_path);
        unregister_runtime_proxy_persistence_mode(&self.log_path);
        unregister_runtime_broker_metadata(&self.log_path);
        self.shutdown.store(true, Ordering::SeqCst);
        for _ in 0..self.accept_worker_count {
            self.server.unblock();
        }
        if !runtime_proxy_join_workers_on_drop() {
            let detached_workers = self.worker_threads.len();
            runtime_proxy_log_to_path(
                &self.log_path,
                &format!(
                    "runtime proxy shutdown detach_workers={detached_workers} active_requests={}",
                    self.active_request_count.load(Ordering::SeqCst)
                ),
            );
            runtime_proxy_flush_logs_for_path(&self.log_path);
            self.worker_threads.clear();
            let _ = self.owner_lock.take();
            return;
        }
        while let Some(worker) = self.worker_threads.pop() {
            let _ = worker.join();
        }
        runtime_proxy_flush_logs_for_path(&self.log_path);
        let _ = self.owner_lock.take();
    }
}

fn runtime_proxy_join_workers_on_drop() -> bool {
    cfg!(test)
}

#[allow(dead_code)]
pub(crate) fn try_acquire_runtime_proxy_active_request_slot(
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
) -> Result<RuntimeProxyActiveRequestGuard, RuntimeProxyAdmissionRejection> {
    match probe_runtime_proxy_active_request_slot(shared, transport, path, None) {
        Ok(guard) => Ok(guard),
        Err(rejection) => {
            record_runtime_proxy_admission_rejection(shared, transport, path, rejection);
            Err(rejection)
        }
    }
}

fn probe_runtime_proxy_active_request_slot(
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
    request: Option<&RuntimeProxyRequest>,
) -> Result<RuntimeProxyActiveRequestGuard, RuntimeProxyAdmissionRejection> {
    let lane = runtime_proxy_request_lane(path, transport == "websocket");
    let lane_active_count = shared.lane_admission.active_counter(lane);
    let lane_limit = shared.lane_admission.limit(lane);
    let bypass_owned_affinity_lane_limit = request.is_some_and(|request| {
        runtime_proxy_request_has_owned_lane_affinity(shared, lane, request)
    });
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
        let bypass_lane_limit = lane == RuntimeRouteKind::Standard
            && runtime_proxy_startup_standard_lane_priority_path(path)
            || bypass_owned_affinity_lane_limit;
        if lane_active >= lane_limit && !bypass_lane_limit {
            runtime_proxy_log(
                shared,
                format!(
                    "runtime_proxy_lane_limit_reached transport={transport} path={path} lane={} active={lane_active} limit={lane_limit}",
                    runtime_route_kind_label(lane)
                ),
            );
            return Err(RuntimeProxyAdmissionRejection::LaneLimit(lane));
        }
        if lane_active >= lane_limit && bypass_owned_affinity_lane_limit {
            runtime_proxy_log(
                shared,
                format!(
                    "runtime_proxy_lane_limit_bypassed_affinity transport={transport} path={path} lane={} active={lane_active} limit={lane_limit}",
                    runtime_route_kind_label(lane)
                ),
            );
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
                shared
                    .lane_admission
                    .admissions_total_counter(lane)
                    .fetch_add(1, Ordering::Relaxed);
                return Ok(RuntimeProxyActiveRequestGuard {
                    active_request_count: Arc::clone(&shared.active_request_count),
                    lane_active_count,
                    lane_releases_total: shared.lane_admission.releases_total_counter(lane),
                    active_request_release_underflows_total: Arc::clone(
                        &shared
                            .lane_admission
                            .active_request_release_underflows_total,
                    ),
                    lane_release_underflows_total: shared
                        .lane_admission
                        .release_underflows_total_counter(lane),
                    wait: Arc::clone(&shared.lane_admission.wait),
                });
            }
            shared.active_request_count.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn runtime_proxy_request_has_owned_lane_affinity(
    shared: &RuntimeRotationProxyShared,
    lane: RuntimeRouteKind,
    request: &RuntimeProxyRequest,
) -> bool {
    let Ok(runtime) = shared.runtime.lock() else {
        return false;
    };
    let profile_exists = |profile_name: &str| runtime.state.profiles.contains_key(profile_name);
    let binding_profile_exists =
        |binding: &ResponseProfileBinding| profile_exists(binding.profile_name.as_str());

    if lane == RuntimeRouteKind::Responses {
        if runtime_request_previous_response_id(request)
            .as_deref()
            .and_then(|response_id| runtime.state.response_profile_bindings.get(response_id))
            .is_some_and(binding_profile_exists)
        {
            return true;
        }
        if runtime_request_turn_state(request)
            .as_deref()
            .and_then(|turn_state| runtime.turn_state_bindings.get(turn_state))
            .is_some_and(binding_profile_exists)
        {
            return true;
        }
    }

    if lane == RuntimeRouteKind::Compact
        && let Some(session_id) = runtime_request_session_id(request)
    {
        return runtime
            .session_id_bindings
            .get(session_id.as_str())
            .or_else(|| {
                runtime
                    .state
                    .session_profile_bindings
                    .get(session_id.as_str())
            })
            .is_some_and(binding_profile_exists);
    }

    false
}

fn record_runtime_proxy_admission_rejection(
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
    rejection: RuntimeProxyAdmissionRejection,
) {
    let lane = match rejection {
        RuntimeProxyAdmissionRejection::GlobalLimit => {
            runtime_proxy_request_lane(path, transport == "websocket")
        }
        RuntimeProxyAdmissionRejection::LaneLimit(lane) => lane,
    };
    match rejection {
        RuntimeProxyAdmissionRejection::GlobalLimit => shared
            .lane_admission
            .global_limit_rejections_total_counter(lane)
            .fetch_add(1, Ordering::Relaxed),
        RuntimeProxyAdmissionRejection::LaneLimit(lane) => shared
            .lane_admission
            .lane_limit_rejections_total_counter(lane)
            .fetch_add(1, Ordering::Relaxed),
    };
}

pub(crate) fn acquire_runtime_proxy_active_request_slot_with_wait(
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
) -> Result<RuntimeProxyActiveRequestGuard, RuntimeProxyAdmissionRejection> {
    acquire_runtime_proxy_active_request_slot_with_wait_for_request(shared, transport, path, None)
}

pub(crate) fn acquire_runtime_proxy_active_request_slot_with_wait_for_request(
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
    request: Option<&RuntimeProxyRequest>,
) -> Result<RuntimeProxyActiveRequestGuard, RuntimeProxyAdmissionRejection> {
    let started_at = Instant::now();
    let pressure_mode =
        runtime_proxy_pressure_mode_active_for_request_path(shared, path, transport == "websocket");
    let budget = runtime_proxy_admission_wait_budget_with_config(
        path,
        pressure_mode,
        &shared.runtime_config,
    );
    let mut waited = false;
    loop {
        match probe_runtime_proxy_active_request_slot(shared, transport, path, request) {
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
                    record_runtime_proxy_admission_rejection(shared, transport, path, rejection);
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
                    probe_runtime_proxy_active_request_slot(shared, transport, path, request)
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

pub(crate) fn wait_for_runtime_proxy_queue_capacity<T, F>(
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
    let budget = runtime_proxy_long_lived_queue_wait_budget_with_config(
        path,
        pressure_mode,
        &shared.runtime_config,
    );
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
                            runtime_proxy_structured_log_message(
                                "runtime_proxy_queue_wait_exhausted",
                                [
                                    runtime_proxy_log_field("transport", transport),
                                    runtime_proxy_log_field("path", runtime_proxy_log_url(path)),
                                    runtime_proxy_log_field(
                                        "waited_ms",
                                        started_at.elapsed().as_millis().to_string(),
                                    ),
                                    runtime_proxy_log_field(
                                        "reason",
                                        "long_lived_queue_disconnected",
                                    ),
                                ],
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
                    runtime_proxy_structured_log_message(
                        "runtime_proxy_queue_wait_exhausted",
                        [
                            runtime_proxy_log_field("transport", transport),
                            runtime_proxy_log_field("path", runtime_proxy_log_url(path)),
                            runtime_proxy_log_field(
                                "waited_ms",
                                started_at.elapsed().as_millis().to_string(),
                            ),
                            runtime_proxy_log_field("reason", "long_lived_queue_disconnected"),
                        ],
                    ),
                );
                return Err((RuntimeProxyQueueRejection::Disconnected, returned_item));
            }
        }
    }
}

#[allow(clippy::result_large_err)]
pub(crate) fn enqueue_runtime_proxy_long_lived_request_with_wait(
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
