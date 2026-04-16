use super::*;

pub(super) fn runtime_proxy_admin_token(request: &tiny_http::Request) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv("X-Prodex-Admin-Token"))
        .map(|header| header.value.as_str().trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn build_runtime_proxy_json_response(
    status: u16,
    body: String,
) -> tiny_http::ResponseBox {
    let mut response = TinyResponse::from_string(body).with_status_code(status);
    if let Ok(header) = TinyHeader::from_bytes("Content-Type", "application/json") {
        response = response.with_header(header);
    }
    response.boxed()
}

pub(super) fn build_runtime_proxy_string_response(
    status: u16,
    body: String,
    content_type: &str,
) -> tiny_http::ResponseBox {
    let mut response = TinyResponse::from_string(body).with_status_code(status);
    if let Ok(header) = TinyHeader::from_bytes("Content-Type", content_type) {
        response = response.with_header(header);
    }
    response.boxed()
}

pub(super) fn build_runtime_proxy_prometheus_response(
    status: u16,
    body: String,
) -> tiny_http::ResponseBox {
    build_runtime_proxy_string_response(status, body, "text/plain; version=0.0.4; charset=utf-8")
}

pub(super) fn update_runtime_broker_current_profile(log_path: &Path, current_profile: &str) {
    let mut metadata_by_path = runtime_broker_metadata_by_log_path()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(metadata) = metadata_by_path.get_mut(log_path) {
        metadata.current_profile = current_profile.to_string();
    }
}

pub(super) fn runtime_broker_continuation_metrics(
    statuses: &RuntimeContinuationStatuses,
) -> RuntimeBrokerContinuationMetrics {
    let mut metrics = RuntimeBrokerContinuationMetrics {
        response_bindings: statuses.response.len(),
        turn_state_bindings: statuses.turn_state.len(),
        session_id_bindings: statuses.session_id.len(),
        warm: 0,
        verified: 0,
        suspect: 0,
        dead: 0,
    };
    for status in statuses
        .response
        .values()
        .chain(statuses.turn_state.values())
        .chain(statuses.session_id.values())
    {
        match status.state {
            RuntimeContinuationBindingLifecycle::Warm => metrics.warm += 1,
            RuntimeContinuationBindingLifecycle::Verified => metrics.verified += 1,
            RuntimeContinuationBindingLifecycle::Suspect => metrics.suspect += 1,
            RuntimeContinuationBindingLifecycle::Dead => metrics.dead += 1,
        }
    }
    metrics
}

pub(super) fn runtime_broker_prometheus_snapshot(
    metadata: &RuntimeBrokerMetadata,
    metrics: &RuntimeBrokerMetrics,
) -> runtime_metrics::RuntimeBrokerSnapshot {
    RuntimeBrokerSnapshotBuilder::new(metadata, metrics).build()
}

struct RuntimeBrokerSnapshotBuilder<'a> {
    metadata: &'a RuntimeBrokerMetadata,
    metrics: &'a RuntimeBrokerMetrics,
}

impl<'a> RuntimeBrokerSnapshotBuilder<'a> {
    fn new(metadata: &'a RuntimeBrokerMetadata, metrics: &'a RuntimeBrokerMetrics) -> Self {
        Self { metadata, metrics }
    }

    fn build(&self) -> runtime_metrics::RuntimeBrokerSnapshot {
        runtime_metrics::RuntimeBrokerSnapshot {
            broker_key: self.metadata.broker_key.clone(),
            listen_addr: self.metadata.listen_addr.clone(),
            pid: self.metrics.health.pid,
            started_at_unix_seconds: self.metrics.health.started_at,
            current_profile: self.metrics.health.current_profile.clone(),
            include_code_review: self.metrics.health.include_code_review,
            persistence_role: self.metrics.health.persistence_role.clone(),
            active_requests: self.metrics.health.active_requests as u64,
            active_request_limit: self.metrics.active_request_limit as u64,
            local_overload_backoff_remaining_seconds: self
                .metrics
                .local_overload_backoff_remaining_seconds,
            traffic: self.build_traffic(),
            profile_inflight: self.build_profile_inflight(),
            retry_backoffs: self.metrics.retry_backoffs as u64,
            transport_backoffs: self.metrics.transport_backoffs as u64,
            route_circuits: self.metrics.route_circuits as u64,
            degraded_profiles: self.metrics.degraded_profiles as u64,
            degraded_routes: self.metrics.degraded_routes as u64,
            continuations: self.build_continuations(),
        }
    }

    fn build_traffic(&self) -> runtime_metrics::RuntimeBrokerTrafficMetrics {
        runtime_metrics::RuntimeBrokerTrafficMetrics {
            responses: Self::build_lane(&self.metrics.traffic.responses),
            compact: Self::build_lane(&self.metrics.traffic.compact),
            websocket: Self::build_lane(&self.metrics.traffic.websocket),
            standard: Self::build_lane(&self.metrics.traffic.standard),
        }
    }

    fn build_profile_inflight(&self) -> std::collections::BTreeMap<String, u64> {
        self.metrics
            .profile_inflight
            .iter()
            .map(|(profile, count)| (profile.clone(), *count as u64))
            .collect()
    }

    fn build_continuations(&self) -> runtime_metrics::RuntimeBrokerContinuationMetrics {
        runtime_metrics::RuntimeBrokerContinuationMetrics {
            response_bindings: self.metrics.continuations.response_bindings as u64,
            turn_state_bindings: self.metrics.continuations.turn_state_bindings as u64,
            session_id_bindings: self.metrics.continuations.session_id_bindings as u64,
            warm: self.metrics.continuations.warm as u64,
            verified: self.metrics.continuations.verified as u64,
            suspect: self.metrics.continuations.suspect as u64,
            dead: self.metrics.continuations.dead as u64,
        }
    }

    fn build_lane(lane: &RuntimeBrokerLaneMetrics) -> runtime_metrics::RuntimeBrokerLaneMetrics {
        runtime_metrics::RuntimeBrokerLaneMetrics {
            active: lane.active as u64,
            limit: lane.limit as u64,
        }
    }
}

pub(super) fn runtime_broker_metrics_snapshot(
    shared: &RuntimeRotationProxyShared,
    metadata: &RuntimeBrokerMetadata,
) -> Result<RuntimeBrokerMetrics> {
    let now = Local::now().timestamp();
    let now_u64 = now.max(0) as u64;
    let runtime = shared
        .runtime
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;

    let health = RuntimeBrokerHealth {
        pid: std::process::id(),
        started_at: metadata.started_at,
        current_profile: metadata.current_profile.clone(),
        include_code_review: metadata.include_code_review,
        active_requests: shared.active_request_count.load(Ordering::SeqCst),
        instance_token: metadata.instance_token.clone(),
        persistence_role: if runtime_proxy_persistence_enabled(shared) {
            "owner".to_string()
        } else {
            "follower".to_string()
        },
    };

    let degraded_profiles = runtime
        .profile_health
        .iter()
        .filter(|(key, entry)| {
            !key.starts_with("__") && runtime_profile_effective_health_score(entry, now) > 0
        })
        .count();
    let degraded_routes = runtime
        .profile_health
        .iter()
        .filter(|(key, entry)| {
            key.starts_with("__route_health__:")
                && runtime_profile_effective_health_score(entry, now) > 0
        })
        .count();

    Ok(RuntimeBrokerMetrics {
        health,
        active_request_limit: shared.active_request_limit,
        local_overload_backoff_remaining_seconds: shared
            .local_overload_backoff_until
            .load(Ordering::SeqCst)
            .saturating_sub(now_u64),
        traffic: RuntimeBrokerTrafficMetrics {
            responses: RuntimeBrokerLaneMetrics {
                active: shared
                    .lane_admission
                    .responses_active
                    .load(Ordering::SeqCst),
                limit: shared.lane_admission.limits.responses,
            },
            compact: RuntimeBrokerLaneMetrics {
                active: shared.lane_admission.compact_active.load(Ordering::SeqCst),
                limit: shared.lane_admission.limits.compact,
            },
            websocket: RuntimeBrokerLaneMetrics {
                active: shared
                    .lane_admission
                    .websocket_active
                    .load(Ordering::SeqCst),
                limit: shared.lane_admission.limits.websocket,
            },
            standard: RuntimeBrokerLaneMetrics {
                active: shared.lane_admission.standard_active.load(Ordering::SeqCst),
                limit: shared.lane_admission.limits.standard,
            },
        },
        profile_inflight: runtime.profile_inflight.clone(),
        retry_backoffs: runtime
            .profile_retry_backoff_until
            .values()
            .filter(|until| **until > now)
            .count(),
        transport_backoffs: runtime
            .profile_transport_backoff_until
            .values()
            .filter(|until| **until > now)
            .count(),
        route_circuits: runtime
            .profile_route_circuit_open_until
            .values()
            .filter(|until| **until > now)
            .count(),
        degraded_profiles,
        degraded_routes,
        continuations: runtime_broker_continuation_metrics(&runtime.continuation_statuses),
    })
}

pub(super) fn handle_runtime_proxy_admin_request(
    request: &mut tiny_http::Request,
    shared: &RuntimeRotationProxyShared,
) -> Option<tiny_http::ResponseBox> {
    let path = path_without_query(request.url());
    if path != "/__prodex/runtime/health"
        && path != "/__prodex/runtime/metrics"
        && path != "/__prodex/runtime/metrics/prometheus"
        && path != "/__prodex/runtime/activate"
    {
        return None;
    }

    let Some(metadata) = runtime_broker_metadata_for_log_path(&shared.log_path) else {
        return Some(build_runtime_proxy_json_error_response(
            404,
            "not_found",
            "runtime broker admin endpoint is not enabled for this proxy",
        ));
    };
    if runtime_proxy_admin_token(request).as_deref() != Some(metadata.admin_token.as_str()) {
        return Some(build_runtime_proxy_json_error_response(
            403,
            "forbidden",
            "missing or invalid runtime broker admin token",
        ));
    }

    if path == "/__prodex/runtime/health" {
        let health = RuntimeBrokerHealth {
            pid: std::process::id(),
            started_at: metadata.started_at,
            current_profile: metadata.current_profile,
            include_code_review: metadata.include_code_review,
            active_requests: shared.active_request_count.load(Ordering::SeqCst),
            instance_token: metadata.instance_token,
            persistence_role: if runtime_proxy_persistence_enabled(shared) {
                "owner".to_string()
            } else {
                "follower".to_string()
            },
        };
        let body = serde_json::to_string(&health).ok()?;
        return Some(build_runtime_proxy_json_response(200, body));
    }

    if path == "/__prodex/runtime/metrics" {
        let metrics = match runtime_broker_metrics_snapshot(shared, &metadata) {
            Ok(metrics) => metrics,
            Err(err) => {
                return Some(build_runtime_proxy_json_error_response(
                    500,
                    "internal_error",
                    &err.to_string(),
                ));
            }
        };
        let body = serde_json::to_string(&metrics).ok()?;
        return Some(build_runtime_proxy_json_response(200, body));
    }

    if path == "/__prodex/runtime/metrics/prometheus" {
        let metrics = match runtime_broker_metrics_snapshot(shared, &metadata) {
            Ok(metrics) => metrics,
            Err(err) => {
                return Some(build_runtime_proxy_json_error_response(
                    500,
                    "internal_error",
                    &err.to_string(),
                ));
            }
        };
        let snapshot = runtime_broker_prometheus_snapshot(&metadata, &metrics);
        let body = runtime_metrics::render_runtime_broker_prometheus(&snapshot);
        return Some(build_runtime_proxy_prometheus_response(200, body));
    }

    if request.method().as_str() != "POST" {
        return Some(build_runtime_proxy_json_error_response(
            405,
            "method_not_allowed",
            "runtime broker activation requires POST",
        ));
    }

    let mut body = Vec::new();
    if let Err(err) = request.as_reader().read_to_end(&mut body) {
        return Some(build_runtime_proxy_json_error_response(
            400,
            "invalid_request",
            &format!("failed to read runtime broker activation body: {err}"),
        ));
    }
    let current_profile = match serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("current_profile")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .map(str::to_string)
        })
        .filter(|value| !value.is_empty())
    {
        Some(current_profile) => current_profile,
        None => {
            return Some(build_runtime_proxy_json_error_response(
                400,
                "invalid_request",
                "runtime broker activation requires a non-empty current_profile",
            ));
        }
    };

    let update_result = (|| -> Result<()> {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
        runtime.current_profile = current_profile.clone();
        runtime.state.active_profile = Some(current_profile.clone());
        Ok(())
    })();
    if let Err(err) = update_result {
        return Some(build_runtime_proxy_json_error_response(
            500,
            "internal_error",
            &err.to_string(),
        ));
    }
    update_runtime_broker_current_profile(&shared.log_path, &current_profile);
    runtime_proxy_log(
        shared,
        format!("runtime_broker_activate current_profile={current_profile}"),
    );
    audit_log_event_best_effort(
        "runtime_broker",
        "activate_profile",
        "success",
        serde_json::json!({
            "broker_key": metadata.broker_key,
            "listen_addr": metadata.listen_addr,
            "current_profile": current_profile,
        }),
    );
    Some(build_runtime_proxy_json_response(
        200,
        serde_json::json!({
            "ok": true,
            "current_profile": current_profile,
        })
        .to_string(),
    ))
}

pub(super) fn runtime_broker_key(upstream_base_url: &str, include_code_review: bool) -> String {
    let mut hasher = DefaultHasher::new();
    upstream_base_url.hash(&mut hasher);
    include_code_review.hash(&mut hasher);
    RUNTIME_PROXY_OPENAI_MOUNT_PATH.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(super) fn runtime_process_pid_alive(pid: u32) -> bool {
    let proc_dir = PathBuf::from(format!("/proc/{pid}"));
    if proc_dir.exists() {
        return true;
    }
    collect_process_rows().into_iter().any(|row| row.pid == pid)
}

pub(super) fn runtime_random_token(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = STATE_SAVE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{nanos:x}-{sequence:x}", std::process::id())
}

pub(super) fn runtime_broker_startup_grace_seconds() -> i64 {
    let ready_timeout_seconds = runtime_broker_ready_timeout_ms().div_ceil(1_000) as i64;
    ready_timeout_seconds
        .saturating_add(1)
        .max(RUNTIME_BROKER_IDLE_GRACE_SECONDS)
}

pub(super) fn load_runtime_broker_registry(
    paths: &AppPaths,
    broker_key: &str,
) -> Result<Option<RuntimeBrokerRegistry>> {
    let path = runtime_broker_registry_file_path(paths, broker_key);
    let backup_path = runtime_broker_registry_last_good_file_path(paths, broker_key);
    if !path.exists() && !backup_path.exists() {
        return Ok(None);
    }
    match load_json_file_with_backup::<RuntimeBrokerRegistry>(&path, &backup_path) {
        Ok(loaded) => Ok(Some(loaded.value)),
        Err(_err) if !path.exists() && !backup_path.exists() => Ok(None),
        Err(err) => Err(err),
    }
}

pub(super) fn save_runtime_broker_registry(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
) -> Result<()> {
    let path = runtime_broker_registry_file_path(paths, broker_key);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(registry)
        .context("failed to serialize runtime broker registry")?;
    write_json_file_with_backup(
        &path,
        &runtime_broker_registry_last_good_file_path(paths, broker_key),
        &json,
        |content| {
            let _: RuntimeBrokerRegistry = serde_json::from_str(content)
                .context("failed to validate runtime broker registry")?;
            Ok(())
        },
    )
}

pub(super) fn remove_runtime_broker_registry_if_token_matches(
    paths: &AppPaths,
    broker_key: &str,
    instance_token: &str,
) {
    let Ok(Some(existing)) = load_runtime_broker_registry(paths, broker_key) else {
        return;
    };
    if existing.instance_token != instance_token {
        return;
    }
    for path in [
        runtime_broker_registry_file_path(paths, broker_key),
        runtime_broker_registry_last_good_file_path(paths, broker_key),
    ] {
        let _ = fs::remove_file(path);
    }
}

pub(super) fn runtime_broker_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_millis(
            runtime_broker_health_connect_timeout_ms(),
        ))
        .timeout(Duration::from_millis(
            runtime_broker_health_read_timeout_ms(),
        ))
        .build()
        .context("failed to build runtime broker control client")
}

pub(super) fn runtime_broker_health_url(registry: &RuntimeBrokerRegistry) -> String {
    format!("http://{}/__prodex/runtime/health", registry.listen_addr)
}

pub(super) fn runtime_broker_metrics_url(registry: &RuntimeBrokerRegistry) -> String {
    format!("http://{}/__prodex/runtime/metrics", registry.listen_addr)
}

pub(super) fn runtime_broker_metrics_prometheus_url(registry: &RuntimeBrokerRegistry) -> String {
    format!(
        "http://{}/__prodex/runtime/metrics/prometheus",
        registry.listen_addr
    )
}

pub(super) fn runtime_broker_activate_url(registry: &RuntimeBrokerRegistry) -> String {
    format!("http://{}/__prodex/runtime/activate", registry.listen_addr)
}

pub(super) fn legacy_runtime_proxy_openai_mount_path(version: &str) -> String {
    format!("{LEGACY_RUNTIME_PROXY_OPENAI_MOUNT_PATH_PREFIX}{version}")
}

pub(super) fn parse_prodex_version_output(output: &str) -> Option<String> {
    let mut parts = output.split_whitespace();
    let binary_name = parts.next()?;
    let version = parts.next()?;
    if binary_name == "prodex" && !version.is_empty() {
        return Some(version.to_string());
    }
    None
}

pub(super) fn read_prodex_version_from_executable(executable: &Path) -> Result<String> {
    let output = Command::new(executable)
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .with_context(|| format!("failed to run {} --version", executable.display()))?;
    if !output.status.success() {
        bail!(
            "{} --version exited with status {}",
            executable.display(),
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_string())
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_prodex_version_output(&stdout).with_context(|| {
        format!(
            "failed to parse prodex version output from {}",
            executable.display()
        )
    })
}

pub(super) fn runtime_process_executable_path(pid: u32) -> Option<PathBuf> {
    collect_process_rows()
        .into_iter()
        .find(|row| row.pid == pid)
        .and_then(|row| row.args.into_iter().rfind(|arg| Path::new(arg).exists()))
        .map(PathBuf::from)
        .or_else(|| fs::read_link(format!("/proc/{pid}/exe")).ok())
}

pub(super) fn runtime_broker_openai_mount_path(registry: &RuntimeBrokerRegistry) -> Result<String> {
    if let Some(openai_mount_path) = registry.openai_mount_path.as_deref() {
        return Ok(openai_mount_path.to_string());
    }

    let executable = runtime_process_executable_path(registry.pid).with_context(|| {
        format!(
            "failed to resolve executable for runtime broker pid {}",
            registry.pid
        )
    })?;
    let version = read_prodex_version_from_executable(&executable)?;
    Ok(legacy_runtime_proxy_openai_mount_path(&version))
}

pub(super) fn runtime_proxy_endpoint_from_registry(
    paths: &AppPaths,
    broker_key: &str,
    registry: &RuntimeBrokerRegistry,
) -> Result<RuntimeProxyEndpoint> {
    let lease = create_runtime_broker_lease(paths, broker_key)?;
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    let listen_addr = registry.listen_addr.parse().with_context(|| {
        format!(
            "invalid runtime broker listen address {}",
            registry.listen_addr
        )
    })?;
    Ok(RuntimeProxyEndpoint {
        listen_addr,
        openai_mount_path: runtime_broker_openai_mount_path(registry)?,
        lease_dir,
        _lease: Some(lease),
    })
}

pub(super) fn runtime_broker_process_args(
    current_profile: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    broker_key: &str,
    instance_token: &str,
    admin_token: &str,
    listen_addr: Option<&str>,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("__runtime-broker"),
        OsString::from("--current-profile"),
        OsString::from(current_profile),
        OsString::from("--upstream-base-url"),
        OsString::from(upstream_base_url),
    ];
    if include_code_review {
        args.push(OsString::from("--include-code-review"));
    }
    args.extend([
        OsString::from("--broker-key"),
        OsString::from(broker_key),
        OsString::from("--instance-token"),
        OsString::from(instance_token),
        OsString::from("--admin-token"),
        OsString::from(admin_token),
    ]);
    if let Some(listen_addr) = listen_addr {
        args.push(OsString::from("--listen-addr"));
        args.push(OsString::from(listen_addr));
    }
    args
}

pub(super) fn probe_runtime_broker_health(
    client: &Client,
    registry: &RuntimeBrokerRegistry,
) -> Result<Option<RuntimeBrokerHealth>> {
    let response = match client
        .get(runtime_broker_health_url(registry))
        .header("X-Prodex-Admin-Token", &registry.admin_token)
        .send()
    {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    if !response.status().is_success() {
        return Ok(None);
    }
    let health = response
        .json::<RuntimeBrokerHealth>()
        .context("failed to decode runtime broker health response")?;
    Ok(Some(health))
}

pub(super) fn probe_runtime_broker_metrics(
    client: &Client,
    registry: &RuntimeBrokerRegistry,
) -> Result<Option<RuntimeBrokerMetrics>> {
    let response = match client
        .get(runtime_broker_metrics_url(registry))
        .header("X-Prodex-Admin-Token", &registry.admin_token)
        .send()
    {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    if !response.status().is_success() {
        return Ok(None);
    }
    let metrics = response
        .json::<RuntimeBrokerMetrics>()
        .context("failed to decode runtime broker metrics response")?;
    Ok(Some(metrics))
}

pub(super) fn collect_live_runtime_broker_observations(
    paths: &AppPaths,
) -> Vec<RuntimeBrokerObservation> {
    let Ok(client) = runtime_broker_client() else {
        return Vec::new();
    };

    let mut observations = Vec::new();
    for broker_key in runtime_broker_registry_keys(paths) {
        let Ok(Some(registry)) = load_runtime_broker_registry(paths, &broker_key) else {
            continue;
        };
        if !runtime_process_pid_alive(registry.pid) {
            continue;
        }
        let Ok(Some(metrics)) = probe_runtime_broker_metrics(&client, &registry) else {
            continue;
        };
        observations.push(RuntimeBrokerObservation {
            broker_key,
            listen_addr: registry.listen_addr,
            metrics,
        });
    }
    observations
}

pub(super) fn collect_runtime_broker_metrics_targets(paths: &AppPaths) -> Vec<String> {
    let mut targets = Vec::new();
    for broker_key in runtime_broker_registry_keys(paths) {
        let Ok(Some(registry)) = load_runtime_broker_registry(paths, &broker_key) else {
            continue;
        };
        if !runtime_process_pid_alive(registry.pid) {
            continue;
        }
        targets.push(runtime_broker_metrics_prometheus_url(&registry));
    }
    targets
}

pub(super) fn format_runtime_broker_metrics_targets(targets: &[String]) -> String {
    match targets {
        [] => "-".to_string(),
        [target] => target.clone(),
        [first, rest @ ..] => format!("{first} (+{} more)", rest.len()),
    }
}

pub(super) fn activate_runtime_broker_profile(
    client: &Client,
    registry: &RuntimeBrokerRegistry,
    current_profile: &str,
) -> Result<()> {
    let response = client
        .post(runtime_broker_activate_url(registry))
        .header("X-Prodex-Admin-Token", &registry.admin_token)
        .json(&serde_json::json!({
            "current_profile": current_profile,
        }))
        .send()
        .context("failed to send runtime broker activation request")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        bail!(
            "runtime broker activation failed with HTTP {}{}",
            status,
            if body.is_empty() {
                String::new()
            } else {
                format!(": {body}")
            }
        );
    }
    Ok(())
}

pub(super) fn create_runtime_broker_lease(
    paths: &AppPaths,
    broker_key: &str,
) -> Result<RuntimeBrokerLease> {
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    create_runtime_broker_lease_in_dir_for_pid(&lease_dir, std::process::id())
}

pub(super) fn create_runtime_broker_lease_in_dir_for_pid(
    lease_dir: &Path,
    pid: u32,
) -> Result<RuntimeBrokerLease> {
    fs::create_dir_all(lease_dir)
        .with_context(|| format!("failed to create {}", lease_dir.display()))?;
    let path = lease_dir.join(format!("{}-{}.lease", pid, runtime_random_token("lease")));
    fs::write(&path, format!("pid={pid}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(RuntimeBrokerLease { path })
}

pub(super) fn cleanup_runtime_broker_stale_leases(paths: &AppPaths, broker_key: &str) -> usize {
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    let Ok(entries) = fs::read_dir(&lease_dir) else {
        return 0;
    };
    let mut live = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let pid = file_name
            .split('-')
            .next()
            .and_then(|value| value.parse::<u32>().ok());
        if pid.is_some_and(runtime_process_pid_alive) {
            live += 1;
        } else {
            let _ = fs::remove_file(path);
        }
    }
    live
}

pub(super) fn wait_for_existing_runtime_broker_recovery_or_exit(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    upstream_base_url: &str,
    include_code_review: bool,
) -> Result<Option<RuntimeBrokerRegistry>> {
    let started_at = Instant::now();
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    while started_at.elapsed() < Duration::from_millis(runtime_broker_ready_timeout_ms()) {
        let Some(existing) = load_runtime_broker_registry(paths, broker_key)? else {
            return Ok(None);
        };

        if existing.upstream_base_url == upstream_base_url
            && existing.include_code_review == include_code_review
            && let Some(health) = probe_runtime_broker_health(client, &existing)?
            && health.instance_token == existing.instance_token
        {
            return Ok(Some(existing));
        }

        if !runtime_process_pid_alive(existing.pid) {
            remove_runtime_broker_registry_if_token_matches(
                paths,
                broker_key,
                &existing.instance_token,
            );
            return Ok(None);
        }

        thread::sleep(poll_interval);
    }

    Ok(None)
}

pub(super) fn find_compatible_runtime_broker_registry(
    client: &Client,
    paths: &AppPaths,
    excluded_broker_key: &str,
    upstream_base_url: &str,
    include_code_review: bool,
) -> Result<Option<(String, RuntimeBrokerRegistry)>> {
    for broker_key in runtime_broker_registry_keys(paths) {
        if broker_key == excluded_broker_key {
            continue;
        }

        let Some(registry) = load_runtime_broker_registry(paths, &broker_key)? else {
            continue;
        };
        if registry.upstream_base_url != upstream_base_url
            || registry.include_code_review != include_code_review
        {
            continue;
        }
        if !runtime_process_pid_alive(registry.pid) {
            remove_runtime_broker_registry_if_token_matches(
                paths,
                &broker_key,
                &registry.instance_token,
            );
            continue;
        }
        if let Some(health) = probe_runtime_broker_health(client, &registry)?
            && health.instance_token == registry.instance_token
        {
            return Ok(Some((broker_key, registry)));
        }
    }

    Ok(None)
}

pub(super) fn wait_for_runtime_broker_ready(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    expected_instance_token: &str,
) -> Result<RuntimeBrokerRegistry> {
    let started_at = Instant::now();
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    while started_at.elapsed() < Duration::from_millis(runtime_broker_ready_timeout_ms()) {
        if let Some(registry) = load_runtime_broker_registry(paths, broker_key)?
            && registry.instance_token == expected_instance_token
            && let Some(health) = probe_runtime_broker_health(client, &registry)?
            && health.instance_token == expected_instance_token
        {
            return Ok(registry);
        }
        thread::sleep(poll_interval);
    }
    bail!("timed out waiting for runtime broker readiness");
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_runtime_broker_process(
    paths: &AppPaths,
    current_profile: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    broker_key: &str,
    instance_token: &str,
    admin_token: &str,
    listen_addr: Option<&str>,
) -> Result<()> {
    let current_exe = env::current_exe().context("failed to locate current prodex binary")?;
    Command::new(current_exe)
        .args(runtime_broker_process_args(
            current_profile,
            upstream_base_url,
            include_code_review,
            broker_key,
            instance_token,
            admin_token,
            listen_addr,
        ))
        .env("PRODEX_HOME", &paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn runtime broker process")?;
    Ok(())
}

pub(super) fn preferred_runtime_broker_listen_addr(
    paths: &AppPaths,
    broker_key: &str,
) -> Result<Option<String>> {
    Ok(
        load_runtime_broker_registry(paths, broker_key)?.and_then(|registry| {
            (!runtime_process_pid_alive(registry.pid)).then_some(registry.listen_addr)
        }),
    )
}

pub(super) fn ensure_runtime_rotation_proxy_endpoint(
    paths: &AppPaths,
    current_profile: &str,
    upstream_base_url: &str,
    include_code_review: bool,
) -> Result<RuntimeProxyEndpoint> {
    let broker_key = runtime_broker_key(upstream_base_url, include_code_review);
    let ensure_lock_path = runtime_broker_ensure_lock_path(paths, &broker_key);
    let _ensure_lock = acquire_json_file_lock(&ensure_lock_path)?;
    let preferred_listen_addr = preferred_runtime_broker_listen_addr(paths, &broker_key)?;
    let broker_client = runtime_broker_client()?;

    if let Some(existing) = wait_for_existing_runtime_broker_recovery_or_exit(
        &broker_client,
        paths,
        &broker_key,
        upstream_base_url,
        include_code_review,
    )? {
        activate_runtime_broker_profile(&broker_client, &existing, current_profile)?;
        return runtime_proxy_endpoint_from_registry(paths, &broker_key, &existing);
    }

    if let Some(existing) = load_runtime_broker_registry(paths, &broker_key)? {
        if !runtime_process_pid_alive(existing.pid) {
            remove_runtime_broker_registry_if_token_matches(
                paths,
                &broker_key,
                &existing.instance_token,
            );
        } else if existing.upstream_base_url == upstream_base_url
            && existing.include_code_review == include_code_review
            && let Some(health) = probe_runtime_broker_health(&broker_client, &existing)?
            && health.instance_token == existing.instance_token
        {
            activate_runtime_broker_profile(&broker_client, &existing, current_profile)?;
            return runtime_proxy_endpoint_from_registry(paths, &broker_key, &existing);
        }
    }

    if let Some((existing_broker_key, existing)) = find_compatible_runtime_broker_registry(
        &broker_client,
        paths,
        &broker_key,
        upstream_base_url,
        include_code_review,
    )? {
        activate_runtime_broker_profile(&broker_client, &existing, current_profile)?;
        return runtime_proxy_endpoint_from_registry(paths, &existing_broker_key, &existing);
    }

    let instance_token = runtime_random_token("broker");
    let admin_token = runtime_random_token("admin");
    spawn_runtime_broker_process(
        paths,
        current_profile,
        upstream_base_url,
        include_code_review,
        &broker_key,
        &instance_token,
        &admin_token,
        preferred_listen_addr.as_deref(),
    )?;
    let registry =
        wait_for_runtime_broker_ready(&broker_client, paths, &broker_key, &instance_token)?;
    activate_runtime_broker_profile(&broker_client, &registry, current_profile)?;
    runtime_proxy_endpoint_from_registry(paths, &broker_key, &registry)
}
