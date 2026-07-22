//! Authorization audit and trace planning.

use super::super::*;
use super::routing::validate_control_plane_http_action_for_audit;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlanePlan {
    pub decision: ControlPlaneDecision,
}

pub fn plan_application_control_plane(
    request: ControlPlaneActionRequest,
) -> ApplicationControlPlanePlan {
    ApplicationControlPlanePlan {
        decision: decide_control_plane_action(request),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationBreakGlassAuditRequest {
    pub durable_store: DurableStoreKind,
    pub action: ControlPlaneActionRequest,
    pub authorization: BreakGlassAuthorization,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditStoragePlan {
    Postgres(PostgresAppendOnlyAuditSqlPlan),
    Sqlite(SqliteAppendOnlyAuditSqlPlan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditPlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditHttpPlan {
    pub action: ControlPlaneActionRequest,
    pub route: GatewayControlPlaneRoutePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditCorrelationRequest {
    pub request_id: RequestId,
    pub call_id: Option<CallId>,
    pub http_policy: GatewayHttpPolicy,
    pub http: GatewayHttpRequestMeta,
    pub audit: ApplicationControlPlaneAuditPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditCorrelationPlan {
    pub correlation: CorrelationContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditEmissionSpanRequest {
    pub correlation: CorrelationContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditEmissionSpanPlan {
    pub span: SpanPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditPersistenceSpanRequest {
    pub correlation: CorrelationContext,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditPersistenceSpanPlan {
    pub span: SpanPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationBreakGlassAuditPlan {
    pub decision: ControlPlaneDecision,
    pub audit_storage: ApplicationControlPlaneAuditStoragePlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditError {
    HttpRoute(ApplicationControlPlaneHttpRouteError),
    OperationMismatch {
        route_operation: ControlPlaneOperation,
        action_operation: ControlPlaneOperation,
    },
    AuditNotRequired {
        route_operation: GatewayControlPlaneOperation,
        action_operation: ControlPlaneOperation,
    },
    Postgres(prodex_storage_postgres::PostgresStoragePlanError),
    Sqlite(prodex_storage_sqlite::SqliteStoragePlanError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditCorrelationError {
    Http(GatewayHttpPlanError),
    MissingTraceContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditEmissionSpanError {
    Span(SpanPlanError),
    MissingTenant,
    MissingAuditEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditPersistenceSpanError {
    Span(SpanPlanError),
    MissingTenant,
    MissingAuditEvent,
}

impl fmt::Display for ApplicationControlPlaneAuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpRoute(err) => err.fmt(f),
            Self::OperationMismatch { .. } => {
                write!(
                    f,
                    "control-plane HTTP route does not match action operation"
                )
            }
            Self::AuditNotRequired { .. } => {
                write!(f, "control-plane route does not require audit")
            }
            Self::Postgres(err) => err.fmt(f),
            Self::Sqlite(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationControlPlaneAuditError {}

impl fmt::Display for ApplicationControlPlaneAuditCorrelationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(error) => error.fmt(f),
            Self::MissingTraceContext => write!(f, "trace context is required for audit"),
        }
    }
}

impl Error for ApplicationControlPlaneAuditCorrelationError {}

impl fmt::Display for ApplicationControlPlaneAuditEmissionSpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Span(error) => error.fmt(f),
            Self::MissingTenant => write!(f, "audit emission span requires tenant correlation"),
            Self::MissingAuditEvent => {
                write!(f, "audit emission span requires audit event correlation")
            }
        }
    }
}

impl Error for ApplicationControlPlaneAuditEmissionSpanError {}

impl fmt::Display for ApplicationControlPlaneAuditPersistenceSpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Span(error) => error.fmt(f),
            Self::MissingTenant => write!(f, "audit persistence span requires tenant correlation"),
            Self::MissingAuditEvent => {
                write!(f, "audit persistence span requires audit event correlation")
            }
        }
    }
}

impl Error for ApplicationControlPlaneAuditPersistenceSpanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditErrorStatus {
    BadRequest,
    MethodNotAllowed,
    ServiceUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditCorrelationErrorStatus {
    BadRequest,
    MethodNotAllowed,
    PayloadTooLarge,
    RequestHeaderFieldsTooLarge,
    ServiceUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditEmissionSpanErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationControlPlaneAuditPersistenceSpanErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditErrorResponsePlan {
    pub status: ApplicationControlPlaneAuditErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditCorrelationErrorResponsePlan {
    pub status: ApplicationControlPlaneAuditCorrelationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditEmissionSpanErrorResponsePlan {
    pub status: ApplicationControlPlaneAuditEmissionSpanErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationControlPlaneAuditPersistenceSpanErrorResponsePlan {
    pub status: ApplicationControlPlaneAuditPersistenceSpanErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_control_plane_audit_error_response(
    error: &ApplicationControlPlaneAuditError,
) -> ApplicationControlPlaneAuditErrorResponsePlan {
    match error {
        ApplicationControlPlaneAuditError::HttpRoute(error) => {
            let response = match error {
                ApplicationControlPlaneHttpRouteError::Route(error) => {
                    plan_gateway_control_plane_route_error_response(error)
                }
            };
            ApplicationControlPlaneAuditErrorResponsePlan {
                status: match response.status {
                    GatewayControlPlaneRouteErrorStatus::BadRequest => {
                        ApplicationControlPlaneAuditErrorStatus::BadRequest
                    }
                    GatewayControlPlaneRouteErrorStatus::MethodNotAllowed => {
                        ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneAuditError::OperationMismatch { .. } => {
            ApplicationControlPlaneAuditErrorResponsePlan {
                status: ApplicationControlPlaneAuditErrorStatus::BadRequest,
                code: "control_plane_route_invalid",
                message: "control-plane route is invalid",
            }
        }
        ApplicationControlPlaneAuditError::AuditNotRequired { .. } => {
            ApplicationControlPlaneAuditErrorResponsePlan {
                status: ApplicationControlPlaneAuditErrorStatus::BadRequest,
                code: "control_plane_audit_required",
                message: "control-plane audit is required",
            }
        }
        ApplicationControlPlaneAuditError::Postgres(error) => {
            application_control_plane_audit_response_from_postgres(
                plan_postgres_storage_error_response(error),
                "audit_storage_unavailable",
                "audit storage is temporarily unavailable",
            )
        }
        ApplicationControlPlaneAuditError::Sqlite(error) => {
            application_control_plane_audit_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
                "audit_storage_unavailable",
                "audit storage is temporarily unavailable",
            )
        }
    }
}

pub fn plan_application_control_plane_audit_correlation_error_response(
    error: &ApplicationControlPlaneAuditCorrelationError,
) -> ApplicationControlPlaneAuditCorrelationErrorResponsePlan {
    match error {
        ApplicationControlPlaneAuditCorrelationError::Http(error) => {
            let response = plan_gateway_http_error_response(error);
            ApplicationControlPlaneAuditCorrelationErrorResponsePlan {
                status: match response.status {
                    GatewayHttpErrorStatus::BadRequest => {
                        ApplicationControlPlaneAuditCorrelationErrorStatus::BadRequest
                    }
                    GatewayHttpErrorStatus::MethodNotAllowed => {
                        ApplicationControlPlaneAuditCorrelationErrorStatus::MethodNotAllowed
                    }
                    GatewayHttpErrorStatus::PayloadTooLarge => {
                        ApplicationControlPlaneAuditCorrelationErrorStatus::PayloadTooLarge
                    }
                    GatewayHttpErrorStatus::RequestHeaderFieldsTooLarge => {
                        ApplicationControlPlaneAuditCorrelationErrorStatus::RequestHeaderFieldsTooLarge
                    }
                    GatewayHttpErrorStatus::InternalServerError => {
                        ApplicationControlPlaneAuditCorrelationErrorStatus::ServiceUnavailable
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneAuditCorrelationError::MissingTraceContext => {
            ApplicationControlPlaneAuditCorrelationErrorResponsePlan {
                status: ApplicationControlPlaneAuditCorrelationErrorStatus::BadRequest,
                code: "invalid_trace_context",
                message: "trace context is required and must be valid",
            }
        }
    }
}

pub fn plan_application_control_plane_audit_emission_span_error_response(
    error: &ApplicationControlPlaneAuditEmissionSpanError,
) -> ApplicationControlPlaneAuditEmissionSpanErrorResponsePlan {
    match error {
        ApplicationControlPlaneAuditEmissionSpanError::Span(error) => {
            let response = plan_span_error_response(error);
            ApplicationControlPlaneAuditEmissionSpanErrorResponsePlan {
                status: match response.status {
                    ObservabilityErrorStatus::ServiceUnavailable => {
                        ApplicationControlPlaneAuditEmissionSpanErrorStatus::ServiceUnavailable
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneAuditEmissionSpanError::MissingTenant
        | ApplicationControlPlaneAuditEmissionSpanError::MissingAuditEvent => {
            ApplicationControlPlaneAuditEmissionSpanErrorResponsePlan {
                status: ApplicationControlPlaneAuditEmissionSpanErrorStatus::ServiceUnavailable,
                code: "telemetry_unavailable",
                message: "telemetry planning is temporarily unavailable",
            }
        }
    }
}

pub fn plan_application_control_plane_audit_persistence_span_error_response(
    error: &ApplicationControlPlaneAuditPersistenceSpanError,
) -> ApplicationControlPlaneAuditPersistenceSpanErrorResponsePlan {
    match error {
        ApplicationControlPlaneAuditPersistenceSpanError::Span(error) => {
            let response = plan_span_error_response(error);
            ApplicationControlPlaneAuditPersistenceSpanErrorResponsePlan {
                status: match response.status {
                    ObservabilityErrorStatus::ServiceUnavailable => {
                        ApplicationControlPlaneAuditPersistenceSpanErrorStatus::ServiceUnavailable
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        ApplicationControlPlaneAuditPersistenceSpanError::MissingTenant
        | ApplicationControlPlaneAuditPersistenceSpanError::MissingAuditEvent => {
            ApplicationControlPlaneAuditPersistenceSpanErrorResponsePlan {
                status: ApplicationControlPlaneAuditPersistenceSpanErrorStatus::ServiceUnavailable,
                code: "telemetry_unavailable",
                message: "telemetry planning is temporarily unavailable",
            }
        }
    }
}

fn application_control_plane_audit_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationControlPlaneAuditErrorResponsePlan {
    ApplicationControlPlaneAuditErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

fn application_control_plane_audit_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
    code: &'static str,
    message: &'static str,
) -> ApplicationControlPlaneAuditErrorResponsePlan {
    ApplicationControlPlaneAuditErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable
            }
        },
        code,
        message,
    }
}

pub fn plan_application_control_plane_with_audit_storage(
    request: ApplicationControlPlaneAuditRequest,
) -> Result<ApplicationControlPlaneAuditPlan, ApplicationControlPlaneAuditError> {
    let decision = decide_control_plane_action(request.action);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)?,
        ),
    };
    Ok(ApplicationControlPlaneAuditPlan {
        decision,
        audit_storage,
    })
}

pub fn plan_application_control_plane_audit_from_http(
    action: ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<ApplicationControlPlaneAuditHttpPlan, ApplicationControlPlaneAuditError> {
    let route = validate_control_plane_http_action_for_audit(&action, http)?;
    Ok(ApplicationControlPlaneAuditHttpPlan {
        action,
        route: route.http,
    })
}

pub fn plan_application_control_plane_with_audit_storage_from_http(
    request: ApplicationControlPlaneAuditRequest,
    http: &GatewayHttpRequestMeta,
) -> Result<ApplicationControlPlaneAuditPlan, ApplicationControlPlaneAuditError> {
    validate_control_plane_http_action_for_audit(&request.action, http)?;
    plan_application_control_plane_with_audit_storage(request)
}

pub fn plan_application_control_plane_audit_correlation_from_http(
    request: ApplicationControlPlaneAuditCorrelationRequest,
) -> Result<ApplicationControlPlaneAuditCorrelationPlan, ApplicationControlPlaneAuditCorrelationError>
{
    let http = plan_gateway_http_request(request.http_policy, request.http)
        .map_err(ApplicationControlPlaneAuditCorrelationError::Http)?;
    let trace_context = http
        .trace_context
        .ok_or(ApplicationControlPlaneAuditCorrelationError::MissingTraceContext)?;
    let audit_write = control_plane_audit_write(&request.audit.decision);
    let mut correlation = CorrelationContext::new(request.request_id)
        .with_trace_id(trace_context.trace_id)
        .with_tenant_id(audit_write.tenant_partition_key)
        .with_audit_event_id(audit_write.event.id);
    if let Some(call_id) = request.call_id {
        correlation = correlation.with_call_id(call_id);
    }
    Ok(ApplicationControlPlaneAuditCorrelationPlan { correlation })
}

pub fn plan_application_control_plane_audit_emission_span(
    request: ApplicationControlPlaneAuditEmissionSpanRequest,
) -> Result<
    ApplicationControlPlaneAuditEmissionSpanPlan,
    ApplicationControlPlaneAuditEmissionSpanError,
> {
    let tenant_id = request
        .correlation
        .tenant_id
        .ok_or(ApplicationControlPlaneAuditEmissionSpanError::MissingTenant)?;
    let audit_event_id = request
        .correlation
        .audit_event_id
        .ok_or(ApplicationControlPlaneAuditEmissionSpanError::MissingAuditEvent)?;
    let span = plan_gateway_span(
        GatewaySpanKind::AuditEmission,
        "prodex.control_plane.audit.emit",
        request.correlation,
        None,
        vec![
            tenant_trace_attribute(tenant_id),
            TelemetryAttribute::trace_only("audit_event_id", audit_event_id.to_string()),
        ],
    )
    .map_err(ApplicationControlPlaneAuditEmissionSpanError::Span)?;
    Ok(ApplicationControlPlaneAuditEmissionSpanPlan { span })
}

pub fn plan_application_control_plane_audit_persistence_span(
    request: ApplicationControlPlaneAuditPersistenceSpanRequest,
) -> Result<
    ApplicationControlPlaneAuditPersistenceSpanPlan,
    ApplicationControlPlaneAuditPersistenceSpanError,
> {
    let tenant_id = request
        .correlation
        .tenant_id
        .ok_or(ApplicationControlPlaneAuditPersistenceSpanError::MissingTenant)?;
    let audit_event_id = request
        .correlation
        .audit_event_id
        .ok_or(ApplicationControlPlaneAuditPersistenceSpanError::MissingAuditEvent)?;
    let storage_backend = application_control_plane_audit_storage_backend(&request.audit_storage);
    let span = plan_gateway_span(
        GatewaySpanKind::Persistence,
        "prodex.control_plane.audit.persist",
        request.correlation,
        None,
        vec![
            TelemetryAttribute::metric_label("storage_backend", storage_backend),
            tenant_trace_attribute(tenant_id),
            TelemetryAttribute::trace_only("audit_event_id", audit_event_id.to_string()),
        ],
    )
    .map_err(ApplicationControlPlaneAuditPersistenceSpanError::Span)?;
    Ok(ApplicationControlPlaneAuditPersistenceSpanPlan { span })
}

fn application_control_plane_audit_storage_backend(
    audit_storage: &ApplicationControlPlaneAuditStoragePlan,
) -> &'static str {
    match audit_storage {
        ApplicationControlPlaneAuditStoragePlan::Postgres(_) => "postgres",
        ApplicationControlPlaneAuditStoragePlan::Sqlite(_) => "sqlite",
    }
}

pub fn plan_application_break_glass_with_audit_storage(
    request: ApplicationBreakGlassAuditRequest,
) -> Result<ApplicationBreakGlassAuditPlan, ApplicationControlPlaneAuditError> {
    let decision = decide_break_glass_action(request.action, request.authorization);
    let audit_write = control_plane_audit_write(&decision);
    let audit_command = AppendOnlyAuditCommand {
        storage_key: TenantStorageKey::tenant(audit_write.tenant_partition_key),
        event: audit_write.event.clone(),
        previous_digest: request.previous_digest,
        event_digest: request.event_digest,
    };
    let audit_storage = match request.durable_store {
        DurableStoreKind::Postgres => ApplicationControlPlaneAuditStoragePlan::Postgres(
            plan_postgres_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Postgres)?,
        ),
        DurableStoreKind::Sqlite => ApplicationControlPlaneAuditStoragePlan::Sqlite(
            plan_sqlite_append_only_audit(audit_command)
                .map_err(ApplicationControlPlaneAuditError::Sqlite)?,
        ),
    };
    Ok(ApplicationBreakGlassAuditPlan {
        decision,
        audit_storage,
    })
}

pub(crate) fn control_plane_audit_write(
    decision: &ControlPlaneDecision,
) -> &ControlPlaneAuditWritePlan {
    match decision {
        ControlPlaneDecision::Authorized(plan) => &plan.audit_write,
        ControlPlaneDecision::Denied { audit_write, .. } => audit_write,
    }
}
