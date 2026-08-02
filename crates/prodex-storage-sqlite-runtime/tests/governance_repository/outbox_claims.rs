use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn outbox_worker_claims_once_across_repository_instances() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let first_repository = database.repository();
    let second_repository = database.repository();
    let command = audit_outbox(
        tenant_id,
        &principal(tenant_id),
        "governance.worker.concurrent",
        1_000,
        None,
        "concurrent-digest",
    );
    let expected_event_id = command.outbox_event_id;
    first_repository.append_audit_outbox(command).unwrap();

    let entered_delivery = Arc::new(Barrier::new(2));
    let release_delivery = Arc::new(Barrier::new(2));
    let delivery_count = Arc::new(AtomicUsize::new(0));
    std::thread::scope(|scope| {
        let entered_delivery_for_worker = Arc::clone(&entered_delivery);
        let release_delivery_for_worker = Arc::clone(&release_delivery);
        let delivery_count_for_worker = Arc::clone(&delivery_count);
        let first = scope.spawn(move || {
            first_repository
                .run_siem_outbox_batch(
                    1_000,
                    1,
                    SiemOutboxRetryPolicy::bounded(3, 100, 1_000).unwrap(),
                    |event| {
                        assert_eq!(event.event_id, expected_event_id);
                        delivery_count_for_worker.fetch_add(1, Ordering::SeqCst);
                        entered_delivery_for_worker.wait();
                        release_delivery_for_worker.wait();
                        Ok::<(), ()>(())
                    },
                )
                .unwrap()
        });

        entered_delivery.wait();
        let second = second_repository
            .run_siem_outbox_batch(
                1_000,
                1,
                SiemOutboxRetryPolicy::bounded(3, 100, 1_000).unwrap(),
                |_event| {
                    delivery_count.fetch_add(1, Ordering::SeqCst);
                    Ok::<(), ()>(())
                },
            )
            .unwrap();
        assert_eq!(second.selected, 0);
        release_delivery.wait();

        let first = first.join().unwrap();
        assert_eq!(first.selected, 1);
        assert_eq!(first.delivered, 1);
    });
    assert_eq!(delivery_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        database
            .repository()
            .outbox_health(tenant_id)
            .unwrap()
            .pending,
        0
    );
}

#[test]
fn outbox_worker_leases_only_the_event_being_delivered() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let concurrent_repository = database.repository();
    let actor = principal(tenant_id);
    let mut audit = AuditCursor::default();
    for action in ["governance.worker.first", "governance.worker.second"] {
        repository
            .append_audit_outbox(audit.next(tenant_id, &actor, action))
            .unwrap();
    }

    let retry = SiemOutboxRetryPolicy::bounded(3, 100, 1_000).unwrap();
    let mut concurrently_claimed = Vec::new();
    let report = repository
        .run_siem_outbox_batch(2_000, 2, retry, |event| {
            let claim = concurrent_repository
                .claim_siem_outbox_batch(2_000, 1, 60_000)
                .unwrap()
                .into_iter()
                .next()
                .expect("the next event must remain available to another worker");
            assert_ne!(claim.event_id, event.event_id);
            concurrently_claimed.push(claim);
            Ok::<(), ()>(())
        })
        .unwrap();

    assert_eq!(report.selected, 1);
    assert_eq!(report.delivered, 1);
    assert_eq!(concurrently_claimed.len(), 1);
    assert_eq!(
        concurrent_repository.finalize_siem_outbox_claim(
            &concurrently_claimed[0],
            true,
            2_000,
            retry,
        ),
        Ok(prodex_storage::SiemOutboxDeliveryDecision::Delivered)
    );
    assert_eq!(repository.outbox_health(tenant_id).unwrap().pending, 0);
}

#[test]
fn expired_outbox_claim_is_reclaimed_and_stale_finalize_is_rejected() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    repository
        .append_audit_outbox(audit_outbox(
            tenant_id,
            &principal(tenant_id),
            "governance.worker.lease",
            1_000,
            None,
            "lease-digest",
        ))
        .unwrap();
    let retry = SiemOutboxRetryPolicy::bounded(3, 100, 1_000).unwrap();

    let first = repository.claim_siem_outbox_batch(1_000, 1, 100).unwrap();
    let first = &first[0];
    assert_eq!(
        repository.finalize_siem_outbox_claim(first, true, 1_100, retry),
        Err(GovernanceRepositoryError::Conflict)
    );
    let second = repository.claim_siem_outbox_batch(1_100, 1, 100).unwrap();
    let second = &second[0];
    assert_eq!(first.event_id, second.event_id);
    assert_ne!(first.claim_token, second.claim_token);
    assert_eq!(
        repository.finalize_siem_outbox_claim(first, true, 1_100, retry),
        Err(GovernanceRepositoryError::Conflict)
    );
    assert_eq!(
        repository.finalize_siem_outbox_claim(second, false, 1_100, retry),
        Ok(prodex_storage::SiemOutboxDeliveryDecision::RetryAt(1_200))
    );
    assert!(
        repository
            .claim_siem_outbox_batch(1_199, 1, 100)
            .unwrap()
            .is_empty()
    );

    let recovered = repository.claim_siem_outbox_batch(1_200, 1, 100).unwrap();
    assert_eq!(recovered.len(), 1);
    assert_ne!(recovered[0].claim_token, second.claim_token);
    assert_eq!(
        repository.finalize_siem_outbox_claim(&recovered[0], true, 1_200, retry),
        Ok(prodex_storage::SiemOutboxDeliveryDecision::Delivered)
    );
    assert_eq!(repository.outbox_health(tenant_id).unwrap().pending, 0);
}

#[test]
fn outbox_retries_with_stable_id_then_dead_letters() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let actor = principal(tenant_id);
    let command = audit_outbox(
        tenant_id,
        &actor,
        "governance.worker.test",
        1_000,
        None,
        "outbox-digest",
    );
    let expected_event_id = command.outbox_event_id;
    repository.append_audit_outbox(command).unwrap();
    assert_eq!(repository.aggregate_outbox_health().unwrap().pending, 1);
    let health = repository.outbox_health(tenant_id).unwrap();
    assert_eq!(health.pending, 1);
    assert_eq!(health.dead_lettered, 0);
    let integrity = repository.audit_integrity_health(tenant_id).unwrap();
    assert_eq!(integrity.event_count, 1);
    assert_eq!(integrity.chain_head_count, 1);
    assert!(integrity.chain_valid);
    let exported = repository.governance_export_audit(tenant_id, 10).unwrap();
    assert_eq!(exported.len(), 1);
    assert_eq!(exported[0].action, "governance.worker.test");
    assert_eq!(exported[0].resource_kind, "governance_revision");
    assert_eq!(
        repository.governance_export_audit(tenant_id, 0),
        Err(GovernanceRepositoryError::InvalidInput)
    );
    let retry = SiemOutboxRetryPolicy::bounded(3, 100, 1_000).unwrap();
    let started_at = std::time::Instant::now();
    let logical_now = |cumulative_backoff_ms: u64| {
        1_000 + u64::try_from(started_at.elapsed().as_millis()).unwrap() + cumulative_backoff_ms
    };
    let mut observed = Vec::new();
    let first = repository
        .run_siem_outbox_batch(logical_now(0), 1, retry, |event| {
            observed.push(event.event_id);
            Err::<(), ()>(())
        })
        .unwrap();
    assert_eq!(first.retried, 1);
    let second = repository
        .run_siem_outbox_batch(logical_now(100), 1, retry, |event| {
            observed.push(event.event_id);
            Err::<(), ()>(())
        })
        .unwrap();
    assert_eq!(second.retried, 1);
    let third = repository
        .run_siem_outbox_batch(logical_now(300), 1, retry, |event| {
            observed.push(event.event_id);
            Err::<(), ()>(())
        })
        .unwrap();
    assert_eq!(third.dead_lettered, 1);
    assert_eq!(observed, vec![expected_event_id; 3]);
    let connection = Connection::open(database.path()).unwrap();
    let dead_letters: i64 = connection
        .query_row("SELECT COUNT(*) FROM prodex_siem_dead_letters", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(dead_letters, 1);
}
