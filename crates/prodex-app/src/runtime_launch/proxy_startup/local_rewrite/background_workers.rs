use super::*;

pub(in crate::runtime_launch::proxy_startup) struct RuntimeLocalRewriteWorkers {
    pub(in crate::runtime_launch::proxy_startup) worker_threads: Vec<thread::JoinHandle<()>>,
    pub(in crate::runtime_launch::proxy_startup) gemini_live_sidecar_addr:
        Option<std::net::SocketAddr>,
}

pub(super) fn runtime_local_rewrite_usage_state(
    usage: BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
    path: PathBuf,
) -> RuntimeGatewayVirtualKeyUsageState {
    RuntimeGatewayVirtualKeyUsageState {
        usage: Arc::new(Mutex::new(usage)),
        path: Some(path),
        save_in_flight: Arc::new(AtomicBool::new(false)),
        save_dirty: Arc::new(AtomicBool::new(false)),
        usage_slots: Arc::new(tokio::sync::Semaphore::new(
            super::super::local_rewrite_gateway_usage::RUNTIME_GATEWAY_PENDING_USAGE_DELTA_LIMIT,
        )),
        pending_deltas: Arc::new(Mutex::new(Vec::new())),
        reconciliation: RuntimeGatewayReconciliationQueue::new(),
        request_ids: Arc::new(Mutex::new(BTreeSet::new())),
        typed_request_ids: Arc::new(Mutex::new(BTreeMap::new())),
        call_ids: Arc::new(Mutex::new(BTreeMap::new())),
        ledger_scopes: Arc::new(Mutex::new(BTreeMap::new())),
        durable_reservations: Arc::new(Mutex::new(BTreeMap::new())),
    }
}
pub(super) fn runtime_local_rewrite_log_path(runtime_config: &RuntimeConfig) -> Result<PathBuf> {
    let log_path = initialize_runtime_proxy_log_path_from_config(runtime_config)?;
    for key in runtime_config.compatibility_defaults() {
        runtime_proxy_log_to_path(
            &log_path,
            &runtime_proxy_structured_log_message(
                "runtime_config_compatibility_default",
                [runtime_proxy_log_field("key", *key)],
            ),
        );
    }
    Ok(log_path)
}
pub(super) fn runtime_local_rewrite_server(
    preferred_listen_addr: Option<&str>,
) -> Result<(Arc<TinyServer>, std::net::SocketAddr)> {
    let bind_addr = preferred_listen_addr.unwrap_or("127.0.0.1:0");
    let server = Arc::new(TinyServer::http(bind_addr).map_err(|err| {
        anyhow::anyhow!("failed to bind runtime local rewrite proxy on {bind_addr}: {err}")
    })?);
    let listen_addr = server
        .server_addr()
        .to_ip()
        .context("runtime local rewrite proxy did not expose a TCP listen address")?;
    Ok((server, listen_addr))
}

fn runtime_gateway_siem_wait(shutdown: &AtomicBool) {
    for _ in 0..50 {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn runtime_gateway_siem_postgres_loop(
    siem_worker: Arc<RuntimeSiemWorkerConfig>,
    repository: prodex_storage_postgres_runtime::PostgresRepository,
    runtime: tokio::runtime::Handle,
    governance_authority: Option<RuntimeGovernanceAuthority>,
    shutdown: Arc<AtomicBool>,
    log_path: PathBuf,
) {
    while !shutdown.load(Ordering::SeqCst) {
        let now_unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        let tenant_ids = governance_authority
            .as_ref()
            .ok_or(prodex_storage::GovernanceRepositoryError::Database)
            .and_then(RuntimeGovernanceAuthority::tenant_ids);
        let (status, phase) = match tenant_ids {
            Ok(tenant_ids)
                if siem_worker
                    .run_once_postgres(&repository, &runtime, &tenant_ids, now_unix_ms)
                    .is_ok() =>
            {
                ("success", "export")
            }
            Ok(_) => ("error", "export"),
            Err(_) => ("error", "tenant_discovery"),
        };
        runtime_proxy_log_to_path(
            &log_path,
            &format!("governance_siem_worker status={status} backend=postgres phase={phase}"),
        );
        runtime_gateway_siem_wait(&shutdown);
    }
}

fn runtime_gateway_siem_sqlite_iteration(
    siem_worker: &RuntimeSiemWorkerConfig,
    repository: &prodex_storage_sqlite_runtime::GovernanceSqliteRepository,
    log_path: &std::path::Path,
    now_unix_ms: u64,
) {
    match siem_worker.run_once(repository, now_unix_ms) {
        Ok(report) => match repository.aggregate_outbox_health().and_then(|health| {
            siem_worker
                .plan_health(health, now_unix_ms)
                .map_err(|_| prodex_storage_sqlite_runtime::GovernanceRepositoryError::Database)
        }) {
            Ok(metric) => {
                crate::record_runtime_siem_outbox_health_metric(&metric);
                let status = metric
                    .status_label
                    .as_metric_label()
                    .map(|(_, value)| value)
                    .unwrap_or("error");
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "governance_siem_worker status=success selected={} delivered={} retried={} dead_lettered={} {}={} {}={} {}={} health={status}",
                        report.selected,
                        report.delivered,
                        report.retried,
                        report.dead_lettered,
                        metric.pending_metric_name,
                        metric.pending,
                        metric.dead_letter_metric_name,
                        metric.dead_lettered,
                        metric.lag_metric_name,
                        metric.lag_milliseconds,
                    ),
                );
            }
            Err(_) => runtime_proxy_log_to_path(
                log_path,
                "governance_siem_worker status=error code=health_unavailable",
            ),
        },
        Err(_) => runtime_proxy_log_to_path(
            log_path,
            "governance_siem_worker status=error code=outbox_unavailable",
        ),
    }
}

fn runtime_gateway_siem_sqlite_loop(
    siem_worker: Arc<RuntimeSiemWorkerConfig>,
    repository: prodex_storage_sqlite_runtime::GovernanceSqliteRepository,
    shutdown: Arc<AtomicBool>,
    log_path: PathBuf,
) {
    while !shutdown.load(Ordering::SeqCst) {
        let now_unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        runtime_gateway_siem_sqlite_iteration(&siem_worker, &repository, &log_path, now_unix_ms);
        runtime_gateway_siem_wait(&shutdown);
    }
}

fn spawn_runtime_gateway_siem_worker(
    shared: &RuntimeLocalRewriteProxyShared,
    shutdown: &Arc<AtomicBool>,
) -> Result<Option<thread::JoinHandle<()>>> {
    let Some(siem_worker) = shared.gateway_observability.siem_worker.clone() else {
        return Ok(None);
    };
    let worker = match &shared.gateway_state_store {
        RuntimeGatewayStateStore::Postgres { .. } => {
            let repository = shared
                .gateway_postgres_repository
                .clone()
                .ok_or_else(|| anyhow::anyhow!("failed to open the SIEM governance outbox"))?;
            let governance_authority = shared.governance_authority.clone();
            let runtime = shared.runtime_shared.async_runtime.handle().clone();
            let shutdown = Arc::clone(shutdown);
            let log_path = shared.runtime_shared.log_path.clone();
            thread::spawn(move || {
                runtime_gateway_siem_postgres_loop(
                    siem_worker,
                    repository,
                    runtime,
                    governance_authority,
                    shutdown,
                    log_path,
                )
            })
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            let repository = prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path)
                .map_err(|_| anyhow::anyhow!("failed to open the SIEM governance outbox"))?;
            let shutdown = Arc::clone(shutdown);
            let log_path = shared.runtime_shared.log_path.clone();
            thread::spawn(move || {
                runtime_gateway_siem_sqlite_loop(siem_worker, repository, shutdown, log_path)
            })
        }
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            anyhow::bail!("configured SIEM worker requires a durable governance outbox")
        }
    };
    Ok(Some(worker))
}

fn runtime_gateway_spawn_core_workers(
    shared: &RuntimeLocalRewriteProxyShared,
    shutdown: &Arc<AtomicBool>,
    worker_threads: &mut Vec<thread::JoinHandle<()>>,
) -> Result<()> {
    if let Some(worker) = spawn_runtime_gateway_governance_invalidation_worker(shared, shutdown)? {
        worker_threads.push(worker);
    }
    if let Some(worker) = spawn_runtime_gateway_reservation_recovery_worker(shared, shutdown)? {
        worker_threads.push(worker);
    }
    if let Some(authority) = shared.governance_authority.clone() {
        worker_threads.push(
            shared
                .governance_sessions
                .spawn_durable_bank(
                    authority.clone(),
                    shared.runtime_shared.runtime_config.governance.mode
                        == prodex_config::GovernanceMode::BankEnforce,
                    Arc::clone(shutdown),
                )
                .map_err(|_| anyhow::anyhow!("failed to start governance session bank"))?,
        );
        worker_threads.push(
            shared
                .governance_audit_writer
                .spawn(authority, Arc::clone(shutdown))
                .map_err(|_| anyhow::anyhow!("failed to start governance audit writer"))?,
        );
    }
    if let Some(worker) = spawn_runtime_gateway_governance_refresh_worker(shared, shutdown) {
        worker_threads.push(worker);
    }
    if let Some(worker) = spawn_runtime_gateway_siem_worker(shared, shutdown)? {
        worker_threads.push(worker);
    }
    Ok(())
}

pub(in crate::runtime_launch::proxy_startup) fn spawn_runtime_local_rewrite_workers(
    shared: &RuntimeLocalRewriteProxyShared,
    server: Option<&Arc<TinyServer>>,
    shutdown: &Arc<AtomicBool>,
    worker_count: usize,
    secret_refresh: Option<RuntimeGatewayCredentialRefreshPlan>,
    spawn_gemini_sidecar_listener: bool,
) -> Result<RuntimeLocalRewriteWorkers> {
    let mut worker_threads = Vec::new();
    runtime_gateway_spawn_core_workers(shared, shutdown, &mut worker_threads)?;
    if let Some(pool) = shared.gemini_oauth_pool.as_ref()
        && let Some(worker) = pool.spawn_quota_refresh(shared.runtime_shared.log_path.clone())
    {
        worker_threads.push(worker);
    }
    if shared.gateway_sso.oidc.is_some() || shared.gateway_sso.workload_identity.is_some() {
        let shared = shared.clone();
        let shutdown = Arc::clone(shutdown);
        worker_threads.push(thread::spawn(move || {
            runtime_gateway_run_oidc_background_refresh_loop(shared, shutdown);
        }));
    }
    if let Some(secret_refresh) = secret_refresh {
        worker_threads.push(runtime_gateway_spawn_secret_refresh(
            shared.clone(),
            Arc::clone(shutdown),
            secret_refresh,
        ));
    }
    let gemini_live_sidecar_addr = if spawn_gemini_sidecar_listener
        && shared
            .runtime_shared
            .runtime_config
            .governance
            .mode
            .allows_anonymous_compatibility()
        && matches!(
            shared.provider.as_ref(),
            RuntimeLocalRewriteProviderOptions::Gemini { .. }
        ) {
        Some(spawn_runtime_gemini_live_sidecar(
            shared.clone(),
            Arc::clone(shutdown),
            &mut worker_threads,
        )?)
    } else {
        None
    };
    for worker_index in 0..worker_count {
        let Some(server) = server else {
            break;
        };
        let worker = spawn_runtime_local_rewrite_listener_worker(
            worker_index,
            Arc::clone(server),
            Arc::clone(shutdown),
            shared.clone(),
        );
        match worker {
            Ok(worker) => worker_threads.push(worker),
            Err(err) => {
                shutdown.store(true, Ordering::SeqCst);
                for _ in 0..worker_index {
                    server.unblock();
                }
                return Err(err.into());
            }
        }
    }
    Ok(RuntimeLocalRewriteWorkers {
        worker_threads,
        gemini_live_sidecar_addr,
    })
}
