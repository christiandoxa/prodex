use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualKeySecretReferenceKind {
    Create,
    Rotate,
}

#[derive(Clone, PartialEq, Eq)]
pub struct VirtualKeySecretReferenceCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub virtual_key_id: VirtualKeyId,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub secret_ref: SecretRef,
    pub kind: VirtualKeySecretReferenceKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for VirtualKeySecretReferenceCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualKeySecretReferenceCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("virtual_key_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("secret_ref", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct VirtualKeySecretReferencePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub virtual_key_id: VirtualKeyId,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub secret_ref: SecretRef,
    pub kind: VirtualKeySecretReferenceKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for VirtualKeySecretReferencePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualKeySecretReferencePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("virtual_key_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("secret_ref", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum VirtualKeySecretReferencePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
    VirtualKeyMismatch {
        key_virtual_key_id: Option<VirtualKeyId>,
        request_virtual_key_id: VirtualKeyId,
    },
}

impl fmt::Debug for VirtualKeySecretReferencePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
            Self::VirtualKeyMismatch { .. } => f
                .debug_struct("VirtualKeyMismatch")
                .field("key_virtual_key_id", &"<redacted>")
                .field("request_virtual_key_id", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for VirtualKeySecretReferencePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => {
                write!(f, "virtual-key secret reference request is invalid")
            }
            Self::VirtualKeyMismatch { .. } => {
                write!(f, "virtual-key secret reference request is invalid")
            }
        }
    }
}

impl Error for VirtualKeySecretReferencePlanError {}

pub fn plan_virtual_key_secret_reference_error_response(
    _error: &VirtualKeySecretReferencePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "virtual_key_secret_reference_rejected",
        message: "virtual-key secret reference request is invalid",
    }
}

pub fn plan_virtual_key_secret_reference(
    command: VirtualKeySecretReferenceCommand,
) -> Result<VirtualKeySecretReferencePlan, VirtualKeySecretReferencePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(VirtualKeySecretReferencePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    if command.storage_key.virtual_key_id != Some(command.virtual_key_id) {
        return Err(VirtualKeySecretReferencePlanError::VirtualKeyMismatch {
            key_virtual_key_id: command.storage_key.virtual_key_id,
            request_virtual_key_id: command.virtual_key_id,
        });
    }
    Ok(VirtualKeySecretReferencePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        virtual_key_id: command.virtual_key_id,
        principal_id: command.principal_id,
        display_name: command.display_name,
        secret_ref: command.secret_ref,
        kind: command.kind,
        occurred_at_unix_ms: command.occurred_at_unix_ms,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderCredentialReferenceCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub provider_credential_id: ProviderCredentialId,
    pub provider_name: String,
    pub secret_ref: SecretRef,
    pub rotated_at_unix_ms: u64,
}

impl fmt::Debug for ProviderCredentialReferenceCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderCredentialReferenceCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("provider_credential_id", &"<redacted>")
            .field("provider_name", &"<redacted>")
            .field("secret_ref", &"<redacted>")
            .field("rotated_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderCredentialReferencePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub provider_credential_id: ProviderCredentialId,
    pub provider_name: String,
    pub secret_ref: SecretRef,
    pub rotated_at_unix_ms: u64,
}

impl fmt::Debug for ProviderCredentialReferencePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderCredentialReferencePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("provider_credential_id", &"<redacted>")
            .field("provider_name", &"<redacted>")
            .field("secret_ref", &"<redacted>")
            .field("rotated_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ProviderCredentialReferencePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
}

impl fmt::Debug for ProviderCredentialReferencePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ProviderCredentialReferencePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => {
                write!(f, "provider credential reference request is invalid")
            }
        }
    }
}

impl Error for ProviderCredentialReferencePlanError {}

pub fn plan_provider_credential_reference_error_response(
    _error: &ProviderCredentialReferencePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "provider_credential_reference_rejected",
        message: "provider credential reference request is invalid",
    }
}

pub fn plan_provider_credential_reference(
    command: ProviderCredentialReferenceCommand,
) -> Result<ProviderCredentialReferencePlan, ProviderCredentialReferencePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(ProviderCredentialReferencePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    Ok(ProviderCredentialReferencePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        provider_credential_id: command.provider_credential_id,
        provider_name: command.provider_name,
        secret_ref: command.secret_ref,
        rotated_at_unix_ms: command.rotated_at_unix_ms,
    })
}
