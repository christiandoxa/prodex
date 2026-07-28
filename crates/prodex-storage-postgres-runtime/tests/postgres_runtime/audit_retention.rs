use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires PRODEX_TEST_POSTGRES_URL"]
async fn postgres_audit_retention_mutations_replay_idempotently() {
    let url = std::env::var("PRODEX_TEST_POSTGRES_URL")
        .expect("PRODEX_TEST_POSTGRES_URL must point to the test PostgreSQL instance");
    let config = PostgresRuntimeConfig::new(url, 2).expect("test config should be valid");
    let pool = config
        .create_pool_explicit_no_tls()
        .expect("test pool should build");
    let repository = PostgresRepository::from_pool_with_config(pool.clone(), &config);
    let tenant_id = TenantId::new();
    create_tenant(&pool, tenant_id).await;
    let admin = governance_principal(tenant_id);

    let (fixture_audit, fixture_digest) =
        governance_audit(tenant_id, &admin, "governance.audit.fixture", 1_000, None);
    let fixture_id = fixture_audit.audit.event.id;
    repository
        .governance_append_audit_outbox(fixture_audit)
        .await
        .unwrap();

    let hold = AuditRetentionHold::new(
        TenantContext { tenant_id },
        fixture_id,
        AuditReasonCode::new("legal.investigation").unwrap(),
        None,
    );
    let (hold_audit, hold_digest) = governance_audit(
        tenant_id,
        &admin,
        "governance.audit_legal_hold.upsert",
        2_000,
        Some(fixture_digest),
    );
    let hold_audit_id = hold_audit.audit.event.id;
    let hold_idempotency = GovernanceMutationIdempotency {
        operation: IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("legal-hold-upsert").unwrap(),
            "sha256:legal-hold-upsert",
        )
        .unwrap(),
        started_at_unix_ms: 1_999,
    };
    repository
        .governance_upsert_audit_legal_hold_idempotent(
            hold.clone(),
            admin.id,
            2_000,
            hold_audit.clone(),
            hold_idempotency.clone(),
        )
        .await
        .unwrap();
    repository
        .governance_upsert_audit_legal_hold_idempotent(
            hold,
            admin.id,
            2_000,
            hold_audit,
            hold_idempotency,
        )
        .await
        .unwrap();

    let (delete_audit, delete_digest) = governance_audit(
        tenant_id,
        &admin,
        "governance.audit_legal_hold.delete",
        3_000,
        Some(hold_digest),
    );
    let delete_audit_id = delete_audit.audit.event.id;
    let delete_idempotency = GovernanceMutationIdempotency {
        operation: IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("legal-hold-delete").unwrap(),
            "sha256:legal-hold-delete",
        )
        .unwrap(),
        started_at_unix_ms: 2_999,
    };
    assert!(
        repository
            .governance_delete_audit_legal_hold_idempotent(
                tenant_id,
                fixture_id,
                delete_audit.clone(),
                delete_idempotency.clone(),
            )
            .await
            .unwrap()
    );
    assert!(
        repository
            .governance_delete_audit_legal_hold_idempotent(
                tenant_id,
                fixture_id,
                delete_audit,
                delete_idempotency,
            )
            .await
            .unwrap()
    );

    let (purge_audit, _) = governance_audit(
        tenant_id,
        &admin,
        "governance.audit_retention.purge",
        10_000,
        Some(delete_digest),
    );
    let purge_ids = vec![fixture_id, hold_audit_id, delete_audit_id];
    let purge_idempotency = GovernanceMutationIdempotency {
        operation: IdempotentOperation::new(
            tenant_id,
            IdempotencyKey::new("audit-retention-purge").unwrap(),
            "sha256:audit-retention-purge",
        )
        .unwrap(),
        started_at_unix_ms: 9_999,
    };
    let purged = repository
        .governance_purge_audit_events_idempotent(
            tenant_id,
            purge_ids.clone(),
            10_000,
            5_000,
            purge_audit.clone(),
            purge_idempotency.clone(),
        )
        .await
        .unwrap();
    assert_eq!(purged, purge_ids);
    assert_eq!(
        repository
            .governance_purge_audit_events_idempotent(
                tenant_id,
                purge_ids,
                10_000,
                5_000,
                purge_audit,
                purge_idempotency,
            )
            .await
            .unwrap(),
        purged
    );
    let audit = repository
        .governance_export_audit(tenant_id, 16)
        .await
        .unwrap();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].action, "governance.audit_retention.purge");
}
