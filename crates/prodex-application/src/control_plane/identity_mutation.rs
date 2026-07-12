//! Pure gateway virtual-key and SCIM-user mutation policy.

use std::error::Error;
use std::fmt;

use prodex_control_plane::{ControlPlaneActionPlan, ControlPlaneOperation};
use prodex_domain::{AuditOutcome, ResourceKind};

use super::ApplicationControlPlaneGovernanceScope;

mod scim_user;
mod virtual_key;

pub use scim_user::*;
pub use virtual_key::*;

#[cfg(test)]
mod scim_user_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod virtual_key_tests;

#[derive(Clone, Default, PartialEq, Eq)]
pub enum ApplicationPatchValue<T> {
    #[default]
    Unchanged,
    Set(T),
    Clear,
}

impl<T> fmt::Debug for ApplicationPatchValue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unchanged => "Unchanged",
            Self::Set(_) => "Set(<redacted>)",
            Self::Clear => "Clear",
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationGatewayIdentityProjection<T> {
    Insert { record: T },
    Replace { index: usize, record: T },
    Delete { index: usize, record: T },
}

impl<T> fmt::Debug for ApplicationGatewayIdentityProjection<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Insert { .. } => f.write_str("Insert(<redacted>)"),
            Self::Replace { index, .. } => f
                .debug_struct("Replace")
                .field("index", index)
                .field("record", &"<redacted>")
                .finish(),
            Self::Delete { index, .. } => f
                .debug_struct("Delete")
                .field("index", index)
                .field("record", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationSecretFingerprint(String);

impl ApplicationSecretFingerprint {
    pub fn new(value: impl Into<String>) -> Result<Self, ApplicationGatewayIdentityMutationError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 512
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(ApplicationGatewayIdentityMutationError::InvalidSecretFingerprint);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApplicationSecretFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ApplicationSecretFingerprint")
            .field(&"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationSecretMutation {
    None,
    Create(ApplicationSecretFingerprint),
    Rotate(ApplicationSecretFingerprint),
}

impl fmt::Debug for ApplicationSecretMutation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "None",
            Self::Create(_) => "Create(<redacted>)",
            Self::Rotate(_) => "Rotate(<redacted>)",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationGatewayRecordSource {
    Editable,
    ReadOnly,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ApplicationGatewayIdentityMutationError {
    OperationMismatch,
    ResourceKindMismatch,
    TenantMismatch,
    AuditContinuityMismatch,
    ResourceIdMismatch,
    NotFound,
    DuplicateIdentity,
    DuplicateName,
    GovernanceDenied,
    ReadOnly,
    InvalidKeyName,
    InvalidModel,
    DuplicateModel,
    InvalidNumericValue,
    InvalidScopeValue,
    InvalidTimestamp,
    InvalidSecretFingerprint,
    InvalidScimUserName,
    InvalidScimRole,
    InvalidScimField,
    InvalidKeyPrefix,
    DuplicateKeyPrefix,
    RequiredFieldMissing,
}

impl fmt::Debug for ApplicationGatewayIdentityMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OperationMismatch => "OperationMismatch",
            Self::ResourceKindMismatch => "ResourceKindMismatch",
            Self::TenantMismatch => "TenantMismatch",
            Self::AuditContinuityMismatch => "AuditContinuityMismatch",
            Self::ResourceIdMismatch => "ResourceIdMismatch",
            Self::NotFound => "NotFound",
            Self::DuplicateIdentity => "DuplicateIdentity",
            Self::DuplicateName => "DuplicateName",
            Self::GovernanceDenied => "GovernanceDenied",
            Self::ReadOnly => "ReadOnly",
            Self::InvalidKeyName => "InvalidKeyName",
            Self::InvalidModel => "InvalidModel",
            Self::DuplicateModel => "DuplicateModel",
            Self::InvalidNumericValue => "InvalidNumericValue",
            Self::InvalidScopeValue => "InvalidScopeValue",
            Self::InvalidTimestamp => "InvalidTimestamp",
            Self::InvalidSecretFingerprint => "InvalidSecretFingerprint",
            Self::InvalidScimUserName => "InvalidScimUserName",
            Self::InvalidScimRole => "InvalidScimRole",
            Self::InvalidScimField => "InvalidScimField",
            Self::InvalidKeyPrefix => "InvalidKeyPrefix",
            Self::DuplicateKeyPrefix => "DuplicateKeyPrefix",
            Self::RequiredFieldMissing => "RequiredFieldMissing",
        })
    }
}

impl fmt::Display for ApplicationGatewayIdentityMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gateway identity mutation is invalid")
    }
}

impl Error for ApplicationGatewayIdentityMutationError {}

fn validate_action(
    action: &ControlPlaneActionPlan,
    operation: ControlPlaneOperation,
    resource_kind: ResourceKind,
    resource_label: &str,
    resource_id: &str,
    collection_resource_allowed: bool,
) -> Result<ControlPlaneActionPlan, ApplicationGatewayIdentityMutationError> {
    if action.operation != operation || action.requirement != operation.requirement() {
        return Err(ApplicationGatewayIdentityMutationError::OperationMismatch);
    }
    if action.requirement.resource != resource_kind {
        return Err(ApplicationGatewayIdentityMutationError::ResourceKindMismatch);
    }
    let tenant_id = action.tenant.tenant_id;
    if action.audit_write.tenant_partition_key != tenant_id
        || action.audit_event.tenant_id != tenant_id
        || action.audit_event.resource.tenant_id != Some(tenant_id)
    {
        return Err(ApplicationGatewayIdentityMutationError::TenantMismatch);
    }
    if action.audit_write.event != action.audit_event
        || action.audit_event.action != operation.audit_action()
        || action.audit_event.outcome != AuditOutcome::Success
        || action.audit_event.resource.kind != resource_label
    {
        return Err(ApplicationGatewayIdentityMutationError::AuditContinuityMismatch);
    }
    match action.audit_event.resource.id.as_deref() {
        Some(id) if id == resource_id => {}
        None if collection_resource_allowed => {}
        _ => return Err(ApplicationGatewayIdentityMutationError::ResourceIdMismatch),
    }
    Ok(action.clone())
}

fn validate_scope_value(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(|value| {
        !value.is_empty()
            && value.len() <= 128
            && !value
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
    })
}

fn validate_optional_text(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(|value| {
        !value.is_empty()
            && value.len() <= 512
            && value.trim() == value
            && !value.chars().any(char::is_control)
    })
}

fn timestamp_is_valid(created_at_unix_ms: u64, updated_at_unix_ms: u64) -> bool {
    created_at_unix_ms > 0 && updated_at_unix_ms >= created_at_unix_ms
}
