//! Durable governance revision, activation, and SIEM outbox plans.

use super::*;
use prodex_domain::{
    ApprovalError, ApprovalFingerprint, ApprovalId, ApprovalReasonCode, ApprovalRecord,
    ApprovalState, AuditAction, AuditDigest, AuditEvent, AuditEventId, AuditOutcome,
    AuditReasonCode, AuditResource, AuditResourceId, AuditTimestamp, Channel, CredentialScope,
    DataClassification, IdempotencyKey, PolicyRevisionId, Principal, PrincipalId, TenantId,
    compute_audit_chain_digest,
};

pub const MAX_COMPILED_GOVERNANCE_ARTIFACT_BYTES: usize = 1024 * 1024;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceActivationAction {
    Activate,
    Rollback,
}

impl GovernanceActivationAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Activate => "activate",
            Self::Rollback => "rollback",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceActivationRequest {
    pub tenant_id: TenantId,
    pub kind: GovernanceArtifactKind,
    pub revision_id: String,
    pub approval_id: ApprovalId,
    pub actor: Principal,
    pub action: GovernanceActivationAction,
    pub expected_etag: Option<String>,
    pub idempotency_key: IdempotencyKey,
    pub request_fingerprint: String,
    pub audit_outbox: AuditOutboxWriteCommand,
    pub activated_at_unix_ms: u64,
}

impl fmt::Debug for GovernanceActivationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceActivationRequest")
            .field("tenant_id", &"<redacted>")
            .field("kind", &self.kind)
            .field("revision_id", &"<redacted>")
            .field("approval_id", &"<redacted>")
            .field("actor", &"<redacted>")
            .field("action", &self.action)
            .field(
                "expected_etag",
                &self.expected_etag.as_ref().map(|_| "<redacted>"),
            )
            .field("idempotency_key", &"<redacted>")
            .field("request_fingerprint", &"<redacted>")
            .field("audit_outbox", &"<redacted>")
            .field("activated_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GovernanceActivationResult {
    pub outcome: GovernanceWriteOutcome,
    pub kind: GovernanceArtifactKind,
    pub revision_id: String,
    pub etag: String,
    pub last_known_good_revision_id: String,
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

#[derive(Clone, PartialEq, Eq)]
pub struct ApprovalVoteIdempotency {
    pub operation: IdempotentOperation,
    pub started_at_unix_ms: u64,
}

impl fmt::Debug for ApprovalVoteIdempotency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovalVoteIdempotency")
            .field("operation", &"<redacted>")
            .field("started_at_unix_ms", &"<redacted>")
            .finish()
    }
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

pub fn denied_approval_audit_outbox(
    mut command: AuditOutboxWriteCommand,
    denial: ApprovalVoteStableDenial,
) -> AuditOutboxWriteCommand {
    command.audit.event.outcome = AuditOutcome::Denied;
    command.audit.event.reason_code = Some(denial.reason_code().to_string());
    command.audit.event_digest =
        compute_audit_chain_digest(command.audit.previous_digest.as_ref(), &command.audit.event);
    command
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GovernanceAuditIntegrityHealth {
    pub event_count: u64,
    pub chain_head_count: u64,
    pub chain_valid: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceAuditExportRecord {
    pub audit_event_id: String,
    pub occurred_at_unix_ms: u64,
    pub principal_id: String,
    pub action: String,
    pub resource_kind: String,
    pub resource_id: Option<String>,
    pub outcome: String,
    pub reason_code: Option<String>,
    pub previous_digest: Option<String>,
    pub event_digest: String,
}

/// Verifies both the topology and the canonical digest of every persisted audit event.
/// Invalid persisted fields deliberately collapse to the same redacted health result.
pub fn verify_governance_audit_integrity(
    tenant_id: TenantId,
    records: &[GovernanceAuditExportRecord],
) -> GovernanceAuditIntegrityHealth {
    use std::collections::{HashMap, HashSet};

    let event_count = u64::try_from(records.len()).unwrap_or(u64::MAX);
    let mut valid = true;
    let mut digests = HashSet::with_capacity(records.len());
    let mut child_by_previous = HashMap::with_capacity(records.len());
    let mut root = None;

    for record in records {
        if !digests.insert(record.event_digest.clone()) {
            valid = false;
        }
        match record.previous_digest.as_ref() {
            Some(previous) => {
                if child_by_previous
                    .insert(previous.clone(), record.event_digest.clone())
                    .is_some()
                {
                    valid = false;
                }
            }
            None => {
                if root.replace(record.event_digest.clone()).is_some() {
                    valid = false;
                }
            }
        }
        valid &= audit_record_digest_is_valid(tenant_id, record);
    }

    valid &= child_by_previous
        .keys()
        .all(|previous| digests.contains(previous));
    let chain_head_count = u64::try_from(
        digests
            .iter()
            .filter(|digest| !child_by_previous.contains_key(digest.as_str()))
            .count(),
    )
    .unwrap_or(u64::MAX);

    if records.is_empty() {
        valid &= root.is_none() && chain_head_count == 0;
    } else if let Some(mut current) = root {
        let mut visited = HashSet::with_capacity(records.len());
        while visited.insert(current.clone()) {
            let Some(next) = child_by_previous.get(&current) else {
                break;
            };
            current = next.clone();
        }
        valid &= visited.len() == records.len() && chain_head_count == 1;
    } else {
        valid = false;
    }

    GovernanceAuditIntegrityHealth {
        event_count,
        chain_head_count,
        chain_valid: valid,
    }
}

fn audit_record_digest_is_valid(tenant_id: TenantId, record: &GovernanceAuditExportRecord) -> bool {
    let parsed = (|| {
        let id = record.audit_event_id.parse::<AuditEventId>().ok()?;
        AuditTimestamp::new(record.occurred_at_unix_ms).ok()?;
        let principal_id = record.principal_id.parse::<PrincipalId>().ok()?;
        let action = AuditAction::try_new(record.action.clone()).ok()?;
        let resource_id = record
            .resource_id
            .clone()
            .map(AuditResourceId::new)
            .transpose()
            .ok()?;
        let resource = AuditResource::new_with_resource_id(
            record.resource_kind.clone(),
            resource_id,
            Some(tenant_id),
        )
        .ok()?;
        let outcome = AuditOutcome::parse(&record.outcome).ok()?;
        if let Some(reason_code) = record.reason_code.as_ref() {
            AuditReasonCode::new(reason_code.clone()).ok()?;
        }
        let previous_digest = record
            .previous_digest
            .clone()
            .map(AuditDigest::new)
            .transpose()
            .ok()?;
        let stored_digest = AuditDigest::new(record.event_digest.clone()).ok()?;
        Some((
            AuditEvent {
                id,
                occurred_at_unix_ms: record.occurred_at_unix_ms,
                tenant_id,
                principal_id,
                action,
                resource,
                outcome,
                reason_code: record.reason_code.clone(),
            },
            previous_digest,
            stored_digest,
        ))
    })();
    let Some((event, previous_digest, stored_digest)) = parsed else {
        return false;
    };
    compute_audit_chain_digest(previous_digest.as_ref(), &event) == stored_digest
}

impl fmt::Debug for GovernanceAuditExportRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceAuditExportRecord")
            .field("audit_event_id", &"<redacted>")
            .field("occurred_at_unix_ms", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("action", &self.action)
            .field("resource_kind", &self.resource_kind)
            .field(
                "resource_id",
                &self.resource_id.as_ref().map(|_| "<redacted>"),
            )
            .field("outcome", &self.outcome)
            .field("reason_code", &self.reason_code)
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
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
