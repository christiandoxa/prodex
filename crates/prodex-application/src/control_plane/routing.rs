//! HTTP route, precondition, pagination, and idempotency planning.

use super::super::*;
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneIdempotencyRequest {
    pub action: ControlPlaneActionRequest,
    pub idempotency_key: Option<IdempotencyKey>,
    pub request_fingerprint: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneIdempotencyPlan {
    pub action: ControlPlaneActionRequest,
    pub operation: Option<IdempotentOperation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneHttpRoutePlan {
    pub http: GatewayControlPlaneRoutePlan,
    pub operation: ControlPlaneOperation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneHttpRouteError {
    Route(GatewayControlPlaneRouteError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneHttpRouteErrorStatus {
    BadRequest,
    MethodNotAllowed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneHttpRouteErrorResponsePlan {
    pub status: ApplicationControlPlaneHttpRouteErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Display for ApplicationControlPlaneHttpRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Route(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationControlPlaneHttpRouteError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneIdempotencyError {
    IdempotencyKeyRequired,
    IdempotencyKeyInvalid(prodex_gateway_http::GatewayHttpIdempotencyKeyError),
    RequestFingerprintInvalid(prodex_gateway_http::GatewayHttpRequestFingerprintError),
    RequestFingerprintDomainInvalid(IdempotentOperationError),
    HttpRoute(ApplicationControlPlaneHttpRouteError),
    OperationMismatch {
        route_operation: ControlPlaneOperation,
        action_operation: ControlPlaneOperation,
    },
    ReplayConflict(IdempotencyConflict),
    LookupRowInvalid(IdempotencyRecordLookupRowError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlanePageRequestPlan {
    pub action: ControlPlaneActionRequest,
    pub page_request: PageRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlanePreconditionPlan {
    pub action: ControlPlaneActionRequest,
    pub entity_tag: Option<EntityTag>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlanePageRequestError {
    HttpRoute(ApplicationControlPlaneHttpRouteError),
    OperationMismatch {
        route_operation: ControlPlaneOperation,
        action_operation: ControlPlaneOperation,
    },
    Pagination(prodex_gateway_http::GatewayHttpPaginationQueryError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlanePreconditionError {
    HttpRoute(ApplicationControlPlaneHttpRouteError),
    OperationMismatch {
        route_operation: ControlPlaneOperation,
        action_operation: ControlPlaneOperation,
    },
    EntityTag(prodex_gateway_http::GatewayHttpEntityTagError),
}

impl fmt::Display for ApplicationControlPlanePageRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpRoute(error) => error.fmt(f),
            Self::OperationMismatch { .. } => {
                write!(
                    f,
                    "control-plane HTTP route does not match action operation"
                )
            }
            Self::Pagination(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationControlPlanePageRequestError {}

impl fmt::Display for ApplicationControlPlanePreconditionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpRoute(error) => error.fmt(f),
            Self::OperationMismatch { .. } => {
                write!(
                    f,
                    "control-plane HTTP route does not match action operation"
                )
            }
            Self::EntityTag(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationControlPlanePreconditionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlanePageRequestErrorStatus {
    BadRequest,
    MethodNotAllowed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlanePageRequestErrorResponsePlan {
    pub status: ApplicationControlPlanePageRequestErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlanePreconditionErrorStatus {
    BadRequest,
    MethodNotAllowed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlanePreconditionErrorResponsePlan {
    pub status: ApplicationControlPlanePreconditionErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Display for ApplicationControlPlaneIdempotencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdempotencyKeyRequired => {
                write!(
                    f,
                    "idempotency key is required for mutating control-plane operation"
                )
            }
            Self::IdempotencyKeyInvalid(error) => error.fmt(f),
            Self::RequestFingerprintInvalid(error) => error.fmt(f),
            Self::RequestFingerprintDomainInvalid(error) => error.fmt(f),
            Self::HttpRoute(error) => error.fmt(f),
            Self::OperationMismatch { .. } => {
                write!(
                    f,
                    "control-plane HTTP route does not match action operation"
                )
            }
            Self::ReplayConflict(error) => error.fmt(f),
            Self::LookupRowInvalid(error) => error.fmt(f),
        }
    }
}

impl Error for ApplicationControlPlaneIdempotencyError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneIdempotencyErrorStatus {
    BadRequest,
    Conflict,
    MethodNotAllowed,
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneIdempotencyErrorResponsePlan {
    pub status: ApplicationControlPlaneIdempotencyErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_control_plane_idempotency_error_response(
    error: &ApplicationControlPlaneIdempotencyError,
) -> ApplicationControlPlaneIdempotencyErrorResponsePlan {
    match error {
        ApplicationControlPlaneIdempotencyError::IdempotencyKeyRequired => {
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: ApplicationControlPlaneIdempotencyErrorStatus::BadRequest,
                code: "control_plane_idempotency_key_required",
                message: "control-plane idempotency key is required",
            }
        }
        ApplicationControlPlaneIdempotencyError::IdempotencyKeyInvalid(error) => {
            let response =
                prodex_gateway_http::plan_gateway_http_idempotency_key_error_response(error);
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: match response.status {
                    prodex_gateway_http::GatewayHttpIdempotencyKeyErrorStatus::BadRequest => {
                        ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneIdempotencyError::RequestFingerprintInvalid(error) => {
            let response =
                prodex_gateway_http::plan_gateway_http_request_fingerprint_error_response(error);
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: match response.status {
                    prodex_gateway_http::GatewayHttpRequestFingerprintErrorStatus::BadRequest => {
                        ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneIdempotencyError::RequestFingerprintDomainInvalid(_) => {
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: ApplicationControlPlaneIdempotencyErrorStatus::BadRequest,
                code: "request_fingerprint_invalid",
                message: "request fingerprint is invalid",
            }
        }
        ApplicationControlPlaneIdempotencyError::HttpRoute(error) => {
            application_control_plane_route_response_from_gateway(error)
        }
        ApplicationControlPlaneIdempotencyError::OperationMismatch { .. } => {
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: ApplicationControlPlaneIdempotencyErrorStatus::BadRequest,
                code: "control_plane_route_invalid",
                message: "control-plane route is invalid",
            }
        }
        ApplicationControlPlaneIdempotencyError::ReplayConflict(error) => {
            let response = plan_idempotency_conflict_response(error);
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: match response.status {
                    IdempotencyConflictStatus::Conflict => {
                        ApplicationControlPlaneIdempotencyErrorStatus::Conflict
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneIdempotencyError::LookupRowInvalid(error) => {
            let response = materialize_idempotency_record_lookup_row_error_response(error);
            ApplicationControlPlaneIdempotencyErrorResponsePlan {
                status: match response.status {
                    StoragePlanErrorStatus::BadRequest
                    | StoragePlanErrorStatus::InvalidConfiguration => {
                        ApplicationControlPlaneIdempotencyErrorStatus::ServiceUnavailable
                    }
                },
                code: "control_plane_idempotency_lookup_invalid",
                message: "control-plane idempotency lookup is invalid",
            }
        }
    }
}

fn application_control_plane_route_response_from_gateway(
    error: &ApplicationControlPlaneHttpRouteError,
) -> ApplicationControlPlaneIdempotencyErrorResponsePlan {
    let response = match error {
        ApplicationControlPlaneHttpRouteError::Route(error) => {
            plan_gateway_control_plane_route_error_response(error)
        }
    };
    application_control_plane_route_response_from_gateway_plan(response)
}

fn application_control_plane_route_response_from_gateway_plan(
    response: GatewayControlPlaneRouteErrorResponsePlan,
) -> ApplicationControlPlaneIdempotencyErrorResponsePlan {
    ApplicationControlPlaneIdempotencyErrorResponsePlan {
        status: match response.status {
            GatewayControlPlaneRouteErrorStatus::BadRequest => {
                ApplicationControlPlaneIdempotencyErrorStatus::BadRequest
            }
            GatewayControlPlaneRouteErrorStatus::MethodNotAllowed => {
                ApplicationControlPlaneIdempotencyErrorStatus::MethodNotAllowed
            }
        },
        code: response.code,
        message: response.message,
    }
}

pub fn plan_application_control_plane_page_request_error_response(
    error: &ApplicationControlPlanePageRequestError,
) -> ApplicationControlPlanePageRequestErrorResponsePlan {
    match error {
        ApplicationControlPlanePageRequestError::HttpRoute(error) => {
            let response = match error {
                ApplicationControlPlaneHttpRouteError::Route(error) => {
                    plan_gateway_control_plane_route_error_response(error)
                }
            };
            ApplicationControlPlanePageRequestErrorResponsePlan {
                status: match response.status {
                    GatewayControlPlaneRouteErrorStatus::BadRequest => {
                        ApplicationControlPlanePageRequestErrorStatus::BadRequest
                    }
                    GatewayControlPlaneRouteErrorStatus::MethodNotAllowed => {
                        ApplicationControlPlanePageRequestErrorStatus::MethodNotAllowed
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlanePageRequestError::OperationMismatch { .. } => {
            ApplicationControlPlanePageRequestErrorResponsePlan {
                status: ApplicationControlPlanePageRequestErrorStatus::BadRequest,
                code: "control_plane_route_invalid",
                message: "control-plane route is invalid",
            }
        }
        ApplicationControlPlanePageRequestError::Pagination(error) => {
            let response = plan_gateway_http_pagination_query_error_response(error);
            ApplicationControlPlanePageRequestErrorResponsePlan {
                status: match response.status {
                    prodex_gateway_http::GatewayHttpPaginationQueryErrorStatus::BadRequest => {
                        ApplicationControlPlanePageRequestErrorStatus::BadRequest
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
    }
}

pub fn plan_application_control_plane_precondition_error_response(
    error: &ApplicationControlPlanePreconditionError,
) -> ApplicationControlPlanePreconditionErrorResponsePlan {
    match error {
        ApplicationControlPlanePreconditionError::HttpRoute(error) => {
            let response = match error {
                ApplicationControlPlaneHttpRouteError::Route(error) => {
                    plan_gateway_control_plane_route_error_response(error)
                }
            };
            ApplicationControlPlanePreconditionErrorResponsePlan {
                status: match response.status {
                    GatewayControlPlaneRouteErrorStatus::BadRequest => {
                        ApplicationControlPlanePreconditionErrorStatus::BadRequest
                    }
                    GatewayControlPlaneRouteErrorStatus::MethodNotAllowed => {
                        ApplicationControlPlanePreconditionErrorStatus::MethodNotAllowed
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlanePreconditionError::OperationMismatch { .. } => {
            ApplicationControlPlanePreconditionErrorResponsePlan {
                status: ApplicationControlPlanePreconditionErrorStatus::BadRequest,
                code: "control_plane_route_invalid",
                message: "control-plane route is invalid",
            }
        }
        ApplicationControlPlanePreconditionError::EntityTag(error) => {
            let response = plan_gateway_http_entity_tag_error_response(error);
            ApplicationControlPlanePreconditionErrorResponsePlan {
                status: match response.status {
                    prodex_gateway_http::GatewayHttpEntityTagErrorStatus::BadRequest => {
                        ApplicationControlPlanePreconditionErrorStatus::BadRequest
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
    }
}

pub fn plan_application_control_plane_idempotency(
    request: ApplicationControlPlaneIdempotencyRequest,
) -> Result<ApplicationControlPlaneIdempotencyPlan, ApplicationControlPlaneIdempotencyError> {
    let operation = if request.action.operation.requires_idempotency() {
        let key = request
            .idempotency_key
            .ok_or(ApplicationControlPlaneIdempotencyError::IdempotencyKeyRequired)?;
        let key = application_control_plane_principal_scoped_idempotency_key(
            request.action.principal.id,
            &key,
        );
        request
            .action
            .idempotent_operation(key, request.request_fingerprint)
            .map_err(ApplicationControlPlaneIdempotencyError::RequestFingerprintDomainInvalid)?
    } else {
        None
    };
    Ok(ApplicationControlPlaneIdempotencyPlan {
        action: request.action,
        operation,
    })
}

pub fn application_control_plane_principal_scoped_idempotency_key(
    principal_id: prodex_domain::PrincipalId,
    presented_key: &IdempotencyKey,
) -> IdempotencyKey {
    let digest = Sha256::digest(presented_key.as_str().as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    IdempotencyKey::new(format!("cp:v1:{principal_id}:{hex}"))
        .expect("principal-scoped control-plane idempotency key is valid")
}

pub fn plan_application_control_plane_http_route(
    http: &GatewayHttpRequestMeta,
) -> Result<ApplicationControlPlaneHttpRoutePlan, ApplicationControlPlaneHttpRouteError> {
    let route =
        plan_control_plane_route(http).map_err(ApplicationControlPlaneHttpRouteError::Route)?;
    Ok(ApplicationControlPlaneHttpRoutePlan {
        operation: control_plane_operation_from_gateway_route(route.operation),
        http: route,
    })
}

pub fn plan_application_control_plane_http_route_error_response(
    error: &ApplicationControlPlaneHttpRouteError,
) -> ApplicationControlPlaneHttpRouteErrorResponsePlan {
    let response = match error {
        ApplicationControlPlaneHttpRouteError::Route(error) => {
            plan_gateway_control_plane_route_error_response(error)
        }
    };
    ApplicationControlPlaneHttpRouteErrorResponsePlan {
        status: match response.status {
            GatewayControlPlaneRouteErrorStatus::BadRequest => {
                ApplicationControlPlaneHttpRouteErrorStatus::BadRequest
            }
            GatewayControlPlaneRouteErrorStatus::MethodNotAllowed => {
                ApplicationControlPlaneHttpRouteErrorStatus::MethodNotAllowed
            }
        },
        code: response.code,
        message: response.message,
    }
}

pub fn plan_application_control_plane_idempotency_from_http(
    action: ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
    request_fingerprint: impl Into<String>,
) -> Result<ApplicationControlPlaneIdempotencyPlan, ApplicationControlPlaneIdempotencyError> {
    validate_control_plane_http_action(&action, http)?;
    let idempotency_key = idempotency_key_from_headers(&http.headers)
        .map_err(ApplicationControlPlaneIdempotencyError::IdempotencyKeyInvalid)?;
    plan_application_control_plane_idempotency(ApplicationControlPlaneIdempotencyRequest {
        action,
        idempotency_key,
        request_fingerprint: request_fingerprint.into(),
    })
}

pub fn plan_application_control_plane_idempotency_from_http_digest(
    action: ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
    body_digest: impl AsRef<str>,
) -> Result<ApplicationControlPlaneIdempotencyPlan, ApplicationControlPlaneIdempotencyError> {
    validate_control_plane_http_action(&action, http)?;
    let request_fingerprint = control_plane_request_fingerprint(http, body_digest)
        .map_err(ApplicationControlPlaneIdempotencyError::RequestFingerprintInvalid)?;
    plan_application_control_plane_idempotency_from_http(action, http, request_fingerprint)
}

pub fn plan_application_control_plane_page_request_from_http_query(
    action: ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
    query: impl AsRef<str>,
) -> Result<ApplicationControlPlanePageRequestPlan, ApplicationControlPlanePageRequestError> {
    validate_control_plane_http_action_for_page_request(&action, http)?;
    let page_request = page_request_from_query(query.as_ref())
        .map_err(ApplicationControlPlanePageRequestError::Pagination)?;
    Ok(ApplicationControlPlanePageRequestPlan {
        action,
        page_request,
    })
}

pub fn plan_application_control_plane_precondition_from_http(
    action: ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<ApplicationControlPlanePreconditionPlan, ApplicationControlPlanePreconditionError> {
    validate_control_plane_http_action_for_precondition(&action, http)?;
    let entity_tag = entity_tag_from_if_match_headers(&http.headers)
        .map_err(ApplicationControlPlanePreconditionError::EntityTag)?;
    Ok(ApplicationControlPlanePreconditionPlan { action, entity_tag })
}

fn validate_control_plane_http_action(
    action: &ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<(), ApplicationControlPlaneIdempotencyError> {
    let route = plan_application_control_plane_http_route(http)
        .map_err(ApplicationControlPlaneIdempotencyError::HttpRoute)?;
    if route.operation != action.operation {
        if control_plane_operations_share_route_family(route.operation, action.operation)
            && !control_plane_operation_allows_http_method(action.operation, http.method)
        {
            return Err(ApplicationControlPlaneIdempotencyError::HttpRoute(
                ApplicationControlPlaneHttpRouteError::Route(
                    GatewayControlPlaneRouteError::MethodNotAllowed {
                        operation: gateway_operation_from_control_plane_action(action.operation),
                        method: http.method,
                    },
                ),
            ));
        }
        return Err(ApplicationControlPlaneIdempotencyError::OperationMismatch {
            route_operation: route.operation,
            action_operation: action.operation,
        });
    }
    Ok(())
}

fn validate_control_plane_http_action_for_page_request(
    action: &ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<(), ApplicationControlPlanePageRequestError> {
    let route = plan_application_control_plane_http_route(http)
        .map_err(ApplicationControlPlanePageRequestError::HttpRoute)?;
    if route.operation != action.operation {
        return Err(ApplicationControlPlanePageRequestError::OperationMismatch {
            route_operation: route.operation,
            action_operation: action.operation,
        });
    }
    Ok(())
}

fn validate_control_plane_http_action_for_precondition(
    action: &ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<(), ApplicationControlPlanePreconditionError> {
    let route = plan_application_control_plane_http_route(http)
        .map_err(ApplicationControlPlanePreconditionError::HttpRoute)?;
    if route.operation != action.operation {
        return Err(
            ApplicationControlPlanePreconditionError::OperationMismatch {
                route_operation: route.operation,
                action_operation: action.operation,
            },
        );
    }
    Ok(())
}

pub(crate) fn validate_control_plane_http_action_for_audit(
    action: &ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<ApplicationControlPlaneHttpRoutePlan, ApplicationControlPlaneAuditError> {
    let route = plan_application_control_plane_http_route(http)
        .map_err(ApplicationControlPlaneAuditError::HttpRoute)?;
    if route.operation != action.operation {
        return Err(ApplicationControlPlaneAuditError::OperationMismatch {
            route_operation: route.operation,
            action_operation: action.operation,
        });
    }
    if !route.http.requires_audit || !action.operation.requires_immutable_audit() {
        return Err(ApplicationControlPlaneAuditError::AuditNotRequired {
            route_operation: route.http.operation,
            action_operation: action.operation,
        });
    }
    Ok(route)
}

fn control_plane_operation_from_gateway_route(
    operation: GatewayControlPlaneOperation,
) -> ControlPlaneOperation {
    match operation {
        GatewayControlPlaneOperation::GatewayAdminRead => ControlPlaneOperation::GatewayAdminRead,
        GatewayControlPlaneOperation::RouteExplain => ControlPlaneOperation::RouteExplain,
        GatewayControlPlaneOperation::TenantCreate => ControlPlaneOperation::TenantCreate,
        GatewayControlPlaneOperation::TenantUpdate => ControlPlaneOperation::TenantUpdate,
        GatewayControlPlaneOperation::UserInvite => ControlPlaneOperation::UserInvite,
        GatewayControlPlaneOperation::ScimUserRead => ControlPlaneOperation::ScimUserRead,
        GatewayControlPlaneOperation::ScimUserCreate => ControlPlaneOperation::ScimUserCreate,
        GatewayControlPlaneOperation::ScimUserUpdate => ControlPlaneOperation::ScimUserUpdate,
        GatewayControlPlaneOperation::ScimUserDelete => ControlPlaneOperation::ScimUserDelete,
        GatewayControlPlaneOperation::RoleBindingGrant => ControlPlaneOperation::RoleBindingGrant,
        GatewayControlPlaneOperation::RoleBindingRevoke => ControlPlaneOperation::RoleBindingRevoke,
        GatewayControlPlaneOperation::ServiceIdentityCreate => {
            ControlPlaneOperation::ServiceIdentityCreate
        }
        GatewayControlPlaneOperation::VirtualKeyRead => ControlPlaneOperation::VirtualKeyRead,
        GatewayControlPlaneOperation::VirtualKeyCreate => ControlPlaneOperation::VirtualKeyCreate,
        GatewayControlPlaneOperation::VirtualKeyUpdate => ControlPlaneOperation::VirtualKeyUpdate,
        GatewayControlPlaneOperation::VirtualKeyDelete => ControlPlaneOperation::VirtualKeyDelete,
        GatewayControlPlaneOperation::VirtualKeyRotateSecret => {
            ControlPlaneOperation::VirtualKeyRotateSecret
        }
        GatewayControlPlaneOperation::PolicyPublish => ControlPlaneOperation::PolicyPublish,
        GatewayControlPlaneOperation::ProviderCredentialRotate => {
            ControlPlaneOperation::ProviderCredentialRotate
        }
        GatewayControlPlaneOperation::BudgetUpdate => ControlPlaneOperation::BudgetUpdate,
        GatewayControlPlaneOperation::BillingRead => ControlPlaneOperation::BillingRead,
        GatewayControlPlaneOperation::AuditExport => ControlPlaneOperation::AuditExport,
        GatewayControlPlaneOperation::AuditRetentionPurge => {
            ControlPlaneOperation::AuditRetentionPurge
        }
        GatewayControlPlaneOperation::ConfigurationPublish => {
            ControlPlaneOperation::ConfigurationPublish
        }
    }
}

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
            | (PolicyPublish, PolicyPublish)
            | (ConfigurationPublish, ConfigurationPublish)
            | (BillingRead, BillingRead)
            | (
                AuditExport | AuditRetentionPurge,
                AuditExport | AuditRetentionPurge
            )
    )
}

fn gateway_operation_from_control_plane_action(
    operation: ControlPlaneOperation,
) -> GatewayControlPlaneOperation {
    match operation {
        ControlPlaneOperation::GatewayAdminRead => GatewayControlPlaneOperation::GatewayAdminRead,
        ControlPlaneOperation::RouteExplain => GatewayControlPlaneOperation::RouteExplain,
        ControlPlaneOperation::TenantCreate => GatewayControlPlaneOperation::TenantCreate,
        ControlPlaneOperation::TenantUpdate => GatewayControlPlaneOperation::TenantUpdate,
        ControlPlaneOperation::UserInvite => GatewayControlPlaneOperation::UserInvite,
        ControlPlaneOperation::ScimUserRead => GatewayControlPlaneOperation::ScimUserRead,
        ControlPlaneOperation::ScimUserCreate => GatewayControlPlaneOperation::ScimUserCreate,
        ControlPlaneOperation::ScimUserUpdate => GatewayControlPlaneOperation::ScimUserUpdate,
        ControlPlaneOperation::ScimUserDelete => GatewayControlPlaneOperation::ScimUserDelete,
        ControlPlaneOperation::RoleBindingGrant => GatewayControlPlaneOperation::RoleBindingGrant,
        ControlPlaneOperation::RoleBindingRevoke => GatewayControlPlaneOperation::RoleBindingRevoke,
        ControlPlaneOperation::ServiceIdentityCreate => {
            GatewayControlPlaneOperation::ServiceIdentityCreate
        }
        ControlPlaneOperation::VirtualKeyRead => GatewayControlPlaneOperation::VirtualKeyRead,
        ControlPlaneOperation::VirtualKeyCreate => GatewayControlPlaneOperation::VirtualKeyCreate,
        ControlPlaneOperation::VirtualKeyUpdate => GatewayControlPlaneOperation::VirtualKeyUpdate,
        ControlPlaneOperation::VirtualKeyDelete => GatewayControlPlaneOperation::VirtualKeyDelete,
        ControlPlaneOperation::VirtualKeyRotateSecret => {
            GatewayControlPlaneOperation::VirtualKeyRotateSecret
        }
        ControlPlaneOperation::ProviderCredentialRotate => {
            GatewayControlPlaneOperation::ProviderCredentialRotate
        }
        ControlPlaneOperation::BudgetUpdate => GatewayControlPlaneOperation::BudgetUpdate,
        ControlPlaneOperation::PolicyPublish => GatewayControlPlaneOperation::PolicyPublish,
        ControlPlaneOperation::ConfigurationPublish => {
            GatewayControlPlaneOperation::ConfigurationPublish
        }
        ControlPlaneOperation::BillingRead => GatewayControlPlaneOperation::BillingRead,
        ControlPlaneOperation::AuditExport => GatewayControlPlaneOperation::AuditExport,
        ControlPlaneOperation::AuditRetentionPurge => {
            GatewayControlPlaneOperation::AuditRetentionPurge
        }
    }
}

fn control_plane_operation_allows_http_method(
    operation: ControlPlaneOperation,
    method: prodex_gateway_http::GatewayHttpMethod,
) -> bool {
    use prodex_gateway_http::GatewayHttpMethod::{Delete, Get, Patch, Post};

    match operation {
        ControlPlaneOperation::GatewayAdminRead
        | ControlPlaneOperation::ScimUserRead
        | ControlPlaneOperation::VirtualKeyRead
        | ControlPlaneOperation::BillingRead => method == Get,
        ControlPlaneOperation::RouteExplain
        | ControlPlaneOperation::TenantCreate
        | ControlPlaneOperation::UserInvite
        | ControlPlaneOperation::ScimUserCreate
        | ControlPlaneOperation::RoleBindingGrant
        | ControlPlaneOperation::ServiceIdentityCreate
        | ControlPlaneOperation::VirtualKeyCreate
        | ControlPlaneOperation::VirtualKeyRotateSecret
        | ControlPlaneOperation::ProviderCredentialRotate
        | ControlPlaneOperation::PolicyPublish
        | ControlPlaneOperation::ConfigurationPublish
        | ControlPlaneOperation::AuditExport => method == Post,
        ControlPlaneOperation::TenantUpdate
        | ControlPlaneOperation::ScimUserUpdate
        | ControlPlaneOperation::VirtualKeyUpdate
        | ControlPlaneOperation::BudgetUpdate => method == Patch,
        ControlPlaneOperation::ScimUserDelete
        | ControlPlaneOperation::RoleBindingRevoke
        | ControlPlaneOperation::VirtualKeyDelete
        | ControlPlaneOperation::AuditRetentionPurge => method == Delete,
    }
}

pub fn plan_application_control_plane_idempotency_replay<R: Clone>(
    operation: &IdempotentOperation,
    existing: Option<&IdempotencyEntry<R>>,
) -> Result<IdempotencyReplayDecision<R>, ApplicationControlPlaneIdempotencyError> {
    decide_idempotency_replay(operation, existing)
        .map_err(ApplicationControlPlaneIdempotencyError::ReplayConflict)
}

pub fn plan_application_control_plane_idempotency_replay_from_lookup_row(
    operation: &IdempotentOperation,
    row: Option<IdempotencyRecordLookupRow>,
) -> Result<IdempotencyReplayDecision<Vec<u8>>, ApplicationControlPlaneIdempotencyError> {
    let entry = row
        .map(|row| materialize_idempotency_record_lookup_row(operation, row))
        .transpose()
        .map_err(ApplicationControlPlaneIdempotencyError::LookupRowInvalid)?;
    plan_application_control_plane_idempotency_replay(operation, entry.as_ref())
}
