use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleBindingMutationKind {
    Grant,
    Revoke,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RoleBindingMutationCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub role_binding_id: RoleBindingId,
    pub principal_id: PrincipalId,
    pub role: Role,
    pub kind: RoleBindingMutationKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for RoleBindingMutationCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RoleBindingMutationCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("role_binding_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("role", &self.role)
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RoleBindingMutationPlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub role_binding_id: RoleBindingId,
    pub principal_id: PrincipalId,
    pub role: Role,
    pub kind: RoleBindingMutationKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for RoleBindingMutationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RoleBindingMutationPlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("role_binding_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("role", &self.role)
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum RoleBindingMutationPlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
}

impl fmt::Debug for RoleBindingMutationPlanError {
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

impl fmt::Display for RoleBindingMutationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "role-binding mutation request is invalid"),
        }
    }
}

impl Error for RoleBindingMutationPlanError {}

pub fn plan_role_binding_mutation_error_response(
    _error: &RoleBindingMutationPlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "role_binding_mutation_rejected",
        message: "role-binding mutation request is invalid",
    }
}

pub fn plan_role_binding_mutation(
    command: RoleBindingMutationCommand,
) -> Result<RoleBindingMutationPlan, RoleBindingMutationPlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(RoleBindingMutationPlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    Ok(RoleBindingMutationPlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        role_binding_id: command.role_binding_id,
        principal_id: command.principal_id,
        role: command.role,
        kind: command.kind,
        occurred_at_unix_ms: command.occurred_at_unix_ms,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ServiceIdentityCreateCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub created_at_unix_ms: u64,
}

impl fmt::Debug for ServiceIdentityCreateCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceIdentityCreateCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("created_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ServiceIdentityCreatePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub created_at_unix_ms: u64,
}

impl fmt::Debug for ServiceIdentityCreatePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceIdentityCreatePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("created_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ServiceIdentityCreatePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
}

impl fmt::Debug for ServiceIdentityCreatePlanError {
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

impl fmt::Display for ServiceIdentityCreatePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => write!(f, "service identity create request is invalid"),
        }
    }
}

impl Error for ServiceIdentityCreatePlanError {}

pub fn plan_service_identity_create_error_response(
    _error: &ServiceIdentityCreatePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "service_identity_create_rejected",
        message: "service identity create request is invalid",
    }
}

pub fn plan_service_identity_create(
    command: ServiceIdentityCreateCommand,
) -> Result<ServiceIdentityCreatePlan, ServiceIdentityCreatePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(ServiceIdentityCreatePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    Ok(ServiceIdentityCreatePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        principal_id: command.principal_id,
        display_name: command.display_name,
        created_at_unix_ms: command.created_at_unix_ms,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserLifecycleKind {
    Create,
    Update,
    Delete,
}

#[derive(Clone, PartialEq, Eq)]
pub struct UserLifecycleCommand {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub principal_id: PrincipalId,
    pub external_id: String,
    pub display_name: String,
    pub kind: UserLifecycleKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for UserLifecycleCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserLifecycleCommand")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("external_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct UserLifecyclePlan {
    pub storage_key: TenantStorageKey,
    pub tenant_id: TenantId,
    pub principal_id: PrincipalId,
    pub external_id: String,
    pub display_name: String,
    pub kind: UserLifecycleKind,
    pub occurred_at_unix_ms: u64,
}

impl fmt::Debug for UserLifecyclePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserLifecyclePlan")
            .field("storage_key", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("external_id", &"<redacted>")
            .field("display_name", &"<redacted>")
            .field("kind", &self.kind)
            .field("occurred_at_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum UserLifecyclePlanError {
    TenantMismatch {
        key_tenant: TenantId,
        request_tenant: TenantId,
    },
    EmptyExternalId,
    EmptyDisplayName,
}

impl fmt::Debug for UserLifecyclePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } => f
                .debug_struct("TenantMismatch")
                .field("key_tenant", &"<redacted>")
                .field("request_tenant", &"<redacted>")
                .finish(),
            Self::EmptyExternalId => f.write_str("EmptyExternalId"),
            Self::EmptyDisplayName => f.write_str("EmptyDisplayName"),
        }
    }
}

impl fmt::Display for UserLifecyclePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TenantMismatch { .. } | Self::EmptyExternalId | Self::EmptyDisplayName => {
                write!(f, "user lifecycle request is invalid")
            }
        }
    }
}

impl Error for UserLifecyclePlanError {}

pub fn plan_user_lifecycle_error_response(
    _error: &UserLifecyclePlanError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::BadRequest,
        code: "user_lifecycle_rejected",
        message: "user lifecycle request is invalid",
    }
}

pub fn plan_user_lifecycle(
    command: UserLifecycleCommand,
) -> Result<UserLifecyclePlan, UserLifecyclePlanError> {
    if command.storage_key.tenant_id != command.tenant_id {
        return Err(UserLifecyclePlanError::TenantMismatch {
            key_tenant: command.storage_key.tenant_id,
            request_tenant: command.tenant_id,
        });
    }
    if command.external_id.trim().is_empty() {
        return Err(UserLifecyclePlanError::EmptyExternalId);
    }
    if command.kind != UserLifecycleKind::Delete && command.display_name.trim().is_empty() {
        return Err(UserLifecyclePlanError::EmptyDisplayName);
    }
    Ok(UserLifecyclePlan {
        storage_key: command.storage_key,
        tenant_id: command.tenant_id,
        principal_id: command.principal_id,
        external_id: command.external_id,
        display_name: command.display_name,
        kind: command.kind,
        occurred_at_unix_ms: command.occurred_at_unix_ms,
    })
}
