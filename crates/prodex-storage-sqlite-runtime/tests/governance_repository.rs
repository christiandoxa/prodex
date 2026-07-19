use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalReasonCode,
    ApprovalRecord, ApprovalScope, ApprovalState, AuditAction, AuditDigest, AuditEvent,
    AuditEventId, AuditOutcome, AuditReasonCode, AuditResource, AuditRetentionHold, Channel,
    CredentialScope, DataClassification, IdempotencyKey, IdempotentOperation, PolicyRevisionId,
    Principal, PrincipalId, PrincipalKind, Role, TenantContext, TenantId,
    compute_audit_chain_digest,
};
use prodex_storage::{
    AppendOnlyAuditCommand, ApprovalVoteIdempotency, AuditOutboxWriteCommand,
    GovernanceArtifactKind, GovernanceRevisionWriteCommand, GovernanceSessionRevokeCommand,
    GovernanceSessionUpsertCommand, GovernanceSessionUpsertOutcome, SiemOutboxRetryPolicy,
    TenantStorageKey,
};
use prodex_storage_sqlite::SQLITE_MIGRATIONS;
use prodex_storage_sqlite_runtime::{
    ApprovalVoteRequest, GovernanceActivationAction, GovernanceActivationRequest,
    GovernanceRepositoryError, GovernanceSnapshotSource, GovernanceSqliteRepository,
    GovernanceWriteOutcome,
};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

struct TestDatabase {
    root: PathBuf,
    path: PathBuf,
}

impl TestDatabase {
    fn new(tenants: &[TenantId]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "prodex-governance-repository-{}",
            AuditEventId::new()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("state.sqlite");
        let connection = Connection::open(&path).unwrap();
        for migration in SQLITE_MIGRATIONS {
            connection.execute_batch(migration.sql).unwrap();
        }
        for (index, tenant_id) in tenants.iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO prodex_tenants (
                        tenant_id, display_name, created_at_unix_ms, updated_at_unix_ms
                     ) VALUES (?1, ?2, 1, 1)",
                    rusqlite::params![tenant_id.to_string(), format!("tenant-{index}")],
                )
                .unwrap();
        }
        drop(connection);
        Self { root, path }
    }

    fn repository(&self) -> GovernanceSqliteRepository {
        GovernanceSqliteRepository::open(&self.path).unwrap()
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn governance_tenant_discovery_is_bounded_and_durable() {
    let tenants = [TenantId::new(), TenantId::new()];
    let database = TestDatabase::new(&tenants);
    let repository = database.repository();

    let mut expected = tenants.to_vec();
    expected.sort();
    assert_eq!(repository.governance_list_tenant_ids(3).unwrap(), expected);
    assert_eq!(repository.governance_list_tenant_ids(1).unwrap().len(), 1);
    assert_eq!(
        repository.governance_list_tenant_ids(0),
        Err(GovernanceRepositoryError::InvalidInput)
    );
}

#[derive(Default)]
struct AuditCursor {
    previous_digest: Option<String>,
    sequence: u64,
}

impl AuditCursor {
    fn next(
        &mut self,
        tenant_id: TenantId,
        principal: &Principal,
        action: &str,
    ) -> AuditOutboxWriteCommand {
        self.sequence += 1;
        let command = audit_outbox(
            tenant_id,
            principal,
            action,
            1_000 + self.sequence,
            self.previous_digest.as_deref(),
            "ignored",
        );
        self.previous_digest = Some(command.audit.event_digest.as_str().to_string());
        command
    }

    fn fork(
        &self,
        tenant_id: TenantId,
        principal: &Principal,
        action: &str,
        sequence: u64,
    ) -> AuditOutboxWriteCommand {
        audit_outbox(
            tenant_id,
            principal,
            action,
            10_000 + sequence,
            self.previous_digest.as_deref(),
            &format!("digest-{}", AuditEventId::new()),
        )
    }
}

fn principal(tenant_id: TenantId) -> Principal {
    Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    )
}

fn checksum(artifact: &[u8]) -> String {
    let digest = Sha256::digest(artifact);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

fn revision_command(
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    artifact: &[u8],
    maker: PrincipalId,
) -> GovernanceRevisionWriteCommand {
    GovernanceRevisionWriteCommand {
        storage_key: TenantStorageKey::tenant(tenant_id),
        tenant_id,
        kind,
        revision_id: revision_id.to_string(),
        fingerprint: ApprovalFingerprint::new(checksum(artifact)).unwrap(),
        compiled_artifact: artifact.to_vec(),
        created_by: maker,
        created_at_unix_ms: 1_000,
    }
}

fn approval_kind(kind: GovernanceArtifactKind) -> ApprovalKind {
    match kind {
        GovernanceArtifactKind::Policy => ApprovalKind::PolicyRevision,
        GovernanceArtifactKind::ClassificationRules => ApprovalKind::ClassificationRuleRevision,
        GovernanceArtifactKind::ProviderRegistry => ApprovalKind::ProviderRegistryRevision,
        GovernanceArtifactKind::RoutingScores => ApprovalKind::RoutingScoreRevision,
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_approved_revision(
    repository: &GovernanceSqliteRepository,
    audit: &mut AuditCursor,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    artifact: &[u8],
    maker: &Principal,
    checker: &Principal,
    scope: &str,
) -> ApprovalRecord {
    let command = revision_command(tenant_id, kind, revision_id, artifact, maker.id);
    repository
        .write_revision(
            command.clone(),
            audit.next(tenant_id, maker, "governance.revision.write"),
        )
        .unwrap();
    prepare_approval_for_existing(
        repository,
        audit,
        tenant_id,
        kind,
        command.fingerprint,
        maker,
        checker,
        scope,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_approval_for_existing(
    repository: &GovernanceSqliteRepository,
    audit: &mut AuditCursor,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    fingerprint: ApprovalFingerprint,
    maker: &Principal,
    checker: &Principal,
    scope: &str,
) -> ApprovalRecord {
    let approval = ApprovalRecord::pending(
        ApprovalId::new(format!("approval-{}", AuditEventId::new())).unwrap(),
        tenant_id,
        approval_kind(kind),
        ApprovalScope::new(scope).unwrap(),
        fingerprint,
        maker.id,
        1,
        100_000,
    )
    .unwrap();
    repository
        .create_approval(
            approval.clone(),
            audit.next(tenant_id, maker, "governance.approval.create"),
        )
        .unwrap();
    repository
        .vote_approval(ApprovalVoteRequest {
            tenant_id,
            approval_id: approval.id,
            actor: checker.clone(),
            expected_version: 1,
            now_unix_ms: 2_000,
            reason: None,
            audit_outbox: audit.next(tenant_id, checker, "governance.approval.vote"),
        })
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn activation_request(
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    approval: &ApprovalRecord,
    actor: &Principal,
    action: GovernanceActivationAction,
    expected_etag: Option<String>,
    idempotency: &str,
    audit_outbox: AuditOutboxWriteCommand,
) -> GovernanceActivationRequest {
    GovernanceActivationRequest {
        tenant_id,
        kind,
        revision_id: revision_id.to_string(),
        approval_id: approval.id.clone(),
        actor: actor.clone(),
        action,
        expected_etag,
        idempotency_key: IdempotencyKey::new(idempotency).unwrap(),
        request_fingerprint: format!("request:{idempotency}"),
        audit_outbox,
        activated_at_unix_ms: 5_000,
    }
}

#[test]
fn cancellation_reason_survives_storage_and_replays_idempotently() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let command = revision_command(
        tenant_id,
        GovernanceArtifactKind::Policy,
        &PolicyRevisionId::new().to_string(),
        b"policy",
        maker.id,
    );
    repository
        .write_revision(
            command.clone(),
            audit.next(tenant_id, &maker, "governance.revision.write"),
        )
        .unwrap();
    let approval = ApprovalRecord::pending(
        ApprovalId::new("cancel-persisted").unwrap(),
        tenant_id,
        ApprovalKind::PolicyRevision,
        ApprovalScope::new("policy/cancel").unwrap(),
        command.fingerprint,
        maker.id,
        1,
        100_000,
    )
    .unwrap();
    repository
        .create_approval(
            approval.clone(),
            audit.next(tenant_id, &maker, "governance.approval.create"),
        )
        .unwrap();
    let reason = ApprovalReasonCode::new("operator.cancelled").unwrap();
    let cancelled = repository
        .transition_approval(
            ApprovalVoteRequest {
                tenant_id,
                approval_id: approval.id.clone(),
                actor: maker.clone(),
                expected_version: 1,
                now_unix_ms: 2_000,
                reason: Some(reason.clone()),
                audit_outbox: audit.next(tenant_id, &maker, "governance.approval.cancel"),
            },
            ApprovalAction::Cancel,
        )
        .unwrap();
    assert_eq!(cancelled.state, ApprovalState::Cancelled);
    assert_eq!(cancelled.termination_reason, Some(reason.clone()));
    let loaded = repository.get_approval(tenant_id, &approval.id).unwrap();
    assert_eq!(loaded.termination_reason, Some(reason.clone()));

    let replay = repository
        .transition_approval(
            ApprovalVoteRequest {
                tenant_id,
                approval_id: approval.id,
                actor: maker.clone(),
                expected_version: 2,
                now_unix_ms: 2_001,
                reason: Some(reason),
                audit_outbox: audit.next(tenant_id, &maker, "governance.approval.cancel"),
            },
            ApprovalAction::Cancel,
        )
        .unwrap();
    assert_eq!(replay.version, 2);
}

#[test]
fn execution_approval_is_quorum_gated_and_consumed_once() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::DataPlane,
    );
    let checker_one = principal(tenant_id);
    let checker_two = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let approval = ApprovalRecord::pending(
        ApprovalId::new("execution:sha256:fixture").unwrap(),
        tenant_id,
        ApprovalKind::Execution,
        ApprovalScope::new("execution").unwrap(),
        ApprovalFingerprint::new("sha256:execution-fixture").unwrap(),
        maker.id,
        1,
        100_000,
    )
    .unwrap();

    repository
        .create_approval(
            approval.clone(),
            audit.next(tenant_id, &maker, "governance.execution_approval.create"),
        )
        .unwrap();
    let first = repository
        .vote_approval(ApprovalVoteRequest {
            tenant_id,
            approval_id: approval.id.clone(),
            actor: checker_one.clone(),
            expected_version: 1,
            now_unix_ms: 2_000,
            reason: None,
            audit_outbox: audit.next(
                tenant_id,
                &checker_one,
                "governance.execution_approval.vote",
            ),
        })
        .unwrap();
    assert_eq!(first.state, ApprovalState::PendingApproval);
    let approved = repository
        .vote_approval(ApprovalVoteRequest {
            tenant_id,
            approval_id: approval.id.clone(),
            actor: checker_two.clone(),
            expected_version: 2,
            now_unix_ms: 2_001,
            reason: None,
            audit_outbox: audit.next(
                tenant_id,
                &checker_two,
                "governance.execution_approval.vote",
            ),
        })
        .unwrap();
    assert_eq!(approved.state, ApprovalState::Approved);
    let consumed = repository
        .transition_approval(
            ApprovalVoteRequest {
                tenant_id,
                approval_id: approval.id.clone(),
                actor: maker.clone(),
                expected_version: 3,
                now_unix_ms: 3_000,
                reason: None,
                audit_outbox: audit.next(
                    tenant_id,
                    &maker,
                    "governance.execution_approval.consume",
                ),
            },
            ApprovalAction::Activate,
        )
        .unwrap();
    assert_eq!(consumed.state, ApprovalState::Active);
    let replay = repository.transition_approval(
        ApprovalVoteRequest {
            tenant_id,
            approval_id: approval.id,
            actor: maker.clone(),
            expected_version: 4,
            now_unix_ms: 3_001,
            reason: None,
            audit_outbox: audit.next(tenant_id, &maker, "governance.execution_approval.consume"),
        },
        ApprovalAction::Activate,
    );
    assert_eq!(replay, Err(GovernanceRepositoryError::InvalidTransition));
}

#[test]
fn execution_vote_idempotency_replays_denial_without_vote_or_duplicate_audit() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let approval = ApprovalRecord::pending(
        ApprovalId::new("execution:sha256:self-denied").unwrap(),
        tenant_id,
        ApprovalKind::Execution,
        ApprovalScope::new("execution").unwrap(),
        ApprovalFingerprint::new("sha256:self-denied").unwrap(),
        maker.id,
        2,
        100_000,
    )
    .unwrap();
    repository
        .create_approval(
            approval.clone(),
            audit.next(tenant_id, &maker, "governance.execution_approval.create"),
        )
        .unwrap();
    let operation = IdempotentOperation::new(
        tenant_id,
        IdempotencyKey::new("execution-self-denial").unwrap(),
        "sha256:self-denial-request",
    )
    .unwrap();
    let vote = |audit_outbox| ApprovalVoteRequest {
        tenant_id,
        approval_id: approval.id.clone(),
        actor: maker.clone(),
        expected_version: 1,
        now_unix_ms: 2_000,
        reason: None,
        audit_outbox,
    };
    let idempotency = ApprovalVoteIdempotency {
        operation: operation.clone(),
        started_at_unix_ms: 1_999,
    };
    assert_eq!(
        repository.transition_approval_idempotent(
            vote(audit.next(tenant_id, &maker, "governance.execution_approval.approve",)),
            ApprovalAction::Approve,
            idempotency.clone(),
        ),
        Err(GovernanceRepositoryError::ApprovalSelfAction)
    );
    audit.previous_digest = repository
        .latest_audit_digest(tenant_id)
        .unwrap()
        .map(|digest| digest.as_str().to_string());
    assert_eq!(
        repository.transition_approval_idempotent(
            vote(audit.next(tenant_id, &maker, "governance.execution_approval.approve",)),
            ApprovalAction::Approve,
            idempotency,
        ),
        Err(GovernanceRepositoryError::ApprovalSelfAction)
    );
    assert_eq!(
        repository.transition_approval_idempotent(
            vote(audit.next(tenant_id, &maker, "governance.execution_approval.approve",)),
            ApprovalAction::Approve,
            ApprovalVoteIdempotency {
                operation: IdempotentOperation::new(
                    tenant_id,
                    operation.key,
                    "sha256:conflicting-request",
                )
                .unwrap(),
                started_at_unix_ms: 1_999,
            },
        ),
        Err(GovernanceRepositoryError::Conflict)
    );

    let connection = Connection::open(database.path()).unwrap();
    let counts = connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM prodex_approval_votes WHERE tenant_id = ?1),
                (SELECT COUNT(*) FROM prodex_audit_log WHERE tenant_id = ?1),
                (SELECT COUNT(*) FROM prodex_siem_outbox WHERE tenant_id = ?1),
                (SELECT COUNT(*) FROM prodex_idempotency_records WHERE tenant_id = ?1)",
            [tenant_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(counts, (0, 2, 2, 1));
    let (outcome, reason, response): (String, String, Vec<u8>) = connection
        .query_row(
            "SELECT audit.outcome, audit.reason_code, idem.response_body
             FROM prodex_audit_log audit
             JOIN prodex_idempotency_records idem ON idem.tenant_id = audit.tenant_id
             WHERE audit.tenant_id = ?1 AND audit.reason_code IS NOT NULL",
            [tenant_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(outcome, "denied");
    assert_eq!(reason, "approval.self_approval_denied");
    let response = String::from_utf8(response).unwrap();
    assert_eq!(response, "v1|denied|approval.self_approval_denied");
    assert!(!response.contains("prompt"));
}

#[test]
fn stale_and_invalid_execution_votes_are_canonically_audited() {
    for (state, version, expected_version, expected_error, reason) in [
        (
            "pending_approval",
            1,
            9,
            GovernanceRepositoryError::StaleVersion,
            "approval.stale_version",
        ),
        (
            "approved",
            2,
            2,
            GovernanceRepositoryError::InvalidTransition,
            "approval.invalid_transition",
        ),
    ] {
        let tenant_id = TenantId::new();
        let database = TestDatabase::new(&[tenant_id]);
        let repository = database.repository();
        let maker = principal(tenant_id);
        let checker = principal(tenant_id);
        let mut audit = AuditCursor::default();
        let approval = ApprovalRecord::pending(
            ApprovalId::new(format!("execution:sha256:{reason}")).unwrap(),
            tenant_id,
            ApprovalKind::Execution,
            ApprovalScope::new("execution").unwrap(),
            ApprovalFingerprint::new(format!("sha256:{reason}")).unwrap(),
            maker.id,
            2,
            100_000,
        )
        .unwrap();
        repository
            .create_approval(
                approval.clone(),
                audit.next(tenant_id, &maker, "governance.execution_approval.create"),
            )
            .unwrap();
        Connection::open(database.path())
            .unwrap()
            .execute(
                "UPDATE prodex_approvals SET lifecycle_state = ?2, resource_version = ?3
                 WHERE tenant_id = ?1",
                rusqlite::params![tenant_id.to_string(), state, version],
            )
            .unwrap();
        assert_eq!(
            repository.transition_approval(
                ApprovalVoteRequest {
                    tenant_id,
                    approval_id: approval.id,
                    actor: checker.clone(),
                    expected_version,
                    now_unix_ms: 2_000,
                    reason: None,
                    audit_outbox: audit.next(
                        tenant_id,
                        &checker,
                        "governance.execution_approval.approve",
                    ),
                },
                ApprovalAction::Approve,
            ),
            Err(expected_error)
        );
        let connection = Connection::open(database.path()).unwrap();
        let (outcome, stored_reason): (String, String) = connection
            .query_row(
                "SELECT outcome, reason_code FROM prodex_audit_log
                 WHERE tenant_id = ?1 AND reason_code IS NOT NULL",
                [tenant_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(outcome, "denied");
        assert_eq!(stored_reason, reason);
    }
}

#[test]
fn legal_hold_persists_and_blocks_retention_purge_atomically() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let admin = principal(tenant_id);
    let mut audit = AuditCursor::default();

    let protected = audit.next(tenant_id, &admin, "governance.audit.fixture");
    let protected_id = protected.audit.event.id;
    repository.append_audit_outbox(protected).unwrap();
    let purgeable = audit.next(tenant_id, &admin, "governance.audit.fixture");
    let purgeable_id = purgeable.audit.event.id;
    repository.append_audit_outbox(purgeable).unwrap();

    let hold = AuditRetentionHold::new(
        TenantContext { tenant_id },
        protected_id,
        AuditReasonCode::new("legal.investigation").unwrap(),
        None,
    );
    repository
        .upsert_audit_legal_hold(
            &hold,
            admin.id,
            2_000,
            audit.next(tenant_id, &admin, "governance.audit_legal_hold.upsert"),
        )
        .unwrap();
    assert_eq!(
        repository.list_audit_legal_holds(tenant_id).unwrap(),
        vec![hold]
    );

    let purged = repository
        .purge_audit_events(
            tenant_id,
            &[protected_id, purgeable_id],
            10_000,
            5_000,
            audit.next(tenant_id, &admin, "governance.audit_retention.purge"),
        )
        .unwrap();
    assert!(purged.is_empty());

    let connection = Connection::open(database.path()).unwrap();
    let protected_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM prodex_audit_log
             WHERE tenant_id = ?1 AND audit_event_id = ?2",
            rusqlite::params![tenant_id.to_string(), protected_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(protected_count, 1);
}

#[test]
fn retention_purge_requires_a_contiguous_chain_prefix() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let admin = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let first = audit.next(tenant_id, &admin, "governance.audit.fixture");
    let first_id = first.audit.event.id;
    repository.append_audit_outbox(first).unwrap();
    let second = audit.next(tenant_id, &admin, "governance.audit.fixture");
    let second_id = second.audit.event.id;
    repository.append_audit_outbox(second).unwrap();

    let skipped = repository
        .purge_audit_events(
            tenant_id,
            &[second_id],
            10_000,
            5_000,
            audit.next(tenant_id, &admin, "governance.audit_retention.purge"),
        )
        .unwrap();
    assert!(skipped.is_empty());

    let purged = repository
        .purge_audit_events(
            tenant_id,
            &[first_id, second_id],
            10_001,
            5_000,
            audit.next(tenant_id, &admin, "governance.audit_retention.purge"),
        )
        .unwrap();
    assert_eq!(purged, vec![first_id, second_id]);
    assert!(
        repository
            .audit_integrity_health(tenant_id)
            .unwrap()
            .chain_valid
    );
}

#[test]
fn break_glass_approval_round_trips_as_non_revision_approval() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let approval = ApprovalRecord::pending(
        ApprovalId::new("break-glass:incident-1").unwrap(),
        tenant_id,
        ApprovalKind::BreakGlass,
        ApprovalScope::new("audit_retention:incident.response").unwrap(),
        ApprovalFingerprint::new("sha256:break-glass-fixture").unwrap(),
        maker.id,
        2,
        100_000,
    )
    .unwrap();
    let mut audit = AuditCursor::default();
    repository
        .create_approval(
            approval.clone(),
            audit.next(tenant_id, &maker, "governance.break_glass_approval.create"),
        )
        .unwrap();
    assert_eq!(
        repository
            .list_approvals(tenant_id, ApprovalKind::BreakGlass)
            .unwrap(),
        vec![approval]
    );
}

fn audit_outbox(
    tenant_id: TenantId,
    principal: &Principal,
    action: &str,
    occurred_at_unix_ms: u64,
    previous_digest: Option<&str>,
    _event_digest: &str,
) -> AuditOutboxWriteCommand {
    let event = AuditEvent::new(
        occurred_at_unix_ms,
        TenantContext { tenant_id },
        principal,
        AuditAction::try_new(action).unwrap(),
        AuditResource::new("governance_revision", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let previous_digest = previous_digest.map(|value| AuditDigest::new(value).unwrap());
    let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), &event);
    AuditOutboxWriteCommand {
        outbox_event_id: AuditEventId::new(),
        audit: AppendOnlyAuditCommand {
            storage_key: TenantStorageKey::tenant(tenant_id),
            event,
            previous_digest,
            event_digest,
        },
    }
}

#[test]
fn all_governance_artifact_kinds_use_revisioned_authority() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let checker = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let revisions = [
        (
            GovernanceArtifactKind::Policy,
            PolicyRevisionId::new().to_string(),
            b"policy-v1".as_slice(),
        ),
        (
            GovernanceArtifactKind::ClassificationRules,
            "classification-v1".to_string(),
            b"classification-v1".as_slice(),
        ),
        (
            GovernanceArtifactKind::ProviderRegistry,
            "registry-v1".to_string(),
            b"registry-v1".as_slice(),
        ),
        (
            GovernanceArtifactKind::RoutingScores,
            "scores-v1".to_string(),
            b"scores-v1".as_slice(),
        ),
    ];

    for (index, (kind, revision_id, artifact)) in revisions.iter().enumerate() {
        let approval = prepare_approved_revision(
            &repository,
            &mut audit,
            tenant_id,
            *kind,
            revision_id,
            artifact,
            &maker,
            &checker,
            &format!("governance/{index}"),
        );
        let request = activation_request(
            tenant_id,
            *kind,
            revision_id,
            &approval,
            &checker,
            GovernanceActivationAction::Activate,
            None,
            &format!("activate-{index}"),
            audit.next(tenant_id, &checker, "governance.revision.activate"),
        );
        let activation = repository
            .activate_revision(request.clone(), |_| true)
            .unwrap();
        assert_eq!(activation.outcome, GovernanceWriteOutcome::Applied);
        assert_eq!(
            repository
                .activate_revision(request, |_| true)
                .unwrap()
                .outcome,
            GovernanceWriteOutcome::Replayed
        );
        let snapshot = repository
            .load_snapshot(tenant_id, *kind, |_| true)
            .unwrap();
        assert_eq!(snapshot.revision_id, *revision_id);
        assert_eq!(snapshot.compiled_artifact, *artifact);
        assert_eq!(snapshot.source, GovernanceSnapshotSource::Active);
    }
}

#[test]
fn tenant_scoping_prevents_cross_tenant_snapshot_access() {
    let tenant_a = TenantId::new();
    let tenant_b = TenantId::new();
    let database = TestDatabase::new(&[tenant_a, tenant_b]);
    let repository = database.repository();
    let maker = principal(tenant_a);
    let checker = principal(tenant_a);
    let mut audit = AuditCursor::default();
    let revision_id = PolicyRevisionId::new().to_string();
    let approval = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_a,
        GovernanceArtifactKind::Policy,
        &revision_id,
        b"tenant-a-policy",
        &maker,
        &checker,
        "policy/tenant-a",
    );
    repository
        .activate_revision(
            activation_request(
                tenant_a,
                GovernanceArtifactKind::Policy,
                &revision_id,
                &approval,
                &checker,
                GovernanceActivationAction::Activate,
                None,
                "tenant-a-activate",
                audit.next(tenant_a, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();

    assert_eq!(
        repository.load_snapshot(tenant_b, GovernanceArtifactKind::Policy, |_| true),
        Err(GovernanceRepositoryError::SnapshotUnavailable)
    );
}

#[test]
fn concurrent_activation_uses_etag_compare_and_swap() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = Arc::new(database.repository());
    let maker = principal(tenant_id);
    let checker = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let initial_revision = PolicyRevisionId::new().to_string();
    let initial_approval = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &initial_revision,
        b"policy-v1",
        &maker,
        &checker,
        "policy/v1",
    );
    let initial = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &initial_revision,
                &initial_approval,
                &checker,
                GovernanceActivationAction::Activate,
                None,
                "activate-v1",
                audit.next(tenant_id, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();
    let candidates = [
        (PolicyRevisionId::new().to_string(), b"policy-v2".as_slice()),
        (PolicyRevisionId::new().to_string(), b"policy-v3".as_slice()),
    ]
    .map(|(revision, artifact)| {
        let approval = prepare_approved_revision(
            &repository,
            &mut audit,
            tenant_id,
            GovernanceArtifactKind::Policy,
            &revision,
            artifact,
            &maker,
            &checker,
            &format!("policy/{revision}"),
        );
        (revision, approval)
    });
    let requests = candidates.map(|(revision, approval)| {
        activation_request(
            tenant_id,
            GovernanceArtifactKind::Policy,
            &revision,
            &approval,
            &checker,
            GovernanceActivationAction::Activate,
            Some(initial.etag.clone()),
            &format!("activate-{revision}"),
            audit.fork(
                tenant_id,
                &checker,
                "governance.revision.activate",
                u64::from(revision.as_bytes()[0]),
            ),
        )
    });
    let barrier = Arc::new(Barrier::new(2));
    let [first_request, second_request] = requests;
    let run = |request: GovernanceActivationRequest, barrier: Arc<Barrier>| {
        let repository = Arc::clone(&repository);
        std::thread::spawn(move || {
            barrier.wait();
            repository.activate_revision(request, |_| true)
        })
    };
    let first = run(first_request, Arc::clone(&barrier));
    let second = run(second_request, barrier);
    let results = [first.join().unwrap(), second.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(GovernanceRepositoryError::EtagMismatch)))
            .count(),
        1
    );
}

#[test]
fn malformed_active_snapshot_keeps_lkg_and_rollback_is_atomic() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let checker = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let v1 = PolicyRevisionId::new().to_string();
    let approval_v1 = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v1,
        b"valid-v1",
        &maker,
        &checker,
        "policy/v1",
    );
    let active_v1 = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v1,
                &approval_v1,
                &checker,
                GovernanceActivationAction::Activate,
                None,
                "activate-v1",
                audit.next(tenant_id, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();
    let v2 = PolicyRevisionId::new().to_string();
    let approval_v2 = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &v2,
        b"malformed-v2",
        &maker,
        &checker,
        "policy/v2",
    );
    let active_v2 = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v2,
                &approval_v2,
                &checker,
                GovernanceActivationAction::Activate,
                Some(active_v1.etag),
                "activate-v2",
                audit.next(tenant_id, &checker, "governance.revision.activate"),
            ),
            |_| true,
        )
        .unwrap();
    let fallback = repository
        .load_snapshot(tenant_id, GovernanceArtifactKind::Policy, |artifact| {
            artifact != b"malformed-v2"
        })
        .unwrap();
    assert_eq!(fallback.revision_id, v1);
    assert_eq!(fallback.source, GovernanceSnapshotSource::LastKnownGood);

    let rollback_approval = prepare_approval_for_existing(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        ApprovalFingerprint::new(checksum(b"valid-v1")).unwrap(),
        &maker,
        &checker,
        "policy/rollback-v1",
    );
    let rollback = repository
        .activate_revision(
            activation_request(
                tenant_id,
                GovernanceArtifactKind::Policy,
                &v1,
                &rollback_approval,
                &checker,
                GovernanceActivationAction::Rollback,
                Some(active_v2.etag),
                "rollback-v1",
                audit.next(tenant_id, &checker, "governance.revision.rollback"),
            ),
            |_| true,
        )
        .unwrap();
    assert_eq!(rollback.revision_id, v1);
    assert_eq!(rollback.last_known_good_revision_id, v1);
}

#[test]
fn audit_failure_rolls_back_activation_fail_closed() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let checker = principal(tenant_id);
    let mut audit = AuditCursor::default();
    let revision = PolicyRevisionId::new().to_string();
    let approval = prepare_approved_revision(
        &repository,
        &mut audit,
        tenant_id,
        GovernanceArtifactKind::Policy,
        &revision,
        b"policy-v1",
        &maker,
        &checker,
        "policy/v1",
    );
    let request = activation_request(
        tenant_id,
        GovernanceArtifactKind::Policy,
        &revision,
        &approval,
        &checker,
        GovernanceActivationAction::Activate,
        None,
        "activate-v1",
        audit.next(tenant_id, &checker, "governance.revision.activate"),
    );
    let fault = Connection::open(database.path()).unwrap();
    fault
        .execute_batch(
            "CREATE TRIGGER fail_governance_audit
             BEFORE INSERT ON prodex_audit_log
             BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
        )
        .unwrap();
    assert_eq!(
        repository.activate_revision(request.clone(), |_| true),
        Err(GovernanceRepositoryError::Database)
    );
    assert_eq!(
        repository.load_snapshot(tenant_id, GovernanceArtifactKind::Policy, |_| true),
        Err(GovernanceRepositoryError::SnapshotUnavailable)
    );
    fault
        .execute_batch("DROP TRIGGER fail_governance_audit;")
        .unwrap();
    assert_eq!(
        repository
            .activate_revision(request, |_| true)
            .unwrap()
            .outcome,
        GovernanceWriteOutcome::Applied
    );
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
    let mut observed = Vec::new();
    let first = repository
        .run_siem_outbox_batch(1_000, 1, retry, |event| {
            observed.push(event.event_id);
            Err::<(), ()>(())
        })
        .unwrap();
    assert_eq!(first.retried, 1);
    let second = repository
        .run_siem_outbox_batch(1_100, 1, retry, |event| {
            observed.push(event.event_id);
            Err::<(), ()>(())
        })
        .unwrap();
    assert_eq!(second.retried, 1);
    let third = repository
        .run_siem_outbox_batch(1_300, 1, retry, |event| {
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

#[test]
fn outbox_worker_delivers_through_the_bounded_exporter_port() {
    struct RecordingExporter {
        capabilities: prodex_storage::SiemExporterCapabilities,
        event_ids: Vec<AuditEventId>,
    }

    impl prodex_storage::GovernanceAuditExporter for RecordingExporter {
        type Error = ();

        fn capabilities(&self) -> &prodex_storage::SiemExporterCapabilities {
            &self.capabilities
        }

        fn export_batch(
            &mut self,
            batch: &prodex_storage::SiemExportBatch,
        ) -> Result<prodex_storage::SiemExportReceipt, Self::Error> {
            self.event_ids
                .extend(batch.events().iter().map(|event| event.event_id));
            Ok(prodex_storage::SiemExportReceipt {
                accepted_events: batch.events().len() as u16,
            })
        }
    }

    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let command = audit_outbox(
        tenant_id,
        &principal(tenant_id),
        "governance.worker.export",
        1_000,
        None,
        "export-digest",
    );
    let expected_event_id = command.outbox_event_id;
    repository.append_audit_outbox(command).unwrap();
    let capabilities = prodex_storage::SiemExporterCapabilities::bounded(
        prodex_domain::SecretRef::new("projected", "siem-token", Some("v1")),
        Some(prodex_domain::SecretRef::new(
            "projected",
            "siem-mtls",
            Some("v1"),
        )),
        Some(prodex_domain::SecretRef::new(
            "projected",
            "siem-signing",
            Some("v1"),
        )),
        8,
        4096,
    )
    .unwrap();
    let mut exporter = RecordingExporter {
        capabilities,
        event_ids: Vec::new(),
    };

    let report = repository
        .run_siem_outbox_exporter_batch(
            1_000,
            8,
            SiemOutboxRetryPolicy::bounded(3, 100, 1_000).unwrap(),
            &mut exporter,
        )
        .unwrap();

    assert_eq!(report.delivered, 1);
    assert_eq!(exporter.event_ids, vec![expected_event_id]);
    assert_eq!(repository.outbox_health(tenant_id).unwrap().pending, 0);
    assert_eq!(repository.aggregate_outbox_health().unwrap().pending, 0);
}

#[test]
fn audit_integrity_health_detects_missing_same_tenant_parent() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let connection = Connection::open(database.path()).unwrap();
    connection
        .execute(
            "INSERT INTO prodex_audit_log (
                tenant_id, audit_event_id, previous_digest, event_digest,
                occurred_at_unix_ms, principal_id, action, resource_kind,
                resource_id, outcome, reason_code
             ) VALUES (?1, ?2, 'missing-parent', 'orphan-head', 1, ?3,
                       'governance.audit.test', 'governance_policy_revision', NULL,
                       'success', NULL)",
            rusqlite::params![
                tenant_id.to_string(),
                AuditEventId::new().to_string(),
                PrincipalId::new().to_string(),
            ],
        )
        .unwrap();
    drop(connection);

    let health = database
        .repository()
        .audit_integrity_health(tenant_id)
        .unwrap();
    assert_eq!(health.event_count, 1);
    assert_eq!(health.chain_head_count, 1);
    assert!(!health.chain_valid);
}

#[test]
fn audit_integrity_health_detects_canonical_field_tampering() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    repository
        .append_audit_outbox(audit_outbox(
            tenant_id,
            &principal(tenant_id),
            "governance.audit.test",
            1_000,
            None,
            "ignored",
        ))
        .unwrap();
    let connection = Connection::open(database.path()).unwrap();
    connection
        .execute(
            "UPDATE prodex_audit_log SET action = 'governance.audit.tampered'
             WHERE tenant_id = ?1",
            [tenant_id.to_string()],
        )
        .unwrap();

    let health = repository.audit_integrity_health(tenant_id).unwrap();
    assert_eq!(health.event_count, 1);
    assert_eq!(health.chain_head_count, 1);
    assert!(!health.chain_valid);
}

#[test]
fn governance_sessions_are_monotonic_bounded_and_revocable() {
    let tenant_id = TenantId::new();
    let database = TestDatabase::new(&[tenant_id]);
    let repository = database.repository();
    let maker = principal(tenant_id);
    let principal_id = PrincipalId::new();
    let policy_v1 = PolicyRevisionId::new();
    let policy_v2 = PolicyRevisionId::new();
    let registry_v1 = "registry-v1";
    let registry_v2 = "registry-v2";
    let mut audit = AuditCursor::default();
    for (kind, revision, artifact) in [
        (
            GovernanceArtifactKind::Policy,
            policy_v1.to_string(),
            b"policy-v1".as_slice(),
        ),
        (
            GovernanceArtifactKind::Policy,
            policy_v2.to_string(),
            b"policy-v2".as_slice(),
        ),
        (
            GovernanceArtifactKind::ProviderRegistry,
            registry_v1.to_string(),
            b"registry-v1".as_slice(),
        ),
        (
            GovernanceArtifactKind::ProviderRegistry,
            registry_v2.to_string(),
            b"registry-v2".as_slice(),
        ),
    ] {
        repository
            .write_revision(
                revision_command(tenant_id, kind, &revision, artifact, maker.id),
                audit.next(tenant_id, &maker, "governance.revision.write"),
            )
            .unwrap();
    }
    let session = "a".repeat(64);
    let command = |session_id_hash: String,
                   classification,
                   policy_revision_id,
                   provider_registry_revision: &str,
                   provider_descriptor_revision,
                   provider_affinity: &str,
                   created_at_unix_ms,
                   last_seen_at_unix_ms| GovernanceSessionUpsertCommand {
        tenant_id,
        session_id_hash,
        principal_id,
        channel: Channel::Api,
        credential_scope: CredentialScope::DataPlane,
        classification,
        policy_revision_id,
        provider_registry_revision: provider_registry_revision.to_string(),
        provider_descriptor_revision,
        provider_affinity: Some(provider_affinity.to_string()),
        created_at_unix_ms,
        last_seen_at_unix_ms,
        absolute_expires_at_unix_ms: 10_000,
        idle_expires_at_unix_ms: last_seen_at_unix_ms + 1_000,
        max_concurrent: Some(1),
    };
    let first = repository
        .governance_upsert_session(command(
            session.clone(),
            DataClassification::Public,
            policy_v1,
            registry_v1,
            11,
            "provider-v1",
            1_000,
            1_000,
        ))
        .unwrap();
    assert!(matches!(first, GovernanceSessionUpsertOutcome::Stored(_)));
    assert_eq!(
        repository
            .governance_upsert_session(command(
                "b".repeat(64),
                DataClassification::Internal,
                policy_v1,
                registry_v1,
                11,
                "provider-v1",
                1_001,
                1_001,
            ))
            .unwrap(),
        GovernanceSessionUpsertOutcome::ConcurrentLimitReached
    );

    let stale = repository
        .governance_upsert_session(command(
            session.clone(),
            DataClassification::Restricted,
            policy_v2,
            registry_v2,
            22,
            "provider-v2",
            900,
            900,
        ))
        .unwrap();
    let GovernanceSessionUpsertOutcome::Stored(stale) = stale else {
        panic!("existing session should be stored");
    };
    assert_eq!(stale.classification, DataClassification::Restricted);
    assert_eq!(stale.policy_revision_id, policy_v1);
    assert_eq!(stale.provider_registry_revision, registry_v1);
    assert_eq!(stale.provider_descriptor_revision, 11);
    assert_eq!(stale.provider_affinity.as_deref(), Some("provider-v1"));

    let fresh = repository
        .governance_upsert_session(command(
            session.clone(),
            DataClassification::Confidential,
            policy_v2,
            registry_v2,
            22,
            "provider-v2",
            1_000,
            1_100,
        ))
        .unwrap();
    let GovernanceSessionUpsertOutcome::Stored(fresh) = fresh else {
        panic!("existing session should be stored");
    };
    assert_eq!(fresh.classification, DataClassification::Restricted);
    assert_eq!(fresh.policy_revision_id, policy_v2);
    assert_eq!(fresh.provider_registry_revision, registry_v2);
    assert_eq!(fresh.provider_descriptor_revision, 22);
    assert_eq!(fresh.provider_affinity.as_deref(), Some("provider-v2"));
    assert_eq!(
        repository
            .governance_count_concurrent_sessions(tenant_id, principal_id, 1_100)
            .unwrap(),
        1
    );

    let revoke = GovernanceSessionRevokeCommand {
        tenant_id,
        session_id_hash: session,
        revoked_at_unix_ms: 1_200,
        reason_code: "session.revoked".to_string(),
        audit_outbox: audit.next(tenant_id, &maker, "governance.session.revoke"),
    };
    assert_eq!(
        repository
            .governance_session_revocation_epoch(tenant_id)
            .unwrap(),
        0
    );
    assert_eq!(
        repository
            .governance_revoke_session(revoke.clone())
            .unwrap(),
        GovernanceWriteOutcome::Applied
    );
    assert_eq!(
        repository
            .governance_session_revocation_epoch(tenant_id)
            .unwrap(),
        1
    );
    let mut replay = revoke;
    replay.revoked_at_unix_ms += 1;
    assert_eq!(
        repository.governance_revoke_session(replay).unwrap(),
        GovernanceWriteOutcome::Replayed
    );
    assert_eq!(
        repository
            .governance_session_revocation_epoch(tenant_id)
            .unwrap(),
        1,
        "idempotent replay must not publish another invalidation"
    );
    assert_eq!(
        repository
            .governance_count_concurrent_sessions(tenant_id, principal_id, 1_200)
            .unwrap(),
        0
    );
    let sessions = repository
        .governance_load_sessions(tenant_id, 1_200, 16)
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].revocation_reason_code.as_deref(),
        Some("session.revoked")
    );
}
