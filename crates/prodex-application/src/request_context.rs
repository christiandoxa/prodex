use std::error::Error;
use std::fmt;
use std::time::Instant;

use prodex_authn::{
    VerifiedCredentialAuthenticationError, VerifiedCredentialAuthenticationRequest,
    VerifiedCredentialEvidence, authenticate_verified_credential,
};
use prodex_authz::{
    BoundaryAuthorizationError, BoundaryKind, authorize_boundary_role, authorize_boundary_scope,
};
use prodex_control_plane::{
    ControlPlaneActionPlan, ControlPlaneActionRequest, ControlPlaneAuthorizationError,
    ControlPlaneDecision, decide_control_plane_action,
};
use prodex_domain::{
    CorrelationContext, CredentialScope, Principal, RequestId, TenantContext, TenantMode,
    TenantResolutionError,
};
use prodex_gateway_http::{
    CanonicalRequestTarget, GatewayHttpHeader, GatewayHttpPlanError, GatewayHttpRouteKind,
    GatewayHttpRoutePlane, classify_request_target, trace_context_from_headers,
};
use prodex_observability::TraceContext;

pub const APPLICATION_REQUEST_METADATA_HEADER_LIMIT: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApplicationRequestDeadline(Instant);

impl ApplicationRequestDeadline {
    pub const fn at(deadline: Instant) -> Self {
        Self(deadline)
    }

    pub const fn instant(self) -> Instant {
        self.0
    }

    pub fn is_expired_at(self, now: Instant) -> bool {
        now >= self.0
    }
}

impl fmt::Debug for ApplicationRequestDeadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ApplicationRequestDeadline")
            .field(&"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplicationRequestMetadata {
    observed_header_count: u8,
    headers_truncated: bool,
    trace_context_present: bool,
    credential_present: bool,
    affinity_present: bool,
    codex_metadata_present: bool,
    user_agent_present: bool,
}

impl ApplicationRequestMetadata {
    fn from_headers(headers: &[GatewayHttpHeader]) -> Self {
        let mut metadata = Self {
            observed_header_count: headers.len().min(APPLICATION_REQUEST_METADATA_HEADER_LIMIT)
                as u8,
            headers_truncated: headers.len() > APPLICATION_REQUEST_METADATA_HEADER_LIMIT,
            ..Self::default()
        };
        for header in headers
            .iter()
            .take(APPLICATION_REQUEST_METADATA_HEADER_LIMIT)
        {
            match header.normalized_name().as_str() {
                "traceparent" => metadata.trace_context_present = true,
                "authorization" | "chatgpt-account-id" => metadata.credential_present = true,
                "session_id" | "x-codex-turn-state" => metadata.affinity_present = true,
                "x-openai-subagent" | "x-codex-turn-metadata" | "x-codex-beta-features" => {
                    metadata.codex_metadata_present = true
                }
                "user-agent" => metadata.user_agent_present = true,
                _ => {}
            }
        }
        metadata
    }

    pub const fn observed_header_count(self) -> usize {
        self.observed_header_count as usize
    }

    pub const fn headers_truncated(self) -> bool {
        self.headers_truncated
    }

    pub const fn trace_context_present(self) -> bool {
        self.trace_context_present
    }

    pub const fn credential_present(self) -> bool {
        self.credential_present
    }

    pub const fn affinity_present(self) -> bool {
        self.affinity_present
    }

    pub const fn codex_metadata_present(self) -> bool {
        self.codex_metadata_present
    }

    pub const fn user_agent_present(self) -> bool {
        self.user_agent_present
    }
}

impl fmt::Debug for ApplicationRequestMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRequestMetadata")
            .field("observed_header_count", &self.observed_header_count)
            .field("headers_truncated", &self.headers_truncated)
            .field("trace_context_present", &self.trace_context_present)
            .field("credential_present", &self.credential_present)
            .field("affinity_present", &self.affinity_present)
            .field("codex_metadata_present", &self.codex_metadata_present)
            .field("user_agent_present", &self.user_agent_present)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRequestContext<'a> {
    target: &'a CanonicalRequestTarget,
    request_id: RequestId,
    deadline: ApplicationRequestDeadline,
    route: GatewayHttpRouteKind,
    plane: GatewayHttpRoutePlane,
    required_credential_scope: Option<CredentialScope>,
    trace_context: Option<TraceContext>,
    correlation: CorrelationContext,
    metadata: ApplicationRequestMetadata,
}

impl<'a> ApplicationRequestContext<'a> {
    pub const fn target(&self) -> &'a CanonicalRequestTarget {
        self.target
    }

    pub const fn request_id(&self) -> RequestId {
        self.request_id
    }

    pub const fn deadline(&self) -> ApplicationRequestDeadline {
        self.deadline
    }

    pub const fn route(&self) -> GatewayHttpRouteKind {
        self.route
    }

    pub const fn plane(&self) -> GatewayHttpRoutePlane {
        self.plane
    }

    pub const fn required_credential_scope(&self) -> Option<CredentialScope> {
        self.required_credential_scope
    }

    pub fn trace_context(&self) -> Option<&TraceContext> {
        self.trace_context.as_ref()
    }

    pub const fn correlation_context(&self) -> &CorrelationContext {
        &self.correlation
    }

    pub const fn metadata(&self) -> ApplicationRequestMetadata {
        self.metadata
    }
}

impl fmt::Debug for ApplicationRequestContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRequestContext")
            .field("target", &"<redacted>")
            .field("request_id", &"<redacted>")
            .field("deadline", &"<redacted>")
            .field("route", &self.route)
            .field("plane", &self.plane)
            .field("required_credential_scope", &self.required_credential_scope)
            .field(
                "trace_context",
                &self.trace_context.as_ref().map(|_| "<redacted>"),
            )
            .field("correlation", &"<redacted>")
            .field("metadata", &self.metadata)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationAuthenticatedRequestContext<'a> {
    request: ApplicationRequestContext<'a>,
    principal: Option<Principal>,
}

impl<'a> ApplicationAuthenticatedRequestContext<'a> {
    pub const fn request(&self) -> &ApplicationRequestContext<'a> {
        &self.request
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
    control_plane_action: Option<ControlPlaneActionPlan>,
    correlation: CorrelationContext,
}

impl<'a> ApplicationAuthorizedRequestContext<'a> {
    pub const fn request(&self) -> &ApplicationRequestContext<'a> {
        &self.authenticated.request
    }

    pub fn principal(&self) -> Option<&Principal> {
        self.authenticated.principal.as_ref()
    }

    pub const fn tenant_context(&self) -> Option<TenantContext> {
        self.tenant
    }

    pub fn control_plane_action(&self) -> Option<&ControlPlaneActionPlan> {
        self.control_plane_action.as_ref()
    }

    pub const fn correlation_context(&self) -> &CorrelationContext {
        &self.correlation
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
            .field(
                "control_plane_action",
                &self.control_plane_action.as_ref().map(|_| "<redacted>"),
            )
            .field("correlation", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationRequestContextError {
    UnknownRoute,
    Trace(GatewayHttpPlanError),
}

impl fmt::Debug for ApplicationRequestContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRoute => f.write_str("UnknownRoute"),
            Self::Trace(_) => f.debug_tuple("Trace").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationRequestContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRoute => f.write_str("application route is not available"),
            Self::Trace(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationRequestContextError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Trace(error) => Some(error),
            Self::UnknownRoute => None,
        }
    }
}

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

pub fn plan_application_request_context<'a>(
    target: &'a CanonicalRequestTarget,
    request_id: RequestId,
    deadline: ApplicationRequestDeadline,
    headers: &[GatewayHttpHeader],
) -> Result<ApplicationRequestContext<'a>, ApplicationRequestContextError> {
    let route =
        classify_request_target(target).ok_or(ApplicationRequestContextError::UnknownRoute)?;
    let trace_context =
        trace_context_from_headers(headers).map_err(ApplicationRequestContextError::Trace)?;
    let correlation = match trace_context.as_ref() {
        Some(trace) => CorrelationContext::new(request_id).with_trace_id(trace.trace_id.clone()),
        None => CorrelationContext::new(request_id),
    };
    Ok(ApplicationRequestContext {
        target,
        request_id,
        deadline,
        route: route.kind,
        plane: route.plane,
        required_credential_scope: required_credential_scope_for_plane(route.plane),
        trace_context,
        correlation,
        metadata: ApplicationRequestMetadata::from_headers(headers),
    })
}

/// Authenticates transport-verified evidence against the already canonicalized request.
///
/// The request context supplies the authoritative route credential scope. The
/// evidence adapter cannot reparse the target or choose a different scope.
pub fn plan_application_request_authentication_from_evidence(
    request: ApplicationRequestContext<'_>,
    evidence: Option<VerifiedCredentialEvidence>,
    anonymous_allowed: bool,
) -> Result<ApplicationAuthenticatedRequestContext<'_>, VerifiedCredentialAuthenticationError> {
    let principal = authenticate_verified_credential(VerifiedCredentialAuthenticationRequest {
        evidence,
        required_scope: request.required_credential_scope,
        anonymous_allowed,
    })?;
    Ok(ApplicationAuthenticatedRequestContext { request, principal })
}

pub fn plan_application_data_plane_authorization(
    authenticated: ApplicationAuthenticatedRequestContext<'_>,
) -> Result<ApplicationAuthorizedRequestContext<'_>, ApplicationRequestAuthorizationError> {
    if authenticated.request.plane != GatewayHttpRoutePlane::DataPlane {
        return Err(ApplicationRequestAuthorizationError::WrongPlane);
    }
    let Some(principal) = authenticated.principal.as_ref() else {
        return Ok(application_authorized_request_context(
            authenticated,
            None,
            None,
        ));
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
    Ok(application_authorized_request_context(
        authenticated,
        Some(tenant),
        None,
    ))
}

pub fn plan_application_control_plane_authorization(
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
    let plan = match decide_control_plane_action(action) {
        ControlPlaneDecision::Authorized(plan) => plan,
        ControlPlaneDecision::Denied { error, .. } => {
            return Err(ApplicationRequestAuthorizationError::ControlPlane(error));
        }
    };
    Ok(application_authorized_request_context(
        authenticated,
        Some(plan.tenant),
        Some(plan),
    ))
}

fn application_authorized_request_context(
    authenticated: ApplicationAuthenticatedRequestContext<'_>,
    tenant: Option<TenantContext>,
    control_plane_action: Option<ControlPlaneActionPlan>,
) -> ApplicationAuthorizedRequestContext<'_> {
    let correlation = match tenant {
        Some(tenant) => authenticated
            .request
            .correlation
            .clone()
            .with_tenant_id(tenant.tenant_id),
        None => authenticated.request.correlation.clone(),
    };
    ApplicationAuthorizedRequestContext {
        authenticated,
        tenant,
        control_plane_action,
        correlation,
    }
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
    use std::time::Duration;

    const REQUEST_ID: &str = "00000000-0000-7000-8000-000000000004";

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

    fn evidence(principal: Principal) -> Option<VerifiedCredentialEvidence> {
        Some(VerifiedCredentialEvidence::Principal(principal))
    }

    fn request_context<'a>(target: &'a CanonicalRequestTarget) -> ApplicationRequestContext<'a> {
        request_context_with_headers(target, &[]).unwrap()
    }

    fn request_context_with_headers<'a>(
        target: &'a CanonicalRequestTarget,
        headers: &[GatewayHttpHeader],
    ) -> Result<ApplicationRequestContext<'a>, ApplicationRequestContextError> {
        plan_application_request_context(
            target,
            id::<RequestId>(REQUEST_ID),
            ApplicationRequestDeadline::at(Instant::now() + Duration::from_secs(30)),
            headers,
        )
    }

    #[test]
    fn canonical_context_and_verified_evidence_are_one_scope_gate() {
        let data_target = CanonicalRequestTarget::parse("/v1/responses?stream=true").unwrap();
        let data = request_context(&data_target);
        assert_eq!(data.target().path_and_query(), "/v1/responses?stream=true");
        assert_eq!(data.plane(), GatewayHttpRoutePlane::DataPlane);
        let debug = format!("{data:?}");
        assert!(!debug.contains("/v1/responses"));
        assert!(!debug.contains("stream=true"));
        let authenticated = plan_application_request_authentication_from_evidence(
            data.clone(),
            evidence(principal(CredentialScope::DataPlane, Role::Operator)),
            false,
        )
        .unwrap();
        assert!(std::ptr::eq(authenticated.request().target(), &data_target,));
        assert!(
            plan_application_request_authentication_from_evidence(
                data,
                evidence(principal(CredentialScope::ControlPlane, Role::Admin)),
                false,
            )
            .is_err()
        );

        let health_target = CanonicalRequestTarget::parse("/readyz").unwrap();
        let health = request_context(&health_target);
        assert_eq!(health.plane(), GatewayHttpRoutePlane::Health);
        assert!(plan_application_request_authentication_from_evidence(health, None, true).is_ok());
    }

    #[test]
    fn unknown_route_never_reaches_evidence_authentication() {
        let target = CanonicalRequestTarget::parse("/v1/not-supported").unwrap();
        assert_eq!(
            request_context_with_headers(&target, &[]),
            Err(ApplicationRequestContextError::UnknownRoute),
        );
    }

    #[test]
    fn evidence_authorization_binds_data_plane_scope_role_and_tenant() {
        let target = CanonicalRequestTarget::parse("/v1/responses").unwrap();
        let request = request_context(&target);
        let operator = principal(CredentialScope::DataPlane, Role::Operator);
        let expected_tenant = operator.tenant_id.unwrap();
        let authenticated = plan_application_request_authentication_from_evidence(
            request.clone(),
            evidence(operator),
            false,
        )
        .unwrap();
        let authorized = plan_application_data_plane_authorization(authenticated).unwrap();
        assert_eq!(
            authorized.tenant_context().unwrap().tenant_id,
            expected_tenant
        );
        assert!(authorized.control_plane_action().is_none());
        assert!(!format!("{authorized:?}").contains(&expected_tenant.to_string()));

        let viewer = plan_application_request_authentication_from_evidence(
            request.clone(),
            evidence(principal(CredentialScope::DataPlane, Role::Viewer)),
            false,
        )
        .unwrap();
        assert!(
            plan_application_data_plane_authorization(viewer).is_err(),
            "data-plane viewer must not reach admission",
        );
        assert!(
            plan_application_request_authentication_from_evidence(
                request.clone(),
                evidence(principal(CredentialScope::ControlPlane, Role::Admin)),
                false,
            )
            .is_err(),
            "control-plane scope must not enter the data plane",
        );
        assert!(
            plan_application_request_authentication_from_evidence(request, None, false).is_err(),
            "anonymous requests must not enter a configured data plane",
        );
    }

    #[test]
    fn evidence_authorization_binds_control_plane_principal_and_tenant() {
        let target = CanonicalRequestTarget::parse("/admin/keys").unwrap();
        let request = request_context(&target);
        let admin = principal(CredentialScope::ControlPlane, Role::Admin);
        let tenant_id = admin.tenant_id.unwrap();
        let action = |principal: Principal, resource_tenant_id| ControlPlaneActionRequest {
            principal,
            operation: ControlPlaneOperation::VirtualKeyCreate,
            resource: ControlPlaneResourceRef::new(
                resource_tenant_id,
                ResourceKind::VirtualKey,
                Some("key-1"),
            ),
            occurred_at_unix_ms: 1,
        };

        assert!(
            plan_application_request_authentication_from_evidence(request.clone(), None, false)
                .is_err(),
            "anonymous requests must not enter the control plane",
        );
        assert!(
            plan_application_request_authentication_from_evidence(
                request.clone(),
                evidence(principal(CredentialScope::DataPlane, Role::Operator)),
                false,
            )
            .is_err(),
            "data-plane scope must not enter the control plane",
        );

        let authenticated = plan_application_request_authentication_from_evidence(
            request.clone(),
            evidence(admin.clone()),
            false,
        )
        .unwrap();
        let authorized = plan_application_control_plane_authorization(
            authenticated,
            action(admin.clone(), tenant_id),
        )
        .unwrap();
        assert_eq!(authorized.tenant_context().unwrap().tenant_id, tenant_id);
        let authorized_action = authorized.control_plane_action().unwrap();
        assert_eq!(
            authorized_action.operation,
            ControlPlaneOperation::VirtualKeyCreate
        );
        assert_eq!(
            authorized_action.requirement.resource,
            ResourceKind::VirtualKey
        );
        assert_eq!(authorized_action.tenant.tenant_id, tenant_id);
        assert_eq!(authorized_action.audit_event.resource.kind, "virtual_key");
        assert_eq!(
            authorized_action.audit_event.resource.id.as_deref(),
            Some("key-1")
        );
        assert_eq!(
            authorized_action.audit_event.resource.tenant_id,
            Some(tenant_id)
        );
        let debug = format!("{authorized:?}");
        assert!(!debug.contains(&tenant_id.to_string()));
        assert!(!debug.contains("virtual_key"));

        let authenticated = plan_application_request_authentication_from_evidence(
            request.clone(),
            evidence(admin.clone()),
            false,
        )
        .unwrap();
        assert_eq!(
            plan_application_control_plane_authorization(
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
        let authenticated = plan_application_request_authentication_from_evidence(
            request.clone(),
            evidence(viewer.clone()),
            false,
        )
        .unwrap();
        assert!(
            plan_application_control_plane_authorization(authenticated, action(viewer, tenant_id),)
                .is_err(),
            "control-plane viewer must not mutate virtual keys",
        );

        let authenticated = plan_application_request_authentication_from_evidence(
            request,
            evidence(admin.clone()),
            false,
        )
        .unwrap();
        let foreign_tenant = id::<TenantId>("00000000-0000-7000-8000-000000000003");
        let error = plan_application_control_plane_authorization(
            authenticated,
            action(admin, foreign_tenant),
        )
        .expect_err("cross-tenant control-plane access must fail closed");
        assert!(!format!("{error:?}").contains(&foreign_tenant.to_string()));
    }
}

#[cfg(test)]
#[path = "request_context_snapshot_tests.rs"]
mod snapshot_tests;
