//! Data-plane admission and quota-read plans.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationDataPlaneExecutionError {
    WrongPlane,
    Http(GatewayHttpPlanError),
}

impl fmt::Display for ApplicationDataPlaneExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongPlane => write!(f, "application route is not available"),
            Self::Http(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationDataPlaneExecutionError {}

pub fn plan_application_data_plane_execution(
    policy: GatewayHttpPolicy,
    authorized: &ApplicationAuthorizedRequestContext<'_>,
) -> Result<GatewayHttpExecutionPlan, ApplicationDataPlaneExecutionError> {
    let request = authorized.request();
    if request.plane() != prodex_gateway_http::GatewayHttpRoutePlane::DataPlane {
        return Err(ApplicationDataPlaneExecutionError::WrongPlane);
    }
    plan_gateway_http_execution(policy, request.route())
        .map_err(ApplicationDataPlaneExecutionError::Http)
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationDataPlaneRequest<R> {
    pub http: GatewayHttpRequestMeta,
    pub admission: GatewayAdmissionRequest<R>,
}

impl<R> fmt::Debug for ApplicationDataPlaneRequest<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationDataPlaneRequest")
            .field("http", &"<redacted>")
            .field("admission", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationDataPlanePlan {
    pub http: GatewayHttpPlan,
    pub admission: GatewayAdmissionPlan,
}

impl fmt::Debug for ApplicationDataPlanePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationDataPlanePlan")
            .field("http", &"<redacted>")
            .field("admission", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationDataPlaneError {
    Http(GatewayHttpPlanError),
    WrongRoute(GatewayHttpRouteKind),
    Admission(prodex_gateway_core::GatewayAdmissionError),
}

impl fmt::Debug for ApplicationDataPlaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(_) => f.debug_tuple("Http").field(&"<redacted>").finish(),
            Self::WrongRoute(_) => f.debug_tuple("WrongRoute").field(&"<redacted>").finish(),
            Self::Admission(_) => f.debug_tuple("Admission").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationDataPlaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(err) => err.fmt(f),
            Self::WrongRoute(_) => write!(f, "application route is not available"),
            Self::Admission(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationDataPlaneError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationDataPlaneErrorResponsePlan {
    pub http: Option<GatewayHttpErrorResponsePlan>,
    pub admission: Option<GatewayAdmissionErrorResponsePlan>,
}

pub fn plan_application_data_plane_error_response(
    error: &ApplicationDataPlaneError,
) -> ApplicationDataPlaneErrorResponsePlan {
    match error {
        ApplicationDataPlaneError::Http(error) => ApplicationDataPlaneErrorResponsePlan {
            http: Some(plan_gateway_http_error_response(error)),
            admission: None,
        },
        ApplicationDataPlaneError::Admission(error) => ApplicationDataPlaneErrorResponsePlan {
            http: None,
            admission: Some(plan_gateway_admission_error_response(error)),
        },
        ApplicationDataPlaneError::WrongRoute(_) => ApplicationDataPlaneErrorResponsePlan {
            http: Some(GatewayHttpErrorResponsePlan {
                status: prodex_gateway_http::GatewayHttpErrorStatus::BadRequest,
                code: "route_not_available",
                message: "route is not available for this endpoint",
            }),
            admission: None,
        },
    }
}

pub fn plan_application_data_plane<R>(
    policy: GatewayHttpPolicy,
    request: ApplicationDataPlaneRequest<R>,
) -> Result<ApplicationDataPlanePlan, ApplicationDataPlaneError>
where
    R: prodex_domain::TenantScopedResource,
{
    let http =
        plan_gateway_http_request(policy, request.http).map_err(ApplicationDataPlaneError::Http)?;
    if !http.route.is_provider_data_plane() {
        return Err(ApplicationDataPlaneError::WrongRoute(http.route));
    }
    let admission = plan_data_plane_admission(request.admission)
        .map_err(ApplicationDataPlaneError::Admission)?;
    Ok(ApplicationDataPlanePlan { http, admission })
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationQuotaReadRequest<R> {
    pub http: GatewayHttpRequestMeta,
    pub authorization: GatewayQuotaReadAuthorizationRequest<R>,
}

impl<R> fmt::Debug for ApplicationQuotaReadRequest<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationQuotaReadRequest")
            .field("http", &"<redacted>")
            .field("authorization", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationQuotaReadPlan {
    pub http: GatewayHttpPlan,
    pub authorization: GatewayQuotaReadAuthorizationPlan,
}

impl fmt::Debug for ApplicationQuotaReadPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationQuotaReadPlan")
            .field("http", &"<redacted>")
            .field("authorization", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ApplicationQuotaReadError {
    Http(GatewayHttpPlanError),
    WrongRoute(GatewayHttpRouteKind),
    Authorization(prodex_gateway_core::GatewayQuotaReadAuthorizationError),
}

impl fmt::Debug for ApplicationQuotaReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(_) => f.debug_tuple("Http").field(&"<redacted>").finish(),
            Self::WrongRoute(_) => f.debug_tuple("WrongRoute").field(&"<redacted>").finish(),
            Self::Authorization(_) => f.debug_tuple("Authorization").field(&"<redacted>").finish(),
        }
    }
}

impl fmt::Display for ApplicationQuotaReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(err) => err.fmt(f),
            Self::WrongRoute(_) => write!(f, "application route is not available"),
            Self::Authorization(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationQuotaReadError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationQuotaReadErrorResponsePlan {
    pub http: Option<GatewayHttpErrorResponsePlan>,
    pub authorization: Option<GatewayQuotaReadAuthorizationErrorResponsePlan>,
}

pub fn plan_application_quota_read_error_response(
    error: &ApplicationQuotaReadError,
) -> ApplicationQuotaReadErrorResponsePlan {
    match error {
        ApplicationQuotaReadError::Http(error) => ApplicationQuotaReadErrorResponsePlan {
            http: Some(plan_gateway_http_error_response(error)),
            authorization: None,
        },
        ApplicationQuotaReadError::Authorization(error) => ApplicationQuotaReadErrorResponsePlan {
            http: None,
            authorization: Some(plan_gateway_quota_read_authorization_error_response(error)),
        },
        ApplicationQuotaReadError::WrongRoute(_) => ApplicationQuotaReadErrorResponsePlan {
            http: Some(GatewayHttpErrorResponsePlan {
                status: prodex_gateway_http::GatewayHttpErrorStatus::BadRequest,
                code: "route_not_available",
                message: "route is not available for this endpoint",
            }),
            authorization: None,
        },
    }
}

pub fn plan_application_quota_read<R>(
    policy: GatewayHttpPolicy,
    request: ApplicationQuotaReadRequest<R>,
) -> Result<ApplicationQuotaReadPlan, ApplicationQuotaReadError>
where
    R: prodex_domain::TenantScopedResource,
{
    let http =
        plan_gateway_http_request(policy, request.http).map_err(ApplicationQuotaReadError::Http)?;
    if http.route != GatewayHttpRouteKind::DataPlaneQuota {
        return Err(ApplicationQuotaReadError::WrongRoute(http.route));
    }
    let authorization = plan_gateway_quota_read_authorization(request.authorization)
        .map_err(ApplicationQuotaReadError::Authorization)?;
    Ok(ApplicationQuotaReadPlan {
        http,
        authorization,
    })
}
