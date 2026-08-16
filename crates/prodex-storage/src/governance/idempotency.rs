use super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceMutationIdempotency {
    pub operation: IdempotentOperation,
    pub started_at_unix_ms: u64,
}

impl fmt::Debug for GovernanceMutationIdempotency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceMutationIdempotency")
            .field("operation", &"<redacted>")
            .field("started_at_unix_ms", &"<redacted>")
            .finish()
    }
}

pub type ApprovalVoteIdempotency = GovernanceMutationIdempotency;

pub const GOVERNANCE_REVISION_WRITE_IDEMPOTENCY_RESPONSE: &[u8] =
    b"v1|governance_revision_write|ok";
pub const GOVERNANCE_APPROVAL_CREATE_IDEMPOTENCY_RESPONSE: &[u8] =
    b"v1|governance_approval_create|ok";
pub const GOVERNANCE_SESSION_REVOKE_IDEMPOTENCY_RESPONSE: &[u8] =
    b"v1|governance_session_revoke|ok";
pub const GOVERNANCE_AUDIT_LEGAL_HOLD_UPSERT_IDEMPOTENCY_RESPONSE: &[u8] =
    b"v1|governance_audit_legal_hold_upsert|ok";
pub const GOVERNANCE_AUDIT_LEGAL_HOLD_DELETE_APPLIED_IDEMPOTENCY_RESPONSE: &[u8] =
    b"v1|governance_audit_legal_hold_delete|applied";
pub const GOVERNANCE_AUDIT_LEGAL_HOLD_DELETE_NOT_FOUND_IDEMPOTENCY_RESPONSE: &[u8] =
    b"v1|governance_audit_legal_hold_delete|not_found";

const GOVERNANCE_AUDIT_RETENTION_PURGE_IDEMPOTENCY_PREFIX: &str =
    "v1|governance_audit_retention_purge|";
const GOVERNANCE_AUDIT_RETENTION_PURGE_IDEMPOTENCY_MAX_BYTES: usize = 40 * 1024;

pub fn encode_governance_audit_retention_purge_idempotency_response(
    event_ids: &[AuditEventId],
) -> Vec<u8> {
    format!(
        "{GOVERNANCE_AUDIT_RETENTION_PURGE_IDEMPOTENCY_PREFIX}{}",
        event_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
    .into_bytes()
}

pub fn decode_governance_audit_retention_purge_idempotency_response(
    response: &[u8],
) -> Result<Vec<AuditEventId>, GovernanceRepositoryError> {
    if response.len() > GOVERNANCE_AUDIT_RETENTION_PURGE_IDEMPOTENCY_MAX_BYTES {
        return Err(GovernanceRepositoryError::InvalidInput);
    }
    let response = std::str::from_utf8(response)
        .map_err(|_| GovernanceRepositoryError::InvalidInput)?
        .strip_prefix(GOVERNANCE_AUDIT_RETENTION_PURGE_IDEMPOTENCY_PREFIX)
        .ok_or(GovernanceRepositoryError::InvalidInput)?;
    if response.is_empty() {
        return Ok(Vec::new());
    }
    response
        .split(',')
        .map(|event_id| {
            event_id
                .parse()
                .map_err(|_| GovernanceRepositoryError::InvalidInput)
        })
        .collect()
}
