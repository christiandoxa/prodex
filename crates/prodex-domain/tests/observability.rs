use prodex_domain::{
    GatewaySpanDescriptor, GatewaySpanKind, TelemetryAttribute, TelemetryAttributeError,
    TelemetryAttributeErrorStatus, TelemetryAttributeScope, TenantId,
    plan_telemetry_attribute_error_response, tenant_trace_attribute,
};

#[test]
fn gateway_span_descriptor_models_required_data_plane_spans() {
    let expected = [
        GatewaySpanKind::Authentication,
        GatewaySpanKind::TenantResolution,
        GatewaySpanKind::Authorization,
        GatewaySpanKind::BudgetReservation,
        GatewaySpanKind::RoutingDecision,
        GatewaySpanKind::ProviderRequest,
        GatewaySpanKind::StreamingLifecycle,
        GatewaySpanKind::Persistence,
        GatewaySpanKind::Reconciliation,
        GatewaySpanKind::AuditEmission,
    ];

    for kind in expected {
        let span = GatewaySpanDescriptor::new(kind, format!("prodex.gateway.{kind:?}"));
        assert_eq!(span.kind, kind);
    }
}

#[test]
fn metric_labels_allow_low_cardinality_provider_route_and_status() {
    let span = GatewaySpanDescriptor::new(GatewaySpanKind::ProviderRequest, "provider.request")
        .with_attribute(TelemetryAttribute::metric_label("provider", "openai"))
        .with_attribute(TelemetryAttribute::metric_label("route", "responses"))
        .with_attribute(TelemetryAttribute::metric_label("status_class", "2xx"));

    assert_eq!(
        span.metric_labels().unwrap(),
        vec![
            ("provider", "openai"),
            ("route", "responses"),
            ("status_class", "2xx")
        ]
    );
}

#[test]
fn metric_labels_reject_raw_tenant_user_key_prompt_and_request_identifiers() {
    for key in [
        "tenant_id",
        "tenantId",
        "tenant",
        "user.id",
        "userId",
        "user",
        "principal-id",
        "principal",
        "virtual_key",
        "api_key_hash",
        "apiKeyHash",
        "key",
        "token",
        "secret",
        "credential",
        "password",
        "request_id",
        "requestId",
        "request",
        "call.id",
        "callId",
        "call",
        "prompt_text",
        "prompt",
    ] {
        let attribute = TelemetryAttribute::metric_label(key, "raw-value");
        assert_eq!(
            attribute.as_metric_label(),
            Err(TelemetryAttributeError::ForbiddenMetricLabelKey),
            "key should be forbidden as metric label: {key}"
        );
    }
}

#[test]
fn metric_label_keys_reject_empty_whitespace_control_and_non_ascii_values() {
    for key in ["", " ", "provider name", "provider\nname", "provider-é"] {
        assert_eq!(
            TelemetryAttribute::metric_label(key, "openai").as_metric_label(),
            Err(TelemetryAttributeError::ForbiddenMetricLabelKey),
            "key should be forbidden as metric label: {key:?}"
        );
    }
}

#[test]
fn metric_label_guard_preserves_closed_domain_surface_labels() {
    for key in [
        "tenant_isolation_surface",
        "user_lifecycle_operation",
        "api_precondition_surface",
        "request_count_total",
        "secret_backend",
        "credential_rotation_result",
    ] {
        assert_eq!(
            TelemetryAttribute::metric_label(key, "closed_value").as_metric_label(),
            Ok((key, "closed_value"))
        );
    }
}

#[test]
fn trace_only_and_redacted_attributes_do_not_become_metric_labels() {
    assert_eq!(
        TelemetryAttribute::trace_only("tenant_id", "tenant-a").as_metric_label(),
        Err(TelemetryAttributeError::TraceOnlyAttribute)
    );
    let redacted = TelemetryAttribute::redacted_trace_only("prompt");
    assert_eq!(redacted.value, "<redacted>");
    assert_eq!(
        redacted.as_metric_label(),
        Err(TelemetryAttributeError::TraceOnlyAttribute)
    );
}

#[test]
fn tenant_trace_attribute_keeps_raw_tenant_id_out_of_metric_labels() {
    let tenant_id = TenantId::new();
    let attribute = tenant_trace_attribute(tenant_id);

    assert_eq!(attribute.key, "tenant_id");
    assert_eq!(attribute.value, tenant_id.to_string());
    assert_eq!(attribute.scope, TelemetryAttributeScope::TraceOnly);
    assert_eq!(
        attribute.as_metric_label(),
        Err(TelemetryAttributeError::TraceOnlyAttribute)
    );
}

#[test]
fn telemetry_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let attribute = tenant_trace_attribute(tenant_id);
    let span = GatewaySpanDescriptor::new(GatewaySpanKind::TenantResolution, "tenant.resolve")
        .with_attribute(attribute.clone())
        .with_attribute(TelemetryAttribute::metric_label("provider", "openai"));
    let too_long = TelemetryAttributeError::MetricLabelValueTooLong { length: 1_000 };

    let rendered = format!("{attribute:?} {span:?} {too_long:?}");
    for sensitive in [
        tenant_id.to_string(),
        "openai".to_string(),
        "tenant.resolve".to_string(),
        "1000".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "telemetry debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("key: \"tenant_id\""));
    assert!(rendered.contains("value: \"<redacted>\""));
    assert!(rendered.contains("name: \"<redacted>\""));
    assert!(rendered.contains("attributes: \"<redacted>\""));
    assert!(rendered.contains("MetricLabelValueTooLong { length: \"<redacted>\" }"));
}

#[test]
fn metric_label_values_are_bounded_to_reduce_cardinality_pressure() {
    let value = "a".repeat(129);
    assert_eq!(
        TelemetryAttribute::metric_label("provider", value).as_metric_label(),
        Err(TelemetryAttributeError::MetricLabelValueTooLong { length: 129 })
    );
}

#[test]
fn metric_label_keys_are_bounded_to_reduce_cardinality_pressure() {
    let key = "a".repeat(129);
    assert_eq!(
        TelemetryAttribute::metric_label(key, "openai").as_metric_label(),
        Err(TelemetryAttributeError::MetricLabelKeyTooLong { length: 129 })
    );
}

#[test]
fn metric_label_values_reject_empty_whitespace_control_and_non_ascii_values() {
    for value in ["", " ", "open ai", "open\nai", "openai-é"] {
        assert_eq!(
            TelemetryAttribute::metric_label("provider", value).as_metric_label(),
            Err(TelemetryAttributeError::ForbiddenMetricLabelValue),
            "value should be forbidden as metric label: {value:?}"
        );
    }
}

#[test]
fn metric_label_values_reject_raw_uuid_like_identifiers() {
    let tenant_id = TenantId::new().to_string();
    assert_eq!(
        TelemetryAttribute::metric_label("provider", tenant_id).as_metric_label(),
        Err(TelemetryAttributeError::ForbiddenMetricLabelValue)
    );
    assert_eq!(
        TelemetryAttribute::metric_label("provider", "018f1f77bc2476d4a4b2c65b3f3d9a01")
            .as_metric_label(),
        Err(TelemetryAttributeError::ForbiddenMetricLabelValue)
    );

    assert_eq!(
        TelemetryAttribute::metric_label("provider", "openai").as_metric_label(),
        Ok(("provider", "openai"))
    );
}

#[test]
fn metric_label_values_reject_secret_like_prefixes() {
    for value in [
        "Bearer token-value",
        "  bearer token-value",
        "sk-project-secret",
        "ghp_secret",
        "gho_secret",
        "github_pat_secret",
    ] {
        assert_eq!(
            TelemetryAttribute::metric_label("provider", value).as_metric_label(),
            Err(TelemetryAttributeError::ForbiddenMetricLabelValue),
            "value should be forbidden as metric label: {value}"
        );
    }

    for value in ["openai", "success", "2xx"] {
        assert_eq!(
            TelemetryAttribute::metric_label("provider", value).as_metric_label(),
            Ok(("provider", value))
        );
    }
}

#[test]
fn metric_label_values_reject_jwt_like_values() {
    let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJwcmluY2lwYWwifQ.signature";

    assert_eq!(
        TelemetryAttribute::metric_label("provider", jwt).as_metric_label(),
        Err(TelemetryAttributeError::ForbiddenMetricLabelValue)
    );
    assert_eq!(
        TelemetryAttribute::metric_label("provider", "openai.responses").as_metric_label(),
        Ok(("provider", "openai.responses"))
    );
}

#[test]
fn telemetry_attribute_error_responses_are_stable_and_redacted() {
    let forbidden =
        plan_telemetry_attribute_error_response(&TelemetryAttributeError::ForbiddenMetricLabelKey);
    assert_eq!(
        forbidden.status,
        TelemetryAttributeErrorStatus::InvalidRequest
    );
    assert_eq!(forbidden.code, "telemetry_metric_label_forbidden");
    assert_eq!(forbidden.message, "telemetry attribute is invalid");

    let too_long = plan_telemetry_attribute_error_response(
        &TelemetryAttributeError::MetricLabelValueTooLong { length: 1_000 },
    );
    assert_eq!(too_long.code, "telemetry_metric_label_invalid");

    let long_key =
        plan_telemetry_attribute_error_response(&TelemetryAttributeError::MetricLabelKeyTooLong {
            length: 1_000,
        });
    assert_eq!(long_key.code, "telemetry_metric_label_invalid");

    let forbidden_value = plan_telemetry_attribute_error_response(
        &TelemetryAttributeError::ForbiddenMetricLabelValue,
    );
    assert_eq!(forbidden_value.code, "telemetry_metric_label_forbidden");

    let trace_only =
        plan_telemetry_attribute_error_response(&TelemetryAttributeError::TraceOnlyAttribute);
    assert_eq!(trace_only.code, "telemetry_attribute_scope_invalid");

    let rendered = format!("{forbidden:?} {too_long:?} {long_key:?} {trace_only:?}");
    for sensitive in [
        "tenant_id",
        "user_id",
        "principal_id",
        "virtual_key",
        "api_key",
        "request_id",
        "call_id",
        "prompt",
        "1000",
        "raw-value",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "telemetry response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}
