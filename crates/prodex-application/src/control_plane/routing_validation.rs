use super::*;
use prodex_control_plane::ControlPlaneOperation;
use prodex_gateway_http::GatewayHttpMethod;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(any(not(feature = "mojo"), test))]
pub(super) enum ControlPlaneRouteValidationMode {
    Exact,
    AllowAlias,
    AllowAliasAndMethodCheck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(any(not(feature = "mojo"), test))]
pub(super) enum ControlPlaneRouteValidationDecision {
    Allow,
    OperationMismatch,
    MethodNotAllowed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlPlaneRequestValidationKind {
    Idempotency,
    Page,
    Precondition,
    Audit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlPlaneRequestValidationDecision {
    Allow,
    OperationMismatch,
    MethodNotAllowed,
    AuditNotRequired,
}

pub(super) fn control_plane_request_validation(
    route_operation: ControlPlaneOperation,
    action_operation: ControlPlaneOperation,
    method: GatewayHttpMethod,
    kind: ControlPlaneRequestValidationKind,
    route_requires_audit: bool,
    action_requires_audit: bool,
) -> Result<ControlPlaneRequestValidationDecision, ApplicationControlPlaneHttpRouteError> {
    #[cfg(feature = "mojo")]
    {
        use prodex_mojo_core::control_plane_routing as mojo;

        let operation_tag = |operation| {
            ControlPlaneOperation::ALL
                .iter()
                .position(|candidate| *candidate == operation)
                .and_then(mojo::ControlPlaneOperationTag::new)
                .ok_or_else(control_plane_route_kernel_failure)
        };
        let input = mojo::ControlPlaneRequestValidationInput {
            route_operation: operation_tag(route_operation)?,
            action_operation: operation_tag(action_operation)?,
            method: mojo_method(method),
            kind: match kind {
                ControlPlaneRequestValidationKind::Idempotency => {
                    mojo::ControlPlaneRequestKind::Idempotency
                }
                ControlPlaneRequestValidationKind::Page => mojo::ControlPlaneRequestKind::Page,
                ControlPlaneRequestValidationKind::Precondition => {
                    mojo::ControlPlaneRequestKind::Precondition
                }
                ControlPlaneRequestValidationKind::Audit => mojo::ControlPlaneRequestKind::Audit,
            },
            route_requires_audit,
            action_requires_audit,
        };
        mojo::validate_request(input)
            .map(|decision| match decision {
                mojo::ControlPlaneRequestValidationDecision::Allow => {
                    ControlPlaneRequestValidationDecision::Allow
                }
                mojo::ControlPlaneRequestValidationDecision::OperationMismatch => {
                    ControlPlaneRequestValidationDecision::OperationMismatch
                }
                mojo::ControlPlaneRequestValidationDecision::MethodNotAllowed => {
                    ControlPlaneRequestValidationDecision::MethodNotAllowed
                }
                mojo::ControlPlaneRequestValidationDecision::AuditNotRequired => {
                    ControlPlaneRequestValidationDecision::AuditNotRequired
                }
            })
            .map_err(|_| control_plane_route_kernel_failure())
    }

    #[cfg(not(feature = "mojo"))]
    Ok(control_plane_request_validation_rust(
        route_operation,
        action_operation,
        method,
        kind,
        route_requires_audit,
        action_requires_audit,
    ))
}

#[cfg(test)]
pub(super) fn control_plane_route_validation(
    route_operation: ControlPlaneOperation,
    action_operation: ControlPlaneOperation,
    method: GatewayHttpMethod,
    mode: ControlPlaneRouteValidationMode,
) -> Result<ControlPlaneRouteValidationDecision, ApplicationControlPlaneHttpRouteError> {
    #[cfg(feature = "mojo")]
    {
        let route_operation = ControlPlaneOperation::ALL
            .iter()
            .position(|operation| *operation == route_operation)
            .and_then(prodex_mojo_core::control_plane_routing::ControlPlaneOperationTag::new)
            .ok_or_else(control_plane_route_kernel_failure)?;
        let action_operation = ControlPlaneOperation::ALL
            .iter()
            .position(|operation| *operation == action_operation)
            .and_then(prodex_mojo_core::control_plane_routing::ControlPlaneOperationTag::new)
            .ok_or_else(control_plane_route_kernel_failure)?;
        let method = mojo_method(method);
        let mode = match mode {
            ControlPlaneRouteValidationMode::Exact => {
                prodex_mojo_core::control_plane_routing::ControlPlaneRouteValidationMode::Exact
            }
            ControlPlaneRouteValidationMode::AllowAlias => {
                prodex_mojo_core::control_plane_routing::ControlPlaneRouteValidationMode::AllowAlias
            }
            ControlPlaneRouteValidationMode::AllowAliasAndMethodCheck => {
                prodex_mojo_core::control_plane_routing::ControlPlaneRouteValidationMode::AllowAliasAndMethodCheck
            }
        };
        prodex_mojo_core::control_plane_routing::validate(
            prodex_mojo_core::control_plane_routing::ControlPlaneRouteValidationInput {
                route_operation,
                action_operation,
                method,
                mode,
            },
        )
        .map(|decision| match decision {
            prodex_mojo_core::control_plane_routing::ControlPlaneRouteValidationDecision::Allow => {
                ControlPlaneRouteValidationDecision::Allow
            }
            prodex_mojo_core::control_plane_routing::ControlPlaneRouteValidationDecision::OperationMismatch => {
                ControlPlaneRouteValidationDecision::OperationMismatch
            }
            prodex_mojo_core::control_plane_routing::ControlPlaneRouteValidationDecision::MethodNotAllowed => {
                ControlPlaneRouteValidationDecision::MethodNotAllowed
            }
        })
        .map_err(|_| control_plane_route_kernel_failure())
    }

    #[cfg(not(feature = "mojo"))]
    Ok(control_plane_route_validation_rust(
        route_operation,
        action_operation,
        method,
        mode,
    ))
}

#[cfg(feature = "mojo")]
fn mojo_method(
    method: GatewayHttpMethod,
) -> prodex_mojo_core::control_plane_routing::ControlPlaneHttpMethod {
    use prodex_mojo_core::control_plane_routing::ControlPlaneHttpMethod as MojoMethod;

    match method {
        GatewayHttpMethod::Get => MojoMethod::Get,
        GatewayHttpMethod::Post => MojoMethod::Post,
        GatewayHttpMethod::Put => MojoMethod::Put,
        GatewayHttpMethod::Patch => MojoMethod::Patch,
        GatewayHttpMethod::Delete => MojoMethod::Delete,
        GatewayHttpMethod::Options => MojoMethod::Options,
        GatewayHttpMethod::Other => MojoMethod::Other,
    }
}

#[cfg(feature = "mojo")]
fn control_plane_route_kernel_failure() -> ApplicationControlPlaneHttpRouteError {
    ApplicationControlPlaneHttpRouteError::Route(
        GatewayControlPlaneRouteError::UnknownControlPlaneRoute,
    )
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn control_plane_route_validation_rust(
    route_operation: ControlPlaneOperation,
    action_operation: ControlPlaneOperation,
    method: GatewayHttpMethod,
    mode: ControlPlaneRouteValidationMode,
) -> ControlPlaneRouteValidationDecision {
    if route_operation == action_operation {
        return ControlPlaneRouteValidationDecision::Allow;
    }
    if !matches!(mode, ControlPlaneRouteValidationMode::Exact)
        && control_plane_http_action_alias_allowed(route_operation, action_operation, method)
    {
        return ControlPlaneRouteValidationDecision::Allow;
    }
    if matches!(
        mode,
        ControlPlaneRouteValidationMode::AllowAliasAndMethodCheck
    ) && control_plane_operations_share_route_family(route_operation, action_operation)
        && !control_plane_operation_allows_http_method(action_operation, method)
    {
        return ControlPlaneRouteValidationDecision::MethodNotAllowed;
    }
    ControlPlaneRouteValidationDecision::OperationMismatch
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn control_plane_request_validation_rust(
    route_operation: ControlPlaneOperation,
    action_operation: ControlPlaneOperation,
    method: GatewayHttpMethod,
    kind: ControlPlaneRequestValidationKind,
    route_requires_audit: bool,
    action_requires_audit: bool,
) -> ControlPlaneRequestValidationDecision {
    let mode = match kind {
        ControlPlaneRequestValidationKind::Idempotency => {
            ControlPlaneRouteValidationMode::AllowAliasAndMethodCheck
        }
        ControlPlaneRequestValidationKind::Page => ControlPlaneRouteValidationMode::Exact,
        ControlPlaneRequestValidationKind::Precondition
        | ControlPlaneRequestValidationKind::Audit => ControlPlaneRouteValidationMode::AllowAlias,
    };
    match control_plane_route_validation_rust(route_operation, action_operation, method, mode) {
        ControlPlaneRouteValidationDecision::Allow
            if kind == ControlPlaneRequestValidationKind::Audit
                && (!route_requires_audit || !action_requires_audit) =>
        {
            ControlPlaneRequestValidationDecision::AuditNotRequired
        }
        ControlPlaneRouteValidationDecision::Allow => ControlPlaneRequestValidationDecision::Allow,
        ControlPlaneRouteValidationDecision::OperationMismatch => {
            ControlPlaneRequestValidationDecision::OperationMismatch
        }
        ControlPlaneRouteValidationDecision::MethodNotAllowed => {
            ControlPlaneRequestValidationDecision::MethodNotAllowed
        }
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn control_plane_operations_share_route_family(
    route_operation: ControlPlaneOperation,
    action_operation: ControlPlaneOperation,
) -> bool {
    use ControlPlaneOperation::*;

    matches!(
        (route_operation, action_operation),
        (GatewayAdminRead, GatewayAdminRead)
            | (RouteExplain, RouteExplain)
            | (TenantCreate | TenantUpdate, TenantCreate | TenantUpdate)
            | (UserInvite, UserInvite)
            | (
                ScimUserRead | ScimUserCreate | ScimUserUpdate | ScimUserDelete,
                ScimUserRead | ScimUserCreate | ScimUserUpdate | ScimUserDelete
            )
            | (
                RoleBindingGrant | RoleBindingRevoke,
                RoleBindingGrant | RoleBindingRevoke
            )
            | (ServiceIdentityCreate, ServiceIdentityCreate)
            | (
                VirtualKeyRead
                    | VirtualKeyCreate
                    | VirtualKeyUpdate
                    | VirtualKeyDelete
                    | VirtualKeyRotateSecret,
                VirtualKeyRead
                    | VirtualKeyCreate
                    | VirtualKeyUpdate
                    | VirtualKeyDelete
                    | VirtualKeyRotateSecret
            )
            | (ProviderCredentialRotate, ProviderCredentialRotate)
            | (BudgetUpdate, BudgetUpdate)
            | (
                PolicyRead
                    | PolicyCreate
                    | PolicyValidate
                    | PolicySubmit
                    | PolicyVote
                    | PolicyActivate
                    | PolicyRollback
                    | PolicyRevoke
                    | PolicyPublish,
                PolicyRead
                    | PolicyCreate
                    | PolicyValidate
                    | PolicySubmit
                    | PolicyVote
                    | PolicyActivate
                    | PolicyRollback
                    | PolicyRevoke
                    | PolicyPublish
            )
            | (ConfigurationPublish, ConfigurationPublish)
            | (BillingRead, BillingRead)
            | (
                AuditExport | AuditRetentionPurge,
                AuditExport | AuditRetentionPurge
            )
            | (
                AuditLegalHoldRead | AuditLegalHoldUpsert | AuditLegalHoldDelete,
                AuditLegalHoldRead | AuditLegalHoldUpsert | AuditLegalHoldDelete
            )
    )
}

#[cfg(any(not(feature = "mojo"), test))]
fn control_plane_http_action_alias_allowed(
    route_operation: ControlPlaneOperation,
    action_operation: ControlPlaneOperation,
    method: GatewayHttpMethod,
) -> bool {
    matches!(
        (route_operation, action_operation, method),
        (
            ControlPlaneOperation::VirtualKeyUpdate,
            ControlPlaneOperation::VirtualKeyRotateSecret,
            GatewayHttpMethod::Patch,
        )
    )
}

#[cfg(any(not(feature = "mojo"), test))]
fn control_plane_operation_allows_http_method(
    operation: ControlPlaneOperation,
    method: GatewayHttpMethod,
) -> bool {
    use ControlPlaneOperation::*;
    use GatewayHttpMethod::{Delete, Get, Patch, Post, Put};

    match operation {
        GatewayAdminRead | ScimUserRead | VirtualKeyRead | PolicyRead | BillingRead
        | AuditLegalHoldRead => method == Get,
        RouteExplain
        | TenantCreate
        | UserInvite
        | ScimUserCreate
        | RoleBindingGrant
        | ServiceIdentityCreate
        | VirtualKeyCreate
        | VirtualKeyRotateSecret
        | ProviderCredentialRotate
        | ConfigurationPublish
        | AuditExport
        | AuditLegalHoldUpsert => method == Post,
        PolicyValidate | PolicyCreate | PolicySubmit | PolicyVote | PolicyActivate
        | PolicyRollback | PolicyRevoke => method == Post,
        PolicyPublish => matches!(method, Get | Post),
        TenantUpdate | VirtualKeyUpdate | BudgetUpdate => method == Patch,
        ScimUserUpdate => matches!(method, Patch | Put),
        ScimUserDelete | RoleBindingRevoke | VirtualKeyDelete | AuditLegalHoldDelete
        | AuditRetentionPurge => method == Delete,
    }
}
