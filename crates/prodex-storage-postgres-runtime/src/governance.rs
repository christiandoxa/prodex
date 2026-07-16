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
    ApprovalAction, ApprovalFingerprint, ApprovalId, ApprovalReasonCode, ApprovalRecord,
    ApprovalScope, ApprovalState, ApprovalVote, AuditDigest, AuditEventId,
    IdempotencyReplayDecision, PolicyRevisionId, PrincipalId, TenantId, decide_idempotency_replay,
};
use prodex_storage::{
    ApprovalVoteIdempotency, ApprovalVoteMutationOutcome, ApprovalVoteRequest,
    ApprovalVoteSnapshot, ApprovalVoteStableOutcome, ApprovalVoteTransitionDecision,
    AuditOutboxWriteCommand, GovernanceActivationCurrent, GovernanceActivationRequest,
    GovernanceActivationResult, GovernanceArtifactKind, GovernanceAuditExportRecord,
    GovernanceAuditIntegrityHealth, GovernanceOutboxHealth, GovernanceRepositoryError,
    GovernanceRevisionSummary, GovernanceRevisionWriteCommand, GovernanceSessionRecord,
    GovernanceSessionRevokeCommand, GovernanceSessionUpsertCommand, GovernanceSessionUpsertOutcome,
    GovernanceSnapshot, GovernanceSnapshotSource, GovernanceStatus, GovernanceWriteOutcome,
    IdempotencyRecordLookupRow, IdempotencyRecordLookupRowStatus, SiemOutboxDeliveryDecision,
    SiemOutboxRetryPolicy, denied_approval_audit_outbox,
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

mod activation_sessions;
mod outbox_claims;
mod repository_core;

impl PostgresRepository {
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
