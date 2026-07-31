use super::{
    RuntimeGatewayStateStore, RuntimeGovernanceArtifactRefreshOutcome, RuntimeGovernanceAuthority,
    RuntimeLocalRewriteProxyShared,
};
use crate::runtime_background::try_spawn_runtime_supervised_worker;
use crate::runtime_core_shared::runtime_proxy_log_to_path;
use postgres::fallible_iterator::FallibleIterator;
use prodex_storage::{
    GOVERNANCE_INVALIDATION_CHANNEL, GovernanceRepositoryError,
    MAX_GOVERNANCE_INVALIDATION_PAYLOAD_BYTES,
};
use prodex_storage_postgres_runtime::PostgresGovernanceInvalidation;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const GOVERNANCE_INVALIDATION_BATCH: u16 = 64;
const GOVERNANCE_INVALIDATION_LISTEN_TIMEOUT: Duration = Duration::from_millis(250);
const GOVERNANCE_INVALIDATION_OUTBOX_POLL_INTERVAL: Duration = Duration::from_secs(5);
const GOVERNANCE_INVALIDATION_RECONNECT_DELAY: Duration = Duration::from_secs(5);

pub(super) fn spawn_runtime_gateway_governance_invalidation_worker(
    shared: &RuntimeLocalRewriteProxyShared,
    shutdown: &Arc<AtomicBool>,
) -> std::io::Result<Option<thread::JoinHandle<()>>> {
    let RuntimeGatewayStateStore::Postgres { url, tls, .. } = &shared.gateway_state_store else {
        return Ok(None);
    };
    if !matches!(
        shared.governance_authority.as_ref(),
        Some(RuntimeGovernanceAuthority::Postgres { .. })
    ) {
        return Ok(None);
    }
    let url = url.clone();
    let tls = tls.clone();
    let replica_id = format!("gateway-{}", uuid::Uuid::now_v7());
    let shared = shared.clone();
    let shutdown = Arc::clone(shutdown);
    let log_path = shared.runtime_shared.log_path.clone();
    try_spawn_runtime_supervised_worker(
        "prodex-governance-invalidation-listener",
        log_path,
        Arc::clone(&shutdown),
        move || {
            if listen_for_governance_invalidations(&url, &tls, &replica_id, &shared, &shutdown)
                .is_err()
            {
                runtime_proxy_log_to_path(
                    &shared.runtime_shared.log_path,
                    "governance_invalidation_listener status=error action=reconnect",
                );
            }
            wait_for_reconnect(&shutdown);
        },
    )
    .map(Some)
}

fn listen_for_governance_invalidations(
    url: &str,
    tls: &prodex_storage_postgres_runtime::PostgresTlsConfig,
    replica_id: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    shutdown: &AtomicBool,
) -> Result<(), ()> {
    let mut client = prodex_storage_postgres_runtime::connect_blocking(url, tls).map_err(|_| ())?;
    client
        .batch_execute(&format!("LISTEN {GOVERNANCE_INVALIDATION_CHANNEL}"))
        .map_err(|_| ())?;
    drain_governance_invalidation_outbox(replica_id, shared);
    let mut last_poll = Instant::now();
    while !shutdown.load(Ordering::Acquire) {
        let notification = client
            .notifications()
            .timeout_iter(GOVERNANCE_INVALIDATION_LISTEN_TIMEOUT)
            .next()
            .map_err(|_| ())?;
        let hinted = notification.is_some_and(|notification| {
            notification.channel() == GOVERNANCE_INVALIDATION_CHANNEL
                && valid_governance_invalidation_hint(notification.payload())
        });
        if !hinted && client.is_closed() {
            return Err(());
        }
        if hinted || last_poll.elapsed() >= GOVERNANCE_INVALIDATION_OUTBOX_POLL_INTERVAL {
            drain_governance_invalidation_outbox(replica_id, shared);
            last_poll = Instant::now();
        }
    }
    Ok(())
}

fn drain_governance_invalidation_outbox(replica_id: &str, shared: &RuntimeLocalRewriteProxyShared) {
    let Some(
        authority @ RuntimeGovernanceAuthority::Postgres {
            repository,
            runtime,
            ..
        },
    ) = shared.governance_authority.as_ref()
    else {
        return;
    };
    let tenant_ids = match authority.tenant_ids() {
        Ok(tenant_ids) => tenant_ids,
        Err(_) => {
            log_poll_fallback(shared);
            return;
        }
    };
    let mut failed = false;
    for tenant_id in tenant_ids {
        let events = match runtime.block_on(repository.governance_poll_invalidation_outbox(
            tenant_id,
            replica_id,
            GOVERNANCE_INVALIDATION_BATCH,
        )) {
            Ok(events) => events,
            Err(_) => {
                failed = true;
                continue;
            }
        };
        for event in events {
            if process_governance_invalidation_event(
                event,
                &shared.governance_refresh_requested,
                |event| {
                    shared.refresh_committed_governance_artifact_kind(event.tenant_id, event.kind)
                },
                |event| {
                    runtime.block_on(
                        repository.governance_ack_invalidation_outbox_event(replica_id, event),
                    )
                },
            )
            .is_err()
            {
                failed = true;
            }
        }
        if runtime
            .block_on(repository.governance_compact_invalidation_outbox(tenant_id))
            .is_err()
        {
            failed = true;
        }
    }
    if failed {
        log_poll_fallback(shared);
    }
}

fn process_governance_invalidation_event(
    event: PostgresGovernanceInvalidation,
    refresh_requested: &AtomicBool,
    refresh: impl FnOnce(
        PostgresGovernanceInvalidation,
    )
        -> Result<RuntimeGovernanceArtifactRefreshOutcome, GovernanceRepositoryError>,
    acknowledge: impl FnOnce(PostgresGovernanceInvalidation) -> Result<(), GovernanceRepositoryError>,
) -> Result<(), ()> {
    let result = refresh(event);
    refresh_requested.store(true, Ordering::Release);
    result.map_err(|_| ())?;
    acknowledge(event).map_err(|_| ())
}

fn valid_governance_invalidation_hint(payload: &str) -> bool {
    !payload.is_empty() && payload.len() <= MAX_GOVERNANCE_INVALIDATION_PAYLOAD_BYTES
}

fn log_poll_fallback(shared: &RuntimeLocalRewriteProxyShared) {
    shared
        .governance_refresh_requested
        .store(true, Ordering::Release);
    runtime_proxy_log_to_path(
        &shared.runtime_shared.log_path,
        "governance_invalidation_outbox status=error action=poll_fallback",
    );
}

fn wait_for_reconnect(shutdown: &AtomicBool) {
    let waits = GOVERNANCE_INVALIDATION_RECONNECT_DELAY
        .as_millis()
        .div_ceil(100);
    for _ in 0..waits {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_domain::TenantId;
    use prodex_storage::GovernanceArtifactKind;

    fn event() -> PostgresGovernanceInvalidation {
        PostgresGovernanceInvalidation {
            event_id: 1,
            tenant_id: "00000000-0000-4000-8000-000000000001"
                .parse::<TenantId>()
                .unwrap(),
            kind: GovernanceArtifactKind::Policy,
        }
    }

    #[test]
    fn notify_payload_is_only_a_bounded_wakeup_hint() {
        assert!(valid_governance_invalidation_hint("not-authoritative-json"));
        assert!(!valid_governance_invalidation_hint(""));
        assert!(!valid_governance_invalidation_hint(
            &"x".repeat(MAX_GOVERNANCE_INVALIDATION_PAYLOAD_BYTES + 1)
        ));
    }

    #[test]
    fn durable_event_is_acknowledged_only_after_refresh() {
        let refresh_requested = AtomicBool::new(false);
        let refreshed = Arc::new(AtomicBool::new(false));
        let refreshed_for_refresh = Arc::clone(&refreshed);
        let refreshed_for_ack = Arc::clone(&refreshed);
        let mut acknowledged = false;
        process_governance_invalidation_event(
            event(),
            &refresh_requested,
            |_| {
                refreshed_for_refresh.store(true, Ordering::Release);
                Ok(RuntimeGovernanceArtifactRefreshOutcome::Invalidated)
            },
            |_| {
                assert!(refreshed_for_ack.load(Ordering::Acquire));
                acknowledged = true;
                Ok(())
            },
        )
        .unwrap();
        assert!(acknowledged);
        assert!(refresh_requested.load(Ordering::Acquire));

        acknowledged = false;
        assert!(
            process_governance_invalidation_event(
                event(),
                &refresh_requested,
                |_| Err(GovernanceRepositoryError::Database),
                |_| {
                    acknowledged = true;
                    Ok(())
                },
            )
            .is_err()
        );
        assert!(
            !acknowledged,
            "failed refresh must remain pending for recovery"
        );
    }
}
