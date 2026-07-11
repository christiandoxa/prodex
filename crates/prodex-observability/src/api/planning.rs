use super::*;
use prodex_domain::{TelemetryAttribute, TelemetryAttributeError};

pub fn plan_api_red_metric(
    route: ApiRouteKind,
    status_class: ApiStatusClass,
    duration_ms: u64,
) -> Result<ApiRedMetricPlan, TelemetryAttributeError> {
    let route_label = TelemetryAttribute::metric_label("api_route", api_route_kind_label(route));
    let status_label =
        TelemetryAttribute::metric_label("status_class", api_status_class_label(status_class));
    route_label.as_metric_label()?;
    status_label.as_metric_label()?;
    Ok(ApiRedMetricPlan {
        request_count_metric_name: "prodex_api_requests_total",
        duration_metric_name: "prodex_api_request_duration_ms",
        increment: 1,
        duration_ms,
        route_label,
        status_label,
    })
}

pub fn plan_api_admission_metric(
    route: ApiRouteKind,
    result: ApiAdmissionResult,
) -> Result<ApiAdmissionMetricPlan, TelemetryAttributeError> {
    let route_label =
        TelemetryAttribute::metric_label("api_admission_route", api_route_kind_label(route));
    let result_label = TelemetryAttribute::metric_label(
        "api_admission_result",
        api_admission_result_label(result),
    );
    route_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiAdmissionMetricPlan {
        metric_name: "prodex_api_admission_decisions_total",
        increment: 1,
        route_label,
        result_label,
    })
}

pub fn plan_api_schema_validation_metric(
    surface: ApiSchemaSurface,
    result: ApiSchemaValidationResult,
) -> Result<ApiSchemaValidationMetricPlan, TelemetryAttributeError> {
    let surface_label =
        TelemetryAttribute::metric_label("api_schema_surface", api_schema_surface_label(surface));
    let result_label = TelemetryAttribute::metric_label(
        "api_schema_result",
        api_schema_validation_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiSchemaValidationMetricPlan {
        metric_name: "prodex_api_schema_validation_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_deprecation_metric(
    surface: ApiDeprecationSurface,
    signal: ApiDeprecationSignal,
) -> Result<ApiDeprecationMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_deprecation_surface",
        api_deprecation_surface_label(surface),
    );
    let signal_label = TelemetryAttribute::metric_label(
        "api_deprecation_signal",
        api_deprecation_signal_label(signal),
    );
    surface_label.as_metric_label()?;
    signal_label.as_metric_label()?;
    Ok(ApiDeprecationMetricPlan {
        metric_name: "prodex_api_deprecation_events_total",
        increment: 1,
        surface_label,
        signal_label,
    })
}

pub fn plan_api_pagination_metric(
    surface: ApiPaginationSurface,
    result: ApiPaginationResult,
) -> Result<ApiPaginationMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_pagination_surface",
        api_pagination_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_pagination_result",
        api_pagination_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiPaginationMetricPlan {
        metric_name: "prodex_api_pagination_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_precondition_metric(
    surface: ApiPreconditionSurface,
    result: ApiPreconditionResult,
) -> Result<ApiPreconditionMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_precondition_surface",
        api_precondition_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_precondition_result",
        api_precondition_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiPreconditionMetricPlan {
        metric_name: "prodex_api_precondition_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_idempotency_metric(
    surface: ApiIdempotencySurface,
    result: ApiIdempotencyResult,
) -> Result<ApiIdempotencyMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_idempotency_surface",
        api_idempotency_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_idempotency_result",
        api_idempotency_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiIdempotencyMetricPlan {
        metric_name: "prodex_api_idempotency_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_idempotency_record_metric(
    backend: IdempotencyRecordBackend,
    operation: IdempotencyRecordOperation,
    result: IdempotencyRecordResult,
) -> Result<IdempotencyRecordMetricPlan, TelemetryAttributeError> {
    let backend_label = TelemetryAttribute::metric_label(
        "idempotency_record_backend",
        idempotency_record_backend_label(backend),
    );
    let operation_label = TelemetryAttribute::metric_label(
        "idempotency_record_operation",
        idempotency_record_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "idempotency_record_result",
        idempotency_record_result_label(result),
    );
    backend_label.as_metric_label()?;
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(IdempotencyRecordMetricPlan {
        metric_name: "prodex_idempotency_record_events_total",
        increment: 1,
        backend_label,
        operation_label,
        result_label,
    })
}

pub fn plan_api_compatibility_metric(
    surface: ApiCompatibilitySurface,
    result: ApiCompatibilityResult,
) -> Result<ApiCompatibilityMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_compatibility_surface",
        api_compatibility_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_compatibility_result",
        api_compatibility_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiCompatibilityMetricPlan {
        metric_name: "prodex_api_compatibility_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_mutation_audit_metric(
    surface: ApiMutationAuditSurface,
    result: ApiMutationAuditResult,
) -> Result<ApiMutationAuditMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_mutation_audit_surface",
        api_mutation_audit_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_mutation_audit_result",
        api_mutation_audit_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiMutationAuditMetricPlan {
        metric_name: "prodex_api_mutation_audit_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_version_metric(
    surface: ApiVersionSurface,
    result: ApiVersionResult,
) -> Result<ApiVersionMetricPlan, TelemetryAttributeError> {
    let surface_label =
        TelemetryAttribute::metric_label("api_version_surface", api_version_surface_label(surface));
    let result_label =
        TelemetryAttribute::metric_label("api_version_result", api_version_result_label(result));
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiVersionMetricPlan {
        metric_name: "prodex_api_version_negotiation_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_spec_publication_metric(
    surface: ApiSpecSurface,
    result: ApiSpecPublicationResult,
) -> Result<ApiSpecPublicationMetricPlan, TelemetryAttributeError> {
    let surface_label =
        TelemetryAttribute::metric_label("api_spec_surface", api_spec_surface_label(surface));
    let result_label = TelemetryAttribute::metric_label(
        "api_spec_publication_result",
        api_spec_publication_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiSpecPublicationMetricPlan {
        metric_name: "prodex_api_spec_publication_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_error_envelope_metric(
    surface: ApiErrorEnvelopeSurface,
    result: ApiErrorEnvelopeResult,
) -> Result<ApiErrorEnvelopeMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_error_envelope_surface",
        api_error_envelope_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_error_envelope_result",
        api_error_envelope_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiErrorEnvelopeMetricPlan {
        metric_name: "prodex_api_error_envelope_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_body_limit_metric(
    surface: ApiBodyLimitSurface,
    result: ApiBodyLimitResult,
) -> Result<ApiBodyLimitMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_body_limit_surface",
        api_body_limit_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_body_limit_result",
        api_body_limit_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiBodyLimitMetricPlan {
        metric_name: "prodex_api_body_limit_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_timeout_budget_metric(
    surface: ApiTimeoutBudgetSurface,
    result: ApiTimeoutBudgetResult,
) -> Result<ApiTimeoutBudgetMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_timeout_budget_surface",
        api_timeout_budget_surface_label(surface),
    );
    let result_label = TelemetryAttribute::metric_label(
        "api_timeout_budget_result",
        api_timeout_budget_result_label(result),
    );
    surface_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(ApiTimeoutBudgetMetricPlan {
        metric_name: "prodex_api_timeout_budget_events_total",
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_cancellation_metric(
    surface: ApiCancellationSurface,
    source: ApiCancellationSource,
) -> Result<ApiCancellationMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_cancellation_surface",
        api_cancellation_surface_label(surface),
    );
    let source_label = TelemetryAttribute::metric_label(
        "api_cancellation_source",
        api_cancellation_source_label(source),
    );
    surface_label.as_metric_label()?;
    source_label.as_metric_label()?;
    Ok(ApiCancellationMetricPlan {
        metric_name: "prodex_api_cancellation_events_total",
        increment: 1,
        surface_label,
        source_label,
    })
}

pub fn plan_api_stream_backpressure_metric(
    surface: ApiStreamBackpressureSurface,
    state: ApiStreamBackpressureState,
) -> Result<ApiStreamBackpressureMetricPlan, TelemetryAttributeError> {
    let surface_label = TelemetryAttribute::metric_label(
        "api_stream_backpressure_surface",
        api_stream_backpressure_surface_label(surface),
    );
    let state_label = TelemetryAttribute::metric_label(
        "api_stream_backpressure_state",
        api_stream_backpressure_state_label(state),
    );
    surface_label.as_metric_label()?;
    state_label.as_metric_label()?;
    Ok(ApiStreamBackpressureMetricPlan {
        metric_name: "prodex_api_stream_backpressure_events_total",
        increment: 1,
        surface_label,
        state_label,
    })
}

fn api_route_kind_label(route: ApiRouteKind) -> &'static str {
    match route {
        ApiRouteKind::Responses => "responses",
        ApiRouteKind::Compact => "compact",
        ApiRouteKind::Websocket => "websocket",
        ApiRouteKind::ControlPlane => "control_plane",
        ApiRouteKind::Health => "health",
    }
}

fn api_status_class_label(status_class: ApiStatusClass) -> &'static str {
    match status_class {
        ApiStatusClass::Informational => "1xx",
        ApiStatusClass::Success => "2xx",
        ApiStatusClass::Redirection => "3xx",
        ApiStatusClass::ClientError => "4xx",
        ApiStatusClass::ServerError => "5xx",
    }
}

fn api_admission_result_label(result: ApiAdmissionResult) -> &'static str {
    match result {
        ApiAdmissionResult::Accepted => "accepted",
        ApiAdmissionResult::GlobalLimitReached => "global_limit_reached",
        ApiAdmissionResult::RouteLimitReached => "route_limit_reached",
        ApiAdmissionResult::QueueFull => "queue_full",
        ApiAdmissionResult::Draining => "draining",
    }
}

fn api_schema_surface_label(surface: ApiSchemaSurface) -> &'static str {
    match surface {
        ApiSchemaSurface::Request => "request",
        ApiSchemaSurface::Response => "response",
        ApiSchemaSurface::OpenApi => "openapi",
        ApiSchemaSurface::ErrorEnvelope => "error_envelope",
    }
}

fn api_schema_validation_result_label(result: ApiSchemaValidationResult) -> &'static str {
    match result {
        ApiSchemaValidationResult::Valid => "valid",
        ApiSchemaValidationResult::Invalid => "invalid",
        ApiSchemaValidationResult::MissingSchema => "missing_schema",
        ApiSchemaValidationResult::Incompatible => "incompatible",
    }
}

fn api_deprecation_surface_label(surface: ApiDeprecationSurface) -> &'static str {
    match surface {
        ApiDeprecationSurface::DataPlane => "data_plane",
        ApiDeprecationSurface::ControlPlane => "control_plane",
        ApiDeprecationSurface::Scim => "scim",
        ApiDeprecationSurface::Health => "health",
    }
}

fn api_deprecation_signal_label(signal: ApiDeprecationSignal) -> &'static str {
    match signal {
        ApiDeprecationSignal::Notice => "notice",
        ApiDeprecationSignal::Sunset => "sunset",
        ApiDeprecationSignal::Rejected => "rejected",
    }
}

fn api_pagination_surface_label(surface: ApiPaginationSurface) -> &'static str {
    match surface {
        ApiPaginationSurface::ControlPlane => "control_plane",
        ApiPaginationSurface::Scim => "scim",
        ApiPaginationSurface::AuditExport => "audit_export",
        ApiPaginationSurface::Quota => "quota",
    }
}

fn api_pagination_result_label(result: ApiPaginationResult) -> &'static str {
    match result {
        ApiPaginationResult::PageReturned => "page_returned",
        ApiPaginationResult::EmptyPage => "empty_page",
        ApiPaginationResult::InvalidCursor => "invalid_cursor",
        ApiPaginationResult::ExpiredCursor => "expired_cursor",
    }
}

fn api_precondition_surface_label(surface: ApiPreconditionSurface) -> &'static str {
    match surface {
        ApiPreconditionSurface::Tenant => "tenant",
        ApiPreconditionSurface::Principal => "principal",
        ApiPreconditionSurface::VirtualKey => "virtual_key",
        ApiPreconditionSurface::Policy => "policy",
    }
}

fn api_precondition_result_label(result: ApiPreconditionResult) -> &'static str {
    match result {
        ApiPreconditionResult::Matched => "matched",
        ApiPreconditionResult::Missing => "missing",
        ApiPreconditionResult::Mismatched => "mismatched",
        ApiPreconditionResult::Invalid => "invalid",
    }
}

fn api_idempotency_surface_label(surface: ApiIdempotencySurface) -> &'static str {
    match surface {
        ApiIdempotencySurface::TenantMutation => "tenant_mutation",
        ApiIdempotencySurface::PrincipalMutation => "principal_mutation",
        ApiIdempotencySurface::VirtualKeyMutation => "virtual_key_mutation",
        ApiIdempotencySurface::PolicyMutation => "policy_mutation",
    }
}

fn api_idempotency_result_label(result: ApiIdempotencyResult) -> &'static str {
    match result {
        ApiIdempotencyResult::Accepted => "accepted",
        ApiIdempotencyResult::Replayed => "replayed",
        ApiIdempotencyResult::Conflict => "conflict",
        ApiIdempotencyResult::Missing => "missing",
        ApiIdempotencyResult::Invalid => "invalid",
    }
}

fn idempotency_record_backend_label(backend: IdempotencyRecordBackend) -> &'static str {
    match backend {
        IdempotencyRecordBackend::Postgres => "postgres",
        IdempotencyRecordBackend::Sqlite => "sqlite",
    }
}

fn idempotency_record_operation_label(operation: IdempotencyRecordOperation) -> &'static str {
    match operation {
        IdempotencyRecordOperation::PendingInsert => "pending_insert",
        IdempotencyRecordOperation::Complete => "complete",
        IdempotencyRecordOperation::Lookup => "lookup",
    }
}

fn idempotency_record_result_label(result: IdempotencyRecordResult) -> &'static str {
    match result {
        IdempotencyRecordResult::Recorded => "recorded",
        IdempotencyRecordResult::Replayed => "replayed",
        IdempotencyRecordResult::Conflict => "conflict",
        IdempotencyRecordResult::NotFound => "not_found",
        IdempotencyRecordResult::Failed => "failed",
    }
}

fn api_compatibility_surface_label(surface: ApiCompatibilitySurface) -> &'static str {
    match surface {
        ApiCompatibilitySurface::DataPlane => "data_plane",
        ApiCompatibilitySurface::ControlPlane => "control_plane",
        ApiCompatibilitySurface::Scim => "scim",
        ApiCompatibilitySurface::ErrorEnvelope => "error_envelope",
    }
}

fn api_compatibility_result_label(result: ApiCompatibilityResult) -> &'static str {
    match result {
        ApiCompatibilityResult::Compatible => "compatible",
        ApiCompatibilityResult::AdditiveChange => "additive_change",
        ApiCompatibilityResult::DeprecatedChange => "deprecated_change",
        ApiCompatibilityResult::BreakingChange => "breaking_change",
    }
}

fn api_mutation_audit_surface_label(surface: ApiMutationAuditSurface) -> &'static str {
    match surface {
        ApiMutationAuditSurface::Tenant => "tenant",
        ApiMutationAuditSurface::Principal => "principal",
        ApiMutationAuditSurface::VirtualKey => "virtual_key",
        ApiMutationAuditSurface::Policy => "policy",
    }
}

fn api_mutation_audit_result_label(result: ApiMutationAuditResult) -> &'static str {
    match result {
        ApiMutationAuditResult::Required => "required",
        ApiMutationAuditResult::Persisted => "persisted",
        ApiMutationAuditResult::Missing => "missing",
        ApiMutationAuditResult::Failed => "failed",
    }
}

fn api_version_surface_label(surface: ApiVersionSurface) -> &'static str {
    match surface {
        ApiVersionSurface::DataPlane => "data_plane",
        ApiVersionSurface::ControlPlane => "control_plane",
        ApiVersionSurface::Scim => "scim",
        ApiVersionSurface::Health => "health",
    }
}

fn api_version_result_label(result: ApiVersionResult) -> &'static str {
    match result {
        ApiVersionResult::Accepted => "accepted",
        ApiVersionResult::Defaulted => "defaulted",
        ApiVersionResult::Deprecated => "deprecated",
        ApiVersionResult::Unsupported => "unsupported",
    }
}

fn api_spec_surface_label(surface: ApiSpecSurface) -> &'static str {
    match surface {
        ApiSpecSurface::GatewayOpenApi => "gateway_openapi",
        ApiSpecSurface::ControlPlaneOpenApi => "control_plane_openapi",
        ApiSpecSurface::ScimSchema => "scim_schema",
        ApiSpecSurface::ErrorEnvelope => "error_envelope",
    }
}

fn api_spec_publication_result_label(result: ApiSpecPublicationResult) -> &'static str {
    match result {
        ApiSpecPublicationResult::Generated => "generated",
        ApiSpecPublicationResult::Validated => "validated",
        ApiSpecPublicationResult::Published => "published",
        ApiSpecPublicationResult::Rejected => "rejected",
    }
}

fn api_error_envelope_surface_label(surface: ApiErrorEnvelopeSurface) -> &'static str {
    match surface {
        ApiErrorEnvelopeSurface::DataPlane => "data_plane",
        ApiErrorEnvelopeSurface::ControlPlane => "control_plane",
        ApiErrorEnvelopeSurface::Scim => "scim",
        ApiErrorEnvelopeSurface::Health => "health",
    }
}

fn api_error_envelope_result_label(result: ApiErrorEnvelopeResult) -> &'static str {
    match result {
        ApiErrorEnvelopeResult::Emitted => "emitted",
        ApiErrorEnvelopeResult::Redacted => "redacted",
        ApiErrorEnvelopeResult::ValidationFailed => "validation_failed",
        ApiErrorEnvelopeResult::CompatibilityRejected => "compatibility_rejected",
    }
}

fn api_body_limit_surface_label(surface: ApiBodyLimitSurface) -> &'static str {
    match surface {
        ApiBodyLimitSurface::DataPlane => "data_plane",
        ApiBodyLimitSurface::ControlPlane => "control_plane",
        ApiBodyLimitSurface::Scim => "scim",
        ApiBodyLimitSurface::Upload => "upload",
    }
}

fn api_body_limit_result_label(result: ApiBodyLimitResult) -> &'static str {
    match result {
        ApiBodyLimitResult::Accepted => "accepted",
        ApiBodyLimitResult::RejectedTooLarge => "rejected_too_large",
        ApiBodyLimitResult::UnknownLength => "unknown_length",
        ApiBodyLimitResult::Truncated => "truncated",
    }
}

fn api_timeout_budget_surface_label(surface: ApiTimeoutBudgetSurface) -> &'static str {
    match surface {
        ApiTimeoutBudgetSurface::DataPlane => "data_plane",
        ApiTimeoutBudgetSurface::ControlPlane => "control_plane",
        ApiTimeoutBudgetSurface::Provider => "provider",
        ApiTimeoutBudgetSurface::Persistence => "persistence",
    }
}

fn api_timeout_budget_result_label(result: ApiTimeoutBudgetResult) -> &'static str {
    match result {
        ApiTimeoutBudgetResult::Accepted => "accepted",
        ApiTimeoutBudgetResult::Expired => "expired",
        ApiTimeoutBudgetResult::Exhausted => "exhausted",
        ApiTimeoutBudgetResult::Cancelled => "cancelled",
    }
}

fn api_cancellation_surface_label(surface: ApiCancellationSurface) -> &'static str {
    match surface {
        ApiCancellationSurface::DataPlane => "data_plane",
        ApiCancellationSurface::ControlPlane => "control_plane",
        ApiCancellationSurface::ProviderStream => "provider_stream",
        ApiCancellationSurface::Persistence => "persistence",
    }
}

fn api_cancellation_source_label(source: ApiCancellationSource) -> &'static str {
    match source {
        ApiCancellationSource::ClientDisconnect => "client_disconnect",
        ApiCancellationSource::TimeoutBudget => "timeout_budget",
        ApiCancellationSource::ShutdownDrain => "shutdown_drain",
        ApiCancellationSource::UpstreamAbort => "upstream_abort",
    }
}

fn api_stream_backpressure_surface_label(surface: ApiStreamBackpressureSurface) -> &'static str {
    match surface {
        ApiStreamBackpressureSurface::DataPlaneStream => "data_plane_stream",
        ApiStreamBackpressureSurface::ProviderStream => "provider_stream",
        ApiStreamBackpressureSurface::Websocket => "websocket",
        ApiStreamBackpressureSurface::AuditExport => "audit_export",
    }
}

fn api_stream_backpressure_state_label(state: ApiStreamBackpressureState) -> &'static str {
    match state {
        ApiStreamBackpressureState::Ready => "ready",
        ApiStreamBackpressureState::Paused => "paused",
        ApiStreamBackpressureState::Dropped => "dropped",
        ApiStreamBackpressureState::Closed => "closed",
    }
}
