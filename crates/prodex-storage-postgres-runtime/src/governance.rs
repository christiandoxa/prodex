//! Pooled PostgreSQL execution for policy governance.

use std::future::Future;
use std::str::FromStr;

use deadpool_postgres::Transaction;
use prodex_domain::{
    ApprovalAction, ApprovalError, ApprovalFingerprint, ApprovalId, ApprovalKind,
    ApprovalReasonCode, ApprovalRecord, ApprovalScope, ApprovalState, ApprovalTransitionRequest,
    ApprovalVote, AuditDigest, AuditEventId, Channel, CredentialScope, DataClassification,
    IdempotencyReplayDecision, PolicyRevisionId, Principal, PrincipalId, Role, TenantId,
    decide_idempotency_replay, transition_approval,
};
use prodex_storage::{
    ApprovalVoteIdempotency, ApprovalVoteMutationOutcome, ApprovalVoteRequest,
    ApprovalVoteSnapshot, ApprovalVoteStableDenial, ApprovalVoteStableOutcome,
    AuditOutboxWriteCommand, GovernanceActivationAction, GovernanceActivationRequest,
    GovernanceActivationResult, GovernanceArtifactKind, GovernanceAuditExportRecord,
    GovernanceAuditIntegrityHealth, GovernanceOutboxHealth, GovernanceRepositoryError,
    GovernanceRevisionSummary, GovernanceRevisionWriteCommand, GovernanceSessionRecord,
    GovernanceSessionRevokeCommand, GovernanceSessionUpsertCommand, GovernanceSessionUpsertOutcome,
    GovernanceSnapshot, GovernanceSnapshotSource, GovernanceStatus, GovernanceWriteOutcome,
    IdempotencyRecordLookupRow, IdempotencyRecordLookupRowStatus, SiemOutboxDeliveryDecision,
    SiemOutboxRetryPolicy, denied_approval_audit_outbox, materialize_idempotency_record_lookup_row,
    plan_audit_outbox_write, plan_governance_revision_write, plan_siem_outbox_delivery,
    verify_governance_audit_integrity,
};
use prodex_storage_postgres::{
    APPEND_AUDIT_OUTBOX_ATOMIC_STATEMENT, INSERT_GOVERNANCE_REVISION_ARTIFACT_STATEMENT,
    LOAD_GOVERNANCE_REVISION_ARTIFACT_STATEMENT, postgres_governance_pointer_statements,
};
use sha2::{Digest, Sha256};
use tokio_postgres::Row;
use uuid::Uuid;

use super::{PostgresRepository, set_tenant_context};

#[derive(Clone)]
struct RevisionRow {
    checksum: String,
    compiled_artifact: Vec<u8>,
    created_by: PrincipalId,
    created_at_unix_ms: u64,
}

#[derive(Clone)]
struct GovernancePointer {
    active_revision_id: Option<String>,
    last_known_good_revision_id: Option<String>,
    etag: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PostgresSiemOutboxClaim {
    pub tenant_id: TenantId,
    pub event_id: AuditEventId,
    pub audit_event_id: AuditEventId,
    pub event_envelope: String,
    pub attempt_count: u8,
    pub claim_token: Uuid,
}

impl std::fmt::Debug for PostgresSiemOutboxClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresSiemOutboxClaim")
            .field("tenant_id", &"<redacted>")
            .field("event_id", &"<redacted>")
            .field("audit_event_id", &"<redacted>")
            .field("event_envelope", &"<redacted>")
            .field("event_envelope_bytes", &self.event_envelope.len())
            .field("attempt_count", &self.attempt_count)
            .field("claim_token", &"<redacted>")
            .finish()
    }
}

impl PostgresRepository {
    pub async fn governance_write_revision(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_write_revision_inner(command, audit_outbox))
            .await
    }

    async fn governance_write_revision_inner(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        plan_governance_revision_write(command.clone())
            .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
        if command.kind == GovernanceArtifactKind::Policy {
            policy_revision_id(&command.revision_id)?;
        }
        let checksum = artifact_checksum(&command.compiled_artifact);
        if command.fingerprint.as_str() != checksum {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let created_at = to_i64(command.created_at_unix_ms)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, command.tenant_id)
            .await
            .map_err(database_error)?;

        if let Some(existing) = load_revision_row(
            &transaction,
            command.tenant_id,
            command.kind,
            &command.revision_id,
        )
        .await?
        {
            if existing.checksum == checksum
                && existing.compiled_artifact == command.compiled_artifact
                && existing.created_by == command.created_by
                && existing.created_at_unix_ms == command.created_at_unix_ms
            {
                transaction.commit().await.map_err(database_error)?;
                return Ok(GovernanceWriteOutcome::Replayed);
            }
            return Err(GovernanceRepositoryError::Conflict);
        }

        insert_revision_metadata(&transaction, &command, &checksum, created_at).await?;
        let statement = transaction
            .prepare_cached(INSERT_GOVERNANCE_REVISION_ARTIFACT_STATEMENT.sql)
            .await
            .map_err(database_error)?;
        let inserted = transaction
            .query_opt(
                &statement,
                &[
                    &command.tenant_id.as_uuid(),
                    &artifact_kind_label(command.kind),
                    &command.revision_id,
                    &checksum,
                    &command.compiled_artifact,
                    &command.created_by.as_uuid(),
                    &created_at,
                ],
            )
            .await
            .map_err(database_error)?;
        if inserted.is_none() {
            return Err(GovernanceRepositoryError::Conflict);
        }
        append_audit_outbox_tx(&transaction, audit_outbox).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }

    pub async fn governance_create_approval(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_create_approval_inner(approval, audit_outbox))
            .await
    }

    async fn governance_create_approval_inner(
        &self,
        approval: ApprovalRecord,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        if approval.state != ApprovalState::PendingApproval
            || approval.version != 1
            || !approval.votes.is_empty()
        {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let expires_at = to_i64(approval.expires_at_unix_ms)?;
        let version = to_i64(approval.version)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, approval.tenant_id)
            .await
            .map_err(database_error)?;
        let kind = approval_artifact_kind(approval.kind)?;
        let revision_id = if let Some(kind) = kind {
            Some(
                revision_id_for_fingerprint(
                    &transaction,
                    approval.tenant_id,
                    kind,
                    approval.fingerprint.as_str(),
                )
                .await?
                .ok_or(GovernanceRepositoryError::NotFound)?,
            )
        } else {
            None
        };

        if let Some(existing) =
            load_approval_tx(&transaction, approval.tenant_id, &approval.id).await?
        {
            if existing == approval {
                transaction.commit().await.map_err(database_error)?;
                return Ok(GovernanceWriteOutcome::Replayed);
            }
            return Err(GovernanceRepositoryError::Conflict);
        }
        transaction
            .execute(
                "INSERT INTO prodex_approvals (
                    tenant_id, approval_id, approval_kind, approval_scope, fingerprint,
                    maker_id, lifecycle_state, required_quorum, expires_at_unix_ms,
                    activated_at_unix_ms, termination_reason, resource_version
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, NULL, $10)",
                &[
                    &approval.tenant_id.as_uuid(),
                    &approval.id.as_str(),
                    &approval_kind_label(approval.kind),
                    &approval.scope.as_str(),
                    &approval.fingerprint.as_str(),
                    &approval.maker.as_uuid(),
                    &approval_state_label(approval.state),
                    &i16::from(approval.required_quorum),
                    &expires_at,
                    &version,
                ],
            )
            .await
            .map_err(database_error)?;
        if let (Some(kind), Some(revision_id)) = (kind, revision_id.as_deref()) {
            update_revision_state(
                &transaction,
                approval.tenant_id,
                kind,
                revision_id,
                "pending_approval",
            )
            .await?;
        }
        append_audit_outbox_tx(&transaction, audit_outbox).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }

    pub async fn governance_transition_approval(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        match self
            .governance_timeout(self.governance_transition_approval_inner(request, action, None))
            .await?
        {
            ApprovalVoteMutationOutcome::Applied(approval) => Ok(approval),
            ApprovalVoteMutationOutcome::Replayed(_) => {
                Err(GovernanceRepositoryError::InvalidInput)
            }
        }
    }

    pub async fn governance_transition_approval_idempotent(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: ApprovalVoteIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_transition_approval_inner(
            request,
            action,
            Some(idempotency),
        ))
        .await
    }

    async fn governance_transition_approval_inner(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: Option<ApprovalVoteIdempotency>,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        if !matches!(
            action,
            ApprovalAction::Approve
                | ApprovalAction::Reject
                | ApprovalAction::Cancel
                | ApprovalAction::Activate
        ) {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        if action != ApprovalAction::Activate {
            require_control_plane_admin(request.tenant_id, &request.actor)?;
        }
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, request.tenant_id)
            .await
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match approval_idempotency_replay_postgres(&transaction, request.tenant_id, idempotency)
                .await?
            {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_approval_idempotency_pending_postgres(
                        &transaction,
                        request.tenant_id,
                        idempotency,
                    )
                    .await?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().await.map_err(database_error)?;
                    return ApprovalVoteStableOutcome::replay(&response);
                }
            }
        }
        let current = load_approval_tx(&transaction, request.tenant_id, &request.approval_id)
            .await?
            .ok_or(GovernanceRepositoryError::NotFound)?;
        if action == ApprovalAction::Activate && current.kind != ApprovalKind::Execution {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let transition = match transition_approval(ApprovalTransitionRequest {
            record: &current,
            actor: &request.actor,
            action,
            expected_version: request.expected_version,
            now_unix_ms: request.now_unix_ms,
            reason: request.reason.as_ref(),
        }) {
            Ok(transition) => transition,
            Err(error) => {
                let Some(denial) = ApprovalVoteStableDenial::from_approval_error(error) else {
                    return Err(approval_transition_error(error));
                };
                append_audit_outbox_tx(
                    &transaction,
                    denied_approval_audit_outbox(request.audit_outbox, denial),
                )
                .await?;
                if let Some(idempotency) = idempotency.as_ref() {
                    complete_approval_idempotency_postgres(
                        &transaction,
                        request.tenant_id,
                        idempotency,
                        ApprovalVoteStableOutcome::Denied(denial),
                        request.now_unix_ms,
                    )
                    .await?;
                }
                transaction.commit().await.map_err(database_error)?;
                return Err(denial.repository_error());
            }
        };
        if !transition.changed {
            if let Some(idempotency) = idempotency.as_ref() {
                complete_approval_idempotency_postgres(
                    &transaction,
                    request.tenant_id,
                    idempotency,
                    ApprovalVoteStableOutcome::Success(ApprovalVoteSnapshot::from_record(
                        &transition.record,
                    )),
                    request.now_unix_ms,
                )
                .await?;
            }
            transaction.commit().await.map_err(database_error)?;
            return Ok(ApprovalVoteMutationOutcome::Applied(transition.record));
        }
        persist_approval_transition(&transaction, &current, &transition.record).await?;
        if transition.record.kind != ApprovalKind::Execution
            && matches!(
                transition.record.state,
                ApprovalState::Approved | ApprovalState::Rejected
            )
        {
            let revision_id = revision_id_for_fingerprint(
                &transaction,
                transition.record.tenant_id,
                artifact_kind_for_approval(transition.record.kind)?,
                transition.record.fingerprint.as_str(),
            )
            .await?
            .ok_or(GovernanceRepositoryError::NotFound)?;
            update_revision_state(
                &transaction,
                transition.record.tenant_id,
                artifact_kind_for_approval(transition.record.kind)?,
                &revision_id,
                approval_state_label(transition.record.state),
            )
            .await?;
        }
        append_audit_outbox_tx(&transaction, request.audit_outbox).await?;
        if let Some(idempotency) = idempotency.as_ref() {
            complete_approval_idempotency_postgres(
                &transaction,
                request.tenant_id,
                idempotency,
                ApprovalVoteStableOutcome::Success(ApprovalVoteSnapshot::from_record(
                    &transition.record,
                )),
                request.now_unix_ms,
            )
            .await?;
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(ApprovalVoteMutationOutcome::Applied(transition.record))
    }

    pub async fn governance_list_revisions(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<Vec<GovernanceRevisionSummary>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_list_revisions_inner(tenant_id, kind))
            .await
    }

    async fn governance_list_revisions_inner(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<Vec<GovernanceRevisionSummary>, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let query = format!(
            "SELECT revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
             FROM {} WHERE tenant_id = $1
             ORDER BY created_at_unix_ms DESC, revision_id DESC",
            revision_table(kind)
        );
        let rows = transaction
            .query(&query, &[&tenant_id.as_uuid()])
            .await
            .map_err(database_error)?;
        let summaries = rows
            .into_iter()
            .map(|row| {
                Ok(GovernanceRevisionSummary {
                    revision_id: revision_id_from_row(&row, 0, kind),
                    fingerprint: row.get(1),
                    lifecycle_state: row.get(2),
                    created_at_unix_ms: from_i64(row.get(3))?,
                })
            })
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        transaction.commit().await.map_err(database_error)?;
        Ok(summaries)
    }

    pub async fn governance_get_revision(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        revision_id: &str,
    ) -> Result<GovernanceRevisionSummary, GovernanceRepositoryError> {
        if kind == GovernanceArtifactKind::Policy {
            policy_revision_id(revision_id)?;
        }
        self.governance_timeout(self.governance_get_revision_inner(
            tenant_id,
            kind,
            revision_id.to_string(),
        ))
        .await
    }

    async fn governance_get_revision_inner(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        revision_id: String,
    ) -> Result<GovernanceRevisionSummary, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let query = format!(
            "SELECT revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
             FROM {} WHERE tenant_id = $1 AND revision_id = $2",
            revision_table(kind)
        );
        let row = if kind == GovernanceArtifactKind::Policy {
            let revision_id = policy_revision_id(&revision_id)?;
            transaction
                .query_opt(&query, &[&tenant_id.as_uuid(), &revision_id.as_uuid()])
                .await
        } else {
            transaction
                .query_opt(&query, &[&tenant_id.as_uuid(), &revision_id])
                .await
        }
        .map_err(database_error)?
        .ok_or(GovernanceRepositoryError::NotFound)?;
        let summary = GovernanceRevisionSummary {
            revision_id: revision_id_from_row(&row, 0, kind),
            fingerprint: row.get(1),
            lifecycle_state: row.get(2),
            created_at_unix_ms: from_i64(row.get(3))?,
        };
        transaction.commit().await.map_err(database_error)?;
        Ok(summary)
    }

    pub async fn governance_get_approval(
        &self,
        tenant_id: TenantId,
        approval_id: ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_get_approval_inner(tenant_id, approval_id))
            .await
    }

    async fn governance_get_approval_inner(
        &self,
        tenant_id: TenantId,
        approval_id: ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let approval = load_approval_tx(&transaction, tenant_id, &approval_id)
            .await?
            .ok_or(GovernanceRepositoryError::NotFound)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(approval)
    }

    pub async fn governance_list_execution_approvals(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_list_execution_approvals_inner(tenant_id))
            .await
    }

    async fn governance_list_execution_approvals_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                "SELECT approval_id FROM prodex_approvals
                 WHERE tenant_id = $1 AND approval_kind = 'execution'
                 ORDER BY expires_at_unix_ms DESC, approval_id DESC",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?;
        let mut approvals = Vec::with_capacity(rows.len());
        for row in rows {
            let id = ApprovalId::new(row.get::<_, String>(0))
                .map_err(|_| GovernanceRepositoryError::Database)?;
            approvals.push(
                load_approval_tx(&transaction, tenant_id, &id)
                    .await?
                    .ok_or(GovernanceRepositoryError::Database)?,
            );
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(approvals)
    }

    pub async fn governance_status(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<GovernanceStatus, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_status_inner(tenant_id, kind))
            .await
    }

    async fn governance_status_inner(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<GovernanceStatus, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let pointer = load_pointer_for_kind(&transaction, tenant_id, kind).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(
            pointer.map_or_else(GovernanceStatus::default, |pointer| GovernanceStatus {
                active_revision_id: pointer.active_revision_id,
                last_known_good_revision_id: pointer.last_known_good_revision_id,
                etag: Some(pointer.etag),
            }),
        )
    }

    pub async fn governance_load_snapshot<F>(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        validate_artifact: F,
    ) -> Result<GovernanceSnapshot, GovernanceRepositoryError>
    where
        F: FnMut(&[u8]) -> bool,
    {
        self.governance_timeout(self.governance_load_snapshot_inner(
            tenant_id,
            kind,
            validate_artifact,
        ))
        .await
    }

    async fn governance_load_snapshot_inner<F>(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        mut validate_artifact: F,
    ) -> Result<GovernanceSnapshot, GovernanceRepositoryError>
    where
        F: FnMut(&[u8]) -> bool,
    {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let pointer = load_pointer_for_kind(&transaction, tenant_id, kind)
            .await?
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        let active = pointer
            .active_revision_id
            .as_deref()
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        if let Some(snapshot) = load_verified_snapshot(
            &transaction,
            tenant_id,
            kind,
            active,
            GovernanceSnapshotSource::Active,
            &mut validate_artifact,
        )
        .await?
        {
            transaction.commit().await.map_err(database_error)?;
            return Ok(snapshot);
        }
        let last_known_good = pointer
            .last_known_good_revision_id
            .as_deref()
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        if last_known_good == active {
            return Err(GovernanceRepositoryError::SnapshotUnavailable);
        }
        let snapshot = load_verified_snapshot(
            &transaction,
            tenant_id,
            kind,
            last_known_good,
            GovernanceSnapshotSource::LastKnownGood,
            &mut validate_artifact,
        )
        .await?
        .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(snapshot)
    }

    pub async fn governance_latest_audit_digest(
        &self,
        tenant_id: TenantId,
    ) -> Result<Option<AuditDigest>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_latest_audit_digest_inner(tenant_id))
            .await
    }

    async fn governance_latest_audit_digest_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<Option<AuditDigest>, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let value = latest_audit_digest_tx(&transaction, tenant_id).await?;
        transaction.commit().await.map_err(database_error)?;
        value
            .map(|value| AuditDigest::new(value).map_err(|_| GovernanceRepositoryError::Database))
            .transpose()
    }

    pub async fn governance_outbox_health(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceOutboxHealth, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_outbox_health_inner(tenant_id))
            .await
    }

    async fn governance_outbox_health_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceOutboxHealth, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let row = transaction
            .query_one(
                "SELECT COUNT(*), MIN(created_at_unix_ms) FROM prodex_siem_outbox
                 WHERE tenant_id = $1 AND delivered_at_unix_ms IS NULL",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?;
        let dead_lettered = transaction
            .query_one(
                "SELECT COUNT(*) FROM prodex_siem_dead_letters WHERE tenant_id = $1",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?
            .get::<_, i64>(0);
        let health = GovernanceOutboxHealth {
            pending: from_i64(row.get(0))?,
            dead_lettered: from_i64(dead_lettered)?,
            oldest_pending_at_unix_ms: row.get::<_, Option<i64>>(1).map(from_i64).transpose()?,
        };
        transaction.commit().await.map_err(database_error)?;
        Ok(health)
    }

    pub async fn governance_audit_integrity_health(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceAuditIntegrityHealth, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_audit_integrity_health_inner(tenant_id))
            .await
    }

    async fn governance_audit_integrity_health_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceAuditIntegrityHealth, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                "SELECT audit_event_id, occurred_at_unix_ms, principal_id, action,
                        resource_kind, resource_id, outcome, reason_code,
                        previous_digest, event_digest
                 FROM prodex_audit_log WHERE tenant_id = $1",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?;
        let records = rows
            .into_iter()
            .map(|row| {
                Ok(GovernanceAuditExportRecord {
                    audit_event_id: row.get::<_, Uuid>(0).to_string(),
                    occurred_at_unix_ms: from_i64(row.get(1))?,
                    principal_id: row.get::<_, Uuid>(2).to_string(),
                    action: row.get(3),
                    resource_kind: row.get(4),
                    resource_id: row.get(5),
                    outcome: row.get(6),
                    reason_code: row.get(7),
                    previous_digest: row.get(8),
                    event_digest: row.get(9),
                })
            })
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        let health = verify_governance_audit_integrity(tenant_id, &records);
        transaction.commit().await.map_err(database_error)?;
        Ok(health)
    }

    pub async fn governance_export_audit(
        &self,
        tenant_id: TenantId,
        limit: u16,
    ) -> Result<Vec<GovernanceAuditExportRecord>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_export_audit_inner(tenant_id, limit))
            .await
    }

    async fn governance_export_audit_inner(
        &self,
        tenant_id: TenantId,
        limit: u16,
    ) -> Result<Vec<GovernanceAuditExportRecord>, GovernanceRepositoryError> {
        if limit == 0 || limit > 1_000 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                "SELECT audit_event_id, occurred_at_unix_ms, principal_id, action,
                        resource_kind, resource_id, outcome, reason_code,
                        previous_digest, event_digest
                 FROM prodex_audit_log WHERE tenant_id = $1
                 ORDER BY occurred_at_unix_ms DESC, audit_event_id DESC LIMIT $2",
                &[&tenant_id.as_uuid(), &i64::from(limit)],
            )
            .await
            .map_err(database_error)?;
        let records = rows
            .into_iter()
            .map(|row| {
                Ok(GovernanceAuditExportRecord {
                    audit_event_id: row.get::<_, Uuid>(0).to_string(),
                    occurred_at_unix_ms: from_i64(row.get(1))?,
                    principal_id: row.get::<_, Uuid>(2).to_string(),
                    action: row.get(3),
                    resource_kind: row.get(4),
                    resource_id: row.get(5),
                    outcome: row.get(6),
                    reason_code: row.get(7),
                    previous_digest: row.get(8),
                    event_digest: row.get(9),
                })
            })
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        transaction.commit().await.map_err(database_error)?;
        Ok(records)
    }

    pub async fn governance_activate_revision<F>(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: F,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError>
    where
        F: FnOnce(&[u8]) -> bool,
    {
        self.governance_timeout(self.governance_activate_revision_inner(request, validate_artifact))
            .await
    }

    async fn governance_activate_revision_inner<F>(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: F,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError>
    where
        F: FnOnce(&[u8]) -> bool,
    {
        require_control_plane_admin(request.tenant_id, &request.actor)?;
        if request.kind == GovernanceArtifactKind::Policy {
            policy_revision_id(&request.revision_id)?;
        }
        if request.request_fingerprint.is_empty() || request.request_fingerprint.len() > 256 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let activated_at = to_i64(request.activated_at_unix_ms)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, request.tenant_id)
            .await
            .map_err(database_error)?;

        if let Some(replay) = load_activation_replay(&transaction, &request).await? {
            transaction.commit().await.map_err(database_error)?;
            return Ok(replay);
        }
        let revision = load_revision_row(
            &transaction,
            request.tenant_id,
            request.kind,
            &request.revision_id,
        )
        .await?
        .ok_or(GovernanceRepositoryError::NotFound)?;
        if revision.checksum != artifact_checksum(&revision.compiled_artifact)
            || !validate_artifact(&revision.compiled_artifact)
        {
            return Err(GovernanceRepositoryError::SnapshotUnavailable);
        }
        let approval = load_approval_tx(&transaction, request.tenant_id, &request.approval_id)
            .await?
            .ok_or(GovernanceRepositoryError::ApprovalRequired)?;
        if approval.state != ApprovalState::Approved
            || approval.kind != approval_kind_for_artifact(request.kind)
            || approval.fingerprint.as_str() != revision.checksum
        {
            return Err(GovernanceRepositoryError::ApprovalRequired);
        }
        let pointer = load_pointer_for_kind(&transaction, request.tenant_id, request.kind).await?;
        if pointer.as_ref().map(|value| value.etag.as_str()) != request.expected_etag.as_deref() {
            return Err(GovernanceRepositoryError::EtagMismatch);
        }
        if request.action == GovernanceActivationAction::Rollback
            && pointer
                .as_ref()
                .and_then(|value| value.last_known_good_revision_id.as_deref())
                != Some(request.revision_id.as_str())
        {
            return Err(GovernanceRepositoryError::Conflict);
        }

        let previous_active = pointer
            .as_ref()
            .and_then(|value| value.active_revision_id.clone());
        let last_known_good = match request.action {
            GovernanceActivationAction::Activate => previous_active
                .clone()
                .or_else(|| pointer.as_ref()?.last_known_good_revision_id.clone())
                .unwrap_or_else(|| request.revision_id.clone()),
            GovernanceActivationAction::Rollback => request.revision_id.clone(),
        };
        let etag = activation_etag(&request, pointer.as_ref().map(|value| value.etag.as_str()));
        let activated_approval = transition_approval(ApprovalTransitionRequest {
            record: &approval,
            actor: &request.actor,
            action: ApprovalAction::Activate,
            expected_version: approval.version,
            now_unix_ms: request.activated_at_unix_ms,
            reason: None,
        })
        .map_err(|_| GovernanceRepositoryError::ApprovalRequired)?;

        match previous_active.as_deref() {
            Some(previous) if previous != request.revision_id => {
                update_revision_state(
                    &transaction,
                    request.tenant_id,
                    request.kind,
                    previous,
                    match request.action {
                        GovernanceActivationAction::Activate => "superseded",
                        GovernanceActivationAction::Rollback => "rolled_back",
                    },
                )
                .await?;
            }
            _ => {}
        }
        update_revision_state(
            &transaction,
            request.tenant_id,
            request.kind,
            &request.revision_id,
            "active",
        )
        .await?;
        persist_approval_transition(&transaction, &approval, &activated_approval.record).await?;

        let pointer_statements = postgres_governance_pointer_statements(request.kind);
        let statement = transaction
            .prepare_cached(pointer_statements.compare_and_swap.sql)
            .await
            .map_err(database_error)?;
        let previous_etag = pointer.as_ref().map(|value| value.etag.clone());
        let stored = if request.kind == GovernanceArtifactKind::Policy {
            let revision_id = policy_revision_id(&request.revision_id)?;
            let last_known_good_id = policy_revision_id(&last_known_good)?;
            transaction
                .query_opt(
                    &statement,
                    &[
                        &request.tenant_id.as_uuid(),
                        &revision_id.as_uuid(),
                        &last_known_good_id.as_uuid(),
                        &etag,
                        &activated_at,
                        &previous_etag,
                    ],
                )
                .await
        } else {
            transaction
                .query_opt(
                    &statement,
                    &[
                        &request.tenant_id.as_uuid(),
                        &request.revision_id,
                        &last_known_good,
                        &etag,
                        &activated_at,
                        &previous_etag,
                    ],
                )
                .await
        }
        .map_err(database_error)?;
        if stored.is_none() {
            return Err(GovernanceRepositoryError::EtagMismatch);
        }
        insert_activation_history(
            &transaction,
            &request,
            previous_active.as_deref(),
            activated_at,
        )
        .await?;
        append_audit_outbox_tx(&transaction, request.audit_outbox.clone()).await?;
        transaction
            .execute(
                "INSERT INTO prodex_governance_mutation_idempotency (
                    tenant_id, artifact_kind, idempotency_key, request_fingerprint,
                    action, revision_id, resulting_etag, created_at_unix_ms
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[
                    &request.tenant_id.as_uuid(),
                    &artifact_kind_label(request.kind),
                    &request.idempotency_key.as_str(),
                    &request.request_fingerprint,
                    &request.action.as_str(),
                    &request.revision_id,
                    &etag,
                    &activated_at,
                ],
            )
            .await
            .map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceActivationResult {
            outcome: GovernanceWriteOutcome::Applied,
            kind: request.kind,
            revision_id: request.revision_id,
            etag,
            last_known_good_revision_id: last_known_good,
        })
    }

    pub async fn governance_upsert_session(
        &self,
        command: GovernanceSessionUpsertCommand,
    ) -> Result<GovernanceSessionUpsertOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_upsert_session_inner(command))
            .await
    }

    async fn governance_upsert_session_inner(
        &self,
        command: GovernanceSessionUpsertCommand,
    ) -> Result<GovernanceSessionUpsertOutcome, GovernanceRepositoryError> {
        validate_governance_session_upsert(&command)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, command.tenant_id)
            .await
            .map_err(database_error)?;
        let admission_lock = format!("{}:{}", command.tenant_id, command.principal_id);
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                &[&admission_lock],
            )
            .await
            .map_err(database_error)?;
        let existing =
            load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)
                .await?;
        if let Some(existing) = existing.as_ref() {
            if existing.principal_id != command.principal_id
                || existing.channel != command.channel
                || existing.credential_scope != command.credential_scope
                || existing.revoked_at_unix_ms.is_some()
            {
                return Err(GovernanceRepositoryError::Conflict);
            }
        } else if let Some(max) = command.max_concurrent {
            let active = count_concurrent_sessions_tx(
                &transaction,
                command.tenant_id,
                command.principal_id,
                command.last_seen_at_unix_ms,
            )
            .await?;
            if active >= u64::from(max) {
                transaction.commit().await.map_err(database_error)?;
                return Ok(GovernanceSessionUpsertOutcome::ConcurrentLimitReached);
            }
        }
        let created_at = to_i64(command.created_at_unix_ms)?;
        let last_seen_at = to_i64(command.last_seen_at_unix_ms)?;
        let absolute_expires_at = to_i64(command.absolute_expires_at_unix_ms)?;
        let idle_expires_at = to_i64(command.idle_expires_at_unix_ms)?;
        let provider_descriptor_revision = to_i64(command.provider_descriptor_revision)?;
        transaction
            .execute(
                "INSERT INTO prodex_governance_sessions (
                    tenant_id, session_id_hash, principal_id, channel, credential_scope,
                    classification, policy_revision_id, provider_registry_revision,
                    provider_descriptor_revision, provider_affinity,
                    created_at_unix_ms, last_seen_at_unix_ms, absolute_expires_at_unix_ms,
                    idle_expires_at_unix_ms
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                 ON CONFLICT (tenant_id, session_id_hash) DO UPDATE SET
                    classification = CASE
                        WHEN prodex_governance_sessions.classification = 'restricted'
                          OR EXCLUDED.classification = 'restricted' THEN 'restricted'
                        WHEN prodex_governance_sessions.classification = 'confidential'
                          OR EXCLUDED.classification = 'confidential' THEN 'confidential'
                        WHEN prodex_governance_sessions.classification = 'internal'
                          OR EXCLUDED.classification = 'internal' THEN 'internal'
                        ELSE 'public' END,
                    policy_revision_id = CASE
                        WHEN EXCLUDED.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN EXCLUDED.policy_revision_id ELSE prodex_governance_sessions.policy_revision_id END,
                    provider_registry_revision = CASE
                        WHEN EXCLUDED.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN EXCLUDED.provider_registry_revision
                        ELSE prodex_governance_sessions.provider_registry_revision END,
                    provider_descriptor_revision = CASE
                        WHEN EXCLUDED.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN EXCLUDED.provider_descriptor_revision
                        ELSE prodex_governance_sessions.provider_descriptor_revision END,
                    provider_affinity = CASE
                        WHEN EXCLUDED.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN EXCLUDED.provider_affinity ELSE prodex_governance_sessions.provider_affinity END,
                    last_seen_at_unix_ms = GREATEST(
                        prodex_governance_sessions.last_seen_at_unix_ms,
                        EXCLUDED.last_seen_at_unix_ms
                    ),
                    absolute_expires_at_unix_ms = LEAST(
                        prodex_governance_sessions.absolute_expires_at_unix_ms,
                        EXCLUDED.absolute_expires_at_unix_ms
                    ),
                    idle_expires_at_unix_ms = CASE
                        WHEN EXCLUDED.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN EXCLUDED.idle_expires_at_unix_ms
                        ELSE prodex_governance_sessions.idle_expires_at_unix_ms END
                 WHERE prodex_governance_sessions.principal_id = EXCLUDED.principal_id
                   AND prodex_governance_sessions.channel = EXCLUDED.channel
                   AND prodex_governance_sessions.credential_scope = EXCLUDED.credential_scope",
                &[
                    &command.tenant_id.as_uuid(),
                    &command.session_id_hash,
                    &command.principal_id.as_uuid(),
                    &channel_label(command.channel),
                    &credential_scope_label(command.credential_scope),
                    &command.classification.as_str(),
                    &command.policy_revision_id.as_uuid(),
                    &command.provider_registry_revision,
                    &provider_descriptor_revision,
                    &command.provider_affinity,
                    &created_at,
                    &last_seen_at,
                    &absolute_expires_at,
                    &idle_expires_at,
                ],
            )
            .await
            .map_err(database_error)?;
        let stored =
            load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)
                .await?
                .ok_or(GovernanceRepositoryError::Database)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceSessionUpsertOutcome::Stored(Box::new(stored)))
    }

    pub async fn governance_load_sessions(
        &self,
        tenant_id: TenantId,
        now_unix_ms: u64,
        limit: u16,
    ) -> Result<Vec<GovernanceSessionRecord>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_load_sessions_inner(tenant_id, now_unix_ms, limit))
            .await
    }

    async fn governance_load_sessions_inner(
        &self,
        tenant_id: TenantId,
        now_unix_ms: u64,
        limit: u16,
    ) -> Result<Vec<GovernanceSessionRecord>, GovernanceRepositoryError> {
        if limit == 0 || limit > 4_096 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let now = to_i64(now_unix_ms)?;
        let limit = i64::from(limit);
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                "SELECT session.session_id_hash, session.principal_id, session.channel,
                        session.credential_scope, session.classification,
                        session.policy_revision_id, session.provider_registry_revision,
                        session.provider_descriptor_revision, session.provider_affinity,
                        session.created_at_unix_ms,
                        session.last_seen_at_unix_ms, session.absolute_expires_at_unix_ms,
                        session.idle_expires_at_unix_ms, revocation.revoked_at_unix_ms,
                        revocation.reason_code
                 FROM prodex_governance_sessions session
                 LEFT JOIN prodex_session_revocations revocation
                   ON revocation.tenant_id = session.tenant_id
                  AND revocation.session_id_hash = session.session_id_hash
                 WHERE session.tenant_id = $1
                   AND (revocation.session_id_hash IS NOT NULL
                        OR (session.absolute_expires_at_unix_ms > $2
                            AND session.idle_expires_at_unix_ms > $2))
                 ORDER BY session.last_seen_at_unix_ms DESC, session.session_id_hash
                 LIMIT $3",
                &[&tenant_id.as_uuid(), &now, &limit],
            )
            .await
            .map_err(database_error)?;
        let sessions = rows
            .into_iter()
            .map(|row| governance_session_from_row(tenant_id, &row))
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        transaction.commit().await.map_err(database_error)?;
        Ok(sessions)
    }

    pub async fn governance_count_concurrent_sessions(
        &self,
        tenant_id: TenantId,
        principal_id: PrincipalId,
        now_unix_ms: u64,
    ) -> Result<u64, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_count_concurrent_sessions_inner(
            tenant_id,
            principal_id,
            now_unix_ms,
        ))
        .await
    }

    async fn governance_count_concurrent_sessions_inner(
        &self,
        tenant_id: TenantId,
        principal_id: PrincipalId,
        now_unix_ms: u64,
    ) -> Result<u64, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let count =
            count_concurrent_sessions_tx(&transaction, tenant_id, principal_id, now_unix_ms)
                .await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(count)
    }

    pub async fn governance_session_revocation_epoch(
        &self,
        tenant_id: TenantId,
    ) -> Result<u64, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_session_revocation_epoch_inner(tenant_id))
            .await
    }

    async fn governance_session_revocation_epoch_inner(
        &self,
        tenant_id: TenantId,
    ) -> Result<u64, GovernanceRepositoryError> {
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let row = transaction
            .query_opt(
                "SELECT session_revocation_epoch FROM prodex_tenants WHERE tenant_id = $1",
                &[&tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?
            .ok_or(GovernanceRepositoryError::NotFound)?;
        let epoch = from_i64(row.get(0))?;
        transaction.commit().await.map_err(database_error)?;
        Ok(epoch)
    }

    pub async fn governance_revoke_session(
        &self,
        command: GovernanceSessionRevokeCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_revoke_session_inner(command))
            .await
    }

    async fn governance_revoke_session_inner(
        &self,
        command: GovernanceSessionRevokeCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        validate_governance_session_revoke(&command)?;
        let revoked_at = to_i64(command.revoked_at_unix_ms)?;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, command.tenant_id)
            .await
            .map_err(database_error)?;
        if load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)
            .await?
            .is_none()
        {
            return Err(GovernanceRepositoryError::NotFound);
        }
        let existing = transaction
            .query_opt(
                "SELECT revoked_at_unix_ms, reason_code FROM prodex_session_revocations
                 WHERE tenant_id = $1 AND session_id_hash = $2 FOR UPDATE",
                &[&command.tenant_id.as_uuid(), &command.session_id_hash],
            )
            .await
            .map_err(database_error)?;
        if let Some(existing) = existing {
            if existing.get::<_, String>(1) != command.reason_code {
                return Err(GovernanceRepositoryError::Conflict);
            }
            transaction.commit().await.map_err(database_error)?;
            return Ok(GovernanceWriteOutcome::Replayed);
        }
        transaction
            .execute(
                "INSERT INTO prodex_session_revocations (
                    tenant_id, session_id_hash, revoked_at_unix_ms, reason_code
                 ) VALUES ($1, $2, $3, $4)",
                &[
                    &command.tenant_id.as_uuid(),
                    &command.session_id_hash,
                    &revoked_at,
                    &command.reason_code,
                ],
            )
            .await
            .map_err(database_error)?;
        let updated = transaction
            .execute(
                "UPDATE prodex_tenants
                 SET session_revocation_epoch = session_revocation_epoch + 1
                 WHERE tenant_id = $1",
                &[&command.tenant_id.as_uuid()],
            )
            .await
            .map_err(database_error)?;
        if updated != 1 {
            return Err(GovernanceRepositoryError::NotFound);
        }
        append_audit_outbox_tx(&transaction, command.audit_outbox).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }

    pub async fn governance_claim_siem_outbox_batch(
        &self,
        tenant_id: TenantId,
        now_unix_ms: u64,
        batch_limit: u16,
        lease_ms: u64,
    ) -> Result<Vec<PostgresSiemOutboxClaim>, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_claim_siem_outbox_batch_inner(
            tenant_id,
            now_unix_ms,
            batch_limit,
            lease_ms,
        ))
        .await
    }

    async fn governance_claim_siem_outbox_batch_inner(
        &self,
        tenant_id: TenantId,
        now_unix_ms: u64,
        batch_limit: u16,
        lease_ms: u64,
    ) -> Result<Vec<PostgresSiemOutboxClaim>, GovernanceRepositoryError> {
        if batch_limit == 0 || batch_limit > 256 || lease_ms == 0 || lease_ms > 300_000 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let now = to_i64(now_unix_ms)?;
        let lease_expires_at = to_i64(
            now_unix_ms
                .checked_add(lease_ms)
                .ok_or(GovernanceRepositoryError::InvalidInput)?,
        )?;
        let limit = i64::from(batch_limit);
        let claim_token = Uuid::now_v7();
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        let rows = transaction
            .query(
                "WITH due AS (
                    SELECT tenant_id, event_id
                    FROM prodex_siem_outbox
                    WHERE tenant_id = $1
                      AND delivered_at_unix_ms IS NULL
                      AND next_attempt_at_unix_ms <= $2
                      AND (claim_expires_at_unix_ms IS NULL OR claim_expires_at_unix_ms <= $2)
                    ORDER BY next_attempt_at_unix_ms, event_id
                    FOR UPDATE SKIP LOCKED
                    LIMIT $3
                 )
                 UPDATE prodex_siem_outbox AS outbox
                 SET claim_token = $4, claim_expires_at_unix_ms = $5
                 FROM due
                 WHERE outbox.tenant_id = due.tenant_id AND outbox.event_id = due.event_id
                 RETURNING outbox.event_id, outbox.audit_event_id,
                           outbox.event_envelope, outbox.attempt_count",
                &[
                    &tenant_id.as_uuid(),
                    &now,
                    &limit,
                    &claim_token,
                    &lease_expires_at,
                ],
            )
            .await
            .map_err(database_error)?;
        let claims = rows
            .into_iter()
            .map(|row| {
                let envelope = row.get::<_, serde_json::Value>(2).to_string();
                Ok(PostgresSiemOutboxClaim {
                    tenant_id,
                    event_id: AuditEventId::from_uuid(row.get(0)),
                    audit_event_id: AuditEventId::from_uuid(row.get(1)),
                    event_envelope: envelope,
                    attempt_count: u8::try_from(row.get::<_, i32>(3))
                        .map_err(|_| GovernanceRepositoryError::Database)?,
                    claim_token,
                })
            })
            .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
        transaction.commit().await.map_err(database_error)?;
        Ok(claims)
    }

    pub async fn governance_finalize_siem_outbox_claim(
        &self,
        claim: &PostgresSiemOutboxClaim,
        delivered: bool,
        now_unix_ms: u64,
        retry_policy: SiemOutboxRetryPolicy,
    ) -> Result<SiemOutboxDeliveryDecision, GovernanceRepositoryError> {
        self.governance_timeout(self.governance_finalize_siem_outbox_claim_inner(
            claim,
            delivered,
            now_unix_ms,
            retry_policy,
        ))
        .await
    }

    async fn governance_finalize_siem_outbox_claim_inner(
        &self,
        claim: &PostgresSiemOutboxClaim,
        delivered: bool,
        now_unix_ms: u64,
        retry_policy: SiemOutboxRetryPolicy,
    ) -> Result<SiemOutboxDeliveryDecision, GovernanceRepositoryError> {
        let now = to_i64(now_unix_ms)?;
        let decision =
            plan_siem_outbox_delivery(retry_policy, claim.attempt_count, delivered, now_unix_ms);
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, claim.tenant_id)
            .await
            .map_err(database_error)?;
        let owned = transaction
            .query_opt(
                "SELECT 1 FROM prodex_siem_outbox
                 WHERE tenant_id = $1 AND event_id = $2 AND claim_token = $3
                   AND delivered_at_unix_ms IS NULL
                 FOR UPDATE",
                &[
                    &claim.tenant_id.as_uuid(),
                    &claim.event_id.as_uuid(),
                    &claim.claim_token,
                ],
            )
            .await
            .map_err(database_error)?;
        if owned.is_none() {
            return Err(GovernanceRepositoryError::Conflict);
        }
        match decision {
            SiemOutboxDeliveryDecision::Delivered => {
                transaction
                    .execute(
                        "UPDATE prodex_siem_outbox
                         SET attempt_count = attempt_count + 1, delivered_at_unix_ms = $4,
                             claim_token = NULL, claim_expires_at_unix_ms = NULL
                         WHERE tenant_id = $1 AND event_id = $2 AND claim_token = $3",
                        &[
                            &claim.tenant_id.as_uuid(),
                            &claim.event_id.as_uuid(),
                            &claim.claim_token,
                            &now,
                        ],
                    )
                    .await
                    .map_err(database_error)?;
            }
            SiemOutboxDeliveryDecision::RetryAt(next_attempt_at) => {
                let next_attempt_at = to_i64(next_attempt_at)?;
                transaction
                    .execute(
                        "UPDATE prodex_siem_outbox
                         SET attempt_count = attempt_count + 1, next_attempt_at_unix_ms = $4,
                             claim_token = NULL, claim_expires_at_unix_ms = NULL
                         WHERE tenant_id = $1 AND event_id = $2 AND claim_token = $3",
                        &[
                            &claim.tenant_id.as_uuid(),
                            &claim.event_id.as_uuid(),
                            &claim.claim_token,
                            &next_attempt_at,
                        ],
                    )
                    .await
                    .map_err(database_error)?;
            }
            SiemOutboxDeliveryDecision::DeadLetter => {
                let envelope = serde_json::from_str::<serde_json::Value>(&claim.event_envelope)
                    .map_err(|_| GovernanceRepositoryError::Database)?;
                let attempts = i32::from(claim.attempt_count.saturating_add(1));
                transaction
                    .execute(
                        "INSERT INTO prodex_siem_dead_letters (
                            tenant_id, event_id, audit_event_id, event_envelope,
                            attempt_count, stable_reason_code, failed_at_unix_ms
                         ) VALUES ($1, $2, $3, $4, $5, 'delivery_failed', $6)
                         ON CONFLICT DO NOTHING",
                        &[
                            &claim.tenant_id.as_uuid(),
                            &claim.event_id.as_uuid(),
                            &claim.audit_event_id.as_uuid(),
                            &envelope,
                            &attempts,
                            &now,
                        ],
                    )
                    .await
                    .map_err(database_error)?;
                transaction
                    .execute(
                        "DELETE FROM prodex_siem_outbox
                         WHERE tenant_id = $1 AND event_id = $2 AND claim_token = $3",
                        &[
                            &claim.tenant_id.as_uuid(),
                            &claim.event_id.as_uuid(),
                            &claim.claim_token,
                        ],
                    )
                    .await
                    .map_err(database_error)?;
            }
        }
        transaction.commit().await.map_err(database_error)?;
        Ok(decision)
    }

    pub async fn governance_append_audit_outbox(
        &self,
        command: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        self.governance_timeout(self.governance_append_audit_outbox_inner(command))
            .await
    }

    async fn governance_append_audit_outbox_inner(
        &self,
        command: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        let tenant_id = command.audit.event.tenant_id;
        let mut client = self.pool.get().await.map_err(database_error)?;
        let transaction = client.transaction().await.map_err(database_error)?;
        set_tenant_context(&transaction, tenant_id)
            .await
            .map_err(database_error)?;
        append_audit_outbox_tx(&transaction, command).await?;
        transaction.commit().await.map_err(database_error)
    }

    async fn governance_timeout<T>(
        &self,
        operation: impl Future<Output = Result<T, GovernanceRepositoryError>>,
    ) -> Result<T, GovernanceRepositoryError> {
        tokio::time::timeout(self.operation_timeout, operation)
            .await
            .map_err(|_| GovernanceRepositoryError::Database)?
    }
}

async fn load_revision_row(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
) -> Result<Option<RevisionRow>, GovernanceRepositoryError> {
    let statement = transaction
        .prepare_cached(LOAD_GOVERNANCE_REVISION_ARTIFACT_STATEMENT.sql)
        .await
        .map_err(database_error)?;
    transaction
        .query_opt(
            &statement,
            &[
                &tenant_id.as_uuid(),
                &artifact_kind_label(kind),
                &revision_id,
            ],
        )
        .await
        .map_err(database_error)?
        .map(|row| {
            Ok(RevisionRow {
                checksum: row.get(0),
                compiled_artifact: row.get(1),
                created_by: PrincipalId::from_uuid(row.get::<_, Uuid>(2)),
                created_at_unix_ms: from_i64(row.get(3))?,
            })
        })
        .transpose()
}

async fn load_verified_snapshot(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    source: GovernanceSnapshotSource,
    validate_artifact: &mut impl FnMut(&[u8]) -> bool,
) -> Result<Option<GovernanceSnapshot>, GovernanceRepositoryError> {
    let Some(revision) = load_revision_row(transaction, tenant_id, kind, revision_id).await? else {
        return Ok(None);
    };
    if revision.checksum != artifact_checksum(&revision.compiled_artifact)
        || !validate_artifact(&revision.compiled_artifact)
    {
        return Ok(None);
    }
    Ok(Some(GovernanceSnapshot {
        tenant_id,
        kind,
        revision_id: revision_id.to_string(),
        compiled_artifact: revision.compiled_artifact,
        source,
    }))
}

async fn revision_id_for_fingerprint(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    fingerprint: &str,
) -> Result<Option<String>, GovernanceRepositoryError> {
    transaction
        .query_opt(
            "SELECT revision_id FROM prodex_governance_revision_artifacts
             WHERE tenant_id = $1 AND artifact_kind = $2 AND artifact_checksum = $3",
            &[
                &tenant_id.as_uuid(),
                &artifact_kind_label(kind),
                &fingerprint,
            ],
        )
        .await
        .map_err(database_error)
        .map(|row| row.map(|row| row.get(0)))
}

async fn insert_revision_metadata(
    transaction: &Transaction<'_>,
    command: &GovernanceRevisionWriteCommand,
    checksum: &str,
    created_at: i64,
) -> Result<(), GovernanceRepositoryError> {
    let metadata = serde_json::json!({ "artifact_checksum": checksum });
    let inserted = match command.kind {
        GovernanceArtifactKind::Policy => {
            let revision_id = policy_revision_id(&command.revision_id)?;
            transaction
                .query_opt(
                    "INSERT INTO prodex_policy_revisions (
                        tenant_id, revision_id, artifact_checksum, compiled_metadata,
                        lifecycle_state, created_by, created_at_unix_ms
                     ) VALUES ($1, $2, $3, $4, 'draft', $5, $6)
                     ON CONFLICT DO NOTHING RETURNING revision_id",
                    &[
                        &command.tenant_id.as_uuid(),
                        &revision_id.as_uuid(),
                        &checksum,
                        &metadata,
                        &command.created_by.as_uuid(),
                        &created_at,
                    ],
                )
                .await
        }
        GovernanceArtifactKind::ClassificationRules => {
            transaction
                .query_opt(
                    "INSERT INTO prodex_classification_rule_revisions (
                        tenant_id, revision_id, artifact_checksum, compiled_metadata,
                        lifecycle_state, created_at_unix_ms
                     ) VALUES ($1, $2, $3, $4, 'draft', $5)
                     ON CONFLICT DO NOTHING RETURNING revision_id",
                    &[
                        &command.tenant_id.as_uuid(),
                        &command.revision_id,
                        &checksum,
                        &metadata,
                        &created_at,
                    ],
                )
                .await
        }
        GovernanceArtifactKind::ProviderRegistry => {
            transaction
                .query_opt(
                    "INSERT INTO prodex_provider_registry_revisions (
                        tenant_id, revision_id, artifact_checksum, lifecycle_state,
                        created_at_unix_ms
                     ) VALUES ($1, $2, $3, 'draft', $4)
                     ON CONFLICT DO NOTHING RETURNING revision_id",
                    &[
                        &command.tenant_id.as_uuid(),
                        &command.revision_id,
                        &checksum,
                        &created_at,
                    ],
                )
                .await
        }
        GovernanceArtifactKind::RoutingScores => {
            transaction
                .query_opt(
                    "INSERT INTO prodex_routing_score_revisions (
                        tenant_id, revision_id, artifact_checksum, fixed_point_weights,
                        lifecycle_state, created_at_unix_ms
                     ) VALUES ($1, $2, $3, $4, 'draft', $5)
                     ON CONFLICT DO NOTHING RETURNING revision_id",
                    &[
                        &command.tenant_id.as_uuid(),
                        &command.revision_id,
                        &checksum,
                        &metadata,
                        &created_at,
                    ],
                )
                .await
        }
    }
    .map_err(database_error)?;
    if inserted.is_none() {
        return Err(GovernanceRepositoryError::Conflict);
    }
    Ok(())
}

async fn update_revision_state(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    state: &str,
) -> Result<(), GovernanceRepositoryError> {
    let query = format!(
        "UPDATE {} SET lifecycle_state = $3 WHERE tenant_id = $1 AND revision_id = $2",
        revision_table(kind)
    );
    let changed = if kind == GovernanceArtifactKind::Policy {
        let revision_id = policy_revision_id(revision_id)?;
        transaction
            .execute(
                &query,
                &[&tenant_id.as_uuid(), &revision_id.as_uuid(), &state],
            )
            .await
    } else {
        transaction
            .execute(&query, &[&tenant_id.as_uuid(), &revision_id, &state])
            .await
    }
    .map_err(database_error)?;
    if changed != 1 {
        return Err(GovernanceRepositoryError::NotFound);
    }
    Ok(())
}

fn revision_table(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "prodex_policy_revisions",
        GovernanceArtifactKind::ClassificationRules => "prodex_classification_rule_revisions",
        GovernanceArtifactKind::ProviderRegistry => "prodex_provider_registry_revisions",
        GovernanceArtifactKind::RoutingScores => "prodex_routing_score_revisions",
    }
}

fn revision_id_from_row(row: &Row, index: usize, kind: GovernanceArtifactKind) -> String {
    if kind == GovernanceArtifactKind::Policy {
        row.get::<_, Uuid>(index).to_string()
    } else {
        row.get(index)
    }
}

fn artifact_kind_for_approval(
    kind: ApprovalKind,
) -> Result<GovernanceArtifactKind, GovernanceRepositoryError> {
    match kind {
        ApprovalKind::PolicyRevision => Ok(GovernanceArtifactKind::Policy),
        ApprovalKind::ClassificationRuleRevision => Ok(GovernanceArtifactKind::ClassificationRules),
        ApprovalKind::ProviderRegistryRevision => Ok(GovernanceArtifactKind::ProviderRegistry),
        ApprovalKind::RoutingScoreRevision => Ok(GovernanceArtifactKind::RoutingScores),
        ApprovalKind::HighImpactConfiguration
        | ApprovalKind::Execution
        | ApprovalKind::BreakGlass => Err(GovernanceRepositoryError::InvalidInput),
    }
}

fn approval_artifact_kind(
    approval_kind: ApprovalKind,
) -> Result<Option<GovernanceArtifactKind>, GovernanceRepositoryError> {
    match approval_kind {
        ApprovalKind::Execution => Ok(None),
        ApprovalKind::HighImpactConfiguration | ApprovalKind::BreakGlass => {
            Err(GovernanceRepositoryError::InvalidInput)
        }
        _ => artifact_kind_for_approval(approval_kind).map(Some),
    }
}

fn approval_kind_for_artifact(kind: GovernanceArtifactKind) -> ApprovalKind {
    match kind {
        GovernanceArtifactKind::Policy => ApprovalKind::PolicyRevision,
        GovernanceArtifactKind::ClassificationRules => ApprovalKind::ClassificationRuleRevision,
        GovernanceArtifactKind::ProviderRegistry => ApprovalKind::ProviderRegistryRevision,
        GovernanceArtifactKind::RoutingScores => ApprovalKind::RoutingScoreRevision,
    }
}

async fn persist_approval_transition(
    transaction: &Transaction<'_>,
    previous: &ApprovalRecord,
    next: &ApprovalRecord,
) -> Result<(), GovernanceRepositoryError> {
    let previous_version = to_i64(previous.version)?;
    let next_version = to_i64(next.version)?;
    let activated_at = next.activated_at_unix_ms.map(to_i64).transpose()?;
    let changed = transaction
        .execute(
            "UPDATE prodex_approvals
             SET lifecycle_state = $4, activated_at_unix_ms = $5,
                 termination_reason = $6, resource_version = $7
             WHERE tenant_id = $1 AND approval_id = $2 AND resource_version = $3",
            &[
                &next.tenant_id.as_uuid(),
                &next.id.as_str(),
                &previous_version,
                &approval_state_label(next.state),
                &activated_at,
                &next
                    .termination_reason
                    .as_ref()
                    .map(ApprovalReasonCode::as_str),
                &next_version,
            ],
        )
        .await
        .map_err(database_error)?;
    if changed != 1 {
        return Err(GovernanceRepositoryError::Conflict);
    }
    for vote in &next.votes {
        let approved_at = to_i64(vote.approved_at_unix_ms)?;
        transaction
            .execute(
                "INSERT INTO prodex_approval_votes (
                    tenant_id, approval_id, checker_id, approved_at_unix_ms
                 ) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
                &[
                    &next.tenant_id.as_uuid(),
                    &next.id.as_str(),
                    &vote.checker.as_uuid(),
                    &approved_at,
                ],
            )
            .await
            .map_err(database_error)?;
    }
    Ok(())
}

async fn load_approval_tx(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    approval_id: &ApprovalId,
) -> Result<Option<ApprovalRecord>, GovernanceRepositoryError> {
    let row = transaction
        .query_opt(
            "SELECT approval_kind, approval_scope, fingerprint, maker_id, lifecycle_state,
                    required_quorum, expires_at_unix_ms, activated_at_unix_ms,
                    termination_reason, resource_version
             FROM prodex_approvals WHERE tenant_id = $1 AND approval_id = $2 FOR UPDATE",
            &[&tenant_id.as_uuid(), &approval_id.as_str()],
        )
        .await
        .map_err(database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let vote_rows = transaction
        .query(
            "SELECT checker_id, approved_at_unix_ms FROM prodex_approval_votes
             WHERE tenant_id = $1 AND approval_id = $2 ORDER BY checker_id",
            &[&tenant_id.as_uuid(), &approval_id.as_str()],
        )
        .await
        .map_err(database_error)?;
    let votes = vote_rows
        .into_iter()
        .map(|row| {
            Ok(ApprovalVote {
                checker: PrincipalId::from_uuid(row.get::<_, Uuid>(0)),
                approved_at_unix_ms: from_i64(row.get(1))?,
            })
        })
        .collect::<Result<Vec<_>, GovernanceRepositoryError>>()?;
    approval_from_row(row, approval_id.clone(), tenant_id, votes).map(Some)
}

fn approval_from_row(
    row: Row,
    id: ApprovalId,
    tenant_id: TenantId,
    votes: Vec<ApprovalVote>,
) -> Result<ApprovalRecord, GovernanceRepositoryError> {
    let kind = approval_kind_from_label(row.get::<_, &str>(0))?;
    let scope = ApprovalScope::new(row.get::<_, String>(1))
        .map_err(|_| GovernanceRepositoryError::Database)?;
    let fingerprint = ApprovalFingerprint::new(row.get::<_, String>(2))
        .map_err(|_| GovernanceRepositoryError::Database)?;
    let state = approval_state_from_label(row.get::<_, &str>(4))?;
    Ok(ApprovalRecord {
        id,
        tenant_id,
        kind,
        scope,
        fingerprint,
        maker: PrincipalId::from_uuid(row.get::<_, Uuid>(3)),
        state,
        required_quorum: u8::try_from(row.get::<_, i16>(5))
            .map_err(|_| GovernanceRepositoryError::Database)?,
        votes,
        expires_at_unix_ms: from_i64(row.get(6))?,
        activated_at_unix_ms: row.get::<_, Option<i64>>(7).map(from_i64).transpose()?,
        termination_reason: row
            .get::<_, Option<String>>(8)
            .map(ApprovalReasonCode::new)
            .transpose()
            .map_err(|_| GovernanceRepositoryError::Database)?,
        version: from_i64(row.get(9))?,
    })
}

async fn load_pointer_for_kind(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
) -> Result<Option<GovernancePointer>, GovernanceRepositoryError> {
    let statement = transaction
        .prepare_cached(postgres_governance_pointer_statements(kind).load.sql)
        .await
        .map_err(database_error)?;
    transaction
        .query_opt(&statement, &[&tenant_id.as_uuid()])
        .await
        .map_err(database_error)
        .map(|row| {
            row.map(|row| {
                let (active_revision_id, last_known_good_revision_id) = match kind {
                    GovernanceArtifactKind::Policy => (
                        row.get::<_, Option<Uuid>>(0).map(|value| value.to_string()),
                        row.get::<_, Option<Uuid>>(1).map(|value| value.to_string()),
                    ),
                    GovernanceArtifactKind::ClassificationRules
                    | GovernanceArtifactKind::ProviderRegistry
                    | GovernanceArtifactKind::RoutingScores => (
                        row.get::<_, Option<String>>(0),
                        row.get::<_, Option<String>>(1),
                    ),
                };
                GovernancePointer {
                    active_revision_id,
                    last_known_good_revision_id,
                    etag: row.get(2),
                }
            })
        })
}

async fn insert_activation_history(
    transaction: &Transaction<'_>,
    request: &GovernanceActivationRequest,
    previous_revision_id: Option<&str>,
    occurred_at: i64,
) -> Result<(), GovernanceRepositoryError> {
    let activation_id = request.audit_outbox.audit.event.id.as_uuid();
    if request.kind == GovernanceArtifactKind::Policy {
        let revision_id = policy_revision_id(&request.revision_id)?;
        let previous_revision_id = previous_revision_id
            .map(policy_revision_id)
            .transpose()?
            .map(PolicyRevisionId::as_uuid);
        transaction
            .execute(
                "INSERT INTO prodex_policy_activation_history (
                    tenant_id, activation_id, revision_id, previous_revision_id,
                    action, actor_id, occurred_at_unix_ms
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7)",
                &[
                    &request.tenant_id.as_uuid(),
                    &activation_id,
                    &revision_id.as_uuid(),
                    &previous_revision_id,
                    &request.action.as_str(),
                    &request.actor.id.as_uuid(),
                    &occurred_at,
                ],
            )
            .await
    } else {
        transaction
            .execute(
                "INSERT INTO prodex_governance_activation_history (
                    tenant_id, activation_id, artifact_kind, revision_id,
                    previous_revision_id, action, actor_id, idempotency_key,
                    occurred_at_unix_ms
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                &[
                    &request.tenant_id.as_uuid(),
                    &activation_id,
                    &artifact_kind_label(request.kind),
                    &request.revision_id,
                    &previous_revision_id,
                    &request.action.as_str(),
                    &request.actor.id.as_uuid(),
                    &request.idempotency_key.as_str(),
                    &occurred_at,
                ],
            )
            .await
    }
    .map(|_| ())
    .map_err(database_error)
}

async fn load_activation_replay(
    transaction: &Transaction<'_>,
    request: &GovernanceActivationRequest,
) -> Result<Option<GovernanceActivationResult>, GovernanceRepositoryError> {
    let row = transaction
        .query_opt(
            "SELECT request_fingerprint, action, revision_id, resulting_etag
             FROM prodex_governance_mutation_idempotency
             WHERE tenant_id = $1 AND artifact_kind = $2 AND idempotency_key = $3",
            &[
                &request.tenant_id.as_uuid(),
                &artifact_kind_label(request.kind),
                &request.idempotency_key.as_str(),
            ],
        )
        .await
        .map_err(database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let fingerprint = row.get::<_, String>(0);
    let action = row.get::<_, String>(1);
    let revision_id = row.get::<_, String>(2);
    let etag = row.get::<_, String>(3);
    if fingerprint != request.request_fingerprint
        || action != request.action.as_str()
        || revision_id != request.revision_id
    {
        return Err(GovernanceRepositoryError::Conflict);
    }
    let pointer = load_pointer_for_kind(transaction, request.tenant_id, request.kind)
        .await?
        .ok_or(GovernanceRepositoryError::Database)?;
    Ok(Some(GovernanceActivationResult {
        outcome: GovernanceWriteOutcome::Replayed,
        kind: request.kind,
        revision_id,
        etag,
        last_known_good_revision_id: pointer
            .last_known_good_revision_id
            .ok_or(GovernanceRepositoryError::Database)?,
    }))
}

async fn load_governance_session_tx(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    session_id_hash: &str,
) -> Result<Option<GovernanceSessionRecord>, GovernanceRepositoryError> {
    transaction
        .query_opt(
            "SELECT session.session_id_hash, session.principal_id, session.channel,
                    session.credential_scope, session.classification,
                    session.policy_revision_id, session.provider_registry_revision,
                    session.provider_descriptor_revision, session.provider_affinity,
                    session.created_at_unix_ms,
                    session.last_seen_at_unix_ms, session.absolute_expires_at_unix_ms,
                    session.idle_expires_at_unix_ms, revocation.revoked_at_unix_ms,
                    revocation.reason_code
             FROM prodex_governance_sessions session
             LEFT JOIN prodex_session_revocations revocation
               ON revocation.tenant_id = session.tenant_id
              AND revocation.session_id_hash = session.session_id_hash
             WHERE session.tenant_id = $1 AND session.session_id_hash = $2
             FOR UPDATE OF session",
            &[&tenant_id.as_uuid(), &session_id_hash],
        )
        .await
        .map_err(database_error)?
        .map(|row| governance_session_from_row(tenant_id, &row))
        .transpose()
}

fn governance_session_from_row(
    tenant_id: TenantId,
    row: &Row,
) -> Result<GovernanceSessionRecord, GovernanceRepositoryError> {
    Ok(GovernanceSessionRecord {
        tenant_id,
        session_id_hash: row.get(0),
        principal_id: PrincipalId::from_uuid(row.get::<_, Uuid>(1)),
        channel: channel_from_label(row.get::<_, &str>(2))?,
        credential_scope: credential_scope_from_label(row.get::<_, &str>(3))?,
        classification: classification_from_label(row.get::<_, &str>(4))?,
        policy_revision_id: PolicyRevisionId::from_uuid(row.get::<_, Uuid>(5)),
        provider_registry_revision: row.get(6),
        provider_descriptor_revision: from_i64(row.get(7))?,
        provider_affinity: row.get(8),
        created_at_unix_ms: from_i64(row.get(9))?,
        last_seen_at_unix_ms: from_i64(row.get(10))?,
        absolute_expires_at_unix_ms: from_i64(row.get(11))?,
        idle_expires_at_unix_ms: from_i64(row.get(12))?,
        revoked_at_unix_ms: row.get::<_, Option<i64>>(13).map(from_i64).transpose()?,
        revocation_reason_code: row.get(14),
    })
}

async fn count_concurrent_sessions_tx(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    principal_id: PrincipalId,
    now_unix_ms: u64,
) -> Result<u64, GovernanceRepositoryError> {
    let now = to_i64(now_unix_ms)?;
    let count = transaction
        .query_one(
            "SELECT COUNT(*) FROM prodex_governance_sessions session
             WHERE session.tenant_id = $1 AND session.principal_id = $2
               AND session.absolute_expires_at_unix_ms > $3
               AND session.idle_expires_at_unix_ms > $3
               AND NOT EXISTS (
                   SELECT 1 FROM prodex_session_revocations revocation
                   WHERE revocation.tenant_id = session.tenant_id
                     AND revocation.session_id_hash = session.session_id_hash
               )",
            &[&tenant_id.as_uuid(), &principal_id.as_uuid(), &now],
        )
        .await
        .map_err(database_error)?
        .get::<_, i64>(0);
    from_i64(count)
}

async fn latest_audit_digest_tx(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
) -> Result<Option<String>, GovernanceRepositoryError> {
    transaction
        .query_opt(
            "SELECT audit.event_digest
             FROM prodex_audit_log audit
             WHERE audit.tenant_id = $1
               AND NOT EXISTS (
                   SELECT 1 FROM prodex_audit_log child
                   WHERE child.tenant_id = audit.tenant_id
                     AND child.previous_digest = audit.event_digest
               )
             ORDER BY audit.occurred_at_unix_ms DESC, audit.audit_event_id DESC
             LIMIT 1",
            &[&tenant_id.as_uuid()],
        )
        .await
        .map_err(database_error)
        .map(|row| row.map(|row| row.get(0)))
}

async fn append_audit_outbox_tx(
    transaction: &Transaction<'_>,
    command: AuditOutboxWriteCommand,
) -> Result<(), GovernanceRepositoryError> {
    let plan =
        plan_audit_outbox_write(command).map_err(|_| GovernanceRepositoryError::TenantMismatch)?;
    let envelope = plan.audit.envelope;
    let tenant_id = envelope.event.tenant_id;
    let tenant_lock = tenant_id.to_string();
    transaction
        .query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            &[&tenant_lock],
        )
        .await
        .map_err(database_error)?;
    let current = latest_audit_digest_tx(transaction, tenant_id).await?;
    if current.as_deref()
        != envelope
            .previous_digest
            .as_ref()
            .map(|value| value.as_str())
    {
        return Err(GovernanceRepositoryError::AuditChainConflict);
    }
    let event_envelope =
        serde_json::to_value(&envelope).map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    let occurred_at = to_i64(envelope.event.occurred_at_unix_ms)?;
    let statement = transaction
        .prepare_cached(APPEND_AUDIT_OUTBOX_ATOMIC_STATEMENT.sql)
        .await
        .map_err(database_error)?;
    transaction
        .query_one(
            &statement,
            &[
                &tenant_id.as_uuid(),
                &envelope.event.id.as_uuid(),
                &envelope
                    .previous_digest
                    .as_ref()
                    .map(|value| value.as_str()),
                &envelope.event_digest.as_str(),
                &occurred_at,
                &envelope.event.principal_id.as_uuid(),
                &envelope.event.action.as_str(),
                &envelope.event.resource.kind,
                &envelope.event.resource.id,
                &envelope.event.outcome.as_str(),
                &envelope.event.reason_code,
                &plan.outbox_event_id.as_uuid(),
                &event_envelope,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

fn require_control_plane_admin(
    tenant_id: TenantId,
    actor: &Principal,
) -> Result<(), GovernanceRepositoryError> {
    if actor.tenant_id != Some(tenant_id) {
        return Err(GovernanceRepositoryError::TenantMismatch);
    }
    if actor.credential_scope != CredentialScope::ControlPlane || actor.role != Role::Admin {
        return Err(GovernanceRepositoryError::ApprovalRequired);
    }
    Ok(())
}

async fn approval_idempotency_replay_postgres(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    idempotency: &ApprovalVoteIdempotency,
) -> Result<IdempotencyReplayDecision<Vec<u8>>, GovernanceRepositoryError> {
    if idempotency.operation.tenant_id != tenant_id || idempotency.started_at_unix_ms == 0 {
        return Err(GovernanceRepositoryError::InvalidInput);
    }
    let tenant_lock = tenant_id.to_string();
    transaction
        .query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            &[&tenant_lock],
        )
        .await
        .map_err(database_error)?;
    let row = transaction
        .query_opt(
            "SELECT request_fingerprint, entry_status, started_at_unix_ms,
                    completed_at_unix_ms, response_body
             FROM prodex_idempotency_records
             WHERE tenant_id = $1 AND idempotency_key = $2",
            &[&tenant_id.as_uuid(), &idempotency.operation.key.as_str()],
        )
        .await
        .map_err(database_error)?
        .map(|row| {
            let status = match row.get::<_, String>(1).as_str() {
                "pending" => Ok(IdempotencyRecordLookupRowStatus::Pending),
                "completed" => Ok(IdempotencyRecordLookupRowStatus::Completed),
                _ => Err(GovernanceRepositoryError::InvalidInput),
            }?;
            Ok(IdempotencyRecordLookupRow {
                tenant_id,
                idempotency_key: idempotency.operation.key.clone(),
                request_fingerprint: row.get(0),
                status,
                started_at_unix_ms: from_i64(row.get(2))?,
                completed_at_unix_ms: row.get::<_, Option<i64>>(3).map(from_i64).transpose()?,
                response_body: row.get(4),
            })
        })
        .transpose()?;
    let entry = row
        .map(|row| materialize_idempotency_record_lookup_row(&idempotency.operation, row))
        .transpose()
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    decide_idempotency_replay(&idempotency.operation, entry.as_ref())
        .map_err(|_| GovernanceRepositoryError::Conflict)
}

async fn insert_approval_idempotency_pending_postgres(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    idempotency: &ApprovalVoteIdempotency,
) -> Result<(), GovernanceRepositoryError> {
    let inserted = transaction
        .execute(
            "INSERT INTO prodex_idempotency_records (
                tenant_id, idempotency_key, request_fingerprint, entry_status, started_at_unix_ms
             ) VALUES ($1, $2, $3, 'pending', $4)",
            &[
                &tenant_id.as_uuid(),
                &idempotency.operation.key.as_str(),
                &idempotency.operation.request_fingerprint,
                &to_i64(idempotency.started_at_unix_ms)?,
            ],
        )
        .await
        .map_err(database_error)?;
    if inserted != 1 {
        return Err(GovernanceRepositoryError::Conflict);
    }
    Ok(())
}

async fn complete_approval_idempotency_postgres(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    idempotency: &ApprovalVoteIdempotency,
    outcome: ApprovalVoteStableOutcome,
    completed_at_unix_ms: u64,
) -> Result<(), GovernanceRepositoryError> {
    let response = outcome.encode();
    let updated = transaction
        .execute(
            "UPDATE prodex_idempotency_records
             SET entry_status = 'completed', completed_at_unix_ms = $4, response_body = $5
             WHERE tenant_id = $1 AND idempotency_key = $2
               AND request_fingerprint = $3 AND entry_status = 'pending'",
            &[
                &tenant_id.as_uuid(),
                &idempotency.operation.key.as_str(),
                &idempotency.operation.request_fingerprint,
                &to_i64(completed_at_unix_ms)?,
                &response,
            ],
        )
        .await
        .map_err(database_error)?;
    if updated != 1 {
        return Err(GovernanceRepositoryError::Conflict);
    }
    Ok(())
}

fn approval_transition_error(error: ApprovalError) -> GovernanceRepositoryError {
    match error {
        ApprovalError::SelfApprovalDenied => GovernanceRepositoryError::ApprovalSelfAction,
        ApprovalError::StaleVersion => GovernanceRepositoryError::StaleVersion,
        ApprovalError::ReplayMismatch => GovernanceRepositoryError::Conflict,
        ApprovalError::InvalidTransition => GovernanceRepositoryError::InvalidTransition,
        ApprovalError::TenantMismatch => GovernanceRepositoryError::TenantMismatch,
        ApprovalError::InvalidToken
        | ApprovalError::InvalidQuorum
        | ApprovalError::InvalidExpiry => GovernanceRepositoryError::InvalidInput,
    }
}

fn artifact_kind_label(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "policy",
        GovernanceArtifactKind::ClassificationRules => "classification_rules",
        GovernanceArtifactKind::ProviderRegistry => "provider_registry",
        GovernanceArtifactKind::RoutingScores => "routing_scores",
    }
}

fn validate_governance_session_upsert(
    command: &GovernanceSessionUpsertCommand,
) -> Result<(), GovernanceRepositoryError> {
    if !session_hash_is_valid(&command.session_id_hash)
        || !bounded_session_token(&command.provider_registry_revision)
        || command.provider_descriptor_revision == 0
        || command
            .provider_affinity
            .as_deref()
            .is_some_and(|value| !bounded_session_token(value))
        || command.created_at_unix_ms == 0
        || command.created_at_unix_ms > command.last_seen_at_unix_ms
        || command.last_seen_at_unix_ms >= command.absolute_expires_at_unix_ms
        || command.last_seen_at_unix_ms >= command.idle_expires_at_unix_ms
        || command.max_concurrent == Some(0)
    {
        return Err(GovernanceRepositoryError::InvalidInput);
    }
    Ok(())
}

fn validate_governance_session_revoke(
    command: &GovernanceSessionRevokeCommand,
) -> Result<(), GovernanceRepositoryError> {
    if !session_hash_is_valid(&command.session_id_hash)
        || command.revoked_at_unix_ms == 0
        || !bounded_session_token(&command.reason_code)
        || command.reason_code.len() > 64
    {
        return Err(GovernanceRepositoryError::InvalidInput);
    }
    Ok(())
}

fn session_hash_is_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn bounded_session_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
}

fn channel_label(channel: Channel) -> &'static str {
    match channel {
        Channel::Cli => "cli",
        Channel::Ide => "ide",
        Channel::Api => "api",
        Channel::Mcp => "mcp",
        Channel::InternalService => "internal_service",
    }
}

fn channel_from_label(value: &str) -> Result<Channel, GovernanceRepositoryError> {
    match value {
        "cli" => Ok(Channel::Cli),
        "ide" => Ok(Channel::Ide),
        "api" => Ok(Channel::Api),
        "mcp" => Ok(Channel::Mcp),
        "internal_service" => Ok(Channel::InternalService),
        _ => Err(GovernanceRepositoryError::Database),
    }
}

fn credential_scope_label(scope: CredentialScope) -> &'static str {
    match scope {
        CredentialScope::DataPlane => "data_plane",
        CredentialScope::ControlPlane => "control_plane",
        CredentialScope::BreakGlass => "break_glass",
    }
}

fn credential_scope_from_label(value: &str) -> Result<CredentialScope, GovernanceRepositoryError> {
    match value {
        "data_plane" => Ok(CredentialScope::DataPlane),
        "control_plane" => Ok(CredentialScope::ControlPlane),
        "break_glass" => Ok(CredentialScope::BreakGlass),
        _ => Err(GovernanceRepositoryError::Database),
    }
}

fn classification_from_label(value: &str) -> Result<DataClassification, GovernanceRepositoryError> {
    match value {
        "public" => Ok(DataClassification::Public),
        "internal" => Ok(DataClassification::Internal),
        "confidential" => Ok(DataClassification::Confidential),
        "restricted" => Ok(DataClassification::Restricted),
        _ => Err(GovernanceRepositoryError::Database),
    }
}

fn approval_kind_label(kind: ApprovalKind) -> &'static str {
    match kind {
        ApprovalKind::PolicyRevision => "policy_revision",
        ApprovalKind::ClassificationRuleRevision => "classification_rule_revision",
        ApprovalKind::ProviderRegistryRevision => "provider_registry_revision",
        ApprovalKind::RoutingScoreRevision => "routing_score_revision",
        ApprovalKind::HighImpactConfiguration => "high_impact_configuration",
        ApprovalKind::Execution => "execution",
        ApprovalKind::BreakGlass => "break_glass",
    }
}

fn approval_kind_from_label(value: &str) -> Result<ApprovalKind, GovernanceRepositoryError> {
    match value {
        "policy_revision" => Ok(ApprovalKind::PolicyRevision),
        "classification_rule_revision" => Ok(ApprovalKind::ClassificationRuleRevision),
        "provider_registry_revision" => Ok(ApprovalKind::ProviderRegistryRevision),
        "routing_score_revision" => Ok(ApprovalKind::RoutingScoreRevision),
        "high_impact_configuration" => Ok(ApprovalKind::HighImpactConfiguration),
        "execution" => Ok(ApprovalKind::Execution),
        "break_glass" => Ok(ApprovalKind::BreakGlass),
        _ => Err(GovernanceRepositoryError::Database),
    }
}

fn approval_state_label(state: ApprovalState) -> &'static str {
    match state {
        ApprovalState::Draft => "draft",
        ApprovalState::PendingApproval => "pending_approval",
        ApprovalState::Approved => "approved",
        ApprovalState::Rejected => "rejected",
        ApprovalState::Expired => "expired",
        ApprovalState::Cancelled => "cancelled",
        ApprovalState::Active => "active",
        ApprovalState::Superseded => "superseded",
        ApprovalState::RolledBack => "rolled_back",
    }
}

fn approval_state_from_label(value: &str) -> Result<ApprovalState, GovernanceRepositoryError> {
    match value {
        "draft" => Ok(ApprovalState::Draft),
        "pending_approval" => Ok(ApprovalState::PendingApproval),
        "approved" => Ok(ApprovalState::Approved),
        "rejected" => Ok(ApprovalState::Rejected),
        "expired" => Ok(ApprovalState::Expired),
        "cancelled" => Ok(ApprovalState::Cancelled),
        "active" => Ok(ApprovalState::Active),
        "superseded" => Ok(ApprovalState::Superseded),
        "rolled_back" => Ok(ApprovalState::RolledBack),
        _ => Err(GovernanceRepositoryError::Database),
    }
}

fn policy_revision_id(value: &str) -> Result<PolicyRevisionId, GovernanceRepositoryError> {
    PolicyRevisionId::from_str(value).map_err(|_| GovernanceRepositoryError::InvalidInput)
}

fn activation_etag(request: &GovernanceActivationRequest, previous_etag: Option<&str>) -> String {
    let material = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        request.tenant_id,
        artifact_kind_label(request.kind),
        request.revision_id,
        request.action.as_str(),
        previous_etag.unwrap_or(""),
        request.idempotency_key.as_str(),
    );
    artifact_checksum(material.as_bytes())
}

fn artifact_checksum(artifact: &[u8]) -> String {
    let digest = Sha256::digest(artifact);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

fn to_i64(value: u64) -> Result<i64, GovernanceRepositoryError> {
    i64::try_from(value).map_err(|_| GovernanceRepositoryError::InvalidInput)
}

fn from_i64(value: i64) -> Result<u64, GovernanceRepositoryError> {
    u64::try_from(value).map_err(|_| GovernanceRepositoryError::Database)
}

fn database_error<E>(_: E) -> GovernanceRepositoryError {
    GovernanceRepositoryError::Database
}
