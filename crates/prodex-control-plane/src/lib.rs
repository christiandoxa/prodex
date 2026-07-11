#![forbid(unsafe_code)]
//! HTTP-neutral control-plane authorization and audit planning.
//!
//! This crate keeps admin, tenant, identity, key, billing, audit, and
//! configuration-management decisions out of the data-plane gateway and out of
//! HTTP/storage adapters. It has no framework, filesystem, network, database,
//! async runtime, or provider-SDK dependency.

use std::error::Error;
use std::fmt;

use prodex_config::{
    ConfigPublicationError, ConfigPublicationErrorStatus, ConfigRevision,
    plan_config_publication_error_response, validate_config_publication,
};
use prodex_domain::{
    AuditAction, AuditEvent, AuditOutcome, AuditResource, AuthorizationError, CredentialScope,
    IdempotencyKey, IdempotentOperation, IdempotentOperationError, PolicyRevisionId, Principal,
    ResourceAction, ResourceKind, Role, TenantAccessError, TenantContext, TenantId,
    TenantScopedResource, authorize_min_role, authorize_tenant_access,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlPlaneOperation {
    GatewayAdminRead,
    RouteExplain,
    TenantCreate,
    TenantUpdate,
    UserInvite,
    ScimUserRead,
    ScimUserCreate,
    ScimUserUpdate,
    ScimUserDelete,
    RoleBindingGrant,
    RoleBindingRevoke,
    ServiceIdentityCreate,
    VirtualKeyRead,
    VirtualKeyCreate,
    VirtualKeyUpdate,
    VirtualKeyDelete,
    VirtualKeyRotateSecret,
    PolicyPublish,
    ProviderCredentialRotate,
    BudgetUpdate,
    BillingRead,
    AuditExport,
    AuditRetentionPurge,
    ConfigurationPublish,
}

impl ControlPlaneOperation {
    pub const ALL: [Self; 24] = [
        Self::GatewayAdminRead,
        Self::RouteExplain,
        Self::TenantCreate,
        Self::TenantUpdate,
        Self::UserInvite,
        Self::ScimUserRead,
        Self::ScimUserCreate,
        Self::ScimUserUpdate,
        Self::ScimUserDelete,
        Self::RoleBindingGrant,
        Self::RoleBindingRevoke,
        Self::ServiceIdentityCreate,
        Self::VirtualKeyRead,
        Self::VirtualKeyCreate,
        Self::VirtualKeyUpdate,
        Self::VirtualKeyDelete,
        Self::VirtualKeyRotateSecret,
        Self::PolicyPublish,
        Self::ProviderCredentialRotate,
        Self::BudgetUpdate,
        Self::BillingRead,
        Self::AuditExport,
        Self::AuditRetentionPurge,
        Self::ConfigurationPublish,
    ];

    pub fn requirement(self) -> ControlPlaneRequirement {
        match self {
            Self::GatewayAdminRead => {
                ControlPlaneRequirement::viewer(ResourceKind::Configuration, ResourceAction::Read)
            }
            Self::RouteExplain => {
                ControlPlaneRequirement::viewer(ResourceKind::Configuration, ResourceAction::Read)
            }
            Self::TenantCreate => {
                ControlPlaneRequirement::admin(ResourceKind::Tenant, ResourceAction::Create)
            }
            Self::TenantUpdate => {
                ControlPlaneRequirement::admin(ResourceKind::Tenant, ResourceAction::Update)
            }
            Self::UserInvite => {
                ControlPlaneRequirement::admin(ResourceKind::User, ResourceAction::Create)
            }
            Self::ScimUserRead => {
                ControlPlaneRequirement::viewer(ResourceKind::User, ResourceAction::Read)
            }
            Self::ScimUserCreate => {
                ControlPlaneRequirement::admin(ResourceKind::User, ResourceAction::Create)
            }
            Self::ScimUserUpdate => {
                ControlPlaneRequirement::admin(ResourceKind::User, ResourceAction::Update)
            }
            Self::ScimUserDelete => {
                ControlPlaneRequirement::admin(ResourceKind::User, ResourceAction::Delete)
            }
            Self::RoleBindingGrant => {
                ControlPlaneRequirement::admin(ResourceKind::RoleBinding, ResourceAction::Create)
            }
            Self::RoleBindingRevoke => {
                ControlPlaneRequirement::admin(ResourceKind::RoleBinding, ResourceAction::Delete)
            }
            Self::ServiceIdentityCreate => ControlPlaneRequirement::admin(
                ResourceKind::ServiceIdentity,
                ResourceAction::Create,
            ),
            Self::VirtualKeyRead => {
                ControlPlaneRequirement::viewer(ResourceKind::VirtualKey, ResourceAction::Read)
            }
            Self::VirtualKeyCreate => {
                ControlPlaneRequirement::admin(ResourceKind::VirtualKey, ResourceAction::Create)
            }
            Self::VirtualKeyUpdate => {
                ControlPlaneRequirement::admin(ResourceKind::VirtualKey, ResourceAction::Update)
            }
            Self::VirtualKeyDelete => {
                ControlPlaneRequirement::admin(ResourceKind::VirtualKey, ResourceAction::Delete)
            }
            Self::VirtualKeyRotateSecret => ControlPlaneRequirement::admin(
                ResourceKind::VirtualKey,
                ResourceAction::RotateSecret,
            ),
            Self::PolicyPublish => ControlPlaneRequirement::admin(
                ResourceKind::Policy,
                ResourceAction::PublishRevision,
            ),
            Self::ProviderCredentialRotate => ControlPlaneRequirement::admin(
                ResourceKind::ProviderCredential,
                ResourceAction::RotateSecret,
            ),
            Self::BudgetUpdate => {
                ControlPlaneRequirement::admin(ResourceKind::Budget, ResourceAction::Update)
            }
            Self::BillingRead => {
                ControlPlaneRequirement::viewer(ResourceKind::Billing, ResourceAction::Read)
            }
            Self::AuditExport => {
                ControlPlaneRequirement::admin(ResourceKind::AuditLog, ResourceAction::Export)
            }
            Self::AuditRetentionPurge => {
                ControlPlaneRequirement::admin(ResourceKind::AuditLog, ResourceAction::Delete)
            }
            Self::ConfigurationPublish => ControlPlaneRequirement::admin(
                ResourceKind::Configuration,
                ResourceAction::PublishRevision,
            ),
        }
    }

    pub fn audit_action(self) -> AuditAction {
        AuditAction::new(match self {
            Self::GatewayAdminRead => "control_plane.gateway_admin.read",
            Self::RouteExplain => "control_plane.route.explain",
            Self::TenantCreate => "control_plane.tenant.create",
            Self::TenantUpdate => "control_plane.tenant.update",
            Self::UserInvite => "control_plane.user.invite",
            Self::ScimUserRead => "control_plane.scim_user.read",
            Self::ScimUserCreate => "control_plane.scim_user.create",
            Self::ScimUserUpdate => "control_plane.scim_user.update",
            Self::ScimUserDelete => "control_plane.scim_user.delete",
            Self::RoleBindingGrant => "control_plane.role_binding.grant",
            Self::RoleBindingRevoke => "control_plane.role_binding.revoke",
            Self::ServiceIdentityCreate => "control_plane.service_identity.create",
            Self::VirtualKeyRead => "control_plane.virtual_key.read",
            Self::VirtualKeyCreate => "control_plane.virtual_key.create",
            Self::VirtualKeyUpdate => "control_plane.virtual_key.update",
            Self::VirtualKeyDelete => "control_plane.virtual_key.delete",
            Self::VirtualKeyRotateSecret => "control_plane.virtual_key.rotate_secret",
            Self::PolicyPublish => "control_plane.policy.publish",
            Self::ProviderCredentialRotate => "control_plane.provider_credential.rotate_secret",
            Self::BudgetUpdate => "control_plane.budget.update",
            Self::BillingRead => "control_plane.billing.read",
            Self::AuditExport => "control_plane.audit.export",
            Self::AuditRetentionPurge => "control_plane.audit.retention_purge",
            Self::ConfigurationPublish => "control_plane.configuration.publish",
        })
    }

    pub const fn requires_idempotency(self) -> bool {
        !matches!(
            self,
            Self::GatewayAdminRead
                | Self::RouteExplain
                | Self::ScimUserRead
                | Self::VirtualKeyRead
                | Self::BillingRead
                | Self::AuditExport
        )
    }

    pub const fn requires_immutable_audit(self) -> bool {
        true
    }

    pub fn audit_requirement(self) -> ControlPlaneAuditRequirementPlan {
        ControlPlaneAuditRequirementPlan {
            operation: self,
            action: self.audit_action(),
            write_mode: ControlPlaneAuditWriteMode::AppendOnlyHashChain,
            success_required: true,
            denial_required: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlPlaneRequirement {
    pub resource: ResourceKind,
    pub action: ResourceAction,
    pub required_role: Role,
}

impl ControlPlaneRequirement {
    pub const fn admin(resource: ResourceKind, action: ResourceAction) -> Self {
        Self {
            resource,
            action,
            required_role: Role::Admin,
        }
    }

    pub const fn viewer(resource: ResourceKind, action: ResourceAction) -> Self {
        Self {
            resource,
            action,
            required_role: Role::Viewer,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlPlaneResourceRef {
    pub tenant_id: TenantId,
    pub kind: ResourceKind,
    pub id: Option<String>,
}

impl ControlPlaneResourceRef {
    pub fn new(tenant_id: TenantId, kind: ResourceKind, id: Option<impl Into<String>>) -> Self {
        Self {
            tenant_id,
            kind,
            id: id.map(Into::into),
        }
    }

    fn audit_resource(&self) -> AuditResource {
        AuditResource::new(
            control_plane_resource_kind_label(self.kind),
            self.id.clone(),
            Some(self.tenant_id),
        )
    }
}

fn control_plane_resource_kind_label(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Tenant => "tenant",
        ResourceKind::User => "user",
        ResourceKind::RoleBinding => "role_binding",
        ResourceKind::ServiceIdentity => "service_identity",
        ResourceKind::VirtualKey => "virtual_key",
        ResourceKind::Policy => "policy",
        ResourceKind::ProviderCredential => "provider_credential",
        ResourceKind::Budget => "budget",
        ResourceKind::Billing => "billing",
        ResourceKind::AuditLog => "audit_log",
        ResourceKind::Configuration => "configuration",
    }
}

impl TenantScopedResource for ControlPlaneResourceRef {
    fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlPlaneActionRequest {
    pub principal: Principal,
    pub operation: ControlPlaneOperation,
    pub resource: ControlPlaneResourceRef,
    pub occurred_at_unix_ms: u64,
}

impl ControlPlaneActionRequest {
    pub fn idempotent_operation(
        &self,
        key: IdempotencyKey,
        request_fingerprint: impl Into<String>,
    ) -> Result<Option<IdempotentOperation>, IdempotentOperationError> {
        if self.operation.requires_idempotency() {
            IdempotentOperation::new(self.resource.tenant_id, key, request_fingerprint).map(Some)
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlPlaneActionPlan {
    pub tenant: TenantContext,
    pub operation: ControlPlaneOperation,
    pub requirement: ControlPlaneRequirement,
    pub audit_write: ControlPlaneAuditWritePlan,
    pub audit_event: AuditEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlPlaneAuditWriteMode {
    AppendOnlyHashChain,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlPlaneAuditRequirementPlan {
    pub operation: ControlPlaneOperation,
    pub action: AuditAction,
    pub write_mode: ControlPlaneAuditWriteMode,
    pub success_required: bool,
    pub denial_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlPlaneAuditWritePlan {
    pub event: AuditEvent,
    pub mode: ControlPlaneAuditWriteMode,
    pub tenant_partition_key: TenantId,
}

impl ControlPlaneAuditWritePlan {
    fn append_only(event: AuditEvent) -> Self {
        Self {
            tenant_partition_key: event.tenant_id,
            event,
            mode: ControlPlaneAuditWriteMode::AppendOnlyHashChain,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreakGlassAuthorization {
    pub reason: String,
    pub expires_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlPlaneDecision {
    Authorized(ControlPlaneActionPlan),
    Denied {
        error: ControlPlaneAuthorizationError,
        audit_write: ControlPlaneAuditWritePlan,
        audit_event: AuditEvent,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlPlaneAuthorizationError {
    CredentialScopeMismatch {
        expected: CredentialScope,
        actual: CredentialScope,
    },
    InsufficientRole {
        required: Role,
        actual: Role,
    },
    Tenant(TenantAccessError),
    ResourceKindMismatch {
        operation: ControlPlaneOperation,
        expected: ResourceKind,
        actual: ResourceKind,
    },
    BreakGlassExpired {
        now_unix_ms: u64,
        expires_at_unix_ms: u64,
    },
    BreakGlassReasonMissing,
    BreakGlassReasonMalformed,
}

impl fmt::Display for ControlPlaneAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "control-plane authorization request is denied")
    }
}

impl Error for ControlPlaneAuthorizationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlPlaneAuthorizationErrorStatus {
    Forbidden,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlPlaneAuthorizationErrorResponsePlan {
    pub status: ControlPlaneAuthorizationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_control_plane_authorization_error_response(
    error: &ControlPlaneAuthorizationError,
) -> ControlPlaneAuthorizationErrorResponsePlan {
    match error {
        ControlPlaneAuthorizationError::CredentialScopeMismatch { .. } => {
            ControlPlaneAuthorizationErrorResponsePlan {
                status: ControlPlaneAuthorizationErrorStatus::Forbidden,
                code: "credential_scope_not_allowed",
                message: "credential scope is not allowed for this control-plane operation",
            }
        }
        ControlPlaneAuthorizationError::InsufficientRole { .. } => {
            ControlPlaneAuthorizationErrorResponsePlan {
                status: ControlPlaneAuthorizationErrorStatus::Forbidden,
                code: "role_not_authorized",
                message: "principal role is not authorized for this control-plane operation",
            }
        }
        ControlPlaneAuthorizationError::Tenant(_) => ControlPlaneAuthorizationErrorResponsePlan {
            status: ControlPlaneAuthorizationErrorStatus::Forbidden,
            code: "tenant_access_denied",
            message: "tenant access is denied",
        },
        ControlPlaneAuthorizationError::ResourceKindMismatch { .. } => {
            ControlPlaneAuthorizationErrorResponsePlan {
                status: ControlPlaneAuthorizationErrorStatus::Forbidden,
                code: "resource_not_authorized",
                message: "resource is not authorized for this control-plane operation",
            }
        }
        ControlPlaneAuthorizationError::BreakGlassExpired { .. }
        | ControlPlaneAuthorizationError::BreakGlassReasonMissing
        | ControlPlaneAuthorizationError::BreakGlassReasonMalformed => {
            ControlPlaneAuthorizationErrorResponsePlan {
                status: ControlPlaneAuthorizationErrorStatus::Forbidden,
                code: "break_glass_not_authorized",
                message: "break-glass access is not authorized",
            }
        }
    }
}

pub fn decide_control_plane_action(request: ControlPlaneActionRequest) -> ControlPlaneDecision {
    let result = authorize_control_plane_action(&request, CredentialScope::ControlPlane);
    decision_from_result(request, result)
}

pub fn decide_break_glass_action(
    request: ControlPlaneActionRequest,
    authorization: BreakGlassAuthorization,
) -> ControlPlaneDecision {
    let result = validate_break_glass(&request, &authorization)
        .and_then(|()| authorize_control_plane_action(&request, CredentialScope::BreakGlass));
    decision_from_result(request, result)
}

fn decision_from_result(
    request: ControlPlaneActionRequest,
    result: Result<(TenantContext, ControlPlaneRequirement), ControlPlaneAuthorizationError>,
) -> ControlPlaneDecision {
    match result {
        Ok((tenant, requirement)) => {
            let audit_event = audit_event(&request, tenant, AuditOutcome::Success, None);
            ControlPlaneDecision::Authorized(ControlPlaneActionPlan {
                tenant,
                operation: request.operation,
                requirement,
                audit_write: ControlPlaneAuditWritePlan::append_only(audit_event.clone()),
                audit_event,
            })
        }
        Err(error) => {
            let tenant = TenantContext {
                tenant_id: request.resource.tenant_id,
            };
            let reason = Some(error.reason_code());
            let audit_event = audit_event(&request, tenant, AuditOutcome::Denied, reason);
            ControlPlaneDecision::Denied {
                audit_write: ControlPlaneAuditWritePlan::append_only(audit_event.clone()),
                audit_event,
                error,
            }
        }
    }
}

fn authorize_control_plane_action(
    request: &ControlPlaneActionRequest,
    expected_scope: CredentialScope,
) -> Result<(TenantContext, ControlPlaneRequirement), ControlPlaneAuthorizationError> {
    let requirement = request.operation.requirement();
    if request.principal.credential_scope != expected_scope {
        return Err(ControlPlaneAuthorizationError::CredentialScopeMismatch {
            expected: expected_scope,
            actual: request.principal.credential_scope,
        });
    }
    authorize_min_role(&request.principal, requirement.required_role).map_err(|err| match err {
        AuthorizationError::InsufficientRole { required, actual } => {
            ControlPlaneAuthorizationError::InsufficientRole { required, actual }
        }
        AuthorizationError::CredentialScopeMismatch { expected, actual } => {
            ControlPlaneAuthorizationError::CredentialScopeMismatch { expected, actual }
        }
    })?;
    if request.resource.kind != requirement.resource {
        return Err(ControlPlaneAuthorizationError::ResourceKindMismatch {
            operation: request.operation,
            expected: requirement.resource,
            actual: request.resource.kind,
        });
    }
    let tenant = authorize_tenant_access(&request.principal, &request.resource)
        .map_err(ControlPlaneAuthorizationError::Tenant)?;
    Ok((tenant, requirement))
}

fn validate_break_glass(
    request: &ControlPlaneActionRequest,
    authorization: &BreakGlassAuthorization,
) -> Result<(), ControlPlaneAuthorizationError> {
    if authorization.reason.is_empty() {
        return Err(ControlPlaneAuthorizationError::BreakGlassReasonMissing);
    }
    if authorization.reason.len() > 512
        || authorization.reason.chars().all(char::is_whitespace)
        || authorization.reason.chars().any(char::is_control)
    {
        return Err(ControlPlaneAuthorizationError::BreakGlassReasonMalformed);
    }
    if request.occurred_at_unix_ms >= authorization.expires_at_unix_ms {
        return Err(ControlPlaneAuthorizationError::BreakGlassExpired {
            now_unix_ms: request.occurred_at_unix_ms,
            expires_at_unix_ms: authorization.expires_at_unix_ms,
        });
    }
    Ok(())
}

fn audit_event(
    request: &ControlPlaneActionRequest,
    tenant: TenantContext,
    outcome: AuditOutcome,
    reason_code: Option<String>,
) -> AuditEvent {
    AuditEvent::new(
        request.occurred_at_unix_ms,
        tenant,
        &request.principal,
        request.operation.audit_action(),
        request.resource.audit_resource(),
        outcome,
        reason_code,
    )
}

impl ControlPlaneAuthorizationError {
    fn reason_code(&self) -> String {
        match self {
            Self::CredentialScopeMismatch { .. } => "credential_scope_mismatch",
            Self::InsufficientRole { .. } => "insufficient_role",
            Self::Tenant(_) => "tenant_access_denied",
            Self::ResourceKindMismatch { .. } => "resource_kind_mismatch",
            Self::BreakGlassExpired { .. } => "break_glass_expired",
            Self::BreakGlassReasonMissing => "break_glass_reason_missing",
            Self::BreakGlassReasonMalformed => "break_glass_reason_malformed",
        }
        .to_string()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigurationPublicationRequest<T> {
    pub action: ControlPlaneActionRequest,
    pub current_revision_id: Option<PolicyRevisionId>,
    pub candidate: ConfigRevision<T>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigurationPublicationPlan<T> {
    pub action: ControlPlaneActionPlan,
    pub candidate: ConfigRevision<T>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigurationPublicationDecision<T> {
    Authorized(ConfigurationPublicationPlan<T>),
    Denied {
        error: ConfigurationPublicationError,
        audit_write: ControlPlaneAuditWritePlan,
        audit_event: AuditEvent,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigurationPublicationError {
    Authorization(ControlPlaneAuthorizationError),
    Publication(ConfigPublicationError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigurationPublicationErrorStatus {
    BadRequest,
    Conflict,
    Forbidden,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigurationPublicationErrorResponsePlan {
    pub status: ConfigurationPublicationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_configuration_publication_error_response(
    error: &ConfigurationPublicationError,
) -> ConfigurationPublicationErrorResponsePlan {
    match error {
        ConfigurationPublicationError::Authorization(error) => {
            let response = plan_control_plane_authorization_error_response(error);
            ConfigurationPublicationErrorResponsePlan {
                status: ConfigurationPublicationErrorStatus::Forbidden,
                code: response.code,
                message: response.message,
            }
        }
        ConfigurationPublicationError::Publication(error) => {
            let response = plan_config_publication_error_response(error);
            ConfigurationPublicationErrorResponsePlan {
                status: match response.status {
                    ConfigPublicationErrorStatus::BadRequest => {
                        ConfigurationPublicationErrorStatus::BadRequest
                    }
                    ConfigPublicationErrorStatus::Conflict => {
                        ConfigurationPublicationErrorStatus::Conflict
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
    }
}

pub fn decide_configuration_publication<T>(
    request: ConfigurationPublicationRequest<T>,
) -> ConfigurationPublicationDecision<T> {
    match decide_control_plane_action(request.action.clone()) {
        ControlPlaneDecision::Denied {
            error, audit_event, ..
        } => ConfigurationPublicationDecision::Denied {
            error: ConfigurationPublicationError::Authorization(error),
            audit_write: ControlPlaneAuditWritePlan::append_only(audit_event.clone()),
            audit_event,
        },
        ControlPlaneDecision::Authorized(action) => {
            if let Err(error) = validate_config_publication(
                action.tenant.tenant_id,
                request.current_revision_id,
                &request.candidate,
            ) {
                let denied = audit_event(
                    &request.action,
                    action.tenant,
                    AuditOutcome::Denied,
                    Some("configuration_publication_rejected".to_string()),
                );
                return ConfigurationPublicationDecision::Denied {
                    error: ConfigurationPublicationError::Publication(error),
                    audit_write: ControlPlaneAuditWritePlan::append_only(denied.clone()),
                    audit_event: denied,
                };
            }
            ConfigurationPublicationDecision::Authorized(ConfigurationPublicationPlan {
                action,
                candidate: request.candidate,
            })
        }
    }
}
