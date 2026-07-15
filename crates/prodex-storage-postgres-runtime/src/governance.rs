//! Pooled PostgreSQL execution for policy governance.

mod approvals;
mod audit_outbox;
mod revisions;
mod sessions;

use approvals::*;
use audit_outbox::*;
use revisions::*;
use sessions::*;

use std::future::Future;
use std::str::FromStr;

use deadpool_postgres::Transaction;
use prodex_domain::{
    ApprovalAction, ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalReasonCode,
    ApprovalRecord, ApprovalScope, ApprovalState, ApprovalTransitionRequest, ApprovalVote,
    AuditDigest, AuditEventId, IdempotencyReplayDecision, PolicyRevisionId, PrincipalId, TenantId,
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
    SiemOutboxRetryPolicy, denied_approval_audit_outbox,
    governance_support::{
        activation_etag, approval_artifact_kind, approval_kind_for_artifact,
        approval_kind_from_label, approval_kind_label, approval_state_from_label,
        approval_state_label, approval_transition_error, artifact_checksum,
        artifact_kind_for_approval, artifact_kind_label, channel_from_label, channel_label,
        classification_from_label, credential_scope_from_label, credential_scope_label, from_i64,
        require_control_plane_admin, revision_table, to_i64, validate_governance_session_revoke,
        validate_governance_session_upsert,
    },
    materialize_idempotency_record_lookup_row, plan_audit_outbox_write,
    plan_governance_revision_write, plan_siem_outbox_delivery, verify_governance_audit_integrity,
};
use prodex_storage_postgres::{
    APPEND_AUDIT_OUTBOX_ATOMIC_STATEMENT, INSERT_GOVERNANCE_REVISION_ARTIFACT_STATEMENT,
    LOAD_GOVERNANCE_REVISION_ARTIFACT_STATEMENT, postgres_governance_pointer_statements,
};
use tokio_postgres::Row;
use uuid::Uuid;

use super::{PostgresRepository, set_tenant_context};

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

fn database_error<E>(_: E) -> GovernanceRepositoryError {
    GovernanceRepositoryError::Database
}
