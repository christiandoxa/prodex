use super::*;
use prodex_domain::TelemetryAttributeError;

pub fn plan_api_red_metric(
    route: ApiRouteKind,
    status_class: ApiStatusClass,
    duration_ms: u64,
) -> Result<ApiRedMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let route_label = crate::planning_support::planned_metric_label(17, 0, (route) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let route_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(24, "api_route"),
        api_route_kind_label(route),
    )?;
    #[cfg(feature = "mojo")]
    let status_label = crate::planning_support::planned_metric_label(17, 1, (status_class) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let status_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(139, "status_class"),
        api_status_class_label(status_class),
    )?;
    Ok(ApiRedMetricPlan {
        request_count_metric_name: crate::planning_support::metric_name(
            17,
            0,
            "prodex_api_requests_total",
        ),
        duration_metric_name: crate::planning_support::metric_name(
            17,
            1,
            "prodex_api_request_duration_ms",
        ),
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
    #[cfg(feature = "mojo")]
    let route_label = crate::planning_support::planned_metric_label(7, 0, (route) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let route_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(5, "api_admission_route"),
        api_route_kind_label(route),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(7, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(4, "api_admission_result"),
        api_admission_result_label(result),
    )?;
    Ok(ApiAdmissionMetricPlan {
        metric_name: crate::planning_support::metric_name(
            7,
            0,
            "prodex_api_admission_decisions_total",
        ),
        increment: 1,
        route_label,
        result_label,
    })
}

pub fn plan_api_schema_validation_metric(
    surface: ApiSchemaSurface,
    result: ApiSchemaValidationResult,
) -> Result<ApiSchemaValidationMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(18, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(26, "api_schema_surface"),
        api_schema_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(18, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(25, "api_schema_result"),
        api_schema_validation_result_label(result),
    )?;
    Ok(ApiSchemaValidationMetricPlan {
        metric_name: crate::planning_support::metric_name(
            18,
            0,
            "prodex_api_schema_validation_total",
        ),
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_deprecation_metric(
    surface: ApiDeprecationSurface,
    signal: ApiDeprecationSignal,
) -> Result<ApiDeprecationMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(11, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(13, "api_deprecation_surface"),
        api_deprecation_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let signal_label = crate::planning_support::planned_metric_label(11, 1, (signal) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let signal_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(12, "api_deprecation_signal"),
        api_deprecation_signal_label(signal),
    )?;
    Ok(ApiDeprecationMetricPlan {
        metric_name: crate::planning_support::metric_name(
            11,
            0,
            "prodex_api_deprecation_events_total",
        ),
        increment: 1,
        surface_label,
        signal_label,
    })
}

pub fn plan_api_pagination_metric(
    surface: ApiPaginationSurface,
    result: ApiPaginationResult,
) -> Result<ApiPaginationMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(15, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(21, "api_pagination_surface"),
        api_pagination_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(15, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(20, "api_pagination_result"),
        api_pagination_result_label(result),
    )?;
    Ok(ApiPaginationMetricPlan {
        metric_name: crate::planning_support::metric_name(
            15,
            0,
            "prodex_api_pagination_events_total",
        ),
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_precondition_metric(
    surface: ApiPreconditionSurface,
    result: ApiPreconditionResult,
) -> Result<ApiPreconditionMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(16, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(23, "api_precondition_surface"),
        api_precondition_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(16, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(22, "api_precondition_result"),
        api_precondition_result_label(result),
    )?;
    Ok(ApiPreconditionMetricPlan {
        metric_name: crate::planning_support::metric_name(
            16,
            0,
            "prodex_api_precondition_events_total",
        ),
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_idempotency_metric(
    surface: ApiIdempotencySurface,
    result: ApiIdempotencyResult,
) -> Result<ApiIdempotencyMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(13, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(17, "api_idempotency_surface"),
        api_idempotency_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(13, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(16, "api_idempotency_result"),
        api_idempotency_result_label(result),
    )?;
    Ok(ApiIdempotencyMetricPlan {
        metric_name: crate::planning_support::metric_name(
            13,
            0,
            "prodex_api_idempotency_events_total",
        ),
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
    #[cfg(feature = "mojo")]
    let backend_label = crate::planning_support::planned_metric_label(23, 0, (backend) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let backend_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(74, "idempotency_record_backend"),
        idempotency_record_backend_label(backend),
    )?;
    #[cfg(feature = "mojo")]
    let operation_label = crate::planning_support::planned_metric_label(23, 1, (operation) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(75, "idempotency_record_operation"),
        idempotency_record_operation_label(operation),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(23, 2, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(76, "idempotency_record_result"),
        idempotency_record_result_label(result),
    )?;
    Ok(IdempotencyRecordMetricPlan {
        metric_name: crate::planning_support::metric_name(
            23,
            0,
            "prodex_idempotency_record_events_total",
        ),
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
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(10, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(11, "api_compatibility_surface"),
        api_compatibility_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(10, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(10, "api_compatibility_result"),
        api_compatibility_result_label(result),
    )?;
    Ok(ApiCompatibilityMetricPlan {
        metric_name: crate::planning_support::metric_name(
            10,
            0,
            "prodex_api_compatibility_events_total",
        ),
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_mutation_audit_metric(
    surface: ApiMutationAuditSurface,
    result: ApiMutationAuditResult,
) -> Result<ApiMutationAuditMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(14, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(19, "api_mutation_audit_surface"),
        api_mutation_audit_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(14, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(18, "api_mutation_audit_result"),
        api_mutation_audit_result_label(result),
    )?;
    Ok(ApiMutationAuditMetricPlan {
        metric_name: crate::planning_support::metric_name(
            14,
            0,
            "prodex_api_mutation_audit_events_total",
        ),
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_version_metric(
    surface: ApiVersionSurface,
    result: ApiVersionResult,
) -> Result<ApiVersionMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(22, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(34, "api_version_surface"),
        api_version_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(22, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(33, "api_version_result"),
        api_version_result_label(result),
    )?;
    Ok(ApiVersionMetricPlan {
        metric_name: crate::planning_support::metric_name(
            22,
            0,
            "prodex_api_version_negotiation_events_total",
        ),
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_spec_publication_metric(
    surface: ApiSpecSurface,
    result: ApiSpecPublicationResult,
) -> Result<ApiSpecPublicationMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(19, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(28, "api_spec_surface"),
        api_spec_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(19, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(27, "api_spec_publication_result"),
        api_spec_publication_result_label(result),
    )?;
    Ok(ApiSpecPublicationMetricPlan {
        metric_name: crate::planning_support::metric_name(
            19,
            0,
            "prodex_api_spec_publication_events_total",
        ),
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_error_envelope_metric(
    surface: ApiErrorEnvelopeSurface,
    result: ApiErrorEnvelopeResult,
) -> Result<ApiErrorEnvelopeMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(12, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(15, "api_error_envelope_surface"),
        api_error_envelope_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(12, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(14, "api_error_envelope_result"),
        api_error_envelope_result_label(result),
    )?;
    Ok(ApiErrorEnvelopeMetricPlan {
        metric_name: crate::planning_support::metric_name(
            12,
            0,
            "prodex_api_error_envelope_events_total",
        ),
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_body_limit_metric(
    surface: ApiBodyLimitSurface,
    result: ApiBodyLimitResult,
) -> Result<ApiBodyLimitMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(8, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(7, "api_body_limit_surface"),
        api_body_limit_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(8, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(6, "api_body_limit_result"),
        api_body_limit_result_label(result),
    )?;
    Ok(ApiBodyLimitMetricPlan {
        metric_name: crate::planning_support::metric_name(
            8,
            0,
            "prodex_api_body_limit_events_total",
        ),
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_timeout_budget_metric(
    surface: ApiTimeoutBudgetSurface,
    result: ApiTimeoutBudgetResult,
) -> Result<ApiTimeoutBudgetMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(21, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(32, "api_timeout_budget_surface"),
        api_timeout_budget_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(21, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(31, "api_timeout_budget_result"),
        api_timeout_budget_result_label(result),
    )?;
    Ok(ApiTimeoutBudgetMetricPlan {
        metric_name: crate::planning_support::metric_name(
            21,
            0,
            "prodex_api_timeout_budget_events_total",
        ),
        increment: 1,
        surface_label,
        result_label,
    })
}

pub fn plan_api_cancellation_metric(
    surface: ApiCancellationSurface,
    source: ApiCancellationSource,
) -> Result<ApiCancellationMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(9, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(9, "api_cancellation_surface"),
        api_cancellation_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let source_label = crate::planning_support::planned_metric_label(9, 1, (source) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let source_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(8, "api_cancellation_source"),
        api_cancellation_source_label(source),
    )?;
    Ok(ApiCancellationMetricPlan {
        metric_name: crate::planning_support::metric_name(
            9,
            0,
            "prodex_api_cancellation_events_total",
        ),
        increment: 1,
        surface_label,
        source_label,
    })
}

pub fn plan_api_stream_backpressure_metric(
    surface: ApiStreamBackpressureSurface,
    state: ApiStreamBackpressureState,
) -> Result<ApiStreamBackpressureMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let surface_label = crate::planning_support::planned_metric_label(20, 0, (surface) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let surface_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(30, "api_stream_backpressure_surface"),
        api_stream_backpressure_surface_label(surface),
    )?;
    #[cfg(feature = "mojo")]
    let state_label = crate::planning_support::planned_metric_label(20, 1, (state) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let state_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(29, "api_stream_backpressure_state"),
        api_stream_backpressure_state_label(state),
    )?;
    Ok(ApiStreamBackpressureMetricPlan {
        metric_name: crate::planning_support::metric_name(
            20,
            0,
            "prodex_api_stream_backpressure_events_total",
        ),
        increment: 1,
        surface_label,
        state_label,
    })
}

#[cfg(not(feature = "mojo"))]
fn api_route_kind_label(route: ApiRouteKind) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(31, route as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match route {
            ApiRouteKind::Responses => "responses",
            ApiRouteKind::Compact => "compact",
            ApiRouteKind::Websocket => "websocket",
            ApiRouteKind::ControlPlane => "control_plane",
            ApiRouteKind::Health => "health",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_status_class_label(status_class: ApiStatusClass) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(36, status_class as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match status_class {
            ApiStatusClass::Informational => "1xx",
            ApiStatusClass::Success => "2xx",
            ApiStatusClass::Redirection => "3xx",
            ApiStatusClass::ClientError => "4xx",
            ApiStatusClass::ServerError => "5xx",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_admission_result_label(result: ApiAdmissionResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(12, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiAdmissionResult::Accepted => "accepted",
            ApiAdmissionResult::GlobalLimitReached => "global_limit_reached",
            ApiAdmissionResult::RouteLimitReached => "route_limit_reached",
            ApiAdmissionResult::QueueFull => "queue_full",
            ApiAdmissionResult::Draining => "draining",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_schema_surface_label(surface: ApiSchemaSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(32, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiSchemaSurface::Request => "request",
            ApiSchemaSurface::Response => "response",
            ApiSchemaSurface::OpenApi => "openapi",
            ApiSchemaSurface::ErrorEnvelope => "error_envelope",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_schema_validation_result_label(result: ApiSchemaValidationResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(33, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiSchemaValidationResult::Valid => "valid",
            ApiSchemaValidationResult::Invalid => "invalid",
            ApiSchemaValidationResult::MissingSchema => "missing_schema",
            ApiSchemaValidationResult::Incompatible => "incompatible",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_deprecation_surface_label(surface: ApiDeprecationSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(20, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiDeprecationSurface::DataPlane => "data_plane",
            ApiDeprecationSurface::ControlPlane => "control_plane",
            ApiDeprecationSurface::Scim => "scim",
            ApiDeprecationSurface::Health => "health",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_deprecation_signal_label(signal: ApiDeprecationSignal) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(19, signal as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match signal {
            ApiDeprecationSignal::Notice => "notice",
            ApiDeprecationSignal::Sunset => "sunset",
            ApiDeprecationSignal::Rejected => "rejected",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_pagination_surface_label(surface: ApiPaginationSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(28, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiPaginationSurface::ControlPlane => "control_plane",
            ApiPaginationSurface::Scim => "scim",
            ApiPaginationSurface::AuditExport => "audit_export",
            ApiPaginationSurface::Quota => "quota",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_pagination_result_label(result: ApiPaginationResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(27, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiPaginationResult::PageReturned => "page_returned",
            ApiPaginationResult::EmptyPage => "empty_page",
            ApiPaginationResult::InvalidCursor => "invalid_cursor",
            ApiPaginationResult::ExpiredCursor => "expired_cursor",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_precondition_surface_label(surface: ApiPreconditionSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(30, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiPreconditionSurface::Tenant => "tenant",
            ApiPreconditionSurface::Principal => "principal",
            ApiPreconditionSurface::VirtualKey => "virtual_key",
            ApiPreconditionSurface::Policy => "policy",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_precondition_result_label(result: ApiPreconditionResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(29, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiPreconditionResult::Matched => "matched",
            ApiPreconditionResult::Missing => "missing",
            ApiPreconditionResult::Mismatched => "mismatched",
            ApiPreconditionResult::Invalid => "invalid",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_idempotency_surface_label(surface: ApiIdempotencySurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(24, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiIdempotencySurface::TenantMutation => "tenant_mutation",
            ApiIdempotencySurface::PrincipalMutation => "principal_mutation",
            ApiIdempotencySurface::VirtualKeyMutation => "virtual_key_mutation",
            ApiIdempotencySurface::PolicyMutation => "policy_mutation",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_idempotency_result_label(result: ApiIdempotencyResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(23, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiIdempotencyResult::Accepted => "accepted",
            ApiIdempotencyResult::Replayed => "replayed",
            ApiIdempotencyResult::Conflict => "conflict",
            ApiIdempotencyResult::Missing => "missing",
            ApiIdempotencyResult::Invalid => "invalid",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn idempotency_record_backend_label(backend: IdempotencyRecordBackend) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(43, backend as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match backend {
            IdempotencyRecordBackend::Postgres => "postgres",
            IdempotencyRecordBackend::Sqlite => "sqlite",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn idempotency_record_operation_label(operation: IdempotencyRecordOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(44, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            IdempotencyRecordOperation::PendingInsert => "pending_insert",
            IdempotencyRecordOperation::Complete => "complete",
            IdempotencyRecordOperation::Lookup => "lookup",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn idempotency_record_result_label(result: IdempotencyRecordResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(45, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            IdempotencyRecordResult::Recorded => "recorded",
            IdempotencyRecordResult::Replayed => "replayed",
            IdempotencyRecordResult::Conflict => "conflict",
            IdempotencyRecordResult::NotFound => "not_found",
            IdempotencyRecordResult::Failed => "failed",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_compatibility_surface_label(surface: ApiCompatibilitySurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(18, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiCompatibilitySurface::DataPlane => "data_plane",
            ApiCompatibilitySurface::ControlPlane => "control_plane",
            ApiCompatibilitySurface::Scim => "scim",
            ApiCompatibilitySurface::ErrorEnvelope => "error_envelope",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_compatibility_result_label(result: ApiCompatibilityResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(17, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiCompatibilityResult::Compatible => "compatible",
            ApiCompatibilityResult::AdditiveChange => "additive_change",
            ApiCompatibilityResult::DeprecatedChange => "deprecated_change",
            ApiCompatibilityResult::BreakingChange => "breaking_change",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_mutation_audit_surface_label(surface: ApiMutationAuditSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(26, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiMutationAuditSurface::Tenant => "tenant",
            ApiMutationAuditSurface::Principal => "principal",
            ApiMutationAuditSurface::VirtualKey => "virtual_key",
            ApiMutationAuditSurface::Policy => "policy",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_mutation_audit_result_label(result: ApiMutationAuditResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(25, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiMutationAuditResult::Required => "required",
            ApiMutationAuditResult::Persisted => "persisted",
            ApiMutationAuditResult::Missing => "missing",
            ApiMutationAuditResult::Failed => "failed",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_version_surface_label(surface: ApiVersionSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(42, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiVersionSurface::DataPlane => "data_plane",
            ApiVersionSurface::ControlPlane => "control_plane",
            ApiVersionSurface::Scim => "scim",
            ApiVersionSurface::Health => "health",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_version_result_label(result: ApiVersionResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(41, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiVersionResult::Accepted => "accepted",
            ApiVersionResult::Defaulted => "defaulted",
            ApiVersionResult::Deprecated => "deprecated",
            ApiVersionResult::Unsupported => "unsupported",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_spec_surface_label(surface: ApiSpecSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(35, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiSpecSurface::GatewayOpenApi => "gateway_openapi",
            ApiSpecSurface::ControlPlaneOpenApi => "control_plane_openapi",
            ApiSpecSurface::ScimSchema => "scim_schema",
            ApiSpecSurface::ErrorEnvelope => "error_envelope",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_spec_publication_result_label(result: ApiSpecPublicationResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(34, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiSpecPublicationResult::Generated => "generated",
            ApiSpecPublicationResult::Validated => "validated",
            ApiSpecPublicationResult::Published => "published",
            ApiSpecPublicationResult::Rejected => "rejected",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_error_envelope_surface_label(surface: ApiErrorEnvelopeSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(22, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiErrorEnvelopeSurface::DataPlane => "data_plane",
            ApiErrorEnvelopeSurface::ControlPlane => "control_plane",
            ApiErrorEnvelopeSurface::Scim => "scim",
            ApiErrorEnvelopeSurface::Health => "health",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_error_envelope_result_label(result: ApiErrorEnvelopeResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(21, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiErrorEnvelopeResult::Emitted => "emitted",
            ApiErrorEnvelopeResult::Redacted => "redacted",
            ApiErrorEnvelopeResult::ValidationFailed => "validation_failed",
            ApiErrorEnvelopeResult::CompatibilityRejected => "compatibility_rejected",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_body_limit_surface_label(surface: ApiBodyLimitSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(14, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiBodyLimitSurface::DataPlane => "data_plane",
            ApiBodyLimitSurface::ControlPlane => "control_plane",
            ApiBodyLimitSurface::Scim => "scim",
            ApiBodyLimitSurface::Upload => "upload",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_body_limit_result_label(result: ApiBodyLimitResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(13, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiBodyLimitResult::Accepted => "accepted",
            ApiBodyLimitResult::RejectedTooLarge => "rejected_too_large",
            ApiBodyLimitResult::UnknownLength => "unknown_length",
            ApiBodyLimitResult::Truncated => "truncated",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_timeout_budget_surface_label(surface: ApiTimeoutBudgetSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(40, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiTimeoutBudgetSurface::DataPlane => "data_plane",
            ApiTimeoutBudgetSurface::ControlPlane => "control_plane",
            ApiTimeoutBudgetSurface::Provider => "provider",
            ApiTimeoutBudgetSurface::Persistence => "persistence",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_timeout_budget_result_label(result: ApiTimeoutBudgetResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(39, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ApiTimeoutBudgetResult::Accepted => "accepted",
            ApiTimeoutBudgetResult::Expired => "expired",
            ApiTimeoutBudgetResult::Exhausted => "exhausted",
            ApiTimeoutBudgetResult::Cancelled => "cancelled",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_cancellation_surface_label(surface: ApiCancellationSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(16, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiCancellationSurface::DataPlane => "data_plane",
            ApiCancellationSurface::ControlPlane => "control_plane",
            ApiCancellationSurface::ProviderStream => "provider_stream",
            ApiCancellationSurface::Persistence => "persistence",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_cancellation_source_label(source: ApiCancellationSource) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(15, source as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match source {
            ApiCancellationSource::ClientDisconnect => "client_disconnect",
            ApiCancellationSource::TimeoutBudget => "timeout_budget",
            ApiCancellationSource::ShutdownDrain => "shutdown_drain",
            ApiCancellationSource::UpstreamAbort => "upstream_abort",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_stream_backpressure_surface_label(surface: ApiStreamBackpressureSurface) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(38, surface as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match surface {
            ApiStreamBackpressureSurface::DataPlaneStream => "data_plane_stream",
            ApiStreamBackpressureSurface::ProviderStream => "provider_stream",
            ApiStreamBackpressureSurface::Websocket => "websocket",
            ApiStreamBackpressureSurface::AuditExport => "audit_export",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn api_stream_backpressure_state_label(state: ApiStreamBackpressureState) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(37, state as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match state {
            ApiStreamBackpressureState::Ready => "ready",
            ApiStreamBackpressureState::Paused => "paused",
            ApiStreamBackpressureState::Dropped => "dropped",
            ApiStreamBackpressureState::Closed => "closed",
        })
        .to_string()
    }
}
