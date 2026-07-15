//! Executing SQLite adapter for authoritative governance lifecycle state.

mod approvals;
mod audit_outbox;
mod revisions;
mod sessions;

use approvals::*;
use audit_outbox::*;
use revisions::*;
use sessions::*;

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

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
    GovernanceAuditExporter, GovernanceAuditIntegrityHealth, GovernanceOutboxHealth,
    GovernanceRepositoryError, GovernanceRevisionSummary, GovernanceRevisionWriteCommand,
    GovernanceSessionRecord, GovernanceSessionRevokeCommand, GovernanceSessionUpsertCommand,
    GovernanceSessionUpsertOutcome, GovernanceSnapshot, GovernanceSnapshotSource, GovernanceStatus,
    GovernanceWriteOutcome, IdempotencyRecordLookupRow, IdempotencyRecordLookupRowStatus,
    SiemExportBatch, SiemExportEvent, SiemOutboxDeliveryDecision, SiemOutboxRetryPolicy,
    denied_approval_audit_outbox,
    governance_support::{
        activation_etag, approval_artifact_kind, approval_kind_from_label, approval_kind_label,
        approval_state_from_label, approval_state_label, approval_transition_error,
        artifact_checksum, artifact_kind_for_approval, artifact_kind_label, channel_from_label,
        channel_label, classification_from_label, credential_scope_from_label,
        credential_scope_label, from_i64, require_control_plane_admin, revision_table, to_i64,
        validate_governance_session_revoke, validate_governance_session_upsert,
    },
    materialize_idempotency_record_lookup_row, plan_audit_outbox_write,
    plan_governance_revision_write, plan_siem_outbox_delivery, verify_governance_audit_integrity,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

const MAX_OUTBOX_BATCH: u16 = 256;

#[derive(Clone, PartialEq, Eq)]
pub struct SiemOutboxEvent {
    pub tenant_id: TenantId,
    pub event_id: AuditEventId,
    pub audit_event_id: AuditEventId,
    pub event_envelope: String,
    pub attempt_count: u8,
}

impl fmt::Debug for SiemOutboxEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SiemOutboxEvent")
            .field("tenant_id", &"<redacted>")
            .field("event_id", &"<redacted>")
            .field("audit_event_id", &"<redacted>")
            .field("event_envelope", &"<redacted>")
            .field("attempt_count", &self.attempt_count)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SiemOutboxWorkerReport {
    pub selected: u16,
    pub delivered: u16,
    pub retried: u16,
    pub dead_lettered: u16,
}

pub struct GovernanceSqliteRepository {
    connection: Mutex<Connection>,
}

impl fmt::Debug for GovernanceSqliteRepository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceSqliteRepository")
            .field("connection", &"<redacted>")
            .finish()
    }
}

impl GovernanceSqliteRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, GovernanceRepositoryError> {
        let connection = Connection::open(path).map_err(database_error)?;
        Self::from_connection(connection)
    }

    pub fn from_connection(connection: Connection) -> Result<Self, GovernanceRepositoryError> {
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(database_error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(database_error)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn write_revision(
        &self,
        command: GovernanceRevisionWriteCommand,
        audit_outbox: AuditOutboxWriteCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        plan_governance_revision_write(command.clone())
            .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
        validate_revision_id(command.kind, &command.revision_id)?;
        let checksum = artifact_checksum(&command.compiled_artifact);
        if command.fingerprint.as_str() != checksum {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let created_at = to_i64(command.created_at_unix_ms)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;

        if let Some(existing) = load_revision_row(
            &transaction,
            command.tenant_id,
            command.kind,
            &command.revision_id,
        )? {
            if existing.checksum == checksum
                && existing.compiled_artifact == command.compiled_artifact
                && existing.created_by == command.created_by
                && existing.created_at_unix_ms == command.created_at_unix_ms
            {
                transaction.commit().map_err(database_error)?;
                return Ok(GovernanceWriteOutcome::Replayed);
            }
            return Err(GovernanceRepositoryError::Conflict);
        }

        insert_revision_metadata(&transaction, &command, &checksum, created_at)?;
        transaction
            .execute(
                "INSERT INTO prodex_governance_revision_artifacts (
                    tenant_id, artifact_kind, revision_id, artifact_checksum,
                    compiled_artifact, created_by, created_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    command.tenant_id.to_string(),
                    artifact_kind_label(command.kind),
                    command.revision_id,
                    checksum,
                    command.compiled_artifact,
                    command.created_by.to_string(),
                    created_at,
                ],
            )
            .map_err(database_error)?;
        append_audit_outbox_tx(&transaction, audit_outbox)?;
        transaction.commit().map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }

    pub fn create_approval(
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
        let kind = approval_artifact_kind(approval.kind)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        let revision_id = kind
            .map(|kind| {
                revision_id_for_fingerprint(
                    &transaction,
                    approval.tenant_id,
                    kind,
                    approval.fingerprint.as_str(),
                )?
                .ok_or(GovernanceRepositoryError::NotFound)
            })
            .transpose()?;

        if let Some(existing) = load_approval_tx(&transaction, approval.tenant_id, &approval.id)? {
            if existing == approval {
                transaction.commit().map_err(database_error)?;
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
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL, ?10)",
                params![
                    approval.tenant_id.to_string(),
                    approval.id.as_str(),
                    approval_kind_label(approval.kind),
                    approval.scope.as_str(),
                    approval.fingerprint.as_str(),
                    approval.maker.to_string(),
                    approval_state_label(approval.state),
                    i64::from(approval.required_quorum),
                    to_i64(approval.expires_at_unix_ms)?,
                    to_i64(approval.version)?,
                ],
            )
            .map_err(database_error)?;
        if let (Some(kind), Some(revision_id)) = (kind, revision_id.as_deref()) {
            update_revision_state(
                &transaction,
                approval.tenant_id,
                kind,
                revision_id,
                "pending_approval",
            )?;
        }
        append_audit_outbox_tx(&transaction, audit_outbox)?;
        transaction.commit().map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }

    pub fn vote_approval(
        &self,
        request: ApprovalVoteRequest,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        self.transition_approval(request, ApprovalAction::Approve)
    }

    pub fn transition_approval(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        match self.transition_approval_inner(request, action, None)? {
            ApprovalVoteMutationOutcome::Applied(approval) => Ok(approval),
            ApprovalVoteMutationOutcome::Replayed(_) => {
                Err(GovernanceRepositoryError::InvalidInput)
            }
        }
    }

    pub fn transition_approval_idempotent(
        &self,
        request: ApprovalVoteRequest,
        action: ApprovalAction,
        idempotency: ApprovalVoteIdempotency,
    ) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        self.transition_approval_inner(request, action, Some(idempotency))
    }

    fn transition_approval_inner(
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
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        if let Some(idempotency) = idempotency.as_ref() {
            match approval_idempotency_replay_sqlite(&transaction, request.tenant_id, idempotency)?
            {
                IdempotencyReplayDecision::ExecuteAndRecordPending => {
                    insert_approval_idempotency_pending_sqlite(
                        &transaction,
                        request.tenant_id,
                        idempotency,
                    )?;
                }
                IdempotencyReplayDecision::AlreadyInProgress { .. } => {
                    return Err(GovernanceRepositoryError::Conflict);
                }
                IdempotencyReplayDecision::Replay(response) => {
                    transaction.commit().map_err(database_error)?;
                    return ApprovalVoteStableOutcome::replay(&response);
                }
            }
        }
        let current = load_approval_tx(&transaction, request.tenant_id, &request.approval_id)?
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
                )?;
                if let Some(idempotency) = idempotency.as_ref() {
                    complete_approval_idempotency_sqlite(
                        &transaction,
                        request.tenant_id,
                        idempotency,
                        ApprovalVoteStableOutcome::Denied(denial),
                        request.now_unix_ms,
                    )?;
                }
                transaction.commit().map_err(database_error)?;
                return Err(denial.repository_error());
            }
        };
        if !transition.changed {
            if let Some(idempotency) = idempotency.as_ref() {
                complete_approval_idempotency_sqlite(
                    &transaction,
                    request.tenant_id,
                    idempotency,
                    ApprovalVoteStableOutcome::Success(ApprovalVoteSnapshot::from_record(
                        &transition.record,
                    )),
                    request.now_unix_ms,
                )?;
            }
            transaction.commit().map_err(database_error)?;
            return Ok(ApprovalVoteMutationOutcome::Applied(transition.record));
        }
        persist_approval_transition(&transaction, &current, &transition.record)?;
        if transition.record.kind != ApprovalKind::Execution
            && transition.record.state == ApprovalState::Approved
        {
            let kind = artifact_kind_for_approval(transition.record.kind)?;
            let revision_id = revision_id_for_fingerprint(
                &transaction,
                transition.record.tenant_id,
                kind,
                transition.record.fingerprint.as_str(),
            )?
            .ok_or(GovernanceRepositoryError::NotFound)?;
            update_revision_state(
                &transaction,
                transition.record.tenant_id,
                kind,
                &revision_id,
                "approved",
            )?;
        } else if transition.record.kind != ApprovalKind::Execution
            && transition.record.state == ApprovalState::Rejected
        {
            let kind = artifact_kind_for_approval(transition.record.kind)?;
            let revision_id = revision_id_for_fingerprint(
                &transaction,
                transition.record.tenant_id,
                kind,
                transition.record.fingerprint.as_str(),
            )?
            .ok_or(GovernanceRepositoryError::NotFound)?;
            update_revision_state(
                &transaction,
                transition.record.tenant_id,
                kind,
                &revision_id,
                "rejected",
            )?;
        }
        append_audit_outbox_tx(&transaction, request.audit_outbox)?;
        if let Some(idempotency) = idempotency.as_ref() {
            complete_approval_idempotency_sqlite(
                &transaction,
                request.tenant_id,
                idempotency,
                ApprovalVoteStableOutcome::Success(ApprovalVoteSnapshot::from_record(
                    &transition.record,
                )),
                request.now_unix_ms,
            )?;
        }
        transaction.commit().map_err(database_error)?;
        Ok(ApprovalVoteMutationOutcome::Applied(transition.record))
    }

    pub fn list_revisions(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<Vec<GovernanceRevisionSummary>, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let table = revision_table(kind);
        let mut statement = connection
            .prepare(&format!(
                "SELECT revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
                 FROM {table} WHERE tenant_id = ?1
                 ORDER BY created_at_unix_ms DESC, revision_id DESC"
            ))
            .map_err(database_error)?;
        let rows = statement
            .query_map([tenant_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(database_error)?;
        let mut summaries = Vec::new();
        for row in rows {
            let (revision_id, fingerprint, lifecycle_state, created_at) =
                row.map_err(database_error)?;
            summaries.push(GovernanceRevisionSummary {
                revision_id,
                fingerprint,
                lifecycle_state,
                created_at_unix_ms: from_i64(created_at)?,
            });
        }
        Ok(summaries)
    }

    pub fn get_revision(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        revision_id: &str,
    ) -> Result<GovernanceRevisionSummary, GovernanceRepositoryError> {
        validate_revision_id(kind, revision_id)?;
        self.list_revisions(tenant_id, kind)?
            .into_iter()
            .find(|revision| revision.revision_id == revision_id)
            .ok_or(GovernanceRepositoryError::NotFound)
    }

    pub fn get_approval(
        &self,
        tenant_id: TenantId,
        approval_id: &ApprovalId,
    ) -> Result<ApprovalRecord, GovernanceRepositoryError> {
        let connection = self.connection()?;
        load_approval_tx(&connection, tenant_id, approval_id)?
            .ok_or(GovernanceRepositoryError::NotFound)
    }

    pub fn list_execution_approvals(
        &self,
        tenant_id: TenantId,
    ) -> Result<Vec<ApprovalRecord>, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT approval_id FROM prodex_approvals
                 WHERE tenant_id = ?1 AND approval_kind = 'execution'
                 ORDER BY expires_at_unix_ms DESC, approval_id DESC",
            )
            .map_err(database_error)?;
        let ids = statement
            .query_map([tenant_id.to_string()], |row| row.get::<_, String>(0))
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        ids.into_iter()
            .map(|id| {
                let id = ApprovalId::new(id).map_err(|_| GovernanceRepositoryError::Database)?;
                load_approval_tx(&connection, tenant_id, &id)?
                    .ok_or(GovernanceRepositoryError::Database)
            })
            .collect()
    }

    pub fn status(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
    ) -> Result<GovernanceStatus, GovernanceRepositoryError> {
        let connection = self.connection()?;
        Ok(load_pointer(&connection, tenant_id, kind)?.map_or_else(
            GovernanceStatus::default,
            |pointer| GovernanceStatus {
                active_revision_id: pointer.active_revision_id,
                last_known_good_revision_id: pointer.last_known_good_revision_id,
                etag: Some(pointer.etag),
            },
        ))
    }

    pub fn latest_audit_digest(
        &self,
        tenant_id: TenantId,
    ) -> Result<Option<AuditDigest>, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let value = connection
            .query_row(
                "SELECT audit.event_digest
                 FROM prodex_audit_log audit
                 WHERE audit.tenant_id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM prodex_audit_log child
                       WHERE child.tenant_id = audit.tenant_id
                         AND child.previous_digest = audit.event_digest
                   )
                 ORDER BY audit.occurred_at_unix_ms DESC, audit.audit_event_id DESC
                 LIMIT 1",
                [tenant_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(database_error)?;
        value
            .map(|digest| AuditDigest::new(digest).map_err(|_| GovernanceRepositoryError::Database))
            .transpose()
    }

    pub fn outbox_health(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceOutboxHealth, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let (pending, oldest) = connection
            .query_row(
                "SELECT COUNT(*), MIN(created_at_unix_ms) FROM prodex_siem_outbox
                 WHERE tenant_id = ?1 AND delivered_at_unix_ms IS NULL",
                [tenant_id.to_string()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .map_err(database_error)?;
        let dead_lettered = connection
            .query_row(
                "SELECT COUNT(*) FROM prodex_siem_dead_letters WHERE tenant_id = ?1",
                [tenant_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(database_error)?;
        Ok(GovernanceOutboxHealth {
            pending: from_i64(pending)?,
            dead_lettered: from_i64(dead_lettered)?,
            oldest_pending_at_unix_ms: oldest.map(from_i64).transpose()?,
        })
    }

    pub fn aggregate_outbox_health(
        &self,
    ) -> Result<GovernanceOutboxHealth, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let (pending, oldest) = connection
            .query_row(
                "SELECT COUNT(*), MIN(created_at_unix_ms) FROM prodex_siem_outbox
                 WHERE delivered_at_unix_ms IS NULL",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .map_err(database_error)?;
        let dead_lettered = connection
            .query_row("SELECT COUNT(*) FROM prodex_siem_dead_letters", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(database_error)?;
        Ok(GovernanceOutboxHealth {
            pending: from_i64(pending)?,
            dead_lettered: from_i64(dead_lettered)?,
            oldest_pending_at_unix_ms: oldest.map(from_i64).transpose()?,
        })
    }

    pub fn audit_integrity_health(
        &self,
        tenant_id: TenantId,
    ) -> Result<GovernanceAuditIntegrityHealth, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT audit_event_id, occurred_at_unix_ms, principal_id, action,
                        resource_kind, resource_id, outcome, reason_code,
                        previous_digest, event_digest
                 FROM prodex_audit_log WHERE tenant_id = ?1",
            )
            .map_err(database_error)?;
        let records = statement
            .query_map([tenant_id.to_string()], |row| {
                Ok(GovernanceAuditExportRecord {
                    audit_event_id: row.get(0)?,
                    occurred_at_unix_ms: from_i64(row.get(1)?)
                        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(1, i64::MAX))?,
                    principal_id: row.get(2)?,
                    action: row.get(3)?,
                    resource_kind: row.get(4)?,
                    resource_id: row.get(5)?,
                    outcome: row.get(6)?,
                    reason_code: row.get(7)?,
                    previous_digest: row.get(8)?,
                    event_digest: row.get(9)?,
                })
            })
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        Ok(verify_governance_audit_integrity(tenant_id, &records))
    }

    pub fn governance_export_audit(
        &self,
        tenant_id: TenantId,
        limit: u16,
    ) -> Result<Vec<GovernanceAuditExportRecord>, GovernanceRepositoryError> {
        if limit == 0 || limit > 1_000 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT audit_event_id, occurred_at_unix_ms, principal_id, action,
                        resource_kind, resource_id, outcome, reason_code,
                        previous_digest, event_digest
                 FROM prodex_audit_log WHERE tenant_id = ?1
                 ORDER BY occurred_at_unix_ms DESC, audit_event_id DESC LIMIT ?2",
            )
            .map_err(database_error)?;
        statement
            .query_map(params![tenant_id.to_string(), i64::from(limit)], |row| {
                Ok(GovernanceAuditExportRecord {
                    audit_event_id: row.get(0)?,
                    occurred_at_unix_ms: from_i64(row.get(1)?)
                        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(1, i64::MAX))?,
                    principal_id: row.get(2)?,
                    action: row.get(3)?,
                    resource_kind: row.get(4)?,
                    resource_id: row.get(5)?,
                    outcome: row.get(6)?,
                    reason_code: row.get(7)?,
                    previous_digest: row.get(8)?,
                    event_digest: row.get(9)?,
                })
            })
            .map_err(database_error)?
            .map(|row| row.map_err(database_error))
            .collect()
    }

    pub fn activate_revision(
        &self,
        request: GovernanceActivationRequest,
        validate_artifact: impl FnOnce(&[u8]) -> bool,
    ) -> Result<GovernanceActivationResult, GovernanceRepositoryError> {
        require_control_plane_admin(request.tenant_id, &request.actor)?;
        validate_revision_id(request.kind, &request.revision_id)?;
        if request.request_fingerprint.is_empty() || request.request_fingerprint.len() > 256 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let activated_at = to_i64(request.activated_at_unix_ms)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;

        if let Some(replay) = load_activation_replay(&transaction, &request)? {
            transaction.commit().map_err(database_error)?;
            return Ok(replay);
        }
        let revision = load_revision_row(
            &transaction,
            request.tenant_id,
            request.kind,
            &request.revision_id,
        )?
        .ok_or(GovernanceRepositoryError::NotFound)?;
        if revision.checksum != artifact_checksum(&revision.compiled_artifact)
            || !validate_artifact(&revision.compiled_artifact)
        {
            return Err(GovernanceRepositoryError::SnapshotUnavailable);
        }
        let approval = load_approval_tx(&transaction, request.tenant_id, &request.approval_id)?
            .ok_or(GovernanceRepositoryError::ApprovalRequired)?;
        if approval.state != ApprovalState::Approved
            || approval.fingerprint.as_str() != revision.checksum
            || artifact_kind_for_approval(approval.kind)? != request.kind
        {
            return Err(GovernanceRepositoryError::ApprovalRequired);
        }
        let pointer = load_pointer(&transaction, request.tenant_id, request.kind)?;
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

        if let Some(previous) = previous_active.as_deref()
            && previous != request.revision_id
        {
            let previous_state = match request.action {
                GovernanceActivationAction::Activate => "superseded",
                GovernanceActivationAction::Rollback => "rolled_back",
            };
            update_revision_state(
                &transaction,
                request.tenant_id,
                request.kind,
                previous,
                previous_state,
            )?;
        }
        update_revision_state(
            &transaction,
            request.tenant_id,
            request.kind,
            &request.revision_id,
            "active",
        )?;
        persist_approval_transition(&transaction, &approval, &activated_approval.record)?;
        store_pointer(
            &transaction,
            request.tenant_id,
            request.kind,
            &request.revision_id,
            &last_known_good,
            &etag,
            activated_at,
            pointer.as_ref(),
        )?;
        insert_activation_history(
            &transaction,
            &request,
            previous_active.as_deref(),
            activated_at,
        )?;
        append_audit_outbox_tx(&transaction, request.audit_outbox.clone())?;
        transaction
            .execute(
                "INSERT INTO prodex_governance_mutation_idempotency (
                    tenant_id, artifact_kind, idempotency_key, request_fingerprint,
                    action, revision_id, resulting_etag, created_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    request.tenant_id.to_string(),
                    artifact_kind_label(request.kind),
                    request.idempotency_key.as_str(),
                    request.request_fingerprint,
                    request.action.as_str(),
                    request.revision_id,
                    etag,
                    activated_at,
                ],
            )
            .map_err(database_error)?;
        transaction.commit().map_err(database_error)?;
        Ok(GovernanceActivationResult {
            outcome: GovernanceWriteOutcome::Applied,
            kind: request.kind,
            revision_id: request.revision_id,
            etag,
            last_known_good_revision_id: last_known_good,
        })
    }

    pub fn load_snapshot(
        &self,
        tenant_id: TenantId,
        kind: GovernanceArtifactKind,
        mut validate_artifact: impl FnMut(&[u8]) -> bool,
    ) -> Result<GovernanceSnapshot, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let pointer = load_pointer(&connection, tenant_id, kind)?
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        let active = pointer
            .active_revision_id
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        if let Some(snapshot) = load_verified_snapshot(
            &connection,
            tenant_id,
            kind,
            &active,
            GovernanceSnapshotSource::Active,
            &mut validate_artifact,
        )? {
            return Ok(snapshot);
        }
        let last_known_good = pointer
            .last_known_good_revision_id
            .ok_or(GovernanceRepositoryError::SnapshotUnavailable)?;
        if last_known_good == active {
            return Err(GovernanceRepositoryError::SnapshotUnavailable);
        }
        load_verified_snapshot(
            &connection,
            tenant_id,
            kind,
            &last_known_good,
            GovernanceSnapshotSource::LastKnownGood,
            &mut validate_artifact,
        )?
        .ok_or(GovernanceRepositoryError::SnapshotUnavailable)
    }

    pub fn append_audit_outbox(
        &self,
        command: AuditOutboxWriteCommand,
    ) -> Result<(), GovernanceRepositoryError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        append_audit_outbox_tx(&transaction, command)?;
        transaction.commit().map_err(database_error)
    }

    pub fn governance_upsert_session(
        &self,
        command: GovernanceSessionUpsertCommand,
    ) -> Result<GovernanceSessionUpsertOutcome, GovernanceRepositoryError> {
        validate_governance_session_upsert(&command)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        let existing =
            load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)?;
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
            )?;
            if active >= u64::from(max) {
                transaction.commit().map_err(database_error)?;
                return Ok(GovernanceSessionUpsertOutcome::ConcurrentLimitReached);
            }
        }
        transaction
            .execute(
                "INSERT INTO prodex_governance_sessions (
                    tenant_id, session_id_hash, principal_id, channel, credential_scope,
                    classification, policy_revision_id, provider_registry_revision,
                    provider_descriptor_revision, provider_affinity,
                    created_at_unix_ms, last_seen_at_unix_ms, absolute_expires_at_unix_ms,
                    idle_expires_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT (tenant_id, session_id_hash) DO UPDATE SET
                    classification = CASE
                        WHEN prodex_governance_sessions.classification = 'restricted'
                          OR excluded.classification = 'restricted' THEN 'restricted'
                        WHEN prodex_governance_sessions.classification = 'confidential'
                          OR excluded.classification = 'confidential' THEN 'confidential'
                        WHEN prodex_governance_sessions.classification = 'internal'
                          OR excluded.classification = 'internal' THEN 'internal'
                        ELSE 'public' END,
                    policy_revision_id = CASE
                        WHEN excluded.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN excluded.policy_revision_id ELSE prodex_governance_sessions.policy_revision_id END,
                    provider_registry_revision = CASE
                        WHEN excluded.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN excluded.provider_registry_revision
                        ELSE prodex_governance_sessions.provider_registry_revision END,
                    provider_descriptor_revision = CASE
                        WHEN excluded.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN excluded.provider_descriptor_revision
                        ELSE prodex_governance_sessions.provider_descriptor_revision END,
                    provider_affinity = CASE
                        WHEN excluded.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN excluded.provider_affinity ELSE prodex_governance_sessions.provider_affinity END,
                    last_seen_at_unix_ms = MAX(
                        prodex_governance_sessions.last_seen_at_unix_ms,
                        excluded.last_seen_at_unix_ms
                    ),
                    absolute_expires_at_unix_ms = MIN(
                        prodex_governance_sessions.absolute_expires_at_unix_ms,
                        excluded.absolute_expires_at_unix_ms
                    ),
                    idle_expires_at_unix_ms = CASE
                        WHEN excluded.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN excluded.idle_expires_at_unix_ms
                        ELSE prodex_governance_sessions.idle_expires_at_unix_ms END",
                params![
                    command.tenant_id.to_string(),
                    command.session_id_hash,
                    command.principal_id.to_string(),
                    channel_label(command.channel),
                    credential_scope_label(command.credential_scope),
                    command.classification.as_str(),
                    command.policy_revision_id.to_string(),
                    command.provider_registry_revision,
                    to_i64(command.provider_descriptor_revision)?,
                    command.provider_affinity,
                    to_i64(command.created_at_unix_ms)?,
                    to_i64(command.last_seen_at_unix_ms)?,
                    to_i64(command.absolute_expires_at_unix_ms)?,
                    to_i64(command.idle_expires_at_unix_ms)?,
                ],
            )
            .map_err(database_error)?;
        let stored =
            load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)?
                .ok_or(GovernanceRepositoryError::Database)?;
        transaction.commit().map_err(database_error)?;
        Ok(GovernanceSessionUpsertOutcome::Stored(Box::new(stored)))
    }

    pub fn governance_load_sessions(
        &self,
        tenant_id: TenantId,
        now_unix_ms: u64,
        limit: u16,
    ) -> Result<Vec<GovernanceSessionRecord>, GovernanceRepositoryError> {
        if limit == 0 || limit > 4_096 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
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
                 WHERE session.tenant_id = ?1
                   AND (revocation.session_id_hash IS NOT NULL
                        OR (session.absolute_expires_at_unix_ms > ?2
                            AND session.idle_expires_at_unix_ms > ?2))
                 ORDER BY session.last_seen_at_unix_ms DESC, session.session_id_hash
                 LIMIT ?3",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map(
                params![
                    tenant_id.to_string(),
                    to_i64(now_unix_ms)?,
                    i64::from(limit)
                ],
                governance_session_row,
            )
            .map_err(database_error)?;
        rows.map(|row| {
            row.map_err(database_error)
                .and_then(|row| governance_session_from_values(tenant_id, row))
        })
        .collect()
    }

    pub fn governance_count_concurrent_sessions(
        &self,
        tenant_id: TenantId,
        principal_id: PrincipalId,
        now_unix_ms: u64,
    ) -> Result<u64, GovernanceRepositoryError> {
        let connection = self.connection()?;
        count_concurrent_sessions_tx(&connection, tenant_id, principal_id, now_unix_ms)
    }

    pub fn governance_session_revocation_epoch(
        &self,
        tenant_id: TenantId,
    ) -> Result<u64, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let epoch = connection
            .query_row(
                "SELECT session_revocation_epoch FROM prodex_tenants WHERE tenant_id = ?1",
                params![tenant_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(database_error)?
            .ok_or(GovernanceRepositoryError::NotFound)?;
        from_i64(epoch)
    }

    pub fn governance_revoke_session(
        &self,
        command: GovernanceSessionRevokeCommand,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        validate_governance_session_revoke(&command)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        if load_governance_session_tx(&transaction, command.tenant_id, &command.session_id_hash)?
            .is_none()
        {
            return Err(GovernanceRepositoryError::NotFound);
        }
        let existing = transaction
            .query_row(
                "SELECT revoked_at_unix_ms, reason_code FROM prodex_session_revocations
                 WHERE tenant_id = ?1 AND session_id_hash = ?2",
                params![command.tenant_id.to_string(), command.session_id_hash],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(database_error)?;
        if let Some((_revoked_at, reason)) = existing {
            if reason != command.reason_code {
                return Err(GovernanceRepositoryError::Conflict);
            }
            transaction.commit().map_err(database_error)?;
            return Ok(GovernanceWriteOutcome::Replayed);
        }
        transaction
            .execute(
                "INSERT INTO prodex_session_revocations (
                    tenant_id, session_id_hash, revoked_at_unix_ms, reason_code
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    command.tenant_id.to_string(),
                    &command.session_id_hash,
                    to_i64(command.revoked_at_unix_ms)?,
                    &command.reason_code,
                ],
            )
            .map_err(database_error)?;
        let updated = transaction
            .execute(
                "UPDATE prodex_tenants
                 SET session_revocation_epoch = session_revocation_epoch + 1
                 WHERE tenant_id = ?1",
                params![command.tenant_id.to_string()],
            )
            .map_err(database_error)?;
        if updated != 1 {
            return Err(GovernanceRepositoryError::NotFound);
        }
        append_audit_outbox_tx(&transaction, command.audit_outbox)?;
        transaction.commit().map_err(database_error)?;
        Ok(GovernanceWriteOutcome::Applied)
    }

    pub fn run_siem_outbox_batch<E>(
        &self,
        now_unix_ms: u64,
        batch_limit: u16,
        retry_policy: SiemOutboxRetryPolicy,
        mut deliver: impl FnMut(&SiemOutboxEvent) -> Result<(), E>,
    ) -> Result<SiemOutboxWorkerReport, GovernanceRepositoryError> {
        if batch_limit == 0 || batch_limit > MAX_OUTBOX_BATCH {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let events = self.load_due_outbox(now_unix_ms, batch_limit)?;
        let mut report = SiemOutboxWorkerReport {
            selected: u16::try_from(events.len()).unwrap_or(batch_limit),
            ..SiemOutboxWorkerReport::default()
        };
        for event in events {
            let delivered = deliver(&event).is_ok();
            let decision = plan_siem_outbox_delivery(
                retry_policy,
                event.attempt_count,
                delivered,
                now_unix_ms,
            );
            let mut connection = self.connection()?;
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(database_error)?;
            match decision {
                SiemOutboxDeliveryDecision::Delivered => {
                    transaction
                        .execute(
                            "UPDATE prodex_siem_outbox
                             SET attempt_count = attempt_count + 1, delivered_at_unix_ms = ?3
                             WHERE tenant_id = ?1 AND event_id = ?2 AND delivered_at_unix_ms IS NULL",
                            params![event.tenant_id.to_string(), event.event_id.to_string(), to_i64(now_unix_ms)?],
                        )
                        .map_err(database_error)?;
                    report.delivered = report.delivered.saturating_add(1);
                }
                SiemOutboxDeliveryDecision::RetryAt(next_attempt_at) => {
                    transaction
                        .execute(
                            "UPDATE prodex_siem_outbox
                             SET attempt_count = attempt_count + 1, next_attempt_at_unix_ms = ?3
                             WHERE tenant_id = ?1 AND event_id = ?2 AND delivered_at_unix_ms IS NULL",
                            params![event.tenant_id.to_string(), event.event_id.to_string(), to_i64(next_attempt_at)?],
                        )
                        .map_err(database_error)?;
                    report.retried = report.retried.saturating_add(1);
                }
                SiemOutboxDeliveryDecision::DeadLetter => {
                    transaction
                        .execute(
                            "INSERT OR IGNORE INTO prodex_siem_dead_letters (
                                tenant_id, event_id, audit_event_id, event_envelope,
                                attempt_count, stable_reason_code, failed_at_unix_ms
                             ) VALUES (?1, ?2, ?3, ?4, ?5, 'delivery_failed', ?6)",
                            params![
                                event.tenant_id.to_string(),
                                event.event_id.to_string(),
                                event.audit_event_id.to_string(),
                                event.event_envelope,
                                i64::from(event.attempt_count.saturating_add(1)),
                                to_i64(now_unix_ms)?,
                            ],
                        )
                        .map_err(database_error)?;
                    transaction
                        .execute(
                            "DELETE FROM prodex_siem_outbox WHERE tenant_id = ?1 AND event_id = ?2",
                            params![event.tenant_id.to_string(), event.event_id.to_string()],
                        )
                        .map_err(database_error)?;
                    report.dead_lettered = report.dead_lettered.saturating_add(1);
                }
            }
            transaction.commit().map_err(database_error)?;
        }
        Ok(report)
    }

    pub fn run_siem_outbox_exporter_batch<E: GovernanceAuditExporter>(
        &self,
        now_unix_ms: u64,
        batch_limit: u16,
        retry_policy: SiemOutboxRetryPolicy,
        exporter: &mut E,
    ) -> Result<SiemOutboxWorkerReport, GovernanceRepositoryError> {
        self.run_siem_outbox_batch(now_unix_ms, batch_limit, retry_policy, |event| {
            let batch = SiemExportBatch::bounded(
                vec![SiemExportEvent {
                    event_id: event.event_id,
                    event_envelope: event.event_envelope.clone(),
                }],
                exporter.capabilities(),
            )
            .map_err(|_| ())?;
            match exporter.export_batch(&batch) {
                Ok(receipt) if receipt.accepted_events == 1 => Ok(()),
                Ok(_) | Err(_) => Err(()),
            }
        })
    }

    fn load_due_outbox(
        &self,
        now_unix_ms: u64,
        batch_limit: u16,
    ) -> Result<Vec<SiemOutboxEvent>, GovernanceRepositoryError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT tenant_id, event_id, audit_event_id, event_envelope, attempt_count
                 FROM prodex_siem_outbox
                 WHERE delivered_at_unix_ms IS NULL AND next_attempt_at_unix_ms <= ?1
                 ORDER BY next_attempt_at_unix_ms, event_id
                 LIMIT ?2",
            )
            .map_err(database_error)?;
        let rows = statement
            .query_map(
                params![to_i64(now_unix_ms)?, i64::from(batch_limit)],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .map_err(database_error)?;
        let mut events = Vec::with_capacity(usize::from(batch_limit));
        for row in rows {
            let (tenant_id, event_id, audit_event_id, event_envelope, attempt_count) =
                row.map_err(database_error)?;
            events.push(SiemOutboxEvent {
                tenant_id: TenantId::from_str(&tenant_id)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
                event_id: AuditEventId::from_str(&event_id)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
                audit_event_id: AuditEventId::from_str(&audit_event_id)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
                event_envelope,
                attempt_count: u8::try_from(attempt_count)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
            });
        }
        Ok(events)
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, GovernanceRepositoryError> {
        self.connection
            .lock()
            .map_err(|_| GovernanceRepositoryError::Database)
    }
}

fn database_error(_: rusqlite::Error) -> GovernanceRepositoryError {
    GovernanceRepositoryError::Database
}
