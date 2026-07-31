use super::{
    RuntimeGatewayGovernanceRefreshContext, RuntimeGatewayGovernanceRefreshCounts,
    RuntimeGatewayGovernanceRefreshInputs, RuntimeGovernanceAuthority,
    RuntimeLocalRewriteProxyShared, runtime_gateway_discover_governance_tenants,
    runtime_gateway_refresh_tenant,
};
use crate::runtime_core_shared::runtime_proxy_log_to_path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

fn runtime_gateway_governance_refresh_wait(
    shutdown: &AtomicBool,
    refresh_requested: Option<&AtomicBool>,
) {
    for _ in 0..50 {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        if let Some(refresh_requested) = refresh_requested
            && refresh_requested.swap(false, Ordering::AcqRel)
        {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn runtime_gateway_log_governance_refresh(
    log_path: &std::path::Path,
    counts: RuntimeGatewayGovernanceRefreshCounts,
    tenant_count: usize,
) {
    if counts.changed() {
        runtime_proxy_log_to_path(
            log_path,
            &format!(
                "governance_snapshot_refresh status=success policy={} policy_unavailable={} classification_rules={} classification_rules_unavailable={} provider_registry={} provider_registry_unavailable={} routing_scores={} routing_scores_unavailable={} configured={}",
                counts.policy_refreshed,
                counts.policy_unavailable,
                counts.classification_refreshed,
                counts.classification_unavailable,
                counts.provider_refreshed,
                counts.provider_unavailable,
                counts.routing_refreshed,
                counts.routing_unavailable,
                tenant_count,
            ),
        );
    } else {
        runtime_proxy_log_to_path(
            log_path,
            "governance_snapshot_refresh status=error action=retain_lkg",
        );
    }
}

pub(crate) fn spawn_runtime_gateway_governance_refresh_worker(
    shared: &RuntimeLocalRewriteProxyShared,
    shutdown: &Arc<AtomicBool>,
) -> Option<thread::JoinHandle<()>> {
    let authority = shared.governance_authority.clone()?;
    let governance_snapshots = Arc::clone(&shared.process.governance_snapshots);
    let provider = shared.provider.clone();
    let provider_credential = shared.provider_credential.clone();
    let shutdown = Arc::clone(shutdown);
    let log_path = shared.runtime_shared.log_path.clone();
    let deployment_mode = shared.runtime_shared.runtime_config.governance.mode;
    let governance_policy = shared
        .runtime_shared
        .runtime_config
        .governance_policy
        .clone();
    let governance_refresh_requested = Arc::clone(&shared.governance_refresh_requested);
    Some(thread::spawn(move || {
        let sqlite_repository = match &authority {
            RuntimeGovernanceAuthority::Sqlite { path, .. } => {
                prodex_storage_sqlite_runtime::GovernanceSqliteRepository::open(path).ok()
            }
            RuntimeGovernanceAuthority::Postgres { .. } => None,
        };
        let inputs = RuntimeGatewayGovernanceRefreshInputs {
            authority: &authority,
            sqlite_repository: sqlite_repository.as_ref(),
            governance_policy: &governance_policy,
            deployment_mode,
            provider: &provider,
            provider_credential: provider_credential.as_ref(),
        };
        while !shutdown.load(Ordering::SeqCst) {
            let tenant_ids = match runtime_gateway_discover_governance_tenants(
                &authority,
                sqlite_repository.as_ref(),
            ) {
                Ok(tenant_ids) => tenant_ids,
                Err(_) => {
                    runtime_proxy_log_to_path(
                        &log_path,
                        "governance_snapshot_refresh status=error phase=tenant_discovery action=retain_lkg",
                    );
                    runtime_gateway_governance_refresh_wait(&shutdown, None);
                    continue;
                }
            };
            let mut counts = RuntimeGatewayGovernanceRefreshCounts::default();
            for tenant_id in tenant_ids.iter().copied() {
                let context = RuntimeGatewayGovernanceRefreshContext {
                    inputs: &inputs,
                    tenant_id,
                };
                if let Ok(refresh) = runtime_gateway_refresh_tenant(&context, &governance_snapshots)
                {
                    counts.add(refresh);
                }
            }
            runtime_gateway_log_governance_refresh(&log_path, counts, tenant_ids.len());
            runtime_gateway_governance_refresh_wait(&shutdown, Some(&governance_refresh_requested));
        }
    }))
}
