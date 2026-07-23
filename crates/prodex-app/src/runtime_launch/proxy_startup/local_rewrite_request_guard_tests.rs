use super::super::local_rewrite_gateway_usage::RuntimeGatewayUsageRequestGuard;
use super::{RuntimeGovernanceAuthority, runtime_gateway_try_reserve_background_task};
use prodex_domain::TenantId;
use prodex_storage::GovernanceRepositoryError;
use std::cell::Cell;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

#[test]
fn gateway_usage_request_guard_releases_request_id() {
    let request_ids = Arc::new(Mutex::new(BTreeSet::from([7])));
    {
        let _guard = RuntimeGatewayUsageRequestGuard {
            request_ids: Arc::clone(&request_ids),
            reconciliation: super::RuntimeGatewayReconciliationQueue::new(),
            request_id: 7,
        };
    }

    assert!(request_ids.lock().unwrap().is_empty());
}

#[test]
fn gateway_background_task_slots_are_bounded() {
    let slots = Arc::new(tokio::sync::Semaphore::new(1));
    let permit = runtime_gateway_try_reserve_background_task(&slots).unwrap();
    assert!(runtime_gateway_try_reserve_background_task(&slots).is_none());
    drop(permit);
    assert!(runtime_gateway_try_reserve_background_task(&slots).is_some());
}

#[test]
fn governance_tenant_capacity_is_reserved_before_commit() {
    let configured = (0..crate::runtime_governance::MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS)
        .map(|_| TenantId::new())
        .collect();
    let authority = RuntimeGovernanceAuthority::Sqlite {
        path: "unused.sqlite".into(),
        tenant_ids: Arc::new(Mutex::new(configured)),
    };
    let committed = Cell::new(false);

    assert_eq!(
        authority.commit_for_tenant(TenantId::new(), || {
            committed.set(true);
            Ok(())
        }),
        Err(GovernanceRepositoryError::SnapshotUnavailable)
    );
    assert!(!committed.get());
}

#[test]
fn governance_tenant_lock_failure_is_reported() {
    let tenant_ids = Arc::new(Mutex::new(BTreeSet::new()));
    let poisoned = Arc::clone(&tenant_ids);
    assert!(
        std::thread::spawn(move || {
            let _guard = poisoned.lock().unwrap();
            panic!("poison tenant lock");
        })
        .join()
        .is_err()
    );
    let authority = RuntimeGovernanceAuthority::Sqlite {
        path: "unused.sqlite".into(),
        tenant_ids,
    };

    assert_eq!(
        authority.tenant_ids(),
        Err(GovernanceRepositoryError::Database)
    );
}
