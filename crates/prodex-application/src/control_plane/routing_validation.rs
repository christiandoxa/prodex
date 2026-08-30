use super::*;
use prodex_control_plane::ControlPlaneOperation;
use prodex_gateway_http::GatewayHttpMethod;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlPlaneRouteValidationMode {
    Exact,
    AllowAlias,
    AllowAliasAndMethodCheck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlPlaneRouteValidationDecision {
    Allow,
    OperationMismatch,
    MethodNotAllowed,
}

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
        let method = match method {
            GatewayHttpMethod::Get => {
                prodex_mojo_core::control_plane_routing::ControlPlaneHttpMethod::Get
            }
            GatewayHttpMethod::Post => {
                prodex_mojo_core::control_plane_routing::ControlPlaneHttpMethod::Post
            }
            GatewayHttpMethod::Put => {
                prodex_mojo_core::control_plane_routing::ControlPlaneHttpMethod::Put
            }
            GatewayHttpMethod::Patch => {
                prodex_mojo_core::control_plane_routing::ControlPlaneHttpMethod::Patch
            }
            GatewayHttpMethod::Delete => {
                prodex_mojo_core::control_plane_routing::ControlPlaneHttpMethod::Delete
            }
            GatewayHttpMethod::Options => {
                prodex_mojo_core::control_plane_routing::ControlPlaneHttpMethod::Options
            }
            GatewayHttpMethod::Other => {
                prodex_mojo_core::control_plane_routing::ControlPlaneHttpMethod::Other
            }
        };
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
        return prodex_mojo_core::control_plane_routing::validate(
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
        .map_err(|_| control_plane_route_kernel_failure());
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
