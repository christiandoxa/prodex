//! Tenant-bound maker-checker approval state machine.

use std::error::Error;
use std::fmt;

use serde::Serialize;

use crate::{Principal, PrincipalId, TenantId};

const MAX_APPROVAL_TOKEN_BYTES: usize = 128;
pub const MAX_APPROVAL_QUORUM: u8 = 16;

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
    ProviderRegistryRevision,
    RoutingScoreRevision,
    HighImpactConfiguration,
    Execution,
    BreakGlass,
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
        required_quorum: u8,
        expires_at_unix_ms: u64,
    ) -> Result<Self, ApprovalError> {
        if required_quorum == 0 || required_quorum > MAX_APPROVAL_QUORUM {
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
            required_quorum,
            votes: Vec::new(),
            expires_at_unix_ms,
            activated_at_unix_ms: None,
            version: 1,
        })
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
    let mut record = request.record.clone();
    if request.now_unix_ms >= record.expires_at_unix_ms
        && matches!(
            record.state,
            ApprovalState::Draft | ApprovalState::PendingApproval | ApprovalState::Approved
        )
    {
        record.state = ApprovalState::Expired;
        record.version = record.version.saturating_add(1);
        return Ok(ApprovalTransition {
            record,
            reason_code: ApprovalReasonCode::new("approval.expired")?,
            changed: true,
        });
    }

    let reason = match request.action {
        ApprovalAction::Approve => {
            if record.state != ApprovalState::PendingApproval {
                return Err(ApprovalError::InvalidTransition);
            }
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
            record.votes.push(ApprovalVote {
                checker: request.actor.id,
                approved_at_unix_ms: request.now_unix_ms,
            });
            record.votes.sort_by_key(|vote| vote.checker);
            if record.votes.len() >= usize::from(record.required_quorum) {
                record.state = ApprovalState::Approved;
            }
            "approval.approved"
        }
        ApprovalAction::Reject => {
            if record.state != ApprovalState::PendingApproval {
                return Err(ApprovalError::InvalidTransition);
            }
            record.state = ApprovalState::Rejected;
            "approval.rejected"
        }
        ApprovalAction::Cancel => {
            if !matches!(
                record.state,
                ApprovalState::Draft | ApprovalState::PendingApproval
            ) {
                return Err(ApprovalError::InvalidTransition);
            }
            record.state = ApprovalState::Cancelled;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalError {
    InvalidToken,
    InvalidQuorum,
    InvalidExpiry,
    TenantMismatch,
    StaleVersion,
    SelfApprovalDenied,
    InvalidTransition,
}

impl fmt::Display for ApprovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "approval transition is not permitted")
    }
}

impl Error for ApprovalError {}
