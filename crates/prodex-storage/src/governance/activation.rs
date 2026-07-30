use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceActivationAction {
    Activate,
    Rollback,
    Revoke,
}

impl GovernanceActivationAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Activate => "activate",
            Self::Rollback => "rollback",
            Self::Revoke => "revoke",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceActivationRequest {
    pub tenant_id: TenantId,
    pub kind: GovernanceArtifactKind,
    pub revision_id: String,
    pub approval_id: Option<ApprovalId>,
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
            .field(
                "approval_id",
                &self.approval_id.as_ref().map(|_| "<redacted>"),
            )
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
    pub active_revision_id: Option<String>,
    pub last_known_good_revision_id: Option<String>,
}
