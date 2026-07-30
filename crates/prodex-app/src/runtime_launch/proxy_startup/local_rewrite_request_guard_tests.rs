use super::super::local_rewrite_gateway_usage::RuntimeGatewayUsageRequestGuard;
use super::{RuntimeGovernanceAuthority, runtime_gateway_try_reserve_background_task};
use prodex_domain::TenantId;
use prodex_storage::GovernanceRepositoryError;
use std::cell::Cell;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
fn gateway_usage_request_guard_releases_request_id() {
    let request_ids = Arc::new(Mutex::new(BTreeSet::from([7])));
    {
        let _guard = RuntimeGatewayUsageRequestGuard {
            request_ids: Arc::clone(&request_ids),
            reconciliation: super::RuntimeGatewayReconciliationQueue::new(),
            request_id: 7,
            terminal: None,
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
fn governance_tenant_lock_poison_is_recovered() {
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

    let discovered = TenantId::new();
    assert_eq!(authority.tenant_ids().unwrap(), Vec::<TenantId>::new());
    authority.merge_tenant_ids([discovered]).unwrap();
    assert_eq!(authority.tenant_ids().unwrap(), vec![discovered]);
}

#[test]
fn governance_panicking_commit_does_not_permanently_poison_authority() {
    let tenant_id = TenantId::new();
    let authority = RuntimeGovernanceAuthority::Sqlite {
        path: "unused.sqlite".into(),
        tenant_ids: Arc::new(Mutex::new(BTreeSet::new())),
    };

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = authority
                .commit_for_tenant::<()>(tenant_id, || panic!("fail one governance publication"));
        }))
        .is_err()
    );
    assert!(authority.tenant_ids().unwrap().is_empty());

    assert_eq!(
        authority.commit_for_tenant(tenant_id, || Ok("published")),
        Ok("published")
    );
    assert_eq!(authority.tenant_ids().unwrap(), vec![tenant_id]);
}

#[test]
fn governance_lifecycle_cache_publication_is_serialized_with_mutation() {
    let tenant_id = TenantId::new();
    let authority = RuntimeGovernanceAuthority::Sqlite {
        path: "unused.sqlite".into(),
        tenant_ids: Arc::new(Mutex::new(BTreeSet::from([tenant_id]))),
    };
    let (first_entered_tx, first_entered_rx) = std::sync::mpsc::channel();
    let (release_first_tx, release_first_rx) = std::sync::mpsc::channel();
    let first_authority = authority.clone();
    let first = std::thread::spawn(move || {
        first_authority
            .commit_for_tenant(tenant_id, || {
                first_entered_tx.send(()).unwrap();
                release_first_rx.recv().unwrap();
                Ok(())
            })
            .unwrap();
    });
    first_entered_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap();

    let (second_entered_tx, second_entered_rx) = std::sync::mpsc::channel();
    let second = std::thread::spawn(move || {
        authority
            .commit_for_tenant(tenant_id, || {
                second_entered_tx.send(()).unwrap();
                Ok(())
            })
            .unwrap();
    });
    assert!(
        second_entered_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );

    release_first_tx.send(()).unwrap();
    second_entered_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    first.join().unwrap();
    second.join().unwrap();
}

#[test]
fn governance_cache_publication_is_serialized_across_tenants() {
    let first_tenant = TenantId::new();
    let second_tenant = TenantId::new();
    let authority = RuntimeGovernanceAuthority::Sqlite {
        path: "unused.sqlite".into(),
        tenant_ids: Arc::new(Mutex::new(BTreeSet::from([first_tenant, second_tenant]))),
    };
    let (first_entered_tx, first_entered_rx) = std::sync::mpsc::channel();
    let (release_first_tx, release_first_rx) = std::sync::mpsc::channel();
    let first_authority = authority.clone();
    let first = std::thread::spawn(move || {
        first_authority
            .commit_for_tenant(first_tenant, || {
                first_entered_tx.send(()).unwrap();
                release_first_rx.recv().unwrap();
                Ok(())
            })
            .unwrap();
    });
    first_entered_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap();

    let (second_entered_tx, second_entered_rx) = std::sync::mpsc::channel();
    let second = std::thread::spawn(move || {
        authority
            .commit_for_tenant(second_tenant, || {
                second_entered_tx.send(()).unwrap();
                Ok(())
            })
            .unwrap();
    });
    assert!(
        second_entered_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    release_first_tx.send(()).unwrap();
    second_entered_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    first.join().unwrap();
    second.join().unwrap();
}
