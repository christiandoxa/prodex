use super::*;

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

pub(crate) fn audit_runtime_proxy_startup_state(shared: &RuntimeRotationProxyShared) {
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

#[allow(dead_code)]
pub(crate) fn try_acquire_runtime_proxy_active_request_slot(
    shared: &RuntimeRotationProxyShared,
    transport: &str,
    path: &str,
) -> Result<RuntimeProxyActiveRequestGuard, RuntimeProxyAdmissionRejection> {
    match probe_runtime_proxy_active_request_slot(shared, transport, path) {
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
                shared
                    .lane_admission
                    .admissions_total_counter(lane)
                    .fetch_add(1, Ordering::Relaxed);
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
    let started_at = Instant::now();
    let pressure_mode =
        runtime_proxy_pressure_mode_active_for_request_path(shared, path, transport == "websocket");
    let budget = runtime_proxy_admission_wait_budget(path, pressure_mode);
    let mut waited = false;
    loop {
        match probe_runtime_proxy_active_request_slot(shared, transport, path) {
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
                if let Ok(guard) = probe_runtime_proxy_active_request_slot(shared, transport, path)
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

pub(crate) fn runtime_profile_inflight_release_revision(
    shared: &RuntimeRotationProxyShared,
) -> u64 {
    shared
        .lane_admission
        .inflight_release_revision
        .load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeProfileInFlightWaitOutcome {
    InflightRelease,
    OtherNotify,
    Timeout,
}

pub(crate) fn runtime_profile_wait_outcome_label(
    outcome: RuntimeProfileInFlightWaitOutcome,
) -> &'static str {
    match outcome {
        RuntimeProfileInFlightWaitOutcome::InflightRelease => "release",
        RuntimeProfileInFlightWaitOutcome::OtherNotify => "other_notify",
        RuntimeProfileInFlightWaitOutcome::Timeout => "timeout",
    }
}

pub(crate) fn runtime_profile_inflight_wait_outcome_label(
    outcome: RuntimeProfileInFlightWaitOutcome,
) -> &'static str {
    match outcome {
        RuntimeProfileInFlightWaitOutcome::InflightRelease => "inflight_release",
        RuntimeProfileInFlightWaitOutcome::OtherNotify => "other_notify",
        RuntimeProfileInFlightWaitOutcome::Timeout => "timeout",
    }
}

pub(crate) fn runtime_profile_inflight_wait_outcome_since(
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

pub(crate) fn runtime_probe_refresh_wait_outcome_since(
    timeout: Duration,
    observed_revision: u64,
) -> RuntimeProfileInFlightWaitOutcome {
    if timeout.is_zero() {
        return RuntimeProfileInFlightWaitOutcome::Timeout;
    }
    if runtime_probe_refresh_revision() != observed_revision {
        return RuntimeProfileInFlightWaitOutcome::InflightRelease;
    }
    let queue = runtime_probe_refresh_queue();
    let (mutex, condvar) = &*queue.wait;
    let guard = mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if runtime_probe_refresh_revision() != observed_revision {
        return RuntimeProfileInFlightWaitOutcome::InflightRelease;
    }
    let (_guard, result) = condvar
        .wait_timeout(guard, timeout)
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if runtime_probe_refresh_revision() != observed_revision {
        RuntimeProfileInFlightWaitOutcome::InflightRelease
    } else if result.timed_out() {
        RuntimeProfileInFlightWaitOutcome::Timeout
    } else {
        RuntimeProfileInFlightWaitOutcome::OtherNotify
    }
}

#[cfg(test)]
pub(crate) fn wait_for_runtime_profile_inflight_relief_since(
    shared: &RuntimeRotationProxyShared,
    timeout: Duration,
    observed_revision: u64,
) -> bool {
    matches!(
        runtime_profile_inflight_wait_outcome_since(shared, timeout, observed_revision),
        RuntimeProfileInFlightWaitOutcome::InflightRelease
    )
}

#[cfg(test)]
pub(crate) fn wait_for_runtime_probe_refresh_since(
    timeout: Duration,
    observed_revision: u64,
) -> bool {
    matches!(
        runtime_probe_refresh_wait_outcome_since(timeout, observed_revision),
        RuntimeProfileInFlightWaitOutcome::InflightRelease
    )
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
