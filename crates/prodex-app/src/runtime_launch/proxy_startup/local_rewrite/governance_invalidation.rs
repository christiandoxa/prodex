use super::{
    RuntimeGatewayStateStore, RuntimeGovernanceArtifactRefreshOutcome, RuntimeGovernanceAuthority,
    RuntimeLocalRewriteProxyShared,
};
use crate::runtime_background::try_spawn_runtime_supervised_worker;
use crate::runtime_core_shared::runtime_proxy_log_to_path;
use postgres::fallible_iterator::FallibleIterator;
use prodex_domain::TenantId;
use prodex_storage::{
    GOVERNANCE_INVALIDATION_CHANNEL, GovernanceArtifactKind, GovernanceRepositoryError,
    MAX_GOVERNANCE_INVALIDATION_PAYLOAD_BYTES,
};
use serde::Deserialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const GOVERNANCE_INVALIDATION_LISTEN_TIMEOUT: Duration = Duration::from_millis(250);
const GOVERNANCE_INVALIDATION_RECONNECT_DELAY: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GovernanceInvalidation {
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GovernanceInvalidationPayload {
    tenant_id: String,
    kind: String,
}

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
    let shared = shared.clone();
    let shutdown = Arc::clone(shutdown);
    let log_path = shared.runtime_shared.log_path.clone();
    try_spawn_runtime_supervised_worker(
        "prodex-governance-invalidation-listener",
        log_path,
        Arc::clone(&shutdown),
        move || {
            if listen_for_governance_invalidations(&url, &tls, &shared, &shutdown).is_err() {
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
    shared: &RuntimeLocalRewriteProxyShared,
    shutdown: &AtomicBool,
) -> Result<(), ()> {
    let mut client = prodex_storage_postgres_runtime::connect_blocking(url, tls).map_err(|_| ())?;
    client
        .batch_execute(&format!("LISTEN {GOVERNANCE_INVALIDATION_CHANNEL}"))
        .map_err(|_| ())?;
    while !shutdown.load(Ordering::Acquire) {
        let notification = client
            .notifications()
            .timeout_iter(GOVERNANCE_INVALIDATION_LISTEN_TIMEOUT)
            .next()
            .map_err(|_| ())?;
        let Some(notification) = notification else {
            if client.is_closed() {
                return Err(());
            }
            continue;
        };
        if notification.channel() != GOVERNANCE_INVALIDATION_CHANNEL {
            continue;
        }
        let Some(invalidation) = parse_governance_invalidation(notification.payload()) else {
            continue;
        };
        let Some(authority) = shared.governance_authority.as_ref() else {
            continue;
        };
        if !invalidation_targets_known_tenant(authority, &invalidation) {
            continue;
        }
        if apply_governance_invalidation(
            &invalidation,
            &shared.governance_refresh_requested,
            |event| shared.refresh_committed_governance_artifact_kind(event.tenant_id, event.kind),
        )
        .is_err()
        {
            runtime_proxy_log_to_path(
                &shared.runtime_shared.log_path,
                "governance_invalidation_listener status=error action=poll_fallback",
            );
        }
    }
    Ok(())
}

fn invalidation_targets_known_tenant(
    authority: &RuntimeGovernanceAuthority,
    invalidation: &GovernanceInvalidation,
) -> bool {
    authority
        .tenant_ids()
        .is_ok_and(|tenant_ids| tenant_ids.contains(&invalidation.tenant_id))
}

fn apply_governance_invalidation(
    invalidation: &GovernanceInvalidation,
    refresh_requested: &AtomicBool,
    refresh: impl FnOnce(
        GovernanceInvalidation,
    )
        -> Result<RuntimeGovernanceArtifactRefreshOutcome, GovernanceRepositoryError>,
) -> Result<(), ()> {
    let result = refresh(*invalidation);
    refresh_requested.store(true, Ordering::Release);
    result.map(|_| ()).map_err(|_| ())
}

fn parse_governance_invalidation(payload: &str) -> Option<GovernanceInvalidation> {
    if payload.is_empty() || payload.len() > MAX_GOVERNANCE_INVALIDATION_PAYLOAD_BYTES {
        return None;
    }
    let payload: GovernanceInvalidationPayload = serde_json::from_str(payload).ok()?;
    let tenant_id = payload.tenant_id.parse().ok()?;
    let kind = match payload.kind.as_str() {
        "policy" => GovernanceArtifactKind::Policy,
        "classification_rules" => GovernanceArtifactKind::ClassificationRules,
        "provider_registry" => GovernanceArtifactKind::ProviderRegistry,
        "routing_scores" => GovernanceArtifactKind::RoutingScores,
        _ => return None,
    };
    Some(GovernanceInvalidation { tenant_id, kind })
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
    use std::collections::BTreeSet;
    use std::sync::Mutex;

    fn tenant_id() -> TenantId {
        "00000000-0000-4000-8000-000000000001".parse().unwrap()
    }

    #[test]
    fn invalidation_payload_is_bounded_and_strict() {
        for (label, kind) in [
            ("policy", GovernanceArtifactKind::Policy),
            (
                "classification_rules",
                GovernanceArtifactKind::ClassificationRules,
            ),
            (
                "provider_registry",
                GovernanceArtifactKind::ProviderRegistry,
            ),
            ("routing_scores", GovernanceArtifactKind::RoutingScores),
        ] {
            let payload = format!(r#"{{"tenant_id":"{}","kind":"{label}"}}"#, tenant_id());
            assert_eq!(
                parse_governance_invalidation(&payload),
                Some(GovernanceInvalidation {
                    tenant_id: tenant_id(),
                    kind,
                })
            );
        }
        assert!(parse_governance_invalidation("{}").is_none());
        assert!(
            parse_governance_invalidation(
                r#"{"tenant_id":"bad","kind":"policy","unexpected":true}"#
            )
            .is_none()
        );
        assert!(
            parse_governance_invalidation(
                &"x".repeat(MAX_GOVERNANCE_INVALIDATION_PAYLOAD_BYTES + 1)
            )
            .is_none()
        );
    }

    #[test]
    fn unknown_tenant_notification_cannot_enroll_authority() {
        let known = tenant_id();
        let authority = RuntimeGovernanceAuthority::Sqlite {
            path: "unused.sqlite".into(),
            tenant_ids: Arc::new(Mutex::new(BTreeSet::from([known]))),
        };
        let unknown = GovernanceInvalidation {
            tenant_id: TenantId::new(),
            kind: GovernanceArtifactKind::Policy,
        };

        assert!(!invalidation_targets_known_tenant(&authority, &unknown));
        assert_eq!(authority.tenant_ids().unwrap(), vec![known]);
    }

    #[test]
    fn notification_reloads_latest_snapshot_and_wakes_recovery_poll() {
        let invalidation = GovernanceInvalidation {
            tenant_id: tenant_id(),
            kind: GovernanceArtifactKind::Policy,
        };
        let refresh_requested = AtomicBool::new(false);
        let mut cached_revision = "revoked-revision";
        let authoritative_revision = "promoted-fallback";

        apply_governance_invalidation(&invalidation, &refresh_requested, |event| {
            assert_eq!(event, invalidation);
            cached_revision = authoritative_revision;
            Ok(RuntimeGovernanceArtifactRefreshOutcome::Published)
        })
        .unwrap();

        assert_eq!(cached_revision, "promoted-fallback");
        assert!(refresh_requested.load(Ordering::Acquire));

        cached_revision = "later-activation";
        apply_governance_invalidation(&invalidation, &refresh_requested, |_| {
            cached_revision = "later-activation";
            Ok(RuntimeGovernanceArtifactRefreshOutcome::Published)
        })
        .unwrap();
        assert_eq!(cached_revision, "later-activation");
    }
}
