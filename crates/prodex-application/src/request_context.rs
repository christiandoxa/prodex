use std::error::Error;
use std::fmt;

use prodex_authn::{
    CompatibilityAuthenticationError, CompatibilityAuthenticationRequest,
    authenticate_compatibility_request,
};
use prodex_authz::{
    BoundaryAuthorizationError, BoundaryKind, authorize_boundary_role, authorize_boundary_scope,
};
use prodex_control_plane::{
    ControlPlaneActionRequest, ControlPlaneAuthorizationError, ControlPlaneDecision,
    decide_control_plane_action,
};
use prodex_domain::{CredentialScope, Principal, TenantContext, TenantMode, TenantResolutionError};
use prodex_gateway_http::{
    CanonicalRequestTarget, GatewayHttpRouteKind, GatewayHttpRoutePlane, classify_request_target,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ApplicationRequestContext<'a> {
    target: &'a CanonicalRequestTarget,
    route: GatewayHttpRouteKind,
    plane: GatewayHttpRoutePlane,
    required_credential_scope: Option<CredentialScope>,
}

impl<'a> ApplicationRequestContext<'a> {
    pub const fn target(self) -> &'a CanonicalRequestTarget {
        self.target
    }

    pub const fn route(self) -> GatewayHttpRouteKind {
        self.route
    }

    pub const fn plane(self) -> GatewayHttpRoutePlane {
        self.plane
    }

    pub const fn required_credential_scope(self) -> Option<CredentialScope> {
        self.required_credential_scope
    }
}

impl fmt::Debug for ApplicationRequestContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRequestContext")
            .field("target", &"<redacted>")
            .field("route", &self.route)
            .field("plane", &self.plane)
            .field("required_credential_scope", &self.required_credential_scope)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationAuthenticatedRequestContext<'a> {
    request: ApplicationRequestContext<'a>,
    principal: Option<Principal>,
}

impl<'a> ApplicationAuthenticatedRequestContext<'a> {
    pub const fn request(&self) -> ApplicationRequestContext<'a> {
        self.request
    }

    pub fn principal(&self) -> Option<&Principal> {
        self.principal.as_ref()
    }
}

impl fmt::Debug for ApplicationAuthenticatedRequestContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationAuthenticatedRequestContext")
            .field("request", &self.request)
            .field("principal", &self.principal.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationAuthorizedRequestContext<'a> {
    authenticated: ApplicationAuthenticatedRequestContext<'a>,
    tenant: Option<TenantContext>,
}

impl<'a> ApplicationAuthorizedRequestContext<'a> {
    pub const fn request(&self) -> ApplicationRequestContext<'a> {
        self.authenticated.request
    }

    pub fn principal(&self) -> Option<&Principal> {
        self.authenticated.principal.as_ref()
    }

    pub const fn tenant_context(&self) -> Option<TenantContext> {
        self.tenant
    }
}

impl fmt::Debug for ApplicationAuthorizedRequestContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationAuthorizedRequestContext")
            .field("request", &self.authenticated.request)
            .field(
                "principal",
                &self.authenticated.principal.as_ref().map(|_| "<redacted>"),
            )
            .field("tenant", &self.tenant.map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationRequestContextError {
    UnknownRoute,
}

impl fmt::Display for ApplicationRequestContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("application route is not available")
    }
}

impl Error for ApplicationRequestContextError {}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationRequestAuthorizationError {
    WrongPlane,
    AnonymousNotAllowed,
    PrincipalMismatch,
    DataPlane(BoundaryAuthorizationError),
    Tenant(TenantResolutionError),
    ControlPlane(ControlPlaneAuthorizationError),
}

impl fmt::Debug for ApplicationRequestAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongPlane => f.write_str("WrongPlane"),
            Self::AnonymousNotAllowed => f.write_str("AnonymousNotAllowed"),
            Self::PrincipalMismatch => f.write_str("PrincipalMismatch"),
            Self::DataPlane(_) => f.debug_tuple("DataPlane").field(&"<redacted>").finish(),
            Self::Tenant(_) => f.debug_tuple("Tenant").field(&"<redacted>").finish(),
            Self::ControlPlane(_) => f.debug_tuple("ControlPlane").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationRequestAuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("application request authorization is denied")
    }
}

impl Error for ApplicationRequestAuthorizationError {}

pub fn plan_application_request_context(
    target: &CanonicalRequestTarget,
) -> Result<ApplicationRequestContext<'_>, ApplicationRequestContextError> {
    let route =
        classify_request_target(target).ok_or(ApplicationRequestContextError::UnknownRoute)?;
    Ok(ApplicationRequestContext {
        target,
        route: route.kind,
        plane: route.plane,
        required_credential_scope: required_credential_scope_for_plane(route.plane),
    })
}

/// Strangler binding for credentials still verified by the legacy adapter.
///
/// The returned context is authoritative: callers must not continue to
/// admission or dispatch when this planner rejects the credential scope.
pub fn plan_application_request_authentication_from_compatibility(
    request: ApplicationRequestContext<'_>,
    principal: Option<Principal>,
    anonymous_allowed: bool,
) -> Result<ApplicationAuthenticatedRequestContext<'_>, CompatibilityAuthenticationError> {
    let principal = authenticate_compatibility_request(CompatibilityAuthenticationRequest {
        principal,
        required_scope: request.required_credential_scope,
        anonymous_allowed,
    })?;
    Ok(ApplicationAuthenticatedRequestContext { request, principal })
}

pub fn plan_application_data_plane_authorization_from_compatibility(
    authenticated: ApplicationAuthenticatedRequestContext<'_>,
) -> Result<ApplicationAuthorizedRequestContext<'_>, ApplicationRequestAuthorizationError> {
    if authenticated.request.plane != GatewayHttpRoutePlane::DataPlane {
        return Err(ApplicationRequestAuthorizationError::WrongPlane);
    }
    let Some(principal) = authenticated.principal.as_ref() else {
        return Ok(ApplicationAuthorizedRequestContext {
            authenticated,
            tenant: None,
        });
    };
    let boundary = if authenticated.request.route == GatewayHttpRouteKind::DataPlaneQuota {
        BoundaryKind::DataPlaneQuota
    } else {
        BoundaryKind::DataPlaneInference
    };
    authorize_boundary_scope(boundary, principal)
        .map_err(ApplicationRequestAuthorizationError::DataPlane)?;
    authorize_boundary_role(boundary, principal)
        .map_err(ApplicationRequestAuthorizationError::DataPlane)?;
    let tenant = principal
        .tenant_context(TenantMode::SingleTenant)
        .map_err(ApplicationRequestAuthorizationError::Tenant)?;
    Ok(ApplicationAuthorizedRequestContext {
        authenticated,
        tenant: Some(tenant),
    })
}

pub fn plan_application_control_plane_authorization_from_compatibility(
    authenticated: ApplicationAuthenticatedRequestContext<'_>,
    action: ControlPlaneActionRequest,
) -> Result<ApplicationAuthorizedRequestContext<'_>, ApplicationRequestAuthorizationError> {
    if authenticated.request.plane != GatewayHttpRoutePlane::ControlPlane {
        return Err(ApplicationRequestAuthorizationError::WrongPlane);
    }
    let principal = authenticated
        .principal
        .as_ref()
        .ok_or(ApplicationRequestAuthorizationError::AnonymousNotAllowed)?;
    if principal != &action.principal {
        return Err(ApplicationRequestAuthorizationError::PrincipalMismatch);
    }
    let tenant = match decide_control_plane_action(action) {
        ControlPlaneDecision::Authorized(plan) => plan.tenant,
        ControlPlaneDecision::Denied { error, .. } => {
            return Err(ApplicationRequestAuthorizationError::ControlPlane(error));
        }
    };
    Ok(ApplicationAuthorizedRequestContext {
        authenticated,
        tenant: Some(tenant),
    })
}

pub(crate) const fn required_credential_scope_for_plane(
    plane: GatewayHttpRoutePlane,
) -> Option<CredentialScope> {
    match plane {
        GatewayHttpRoutePlane::DataPlane => Some(CredentialScope::DataPlane),
        GatewayHttpRoutePlane::ControlPlane => Some(CredentialScope::ControlPlane),
        GatewayHttpRoutePlane::Health => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_control_plane::{ControlPlaneOperation, ControlPlaneResourceRef};
    use prodex_domain::{PrincipalId, PrincipalKind, ResourceKind, Role, TenantId};

    fn id<T: std::str::FromStr>(value: &str) -> T
    where
        T::Err: fmt::Debug,
    {
        value.parse().unwrap()
    }

    fn principal(scope: CredentialScope, role: Role) -> Principal {
        Principal::new(
            id::<PrincipalId>("00000000-0000-7000-8000-000000000001"),
            Some(id::<TenantId>("00000000-0000-7000-8000-000000000002")),
            PrincipalKind::ServiceAccount,
            role,
            scope,
        )
    }

    #[test]
    fn canonical_context_and_compatibility_authentication_are_one_scope_gate() {
        let data_target = CanonicalRequestTarget::parse("/v1/responses?stream=true").unwrap();
        let data = plan_application_request_context(&data_target).unwrap();
        assert_eq!(data.target().path_and_query(), "/v1/responses?stream=true");
        assert_eq!(data.plane(), GatewayHttpRoutePlane::DataPlane);
        let debug = format!("{data:?}");
        assert!(!debug.contains("/v1/responses"));
        assert!(!debug.contains("stream=true"));
        assert!(
            plan_application_request_authentication_from_compatibility(
                data,
                Some(principal(CredentialScope::DataPlane, Role::Operator)),
                false,
            )
            .is_ok()
        );
        assert!(
            plan_application_request_authentication_from_compatibility(
                data,
                Some(principal(CredentialScope::ControlPlane, Role::Admin)),
                false,
            )
            .is_err()
        );

        let health_target = CanonicalRequestTarget::parse("/readyz").unwrap();
        let health = plan_application_request_context(&health_target).unwrap();
        assert_eq!(health.plane(), GatewayHttpRoutePlane::Health);
        assert!(
            plan_application_request_authentication_from_compatibility(health, None, true).is_ok()
        );
    }

    #[test]
    fn unknown_route_never_reaches_compatibility_authentication() {
        let target = CanonicalRequestTarget::parse("/v1/not-supported").unwrap();
        assert_eq!(
            plan_application_request_context(&target),
            Err(ApplicationRequestContextError::UnknownRoute),
        );
    }

    #[test]
    fn compatibility_authorization_binds_data_plane_scope_role_and_tenant() {
        let target = CanonicalRequestTarget::parse("/v1/responses").unwrap();
        let request = plan_application_request_context(&target).unwrap();
        let operator = principal(CredentialScope::DataPlane, Role::Operator);
        let expected_tenant = operator.tenant_id.unwrap();
        let authenticated = plan_application_request_authentication_from_compatibility(
            request,
            Some(operator),
            false,
        )
        .unwrap();
        let authorized =
            plan_application_data_plane_authorization_from_compatibility(authenticated).unwrap();
        assert_eq!(
            authorized.tenant_context().unwrap().tenant_id,
            expected_tenant
        );
        assert!(!format!("{authorized:?}").contains(&expected_tenant.to_string()));

        let viewer = plan_application_request_authentication_from_compatibility(
            request,
            Some(principal(CredentialScope::DataPlane, Role::Viewer)),
            false,
        )
        .unwrap();
        assert!(
            plan_application_data_plane_authorization_from_compatibility(viewer).is_err(),
            "data-plane viewer must not reach admission",
        );
        assert!(
            plan_application_request_authentication_from_compatibility(
                request,
                Some(principal(CredentialScope::ControlPlane, Role::Admin)),
                false,
            )
            .is_err(),
            "control-plane scope must not enter the data plane",
        );
        assert!(
            plan_application_request_authentication_from_compatibility(request, None, false)
                .is_err(),
            "anonymous requests must not enter a configured data plane",
        );
    }

    #[test]
    fn compatibility_authorization_binds_control_plane_principal_and_tenant() {
        let target = CanonicalRequestTarget::parse("/admin/keys").unwrap();
        let request = plan_application_request_context(&target).unwrap();
        let admin = principal(CredentialScope::ControlPlane, Role::Admin);
        let tenant_id = admin.tenant_id.unwrap();
        let action = |principal: Principal, resource_tenant_id| ControlPlaneActionRequest {
            principal,
            operation: ControlPlaneOperation::VirtualKeyCreate,
            resource: ControlPlaneResourceRef::new(
                resource_tenant_id,
                ResourceKind::VirtualKey,
                None::<String>,
            ),
            occurred_at_unix_ms: 1,
        };

        assert!(
            plan_application_request_authentication_from_compatibility(request, None, false)
                .is_err(),
            "anonymous requests must not enter the control plane",
        );
        assert!(
            plan_application_request_authentication_from_compatibility(
                request,
                Some(principal(CredentialScope::DataPlane, Role::Operator)),
                false,
            )
            .is_err(),
            "data-plane scope must not enter the control plane",
        );

        let authenticated = plan_application_request_authentication_from_compatibility(
            request,
            Some(admin.clone()),
            false,
        )
        .unwrap();
        let authorized = plan_application_control_plane_authorization_from_compatibility(
            authenticated,
            action(admin.clone(), tenant_id),
        )
        .unwrap();
        assert_eq!(authorized.tenant_context().unwrap().tenant_id, tenant_id);

        let authenticated = plan_application_request_authentication_from_compatibility(
            request,
            Some(admin.clone()),
            false,
        )
        .unwrap();
        assert_eq!(
            plan_application_control_plane_authorization_from_compatibility(
                authenticated,
                action(
                    principal(CredentialScope::ControlPlane, Role::Viewer),
                    tenant_id,
                ),
            ),
            Err(ApplicationRequestAuthorizationError::PrincipalMismatch),
            "the authorized principal must own the control-plane action",
        );

        let viewer = principal(CredentialScope::ControlPlane, Role::Viewer);
        let authenticated = plan_application_request_authentication_from_compatibility(
            request,
            Some(viewer.clone()),
            false,
        )
        .unwrap();
        assert!(
            plan_application_control_plane_authorization_from_compatibility(
                authenticated,
                action(viewer, tenant_id),
            )
            .is_err(),
            "control-plane viewer must not mutate virtual keys",
        );

        let authenticated = plan_application_request_authentication_from_compatibility(
            request,
            Some(admin.clone()),
            false,
        )
        .unwrap();
        let foreign_tenant = id::<TenantId>("00000000-0000-7000-8000-000000000003");
        let error = plan_application_control_plane_authorization_from_compatibility(
            authenticated,
            action(admin, foreign_tenant),
        )
        .expect_err("cross-tenant control-plane access must fail closed");
        assert!(!format!("{error:?}").contains(&foreign_tenant.to_string()));
    }
}
