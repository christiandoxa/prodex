//! Durable governance revision, activation, and SIEM outbox plans.

use super::*;
use prodex_domain::{ApprovalFingerprint, ApprovalRecord, ApprovalState, AuditEventId};

pub const MAX_COMPILED_GOVERNANCE_ARTIFACT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceArtifactKind {
    Policy,
    ClassificationRules,
    ProviderRegistry,
    RoutingScores,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceRevisionWriteCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub kind: GovernanceArtifactKind,
    pub revision_id: String,
    pub fingerprint: ApprovalFingerprint,
    pub compiled_artifact: Vec<u8>,
    pub created_by: PrincipalId,
    pub created_at_unix_ms: u64,
}

impl fmt::Debug for GovernanceRevisionWriteCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceRevisionWriteCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("kind", &self.kind)
            .field("revision_id", &"<redacted>")
            .field("fingerprint", &self.fingerprint)
            .field("compiled_artifact_bytes", &self.compiled_artifact.len())
            .field("created_by", &"<redacted>")
            .field("created_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceRevisionWritePlan(pub GovernanceRevisionWriteCommand);

impl fmt::Debug for GovernanceRevisionWritePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceRevisionWritePlan")
            .field("kind", &self.0.kind)
            .field("compiled_artifact_bytes", &self.0.compiled_artifact.len())
            .finish()
    }
}

pub fn plan_governance_revision_write(
    command: GovernanceRevisionWriteCommand,
) -> Result<GovernanceRevisionWritePlan, GovernanceStorageError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(GovernanceStorageError::TenantMismatch);
    }
    if command.revision_id.is_empty()
        || command.revision_id.len() > 128
        || !command.revision_id.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
    {
        return Err(GovernanceStorageError::InvalidRevision);
    }
    if command.compiled_artifact.is_empty()
        || command.compiled_artifact.len() > MAX_COMPILED_GOVERNANCE_ARTIFACT_BYTES
    {
        return Err(GovernanceStorageError::ArtifactSizeInvalid);
    }
    Ok(GovernanceRevisionWritePlan(command))
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceActivationCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub kind: GovernanceArtifactKind,
    pub revision_id: String,
    pub fingerprint: ApprovalFingerprint,
    pub approval: ApprovalRecord,
    pub expected_etag: String,
    pub current_active_revision_id: Option<String>,
    pub current_last_known_good_revision_id: Option<String>,
    pub activated_at_unix_ms: u64,
}

impl fmt::Debug for GovernanceActivationCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceActivationCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("kind", &self.kind)
            .field("revision_id", &"<redacted>")
            .field("fingerprint", &self.fingerprint)
            .field("approval", &"<redacted>")
            .field("expected_etag", &"<redacted>")
            .field("current_active_revision_id", &"<redacted>")
            .field("current_last_known_good_revision_id", &"<redacted>")
            .field("activated_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceActivationPlan {
    pub tenant_id: TenantId,
    pub kind: GovernanceArtifactKind,
    pub activated_revision_id: String,
    pub previous_active_revision_id: Option<String>,
    pub last_known_good_revision_id: Option<String>,
    pub expected_etag: String,
    pub transactional_audit_required: bool,
    pub invalidation_required: bool,
}

impl fmt::Debug for GovernanceActivationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceActivationPlan")
            .field("tenant_id", &"<redacted>")
            .field("kind", &self.kind)
            .field("activated_revision_id", &"<redacted>")
            .field("previous_active_revision_id", &"<redacted>")
            .field("last_known_good_revision_id", &"<redacted>")
            .field("expected_etag", &"<redacted>")
            .field("transactional_audit_required", &true)
            .field("invalidation_required", &true)
            .finish()
    }
}

pub fn plan_governance_activation(
    command: GovernanceActivationCommand,
) -> Result<GovernanceActivationPlan, GovernanceStorageError> {
    if command.storage_key.tenant_id != command.tenant_id
        || command.approval.tenant_id != command.tenant_id
    {
        return Err(GovernanceStorageError::TenantMismatch);
    }
    if command.approval.state != ApprovalState::Approved
        || command.approval.fingerprint != command.fingerprint
    {
        return Err(GovernanceStorageError::ApprovalRequired);
    }
    if command.expected_etag.is_empty() || command.expected_etag.len() > 128 {
        return Err(GovernanceStorageError::InvalidEtag);
    }
    Ok(GovernanceActivationPlan {
        tenant_id: command.tenant_id,
        kind: command.kind,
        activated_revision_id: command.revision_id,
        last_known_good_revision_id: command
            .current_active_revision_id
            .clone()
            .or(command.current_last_known_good_revision_id),
        previous_active_revision_id: command.current_active_revision_id,
        expected_etag: command.expected_etag,
        transactional_audit_required: true,
        invalidation_required: true,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SiemOutboxRetryPolicy {
    pub max_attempts: u8,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl SiemOutboxRetryPolicy {
    pub fn bounded(
        max_attempts: u8,
        base_delay_ms: u64,
        max_delay_ms: u64,
    ) -> Result<Self, GovernanceStorageError> {
        if max_attempts == 0
            || max_attempts > 32
            || base_delay_ms == 0
            || max_delay_ms < base_delay_ms
        {
            return Err(GovernanceStorageError::InvalidRetryPolicy);
        }
        Ok(Self {
            max_attempts,
            base_delay_ms,
            max_delay_ms,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuditOutboxWriteCommand {
    pub audit: AppendOnlyAuditCommand,
    pub outbox_event_id: AuditEventId,
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuditOutboxWritePlan {
    pub audit: AppendOnlyAuditPlan,
    pub outbox_event_id: AuditEventId,
    pub atomic: bool,
}

pub fn plan_audit_outbox_write(
    command: AuditOutboxWriteCommand,
) -> Result<AuditOutboxWritePlan, GovernanceStorageError> {
    let audit = plan_append_only_audit(command.audit)
        .map_err(|_| GovernanceStorageError::TenantMismatch)?;
    Ok(AuditOutboxWritePlan {
        audit,
        outbox_event_id: command.outbox_event_id,
        atomic: true,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiemOutboxDeliveryDecision {
    Delivered,
    RetryAt(u64),
    DeadLetter,
}

pub fn plan_siem_outbox_delivery(
    policy: SiemOutboxRetryPolicy,
    completed_attempts: u8,
    delivered: bool,
    now_unix_ms: u64,
) -> SiemOutboxDeliveryDecision {
    if delivered {
        return SiemOutboxDeliveryDecision::Delivered;
    }
    let next_attempt = completed_attempts.saturating_add(1);
    if next_attempt >= policy.max_attempts {
        return SiemOutboxDeliveryDecision::DeadLetter;
    }
    let exponent = u32::from(next_attempt.saturating_sub(1)).min(31);
    let delay = policy
        .base_delay_ms
        .saturating_mul(1_u64 << exponent)
        .min(policy.max_delay_ms);
    SiemOutboxDeliveryDecision::RetryAt(now_unix_ms.saturating_add(delay))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceStorageError {
    TenantMismatch,
    InvalidRevision,
    ArtifactSizeInvalid,
    ApprovalRequired,
    InvalidEtag,
    InvalidRetryPolicy,
}

impl fmt::Display for GovernanceStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "governance storage operation is invalid")
    }
}

impl Error for GovernanceStorageError {}
