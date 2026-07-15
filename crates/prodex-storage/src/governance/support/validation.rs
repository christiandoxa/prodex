use crate::{
    GovernanceRepositoryError, GovernanceSessionRevokeCommand, GovernanceSessionUpsertCommand,
};
use prodex_domain::{ApprovalError, CredentialScope, Principal, Role, TenantId};

pub fn validate_governance_session_upsert(
    command: &GovernanceSessionUpsertCommand,
) -> Result<(), GovernanceRepositoryError> {
    if !session_hash_is_valid(&command.session_id_hash)
        || !bounded_session_token(&command.provider_registry_revision)
        || command.provider_descriptor_revision == 0
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

pub fn validate_governance_session_revoke(
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

pub fn require_control_plane_admin(
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

pub fn approval_transition_error(error: ApprovalError) -> GovernanceRepositoryError {
    match error {
        ApprovalError::SelfApprovalDenied => GovernanceRepositoryError::ApprovalSelfAction,
        ApprovalError::StaleVersion => GovernanceRepositoryError::StaleVersion,
        ApprovalError::ReplayMismatch => GovernanceRepositoryError::Conflict,
        ApprovalError::InvalidTransition => GovernanceRepositoryError::InvalidTransition,
        ApprovalError::TenantMismatch => GovernanceRepositoryError::TenantMismatch,
        ApprovalError::InvalidToken
        | ApprovalError::InvalidQuorum
        | ApprovalError::InvalidExpiry => GovernanceRepositoryError::InvalidInput,
    }
}

pub fn to_i64(value: u64) -> Result<i64, GovernanceRepositoryError> {
    i64::try_from(value).map_err(|_| GovernanceRepositoryError::InvalidInput)
}

pub fn from_i64(value: i64) -> Result<u64, GovernanceRepositoryError> {
    u64::try_from(value).map_err(|_| GovernanceRepositoryError::Database)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_integer_conversions_preserve_error_mapping() {
        assert_eq!(to_i64(i64::MAX as u64), Ok(i64::MAX));
        assert_eq!(
            to_i64(i64::MAX as u64 + 1),
            Err(GovernanceRepositoryError::InvalidInput)
        );
        assert_eq!(from_i64(0), Ok(0));
        assert_eq!(from_i64(-1), Err(GovernanceRepositoryError::Database));
    }

    #[test]
    fn session_token_validation_is_bounded_and_lowercase_hash_only() {
        assert!(session_hash_is_valid(&"a".repeat(64)));
        assert!(!session_hash_is_valid(&"A".repeat(64)));
        assert!(bounded_session_token("provider:v1/model"));
        assert!(!bounded_session_token("provider value"));
        assert!(!bounded_session_token(&"x".repeat(129)));
    }
}
