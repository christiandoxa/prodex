//! Durable governance revision, activation, and SIEM outbox plans.

use super::*;
use prodex_domain::{
    ApprovalError, ApprovalFingerprint, ApprovalId, ApprovalReasonCode, ApprovalRecord,
    ApprovalState, AuditEventId, Channel, CredentialScope, DataClassification, IdempotencyKey,
    PolicyRevisionId, Principal, PrincipalId, TenantId,
};

mod decisions;
pub use decisions::*;
mod activation;
pub use activation::*;
mod audit_integrity;
pub use audit_integrity::*;
mod idempotency;
pub use idempotency::*;

pub const MAX_COMPILED_GOVERNANCE_ARTIFACT_BYTES: usize = 1024 * 1024;
pub const MAX_GOVERNANCE_SIGNATURE_BYTES: usize = 128;
pub const GOVERNANCE_INVALIDATION_CHANNEL: &str = "prodex_governance_invalidation";
pub const MAX_GOVERNANCE_INVALIDATION_PAYLOAD_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceArtifactKind {
    Policy,
    ClassificationRules,
    ProviderRegistry,
    RoutingScores,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceWriteOutcome {
    Applied,
    Replayed,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApprovalVoteRequest {
    pub tenant_id: TenantId,
    pub approval_id: ApprovalId,
    pub actor: Principal,
    pub expected_version: u64,
    pub now_unix_ms: u64,
    pub reason: Option<ApprovalReasonCode>,
    pub audit_outbox: AuditOutboxWriteCommand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalVoteStableDenial {
    SelfApproval,
    StaleVersion,
    InvalidTransition,
}

impl ApprovalVoteStableDenial {
    pub const fn from_approval_error(error: ApprovalError) -> Option<Self> {
        match error {
            ApprovalError::SelfApprovalDenied => Some(Self::SelfApproval),
            ApprovalError::StaleVersion => Some(Self::StaleVersion),
            ApprovalError::InvalidTransition => Some(Self::InvalidTransition),
            ApprovalError::InvalidToken
            | ApprovalError::InvalidQuorum
            | ApprovalError::InvalidExpiry
            | ApprovalError::TenantMismatch
            | ApprovalError::ReplayMismatch => None,
        }
    }

    pub const fn reason_code(self) -> &'static str {
        match self {
            Self::SelfApproval => "approval.self_approval_denied",
            Self::StaleVersion => "approval.stale_version",
            Self::InvalidTransition => "approval.invalid_transition",
        }
    }

    pub const fn repository_error(self) -> GovernanceRepositoryError {
        match self {
            Self::SelfApproval => GovernanceRepositoryError::ApprovalSelfAction,
            Self::StaleVersion => GovernanceRepositoryError::StaleVersion,
            Self::InvalidTransition => GovernanceRepositoryError::InvalidTransition,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApprovalVoteSnapshot {
    pub state: ApprovalState,
    pub version: u64,
    pub required_quorum: u8,
    pub vote_count: usize,
    pub expires_at_unix_ms: u64,
    pub activated_at_unix_ms: Option<u64>,
}

impl ApprovalVoteSnapshot {
    pub fn from_record(record: &ApprovalRecord) -> Self {
        Self {
            state: record.state,
            version: record.version,
            required_quorum: record.effective_required_quorum(),
            vote_count: record.votes.len(),
            expires_at_unix_ms: record.expires_at_unix_ms,
            activated_at_unix_ms: record.activated_at_unix_ms,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalVoteStableOutcome {
    Success(ApprovalVoteSnapshot),
    Denied(ApprovalVoteStableDenial),
}

impl ApprovalVoteStableOutcome {
    pub fn encode(self) -> Vec<u8> {
        match self {
            Self::Success(snapshot) => format!(
                "v1|ok|{}|{}|{}|{}|{}|{}",
                crate::governance_support::approval_state_label(snapshot.state),
                snapshot.version,
                snapshot.required_quorum,
                snapshot.vote_count,
                snapshot.expires_at_unix_ms,
                snapshot
                    .activated_at_unix_ms
                    .map_or_else(|| "none".to_string(), |value| value.to_string()),
            )
            .into_bytes(),
            Self::Denied(denial) => format!("v1|denied|{}", denial.reason_code()).into_bytes(),
        }
    }

    pub fn decode(value: &[u8]) -> Result<Self, GovernanceRepositoryError> {
        if value.len() > 256 {
            return Err(GovernanceRepositoryError::InvalidInput);
        }
        let value =
            std::str::from_utf8(value).map_err(|_| GovernanceRepositoryError::InvalidInput)?;
        let fields = value.split('|').collect::<Vec<_>>();
        match fields.as_slice() {
            ["v1", "denied", reason] => Ok(Self::Denied(match *reason {
                "approval.self_approval_denied" => ApprovalVoteStableDenial::SelfApproval,
                "approval.stale_version" => ApprovalVoteStableDenial::StaleVersion,
                "approval.invalid_transition" => ApprovalVoteStableDenial::InvalidTransition,
                _ => return Err(GovernanceRepositoryError::InvalidInput),
            })),
            [
                "v1",
                "ok",
                state,
                version,
                quorum,
                vote_count,
                expires,
                activated,
            ] => {
                let state = crate::governance_support::approval_state_from_label(state)?;
                let required_quorum = quorum
                    .parse::<u8>()
                    .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
                if required_quorum == 0 || required_quorum > prodex_domain::MAX_APPROVAL_QUORUM {
                    return Err(GovernanceRepositoryError::InvalidInput);
                }
                let version = version
                    .parse()
                    .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
                let vote_count = vote_count
                    .parse()
                    .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
                let expires_at_unix_ms = expires
                    .parse()
                    .map_err(|_| GovernanceRepositoryError::InvalidInput)?;
                if version == 0
                    || expires_at_unix_ms == 0
                    || vote_count > usize::from(prodex_domain::MAX_APPROVAL_QUORUM)
                {
                    return Err(GovernanceRepositoryError::InvalidInput);
                }
                Ok(Self::Success(ApprovalVoteSnapshot {
                    state,
                    version,
                    required_quorum,
                    vote_count,
                    expires_at_unix_ms,
                    activated_at_unix_ms: if *activated == "none" {
                        None
                    } else {
                        Some(
                            activated
                                .parse()
                                .map_err(|_| GovernanceRepositoryError::InvalidInput)?,
                        )
                    },
                }))
            }
            _ => Err(GovernanceRepositoryError::InvalidInput),
        }
    }

    pub fn replay(value: &[u8]) -> Result<ApprovalVoteMutationOutcome, GovernanceRepositoryError> {
        match Self::decode(value)? {
            Self::Success(snapshot) => Ok(ApprovalVoteMutationOutcome::Replayed(snapshot)),
            Self::Denied(denial) => Err(denial.repository_error()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalVoteMutationOutcome {
    Applied(ApprovalRecord),
    Replayed(ApprovalVoteSnapshot),
}

impl fmt::Debug for ApprovalVoteRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovalVoteRequest")
            .field("tenant_id", &"<redacted>")
            .field("approval_id", &"<redacted>")
            .field("actor", &"<redacted>")
            .field("expected_version", &"<redacted>")
            .field("now_unix_ms", &"<redacted>")
            .field("reason", &self.reason.as_ref().map(|_| "<redacted>"))
            .field("audit_outbox", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GovernanceRevisionSummary {
    pub revision_id: String,
    pub fingerprint: String,
    pub lifecycle_state: String,
    pub signature_key_id: Option<String>,
    pub created_at_unix_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GovernanceStatus {
    pub active_revision_id: Option<String>,
    pub last_known_good_revision_id: Option<String>,
    pub etag: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceSnapshotSource {
    Active,
    LastKnownGood,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceSnapshot {
    pub tenant_id: TenantId,
    pub kind: GovernanceArtifactKind,
    pub revision_id: String,
    pub compiled_artifact: Vec<u8>,
    pub source: GovernanceSnapshotSource,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceArtifactAuthenticity {
    pub key_id: String,
    pub signature_base64: String,
}

impl fmt::Debug for GovernanceArtifactAuthenticity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceArtifactAuthenticity")
            .field("key_id", &self.key_id)
            .field("signature_base64", &"<redacted>")
            .finish()
    }
}

impl GovernanceArtifactAuthenticity {
    pub fn is_well_formed(&self) -> bool {
        !self.key_id.is_empty()
            && self.key_id.len() <= 64
            && self
                .key_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            && !self.signature_base64.is_empty()
            && self.signature_base64.len() <= MAX_GOVERNANCE_SIGNATURE_BYTES
            && self.signature_base64.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'_' | b'-' | b'=')
            })
    }
}

#[derive(Clone, Copy)]
pub struct GovernanceArtifactValidationInput<'a> {
    pub tenant_id: TenantId,
    pub kind: GovernanceArtifactKind,
    pub revision_id: &'a str,
    pub compiled_artifact: &'a [u8],
    pub authenticity: Option<&'a GovernanceArtifactAuthenticity>,
}

impl fmt::Debug for GovernanceArtifactValidationInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceArtifactValidationInput")
            .field("tenant_id", &"<redacted>")
            .field("kind", &self.kind)
            .field("revision_id", &"<redacted>")
            .field("compiled_artifact_bytes", &self.compiled_artifact.len())
            .field("authenticity", &self.authenticity)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceSessionUpsertCommand {
    pub tenant_id: TenantId,
    pub session_id_hash: String,
    pub principal_id: PrincipalId,
    pub channel: Channel,
    pub credential_scope: CredentialScope,
    pub classification: DataClassification,
    pub policy_revision_id: PolicyRevisionId,
    pub provider_registry_revision: String,
    pub provider_descriptor_revision: u64,
    pub provider_affinity: Option<String>,
    pub created_at_unix_ms: u64,
    pub last_seen_at_unix_ms: u64,
    pub absolute_expires_at_unix_ms: u64,
    pub idle_expires_at_unix_ms: u64,
    pub max_concurrent: Option<u32>,
}

impl fmt::Debug for GovernanceSessionUpsertCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceSessionUpsertCommand")
            .field("tenant_id", &"<redacted>")
            .field("session_id_hash", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("channel", &self.channel)
            .field("credential_scope", &self.credential_scope)
            .field("classification", &self.classification)
            .field("policy_revision_id", &"<redacted>")
            .field("provider_registry_revision", &"<redacted>")
            .field("provider_descriptor_revision", &"<redacted>")
            .field(
                "provider_affinity",
                &self.provider_affinity.as_ref().map(|_| "<redacted>"),
            )
            .field("created_at_unix_ms", &"<redacted>")
            .field("last_seen_at_unix_ms", &"<redacted>")
            .field("absolute_expires_at_unix_ms", &"<redacted>")
            .field("idle_expires_at_unix_ms", &"<redacted>")
            .field("max_concurrent", &self.max_concurrent)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceSessionRecord {
    pub tenant_id: TenantId,
    pub session_id_hash: String,
    pub principal_id: PrincipalId,
    pub channel: Channel,
    pub credential_scope: CredentialScope,
    pub classification: DataClassification,
    pub policy_revision_id: PolicyRevisionId,
    pub provider_registry_revision: String,
    pub provider_descriptor_revision: u64,
    pub provider_affinity: Option<String>,
    pub created_at_unix_ms: u64,
    pub last_seen_at_unix_ms: u64,
    pub absolute_expires_at_unix_ms: u64,
    pub idle_expires_at_unix_ms: u64,
    pub revoked_at_unix_ms: Option<u64>,
    pub revocation_reason_code: Option<String>,
}

impl fmt::Debug for GovernanceSessionRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceSessionRecord")
            .field("tenant_id", &"<redacted>")
            .field("session_id_hash", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("channel", &self.channel)
            .field("credential_scope", &self.credential_scope)
            .field("classification", &self.classification)
            .field("policy_revision_id", &"<redacted>")
            .field("provider_registry_revision", &"<redacted>")
            .field("provider_descriptor_revision", &"<redacted>")
            .field(
                "provider_affinity",
                &self.provider_affinity.as_ref().map(|_| "<redacted>"),
            )
            .field("created_at_unix_ms", &"<redacted>")
            .field("last_seen_at_unix_ms", &"<redacted>")
            .field("absolute_expires_at_unix_ms", &"<redacted>")
            .field("idle_expires_at_unix_ms", &"<redacted>")
            .field("revoked", &self.revoked_at_unix_ms.is_some())
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GovernanceSessionUpsertOutcome {
    Stored(Box<GovernanceSessionRecord>),
    ConcurrentLimitReached,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceSessionRevokeCommand {
    pub tenant_id: TenantId,
    pub session_id_hash: String,
    pub revoked_at_unix_ms: u64,
    pub reason_code: String,
    pub audit_outbox: AuditOutboxWriteCommand,
}

impl fmt::Debug for GovernanceSessionRevokeCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceSessionRevokeCommand")
            .field("tenant_id", &"<redacted>")
            .field("session_id_hash", &"<redacted>")
            .field("revoked_at_unix_ms", &"<redacted>")
            .field("reason_code", &self.reason_code)
            .field("audit_outbox", &"<redacted>")
            .finish()
    }
}

impl fmt::Debug for GovernanceSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceSnapshot")
            .field("tenant_id", &"<redacted>")
            .field("kind", &self.kind)
            .field("revision_id", &"<redacted>")
            .field("compiled_artifact_bytes", &self.compiled_artifact.len())
            .field("source", &self.source)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GovernanceOutboxHealth {
    pub pending: u64,
    pub dead_lettered: u64,
    pub oldest_pending_at_unix_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceRepositoryError {
    Database,
    InvalidInput,
    TenantMismatch,
    NotFound,
    Conflict,
    EtagMismatch,
    ApprovalRequired,
    SnapshotUnavailable,
    AuditChainConflict,
    ApprovalSelfAction,
    StaleVersion,
    InvalidTransition,
    Unsupported,
}

impl fmt::Display for GovernanceRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "governance repository operation failed")
    }
}

impl Error for GovernanceRepositoryError {}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceRevisionWriteCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub kind: GovernanceArtifactKind,
    pub revision_id: String,
    pub fingerprint: ApprovalFingerprint,
    pub compiled_artifact: Vec<u8>,
    pub authenticity: Option<GovernanceArtifactAuthenticity>,
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
            .field("authenticity", &self.authenticity)
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
    if command
        .authenticity
        .as_ref()
        .is_some_and(|authenticity| !authenticity.is_well_formed())
    {
        return Err(GovernanceStorageError::InvalidAuthenticity);
    }
    Ok(GovernanceRevisionWritePlan(command))
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
    InvalidAuthenticity,
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
