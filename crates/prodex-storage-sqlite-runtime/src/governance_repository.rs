//! Executing SQLite adapter for authoritative governance lifecycle state.

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use prodex_domain::{
    ApprovalAction, ApprovalError, ApprovalFingerprint, ApprovalId, ApprovalKind, ApprovalRecord,
    ApprovalScope, ApprovalState, ApprovalTransitionRequest, ApprovalVote, AuditDigest,
    AuditEventId, Channel, CredentialScope, DataClassification, PolicyRevisionId, Principal,
    PrincipalId, Role, TenantId, transition_approval,
};
use prodex_storage::{
    ApprovalVoteRequest, AuditOutboxWriteCommand, GovernanceActivationAction,
    GovernanceActivationRequest, GovernanceActivationResult, GovernanceArtifactKind,
    GovernanceAuditExportRecord, GovernanceAuditExporter, GovernanceAuditIntegrityHealth,
    GovernanceOutboxHealth, GovernanceRepositoryError, GovernanceRevisionSummary,
    GovernanceRevisionWriteCommand, GovernanceSessionRecord, GovernanceSessionRevokeCommand,
    GovernanceSessionUpsertCommand, GovernanceSessionUpsertOutcome, GovernanceSnapshot,
    GovernanceSnapshotSource, GovernanceStatus, GovernanceWriteOutcome, SiemExportBatch,
    SiemExportEvent, SiemOutboxDeliveryDecision, SiemOutboxRetryPolicy, plan_audit_outbox_write,
    plan_governance_revision_write, plan_siem_outbox_delivery,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sha2::{Digest, Sha256};

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
        let kind = artifact_kind_from_approval(approval.kind)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        let revision_id = revision_id_for_fingerprint(
            &transaction,
            approval.tenant_id,
            kind,
            approval.fingerprint.as_str(),
        )?
        .ok_or(GovernanceRepositoryError::NotFound)?;

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
                    activated_at_unix_ms, resource_version
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10)",
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
        update_revision_state(
            &transaction,
            approval.tenant_id,
            kind,
            &revision_id,
            "pending_approval",
        )?;
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
        if !matches!(action, ApprovalAction::Approve | ApprovalAction::Reject) {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        require_control_plane_admin(request.tenant_id, &request.actor)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        let current = load_approval_tx(&transaction, request.tenant_id, &request.approval_id)?
            .ok_or(GovernanceRepositoryError::NotFound)?;
        let transition = transition_approval(ApprovalTransitionRequest {
            record: &current,
            actor: &request.actor,
            action,
            expected_version: request.expected_version,
            now_unix_ms: request.now_unix_ms,
        })
        .map_err(approval_transition_error)?;
        if !transition.changed {
            transaction.commit().map_err(database_error)?;
            return Ok(transition.record);
        }
        persist_approval_transition(&transaction, &current, &transition.record)?;
        if transition.record.state == ApprovalState::Approved {
            let kind = artifact_kind_from_approval(transition.record.kind)?;
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
        } else if transition.record.state == ApprovalState::Rejected {
            let kind = artifact_kind_from_approval(transition.record.kind)?;
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
        transaction.commit().map_err(database_error)?;
        Ok(transition.record)
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
        let (events, heads, invalid_links, roots) = connection
            .query_row(
                "SELECT
                    COUNT(*),
                    SUM(CASE WHEN NOT EXISTS (
                        SELECT 1 FROM prodex_audit_log child
                        WHERE child.tenant_id = audit.tenant_id
                          AND child.previous_digest = audit.event_digest
                    ) THEN 1 ELSE 0 END),
                    SUM(CASE WHEN audit.previous_digest IS NOT NULL AND NOT EXISTS (
                        SELECT 1 FROM prodex_audit_log parent
                        WHERE parent.tenant_id = audit.tenant_id
                          AND parent.event_digest = audit.previous_digest
                    ) THEN 1 ELSE 0 END),
                    SUM(CASE WHEN audit.previous_digest IS NULL THEN 1 ELSE 0 END)
                 FROM prodex_audit_log audit WHERE audit.tenant_id = ?1",
                [tenant_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                        row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                        row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    ))
                },
            )
            .map_err(database_error)?;
        let event_count = from_i64(events)?;
        let chain_head_count = from_i64(heads)?;
        Ok(GovernanceAuditIntegrityHealth {
            event_count,
            chain_head_count,
            chain_valid: invalid_links == 0
                && chain_head_count <= 1
                && ((event_count == 0 && roots == 0) || (event_count > 0 && roots == 1)),
        })
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
            || artifact_kind_from_approval(approval.kind)? != request.kind
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
                    classification, policy_revision_id, registry_revision_id, provider_affinity,
                    created_at_unix_ms, last_seen_at_unix_ms, absolute_expires_at_unix_ms,
                    idle_expires_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
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
                    registry_revision_id = CASE
                        WHEN excluded.last_seen_at_unix_ms >= prodex_governance_sessions.last_seen_at_unix_ms
                        THEN excluded.registry_revision_id ELSE prodex_governance_sessions.registry_revision_id END,
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
                    command.registry_revision_id,
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
        Ok(GovernanceSessionUpsertOutcome::Stored(stored))
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
                        session.policy_revision_id, session.registry_revision_id,
                        session.provider_affinity, session.created_at_unix_ms,
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

struct RawGovernanceSessionRow {
    session_id_hash: String,
    principal_id: String,
    channel: String,
    credential_scope: String,
    classification: String,
    policy_revision_id: String,
    registry_revision_id: String,
    provider_affinity: Option<String>,
    created_at_unix_ms: i64,
    last_seen_at_unix_ms: i64,
    absolute_expires_at_unix_ms: i64,
    idle_expires_at_unix_ms: i64,
    revoked_at_unix_ms: Option<i64>,
    revocation_reason_code: Option<String>,
}

fn governance_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawGovernanceSessionRow> {
    Ok(RawGovernanceSessionRow {
        session_id_hash: row.get(0)?,
        principal_id: row.get(1)?,
        channel: row.get(2)?,
        credential_scope: row.get(3)?,
        classification: row.get(4)?,
        policy_revision_id: row.get(5)?,
        registry_revision_id: row.get(6)?,
        provider_affinity: row.get(7)?,
        created_at_unix_ms: row.get(8)?,
        last_seen_at_unix_ms: row.get(9)?,
        absolute_expires_at_unix_ms: row.get(10)?,
        idle_expires_at_unix_ms: row.get(11)?,
        revoked_at_unix_ms: row.get(12)?,
        revocation_reason_code: row.get(13)?,
    })
}

fn governance_session_from_values(
    tenant_id: TenantId,
    row: RawGovernanceSessionRow,
) -> Result<GovernanceSessionRecord, GovernanceRepositoryError> {
    Ok(GovernanceSessionRecord {
        tenant_id,
        session_id_hash: row.session_id_hash,
        principal_id: PrincipalId::from_str(&row.principal_id)
            .map_err(|_| GovernanceRepositoryError::Database)?,
        channel: channel_from_label(&row.channel)?,
        credential_scope: credential_scope_from_label(&row.credential_scope)?,
        classification: classification_from_label(&row.classification)?,
        policy_revision_id: PolicyRevisionId::from_str(&row.policy_revision_id)
            .map_err(|_| GovernanceRepositoryError::Database)?,
        registry_revision_id: row.registry_revision_id,
        provider_affinity: row.provider_affinity,
        created_at_unix_ms: from_i64(row.created_at_unix_ms)?,
        last_seen_at_unix_ms: from_i64(row.last_seen_at_unix_ms)?,
        absolute_expires_at_unix_ms: from_i64(row.absolute_expires_at_unix_ms)?,
        idle_expires_at_unix_ms: from_i64(row.idle_expires_at_unix_ms)?,
        revoked_at_unix_ms: row.revoked_at_unix_ms.map(from_i64).transpose()?,
        revocation_reason_code: row.revocation_reason_code,
    })
}

fn load_governance_session_tx(
    connection: &Connection,
    tenant_id: TenantId,
    session_id_hash: &str,
) -> Result<Option<GovernanceSessionRecord>, GovernanceRepositoryError> {
    connection
        .query_row(
            "SELECT session.session_id_hash, session.principal_id, session.channel,
                    session.credential_scope, session.classification,
                    session.policy_revision_id, session.registry_revision_id,
                    session.provider_affinity, session.created_at_unix_ms,
                    session.last_seen_at_unix_ms, session.absolute_expires_at_unix_ms,
                    session.idle_expires_at_unix_ms, revocation.revoked_at_unix_ms,
                    revocation.reason_code
             FROM prodex_governance_sessions session
             LEFT JOIN prodex_session_revocations revocation
               ON revocation.tenant_id = session.tenant_id
              AND revocation.session_id_hash = session.session_id_hash
             WHERE session.tenant_id = ?1 AND session.session_id_hash = ?2",
            params![tenant_id.to_string(), session_id_hash],
            governance_session_row,
        )
        .optional()
        .map_err(database_error)?
        .map(|row| governance_session_from_values(tenant_id, row))
        .transpose()
}

fn count_concurrent_sessions_tx(
    connection: &Connection,
    tenant_id: TenantId,
    principal_id: PrincipalId,
    now_unix_ms: u64,
) -> Result<u64, GovernanceRepositoryError> {
    let count = connection
        .query_row(
            "SELECT COUNT(*) FROM prodex_governance_sessions session
             WHERE session.tenant_id = ?1 AND session.principal_id = ?2
               AND session.absolute_expires_at_unix_ms > ?3
               AND session.idle_expires_at_unix_ms > ?3
               AND NOT EXISTS (
                   SELECT 1 FROM prodex_session_revocations revocation
                   WHERE revocation.tenant_id = session.tenant_id
                     AND revocation.session_id_hash = session.session_id_hash
               )",
            params![
                tenant_id.to_string(),
                principal_id.to_string(),
                to_i64(now_unix_ms)?
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(database_error)?;
    from_i64(count)
}

fn insert_revision_metadata(
    transaction: &Transaction<'_>,
    command: &GovernanceRevisionWriteCommand,
    checksum: &str,
    created_at: i64,
) -> Result<(), GovernanceRepositoryError> {
    let common = params![
        command.tenant_id.to_string(),
        command.revision_id,
        checksum,
        command.created_by.to_string(),
        created_at,
    ];
    match command.kind {
        GovernanceArtifactKind::Policy => transaction.execute(
            "INSERT INTO prodex_policy_revisions (
                tenant_id, revision_id, artifact_checksum, compiled_metadata,
                lifecycle_state, created_by, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'draft', ?4, ?5)",
            common,
        ),
        GovernanceArtifactKind::ClassificationRules => transaction.execute(
            "INSERT INTO prodex_classification_rule_revisions (
                tenant_id, revision_id, artifact_checksum, compiled_metadata,
                lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'draft', ?5)",
            common,
        ),
        GovernanceArtifactKind::ProviderRegistry => transaction.execute(
            "INSERT INTO prodex_provider_registry_revisions (
                tenant_id, revision_id, artifact_checksum, lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, 'draft', ?5)",
            common,
        ),
        GovernanceArtifactKind::RoutingScores => transaction.execute(
            "INSERT INTO prodex_routing_score_revisions (
                tenant_id, revision_id, artifact_checksum, fixed_point_weights,
                lifecycle_state, created_at_unix_ms
             ) VALUES (?1, ?2, ?3, '{}', 'draft', ?5)",
            common,
        ),
    }
    .map(|_| ())
    .map_err(database_error)
}

fn update_revision_state(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    state: &str,
) -> Result<(), GovernanceRepositoryError> {
    let table = revision_table(kind);
    let changed = transaction
        .execute(
            &format!(
                "UPDATE {table} SET lifecycle_state = ?3 WHERE tenant_id = ?1 AND revision_id = ?2"
            ),
            params![tenant_id.to_string(), revision_id, state],
        )
        .map_err(database_error)?;
    if changed != 1 {
        return Err(GovernanceRepositoryError::NotFound);
    }
    Ok(())
}

fn load_revision_row(
    connection: &Connection,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
) -> Result<Option<RevisionRow>, GovernanceRepositoryError> {
    let row = connection
        .query_row(
            "SELECT artifact_checksum, compiled_artifact, created_by, created_at_unix_ms
             FROM prodex_governance_revision_artifacts
             WHERE tenant_id = ?1 AND artifact_kind = ?2 AND revision_id = ?3",
            params![
                tenant_id.to_string(),
                artifact_kind_label(kind),
                revision_id
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(database_error)?;
    row.map(
        |(checksum, compiled_artifact, created_by, created_at_unix_ms)| {
            Ok(RevisionRow {
                checksum,
                compiled_artifact,
                created_by: PrincipalId::from_str(&created_by)
                    .map_err(|_| GovernanceRepositoryError::Database)?,
                created_at_unix_ms: from_i64(created_at_unix_ms)?,
            })
        },
    )
    .transpose()
}

fn revision_id_for_fingerprint(
    connection: &Connection,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    fingerprint: &str,
) -> Result<Option<String>, GovernanceRepositoryError> {
    connection
        .query_row(
            "SELECT revision_id FROM prodex_governance_revision_artifacts
             WHERE tenant_id = ?1 AND artifact_kind = ?2 AND artifact_checksum = ?3",
            params![
                tenant_id.to_string(),
                artifact_kind_label(kind),
                fingerprint
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(database_error)
}

fn persist_approval_transition(
    transaction: &Transaction<'_>,
    previous: &ApprovalRecord,
    next: &ApprovalRecord,
) -> Result<(), GovernanceRepositoryError> {
    let changed = transaction
        .execute(
            "UPDATE prodex_approvals
             SET lifecycle_state = ?4, activated_at_unix_ms = ?5, resource_version = ?6
             WHERE tenant_id = ?1 AND approval_id = ?2 AND resource_version = ?3",
            params![
                next.tenant_id.to_string(),
                next.id.as_str(),
                to_i64(previous.version)?,
                approval_state_label(next.state),
                next.activated_at_unix_ms.map(to_i64).transpose()?,
                to_i64(next.version)?,
            ],
        )
        .map_err(database_error)?;
    if changed != 1 {
        return Err(GovernanceRepositoryError::Conflict);
    }
    for vote in &next.votes {
        transaction
            .execute(
                "INSERT OR IGNORE INTO prodex_approval_votes (
                    tenant_id, approval_id, checker_id, approved_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    next.tenant_id.to_string(),
                    next.id.as_str(),
                    vote.checker.to_string(),
                    to_i64(vote.approved_at_unix_ms)?,
                ],
            )
            .map_err(database_error)?;
    }
    Ok(())
}

fn load_approval_tx(
    connection: &Connection,
    tenant_id: TenantId,
    approval_id: &ApprovalId,
) -> Result<Option<ApprovalRecord>, GovernanceRepositoryError> {
    let row = connection
        .query_row(
            "SELECT approval_kind, approval_scope, fingerprint, maker_id, lifecycle_state,
                    required_quorum, expires_at_unix_ms, activated_at_unix_ms, resource_version
             FROM prodex_approvals WHERE tenant_id = ?1 AND approval_id = ?2",
            params![tenant_id.to_string(), approval_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            },
        )
        .optional()
        .map_err(database_error)?;
    let Some((kind, scope, fingerprint, maker, state, quorum, expires, activated, version)) = row
    else {
        return Ok(None);
    };
    let mut statement = connection
        .prepare(
            "SELECT checker_id, approved_at_unix_ms FROM prodex_approval_votes
             WHERE tenant_id = ?1 AND approval_id = ?2 ORDER BY checker_id",
        )
        .map_err(database_error)?;
    let vote_rows = statement
        .query_map(
            params![tenant_id.to_string(), approval_id.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(database_error)?;
    let mut votes = Vec::new();
    for vote in vote_rows {
        let (checker, approved_at) = vote.map_err(database_error)?;
        votes.push(ApprovalVote {
            checker: PrincipalId::from_str(&checker)
                .map_err(|_| GovernanceRepositoryError::Database)?,
            approved_at_unix_ms: from_i64(approved_at)?,
        });
    }
    Ok(Some(ApprovalRecord {
        id: approval_id.clone(),
        tenant_id,
        kind: approval_kind_from_label(&kind)?,
        scope: ApprovalScope::new(scope).map_err(|_| GovernanceRepositoryError::Database)?,
        fingerprint: ApprovalFingerprint::new(fingerprint)
            .map_err(|_| GovernanceRepositoryError::Database)?,
        maker: PrincipalId::from_str(&maker).map_err(|_| GovernanceRepositoryError::Database)?,
        state: approval_state_from_label(&state)?,
        required_quorum: u8::try_from(quorum).map_err(|_| GovernanceRepositoryError::Database)?,
        votes,
        expires_at_unix_ms: from_i64(expires)?,
        activated_at_unix_ms: activated.map(from_i64).transpose()?,
        version: from_i64(version)?,
    }))
}

fn load_pointer(
    connection: &Connection,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
) -> Result<Option<GovernancePointer>, GovernanceRepositoryError> {
    let table = pointer_table(kind);
    connection
        .query_row(
            &format!(
                "SELECT active_revision_id, last_known_good_revision_id, etag
                 FROM {table} WHERE tenant_id = ?1"
            ),
            [tenant_id.to_string()],
            |row| {
                Ok(GovernancePointer {
                    active_revision_id: row.get(0)?,
                    last_known_good_revision_id: row.get(1)?,
                    etag: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(database_error)
}

#[allow(clippy::too_many_arguments)]
fn store_pointer(
    transaction: &Transaction<'_>,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    active_revision_id: &str,
    last_known_good_revision_id: &str,
    etag: &str,
    updated_at: i64,
    previous: Option<&GovernancePointer>,
) -> Result<(), GovernanceRepositoryError> {
    let table = pointer_table(kind);
    let changed = if let Some(previous) = previous {
        transaction
            .execute(
                &format!(
                    "UPDATE {table}
                     SET active_revision_id = ?2, last_known_good_revision_id = ?3,
                         etag = ?4, updated_at_unix_ms = ?5
                     WHERE tenant_id = ?1 AND etag = ?6"
                ),
                params![
                    tenant_id.to_string(),
                    active_revision_id,
                    last_known_good_revision_id,
                    etag,
                    updated_at,
                    previous.etag,
                ],
            )
            .map_err(database_error)?
    } else {
        transaction
            .execute(
                &format!(
                    "INSERT INTO {table} (
                        tenant_id, active_revision_id, last_known_good_revision_id,
                        etag, updated_at_unix_ms
                     ) VALUES (?1, ?2, ?3, ?4, ?5)"
                ),
                params![
                    tenant_id.to_string(),
                    active_revision_id,
                    last_known_good_revision_id,
                    etag,
                    updated_at,
                ],
            )
            .map_err(database_error)?
    };
    if changed != 1 {
        return Err(GovernanceRepositoryError::EtagMismatch);
    }
    Ok(())
}

fn insert_activation_history(
    transaction: &Transaction<'_>,
    request: &GovernanceActivationRequest,
    previous_revision_id: Option<&str>,
    occurred_at: i64,
) -> Result<(), GovernanceRepositoryError> {
    let event_id = request.audit_outbox.audit.event.id.to_string();
    if request.kind == GovernanceArtifactKind::Policy {
        transaction.execute(
            "INSERT INTO prodex_policy_activation_history (
                    tenant_id, activation_id, revision_id, previous_revision_id,
                    action, actor_id, occurred_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                request.tenant_id.to_string(),
                event_id,
                request.revision_id,
                previous_revision_id,
                request.action.as_str(),
                request.actor.id.to_string(),
                occurred_at,
            ],
        )
    } else {
        transaction.execute(
            "INSERT INTO prodex_governance_activation_history (
                    tenant_id, activation_id, artifact_kind, revision_id,
                    previous_revision_id, action, actor_id, idempotency_key,
                    occurred_at_unix_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                request.tenant_id.to_string(),
                event_id,
                artifact_kind_label(request.kind),
                request.revision_id,
                previous_revision_id,
                request.action.as_str(),
                request.actor.id.to_string(),
                request.idempotency_key.as_str(),
                occurred_at,
            ],
        )
    }
    .map(|_| ())
    .map_err(database_error)
}

fn load_activation_replay(
    connection: &Connection,
    request: &GovernanceActivationRequest,
) -> Result<Option<GovernanceActivationResult>, GovernanceRepositoryError> {
    let row = connection
        .query_row(
            "SELECT request_fingerprint, action, revision_id, resulting_etag
             FROM prodex_governance_mutation_idempotency
             WHERE tenant_id = ?1 AND artifact_kind = ?2 AND idempotency_key = ?3",
            params![
                request.tenant_id.to_string(),
                artifact_kind_label(request.kind),
                request.idempotency_key.as_str(),
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(database_error)?;
    let Some((fingerprint, action, revision_id, etag)) = row else {
        return Ok(None);
    };
    if fingerprint != request.request_fingerprint
        || action != request.action.as_str()
        || revision_id != request.revision_id
    {
        return Err(GovernanceRepositoryError::Conflict);
    }
    let pointer = load_pointer(connection, request.tenant_id, request.kind)?
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

fn load_verified_snapshot(
    connection: &Connection,
    tenant_id: TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    source: GovernanceSnapshotSource,
    validate_artifact: &mut impl FnMut(&[u8]) -> bool,
) -> Result<Option<GovernanceSnapshot>, GovernanceRepositoryError> {
    let Some(revision) = load_revision_row(connection, tenant_id, kind, revision_id)? else {
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

fn append_audit_outbox_tx(
    transaction: &Transaction<'_>,
    command: AuditOutboxWriteCommand,
) -> Result<(), GovernanceRepositoryError> {
    let plan =
        plan_audit_outbox_write(command).map_err(|_| GovernanceRepositoryError::TenantMismatch)?;
    let envelope = plan.audit.envelope;
    let tenant_id = envelope.event.tenant_id.to_string();
    let expected_previous = envelope
        .previous_digest
        .as_ref()
        .map(|value| value.as_str());
    let mut statement = transaction
        .prepare(
            "SELECT audit.event_digest
             FROM prodex_audit_log audit
             WHERE audit.tenant_id = ?1
               AND NOT EXISTS (
                   SELECT 1 FROM prodex_audit_log child
                   WHERE child.tenant_id = audit.tenant_id
                     AND child.previous_digest = audit.event_digest
               )
             ORDER BY audit.occurred_at_unix_ms DESC, audit.audit_event_id DESC
             LIMIT 2",
        )
        .map_err(database_error)?;
    let mut rows = statement.query([&tenant_id]).map_err(database_error)?;
    let current = rows
        .next()
        .map_err(database_error)?
        .map(|row| row.get::<_, String>(0))
        .transpose()
        .map_err(database_error)?;
    if rows.next().map_err(database_error)?.is_some() || current.as_deref() != expected_previous {
        return Err(GovernanceRepositoryError::AuditChainConflict);
    }
    drop(rows);
    drop(statement);
    let event_envelope =
        serde_json::to_string(&envelope).map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    transaction
        .execute(
            "INSERT INTO prodex_audit_log (
                tenant_id, audit_event_id, previous_digest, event_digest,
                occurred_at_unix_ms, principal_id, action, resource_kind,
                resource_id, outcome, reason_code
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                tenant_id,
                envelope.event.id.to_string(),
                envelope
                    .previous_digest
                    .as_ref()
                    .map(|value| value.as_str()),
                envelope.event_digest.as_str(),
                to_i64(envelope.event.occurred_at_unix_ms)?,
                envelope.event.principal_id.to_string(),
                envelope.event.action.as_str(),
                envelope.event.resource.kind,
                envelope.event.resource.id,
                envelope.event.outcome.as_str(),
                envelope.event.reason_code,
            ],
        )
        .map_err(database_error)?;
    transaction
        .execute(
            "INSERT INTO prodex_siem_outbox (
                tenant_id, event_id, audit_event_id, event_envelope,
                attempt_count, next_attempt_at_unix_ms, created_at_unix_ms,
                delivered_at_unix_ms
             ) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5, NULL)",
            params![
                envelope.event.tenant_id.to_string(),
                plan.outbox_event_id.to_string(),
                envelope.event.id.to_string(),
                event_envelope,
                to_i64(envelope.event.occurred_at_unix_ms)?,
            ],
        )
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

fn approval_transition_error(error: ApprovalError) -> GovernanceRepositoryError {
    match error {
        ApprovalError::SelfApprovalDenied => GovernanceRepositoryError::ApprovalSelfAction,
        ApprovalError::StaleVersion => GovernanceRepositoryError::StaleVersion,
        ApprovalError::InvalidTransition => GovernanceRepositoryError::InvalidTransition,
        ApprovalError::TenantMismatch => GovernanceRepositoryError::TenantMismatch,
        ApprovalError::InvalidToken
        | ApprovalError::InvalidQuorum
        | ApprovalError::InvalidExpiry => GovernanceRepositoryError::InvalidInput,
    }
}

fn artifact_kind_from_approval(
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
        || !bounded_session_token(&command.registry_revision_id)
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

fn revision_table(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "prodex_policy_revisions",
        GovernanceArtifactKind::ClassificationRules => "prodex_classification_rule_revisions",
        GovernanceArtifactKind::ProviderRegistry => "prodex_provider_registry_revisions",
        GovernanceArtifactKind::RoutingScores => "prodex_routing_score_revisions",
    }
}

fn pointer_table(kind: GovernanceArtifactKind) -> &'static str {
    match kind {
        GovernanceArtifactKind::Policy => "prodex_policy_pointers",
        GovernanceArtifactKind::ClassificationRules => "prodex_classification_rule_pointers",
        GovernanceArtifactKind::ProviderRegistry => "prodex_provider_registry_pointers",
        GovernanceArtifactKind::RoutingScores => "prodex_routing_score_pointers",
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

fn validate_revision_id(
    kind: GovernanceArtifactKind,
    revision_id: &str,
) -> Result<(), GovernanceRepositoryError> {
    if kind == GovernanceArtifactKind::Policy {
        prodex_domain::PolicyRevisionId::from_str(revision_id)
            .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
    }
    Ok(())
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

fn database_error(_: rusqlite::Error) -> GovernanceRepositoryError {
    GovernanceRepositoryError::Database
}
