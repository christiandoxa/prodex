use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use prodex_application::{
    ApplicationExpiredReservationRecoveryRequest, plan_application_expired_reservation_recovery,
};
use prodex_domain::{RequestId, TenantContext, TenantId};
use prodex_gateway_core::GatewayExpiredReservationRecoveryRequest;
use prodex_observability::TraceContext;
use prodex_storage::{
    DurableStoreKind, ExpiredReservationRecoveryCommand, GovernanceRepositoryError,
};

use super::local_rewrite::{RuntimeGovernanceAuthority, RuntimeLocalRewriteProxyShared};
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use crate::runtime_core_shared::runtime_proxy_log_to_path;

const RECOVERY_BATCH_LIMIT: usize = 64;
const RECOVERY_INTERVAL: Duration = Duration::from_secs(5);
const SHUTDOWN_POLL: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RecoveryReport {
    selected: usize,
    released: usize,
}

pub(super) fn spawn_runtime_gateway_reservation_recovery_worker(
    shared: &RuntimeLocalRewriteProxyShared,
    shutdown: &Arc<AtomicBool>,
) -> Result<Option<thread::JoinHandle<()>>> {
    match &shared.gateway_state_store {
        RuntimeGatewayStateStore::Sqlite { path } => {
            let mut repository =
                prodex_storage_sqlite_runtime::SqliteAccountingRepository::open(path)
                    .context("failed to open SQLite reservation recovery repository")?;
            let shutdown = Arc::clone(shutdown);
            let log_path = shared.runtime_shared.log_path.clone();
            Ok(Some(thread::spawn(move || {
                while !shutdown.load(Ordering::SeqCst) {
                    match recover_sqlite_once(&mut repository, now_unix_ms()) {
                        Ok(report) if report.released > 0 => runtime_proxy_log_to_path(
                            &log_path,
                            &format!(
                                "gateway_reservation_recovery status=success backend=sqlite selected={} released={}",
                                report.selected, report.released
                            ),
                        ),
                        Ok(_) => {}
                        Err(_) => runtime_proxy_log_to_path(
                            &log_path,
                            "gateway_reservation_recovery status=error backend=sqlite",
                        ),
                    }
                    wait_for_next_run(&shutdown);
                }
            })))
        }
        RuntimeGatewayStateStore::Postgres { .. } => {
            let repository = shared
                .gateway_postgres_repository
                .clone()
                .context("failed to open PostgreSQL reservation recovery repository")?;
            let runtime = shared.runtime_shared.async_runtime.handle().clone();
            let governance_authority = shared.governance_authority.clone();
            let gateway_credentials = shared.gateway_credentials.clone();
            let shutdown = Arc::clone(shutdown);
            let log_path = shared.runtime_shared.log_path.clone();
            Ok(Some(thread::spawn(move || {
                while !shutdown.load(Ordering::SeqCst) {
                    match recovery_tenant_ids(governance_authority.as_ref(), &gateway_credentials) {
                        Ok(tenant_ids) => match runtime.block_on(recover_postgres_once(
                            &repository,
                            &tenant_ids,
                            now_unix_ms(),
                        )) {
                            Ok(report) if report.released > 0 => runtime_proxy_log_to_path(
                                &log_path,
                                &format!(
                                    "gateway_reservation_recovery status=success backend=postgres selected={} released={}",
                                    report.selected, report.released
                                ),
                            ),
                            Ok(_) => {}
                            Err(_) => runtime_proxy_log_to_path(
                                &log_path,
                                "gateway_reservation_recovery status=error backend=postgres phase=recovery",
                            ),
                        },
                        Err(_) => runtime_proxy_log_to_path(
                            &log_path,
                            "gateway_reservation_recovery status=error backend=postgres phase=tenant_discovery",
                        ),
                    }
                    wait_for_next_run(&shutdown);
                }
            })))
        }
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => Ok(None),
    }
}

fn recover_sqlite_once(
    repository: &mut prodex_storage_sqlite_runtime::SqliteAccountingRepository,
    now_unix_ms: u64,
) -> Result<RecoveryReport> {
    let candidates = repository
        .load_expired_reservations(now_unix_ms, RECOVERY_BATCH_LIMIT)
        .context("failed to load expired SQLite reservations")?;
    let mut report = RecoveryReport {
        selected: candidates.len(),
        released: 0,
    };
    for candidate in candidates {
        let command = ExpiredReservationRecoveryCommand {
            storage_key: candidate.storage_key,
            snapshot: candidate.snapshot,
            record: candidate.record,
            now_unix_ms,
        };
        plan_recovery(DurableStoreKind::Sqlite, command.clone())?;
        repository
            .release_expired(command)
            .context("failed to recover expired SQLite reservation")?;
        report.released += 1;
    }
    Ok(report)
}

async fn recover_postgres_once(
    repository: &prodex_storage_postgres_runtime::PostgresRepository,
    tenant_ids: &[TenantId],
    now_unix_ms: u64,
) -> Result<RecoveryReport> {
    let mut report = RecoveryReport::default();
    for tenant_id in tenant_ids {
        let candidates = repository
            .load_expired_reservations(*tenant_id, now_unix_ms, RECOVERY_BATCH_LIMIT)
            .await
            .context("failed to load expired PostgreSQL reservations")?;
        report.selected += candidates.len();
        for candidate in candidates {
            let command = ExpiredReservationRecoveryCommand {
                storage_key: candidate.storage_key,
                snapshot: candidate.snapshot,
                record: candidate.record,
                now_unix_ms,
            };
            plan_recovery(DurableStoreKind::Postgres, command.clone())?;
            repository
                .release_expired(command)
                .await
                .context("failed to recover expired PostgreSQL reservation")?;
            report.released += 1;
        }
    }
    Ok(report)
}

fn plan_recovery(
    durable_store: DurableStoreKind,
    command: ExpiredReservationRecoveryCommand,
) -> Result<()> {
    let tenant_id = command.record.tenant_id;
    let request_id = RequestId::new();
    let trace_id = request_id.to_string().replace('-', "");
    let trace_context = TraceContext::new(&trace_id, &trace_id[..16], "01")
        .context("failed to build reservation recovery trace context")?;
    plan_application_expired_reservation_recovery(ApplicationExpiredReservationRecoveryRequest {
        durable_store,
        gateway: GatewayExpiredReservationRecoveryRequest {
            tenant: TenantContext { tenant_id },
            request_id,
            recovery: command,
            trace_context,
        },
        coordination: None,
    })
    .context("failed to plan expired reservation recovery")?;
    Ok(())
}

fn recovery_tenant_ids(
    governance_authority: Option<&RuntimeGovernanceAuthority>,
    gateway_credentials: &super::local_rewrite_gateway_credentials::RuntimeGatewayCredentialState,
) -> std::result::Result<Vec<TenantId>, GovernanceRepositoryError> {
    let mut tenant_ids = BTreeSet::new();
    if let Some(authority) = governance_authority {
        tenant_ids.extend(authority.tenant_ids()?);
    }
    let snapshot = gateway_credentials.current.load_full();
    let keys = snapshot
        .virtual_keys
        .lock()
        .map_err(|_| GovernanceRepositoryError::Database)?;
    tenant_ids.extend(
        keys.iter()
            .filter_map(|entry| entry.tenant_id.as_deref())
            .filter_map(|value| value.parse::<TenantId>().ok()),
    );
    Ok(tenant_ids.into_iter().collect())
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn wait_for_next_run(shutdown: &AtomicBool) {
    let polls = RECOVERY_INTERVAL.as_millis() / SHUTDOWN_POLL.as_millis();
    for _ in 0..polls {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(SHUTDOWN_POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::{plan_recovery, recovery_tenant_ids};
    use crate::runtime_launch::proxy_startup::local_rewrite::RuntimeLocalRewriteProviderOptions;
    use crate::runtime_launch::proxy_startup::local_rewrite_gateway_config::{
        RuntimeGatewayGuardrailWebhookConfig, RuntimeGatewayObservabilityConfig,
        RuntimeGatewaySsoConfig,
    };
    use crate::runtime_launch::proxy_startup::local_rewrite_gateway_credentials::{
        RuntimeGatewayCredentialRefreshCandidate, RuntimeGatewayCredentialState,
        runtime_gateway_initial_credential_snapshot,
    };
    use prodex_domain::TenantId;
    use prodex_storage::{DurableStoreKind, ExpiredReservationRecoveryCommand};
    use std::sync::{Arc, Mutex};

    #[test]
    fn recovery_planning_uses_application_boundary_for_both_durable_stores() {
        let tenant_id = TenantId::new();
        let record = prodex_domain::ReservationRecord {
            tenant_id,
            reservation_id: prodex_domain::ReservationId::new(),
            call_id: prodex_domain::CallId::new(),
            reserved: prodex_domain::UsageAmount::new(1, 1),
            created_at_unix_ms: 1,
            expires_at_unix_ms: 2,
        };
        for durable_store in [DurableStoreKind::Sqlite, DurableStoreKind::Postgres] {
            plan_recovery(
                durable_store,
                ExpiredReservationRecoveryCommand {
                    storage_key: prodex_storage::TenantStorageKey::tenant(tenant_id),
                    snapshot: prodex_domain::BudgetSnapshot {
                        reserved: record.reserved,
                        committed: prodex_domain::UsageAmount::ZERO,
                    },
                    record,
                    now_unix_ms: 3,
                },
            )
            .unwrap();
        }
    }

    #[test]
    fn recovery_tenant_discovery_reports_poisoned_virtual_key_state() {
        let virtual_keys = Arc::new(Mutex::new(Vec::new()));
        let poisoned = Arc::clone(&virtual_keys);
        assert!(
            std::thread::spawn(move || {
                let _guard = poisoned.lock().unwrap();
                panic!("poison virtual key lock");
            })
            .join()
            .is_err()
        );
        let credentials =
            RuntimeGatewayCredentialState::new(runtime_gateway_initial_credential_snapshot(
                RuntimeGatewayCredentialRefreshCandidate {
                    fingerprint: [0; 32],
                    provider: RuntimeLocalRewriteProviderOptions::OpenAiResponses {
                        api_keys: vec!["test-api-key".to_string()],
                    },
                    provider_credential: None,
                    auth_token_hash: None,
                    admin_tokens: Vec::new(),
                    sso: RuntimeGatewaySsoConfig::default(),
                    virtual_keys: Vec::new(),
                    guardrail_webhook: RuntimeGatewayGuardrailWebhookConfig::default(),
                    observability: RuntimeGatewayObservabilityConfig::default(),
                },
                virtual_keys,
            ));

        assert!(recovery_tenant_ids(None, &credentials).is_err());
    }
}
