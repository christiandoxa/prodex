use deadpool_postgres::Pool;
use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalRecord, ApprovalScope,
    AuditAction, AuditDigest, AuditEvent, AuditEventId, AuditOutcome, AuditResource, BudgetLimit,
    BudgetSnapshot, CallId, Channel, CredentialScope, DataClassification, IdempotencyKey,
    PolicyRevisionId, Principal, PrincipalId, PrincipalKind, ReservationId,
    ReservationReconciliationReason, ReservationRequest, Role, TenantContext, TenantId,
    UsageAmount, VirtualKeyId, compute_audit_chain_digest,
};
use prodex_storage::{
    AppendOnlyAuditCommand, ApprovalVoteRequest, AtomicReservationCommand, AuditOutboxWriteCommand,
    BudgetStorageScope, ExpiredReservationRecoveryCommand, GovernanceActivationAction,
    GovernanceActivationRequest, GovernanceArtifactKind, GovernanceRevisionWriteCommand,
    GovernanceSessionRevokeCommand, GovernanceSessionUpsertCommand, GovernanceSessionUpsertOutcome,
    GovernanceWriteOutcome, TenantStorageKey, UsageReconciliationCommand,
};
use prodex_storage_postgres::{SET_TENANT_STATEMENT, UPSERT_TENANT_LIFECYCLE_STATEMENT};
use prodex_storage_postgres_runtime::{
    IdempotentWriteOutcome, PostgresRepository, PostgresRuntimeConfig, ReserveOutcome,
    ReserveRejection,
};
use sha2::{Digest, Sha256};

fn reservation_command(tenant_id: TenantId) -> AtomicReservationCommand {
    let call_id = CallId::new();
    let reservation_id = ReservationId::new();
    AtomicReservationCommand {
        storage_key: TenantStorageKey::virtual_key(tenant_id, VirtualKeyId::new()),
        idempotency_key: IdempotencyKey::from_call_reservation(call_id, reservation_id),
        snapshot: BudgetSnapshot::default(),
        limit: BudgetLimit::new(1_000, 10_000),
        request: ReservationRequest {
            tenant_id,
            call_id,
            reservation_id,
            estimate: UsageAmount::new(100, 1_000),
        },
        created_at_unix_ms: 1_800_000_000_000,
        ttl_ms: 60_000,
    }
}

fn grouped_request_command(
    tenant_id: TenantId,
    scope: BudgetStorageScope,
) -> AtomicReservationCommand {
    let mut command = reservation_command(tenant_id);
    command.storage_key = TenantStorageKey::budget_group(tenant_id, VirtualKeyId::new(), scope);
    command.limit = BudgetLimit::new(1_000, 10_000).with_max_requests(1);
    command
}

async fn create_tenant(pool: &Pool, tenant_id: TenantId) {
    let mut client = pool.get().await.expect("postgres pool should connect");
    let transaction = client
        .transaction()
        .await
        .expect("tenant setup transaction should start");
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .expect("tenant context should be set");
    transaction
        .query_one(
            UPSERT_TENANT_LIFECYCLE_STATEMENT.sql,
            &[
                &tenant_id.as_uuid(),
                &"runtime integration tenant",
                &1_800_000_000_000_i64,
            ],
        )
        .await
        .expect("migrated database should accept tenant setup");
    transaction
        .commit()
        .await
        .expect("tenant setup should commit");
}

fn governance_principal(tenant_id: TenantId) -> Principal {
    Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    )
}

fn governance_audit(
    tenant_id: TenantId,
    principal: &Principal,
    action: &str,
    occurred_at_unix_ms: u64,
    previous_digest: Option<AuditDigest>,
) -> (AuditOutboxWriteCommand, AuditDigest) {
    let event = AuditEvent::new(
        occurred_at_unix_ms,
        TenantContext { tenant_id },
        principal,
        AuditAction::new(action),
        AuditResource::new(
            "governance_policy_revision",
            None::<String>,
            Some(tenant_id),
        ),
        AuditOutcome::Success,
        None::<String>,
    );
    let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), &event);
    (
        AuditOutboxWriteCommand {
            outbox_event_id: AuditEventId::new(),
            audit: AppendOnlyAuditCommand {
                storage_key: TenantStorageKey::tenant(tenant_id),
                event,
                previous_digest,
                event_digest: event_digest.clone(),
            },
        },
        event_digest,
    )
}

fn governance_checksum(artifact: &[u8]) -> String {
    let digest = Sha256::digest(artifact);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_policy_governance_activates_and_replays_idempotently() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!(
            "skipping postgres_policy_governance_activates_and_replays_idempotently: \
             PRODEX_TEST_POSTGRES_URL is not set"
        );
        return;
    };
    let config = PostgresRuntimeConfig::new(url, 2).expect("test config should be valid");
    let pool = config
        .create_pool_explicit_no_tls()
        .expect("test pool should build");
    let repository = PostgresRepository::from_pool_with_config(pool.clone(), &config);
    let tenant_id = TenantId::new();
    create_tenant(&pool, tenant_id).await;
    let maker = governance_principal(tenant_id);
    let checker = governance_principal(tenant_id);
    let revision_id = PolicyRevisionId::new();
    let artifact = br#"{"version":1}"#.to_vec();
    let fingerprint = governance_checksum(&artifact);
    let (write_audit, digest) = governance_audit(
        tenant_id,
        &maker,
        "governance.policy.revision.write",
        1_800_000_000_001,
        None,
    );
    let write = GovernanceRevisionWriteCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        kind: GovernanceArtifactKind::Policy,
        revision_id: revision_id.to_string(),
        fingerprint: ApprovalFingerprint::new(fingerprint.clone()).unwrap(),
        compiled_artifact: artifact,
        created_by: maker.id,
        created_at_unix_ms: 1_800_000_000_001,
    };
    assert_eq!(
        repository
            .governance_write_revision(write.clone(), write_audit)
            .await
            .unwrap(),
        GovernanceWriteOutcome::Applied
    );

    let approval_id = ApprovalId::new(format!("approval-{revision_id}")).unwrap();
    let approval = ApprovalRecord::pending(
        approval_id.clone(),
        tenant_id,
        ApprovalKind::PolicyRevision,
        ApprovalScope::new(format!("policy/{revision_id}")).unwrap(),
        ApprovalFingerprint::new(fingerprint).unwrap(),
        maker.id,
        1,
        1_900_000_000_000,
    )
    .unwrap();
    let (approval_audit, digest) = governance_audit(
        tenant_id,
        &maker,
        "governance.policy.approval.create",
        1_800_000_000_002,
        Some(digest),
    );
    repository
        .governance_create_approval(approval, approval_audit)
        .await
        .unwrap();
    let (vote_audit, digest) = governance_audit(
        tenant_id,
        &checker,
        "governance.policy.approval.approve",
        1_800_000_000_003,
        Some(digest),
    );
    let approved = repository
        .governance_transition_approval(
            ApprovalVoteRequest {
                tenant_id,
                approval_id: approval_id.clone(),
                actor: checker,
                expected_version: 1,
                now_unix_ms: 1_800_000_000_003,
                audit_outbox: vote_audit,
            },
            ApprovalAction::Approve,
        )
        .await
        .unwrap();
    assert_eq!(approved.version, 2);

    let (activation_audit, _) = governance_audit(
        tenant_id,
        &maker,
        "governance.policy.revision.activate",
        1_800_000_000_004,
        Some(digest),
    );
    let request = GovernanceActivationRequest {
        tenant_id,
        kind: GovernanceArtifactKind::Policy,
        revision_id: revision_id.to_string(),
        approval_id,
        actor: maker.clone(),
        action: GovernanceActivationAction::Activate,
        expected_etag: None,
        idempotency_key: IdempotencyKey::new(format!("activate-{revision_id}")).unwrap(),
        request_fingerprint: format!("request-{revision_id}"),
        audit_outbox: activation_audit,
        activated_at_unix_ms: 1_800_000_000_004,
    };
    let activated = repository
        .governance_activate_revision(request.clone(), |_| true)
        .await
        .unwrap();
    assert_eq!(activated.outcome, GovernanceWriteOutcome::Applied);
    let replayed = repository
        .governance_activate_revision(request, |_| true)
        .await
        .unwrap();
    assert_eq!(replayed.outcome, GovernanceWriteOutcome::Replayed);
    assert_eq!(replayed.etag, activated.etag);
    assert_eq!(
        repository
            .governance_status(tenant_id, GovernanceArtifactKind::Policy)
            .await
            .unwrap()
            .active_revision_id
            .as_deref(),
        Some(revision_id.to_string().as_str())
    );
    let exported = repository
        .governance_export_audit(tenant_id, 10)
        .await
        .unwrap();
    assert_eq!(exported.len(), 4);
    assert_eq!(exported[0].action, "governance.policy.revision.activate");
    assert_eq!(
        repository.governance_export_audit(tenant_id, 0).await,
        Err(prodex_storage::GovernanceRepositoryError::InvalidInput)
    );
    let claims = repository
        .governance_claim_siem_outbox_batch(tenant_id, 1_800_000_000_005, 2, 60_000)
        .await
        .unwrap();
    assert_eq!(claims.len(), 2);
    let other_claims = repository
        .governance_claim_siem_outbox_batch(tenant_id, 1_800_000_000_005, 4, 60_000)
        .await
        .unwrap();
    assert!(claims.iter().all(|claim| {
        other_claims
            .iter()
            .all(|other| other.event_id != claim.event_id)
    }));
    assert_eq!(
        repository
            .governance_finalize_siem_outbox_claim(
                &claims[0],
                true,
                1_800_000_000_006,
                prodex_storage::SiemOutboxRetryPolicy::bounded(3, 1_000, 10_000).unwrap(),
            )
            .await
            .unwrap(),
        prodex_storage::SiemOutboxDeliveryDecision::Delivered
    );

    let registry_revision = "registry-v1";
    let mut client = pool.get().await.unwrap();
    let transaction = client.transaction().await.unwrap();
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .unwrap();
    transaction
        .execute(
            "INSERT INTO prodex_provider_registry_revisions (
                tenant_id, revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
             ) VALUES ($1, $2, $3, 'active', $4)",
            &[
                &tenant_id.as_uuid(),
                &registry_revision,
                &"sha256:registry-v1",
                &1_800_000_000_007_i64,
            ],
        )
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    let repository_two = PostgresRepository::from_pool_with_config(pool.clone(), &config);
    let principal_id = PrincipalId::new();
    let session_command = |session_id_hash: String| GovernanceSessionUpsertCommand {
        tenant_id,
        session_id_hash,
        principal_id,
        channel: Channel::Api,
        credential_scope: CredentialScope::DataPlane,
        classification: DataClassification::Confidential,
        policy_revision_id: revision_id,
        registry_revision_id: registry_revision.to_string(),
        provider_affinity: Some("provider-v1".to_string()),
        created_at_unix_ms: 1_800_000_000_007,
        last_seen_at_unix_ms: 1_800_000_000_007,
        absolute_expires_at_unix_ms: 1_800_000_100_000,
        idle_expires_at_unix_ms: 1_800_000_010_000,
        max_concurrent: Some(1),
    };
    let session_a = "a".repeat(64);
    let session_b = "b".repeat(64);
    let (first, second) = tokio::join!(
        repository.governance_upsert_session(session_command(session_a.clone())),
        repository_two.governance_upsert_session(session_command(session_b.clone()))
    );
    let outcomes = [first.unwrap(), second.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, GovernanceSessionUpsertOutcome::Stored(_)))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(
                outcome,
                GovernanceSessionUpsertOutcome::ConcurrentLimitReached
            ))
            .count(),
        1
    );
    let winning_session = outcomes
        .iter()
        .find_map(|outcome| match outcome {
            GovernanceSessionUpsertOutcome::Stored(record) => Some(record.session_id_hash.clone()),
            GovernanceSessionUpsertOutcome::ConcurrentLimitReached => None,
        })
        .unwrap();
    let previous_digest = repository
        .governance_latest_audit_digest(tenant_id)
        .await
        .unwrap();
    let (revoke_audit, _) = governance_audit(
        tenant_id,
        &maker,
        "governance.session.revoke",
        1_800_000_000_008,
        previous_digest,
    );
    let revoke = GovernanceSessionRevokeCommand {
        tenant_id,
        session_id_hash: winning_session,
        revoked_at_unix_ms: 1_800_000_000_008,
        reason_code: "session.revoked".to_string(),
        audit_outbox: revoke_audit,
    };
    assert_eq!(
        repository_two
            .governance_revoke_session(revoke.clone())
            .await
            .unwrap(),
        GovernanceWriteOutcome::Applied
    );
    let mut replay = revoke;
    replay.revoked_at_unix_ms += 1;
    assert_eq!(
        repository.governance_revoke_session(replay).await.unwrap(),
        GovernanceWriteOutcome::Replayed
    );
    assert_eq!(
        repository
            .governance_count_concurrent_sessions(tenant_id, principal_id, 1_800_000_000_008,)
            .await
            .unwrap(),
        0
    );
    let loaded = repository
        .governance_load_sessions(tenant_id, 1_800_000_000_008, 16)
        .await
        .unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(
        loaded[0].revocation_reason_code.as_deref(),
        Some("session.revoked")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_governance_lifecycle_supports_all_artifact_kinds() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!(
            "skipping postgres_governance_lifecycle_supports_all_artifact_kinds: \
             PRODEX_TEST_POSTGRES_URL is not set"
        );
        return;
    };
    let config = PostgresRuntimeConfig::new(url, 2).expect("test config should be valid");
    let pool = config
        .create_pool_explicit_no_tls()
        .expect("test pool should build");
    let repository = PostgresRepository::from_pool_with_config(pool.clone(), &config);
    let tenant_id = TenantId::new();
    create_tenant(&pool, tenant_id).await;
    let maker = governance_principal(tenant_id);
    let checker = governance_principal(tenant_id);
    let cases = [
        (
            GovernanceArtifactKind::Policy,
            ApprovalKind::PolicyRevision,
            PolicyRevisionId::new().to_string(),
            "policy",
        ),
        (
            GovernanceArtifactKind::ClassificationRules,
            ApprovalKind::ClassificationRuleRevision,
            "classification-v1".to_string(),
            "classification",
        ),
        (
            GovernanceArtifactKind::ProviderRegistry,
            ApprovalKind::ProviderRegistryRevision,
            "registry-v1".to_string(),
            "registry",
        ),
        (
            GovernanceArtifactKind::RoutingScores,
            ApprovalKind::RoutingScoreRevision,
            "routing-v1".to_string(),
            "routing",
        ),
    ];
    let mut digest = None;
    let mut now = 1_800_100_000_000_u64;
    for (kind, approval_kind, revision_id, label) in cases {
        let artifact = format!(r#"{{"kind":"{label}","version":1}}"#).into_bytes();
        let fingerprint = governance_checksum(&artifact);
        let (write_audit, next_digest) =
            governance_audit(tenant_id, &maker, "governance.revision.write", now, digest);
        digest = Some(next_digest);
        repository
            .governance_write_revision(
                GovernanceRevisionWriteCommand {
                    storage_key: TenantStorageKey::tenant(tenant_id),
                    tenant_id,
                    kind,
                    revision_id: revision_id.clone(),
                    fingerprint: ApprovalFingerprint::new(fingerprint.clone()).unwrap(),
                    compiled_artifact: artifact.clone(),
                    created_by: maker.id,
                    created_at_unix_ms: now,
                },
                write_audit,
            )
            .await
            .unwrap();
        now += 1;

        let approval_id = ApprovalId::new(format!("approval-{label}")).unwrap();
        let approval = ApprovalRecord::pending(
            approval_id.clone(),
            tenant_id,
            approval_kind,
            ApprovalScope::new(format!("governance/{label}/{revision_id}")).unwrap(),
            ApprovalFingerprint::new(fingerprint).unwrap(),
            maker.id,
            1,
            now + 100_000,
        )
        .unwrap();
        let (approval_audit, next_digest) =
            governance_audit(tenant_id, &maker, "governance.approval.create", now, digest);
        digest = Some(next_digest);
        repository
            .governance_create_approval(approval, approval_audit)
            .await
            .unwrap();
        now += 1;

        let (vote_audit, next_digest) = governance_audit(
            tenant_id,
            &checker,
            "governance.approval.approve",
            now,
            digest,
        );
        digest = Some(next_digest);
        repository
            .governance_transition_approval(
                ApprovalVoteRequest {
                    tenant_id,
                    approval_id: approval_id.clone(),
                    actor: checker.clone(),
                    expected_version: 1,
                    now_unix_ms: now,
                    audit_outbox: vote_audit,
                },
                ApprovalAction::Approve,
            )
            .await
            .unwrap();
        now += 1;

        let (activation_audit, next_digest) = governance_audit(
            tenant_id,
            &maker,
            "governance.revision.activate",
            now,
            digest,
        );
        digest = Some(next_digest);
        let activated = repository
            .governance_activate_revision(
                GovernanceActivationRequest {
                    tenant_id,
                    kind,
                    revision_id: revision_id.clone(),
                    approval_id,
                    actor: maker.clone(),
                    action: GovernanceActivationAction::Activate,
                    expected_etag: None,
                    idempotency_key: IdempotencyKey::new(format!("activate-{label}")).unwrap(),
                    request_fingerprint: format!("request-{label}"),
                    audit_outbox: activation_audit,
                    activated_at_unix_ms: now,
                },
                |_| true,
            )
            .await
            .unwrap();
        assert_eq!(activated.kind, kind);
        assert_eq!(activated.revision_id, revision_id);
        assert_eq!(
            repository
                .governance_status(tenant_id, kind)
                .await
                .unwrap()
                .active_revision_id
                .as_deref(),
            Some(revision_id.as_str())
        );
        assert_eq!(
            repository
                .governance_get_revision(tenant_id, kind, &revision_id)
                .await
                .unwrap()
                .lifecycle_state,
            "active"
        );
        assert_eq!(
            repository
                .governance_load_snapshot(tenant_id, kind, |_| true)
                .await
                .unwrap()
                .compiled_artifact,
            artifact
        );
        now += 1;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_repositories_reserve_and_reconcile_idempotently() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!(
            "skipping two_repositories_reserve_and_reconcile_idempotently: \
             PRODEX_TEST_POSTGRES_URL is not set"
        );
        return;
    };
    let config = PostgresRuntimeConfig::new(url, 4).expect("test config should be valid");
    let pool_one = config
        .create_pool_explicit_no_tls()
        .expect("first test pool should build");
    let pool_two = config
        .create_pool_explicit_no_tls()
        .expect("second test pool should build");
    let repository_one = PostgresRepository::from_pool_with_config(pool_one.clone(), &config);
    let repository_two = PostgresRepository::from_pool_with_config(pool_two, &config);
    let tenant_id = TenantId::new();
    create_tenant(&pool_one, tenant_id).await;

    let command = reservation_command(tenant_id);
    let (first, second) = tokio::join!(
        repository_one.reserve(command.clone()),
        repository_two.reserve(command.clone())
    );
    let outcomes = [first.unwrap(), second.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, ReserveOutcome::Reserved(_)))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, ReserveOutcome::Replayed(_)))
            .count(),
        1
    );

    let loaded = repository_two
        .load_reservation(tenant_id, command.request.call_id)
        .await
        .unwrap()
        .expect("reservation should load across repositories");
    let reconcile = UsageReconciliationCommand {
        storage_key: command.storage_key,
        snapshot: BudgetSnapshot {
            reserved: command.request.estimate,
            committed: UsageAmount::ZERO,
        },
        record: loaded.record,
        actual: UsageAmount::new(40, 400),
        reason: ReservationReconciliationReason::Completed,
    };
    assert_eq!(
        repository_one
            .reconcile_usage(reconcile.clone(), 1_800_000_001_000)
            .await
            .unwrap(),
        IdempotentWriteOutcome::Applied
    );
    assert_eq!(
        repository_two
            .reconcile_usage(reconcile, 1_800_000_001_001)
            .await
            .unwrap(),
        IdempotentWriteOutcome::Replayed
    );

    let abandoned = reservation_command(tenant_id);
    let abandoned_record = match repository_one.reserve(abandoned.clone()).await.unwrap() {
        ReserveOutcome::Reserved(record) => record,
        outcome => panic!("unexpected abandoned reservation outcome: {outcome:?}"),
    };
    let recovery = ExpiredReservationRecoveryCommand {
        storage_key: abandoned.storage_key,
        snapshot: BudgetSnapshot {
            reserved: abandoned.request.estimate,
            committed: UsageAmount::new(40, 400),
        },
        record: abandoned_record,
        now_unix_ms: abandoned_record.expires_at_unix_ms,
    };
    assert_eq!(
        repository_one
            .release_expired(recovery.clone())
            .await
            .unwrap(),
        IdempotentWriteOutcome::Applied
    );
    assert_eq!(
        repository_two.release_expired(recovery).await.unwrap(),
        IdempotentWriteOutcome::Replayed
    );

    let mut client = pool_one
        .get()
        .await
        .expect("verification pool should connect");
    let transaction = client
        .transaction()
        .await
        .expect("verification transaction should start");
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .expect("verification tenant context should be set");
    let ledger_count: i64 = transaction
        .query_one(
            "SELECT COUNT(*) FROM prodex_usage_ledger WHERE tenant_id = $1",
            &[&tenant_id.as_uuid()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        ledger_count, 5,
        "completed and abandoned reservations have one event per phase"
    );
    let counter = transaction
        .query_one(
            "SELECT COALESCE(SUM(reserved_tokens), 0)::BIGINT, \
                    COALESCE(SUM(reserved_cost_micros), 0)::BIGINT, \
                    COALESCE(SUM(committed_tokens), 0)::BIGINT, \
                    COALESCE(SUM(committed_cost_micros), 0)::BIGINT \
             FROM prodex_budget_counters \
             WHERE tenant_id = $1",
            &[&tenant_id.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(counter.get::<_, i64>(0), 0);
    assert_eq!(counter.get::<_, i64>(1), 0);
    assert_eq!(counter.get::<_, i64>(2), 40);
    assert_eq!(counter.get::<_, i64>(3), 400);
    transaction.commit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_repositories_enforce_one_grouped_request_atomically() {
    let Some(url) = std::env::var("PRODEX_TEST_POSTGRES_URL").ok() else {
        eprintln!(
            "skipping two_repositories_enforce_one_grouped_request_atomically: \
             PRODEX_TEST_POSTGRES_URL is not set"
        );
        return;
    };
    let config = PostgresRuntimeConfig::new(url, 4).expect("test config should be valid");
    let pool_one = config
        .create_pool_explicit_no_tls()
        .expect("first test pool should build");
    let pool_two = config
        .create_pool_explicit_no_tls()
        .expect("second test pool should build");
    let repository_one = PostgresRepository::from_pool_with_config(pool_one.clone(), &config);
    let repository_two = PostgresRepository::from_pool_with_config(pool_two, &config);
    let tenant_id = TenantId::new();
    create_tenant(&pool_one, tenant_id).await;
    let scope = BudgetStorageScope::from_digest([7; 32]);
    let first = grouped_request_command(tenant_id, scope);
    let second = grouped_request_command(tenant_id, scope);

    let (first, second) = tokio::join!(
        repository_one.reserve(first),
        repository_two.reserve(second)
    );
    let outcomes = [first.unwrap(), second.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, ReserveOutcome::Reserved(_)))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(
                outcome,
                ReserveOutcome::Rejected(ReserveRejection::RequestBudgetExceeded)
            ))
            .count(),
        1
    );

    let mut client = pool_one
        .get()
        .await
        .expect("verification pool should connect");
    let transaction = client
        .transaction()
        .await
        .expect("transaction should start");
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .expect("tenant context should be set");
    let row = transaction
        .query_one(
            "SELECT request_count, \
                    (SELECT COUNT(*) FROM prodex_reservations WHERE tenant_id = $1), \
                    (SELECT COUNT(*) FROM prodex_usage_ledger WHERE tenant_id = $1) \
             FROM prodex_budget_counters WHERE tenant_id = $1",
            &[&tenant_id.as_uuid()],
        )
        .await
        .expect("grouped request counter should exist");
    assert_eq!(row.get::<_, i64>(0), 1);
    assert_eq!(row.get::<_, i64>(1), 1);
    assert_eq!(row.get::<_, i64>(2), 1);
    transaction.commit().await.unwrap();
}
