//! Executing SQLite adapter for authoritative governance lifecycle state.

mod approvals;
mod audit_outbox;
mod audit_retention;
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
    ApprovalAction, ApprovalFingerprint, ApprovalId, ApprovalReasonCode, ApprovalRecord,
    ApprovalScope, ApprovalState, ApprovalVote, AuditDigest, AuditEventId, AuditReasonCode,
    AuditRetentionBatchLimit, AuditRetentionHold, AuditTimestamp, IdempotencyReplayDecision,
    PolicyRevisionId, PrincipalId, TenantId, decide_idempotency_replay,
};
use prodex_storage::{
    ApprovalVoteIdempotency, ApprovalVoteMutationOutcome, ApprovalVoteRequest,
    ApprovalVoteSnapshot, ApprovalVoteStableOutcome, ApprovalVoteTransitionDecision,
    AuditOutboxWriteCommand, GOVERNANCE_APPROVAL_CREATE_IDEMPOTENCY_RESPONSE,
    GOVERNANCE_AUDIT_LEGAL_HOLD_DELETE_APPLIED_IDEMPOTENCY_RESPONSE,
    GOVERNANCE_AUDIT_LEGAL_HOLD_DELETE_NOT_FOUND_IDEMPOTENCY_RESPONSE,
    GOVERNANCE_AUDIT_LEGAL_HOLD_UPSERT_IDEMPOTENCY_RESPONSE,
    GOVERNANCE_REVISION_WRITE_IDEMPOTENCY_RESPONSE, GovernanceActivationAction,
    GovernanceActivationCurrent, GovernanceActivationRequest, GovernanceActivationResult,
    GovernanceArtifactAuthenticity, GovernanceArtifactKind, GovernanceArtifactValidationInput,
    GovernanceAuditExportRecord, GovernanceAuditExporter, GovernanceAuditIntegrityHealth,
    GovernanceMutationIdempotency, GovernanceOutboxHealth, GovernanceRepositoryError,
    GovernanceRevisionSummary, GovernanceRevisionWriteCommand, GovernanceSessionRecord,
    GovernanceSessionRevokeCommand, GovernanceSessionUpsertCommand, GovernanceSessionUpsertOutcome,
    GovernanceSnapshot, GovernanceSnapshotSource, GovernanceStatus, GovernanceWriteOutcome,
    IdempotencyRecordLookupRow, IdempotencyRecordLookupRowStatus, SiemExportBatch, SiemExportEvent,
    SiemOutboxDeliveryDecision, SiemOutboxRetryPolicy,
    decode_governance_audit_retention_purge_idempotency_response, denied_approval_audit_outbox,
    encode_governance_audit_retention_purge_idempotency_response,
    governance_revocation_fallback_candidate,
    governance_support::{
        approval_artifact_kind, approval_kind_from_label, approval_kind_label,
        approval_state_from_label, approval_state_label, artifact_checksum, artifact_kind_label,
        channel_from_label, channel_label, classification_from_label, credential_scope_from_label,
        credential_scope_label, from_i64, revision_table, to_i64,
        validate_governance_activation_request, validate_governance_revision_id,
        validate_governance_session_revoke, validate_governance_session_upsert,
    },
    materialize_idempotency_record_lookup_row, plan_approval_revision_lifecycle_update,
    plan_approval_vote_transition, plan_audit_outbox_write, plan_governance_activation,
    plan_governance_revision_write, plan_siem_outbox_delivery,
    verify_governance_audit_integrity_with_retention_anchor,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

const MAX_OUTBOX_BATCH: u16 = 256;
const DEFAULT_OUTBOX_LEASE_MS: u64 = 60_000;
const MAX_OUTBOX_LEASE_MS: u64 = 300_000;

#[derive(Clone, PartialEq, Eq)]
pub struct SiemOutboxEvent {
    pub tenant_id: TenantId,
    pub event_id: AuditEventId,
    pub audit_event_id: AuditEventId,
    pub event_envelope: String,
    pub attempt_count: u8,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SqliteSiemOutboxClaim {
    pub tenant_id: TenantId,
    pub event_id: AuditEventId,
    pub audit_event_id: AuditEventId,
    pub event_envelope: String,
    pub attempt_count: u8,
    pub claim_token: String,
}

impl fmt::Debug for SqliteSiemOutboxClaim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteSiemOutboxClaim")
            .field("tenant_id", &"<redacted>")
            .field("event_id", &"<redacted>")
            .field("audit_event_id", &"<redacted>")
            .field("event_envelope", &"<redacted>")
            .field("attempt_count", &self.attempt_count)
            .field("claim_token", &"<redacted>")
            .finish()
    }
}

impl SqliteSiemOutboxClaim {
    fn event(&self) -> SiemOutboxEvent {
        SiemOutboxEvent {
            tenant_id: self.tenant_id,
            event_id: self.event_id,
            audit_event_id: self.audit_event_id,
            event_envelope: self.event_envelope.clone(),
            attempt_count: self.attempt_count,
        }
    }
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

mod activation_sessions;
mod outbox_worker;
mod repository_core;

impl GovernanceSqliteRepository {
    fn connection(&self) -> Result<MutexGuard<'_, Connection>, GovernanceRepositoryError> {
        self.connection
            .lock()
            .map_err(|_| GovernanceRepositoryError::Database)
    }
}

fn database_error(_: rusqlite::Error) -> GovernanceRepositoryError {
    GovernanceRepositoryError::Database
}
