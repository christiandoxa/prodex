use super::*;

async fn publish_policy_pointer(pool: &Pool, tenant_id: TenantId, etag: &str) {
    let mut client = pool.get().await.unwrap();
    let transaction = client.transaction().await.unwrap();
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .unwrap();
    transaction
        .execute(
            r#"
            INSERT INTO prodex_policy_pointers (
                tenant_id, active_revision_id, last_known_good_revision_id,
                etag, updated_at_unix_ms
            )
            VALUES ($1, NULL, NULL, $2, 1)
            ON CONFLICT (tenant_id) DO UPDATE SET
                etag = EXCLUDED.etag,
                updated_at_unix_ms = prodex_policy_pointers.updated_at_unix_ms + 1
            "#,
            &[&tenant_id.as_uuid(), &etag],
        )
        .await
        .unwrap();
    transaction.commit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires PRODEX_TEST_POSTGRES_URL"]
async fn governance_invalidation_outbox_converges_replicas_and_compacts_safely() {
    let url = std::env::var("PRODEX_TEST_POSTGRES_URL")
        .expect("PRODEX_TEST_POSTGRES_URL must point to the test PostgreSQL instance");
    let config = PostgresRuntimeConfig::new(url, 4).unwrap();
    let pool = config.create_pool_explicit_no_tls().unwrap();
    let repository = PostgresRepository::from_pool_with_config(pool.clone(), &config);
    let tenant_id = TenantId::new();
    let other_tenant_id = TenantId::new();
    create_tenant(&pool, tenant_id).await;
    create_tenant(&pool, other_tenant_id).await;

    publish_policy_pointer(&pool, tenant_id, "etag-1").await;
    let gateway_a = repository
        .governance_poll_invalidation_outbox(tenant_id, "gateway-a", 16)
        .await
        .unwrap();
    let gateway_b = repository
        .governance_poll_invalidation_outbox(tenant_id, "gateway-b", 16)
        .await
        .unwrap();
    assert_eq!(gateway_a, gateway_b);
    assert_eq!(gateway_a.len(), 1);
    let wrong_kind = PostgresGovernanceInvalidation {
        kind: GovernanceArtifactKind::RoutingScores,
        ..gateway_a[0]
    };
    assert_eq!(
        repository
            .governance_ack_invalidation_outbox_event("gateway-a", wrong_kind)
            .await,
        Err(prodex_storage::GovernanceRepositoryError::NotFound)
    );
    assert!(
        repository
            .governance_poll_invalidation_outbox(other_tenant_id, "gateway-a", 16)
            .await
            .unwrap()
            .is_empty()
    );
    repository
        .governance_ack_invalidation_outbox_event("gateway-a", gateway_a[0])
        .await
        .unwrap();
    repository
        .governance_ack_invalidation_outbox_event("gateway-b", gateway_b[0])
        .await
        .unwrap();
    let retained = repository
        .governance_compact_invalidation_outbox(tenant_id)
        .await
        .unwrap();
    assert_eq!(retained.replica_count, 2);
    assert_eq!(retained.eligible_event_count, 1);
    assert_eq!(retained.removed_event_count, 0);
    assert_eq!(retained.retained_event_count, 1);

    publish_policy_pointer(&pool, tenant_id, "etag-2").await;
    let second_a = repository
        .governance_poll_invalidation_outbox(tenant_id, "gateway-a", 16)
        .await
        .unwrap();
    let second_b = repository
        .governance_poll_invalidation_outbox(tenant_id, "gateway-b", 16)
        .await
        .unwrap();
    assert_eq!(second_a, second_b);
    assert_eq!(second_a.len(), 1);
    repository
        .governance_ack_invalidation_outbox_event("gateway-a", second_a[0])
        .await
        .unwrap();
    let blocked = repository
        .governance_compact_invalidation_outbox(tenant_id)
        .await
        .unwrap();
    assert_eq!(blocked.eligible_event_count, 1);
    assert_eq!(blocked.removed_event_count, 1);
    assert_eq!(blocked.retained_event_count, 1);
    assert_eq!(
        repository
            .governance_poll_invalidation_outbox(tenant_id, "gateway-b", 16)
            .await
            .unwrap(),
        second_b
    );
    repository
        .governance_ack_invalidation_outbox_event("gateway-b", second_b[0])
        .await
        .unwrap();

    let mut client = pool.get().await.unwrap();
    let transaction = client.transaction().await.unwrap();
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .unwrap();
    transaction
        .execute(
            "UPDATE prodex_governance_invalidation_replicas \
             SET registered_at_unix_ms = 0, last_seen_at_unix_ms = 0 \
             WHERE tenant_id = $1 AND replica_id = 'gateway-a'",
            &[&tenant_id.as_uuid()],
        )
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    drop(client);

    publish_policy_pointer(&pool, tenant_id, "etag-3").await;
    let third_b = repository
        .governance_poll_invalidation_outbox(tenant_id, "gateway-b", 16)
        .await
        .unwrap();
    assert_eq!(third_b.len(), 1);
    repository
        .governance_ack_invalidation_outbox_event("gateway-b", third_b[0])
        .await
        .unwrap();
    let cleaned = repository
        .governance_compact_invalidation_outbox(tenant_id)
        .await
        .unwrap();
    assert_eq!(cleaned.replica_count, 1);
    assert_eq!(cleaned.eligible_event_count, 2);
    assert_eq!(cleaned.removed_event_count, 1);
    assert_eq!(cleaned.retained_event_count, 1);

    let recovered_a = repository
        .governance_poll_invalidation_outbox(tenant_id, "gateway-a", 16)
        .await
        .unwrap();
    assert_eq!(
        recovered_a, third_b,
        "expired replica must replay retained head"
    );

    let mut client = pool.get().await.unwrap();
    let transaction = client.transaction().await.unwrap();
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .unwrap();
    transaction
        .execute(
            "UPDATE prodex_governance_invalidation_replicas \
             SET registered_at_unix_ms = 0, last_seen_at_unix_ms = 0 \
             WHERE tenant_id = $1",
            &[&tenant_id.as_uuid()],
        )
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    drop(client);

    publish_policy_pointer(&pool, tenant_id, "etag-4").await;
    let no_live_replicas = repository
        .governance_compact_invalidation_outbox(tenant_id)
        .await
        .unwrap();
    assert_eq!(no_live_replicas.replica_count, 0);
    assert_eq!(no_live_replicas.eligible_event_count, 2);
    assert_eq!(no_live_replicas.removed_event_count, 1);
    assert_eq!(no_live_replicas.retained_event_count, 1);
    let recovered_c = repository
        .governance_poll_invalidation_outbox(tenant_id, "gateway-c", 16)
        .await
        .unwrap();
    assert_eq!(recovered_c.len(), 1);
    assert_eq!(recovered_c[0].kind, GovernanceArtifactKind::Policy);
}
