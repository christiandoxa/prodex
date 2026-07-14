//! Tenant-bound maker-checker approval state machine.

use std::error::Error;
use std::fmt;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{GovernedAction, PolicyRevisionId, Principal, PrincipalId, TenantId};

const MAX_APPROVAL_TOKEN_BYTES: usize = 128;
pub const MAX_APPROVAL_QUORUM: u8 = 16;
pub const HIGH_RISK_MINIMUM_APPROVAL_QUORUM: u8 = 2;
const MAX_EXECUTION_APPROVAL_BINDING_BYTES: usize = 4 * 1024;

macro_rules! approval_token {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ApprovalError> {
                let value = value.into();
                if !approval_token_is_valid(&value) {
                    return Err(ApprovalError::InvalidToken);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&"<redacted>")
                    .finish()
            }
        }
    };
}

approval_token!(ApprovalId);
approval_token!(ApprovalFingerprint);
approval_token!(ApprovalScope);
approval_token!(ApprovalReasonCode);

#[derive(Clone, Copy, Debug)]
pub struct ExecutionApprovalBinding<'a> {
    pub tenant_id: TenantId,
    pub principal_id: PrincipalId,
    pub session_id_hash: &'a str,
    pub action: GovernedAction,
    pub model: Option<&'a str>,
    pub tools: &'a [String],
    pub request_body_digest: &'a [u8; 32],
    pub policy_revision_id: PolicyRevisionId,
}

impl ExecutionApprovalBinding<'_> {
    pub fn fingerprint(&self) -> Result<ApprovalFingerprint, ApprovalError> {
        if self.session_id_hash.is_empty() {
            return Err(ApprovalError::InvalidToken);
        }
        let mut tools = self.tools.iter().map(String::as_str).collect::<Vec<_>>();
        tools.sort_unstable();
        tools.dedup();
        let fields = [
            self.tenant_id.to_string(),
            self.principal_id.to_string(),
            self.session_id_hash.to_string(),
            governed_action_label(self.action).to_string(),
            self.model.unwrap_or("none").to_string(),
            self.policy_revision_id.to_string(),
        ];
        let size = fields.iter().map(String::len).sum::<usize>()
            + tools.iter().map(|tool| tool.len()).sum::<usize>();
        if size > MAX_EXECUTION_APPROVAL_BINDING_BYTES {
            return Err(ApprovalError::InvalidToken);
        }
        let mut digest = Sha256::new();
        digest.update(b"prodex.execution-approval.v1");
        for field in fields {
            hash_field(&mut digest, field.as_bytes());
        }
        hash_field(&mut digest, self.request_body_digest);
        for tool in tools {
            hash_field(&mut digest, tool.as_bytes());
        }
        let hex = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        ApprovalFingerprint::new(format!("sha256:{hex}"))
    }
}

pub fn execution_approval_id(
    fingerprint: &ApprovalFingerprint,
) -> Result<ApprovalId, ApprovalError> {
    ApprovalId::new(format!("execution:{}", fingerprint.as_str()))
}

fn hash_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

const fn governed_action_label(action: GovernedAction) -> &'static str {
    match action {
        GovernedAction::InvokeModel => "invoke_model",
        GovernedAction::UseTool => "use_tool",
        GovernedAction::UploadContent => "upload_content",
        GovernedAction::CompactContext => "compact_context",
        GovernedAction::MutateControlPlane => "mutate_control_plane",
    }
}

fn approval_token_is_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_APPROVAL_TOKEN_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ApprovalKind {
    PolicyRevision,
    ClassificationRuleRevision,
    ProviderRegistryRevision,
    RoutingScoreRevision,
    HighImpactConfiguration,
    Execution,
    BreakGlass,
}

impl ApprovalKind {
    pub const fn minimum_quorum(self) -> u8 {
        match self {
            Self::HighImpactConfiguration | Self::Execution | Self::BreakGlass => {
                HIGH_RISK_MINIMUM_APPROVAL_QUORUM
            }
            Self::PolicyRevision
            | Self::ClassificationRuleRevision
            | Self::ProviderRegistryRevision
            | Self::RoutingScoreRevision => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ApprovalState {
    Draft,
    PendingApproval,
    Approved,
    Rejected,
    Expired,
    Cancelled,
    Active,
    Superseded,
    RolledBack,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct ApprovalVote {
    pub checker: PrincipalId,
    pub approved_at_unix_ms: u64,
}

impl fmt::Debug for ApprovalVote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovalVote")
            .field("checker", &"<redacted>")
            .field("approved_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct ApprovalRecord {
    pub id: ApprovalId,
    pub tenant_id: TenantId,
    pub kind: ApprovalKind,
    pub scope: ApprovalScope,
    pub fingerprint: ApprovalFingerprint,
    pub maker: PrincipalId,
    pub state: ApprovalState,
    pub required_quorum: u8,
    pub votes: Vec<ApprovalVote>,
    pub expires_at_unix_ms: u64,
    pub activated_at_unix_ms: Option<u64>,
    pub termination_reason: Option<ApprovalReasonCode>,
    pub version: u64,
}

impl fmt::Debug for ApprovalRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovalRecord")
            .field("id", &self.id)
            .field("tenant_id", &"<redacted>")
            .field("kind", &self.kind)
            .field("scope", &self.scope)
            .field("fingerprint", &self.fingerprint)
            .field("maker", &"<redacted>")
            .field("state", &self.state)
            .field("required_quorum", &self.required_quorum)
            .field("vote_count", &self.votes.len())
            .field("expires_at_unix_ms", &"<redacted>")
            .field(
                "activated_at_unix_ms",
                &self.activated_at_unix_ms.map(|_| "<redacted>"),
            )
            .field(
                "termination_reason",
                &self.termination_reason.as_ref().map(|_| "<redacted>"),
            )
            .field("version", &self.version)
            .finish()
    }
}

impl ApprovalRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn pending(
        id: ApprovalId,
        tenant_id: TenantId,
        kind: ApprovalKind,
        scope: ApprovalScope,
        fingerprint: ApprovalFingerprint,
        maker: PrincipalId,
        requested_quorum: u8,
        expires_at_unix_ms: u64,
    ) -> Result<Self, ApprovalError> {
        if requested_quorum == 0 || requested_quorum > MAX_APPROVAL_QUORUM {
            return Err(ApprovalError::InvalidQuorum);
        }
        if expires_at_unix_ms == 0 {
            return Err(ApprovalError::InvalidExpiry);
        }
        Ok(Self {
            id,
            tenant_id,
            kind,
            scope,
            fingerprint,
            maker,
            state: ApprovalState::PendingApproval,
            required_quorum: requested_quorum.max(kind.minimum_quorum()),
            votes: Vec::new(),
            expires_at_unix_ms,
            activated_at_unix_ms: None,
            termination_reason: None,
            version: 1,
        })
    }

    pub const fn effective_required_quorum(&self) -> u8 {
        if self.required_quorum < self.kind.minimum_quorum() {
            self.kind.minimum_quorum()
        } else {
            self.required_quorum
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalAction {
    Approve,
    Reject,
    Cancel,
    Activate,
    Supersede,
    RollBack,
}

pub struct ApprovalTransitionRequest<'a> {
    pub record: &'a ApprovalRecord,
    pub actor: &'a Principal,
    pub action: ApprovalAction,
    pub expected_version: u64,
    pub now_unix_ms: u64,
    pub reason: Option<&'a ApprovalReasonCode>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApprovalTransition {
    pub record: ApprovalRecord,
    pub reason_code: ApprovalReasonCode,
    pub changed: bool,
}

impl fmt::Debug for ApprovalTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovalTransition")
            .field("record", &self.record)
            .field("reason_code", &self.reason_code)
            .field("changed", &self.changed)
            .finish()
    }
}

pub fn transition_approval(
    request: ApprovalTransitionRequest<'_>,
) -> Result<ApprovalTransition, ApprovalError> {
    if request.actor.tenant_id != Some(request.record.tenant_id) {
        return Err(ApprovalError::TenantMismatch);
    }
    if request.expected_version != request.record.version {
        return Err(ApprovalError::StaleVersion);
    }
    if request.record.required_quorum == 0 || request.record.required_quorum > MAX_APPROVAL_QUORUM {
        return Err(ApprovalError::InvalidQuorum);
    }
    let mut record = request.record.clone();
    if request.now_unix_ms >= record.expires_at_unix_ms
        && matches!(
            record.state,
            ApprovalState::Draft | ApprovalState::PendingApproval | ApprovalState::Approved
        )
    {
        record.state = ApprovalState::Expired;
        record.termination_reason = Some(ApprovalReasonCode::new("approval.expired")?);
        record.version = record.version.saturating_add(1);
        return Ok(ApprovalTransition {
            record,
            reason_code: ApprovalReasonCode::new("approval.expired")?,
            changed: true,
        });
    }
    if record.state == ApprovalState::Expired {
        return Ok(ApprovalTransition {
            record,
            reason_code: ApprovalReasonCode::new("approval.expiry_replayed")?,
            changed: false,
        });
    }

    let reason = match request.action {
        ApprovalAction::Approve => {
            if request.actor.id == record.maker {
                return Err(ApprovalError::SelfApprovalDenied);
            }
            if record
                .votes
                .iter()
                .any(|vote| vote.checker == request.actor.id)
            {
                return Ok(ApprovalTransition {
                    record,
                    reason_code: ApprovalReasonCode::new("approval.vote_replayed")?,
                    changed: false,
                });
            }
            if record.state != ApprovalState::PendingApproval {
                return Err(ApprovalError::InvalidTransition);
            }
            record.votes.push(ApprovalVote {
                checker: request.actor.id,
                approved_at_unix_ms: request.now_unix_ms,
            });
            record.votes.sort_by_key(|vote| vote.checker);
            if record.votes.len() >= usize::from(record.effective_required_quorum()) {
                record.state = ApprovalState::Approved;
            }
            "approval.approved"
        }
        ApprovalAction::Reject => {
            let termination_reason = request
                .reason
                .cloned()
                .unwrap_or(ApprovalReasonCode::new("approval.rejected")?);
            if record.state == ApprovalState::Rejected {
                return terminal_replay(record, termination_reason, "approval.rejection_replayed");
            }
            if record.state != ApprovalState::PendingApproval {
                return Err(ApprovalError::InvalidTransition);
            }
            record.state = ApprovalState::Rejected;
            record.termination_reason = Some(termination_reason);
            "approval.rejected"
        }
        ApprovalAction::Cancel => {
            let termination_reason = request
                .reason
                .cloned()
                .unwrap_or(ApprovalReasonCode::new("approval.cancelled")?);
            if record.state == ApprovalState::Cancelled {
                return terminal_replay(
                    record,
                    termination_reason,
                    "approval.cancellation_replayed",
                );
            }
            if !matches!(
                record.state,
                ApprovalState::Draft | ApprovalState::PendingApproval
            ) {
                return Err(ApprovalError::InvalidTransition);
            }
            record.state = ApprovalState::Cancelled;
            record.termination_reason = Some(termination_reason);
            "approval.cancelled"
        }
        ApprovalAction::Activate => {
            if record.state != ApprovalState::Approved {
                return Err(ApprovalError::InvalidTransition);
            }
            record.state = ApprovalState::Active;
            record.activated_at_unix_ms = Some(request.now_unix_ms);
            "approval.activated"
        }
        ApprovalAction::Supersede => {
            if record.state != ApprovalState::Active {
                return Err(ApprovalError::InvalidTransition);
            }
            record.state = ApprovalState::Superseded;
            "approval.superseded"
        }
        ApprovalAction::RollBack => {
            if record.state != ApprovalState::Active {
                return Err(ApprovalError::InvalidTransition);
            }
            record.state = ApprovalState::RolledBack;
            "approval.rolled_back"
        }
    };
    record.version = record.version.saturating_add(1);
    Ok(ApprovalTransition {
        record,
        reason_code: ApprovalReasonCode::new(reason)?,
        changed: true,
    })
}

fn terminal_replay(
    record: ApprovalRecord,
    reason: ApprovalReasonCode,
    replay_reason: &'static str,
) -> Result<ApprovalTransition, ApprovalError> {
    if record.termination_reason.as_ref() != Some(&reason) {
        return Err(ApprovalError::ReplayMismatch);
    }
    Ok(ApprovalTransition {
        record,
        reason_code: ApprovalReasonCode::new(replay_reason)?,
        changed: false,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalError {
    InvalidToken,
    InvalidQuorum,
    InvalidExpiry,
    TenantMismatch,
    StaleVersion,
    SelfApprovalDenied,
    ReplayMismatch,
    InvalidTransition,
}

impl fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "approval transition is not permitted")
    }
}

impl Error for ApprovalError {}
