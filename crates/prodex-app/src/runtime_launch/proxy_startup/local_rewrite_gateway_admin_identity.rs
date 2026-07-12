//! Runtime-only adapters between stored gateway identities and application policy DTOs.

use prodex_application::{
    ApplicationGatewayIdentityMutationError, ApplicationGatewayIdentityProjection,
    ApplicationGatewayRecordSource, ApplicationGatewayScimUserMutationPlan,
    ApplicationGatewayScimUserRecord, ApplicationGatewayVirtualKeyMutationPlan,
    ApplicationGatewayVirtualKeyRecord, ApplicationSecretMutation,
};
use prodex_domain::{PrincipalId, VirtualKeyId};

use super::local_rewrite_gateway_admin_response::RuntimeGatewayAdminError;
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayScimUser, RuntimeGatewayStoredVirtualKey, RuntimeGatewayVirtualKeyStoreFile,
};

mod patch;

pub(super) use patch::*;

#[cfg(test)]
#[path = "local_rewrite_gateway_admin_identity_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeGatewayIdentityKind {
    VirtualKey,
    ScimUser,
}

pub(super) fn runtime_gateway_virtual_key_record(
    stored: &RuntimeGatewayStoredVirtualKey,
) -> Result<ApplicationGatewayVirtualKeyRecord, RuntimeGatewayAdminError> {
    let id = stored
        .virtual_key_id
        .as_deref()
        .ok_or_else(runtime_gateway_key_id_required)?
        .parse::<VirtualKeyId>()
        .map_err(|_| runtime_gateway_key_id_invalid())?;
    Ok(ApplicationGatewayVirtualKeyRecord {
        id,
        tenant_id: stored.tenant_id.clone(),
        name: stored.name.clone(),
        source: ApplicationGatewayRecordSource::Editable,
        team_id: stored.team_id.clone(),
        project_id: stored.project_id.clone(),
        user_id: stored.user_id.clone(),
        budget_id: stored.budget_id.clone(),
        allowed_models: stored.allowed_models.clone(),
        budget_microusd: stored.budget_microusd,
        request_budget: stored.request_budget,
        rpm_limit: stored.rpm_limit,
        tpm_limit: stored.tpm_limit,
        disabled: stored.disabled.unwrap_or(false),
        created_at_unix_ms: epoch_seconds_to_millis(stored.created_at_epoch),
        updated_at_unix_ms: epoch_seconds_to_millis(stored.updated_at_epoch),
    })
}

pub(super) fn runtime_gateway_scim_user_record(
    stored: &RuntimeGatewayScimUser,
) -> Result<ApplicationGatewayScimUserRecord, RuntimeGatewayAdminError> {
    let id = stored
        .id
        .parse::<PrincipalId>()
        .map_err(|_| runtime_gateway_scim_id_invalid())?;
    Ok(ApplicationGatewayScimUserRecord {
        id,
        tenant_id: stored.tenant_id.clone(),
        user_name: stored.user_name.clone(),
        external_id: stored.external_id.clone(),
        display_name: stored.display_name.clone(),
        active: stored.active,
        role: stored.role.clone(),
        team_id: stored.team_id.clone(),
        project_id: stored.project_id.clone(),
        user_id: stored.user_id.clone(),
        budget_id: stored.budget_id.clone(),
        allowed_key_prefixes: stored.allowed_key_prefixes.clone(),
        created_at_unix_ms: epoch_seconds_to_millis(stored.created_at_epoch),
        updated_at_unix_ms: epoch_seconds_to_millis(stored.updated_at_epoch),
    })
}

pub(super) fn runtime_gateway_apply_virtual_key_projection(
    store: &mut RuntimeGatewayVirtualKeyStoreFile,
    plan: &ApplicationGatewayVirtualKeyMutationPlan,
) -> Result<RuntimeGatewayStoredVirtualKey, RuntimeGatewayAdminError> {
    match &plan.projection {
        ApplicationGatewayIdentityProjection::Insert { record } => {
            let ApplicationSecretMutation::Create(fingerprint) = &plan.secret_mutation else {
                return Err(runtime_gateway_projection_invalid());
            };
            let stored = stored_virtual_key(record, fingerprint.as_str().to_string());
            store.keys.push(stored.clone());
            Ok(stored)
        }
        ApplicationGatewayIdentityProjection::Replace { index, record } => {
            let current = store
                .keys
                .get(*index)
                .ok_or_else(runtime_gateway_projection_invalid)?;
            ensure_key_identity(current, record)?;
            let token_hash_base64 = match &plan.secret_mutation {
                ApplicationSecretMutation::None => current.token_hash_base64.clone(),
                ApplicationSecretMutation::Rotate(fingerprint) => fingerprint.as_str().to_string(),
                ApplicationSecretMutation::Create(_) => {
                    return Err(runtime_gateway_projection_invalid());
                }
            };
            let stored = stored_virtual_key(record, token_hash_base64);
            store.keys[*index] = stored.clone();
            Ok(stored)
        }
        ApplicationGatewayIdentityProjection::Delete { index, record } => {
            let current = store
                .keys
                .get(*index)
                .ok_or_else(runtime_gateway_projection_invalid)?;
            ensure_key_identity(current, record)?;
            if plan.secret_mutation != ApplicationSecretMutation::None {
                return Err(runtime_gateway_projection_invalid());
            }
            Ok(store.keys.remove(*index))
        }
    }
}

pub(super) fn runtime_gateway_apply_scim_user_projection(
    store: &mut RuntimeGatewayVirtualKeyStoreFile,
    plan: &ApplicationGatewayScimUserMutationPlan,
) -> Result<RuntimeGatewayScimUser, RuntimeGatewayAdminError> {
    match &plan.projection {
        ApplicationGatewayIdentityProjection::Insert { record } => {
            let stored = stored_scim_user(record);
            store.scim_users.push(stored.clone());
            Ok(stored)
        }
        ApplicationGatewayIdentityProjection::Replace { index, record } => {
            let current = store
                .scim_users
                .get(*index)
                .ok_or_else(runtime_gateway_projection_invalid)?;
            ensure_scim_identity(current, record)?;
            let stored = stored_scim_user(record);
            store.scim_users[*index] = stored.clone();
            Ok(stored)
        }
        ApplicationGatewayIdentityProjection::Delete { index, record } => {
            let current = store
                .scim_users
                .get(*index)
                .ok_or_else(runtime_gateway_projection_invalid)?;
            ensure_scim_identity(current, record)?;
            Ok(store.scim_users.remove(*index))
        }
    }
}

pub(super) fn runtime_gateway_identity_error(
    kind: RuntimeGatewayIdentityKind,
    error: ApplicationGatewayIdentityMutationError,
) -> RuntimeGatewayAdminError {
    use ApplicationGatewayIdentityMutationError as Error;
    match (kind, error) {
        (RuntimeGatewayIdentityKind::VirtualKey, Error::NotFound) => RuntimeGatewayAdminError::new(
            404,
            "gateway_key_not_found",
            "gateway virtual key was not found",
        ),
        (RuntimeGatewayIdentityKind::ScimUser, Error::NotFound) => RuntimeGatewayAdminError::new(
            404,
            "gateway_scim_user_not_found",
            "gateway SCIM user was not found",
        ),
        (
            RuntimeGatewayIdentityKind::VirtualKey,
            Error::DuplicateIdentity | Error::DuplicateName,
        ) => RuntimeGatewayAdminError::new(
            409,
            "gateway_key_exists",
            "gateway virtual key name already exists",
        ),
        (RuntimeGatewayIdentityKind::ScimUser, Error::DuplicateIdentity | Error::DuplicateName) => {
            RuntimeGatewayAdminError::new(
                409,
                "gateway_scim_user_exists",
                "gateway SCIM userName already exists",
            )
        }
        (RuntimeGatewayIdentityKind::VirtualKey, Error::ReadOnly) => RuntimeGatewayAdminError::new(
            403,
            "gateway_key_read_only",
            "policy-backed gateway virtual keys cannot be edited through the admin API",
        ),
        (
            RuntimeGatewayIdentityKind::VirtualKey,
            Error::GovernanceDenied | Error::TenantMismatch | Error::ResourceIdMismatch,
        ) => RuntimeGatewayAdminError::new(
            403,
            "gateway_admin_key_scope_forbidden",
            "gateway admin token is not allowed to access this virtual key",
        ),
        (
            RuntimeGatewayIdentityKind::ScimUser,
            Error::GovernanceDenied | Error::TenantMismatch | Error::ResourceIdMismatch,
        ) => RuntimeGatewayAdminError::new(
            403,
            "gateway_admin_key_scope_forbidden",
            "gateway admin token is not allowed to access this tenant",
        ),
        (RuntimeGatewayIdentityKind::VirtualKey, Error::InvalidKeyName) => {
            runtime_gateway_invalid_key_name()
        }
        (RuntimeGatewayIdentityKind::VirtualKey, Error::InvalidModel | Error::DuplicateModel) => {
            runtime_gateway_invalid_models()
        }
        (RuntimeGatewayIdentityKind::VirtualKey, Error::InvalidNumericValue) => {
            runtime_gateway_invalid_numeric()
        }
        (
            RuntimeGatewayIdentityKind::ScimUser,
            Error::InvalidScimUserName | Error::RequiredFieldMissing,
        ) => runtime_gateway_invalid_scim_user_name(),
        (RuntimeGatewayIdentityKind::ScimUser, Error::InvalidScimRole) => {
            RuntimeGatewayAdminError::new(400, "invalid_scim_role", "role must be admin or viewer")
        }
        (
            _,
            Error::InvalidScopeValue
            | Error::InvalidScimField
            | Error::InvalidKeyPrefix
            | Error::DuplicateKeyPrefix,
        ) => runtime_gateway_invalid_scim_field(),
        (
            _,
            Error::OperationMismatch
            | Error::ResourceKindMismatch
            | Error::AuditContinuityMismatch
            | Error::InvalidTimestamp
            | Error::InvalidSecretFingerprint,
        ) => runtime_gateway_projection_invalid(),
        (RuntimeGatewayIdentityKind::ScimUser, Error::ReadOnly)
        | (RuntimeGatewayIdentityKind::ScimUser, Error::InvalidKeyName)
        | (RuntimeGatewayIdentityKind::ScimUser, Error::InvalidModel | Error::DuplicateModel)
        | (RuntimeGatewayIdentityKind::ScimUser, Error::InvalidNumericValue)
        | (RuntimeGatewayIdentityKind::VirtualKey, Error::InvalidScimUserName)
        | (RuntimeGatewayIdentityKind::VirtualKey, Error::InvalidScimRole)
        | (RuntimeGatewayIdentityKind::VirtualKey, Error::RequiredFieldMissing) => {
            runtime_gateway_projection_invalid()
        }
    }
}

fn stored_virtual_key(
    record: &ApplicationGatewayVirtualKeyRecord,
    token_hash_base64: String,
) -> RuntimeGatewayStoredVirtualKey {
    RuntimeGatewayStoredVirtualKey {
        name: record.name.clone(),
        token_hash_base64,
        virtual_key_id: Some(record.id.to_string()),
        tenant_id: record.tenant_id.clone(),
        team_id: record.team_id.clone(),
        project_id: record.project_id.clone(),
        user_id: record.user_id.clone(),
        budget_id: record.budget_id.clone(),
        allowed_models: record.allowed_models.clone(),
        budget_microusd: record.budget_microusd,
        request_budget: record.request_budget,
        rpm_limit: record.rpm_limit,
        tpm_limit: record.tpm_limit,
        disabled: Some(record.disabled),
        created_at_epoch: epoch_millis_to_seconds(record.created_at_unix_ms),
        updated_at_epoch: epoch_millis_to_seconds(record.updated_at_unix_ms),
    }
}

fn stored_scim_user(record: &ApplicationGatewayScimUserRecord) -> RuntimeGatewayScimUser {
    RuntimeGatewayScimUser {
        id: record.id.to_string(),
        user_name: record.user_name.clone(),
        external_id: record.external_id.clone(),
        display_name: record.display_name.clone(),
        active: record.active,
        role: record.role.clone(),
        tenant_id: record.tenant_id.clone(),
        team_id: record.team_id.clone(),
        project_id: record.project_id.clone(),
        user_id: record.user_id.clone(),
        budget_id: record.budget_id.clone(),
        allowed_key_prefixes: record.allowed_key_prefixes.clone(),
        created_at_epoch: epoch_millis_to_seconds(record.created_at_unix_ms),
        updated_at_epoch: epoch_millis_to_seconds(record.updated_at_unix_ms),
    }
}

fn ensure_key_identity(
    stored: &RuntimeGatewayStoredVirtualKey,
    record: &ApplicationGatewayVirtualKeyRecord,
) -> Result<(), RuntimeGatewayAdminError> {
    let id = stored
        .virtual_key_id
        .as_deref()
        .ok_or_else(runtime_gateway_key_id_required)?
        .parse::<VirtualKeyId>()
        .map_err(|_| runtime_gateway_key_id_invalid())?;
    if id != record.id || !stored.name.eq_ignore_ascii_case(&record.name) {
        return Err(runtime_gateway_projection_invalid());
    }
    Ok(())
}

fn ensure_scim_identity(
    stored: &RuntimeGatewayScimUser,
    record: &ApplicationGatewayScimUserRecord,
) -> Result<(), RuntimeGatewayAdminError> {
    let id = stored
        .id
        .parse::<PrincipalId>()
        .map_err(|_| runtime_gateway_scim_id_invalid())?;
    if id != record.id {
        return Err(runtime_gateway_projection_invalid());
    }
    Ok(())
}

fn epoch_seconds_to_millis(seconds: u64) -> u64 {
    seconds.saturating_mul(1_000)
}

fn epoch_millis_to_seconds(milliseconds: u64) -> u64 {
    milliseconds / 1_000
}

fn runtime_gateway_key_id_required() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        400,
        "gateway_key_id_required",
        "gateway virtual key id is required for durable key storage",
    )
}

fn runtime_gateway_key_id_invalid() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        400,
        "gateway_key_id_invalid",
        "gateway virtual key id must be a valid virtual key identifier",
    )
}

fn runtime_gateway_scim_id_invalid() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        400,
        "gateway_scim_user_id_invalid",
        "gateway SCIM user user_id must be a valid principal identifier",
    )
}

fn runtime_gateway_projection_invalid() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        500,
        "gateway_key_store_invalid",
        "gateway virtual-key store is invalid",
    )
}

fn runtime_gateway_invalid_key_name() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        400,
        "invalid_gateway_key_name",
        "gateway virtual key name must use 1-128 ASCII letters, numbers, '.', '-', or '_'",
    )
}

fn runtime_gateway_invalid_models() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        400,
        "invalid_allowed_models",
        "allowed_models must be an array of strings",
    )
}

fn runtime_gateway_invalid_numeric() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        400,
        "invalid_numeric_limit",
        "gateway numeric limit must be a u64 or null",
    )
}

fn runtime_gateway_invalid_scim_user_name() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        400,
        "invalid_scim_user_name",
        "SCIM userName must be 1-320 characters without whitespace",
    )
}

fn runtime_gateway_invalid_scim_field() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(400, "invalid_scim_field", "SCIM field is invalid")
}
