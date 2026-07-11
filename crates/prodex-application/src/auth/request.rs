//! Request authentication planning.

use super::super::*;

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRequestAuthenticationRequest<'a> {
    pub http: GatewayHttpRequestMeta,
    pub oidc_policy: &'a OidcValidationPolicy,
    pub jwks_snapshot: Option<&'a JwksCacheSnapshot>,
    pub role_mapper: &'a ExplicitRoleMapper,
    pub claims: TokenClaims,
    pub now_unix_ms: u64,
}

impl fmt::Debug for ApplicationRequestAuthenticationRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRequestAuthenticationRequest")
            .field("http", &"<redacted>")
            .field("oidc_policy", &"<redacted>")
            .field(
                "jwks_snapshot",
                &self.jwks_snapshot.as_ref().map(|_| "<redacted>"),
            )
            .field("role_mapper", &"<redacted>")
            .field("claims", &"<redacted>")
            .field("now_unix_ms", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationRequestAuthenticationPlan {
    pub http: GatewayHttpPlan,
    pub principal: Principal,
    pub required_scope: CredentialScope,
}

impl fmt::Debug for ApplicationRequestAuthenticationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRequestAuthenticationPlan")
            .field("http", &"<redacted>")
            .field("principal", &"<redacted>")
            .field("required_scope", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationRequestAuthenticationError {
    Http(GatewayHttpPlanError),
    WrongRoute(GatewayHttpRouteKind),
    Authentication(prodex_authn::AuthenticationError),
    CredentialScopeMismatch {
        route: GatewayHttpRouteKind,
        actual: CredentialScope,
        required: CredentialScope,
    },
}

impl fmt::Debug for ApplicationRequestAuthenticationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(_) => f.debug_tuple("Http").field(&"<redacted>").finish(),
            Self::WrongRoute(_) => f.debug_tuple("WrongRoute").field(&"<redacted>").finish(),
            Self::Authentication(_) => f
                .debug_tuple("Authentication")
                .field(&"<redacted>")
                .finish(),
            Self::CredentialScopeMismatch { .. } => f
                .debug_struct("CredentialScopeMismatch")
                .field("route", &"<redacted>")
                .field("actual", &"<redacted>")
                .field("required", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for ApplicationRequestAuthenticationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(err) => err.fmt(f),
            Self::WrongRoute(_) => write!(f, "request authentication is denied"),
            Self::Authentication(err) => err.fmt(f),
            Self::CredentialScopeMismatch { .. } => {
                write!(f, "request authentication is denied")
            }
        }
    }
}

impl Error for ApplicationRequestAuthenticationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRequestAuthenticationErrorResponsePlan {
    pub http: Option<GatewayHttpErrorResponsePlan>,
    pub authentication: Option<AuthenticationErrorResponsePlan>,
}

pub fn plan_application_request_authentication_error_response(
    error: &ApplicationRequestAuthenticationError,
) -> ApplicationRequestAuthenticationErrorResponsePlan {
    match error {
        ApplicationRequestAuthenticationError::Http(error) => {
            ApplicationRequestAuthenticationErrorResponsePlan {
                http: Some(plan_gateway_http_error_response(error)),
                authentication: None,
            }
        }
        ApplicationRequestAuthenticationError::Authentication(error) => {
            ApplicationRequestAuthenticationErrorResponsePlan {
                http: None,
                authentication: Some(plan_authentication_error_response(error)),
            }
        }
        ApplicationRequestAuthenticationError::WrongRoute(_)
        | ApplicationRequestAuthenticationError::CredentialScopeMismatch { .. } => {
            ApplicationRequestAuthenticationErrorResponsePlan {
                http: None,
                authentication: Some(AuthenticationErrorResponsePlan {
                    status: prodex_authn::AuthenticationErrorStatus::Unauthorized,
                    code: "credential_scope_not_allowed",
                    message: "credential scope is not allowed for this endpoint",
                }),
            }
        }
    }
}

pub fn plan_application_request_authentication(
    policy: GatewayHttpPolicy,
    request: ApplicationRequestAuthenticationRequest<'_>,
) -> Result<ApplicationRequestAuthenticationPlan, ApplicationRequestAuthenticationError> {
    let http = plan_gateway_http_request(policy, request.http)
        .map_err(ApplicationRequestAuthenticationError::Http)?;
    let required_scope = required_credential_scope_for_route(http.route).ok_or(
        ApplicationRequestAuthenticationError::WrongRoute(http.route),
    )?;
    let principal = authenticate_oidc_claims(
        request.oidc_policy,
        request.jwks_snapshot,
        request.role_mapper,
        request.claims,
        request.now_unix_ms,
    )
    .map_err(ApplicationRequestAuthenticationError::Authentication)?;
    if principal.credential_scope != required_scope {
        return Err(
            ApplicationRequestAuthenticationError::CredentialScopeMismatch {
                route: http.route,
                actual: principal.credential_scope,
                required: required_scope,
            },
        );
    }
    Ok(ApplicationRequestAuthenticationPlan {
        http,
        principal,
        required_scope,
    })
}

fn required_credential_scope_for_route(route: GatewayHttpRouteKind) -> Option<CredentialScope> {
    route
        .plane()
        .and_then(request_context::required_credential_scope_for_plane)
}
