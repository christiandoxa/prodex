#[path = "postgres_runtime/audit_retention.rs"]
mod audit_retention;
#[path = "postgres_runtime/invalidation_outbox.rs"]
mod invalidation_outbox;
#[path = "postgres_runtime/policy_governance.rs"]
mod policy_governance;

use deadpool_postgres::Pool;
use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalRecord, ApprovalScope,
    AuditAction, AuditDigest, AuditEvent, AuditEventId, AuditOutcome, AuditReasonCode,
    AuditResource, AuditRetentionHold, BudgetLimit, BudgetSnapshot, CallId, Channel,
    CredentialScope, DataClassification, IdempotencyKey, IdempotentOperation, PolicyRevisionId,
    Principal, PrincipalId, PrincipalKind, ReservationId, ReservationReconciliationReason,
    ReservationRequest, Role, TenantContext, TenantId, UsageAmount, VirtualKeyId,
    compute_audit_chain_digest,
};
use prodex_storage::{
    AppendOnlyAuditCommand, ApprovalVoteMutationOutcome, ApprovalVoteRequest,
    AtomicReservationCommand, AuditOutboxWriteCommand, BudgetStorageScope,
    ExpiredReservationRecoveryCommand, GovernanceActivationAction, GovernanceActivationRequest,
    GovernanceArtifactAuthenticity, GovernanceArtifactKind, GovernanceMutationIdempotency,
    GovernanceRevisionWriteCommand, GovernanceSessionRevokeCommand, GovernanceSessionUpsertCommand,
    GovernanceSessionUpsertOutcome, GovernanceWriteOutcome, TenantStorageKey,
    UsageReconciliationCommand,
};
use prodex_storage_postgres::{SET_TENANT_STATEMENT, UPSERT_TENANT_LIFECYCLE_STATEMENT};
use prodex_storage_postgres_runtime::{
    IdempotentWriteOutcome, PostgresGovernanceInvalidation, PostgresRepository,
    PostgresRuntimeConfig, ReserveOutcome, ReserveRejection, StoredReservationState,
};
use sha2::{Digest, Sha256};
use std::str::FromStr;

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
#[ignore = "requires PRODEX_TEST_POSTGRES_URL"]
async fn postgres_governance_lifecycle_supports_all_artifact_kinds() {
    let url = std::env::var("PRODEX_TEST_POSTGRES_URL")
        .expect("PRODEX_TEST_POSTGRES_URL must point to the test PostgreSQL instance");
    let config = PostgresRuntimeConfig::new(url, 2).expect("test config should be valid");
    let pool = config
        .create_pool_explicit_no_tls()
        .expect("test pool should build");
    let repository = PostgresRepository::from_pool_with_config(pool.clone(), &config);
    let replay_repository = PostgresRepository::from_pool_with_config(pool.clone(), &config);
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
                    authenticity: None,
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
                    reason: None,
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
                    approval_id: Some(approval_id.clone()),
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
        let active_status = repository.governance_status(tenant_id, kind).await.unwrap();
        assert_eq!(
            active_status.active_revision_id.as_deref(),
            Some(revision_id.as_str())
        );
        assert_eq!(
            active_status.etag.as_deref(),
            Some(activated.etag.as_str()),
            "{label}: activation result must match committed pointer",
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

        let (revocation_audit, next_digest) =
            governance_audit(tenant_id, &maker, "governance.revision.revoke", now, digest);
        digest = Some(next_digest);
        let revocation = GovernanceActivationRequest {
            tenant_id,
            kind,
            revision_id: revision_id.clone(),
            approval_id: None,
            actor: maker.clone(),
            action: GovernanceActivationAction::Revoke,
            expected_etag: Some(activated.etag.clone()),
            idempotency_key: IdempotencyKey::new(format!("revoke-{label}")).unwrap(),
            request_fingerprint: format!("revoke-request-{label}"),
            audit_outbox: revocation_audit,
            activated_at_unix_ms: now,
        };
        let (first, second) = tokio::join!(
            repository.governance_activate_revision(revocation.clone(), |_| true),
            replay_repository.governance_activate_revision(revocation.clone(), |_| true),
        );
        assert!(
            first.is_ok() && second.is_ok(),
            "{label}: first={:?}, second={:?}",
            first.as_ref().map(|result| result.outcome),
            second.as_ref().map(|result| result.outcome),
        );
        let first = first.unwrap();
        let second = second.unwrap();
        assert_eq!(
            [first.outcome, second.outcome]
                .iter()
                .filter(|outcome| **outcome == GovernanceWriteOutcome::Applied)
                .count(),
            1
        );
        assert_eq!(
            [first.outcome, second.outcome]
                .iter()
                .filter(|outcome| **outcome == GovernanceWriteOutcome::Replayed)
                .count(),
            1
        );
        let revoked = if first.outcome == GovernanceWriteOutcome::Applied {
            &first
        } else {
            &second
        };
        assert_eq!(revoked.active_revision_id, None);
        assert_eq!(revoked.last_known_good_revision_id, None);
        assert_eq!(first.etag, second.etag);
        assert_eq!(first.active_revision_id, second.active_revision_id);
        assert_eq!(
            first.last_known_good_revision_id,
            second.last_known_good_revision_id
        );
        assert_eq!(
            repository
                .governance_load_snapshot(tenant_id, kind, |_| true)
                .await,
            Err(prodex_storage::GovernanceRepositoryError::SnapshotUnavailable)
        );
        assert_eq!(
            repository
                .governance_get_revision(tenant_id, kind, &revision_id)
                .await
                .unwrap()
                .lifecycle_state,
            "revoked"
        );

        let mut client = pool.get().await.unwrap();
        let transaction = client.transaction().await.unwrap();
        transaction
            .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
            .await
            .unwrap();
        let table = match kind {
            GovernanceArtifactKind::Policy => "prodex_policy_revisions",
            GovernanceArtifactKind::ClassificationRules => "prodex_classification_rule_revisions",
            GovernanceArtifactKind::ProviderRegistry => "prodex_provider_registry_revisions",
            GovernanceArtifactKind::RoutingScores => "prodex_routing_score_revisions",
        };
        let query = format!(
            "UPDATE {table} SET lifecycle_state = 'active'
             WHERE tenant_id = $1 AND revision_id = $2"
        );
        let raw_revival = if kind == GovernanceArtifactKind::Policy {
            transaction
                .execute(
                    &query,
                    &[
                        &tenant_id.as_uuid(),
                        &PolicyRevisionId::from_str(&revision_id).unwrap().as_uuid(),
                    ],
                )
                .await
        } else {
            transaction
                .execute(&query, &[&tenant_id.as_uuid(), &revision_id])
                .await
        };
        assert!(raw_revival.is_err());
        drop(transaction);

        let (reactivation_audit, _) = governance_audit(
            tenant_id,
            &maker,
            "governance.revision.activate",
            now + 1,
            digest.clone(),
        );
        assert_eq!(
            repository
                .governance_activate_revision(
                    GovernanceActivationRequest {
                        tenant_id,
                        kind,
                        revision_id: revision_id.clone(),
                        approval_id: Some(approval_id),
                        actor: maker.clone(),
                        action: GovernanceActivationAction::Activate,
                        expected_etag: Some(revoked.etag.clone()),
                        idempotency_key: IdempotencyKey::new(format!("reactivate-revoked-{label}"))
                            .unwrap(),
                        request_fingerprint: format!("reactivate-revoked-request-{label}"),
                        audit_outbox: reactivation_audit,
                        activated_at_unix_ms: now + 1,
                    },
                    |_| true,
                )
                .await,
            Err(prodex_storage::GovernanceRepositoryError::Conflict)
        );
        now += 2;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires PRODEX_TEST_POSTGRES_URL"]
async fn two_repositories_reserve_and_reconcile_idempotently() {
    let url = std::env::var("PRODEX_TEST_POSTGRES_URL")
        .expect("PRODEX_TEST_POSTGRES_URL must point to the test PostgreSQL instance");
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
#[ignore = "requires PRODEX_TEST_POSTGRES_URL"]
async fn released_reservation_reconciles_without_consuming_active_reservation() {
    let url = std::env::var("PRODEX_TEST_POSTGRES_URL")
        .expect("PRODEX_TEST_POSTGRES_URL must point to the test PostgreSQL instance");
    let config = PostgresRuntimeConfig::new(url, 4).expect("test config should be valid");
    let pool = config
        .create_pool_explicit_no_tls()
        .expect("test pool should build");
    let repository = PostgresRepository::from_pool_with_config(pool.clone(), &config);
    let tenant_id = TenantId::new();
    create_tenant(&pool, tenant_id).await;

    let expired = reservation_command(tenant_id);
    let mut active = reservation_command(tenant_id);
    active.storage_key = expired.storage_key;
    let expired_record = match repository.reserve(expired.clone()).await.unwrap() {
        ReserveOutcome::Reserved(record) => record,
        outcome => panic!("unexpected expired reservation outcome: {outcome:?}"),
    };
    let active_record = match repository.reserve(active.clone()).await.unwrap() {
        ReserveOutcome::Reserved(record) => record,
        outcome => panic!("unexpected active reservation outcome: {outcome:?}"),
    };
    repository
        .release_expired(ExpiredReservationRecoveryCommand {
            storage_key: expired.storage_key,
            snapshot: BudgetSnapshot {
                reserved: UsageAmount::new(200, 2_000),
                committed: UsageAmount::ZERO,
            },
            record: expired_record,
            now_unix_ms: expired_record.expires_at_unix_ms,
        })
        .await
        .expect("expired reservation should release");

    let actual = UsageAmount::new(40, 400);
    let reconcile = UsageReconciliationCommand {
        storage_key: expired.storage_key,
        snapshot: BudgetSnapshot {
            reserved: expired_record.reserved,
            committed: UsageAmount::ZERO,
        },
        record: expired_record,
        actual,
        reason: ReservationReconciliationReason::Completed,
    };
    assert_eq!(
        repository
            .reconcile_usage(reconcile.clone(), 1_800_000_061_000)
            .await
            .unwrap(),
        IdempotentWriteOutcome::Applied
    );
    assert_eq!(
        repository
            .reconcile_usage(reconcile, 1_800_000_061_001)
            .await
            .unwrap(),
        IdempotentWriteOutcome::Replayed
    );
    assert_eq!(
        repository
            .load_reservation(tenant_id, expired_record.call_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        StoredReservationState::Committed
    );
    assert_eq!(
        repository
            .load_reservation(tenant_id, active_record.call_id)
            .await
            .unwrap()
            .unwrap()
            .state,
        StoredReservationState::Active
    );

    let mut client = pool.get().await.expect("verification pool should connect");
    let transaction = client.transaction().await.unwrap();
    transaction
        .query_one(SET_TENANT_STATEMENT.sql, &[&tenant_id.to_string()])
        .await
        .unwrap();
    let storage_scope = expired.storage_key.storage_scope();
    let counter = transaction
        .query_one(
            "SELECT reserved_tokens, reserved_cost_micros, committed_tokens, committed_cost_micros FROM prodex_budget_counters WHERE tenant_id = $1 AND storage_scope = $2",
            &[&tenant_id.as_uuid(), &storage_scope],
        )
        .await
        .unwrap();
    assert_eq!(counter.get::<_, i64>(0), 100);
    assert_eq!(counter.get::<_, i64>(1), 1_000);
    assert_eq!(counter.get::<_, i64>(2), 40);
    assert_eq!(counter.get::<_, i64>(3), 400);
    let ledger = transaction
        .query_one(
            "SELECT COUNT(*) FILTER (WHERE event_kind = 'released'), COUNT(*) FILTER (WHERE event_kind = 'committed') FROM prodex_usage_ledger WHERE tenant_id = $1 AND reservation_id = $2",
            &[&tenant_id.as_uuid(), &expired_record.reservation_id.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(ledger.get::<_, i64>(0), 1);
    assert_eq!(ledger.get::<_, i64>(1), 1);
    transaction.commit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires PRODEX_TEST_POSTGRES_URL"]
async fn two_repositories_enforce_one_grouped_request_atomically() {
    let url = std::env::var("PRODEX_TEST_POSTGRES_URL")
        .expect("PRODEX_TEST_POSTGRES_URL must point to the test PostgreSQL instance");
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
