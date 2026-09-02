use super::{
    Arc, AtomicBool, BTreeMap, BTreeSet, Context as _, Duration, Mutex, Ordering, PathBuf, Result,
    RuntimeConfig, RuntimeGatewayCredentialRefreshPlan, RuntimeGatewayReconciliationQueue,
    RuntimeGatewayStateStore, RuntimeGatewayVirtualKeyUsageState, RuntimeGovernanceAuthority,
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared, RuntimeSiemWorkerConfig,
    TinyServer, initialize_runtime_proxy_log_path_from_config,
    runtime_gateway_run_oidc_background_refresh_loop, runtime_gateway_spawn_secret_refresh,
    runtime_proxy_log_field, runtime_proxy_log_to_path, runtime_proxy_structured_log_message,
    spawn_runtime_gateway_governance_invalidation_worker,
    spawn_runtime_gateway_governance_refresh_worker,
    spawn_runtime_gateway_reservation_recovery_worker, spawn_runtime_gemini_live_sidecar,
    spawn_runtime_local_rewrite_listener_worker, thread,
};

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

fn aggregate_siem_outbox_health(
    health: impl IntoIterator<Item = prodex_storage::GovernanceOutboxHealth>,
) -> prodex_storage::GovernanceOutboxHealth {
    health.into_iter().fold(
        prodex_storage::GovernanceOutboxHealth::default(),
        |mut total, health| {
            total.pending = total.pending.saturating_add(health.pending);
            total.dead_lettered = total.dead_lettered.saturating_add(health.dead_lettered);
            total.oldest_pending_at_unix_ms = [
                total.oldest_pending_at_unix_ms,
                health.oldest_pending_at_unix_ms,
            ]
            .into_iter()
            .flatten()
            .min();
            total
        },
    )
}

fn aggregate_siem_outbox_health_results(
    health: impl IntoIterator<
        Item = Result<
            prodex_storage::GovernanceOutboxHealth,
            prodex_storage::GovernanceRepositoryError,
        >,
    >,
) -> Result<prodex_storage::GovernanceOutboxHealth, prodex_storage::GovernanceRepositoryError> {
    health
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map(aggregate_siem_outbox_health)
}

fn runtime_gateway_siem_postgres_health(
    repository: &prodex_storage_postgres_runtime::PostgresRepository,
    runtime: &tokio::runtime::Handle,
    tenant_ids: &[prodex_domain::TenantId],
) -> Result<prodex_storage::GovernanceOutboxHealth, prodex_storage::GovernanceRepositoryError> {
    aggregate_siem_outbox_health_results(
        tenant_ids
            .iter()
            .map(|tenant_id| runtime.block_on(repository.governance_outbox_health(*tenant_id))),
    )
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
        runtime_gateway_siem_postgres_iteration(
            &siem_worker,
            &repository,
            &runtime,
            governance_authority.as_ref(),
            &log_path,
            now_unix_ms,
        );
        runtime_gateway_siem_wait(&shutdown);
    }
}

fn runtime_gateway_siem_postgres_iteration(
    siem_worker: &RuntimeSiemWorkerConfig,
    repository: &prodex_storage_postgres_runtime::PostgresRepository,
    runtime: &tokio::runtime::Handle,
    governance_authority: Option<&RuntimeGovernanceAuthority>,
    log_path: &std::path::Path,
    now_unix_ms: u64,
) -> (&'static str, &'static str) {
    let tenant_ids = governance_authority
        .ok_or(prodex_storage::GovernanceRepositoryError::Database)
        .and_then(RuntimeGovernanceAuthority::tenant_ids);
    let (status, phase) = match tenant_ids {
        Ok(tenant_ids) => {
            match siem_worker.run_once_postgres(repository, runtime, &tenant_ids, now_unix_ms) {
                Ok(()) => {
                    match runtime_gateway_siem_postgres_health(repository, runtime, &tenant_ids)
                        .and_then(|health| {
                            siem_worker
                                .plan_health(health, now_unix_ms)
                                .map_err(|_| prodex_storage::GovernanceRepositoryError::Database)
                        }) {
                        Ok(metric) => {
                            crate::record_runtime_siem_outbox_health_metric(&metric);
                            ("success", "export")
                        }
                        Err(_) => ("error", "health"),
                    }
                }
                Err(_) => ("error", "export"),
            }
        }
        Err(_) => ("error", "tenant_discovery"),
    };
    runtime_proxy_log_to_path(
        log_path,
        &format!("governance_siem_worker status={status} backend=postgres phase={phase}"),
    );
    (status, phase)
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
    worker_threads: &mut Vec<thread::JoinHandle<()>>,
) -> Result<()> {
    let Some(siem_worker) = shared.gateway_observability.siem_worker.clone() else {
        return Ok(());
    };
    match &shared.gateway_state_store {
        RuntimeGatewayStateStore::Postgres { .. } => {
            let repository = shared
                .gateway_postgres_repository
                .clone()
                .ok_or_else(|| anyhow::anyhow!("failed to open the SIEM governance outbox"))?;
            let governance_authority = shared.governance_authority.clone();
            let runtime = shared.runtime_shared.async_runtime.handle().clone();
            let shutdown = Arc::clone(shutdown);
            let log_path = shared.runtime_shared.log_path.clone();
            worker_threads.push(thread::spawn(move || {
                runtime_gateway_siem_postgres_loop(
                    siem_worker,
                    repository,
                    runtime,
                    governance_authority,
                    shutdown,
                    log_path,
                )
            }));
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            let repository = prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path)
                .map_err(|_| anyhow::anyhow!("failed to open the SIEM governance outbox"))?;
            let shutdown = Arc::clone(shutdown);
            let log_path = shared.runtime_shared.log_path.clone();
            worker_threads.push(thread::spawn(move || {
                runtime_gateway_siem_sqlite_loop(siem_worker, repository, shutdown, log_path)
            }));
        }
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            anyhow::bail!("configured SIEM worker requires a durable governance outbox")
        }
    }
    Ok(())
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
    spawn_runtime_gateway_siem_worker(shared, shutdown, worker_threads)?;
    Ok(())
}

pub(in crate::runtime_launch::proxy_startup) fn spawn_runtime_local_rewrite_workers(
    shared: &RuntimeLocalRewriteProxyShared,
    server: Option<&Arc<TinyServer>>,
    shutdown: &Arc<AtomicBool>,
    worker_count: usize,
    secret_refresh: Option<RuntimeGatewayCredentialRefreshPlan>,
    #[cfg(test)] listener_ready: Option<std::sync::mpsc::Sender<()>>,
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
            #[cfg(test)]
            listener_ready.clone(),
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

#[cfg(test)]
mod tests {
    use super::{
        RuntimeGovernanceAuthority, RuntimeSiemWorkerConfig, aggregate_siem_outbox_health,
        aggregate_siem_outbox_health_results, runtime_gateway_siem_postgres_health,
        runtime_gateway_siem_postgres_iteration,
    };
    use std::collections::BTreeSet;
    use std::sync::Arc;

    #[test]
    fn postgres_siem_health_aggregation_sums_tenants_and_keeps_oldest_pending() {
        let health = aggregate_siem_outbox_health([
            prodex_storage::GovernanceOutboxHealth {
                pending: 2,
                dead_lettered: 1,
                oldest_pending_at_unix_ms: Some(900),
            },
            prodex_storage::GovernanceOutboxHealth {
                pending: 3,
                dead_lettered: 0,
                oldest_pending_at_unix_ms: Some(1_100),
            },
            prodex_storage::GovernanceOutboxHealth {
                pending: 0,
                dead_lettered: 2,
                oldest_pending_at_unix_ms: None,
            },
        ]);

        assert_eq!(health.pending, 5);
        assert_eq!(health.dead_lettered, 3);
        assert_eq!(health.oldest_pending_at_unix_ms, Some(900));
    }

    #[test]
    fn postgres_siem_health_aggregation_saturates_counters() {
        let health = aggregate_siem_outbox_health([
            prodex_storage::GovernanceOutboxHealth {
                pending: u64::MAX,
                dead_lettered: u64::MAX,
                oldest_pending_at_unix_ms: None,
            },
            prodex_storage::GovernanceOutboxHealth {
                pending: 1,
                dead_lettered: 1,
                oldest_pending_at_unix_ms: None,
            },
        ]);

        assert_eq!(health.pending, u64::MAX);
        assert_eq!(health.dead_lettered, u64::MAX);
        assert_eq!(health.oldest_pending_at_unix_ms, None);
    }

    #[test]
    fn postgres_siem_health_propagates_a_tenant_error() {
        let result = aggregate_siem_outbox_health_results([
            Ok::<_, prodex_storage::GovernanceRepositoryError>(
                prodex_storage::GovernanceOutboxHealth {
                    pending: 1,
                    dead_lettered: 0,
                    oldest_pending_at_unix_ms: Some(1),
                },
            ),
            Err(prodex_storage::GovernanceRepositoryError::Database),
        ]);

        assert!(matches!(
            result,
            Err(prodex_storage::GovernanceRepositoryError::Database)
        ));
    }

    #[test]
    #[ignore = "requires PRODEX_TEST_POSTGRES_URL"]
    fn postgres_siem_health_uses_all_authority_tenants_and_publishes_the_plan() {
        let url = std::env::var("PRODEX_TEST_POSTGRES_URL")
            .expect("PRODEX_TEST_POSTGRES_URL must point to the test PostgreSQL instance");
        let tls = prodex_storage_postgres_runtime::PostgresTlsConfig::explicit_disable();
        crate::runtime_launch::runtime_gateway_postgres_migrate_enterprise_state(&url, &tls)
            .expect("postgres enterprise migrations should apply");
        crate::runtime_launch::runtime_gateway_postgres_migrate_compatibility_state(&url, &tls)
            .expect("postgres compatibility migrations should apply");

        let config = prodex_storage_postgres_runtime::PostgresRuntimeConfig::new(&url, 4)
            .expect("postgres test config should be valid");
        let pool = config
            .create_pool_explicit_no_tls()
            .expect("postgres test pool should build");
        let repository = prodex_storage_postgres_runtime::PostgresRepository::from_pool_with_config(
            pool.clone(),
            &config,
        );
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("postgres test runtime should build"),
        );
        let tenant_ids = [
            prodex_domain::TenantId::new(),
            prodex_domain::TenantId::new(),
            prodex_domain::TenantId::new(),
        ];

        runtime.block_on(async {
            for (tenant_id, (pending, dead_lettered)) in
                tenant_ids.iter().zip([(2, 1), (1, 2), (0, 0)])
            {
                let mut client = pool.get().await.expect("postgres pool should connect");
                let transaction = client
                    .transaction()
                    .await
                    .expect("tenant setup transaction should start");
                transaction
                    .query_one(
                        prodex_storage_postgres::SET_TENANT_STATEMENT.sql,
                        &[&tenant_id.to_string()],
                    )
                    .await
                    .expect("tenant context should be set");
                transaction
                    .query_one(
                        prodex_storage_postgres::UPSERT_TENANT_LIFECYCLE_STATEMENT.sql,
                        &[
                            &tenant_id.as_uuid(),
                            &"SIEM health test tenant",
                            &1_800_000_000_000_i64,
                        ],
                    )
                    .await
                    .expect("tenant should be created");
                for index in 0..pending {
                    let event_id = prodex_domain::AuditEventId::new();
                    let envelope = format!(r#"{{"event_id":"{event_id}"}}"#);
                    transaction
                        .execute(
                            "INSERT INTO prodex_audit_log
                             (tenant_id, audit_event_id, event_digest, occurred_at_unix_ms,
                              principal_id, action, resource_kind, outcome)
                             VALUES ($1, $2, $3, $4, $5, 'test', 'siem', 'success')",
                            &[
                                &tenant_id.as_uuid(),
                                &event_id.as_uuid(),
                                &format!("siem-health-{event_id}"),
                                &(700_i64 + i64::from(index) * 500),
                                &prodex_domain::PrincipalId::new().as_uuid(),
                            ],
                        )
                        .await
                        .expect("audit event should be inserted");
                    transaction
                        .execute(
                            "INSERT INTO prodex_siem_outbox
                             (tenant_id, event_id, audit_event_id, event_envelope,
                              attempt_count, next_attempt_at_unix_ms, created_at_unix_ms)
                             VALUES ($1, $2, $2, $3::text::jsonb, 0, $4, $4)",
                            &[
                                &tenant_id.as_uuid(),
                                &event_id.as_uuid(),
                                &envelope,
                                &(700_i64 + i64::from(index) * 500),
                            ],
                        )
                        .await
                        .expect("SIEM outbox event should be inserted");
                }
                for _ in 0..dead_lettered {
                    let event_id = prodex_domain::AuditEventId::new();
                    let envelope = format!(r#"{{"event_id":"{event_id}"}}"#);
                    transaction
                        .execute(
                            "INSERT INTO prodex_audit_log
                             (tenant_id, audit_event_id, event_digest, occurred_at_unix_ms,
                              principal_id, action, resource_kind, outcome)
                             VALUES ($1, $2, $3, 900, $4, 'test', 'siem', 'failure')",
                            &[
                                &tenant_id.as_uuid(),
                                &event_id.as_uuid(),
                                &format!("siem-health-dead-{event_id}"),
                                &prodex_domain::PrincipalId::new().as_uuid(),
                            ],
                        )
                        .await
                        .expect("dead-letter audit event should be inserted");
                    transaction
                        .execute(
                            "INSERT INTO prodex_siem_dead_letters
                             (tenant_id, event_id, audit_event_id, event_envelope,
                              attempt_count, stable_reason_code, failed_at_unix_ms)
                             VALUES ($1, $2, $2, $3::text::jsonb, 1, 'test', 900)",
                            &[&tenant_id.as_uuid(), &event_id.as_uuid(), &envelope],
                        )
                        .await
                        .expect("SIEM dead letter should be inserted");
                }
                transaction
                    .commit()
                    .await
                    .expect("tenant SIEM fixture should commit");
            }
        });

        let health =
            runtime_gateway_siem_postgres_health(&repository, runtime.handle(), &tenant_ids)
                .expect("all authority tenant health reads should succeed");
        assert_eq!(health.pending, 3);
        assert_eq!(health.dead_lettered, 3);
        assert_eq!(health.oldest_pending_at_unix_ms, Some(700));

        let worker = Arc::new(RuntimeSiemWorkerConfig::for_health_tests(100));
        let metric = worker
            .plan_health(health, 1_000)
            .expect("aggregate health should produce a metric plan");
        assert_eq!(metric.pending, 3);
        assert_eq!(metric.dead_lettered, 3);
        assert_eq!(metric.lag_milliseconds, 300);
        assert_eq!(
            metric.status_label.as_metric_label().unwrap().1,
            "dead_lettered"
        );

        let log_path = std::env::temp_dir().join(format!(
            "prodex-siem-health-{}.log",
            prodex_domain::RequestId::new()
        ));
        let authority = RuntimeGovernanceAuthority::Postgres {
            repository: repository.clone(),
            runtime: Arc::clone(&runtime),
            tenant_ids: Arc::new(std::sync::Mutex::new(
                tenant_ids.iter().copied().collect::<BTreeSet<_>>(),
            )),
        };
        let outcome = runtime_gateway_siem_postgres_iteration(
            &worker,
            &repository,
            runtime.handle(),
            Some(&authority),
            &log_path,
            1_000,
        );
        assert_eq!(outcome, ("success", "export"));
    }
}
