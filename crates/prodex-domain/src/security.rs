use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{PrincipalId, TenantId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Viewer,
    Operator,
    Admin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialScope {
    DataPlane,
    ControlPlane,
    BreakGlass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalKind {
    User,
    ServiceAccount,
    VirtualKey,
    BreakGlass,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    pub id: PrincipalId,
    pub tenant_id: Option<TenantId>,
    pub kind: PrincipalKind,
    pub role: Role,
    pub credential_scope: CredentialScope,
}

impl fmt::Debug for Principal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Principal")
            .field("id", &"<redacted>")
            .field("tenant_id", &self.tenant_id.map(|_| "<redacted>"))
            .field("kind", &self.kind)
            .field("role", &self.role)
            .field("credential_scope", &self.credential_scope)
            .finish()
    }
}

impl Principal {
    pub fn new(
        id: PrincipalId,
        tenant_id: Option<TenantId>,
        kind: PrincipalKind,
        role: Role,
        credential_scope: CredentialScope,
    ) -> Self {
        Self {
            id,
            tenant_id,
            kind,
            role,
            credential_scope,
        }
    }

    pub fn tenant_context(&self, mode: TenantMode) -> Result<TenantContext, TenantResolutionError> {
        TenantContext::resolve(self.tenant_id, mode)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantMode {
    SingleTenant,
    MultiTenant,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantContext {
    pub tenant_id: TenantId,
}

impl fmt::Debug for TenantContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TenantContext")
            .field("tenant_id", &"<redacted>")
            .finish()
    }
}

impl TenantContext {
    pub fn resolve(
        tenant_id: Option<TenantId>,
        mode: TenantMode,
    ) -> Result<Self, TenantResolutionError> {
        match (mode, tenant_id) {
            (_, Some(tenant_id)) => Ok(Self { tenant_id }),
            (TenantMode::SingleTenant, None) => Err(TenantResolutionError::MissingTenant),
            (TenantMode::MultiTenant, None) => Err(TenantResolutionError::MissingTenant),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TenantResolutionError {
    MissingTenant,
}

impl fmt::Display for TenantResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tenant context is required")
    }
}

impl Error for TenantResolutionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityErrorStatus {
    BadRequest,
    Forbidden,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityErrorResponsePlan {
    pub status: SecurityErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_tenant_resolution_error_response(
    _error: &TenantResolutionError,
) -> SecurityErrorResponsePlan {
    SecurityErrorResponsePlan {
        status: SecurityErrorStatus::BadRequest,
        code: "tenant_required",
        message: "tenant context is required",
    }
}

pub trait TenantScopedResource {
    fn tenant_id(&self) -> TenantId;
}

#[derive(Clone, PartialEq, Eq)]
pub enum TenantAccessError {
    PrincipalMissingTenant,
    CrossTenantAccess {
        principal_tenant_id: TenantId,
        resource_tenant_id: TenantId,
    },
}

impl fmt::Display for TenantAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tenant access is denied")
    }
}

impl fmt::Debug for TenantAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrincipalMissingTenant => f.write_str("PrincipalMissingTenant"),
            Self::CrossTenantAccess { .. } => f.write_str("CrossTenantAccess"),
        }
    }
}

impl Error for TenantAccessError {}

pub fn plan_tenant_access_error_response(_error: &TenantAccessError) -> SecurityErrorResponsePlan {
    SecurityErrorResponsePlan {
        status: SecurityErrorStatus::Forbidden,
        code: "tenant_access_denied",
        message: "tenant access is denied",
    }
}

pub fn authorize_tenant_access<R>(
    principal: &Principal,
    resource: &R,
) -> Result<TenantContext, TenantAccessError>
where
    R: TenantScopedResource,
{
    let principal_tenant_id = principal
        .tenant_id
        .ok_or(TenantAccessError::PrincipalMissingTenant)?;
    let resource_tenant_id = resource.tenant_id();
    if principal_tenant_id == resource_tenant_id {
        Ok(TenantContext {
            tenant_id: principal_tenant_id,
        })
    } else {
        Err(TenantAccessError::CrossTenantAccess {
            principal_tenant_id,
            resource_tenant_id,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuthorizationError {
    CredentialScopeMismatch {
        expected: CredentialScope,
        actual: CredentialScope,
    },
    InsufficientRole {
        required: Role,
        actual: Role,
    },
}

impl fmt::Debug for AuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CredentialScopeMismatch { .. } => f
                .debug_struct("CredentialScopeMismatch")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::InsufficientRole { .. } => f
                .debug_struct("InsufficientRole")
                .field("required", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "authorization request is denied")
    }
}

impl Error for AuthorizationError {}

pub fn plan_domain_authorization_error_response(
    error: &AuthorizationError,
) -> SecurityErrorResponsePlan {
    match error {
        AuthorizationError::CredentialScopeMismatch { .. } => SecurityErrorResponsePlan {
            status: SecurityErrorStatus::Forbidden,
            code: "credential_scope_denied",
            message: "credential scope is not allowed",
        },
        AuthorizationError::InsufficientRole { .. } => SecurityErrorResponsePlan {
            status: SecurityErrorStatus::Forbidden,
            code: "role_denied",
            message: "principal role is not sufficient",
        },
    }
}

pub fn authorize_scope(
    principal: &Principal,
    expected_scope: CredentialScope,
) -> Result<(), AuthorizationError> {
    if principal.credential_scope == expected_scope {
        Ok(())
    } else {
        Err(AuthorizationError::CredentialScopeMismatch {
            expected: expected_scope,
            actual: principal.credential_scope,
        })
    }
}

pub fn authorize_min_role(principal: &Principal, required: Role) -> Result<(), AuthorizationError> {
    if principal.role >= required {
        Ok(())
    } else {
        Err(AuthorizationError::InsufficientRole {
            required,
            actual: principal.role,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoleClaimError {
    Missing,
    Unknown,
}

impl fmt::Display for RoleClaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "role claim is not authorized")
    }
}

impl Error for RoleClaimError {}

pub fn plan_role_claim_error_response(_error: &RoleClaimError) -> SecurityErrorResponsePlan {
    SecurityErrorResponsePlan {
        status: SecurityErrorStatus::Forbidden,
        code: "role_claim_denied",
        message: "role claim is not authorized",
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct ExplicitRoleMapper {
    mappings: BTreeMap<String, Role>,
}

impl fmt::Debug for ExplicitRoleMapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExplicitRoleMapper")
            .field("mappings", &"<redacted>")
            .finish()
    }
}

impl ExplicitRoleMapper {
    pub fn new<I, S>(mappings: I) -> Self
    where
        I: IntoIterator<Item = (S, Role)>,
        S: Into<String>,
    {
        Self {
            mappings: mappings
                .into_iter()
                .filter_map(|(claim, role)| {
                    let claim = claim.into();
                    (!claim.is_empty() && !claim.chars().any(char::is_whitespace))
                        .then_some((claim, role))
                })
                .collect(),
        }
    }

    pub fn role_for_claim(&self, role_claim: Option<&str>) -> Result<Role, RoleClaimError> {
        let role_claim = role_claim.ok_or(RoleClaimError::Missing)?;
        if role_claim.is_empty() {
            return Err(RoleClaimError::Missing);
        }
        if role_claim.chars().any(char::is_whitespace) {
            return Err(RoleClaimError::Unknown);
        }
        self.mappings
            .get(role_claim)
            .copied()
            .ok_or(RoleClaimError::Unknown)
    }

    pub fn viewer_for_missing_or_unknown(&self, role_claim: Option<&str>) -> Role {
        self.role_for_claim(role_claim).unwrap_or(Role::Viewer)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Tenant,
    User,
    RoleBinding,
    ServiceIdentity,
    VirtualKey,
    Policy,
    ProviderCredential,
    Budget,
    Billing,
    AuditLog,
    Configuration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAction {
    Read,
    Create,
    Update,
    Delete,
    RotateSecret,
    Export,
    PublishRevision,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationRequirement {
    pub resource: ResourceKind,
    pub action: ResourceAction,
    pub required_scope: CredentialScope,
    pub required_role: Role,
}

impl fmt::Debug for AuthorizationRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthorizationRequirement")
            .field("resource", &"<redacted>")
            .field("action", &"<redacted>")
            .field("required_scope", &"<redacted>")
            .field("required_role", &"<redacted>")
            .finish()
    }
}

impl AuthorizationRequirement {
    pub fn new(
        resource: ResourceKind,
        action: ResourceAction,
        required_scope: CredentialScope,
        required_role: Role,
    ) -> Self {
        Self {
            resource,
            action,
            required_scope,
            required_role,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ResourceAuthorizationError {
    Scope(AuthorizationError),
    Role(AuthorizationError),
    Tenant(TenantAccessError),
}

impl fmt::Display for ResourceAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "resource authorization request is denied")
    }
}

impl fmt::Debug for ResourceAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(err) => f.debug_tuple("Scope").field(err).finish(),
            Self::Role(err) => f.debug_tuple("Role").field(err).finish(),
            Self::Tenant(_) => f.write_str("Tenant"),
        }
    }
}

impl Error for ResourceAuthorizationError {}

pub fn plan_resource_authorization_error_response(
    error: &ResourceAuthorizationError,
) -> SecurityErrorResponsePlan {
    match error {
        ResourceAuthorizationError::Scope(error) | ResourceAuthorizationError::Role(error) => {
            plan_domain_authorization_error_response(error)
        }
        ResourceAuthorizationError::Tenant(error) => plan_tenant_access_error_response(error),
    }
}

pub fn authorize_resource_action<R>(
    principal: &Principal,
    requirement: AuthorizationRequirement,
    resource: &R,
) -> Result<TenantContext, ResourceAuthorizationError>
where
    R: TenantScopedResource,
{
    authorize_scope(principal, requirement.required_scope)
        .map_err(ResourceAuthorizationError::Scope)?;
    authorize_min_role(principal, requirement.required_role)
        .map_err(ResourceAuthorizationError::Role)?;
    authorize_tenant_access(principal, resource).map_err(ResourceAuthorizationError::Tenant)
}
