#![forbid(unsafe_code)]
//! Application authorization boundary policies for Prodex.
//!
//! This crate composes pure domain authorization primitives into gateway and
//! control-plane boundary decisions without depending on HTTP, storage, CLI, or
//! provider adapter code.

use std::error::Error;
use std::fmt;

use prodex_domain::{
    AuthorizationRequirement, CredentialScope, Principal, PrincipalKind, ResourceAction,
    ResourceKind, Role, TenantAccessError, TenantContext, TenantScopedResource, authorize_min_role,
    authorize_tenant_access,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryKind {
    DataPlaneInference,
    DataPlaneQuota,
    ControlPlaneAdmin,
    ControlPlaneTenantCreate,
    ControlPlaneTenantUpdate,
    ControlPlaneUserCreate,
    ControlPlaneUserUpdate,
    ControlPlaneUserDelete,
    ControlPlaneRoleBindingGrant,
    ControlPlaneRoleBindingRevoke,
    ControlPlaneServiceIdentityCreate,
    ControlPlaneVirtualKeyCreate,
    ControlPlaneVirtualKeyRotateSecret,
    ControlPlaneProviderCredentialRotate,
    ControlPlaneBudgetUpdate,
    ControlPlanePolicyPublish,
    ControlPlaneConfigurationPublish,
    ControlPlaneBillingRead,
    ControlPlaneAuditExport,
    ControlPlaneAuditRetentionPurge,
    BreakGlassAdmin,
}

impl BoundaryKind {
    pub fn requirement(self) -> AuthorizationRequirement {
        match self {
            Self::DataPlaneInference => AuthorizationRequirement::new(
                ResourceKind::VirtualKey,
                ResourceAction::Read,
                CredentialScope::DataPlane,
                Role::Operator,
            ),
            Self::DataPlaneQuota => AuthorizationRequirement::new(
                ResourceKind::Budget,
                ResourceAction::Read,
                CredentialScope::DataPlane,
                Role::Operator,
            ),
            Self::ControlPlaneAdmin => AuthorizationRequirement::new(
                ResourceKind::Configuration,
                ResourceAction::Update,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneTenantCreate => AuthorizationRequirement::new(
                ResourceKind::Tenant,
                ResourceAction::Create,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneTenantUpdate => AuthorizationRequirement::new(
                ResourceKind::Tenant,
                ResourceAction::Update,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneUserCreate => AuthorizationRequirement::new(
                ResourceKind::User,
                ResourceAction::Create,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneUserUpdate => AuthorizationRequirement::new(
                ResourceKind::User,
                ResourceAction::Update,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneUserDelete => AuthorizationRequirement::new(
                ResourceKind::User,
                ResourceAction::Delete,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneRoleBindingGrant => AuthorizationRequirement::new(
                ResourceKind::RoleBinding,
                ResourceAction::Create,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneRoleBindingRevoke => AuthorizationRequirement::new(
                ResourceKind::RoleBinding,
                ResourceAction::Delete,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneServiceIdentityCreate => AuthorizationRequirement::new(
                ResourceKind::ServiceIdentity,
                ResourceAction::Create,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneVirtualKeyCreate => AuthorizationRequirement::new(
                ResourceKind::VirtualKey,
                ResourceAction::Create,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneVirtualKeyRotateSecret => AuthorizationRequirement::new(
                ResourceKind::VirtualKey,
                ResourceAction::RotateSecret,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneProviderCredentialRotate => AuthorizationRequirement::new(
                ResourceKind::ProviderCredential,
                ResourceAction::RotateSecret,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneBudgetUpdate => AuthorizationRequirement::new(
                ResourceKind::Budget,
                ResourceAction::Update,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlanePolicyPublish => AuthorizationRequirement::new(
                ResourceKind::Policy,
                ResourceAction::PublishRevision,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneConfigurationPublish => AuthorizationRequirement::new(
                ResourceKind::Configuration,
                ResourceAction::PublishRevision,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneBillingRead => AuthorizationRequirement::new(
                ResourceKind::Billing,
                ResourceAction::Read,
                CredentialScope::ControlPlane,
                Role::Viewer,
            ),
            Self::ControlPlaneAuditExport => AuthorizationRequirement::new(
                ResourceKind::AuditLog,
                ResourceAction::Export,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::ControlPlaneAuditRetentionPurge => AuthorizationRequirement::new(
                ResourceKind::AuditLog,
                ResourceAction::Delete,
                CredentialScope::ControlPlane,
                Role::Admin,
            ),
            Self::BreakGlassAdmin => AuthorizationRequirement::new(
                ResourceKind::Configuration,
                ResourceAction::Update,
                CredentialScope::BreakGlass,
                Role::Admin,
            ),
        }
    }
}

pub fn boundary_for_requirement(requirement: AuthorizationRequirement) -> Option<BoundaryKind> {
    control_plane_boundary_for_requirement(requirement)
        .or_else(|| data_plane_boundary_for_requirement(requirement))
        .or_else(|| break_glass_boundary_for_requirement(requirement))
}

pub fn control_plane_boundary_for_requirement(
    requirement: AuthorizationRequirement,
) -> Option<BoundaryKind> {
    if requirement.required_scope != CredentialScope::ControlPlane {
        return None;
    }
    match (
        requirement.resource,
        requirement.action,
        requirement.required_role,
    ) {
        (ResourceKind::Tenant, ResourceAction::Create, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneTenantCreate)
        }
        (ResourceKind::Tenant, ResourceAction::Update, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneTenantUpdate)
        }
        (ResourceKind::User, ResourceAction::Create, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneUserCreate)
        }
        (ResourceKind::User, ResourceAction::Update, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneUserUpdate)
        }
        (ResourceKind::User, ResourceAction::Delete, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneUserDelete)
        }
        (ResourceKind::RoleBinding, ResourceAction::Create, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneRoleBindingGrant)
        }
        (ResourceKind::RoleBinding, ResourceAction::Delete, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneRoleBindingRevoke)
        }
        (ResourceKind::ServiceIdentity, ResourceAction::Create, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneServiceIdentityCreate)
        }
        (ResourceKind::VirtualKey, ResourceAction::Create, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneVirtualKeyCreate)
        }
        (ResourceKind::VirtualKey, ResourceAction::RotateSecret, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneVirtualKeyRotateSecret)
        }
        (ResourceKind::ProviderCredential, ResourceAction::RotateSecret, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneProviderCredentialRotate)
        }
        (ResourceKind::Budget, ResourceAction::Update, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneBudgetUpdate)
        }
        (ResourceKind::Policy, ResourceAction::PublishRevision, Role::Admin) => {
            Some(BoundaryKind::ControlPlanePolicyPublish)
        }
        (ResourceKind::Configuration, ResourceAction::PublishRevision, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneConfigurationPublish)
        }
        (ResourceKind::Billing, ResourceAction::Read, Role::Viewer) => {
            Some(BoundaryKind::ControlPlaneBillingRead)
        }
        (ResourceKind::AuditLog, ResourceAction::Export, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneAuditExport)
        }
        (ResourceKind::AuditLog, ResourceAction::Delete, Role::Admin) => {
            Some(BoundaryKind::ControlPlaneAuditRetentionPurge)
        }
        _ => None,
    }
}

pub fn data_plane_boundary_for_requirement(
    requirement: AuthorizationRequirement,
) -> Option<BoundaryKind> {
    if requirement.required_scope != CredentialScope::DataPlane {
        return None;
    }
    match (
        requirement.resource,
        requirement.action,
        requirement.required_role,
    ) {
        (ResourceKind::VirtualKey, ResourceAction::Read, Role::Operator) => {
            Some(BoundaryKind::DataPlaneInference)
        }
        (ResourceKind::Budget, ResourceAction::Read, Role::Operator) => {
            Some(BoundaryKind::DataPlaneQuota)
        }
        _ => None,
    }
}

pub fn break_glass_boundary_for_requirement(
    requirement: AuthorizationRequirement,
) -> Option<BoundaryKind> {
    if requirement.required_scope != CredentialScope::BreakGlass {
        return None;
    }
    match (
        requirement.resource,
        requirement.action,
        requirement.required_role,
    ) {
        (ResourceKind::Configuration, ResourceAction::Update, Role::Admin) => {
            Some(BoundaryKind::BreakGlassAdmin)
        }
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoundaryAuthorizationError {
    CredentialScopeMismatch {
        boundary: BoundaryKind,
        expected: CredentialScope,
        actual: CredentialScope,
    },
    InsufficientRole {
        boundary: BoundaryKind,
        required: Role,
        actual: Role,
    },
    PrincipalKindMismatch {
        boundary: BoundaryKind,
        expected: PrincipalKind,
        actual: PrincipalKind,
    },
    Tenant(TenantAccessError),
}

impl fmt::Display for BoundaryAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "authorization request is denied")
    }
}

impl Error for BoundaryAuthorizationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorizationErrorStatus {
    Forbidden,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizationErrorResponsePlan {
    pub status: AuthorizationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_authorization_error_response(
    error: &BoundaryAuthorizationError,
) -> AuthorizationErrorResponsePlan {
    match error {
        BoundaryAuthorizationError::CredentialScopeMismatch { .. } => {
            AuthorizationErrorResponsePlan {
                status: AuthorizationErrorStatus::Forbidden,
                code: "credential_scope_not_allowed",
                message: "credential scope is not allowed for this operation",
            }
        }
        BoundaryAuthorizationError::InsufficientRole { .. } => AuthorizationErrorResponsePlan {
            status: AuthorizationErrorStatus::Forbidden,
            code: "role_not_authorized",
            message: "principal role is not authorized for this operation",
        },
        BoundaryAuthorizationError::PrincipalKindMismatch { .. } => {
            AuthorizationErrorResponsePlan {
                status: AuthorizationErrorStatus::Forbidden,
                code: "principal_kind_not_allowed",
                message: "principal kind is not allowed for this operation",
            }
        }
        BoundaryAuthorizationError::Tenant(_) => AuthorizationErrorResponsePlan {
            status: AuthorizationErrorStatus::Forbidden,
            code: "tenant_access_denied",
            message: "tenant access is denied",
        },
    }
}

pub fn authorize_boundary_resource<R>(
    boundary: BoundaryKind,
    principal: &Principal,
    resource: &R,
) -> Result<TenantContext, BoundaryAuthorizationError>
where
    R: TenantScopedResource,
{
    authorize_boundary_scope(boundary, principal)?;
    authorize_boundary_role(boundary, principal)?;
    authorize_tenant_access(principal, resource).map_err(BoundaryAuthorizationError::Tenant)
}

pub fn authorize_boundary_scope(
    boundary: BoundaryKind,
    principal: &Principal,
) -> Result<(), BoundaryAuthorizationError> {
    let requirement = boundary.requirement();
    let exact_scope_match = principal.credential_scope == requirement.required_scope;
    if !exact_scope_match {
        return Err(BoundaryAuthorizationError::CredentialScopeMismatch {
            boundary,
            expected: requirement.required_scope,
            actual: principal.credential_scope,
        });
    }
    if boundary == BoundaryKind::BreakGlassAdmin && principal.kind != PrincipalKind::BreakGlass {
        return Err(BoundaryAuthorizationError::PrincipalKindMismatch {
            boundary,
            expected: PrincipalKind::BreakGlass,
            actual: principal.kind,
        });
    }
    Ok(())
}

pub fn authorize_boundary_role(
    boundary: BoundaryKind,
    principal: &Principal,
) -> Result<(), BoundaryAuthorizationError> {
    let requirement = boundary.requirement();
    authorize_min_role(principal, requirement.required_role).map_err(|_| {
        BoundaryAuthorizationError::InsufficientRole {
            boundary,
            required: requirement.required_role,
            actual: principal.role,
        }
    })
}
