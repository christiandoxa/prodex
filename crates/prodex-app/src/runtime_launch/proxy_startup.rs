use super::*;

#[cfg(test)]
pub(crate) fn start_runtime_rotation_proxy(
    paths: &AppPaths,
    state: &AppState,
    current_profile: &str,
    upstream_base_url: String,
    include_code_review: bool,
) -> Result<RuntimeRotationProxy> {
    start_runtime_rotation_proxy_with_listen_addr(
        paths,
        state,
        current_profile,
        upstream_base_url,
        include_code_review,
        false,
        None,
    )
}

pub(crate) fn start_runtime_rotation_proxy_with_listen_addr(
    paths: &AppPaths,
    state: &AppState,
    current_profile: &str,
    upstream_base_url: String,
    include_code_review: bool,
    upstream_no_proxy: bool,
    preferred_listen_addr: Option<&str>,
) -> Result<RuntimeRotationProxy> {
    let log_path = initialize_runtime_proxy_log_path();
    let (server, listen_addr) = match preferred_listen_addr {
        Some(preferred) => match TinyServer::http(preferred) {
            Ok(server) => {
                let server = Arc::new(server);
                let listen_addr = server.server_addr().to_ip().with_context(|| {
                    format!(
                        "runtime auto-rotate proxy did not expose a TCP listen address after binding {preferred}"
                    )
                })?;
                (server, listen_addr)
            }
            Err(err) => {
                runtime_proxy_log_to_path(
                    &log_path,
                    &format!(
                        "runtime proxy preferred_listen_addr_unavailable requested={preferred} error={err}"
                    ),
                );
                let server = Arc::new(TinyServer::http("127.0.0.1:0").map_err(|fallback_err| {
                    anyhow::anyhow!(
                        "failed to bind runtime auto-rotate proxy on {preferred}: {err}; fallback bind also failed: {fallback_err}"
                    )
                })?);
                let listen_addr = server.server_addr().to_ip().context(
                    "runtime auto-rotate proxy did not expose a TCP listen address after fallback bind",
                )?;
                (server, listen_addr)
            }
        },
        None => {
            let server = Arc::new(TinyServer::http("127.0.0.1:0").map_err(|err| {
                anyhow::anyhow!("failed to bind runtime auto-rotate proxy: {err}")
            })?);
            let listen_addr = server
                .server_addr()
                .to_ip()
                .context("runtime auto-rotate proxy did not expose a TCP listen address")?;
            (server, listen_addr)
        }
    };
    let owner_lock = try_acquire_runtime_owner_lock(paths)?;
    let persistence_enabled = owner_lock.is_some();
    let async_worker_count = runtime_proxy_async_worker_count();
    let async_runtime = Arc::new(
        TokioRuntimeBuilder::new_multi_thread()
            .worker_threads(async_worker_count)
            .enable_all()
            .build()
            .context("failed to build runtime auto-rotate async runtime")?,
    );
    let worker_count = runtime_proxy_worker_count();
    let long_lived_worker_count = runtime_proxy_long_lived_worker_count();
    let long_lived_queue_capacity =
        runtime_proxy_long_lived_queue_capacity(long_lived_worker_count);
    let active_request_limit =
        runtime_proxy_active_request_limit(worker_count, long_lived_worker_count);
    let lane_admission = RuntimeProxyLaneAdmission::new(runtime_proxy_lane_limits(
        active_request_limit,
        worker_count,
        long_lived_worker_count,
    ));
    let persisted_state = AppState::load_with_recovery(paths).unwrap_or(RecoveredLoad {
        value: state.clone(),
        recovered_from_backup: false,
    });
    let mut restored_state = merge_runtime_state_snapshot(state.clone(), &persisted_state.value);
    let persisted_continuations =
        load_runtime_continuations_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: RuntimeContinuationStore::default(),
                recovered_from_backup: false,
            },
        );
    let continuation_journal =
        load_runtime_continuation_journal_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: RuntimeContinuationJournal::default(),
                recovered_from_backup: false,
            },
        );
    let fallback_continuations = runtime_continuation_store_from_app_state(&restored_state);
    let restored_continuations = merge_runtime_continuation_store(
        &merge_runtime_continuation_store(
            &fallback_continuations,
            &persisted_continuations.value,
            &restored_state.profiles,
        ),
        &continuation_journal.value.continuations,
        &restored_state.profiles,
    );
    let continuation_sidecar_present = runtime_continuations_file_path(paths).exists()
        || runtime_continuations_last_good_file_path(paths).exists();
    let continuation_migration_needed = !continuation_sidecar_present
        && (restored_continuations != RuntimeContinuationStore::default());
    let restored_session_id_bindings = merge_profile_bindings(
        &restored_continuations.session_profile_bindings,
        &runtime_external_session_id_bindings(&restored_continuations.session_id_bindings),
        &restored_state.profiles,
    );
    let restored_runtime_session_id_bindings = merge_profile_bindings(
        &restored_continuations.session_id_bindings,
        &restored_continuations.session_profile_bindings,
        &restored_state.profiles,
    );
    restored_state.response_profile_bindings =
        restored_continuations.response_profile_bindings.clone();
    restored_state.session_profile_bindings = restored_session_id_bindings.clone();
    let persisted_profile_scores =
        load_runtime_profile_scores_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: BTreeMap::new(),
                recovered_from_backup: false,
            },
        );
    let persisted_usage_snapshots =
        load_runtime_usage_snapshots_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: BTreeMap::new(),
                recovered_from_backup: false,
            },
        );
    let mut persisted_backoffs =
        load_runtime_profile_backoffs_with_recovery(paths, &restored_state.profiles).unwrap_or(
            RecoveredLoad {
                value: RuntimeProfileBackoffs::default(),
                recovered_from_backup: false,
            },
        );
    let startup_now = Local::now().timestamp();
    let persisted_backoffs_softened = runtime_soften_persisted_backoffs_for_startup(
        &mut persisted_backoffs.value,
        &persisted_profile_scores.value,
        startup_now,
    );
    let persisted_profile_scores_count = persisted_profile_scores.value.len();
    let persisted_usage_snapshots_count = persisted_usage_snapshots.value.len();
    let persisted_response_binding_count = runtime_external_response_profile_bindings(
        &restored_continuations.response_profile_bindings,
    )
    .len();
    let persisted_session_binding_count = restored_continuations.session_profile_bindings.len();
    let persisted_turn_state_binding_count = restored_continuations.turn_state_bindings.len();
    let persisted_session_id_binding_count = restored_runtime_session_id_bindings.len();
    let persisted_retry_backoffs_count = persisted_backoffs.value.retry_backoff_until.len();
    let persisted_transport_backoffs_count = persisted_backoffs.value.transport_backoff_until.len();
    let persisted_route_circuit_count = persisted_backoffs.value.route_circuit_open_until.len();
    let expired_usage_snapshot_count = persisted_usage_snapshots
        .value
        .values()
        .filter(|snapshot| !runtime_usage_snapshot_is_usable(snapshot, startup_now))
        .count();
    let restored_global_scores_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| !key.starts_with("__route_"))
        .count();
    let restored_route_scores_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| key.starts_with("__route_health__"))
        .count();
    let restored_bad_pairing_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| key.starts_with("__route_bad_pairing__"))
        .count();
    let restored_success_streak_count = persisted_profile_scores
        .value
        .keys()
        .filter(|key| key.starts_with("__route_success__"))
        .count();
    let shared = RuntimeRotationProxyShared {
        upstream_no_proxy,
        async_client: build_runtime_upstream_async_http_client(upstream_no_proxy)?,
        async_runtime,
        log_path: log_path.clone(),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: lane_admission.clone(),
        runtime: Arc::new(Mutex::new(RuntimeRotationState {
            paths: paths.clone(),
            state: restored_state.clone(),
            upstream_base_url: upstream_base_url.clone(),
            include_code_review,
            current_profile: current_profile.to_string(),
            profile_usage_auth: load_runtime_profile_usage_auth_cache(&restored_state),
            turn_state_bindings: restored_continuations.turn_state_bindings.clone(),
            session_id_bindings: restored_runtime_session_id_bindings,
            continuation_statuses: restored_continuations.statuses.clone(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: persisted_usage_snapshots.value,
            profile_retry_backoff_until: persisted_backoffs.value.retry_backoff_until,
            profile_transport_backoff_until: persisted_backoffs.value.transport_backoff_until,
            profile_route_circuit_open_until: persisted_backoffs.value.route_circuit_open_until,
            profile_inflight: BTreeMap::new(),
            profile_health: persisted_profile_scores.value,
        })),
    };
    register_runtime_proxy_persistence_mode(&log_path, persistence_enabled);
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime proxy started listen_addr={listen_addr} current_profile={current_profile} include_code_review={include_code_review} upstream_base_url={upstream_base_url} persistence_mode={}",
            if persistence_enabled {
                "owner"
            } else {
                "follower"
            }
        ),
    );
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime_proxy_upstream_proxy_mode mode={}",
            runtime_upstream_proxy_mode_label(upstream_no_proxy)
        ),
    );
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime_proxy_restore_counts persisted_scores={} persisted_usage_snapshots={} expired_usage_snapshots={} response_bindings={} session_bindings={} turn_state_bindings={} session_id_bindings={} retry_backoffs={} transport_backoffs={} route_circuits={} global_scores={} route_scores={} bad_pairing_scores={} success_streak_scores={} recovered_state={} recovered_continuations={} recovered_scores={} recovered_usage_snapshots={} recovered_backoffs={} recovered_continuation_journal={}",
            persisted_profile_scores_count,
            persisted_usage_snapshots_count,
            expired_usage_snapshot_count,
            persisted_response_binding_count,
            persisted_session_binding_count,
            persisted_turn_state_binding_count,
            persisted_session_id_binding_count,
            persisted_retry_backoffs_count,
            persisted_transport_backoffs_count,
            persisted_route_circuit_count,
            restored_global_scores_count,
            restored_route_scores_count,
            restored_bad_pairing_count,
            restored_success_streak_count,
            persisted_state.recovered_from_backup,
            persisted_continuations.recovered_from_backup,
            persisted_profile_scores.recovered_from_backup,
            persisted_usage_snapshots.recovered_from_backup,
            persisted_backoffs.recovered_from_backup,
            continuation_journal.recovered_from_backup,
        ),
    );
    audit_runtime_proxy_startup_state(&shared);
    schedule_runtime_startup_probe_warmup(&shared);
    if persisted_backoffs_softened && let Ok(runtime) = shared.runtime.lock() {
        schedule_runtime_state_save_from_runtime(&shared, &runtime, "startup_backoff_soften");
    }
    if continuation_migration_needed && let Ok(runtime) = shared.runtime.lock() {
        schedule_runtime_state_save_from_runtime(
            &shared,
            &runtime,
            "startup_continuation_migration",
        );
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    let mut worker_threads = Vec::new();
    let (long_lived_sender, long_lived_receiver) =
        mpsc::sync_channel::<tiny_http::Request>(long_lived_queue_capacity);
    let long_lived_receiver = Arc::new(Mutex::new(long_lived_receiver));
    runtime_proxy_log_to_path(
        &log_path,
        &format!(
            "runtime proxy worker_count={worker_count} async_worker_count={async_worker_count} long_lived_worker_count={long_lived_worker_count} long_lived_queue_capacity={long_lived_queue_capacity} active_request_limit={active_request_limit} lane_limits=responses:{} compact:{} websocket:{} standard:{}",
            lane_admission.limits.responses,
            lane_admission.limits.compact,
            lane_admission.limits.websocket,
            lane_admission.limits.standard
        ),
    );

    for _ in 0..long_lived_worker_count {
        let shutdown = Arc::clone(&shutdown);
        let shared = shared.clone();
        let receiver = Arc::clone(&long_lived_receiver);
        worker_threads.push(thread::spawn(move || {
            loop {
                let request = {
                    let guard = receiver.lock();
                    let Ok(receiver) = guard else {
                        break;
                    };
                    receiver.recv()
                };
                match request {
                    Ok(request) => {
                        let (mutex, condvar) = &*shared.lane_admission.wait;
                        let guard = mutex
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        condvar.notify_all();
                        drop(guard);
                        handle_runtime_rotation_proxy_request(request, &shared);
                    }
                    Err(_) => break,
                }
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
            }
        }));
    }

    for _ in 0..worker_count {
        let server: Arc<TinyServer> = Arc::clone(&server);
        let shutdown = Arc::clone(&shutdown);
        let shared = shared.clone();
        let long_lived_sender = long_lived_sender.clone();
        worker_threads.push(thread::spawn(move || {
            loop {
                match server.recv() {
                    Ok(request) => {
                        let websocket = is_tiny_http_websocket_upgrade(&request);
                        let long_lived =
                            runtime_proxy_request_is_long_lived(request.url(), websocket);
                        if long_lived {
                            match enqueue_runtime_proxy_long_lived_request_with_wait(
                                &long_lived_sender,
                                request,
                                &shared,
                            ) {
                                Ok(()) => {}
                                Err((RuntimeProxyQueueRejection::Full, request)) => {
                                    mark_runtime_proxy_local_overload(
                                        &shared,
                                        "long_lived_queue_full",
                                    );
                                    reject_runtime_proxy_overloaded_request(
                                        request,
                                        &shared,
                                        "long_lived_queue_full",
                                    );
                                }
                                Err((RuntimeProxyQueueRejection::Disconnected, request)) => {
                                    mark_runtime_proxy_local_overload(
                                        &shared,
                                        "long_lived_queue_disconnected",
                                    );
                                    reject_runtime_proxy_overloaded_request(
                                        request,
                                        &shared,
                                        "long_lived_queue_disconnected",
                                    );
                                }
                            }
                        } else {
                            handle_runtime_rotation_proxy_request(request, &shared);
                        }
                    }
                    Err(_) if shutdown.load(Ordering::SeqCst) => break,
                    Err(_) => {}
                }
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
            }
        }));
    }

    Ok(RuntimeRotationProxy {
        server,
        shutdown,
        worker_threads,
        accept_worker_count: worker_count,
        listen_addr,
        log_path,
        active_request_count: Arc::clone(&shared.active_request_count),
        owner_lock,
    })
}
