use prodex_domain::{
    AuditEventId, CallId, CorrelationContext, GatewaySpanKind, JwksCacheSnapshot,
    PolicyCacheStatus, PolicyDigest, PolicyRefreshWindow, PolicyRevisionId, PolicySignature,
    PolicySnapshot, RequestId, TelemetryAttribute, TelemetryAttributeError, TenantId, TraceId,
};
use prodex_observability::{
    AccountingOperation, AccountingResult, ApiAdmissionResult, ApiBodyLimitResult,
    ApiBodyLimitSurface, ApiCancellationSource, ApiCancellationSurface, ApiCompatibilityResult,
    ApiCompatibilitySurface, ApiDeprecationSignal, ApiDeprecationSurface, ApiErrorEnvelopeResult,
    ApiErrorEnvelopeSurface, ApiIdempotencyResult, ApiIdempotencySurface, ApiMutationAuditResult,
    ApiMutationAuditSurface, ApiPaginationResult, ApiPaginationSurface, ApiPreconditionResult,
    ApiPreconditionSurface, ApiRouteKind, ApiSchemaSurface, ApiSchemaValidationResult,
    ApiSpecPublicationResult, ApiSpecSurface, ApiStatusClass, ApiStreamBackpressureState,
    ApiStreamBackpressureSurface, ApiTimeoutBudgetResult, ApiTimeoutBudgetSurface,
    ApiVersionResult, ApiVersionSurface, AuditChainOperation, AuditChainResult, AuditOperation,
    AuditQueryLifecycleOperation, AuditQueryLifecycleResult, AuditResult,
    AuditRetentionPurgeOperation, AuditRetentionPurgeResult, AuthnTokenValidationResult,
    AuthnTokenValidationStage, AuthzBoundaryKind, AuthzDecisionResult, BackupRestoreOperation,
    BackupRestoreResult, BillingLedgerOperation, BillingLedgerResult, BreakGlassLifecycleOperation,
    BreakGlassLifecycleResult, BudgetPolicyLifecycleOperation, BudgetPolicyLifecycleResult,
    BudgetRejectionReason, ConfigActivationResult, ConfigActivationSource,
    ConfigCacheInvalidationResult, ConfigCacheInvalidationTarget, ConfigPublicationDeliveryResult,
    ConfigPublicationDeliveryTarget, ConnectionPoolKind, CredentialScopeMismatchDirection,
    CredentialScopeMismatchResult, DeploymentRolloutOperation, DeploymentRolloutResult,
    EnterpriseIdKind, EnterpriseIdResult, FaultInjectionResult, FaultInjectionTarget,
    HealthProbeKind, HealthProbeResult, IdempotencyRecordBackend, IdempotencyRecordOperation,
    IdempotencyRecordResult, IdentityContextResult, IdentityContextSurface, JwksRefreshOutcome,
    LoadSoakResult, LoadSoakScenarioKind, MigrationLifecycleOperation, MigrationLifecycleResult,
    ObservabilityErrorStatus, OidcRefreshOperation, OidcRefreshResult, PersistenceOperation,
    PersistenceResult, PolicyLifecycleOperation, PolicyLifecycleResult, PolicyRefreshOutcome,
    PolicyRollbackOperation, PolicyRollbackResult, PostgresTenantContextOperation,
    PostgresTenantContextResult, ProviderCapabilityKind, ProviderCapabilityResult,
    ProviderCircuitBreakerDecision, ProviderCircuitBreakerEvent,
    ProviderCredentialLifecycleOperation, ProviderCredentialLifecycleResult,
    ProviderDegradationSeverity, ProviderDegradationSignal, ProviderKind, ProviderResultClass,
    ProviderRetryAttemptStage, ProviderRetryOutcome, QueueDepthKind, QuotaCorrectnessEvent,
    RateLimitDecision, RateLimitScope, RedisCoordinationOperation, RedisCoordinationResult,
    ReservationRecoveryOperation, ReservationRecoveryResult, RoleBindingLifecycleOperation,
    RoleBindingLifecycleResult, RoutingDecisionOutcome, RoutingLaneKind, SecretProviderBackend,
    SecretProviderOperation, SecretProviderResult, SecretRotationResult, SecretRotationScope,
    SecurityDecisionKind, SecurityDecisionResult, ServiceIdentityLifecycleOperation,
    ServiceIdentityLifecycleResult, ShutdownLifecycleEvent, ShutdownLifecycleResult,
    SloAlertSeverity, SloAlertSli, SpanPlanError, StreamOutcome, StreamTransportKind,
    TelemetryDropReason, TenantIsolationResult, TenantIsolationSurface, TenantLifecycleOperation,
    TenantLifecycleResult, TraceContext, TraceContextError, TracePropagationCarrier,
    TracePropagationResult, UserLifecycleOperation, UserLifecycleResult,
    VirtualKeyLifecycleOperation, VirtualKeyLifecycleResult, plan_accounting_metric,
    plan_api_admission_metric, plan_api_body_limit_metric, plan_api_cancellation_metric,
    plan_api_compatibility_metric, plan_api_deprecation_metric, plan_api_error_envelope_metric,
    plan_api_idempotency_metric, plan_api_mutation_audit_metric, plan_api_pagination_metric,
    plan_api_precondition_metric, plan_api_red_metric, plan_api_schema_validation_metric,
    plan_api_spec_publication_metric, plan_api_stream_backpressure_metric,
    plan_api_timeout_budget_metric, plan_api_version_metric, plan_audit_chain_metric,
    plan_audit_metric, plan_audit_query_lifecycle_metric, plan_audit_retention_purge_metric,
    plan_authn_token_validation_metric, plan_authz_decision_metric, plan_backup_restore_metric,
    plan_billing_ledger_metric, plan_break_glass_lifecycle_metric,
    plan_budget_policy_lifecycle_metric, plan_budget_rejection_metric,
    plan_config_activation_metric, plan_config_cache_invalidation_metric,
    plan_config_publication_delivery_metric, plan_connection_pool_saturation_metric,
    plan_credential_scope_mismatch_metric, plan_deployment_rollout_metric,
    plan_dropped_telemetry_metric, plan_enterprise_id_metric, plan_fault_injection_metric,
    plan_gateway_span, plan_health_probe_metric, plan_idempotency_record_metric,
    plan_identity_context_metric, plan_jwks_cache_age_metric, plan_jwks_refresh_outcome_metric,
    plan_load_soak_metric, plan_migration_lifecycle_metric, plan_oidc_refresh_metric,
    plan_persistence_metric, plan_policy_lifecycle_metric, plan_policy_refresh_outcome_metric,
    plan_policy_rollback_metric, plan_policy_snapshot_age_metric,
    plan_postgres_tenant_context_metric, plan_provider_capability_negotiation_metric,
    plan_provider_circuit_breaker_metric, plan_provider_credential_lifecycle_metric,
    plan_provider_degradation_metric, plan_provider_metric, plan_provider_retry_metric,
    plan_queue_depth_metric, plan_quota_correctness_metric, plan_rate_limit_decision_metric,
    plan_redis_coordination_metric, plan_reservation_recovery_metric,
    plan_role_binding_lifecycle_metric, plan_routing_decision_metric, plan_secret_provider_metric,
    plan_secret_rotation_metric, plan_security_decision_metric,
    plan_service_identity_lifecycle_metric, plan_shutdown_lifecycle_metric, plan_slo_alert_metric,
    plan_span_error_response, plan_streaming_lifecycle_metric, plan_structured_log_correlation,
    plan_tenant_isolation_metric, plan_tenant_lifecycle_metric, plan_trace_context_error_response,
    plan_trace_propagation, plan_trace_propagation_metric, plan_user_lifecycle_metric,
    plan_virtual_key_lifecycle_metric,
};

#[test]
fn trace_context_parses_and_renders_w3c_traceparent() {
    let context =
        TraceContext::parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
            .unwrap();

    assert_eq!(
        context.traceparent(),
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
    );
}

#[test]
fn trace_context_rejects_malformed_or_invalid_context() {
    assert_eq!(
        TraceContext::parse_traceparent("01-abc"),
        Err(TraceContextError::MalformedTraceparent)
    );
    assert_eq!(
        TraceContext::new("4bf92f3577b34da6a3ce929d0e0e4736", "short", "01"),
        Err(TraceContextError::InvalidSpanId)
    );
    assert_eq!(
        TraceContext::new(
            "4bf92f3577b34da6a3ce929d0e0e4736",
            "00f067aa0ba902b7",
            "zzz"
        ),
        Err(TraceContextError::InvalidFlags)
    );
}

#[test]
fn trace_propagation_renders_traceparent_and_optional_state_headers() {
    let context =
        TraceContext::parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
            .unwrap();

    let propagation = plan_trace_propagation(
        &context,
        Some("rojo=00f067aa0ba902b7"),
        Some("tenant_tier=premium,region=us"),
    )
    .unwrap();

    assert_eq!(
        propagation.traceparent,
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
    );
    assert_eq!(
        propagation.tracestate.as_deref(),
        Some("rojo=00f067aa0ba902b7")
    );
    assert_eq!(
        propagation.baggage.as_deref(),
        Some("tenant_tier=premium,region=us")
    );
}

#[test]
fn trace_propagation_rejects_empty_or_control_character_state_headers() {
    let context =
        TraceContext::parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
            .unwrap();

    assert_eq!(
        plan_trace_propagation(&context, Some("   "), None),
        Err(TraceContextError::InvalidTracestate)
    );
    assert_eq!(
        plan_trace_propagation(&context, Some("vendor=value\nbad=value"), None),
        Err(TraceContextError::InvalidTracestate)
    );
    assert_eq!(
        plan_trace_propagation(&context, None, Some("tenant=prod\nbad=value")),
        Err(TraceContextError::InvalidBaggage)
    );
}

#[test]
fn trace_propagation_metric_uses_closed_carrier_and_result_labels() {
    let traceparent = plan_trace_propagation_metric(
        TracePropagationCarrier::Traceparent,
        TracePropagationResult::Propagated,
    )
    .unwrap();
    let tracestate = plan_trace_propagation_metric(
        TracePropagationCarrier::Tracestate,
        TracePropagationResult::Rejected,
    )
    .unwrap();
    let baggage = plan_trace_propagation_metric(
        TracePropagationCarrier::Baggage,
        TracePropagationResult::Missing,
    )
    .unwrap();

    assert_eq!(
        traceparent.metric_name,
        "prodex_trace_propagation_events_total"
    );
    assert_eq!(traceparent.increment, 1);
    assert_eq!(
        traceparent.carrier_label.as_metric_label().unwrap(),
        ("trace_carrier", "traceparent")
    );
    assert_eq!(
        traceparent.result_label.as_metric_label().unwrap(),
        ("trace_propagation_result", "propagated")
    );
    assert_eq!(
        tracestate.carrier_label.as_metric_label().unwrap(),
        ("trace_carrier", "tracestate")
    );
    assert_eq!(
        tracestate.result_label.as_metric_label().unwrap(),
        ("trace_propagation_result", "rejected")
    );
    assert_eq!(
        baggage.carrier_label.as_metric_label().unwrap(),
        ("trace_carrier", "baggage")
    );
    assert_eq!(
        baggage.result_label.as_metric_label().unwrap(),
        ("trace_propagation_result", "missing")
    );

    for metric in [traceparent, tracestate, baggage] {
        assert!(!metric.carrier_label.key.contains("tenant"));
        assert!(!metric.carrier_label.key.contains("request"));
        assert!(!metric.result_label.key.contains("trace_id"));
        assert!(!metric.result_label.key.contains("span_id"));
        assert!(!metric.result_label.key.contains("header_value"));
    }
}

#[test]
fn trace_context_error_response_is_stable_and_redacted() {
    let response = plan_trace_context_error_response(&TraceContextError::MalformedTraceparent);
    let invalid_span = TraceContextError::InvalidSpanId;
    assert!(!invalid_span.to_string().contains("span id"));
    let malformed = TraceContextError::MalformedTraceparent;
    assert!(!malformed.to_string().contains("traceparent"));
    let invalid_tracestate = TraceContextError::InvalidTracestate;
    assert!(!invalid_tracestate.to_string().contains("tracestate"));
    let invalid_baggage = TraceContextError::InvalidBaggage;
    assert!(!invalid_baggage.to_string().contains("baggage"));

    assert_eq!(
        response.status,
        ObservabilityErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "invalid_trace_context");
    assert_eq!(
        response.message,
        "trace context is required and must be valid"
    );
    assert!(!response.message.contains("traceparent"));
    assert!(!response.message.contains("span id"));
    assert!(!response.message.contains("00-"));
}

#[test]
fn span_plan_requires_trace_context_or_correlation_trace() {
    let correlation = CorrelationContext::new(RequestId::new()).with_call_id(CallId::new());

    assert_eq!(
        plan_gateway_span(
            GatewaySpanKind::RoutingDecision,
            "prodex.gateway.route",
            correlation,
            None,
            vec![],
        ),
        Err(SpanPlanError::MissingCorrelationTrace)
    );
}

#[test]
fn span_plan_error_response_is_stable_and_redacted() {
    let response = plan_span_error_response(&SpanPlanError::MissingCorrelationTrace);

    assert_eq!(
        response.status,
        ObservabilityErrorStatus::ServiceUnavailable
    );
    assert_eq!(response.code, "telemetry_unavailable");
    assert_eq!(
        response.message,
        "telemetry planning is temporarily unavailable"
    );
    assert!(!response.message.contains("traceparent"));
    assert!(!response.message.contains("tenant"));
    assert!(!response.message.contains("request"));
}

#[test]
fn span_plan_accepts_trace_context_and_low_cardinality_metric_labels() {
    let trace_context =
        TraceContext::parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
            .unwrap();
    let correlation = CorrelationContext::new(RequestId::new())
        .with_trace_id(TraceId::new("4bf92f3577b34da6a3ce929d0e0e4736").unwrap());

    let span = plan_gateway_span(
        GatewaySpanKind::ProviderRequest,
        "prodex.provider.request",
        correlation,
        Some(trace_context),
        vec![
            TelemetryAttribute::metric_label("provider", "openai"),
            TelemetryAttribute::trace_only("request_id", "not-a-label"),
        ],
    )
    .unwrap();

    assert_eq!(
        span.descriptor.metric_labels().unwrap(),
        vec![("provider", "openai")]
    );
}

#[test]
fn structured_log_correlation_uses_trace_only_high_cardinality_fields() {
    let request_id = RequestId::new();
    let call_id = CallId::new();
    let trace_id = TraceId::new("4bf92f3577b34da6a3ce929d0e0e4736").unwrap();
    let tenant_id = TenantId::new();
    let audit_event_id = AuditEventId::new();
    let correlation = CorrelationContext::new(request_id)
        .with_call_id(call_id)
        .with_trace_id(trace_id.clone())
        .with_tenant_id(tenant_id)
        .with_audit_event_id(audit_event_id);

    let plan = plan_structured_log_correlation(&correlation);

    assert_eq!(plan.fields.len(), 5);
    assert_eq!(plan.fields[0].key, "request_id");
    assert_eq!(plan.fields[0].value, request_id.to_string());
    assert_eq!(plan.fields[1].key, "call_id");
    assert_eq!(plan.fields[1].value, call_id.to_string());
    assert_eq!(plan.fields[2].key, "trace_id");
    assert_eq!(plan.fields[2].value, trace_id.as_str());
    assert_eq!(plan.fields[3].key, "tenant_id");
    assert_eq!(plan.fields[3].value, tenant_id.to_string());
    assert_eq!(plan.fields[4].key, "audit_event_id");
    assert_eq!(plan.fields[4].value, audit_event_id.to_string());
    for field in plan.fields {
        assert_eq!(
            field.as_metric_label(),
            Err(TelemetryAttributeError::TraceOnlyAttribute)
        );
    }
}

#[test]
fn structured_log_correlation_keeps_optional_fields_absent_until_known() {
    let request_id = RequestId::new();
    let correlation = CorrelationContext::new(request_id);

    let plan = plan_structured_log_correlation(&correlation);

    assert_eq!(plan.fields.len(), 1);
    assert_eq!(plan.fields[0].key, "request_id");
    assert_eq!(plan.fields[0].value, request_id.to_string());
    assert_eq!(
        plan.fields[0].as_metric_label(),
        Err(TelemetryAttributeError::TraceOnlyAttribute)
    );
}

#[test]
fn enterprise_id_metric_uses_closed_kind_and_result_labels() {
    let cases = [
        (EnterpriseIdKind::Tenant, "tenant"),
        (EnterpriseIdKind::Principal, "principal"),
        (EnterpriseIdKind::Request, "request"),
        (EnterpriseIdKind::Call, "call"),
        (EnterpriseIdKind::Reservation, "reservation"),
        (EnterpriseIdKind::VirtualKey, "virtual_key"),
        (EnterpriseIdKind::PolicyRevision, "policy_revision"),
        (EnterpriseIdKind::AuditEvent, "audit_event"),
    ];

    for (kind, expected_kind) in cases {
        let metric = plan_enterprise_id_metric(kind, EnterpriseIdResult::Generated).unwrap();

        assert_eq!(metric.metric_name, "prodex_enterprise_id_events_total");
        assert_eq!(metric.increment, 1);
        assert_eq!(metric.kind_label.key, "enterprise_id_kind");
        assert_eq!(metric.kind_label.value, expected_kind);
        assert_eq!(metric.result_label.key, "enterprise_id_result");
        assert_eq!(metric.result_label.value, "generated");
        for label in [metric.kind_label, metric.result_label] {
            for forbidden in [
                "tenant_id",
                "principal_id",
                "request_id",
                "call_id",
                "reservation_id",
                "virtual_key_id",
                "policy_revision_id",
                "audit_event_id",
                "uuid",
            ] {
                assert!(!label.key.contains(forbidden));
            }
        }
    }

    assert_eq!(
        plan_enterprise_id_metric(EnterpriseIdKind::Request, EnterpriseIdResult::Parsed)
            .unwrap()
            .result_label
            .value,
        "parsed"
    );
    assert_eq!(
        plan_enterprise_id_metric(EnterpriseIdKind::Request, EnterpriseIdResult::Rejected)
            .unwrap()
            .result_label
            .value,
        "rejected"
    );
}

#[test]
fn span_plan_rejects_high_cardinality_metric_label_keys() {
    let trace_context =
        TraceContext::parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
            .unwrap();
    let correlation = CorrelationContext::new(RequestId::new());

    let error = plan_gateway_span(
        GatewaySpanKind::Authentication,
        "prodex.authn",
        correlation,
        Some(trace_context),
        vec![TelemetryAttribute::metric_label("tenant_id", "raw-tenant")],
    )
    .unwrap_err();

    assert!(matches!(error, SpanPlanError::MetricLabel(_)));
    assert!(!error.to_string().contains("tenant_id"));
    assert!(!error.to_string().contains("raw-tenant"));
}

#[test]
fn jwks_cache_age_metric_uses_only_low_cardinality_state_label() {
    let fresh = JwksCacheSnapshot {
        fetched_at_unix_ms: 1_000,
        expires_at_unix_ms: 10_000,
        stale_until_unix_ms: 20_000,
        key_count: 2,
        last_refresh_error_at_unix_ms: None,
        retry_after_unix_ms: None,
    };

    let metric = plan_jwks_cache_age_metric(Some(&fresh), 4_500).unwrap();

    assert_eq!(metric.metric_name, "prodex_jwks_cache_age_ms");
    assert_eq!(metric.age_ms, Some(3_500));
    assert_eq!(
        metric.state_label.as_metric_label().unwrap(),
        ("jwks_cache_state", "fresh")
    );
}

#[test]
fn jwks_cache_age_metric_reports_lkg_and_unavailable_without_issuer_or_key_labels() {
    let lkg = JwksCacheSnapshot {
        fetched_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
        stale_until_unix_ms: 20_000,
        key_count: 2,
        last_refresh_error_at_unix_ms: Some(2_100),
        retry_after_unix_ms: Some(10_000),
    };
    let unavailable = JwksCacheSnapshot {
        fetched_at_unix_ms: 1_000,
        expires_at_unix_ms: 2_000,
        stale_until_unix_ms: 3_000,
        key_count: 0,
        last_refresh_error_at_unix_ms: Some(2_100),
        retry_after_unix_ms: Some(10_000),
    };

    let lkg_metric = plan_jwks_cache_age_metric(Some(&lkg), 5_000).unwrap();
    let unavailable_metric = plan_jwks_cache_age_metric(Some(&unavailable), 5_000).unwrap();
    let missing_metric = plan_jwks_cache_age_metric(None, 5_000).unwrap();

    assert_eq!(
        lkg_metric.state_label.as_metric_label().unwrap(),
        ("jwks_cache_state", "last_known_good_backoff")
    );
    assert_eq!(
        unavailable_metric.state_label.as_metric_label().unwrap(),
        ("jwks_cache_state", "unavailable")
    );
    assert_eq!(
        missing_metric.state_label.as_metric_label().unwrap(),
        ("jwks_cache_state", "refresh_now")
    );
    assert_eq!(missing_metric.age_ms, None);
    for metric in [lkg_metric, unavailable_metric, missing_metric] {
        assert!(!metric.state_label.key.contains("issuer"));
        assert!(!metric.state_label.key.contains("tenant"));
        assert!(!metric.state_label.key.contains("key_id"));
    }
}

#[test]
fn policy_snapshot_age_metric_uses_only_low_cardinality_state_label() {
    let active_revision_id = PolicyRevisionId::new();
    let snapshot = PolicySnapshot::new(
        active_revision_id,
        1_000,
        "policy",
        PolicyDigest::new("digest"),
        PolicySignature::new("signature"),
    );
    let status = PolicyCacheStatus {
        active_revision_id: Some(active_revision_id),
        last_known_good_revision_id: Some(active_revision_id),
        invalidated_revision_id: None,
        refresh_window: PolicyRefreshWindow::new(10_000, 20_000, 30_000).unwrap(),
    };

    let metric = plan_policy_snapshot_age_metric(Some(&snapshot), &status, 4_500).unwrap();

    assert_eq!(metric.metric_name, "prodex_policy_snapshot_age_ms");
    assert_eq!(metric.age_ms, Some(3_500));
    assert_eq!(
        metric.state_label.as_metric_label().unwrap(),
        ("policy_cache_state", "active")
    );
    assert!(!metric.state_label.key.contains("revision"));
    assert!(!metric.state_label.key.contains("tenant"));
}

#[test]
fn policy_snapshot_age_metric_reports_lkg_expired_and_missing_without_revision_labels() {
    let active_revision_id = PolicyRevisionId::new();
    let lkg_revision_id = PolicyRevisionId::new();
    let snapshot = PolicySnapshot::new(
        active_revision_id,
        1_000,
        "policy",
        PolicyDigest::new("digest"),
        PolicySignature::new("signature"),
    );
    let status = PolicyCacheStatus {
        active_revision_id: Some(active_revision_id),
        last_known_good_revision_id: Some(lkg_revision_id),
        invalidated_revision_id: None,
        refresh_window: PolicyRefreshWindow::new(2_000, 3_000, 4_000).unwrap(),
    };
    let invalidated = status.clone().invalidate_revision(active_revision_id);

    let lkg_metric = plan_policy_snapshot_age_metric(Some(&snapshot), &status, 3_500).unwrap();
    let expired_metric = plan_policy_snapshot_age_metric(Some(&snapshot), &status, 4_000).unwrap();
    let invalidated_metric =
        plan_policy_snapshot_age_metric(Some(&snapshot), &invalidated, 1_500).unwrap();
    let missing_metric = plan_policy_snapshot_age_metric::<&str>(None, &status, 1_500).unwrap();

    assert_eq!(
        lkg_metric.state_label.as_metric_label().unwrap(),
        ("policy_cache_state", "last_known_good_refresh")
    );
    assert_eq!(
        expired_metric.state_label.as_metric_label().unwrap(),
        ("policy_cache_state", "expired")
    );
    assert_eq!(
        invalidated_metric.state_label.as_metric_label().unwrap(),
        ("policy_cache_state", "invalidated")
    );
    assert_eq!(
        missing_metric.state_label.as_metric_label().unwrap(),
        ("policy_cache_state", "active")
    );
    assert_eq!(missing_metric.age_ms, None);
    for metric in [
        lkg_metric,
        expired_metric,
        invalidated_metric,
        missing_metric,
    ] {
        assert!(!metric.state_label.key.contains("revision"));
        assert!(!metric.state_label.key.contains("tenant"));
        assert!(!metric.state_label.key.contains("policy_id"));
    }
}

#[test]
fn jwks_refresh_outcome_metric_uses_low_cardinality_result_label() {
    let success = plan_jwks_refresh_outcome_metric(JwksRefreshOutcome::Success).unwrap();
    let failure = plan_jwks_refresh_outcome_metric(JwksRefreshOutcome::Failure).unwrap();

    assert_eq!(success.metric_name, "prodex_jwks_refresh_total");
    assert_eq!(success.increment, 1);
    assert_eq!(
        success.result_label.as_metric_label().unwrap(),
        ("jwks_refresh_result", "success")
    );
    assert_eq!(
        failure.result_label.as_metric_label().unwrap(),
        ("jwks_refresh_result", "failure")
    );
    for metric in [success, failure] {
        assert!(!metric.result_label.key.contains("issuer"));
        assert!(!metric.result_label.key.contains("tenant"));
        assert!(!metric.result_label.key.contains("key_id"));
    }
}

#[test]
fn oidc_refresh_metric_uses_closed_operation_and_result_labels() {
    let discovery = plan_oidc_refresh_metric(
        OidcRefreshOperation::DiscoverIssuer,
        OidcRefreshResult::Success,
    )
    .unwrap();
    let fetch =
        plan_oidc_refresh_metric(OidcRefreshOperation::FetchJwks, OidcRefreshResult::Backoff)
            .unwrap();
    let validate = plan_oidc_refresh_metric(
        OidcRefreshOperation::ValidateSnapshot,
        OidcRefreshResult::InvalidSnapshot,
    )
    .unwrap();
    let write = plan_oidc_refresh_metric(
        OidcRefreshOperation::WriteCache,
        OidcRefreshResult::SkippedFresh,
    )
    .unwrap();
    let failed =
        plan_oidc_refresh_metric(OidcRefreshOperation::FetchJwks, OidcRefreshResult::Failed)
            .unwrap();

    assert_eq!(discovery.metric_name, "prodex_oidc_refresh_events_total");
    assert_eq!(discovery.increment, 1);
    assert_eq!(
        discovery.operation_label.as_metric_label().unwrap(),
        ("oidc_refresh_operation", "discover_issuer")
    );
    assert_eq!(
        discovery.result_label.as_metric_label().unwrap(),
        ("oidc_refresh_result", "success")
    );
    assert_eq!(
        fetch.operation_label.as_metric_label().unwrap(),
        ("oidc_refresh_operation", "fetch_jwks")
    );
    assert_eq!(
        fetch.result_label.as_metric_label().unwrap(),
        ("oidc_refresh_result", "backoff")
    );
    assert_eq!(
        validate.operation_label.as_metric_label().unwrap(),
        ("oidc_refresh_operation", "validate_snapshot")
    );
    assert_eq!(
        validate.result_label.as_metric_label().unwrap(),
        ("oidc_refresh_result", "invalid_snapshot")
    );
    assert_eq!(
        write.operation_label.as_metric_label().unwrap(),
        ("oidc_refresh_operation", "write_cache")
    );
    assert_eq!(
        write.result_label.as_metric_label().unwrap(),
        ("oidc_refresh_result", "skipped_fresh")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("oidc_refresh_result", "failed")
    );

    for metric in [discovery, fetch, validate, write, failed] {
        assert!(!metric.operation_label.key.contains("issuer"));
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("key_id"));
        assert!(!metric.operation_label.key.contains("endpoint"));
        assert!(!metric.result_label.key.contains("jwks"));
        assert!(!metric.result_label.key.contains("token"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn policy_refresh_outcome_metric_uses_low_cardinality_result_label() {
    let success = plan_policy_refresh_outcome_metric(PolicyRefreshOutcome::Success).unwrap();
    let failure = plan_policy_refresh_outcome_metric(PolicyRefreshOutcome::Failure).unwrap();
    let fallback =
        plan_policy_refresh_outcome_metric(PolicyRefreshOutcome::LastKnownGoodFallback).unwrap();

    assert_eq!(success.metric_name, "prodex_policy_refresh_total");
    assert_eq!(success.increment, 1);
    assert_eq!(
        success.result_label.as_metric_label().unwrap(),
        ("policy_refresh_result", "success")
    );
    assert_eq!(
        failure.result_label.as_metric_label().unwrap(),
        ("policy_refresh_result", "failure")
    );
    assert_eq!(
        fallback.result_label.as_metric_label().unwrap(),
        ("policy_refresh_result", "last_known_good_fallback")
    );
    for metric in [success, failure, fallback] {
        assert!(!metric.result_label.key.contains("tenant"));
        assert!(!metric.result_label.key.contains("revision"));
        assert!(!metric.result_label.key.contains("digest"));
        assert!(!metric.result_label.key.contains("error"));
    }
}

#[test]
fn policy_rollback_metric_uses_closed_operation_and_result_labels() {
    let lkg = plan_policy_rollback_metric(
        PolicyRollbackOperation::ActivateLastKnownGood,
        PolicyRollbackResult::Success,
    )
    .unwrap();
    let reject = plan_policy_rollback_metric(
        PolicyRollbackOperation::RejectCandidate,
        PolicyRollbackResult::Blocked,
    )
    .unwrap();
    let rollback = plan_policy_rollback_metric(
        PolicyRollbackOperation::Rollback,
        PolicyRollbackResult::Failed,
    )
    .unwrap();
    let verify =
        plan_policy_rollback_metric(PolicyRollbackOperation::Verify, PolicyRollbackResult::Noop)
            .unwrap();

    assert_eq!(lkg.metric_name, "prodex_policy_rollback_events_total");
    assert_eq!(lkg.increment, 1);
    assert_eq!(
        lkg.operation_label.as_metric_label().unwrap(),
        ("policy_rollback_operation", "activate_last_known_good")
    );
    assert_eq!(
        lkg.result_label.as_metric_label().unwrap(),
        ("policy_rollback_result", "success")
    );
    assert_eq!(
        reject.operation_label.as_metric_label().unwrap(),
        ("policy_rollback_operation", "reject_candidate")
    );
    assert_eq!(
        reject.result_label.as_metric_label().unwrap(),
        ("policy_rollback_result", "blocked")
    );
    assert_eq!(
        rollback.operation_label.as_metric_label().unwrap(),
        ("policy_rollback_operation", "rollback")
    );
    assert_eq!(
        rollback.result_label.as_metric_label().unwrap(),
        ("policy_rollback_result", "failed")
    );
    assert_eq!(
        verify.operation_label.as_metric_label().unwrap(),
        ("policy_rollback_operation", "verify")
    );
    assert_eq!(
        verify.result_label.as_metric_label().unwrap(),
        ("policy_rollback_result", "noop")
    );
    for metric in [lkg, reject, rollback, verify] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("revision"));
        assert!(!metric.operation_label.key.contains("digest"));
        assert!(!metric.result_label.key.contains("policy_body"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn config_activation_metric_uses_closed_source_and_result_labels() {
    let published = plan_config_activation_metric(
        ConfigActivationSource::PublishedRevision,
        ConfigActivationResult::Activated,
    )
    .unwrap();
    let lkg = plan_config_activation_metric(
        ConfigActivationSource::LastKnownGood,
        ConfigActivationResult::MissingLastKnownGood,
    )
    .unwrap();
    let rollback = plan_config_activation_metric(
        ConfigActivationSource::Rollback,
        ConfigActivationResult::Rejected,
    )
    .unwrap();
    let fallback = plan_config_activation_metric(
        ConfigActivationSource::InvalidationFallback,
        ConfigActivationResult::InvalidRevision,
    )
    .unwrap();

    assert_eq!(
        published.metric_name,
        "prodex_config_activation_events_total"
    );
    assert_eq!(published.increment, 1);
    assert_eq!(
        published.source_label.as_metric_label().unwrap(),
        ("config_activation_source", "published_revision")
    );
    assert_eq!(
        published.result_label.as_metric_label().unwrap(),
        ("config_activation_result", "activated")
    );
    assert_eq!(
        lkg.source_label.as_metric_label().unwrap(),
        ("config_activation_source", "last_known_good")
    );
    assert_eq!(
        lkg.result_label.as_metric_label().unwrap(),
        ("config_activation_result", "missing_last_known_good")
    );
    assert_eq!(
        rollback.source_label.as_metric_label().unwrap(),
        ("config_activation_source", "rollback")
    );
    assert_eq!(
        rollback.result_label.as_metric_label().unwrap(),
        ("config_activation_result", "rejected")
    );
    assert_eq!(
        fallback.source_label.as_metric_label().unwrap(),
        ("config_activation_source", "invalidation_fallback")
    );
    assert_eq!(
        fallback.result_label.as_metric_label().unwrap(),
        ("config_activation_result", "invalid_revision")
    );
    for metric in [published, lkg, rollback, fallback] {
        assert!(!metric.source_label.key.contains("tenant"));
        assert!(!metric.source_label.key.contains("revision"));
        assert!(!metric.source_label.key.contains("digest"));
        assert!(!metric.result_label.key.contains("config_body"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn config_publication_delivery_metric_uses_closed_target_and_result_labels() {
    let gateway = plan_config_publication_delivery_metric(
        ConfigPublicationDeliveryTarget::GatewayCacheRefresh,
        ConfigPublicationDeliveryResult::Delivered,
    )
    .unwrap();
    let runtime = plan_config_publication_delivery_metric(
        ConfigPublicationDeliveryTarget::RuntimePolicyReload,
        ConfigPublicationDeliveryResult::RetryScheduled,
    )
    .unwrap();
    let audit = plan_config_publication_delivery_metric(
        ConfigPublicationDeliveryTarget::AuditProjection,
        ConfigPublicationDeliveryResult::Failed,
    )
    .unwrap();
    let skipped = plan_config_publication_delivery_metric(
        ConfigPublicationDeliveryTarget::AuditProjection,
        ConfigPublicationDeliveryResult::Skipped,
    )
    .unwrap();

    assert_eq!(
        gateway.metric_name,
        "prodex_config_publication_delivery_total"
    );
    assert_eq!(gateway.increment, 1);
    assert_eq!(
        gateway.target_label.as_metric_label().unwrap(),
        ("config_publication_target", "gateway_cache_refresh")
    );
    assert_eq!(
        gateway.result_label.as_metric_label().unwrap(),
        ("config_publication_result", "delivered")
    );
    assert_eq!(
        runtime.target_label.as_metric_label().unwrap(),
        ("config_publication_target", "runtime_policy_reload")
    );
    assert_eq!(
        runtime.result_label.as_metric_label().unwrap(),
        ("config_publication_result", "retry_scheduled")
    );
    assert_eq!(
        audit.target_label.as_metric_label().unwrap(),
        ("config_publication_target", "audit_projection")
    );
    assert_eq!(
        audit.result_label.as_metric_label().unwrap(),
        ("config_publication_result", "failed")
    );
    assert_eq!(
        skipped.result_label.as_metric_label().unwrap(),
        ("config_publication_result", "skipped")
    );
    for metric in [gateway, runtime, audit, skipped] {
        assert!(!metric.target_label.key.contains("tenant"));
        assert!(!metric.target_label.key.contains("revision"));
        assert!(!metric.target_label.key.contains("topic"));
        assert!(!metric.result_label.key.contains("payload"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn config_cache_invalidation_metric_uses_closed_target_and_result_labels() {
    let gateway = plan_config_cache_invalidation_metric(
        ConfigCacheInvalidationTarget::GatewayPolicyCache,
        ConfigCacheInvalidationResult::Invalidated,
    )
    .unwrap();
    let runtime = plan_config_cache_invalidation_metric(
        ConfigCacheInvalidationTarget::RuntimePolicyCache,
        ConfigCacheInvalidationResult::ReloadScheduled,
    )
    .unwrap();
    let redis = plan_config_cache_invalidation_metric(
        ConfigCacheInvalidationTarget::RedisPolicyCache,
        ConfigCacheInvalidationResult::NotFound,
    )
    .unwrap();
    let failed = plan_config_cache_invalidation_metric(
        ConfigCacheInvalidationTarget::GatewayPolicyCache,
        ConfigCacheInvalidationResult::Failed,
    )
    .unwrap();

    assert_eq!(
        gateway.metric_name,
        "prodex_config_cache_invalidation_events_total"
    );
    assert_eq!(gateway.increment, 1);
    assert_eq!(
        gateway.target_label.as_metric_label().unwrap(),
        ("config_invalidation_target", "gateway_policy_cache")
    );
    assert_eq!(
        gateway.result_label.as_metric_label().unwrap(),
        ("config_invalidation_result", "invalidated")
    );
    assert_eq!(
        runtime.target_label.as_metric_label().unwrap(),
        ("config_invalidation_target", "runtime_policy_cache")
    );
    assert_eq!(
        runtime.result_label.as_metric_label().unwrap(),
        ("config_invalidation_result", "reload_scheduled")
    );
    assert_eq!(
        redis.target_label.as_metric_label().unwrap(),
        ("config_invalidation_target", "redis_policy_cache")
    );
    assert_eq!(
        redis.result_label.as_metric_label().unwrap(),
        ("config_invalidation_result", "not_found")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("config_invalidation_result", "failed")
    );

    for metric in [gateway, runtime, redis, failed] {
        assert!(!metric.target_label.key.contains("tenant"));
        assert!(!metric.target_label.key.contains("revision"));
        assert!(!metric.target_label.key.contains("root_path"));
        assert!(!metric.target_label.key.contains("cache_key"));
        assert!(!metric.result_label.key.contains("payload"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn dropped_telemetry_metric_uses_closed_low_cardinality_reason_label() {
    let queue_full = plan_dropped_telemetry_metric(TelemetryDropReason::QueueFull).unwrap();
    let exporter_unavailable =
        plan_dropped_telemetry_metric(TelemetryDropReason::ExporterUnavailable).unwrap();
    let shutdown = plan_dropped_telemetry_metric(TelemetryDropReason::Shutdown).unwrap();
    let invalid_payload =
        plan_dropped_telemetry_metric(TelemetryDropReason::InvalidPayload).unwrap();

    assert_eq!(queue_full.metric_name, "prodex_telemetry_dropped_total");
    assert_eq!(queue_full.increment, 1);
    assert_eq!(
        queue_full.reason_label.as_metric_label().unwrap(),
        ("telemetry_drop_reason", "queue_full")
    );
    assert_eq!(
        exporter_unavailable.reason_label.as_metric_label().unwrap(),
        ("telemetry_drop_reason", "exporter_unavailable")
    );
    assert_eq!(
        shutdown.reason_label.as_metric_label().unwrap(),
        ("telemetry_drop_reason", "shutdown")
    );
    assert_eq!(
        invalid_payload.reason_label.as_metric_label().unwrap(),
        ("telemetry_drop_reason", "invalid_payload")
    );
    for metric in [queue_full, exporter_unavailable, shutdown, invalid_payload] {
        assert!(!metric.reason_label.key.contains("tenant"));
        assert!(!metric.reason_label.key.contains("request"));
        assert!(!metric.reason_label.key.contains("error"));
    }
}

#[test]
fn queue_depth_metric_uses_closed_queue_kind_label_and_measurement_values() {
    let responses = plan_queue_depth_metric(QueueDepthKind::Responses, 7, 64).unwrap();
    let compact = plan_queue_depth_metric(QueueDepthKind::Compact, 1, 8).unwrap();
    let websocket = plan_queue_depth_metric(QueueDepthKind::Websocket, 3, 16).unwrap();
    let telemetry = plan_queue_depth_metric(QueueDepthKind::Telemetry, 2, 128).unwrap();
    let persistence = plan_queue_depth_metric(QueueDepthKind::Persistence, 5, 32).unwrap();

    assert_eq!(responses.metric_name, "prodex_queue_depth");
    assert_eq!(responses.depth, 7);
    assert_eq!(responses.capacity, 64);
    assert_eq!(
        responses.queue_label.as_metric_label().unwrap(),
        ("queue_kind", "responses")
    );
    assert_eq!(
        compact.queue_label.as_metric_label().unwrap(),
        ("queue_kind", "compact")
    );
    assert_eq!(
        websocket.queue_label.as_metric_label().unwrap(),
        ("queue_kind", "websocket")
    );
    assert_eq!(
        telemetry.queue_label.as_metric_label().unwrap(),
        ("queue_kind", "telemetry")
    );
    assert_eq!(
        persistence.queue_label.as_metric_label().unwrap(),
        ("queue_kind", "persistence")
    );
    for metric in [responses, compact, websocket, telemetry, persistence] {
        assert!(!metric.queue_label.key.contains("tenant"));
        assert!(!metric.queue_label.key.contains("request"));
        assert!(!metric.queue_label.key.contains("depth"));
        assert!(!metric.queue_label.key.contains("capacity"));
    }
}

#[test]
fn connection_pool_saturation_metric_uses_closed_pool_kind_label() {
    let postgres =
        plan_connection_pool_saturation_metric(ConnectionPoolKind::Postgres, 12, 32).unwrap();
    let redis = plan_connection_pool_saturation_metric(ConnectionPoolKind::Redis, 3, 16).unwrap();
    let provider_http =
        plan_connection_pool_saturation_metric(ConnectionPoolKind::ProviderHttp, 22, 64).unwrap();
    let oidc_http =
        plan_connection_pool_saturation_metric(ConnectionPoolKind::OidcHttp, 2, 8).unwrap();

    assert_eq!(postgres.metric_name, "prodex_connection_pool_in_use");
    assert_eq!(postgres.in_use, 12);
    assert_eq!(postgres.capacity, 32);
    assert_eq!(
        postgres.pool_label.as_metric_label().unwrap(),
        ("pool_kind", "postgres")
    );
    assert_eq!(
        redis.pool_label.as_metric_label().unwrap(),
        ("pool_kind", "redis")
    );
    assert_eq!(
        provider_http.pool_label.as_metric_label().unwrap(),
        ("pool_kind", "provider_http")
    );
    assert_eq!(
        oidc_http.pool_label.as_metric_label().unwrap(),
        ("pool_kind", "oidc_http")
    );
    for metric in [postgres, redis, provider_http, oidc_http] {
        assert!(!metric.pool_label.key.contains("tenant"));
        assert!(!metric.pool_label.key.contains("request"));
        assert!(!metric.pool_label.key.contains("endpoint"));
        assert!(!metric.pool_label.key.contains("capacity"));
    }
}

#[test]
fn api_red_metric_uses_closed_route_and_status_class_labels() {
    let responses =
        plan_api_red_metric(ApiRouteKind::Responses, ApiStatusClass::Success, 42).unwrap();
    let compact =
        plan_api_red_metric(ApiRouteKind::Compact, ApiStatusClass::ClientError, 7).unwrap();
    let websocket =
        plan_api_red_metric(ApiRouteKind::Websocket, ApiStatusClass::ServerError, 99).unwrap();
    let control_plane =
        plan_api_red_metric(ApiRouteKind::ControlPlane, ApiStatusClass::Redirection, 15).unwrap();
    let health =
        plan_api_red_metric(ApiRouteKind::Health, ApiStatusClass::Informational, 2).unwrap();

    assert_eq!(
        responses.request_count_metric_name,
        "prodex_api_requests_total"
    );
    assert_eq!(
        responses.duration_metric_name,
        "prodex_api_request_duration_ms"
    );
    assert_eq!(responses.increment, 1);
    assert_eq!(responses.duration_ms, 42);
    assert_eq!(
        responses.route_label.as_metric_label().unwrap(),
        ("api_route", "responses")
    );
    assert_eq!(
        responses.status_label.as_metric_label().unwrap(),
        ("status_class", "2xx")
    );
    assert_eq!(
        compact.route_label.as_metric_label().unwrap(),
        ("api_route", "compact")
    );
    assert_eq!(
        compact.status_label.as_metric_label().unwrap(),
        ("status_class", "4xx")
    );
    assert_eq!(
        websocket.route_label.as_metric_label().unwrap(),
        ("api_route", "websocket")
    );
    assert_eq!(
        websocket.status_label.as_metric_label().unwrap(),
        ("status_class", "5xx")
    );
    assert_eq!(
        control_plane.route_label.as_metric_label().unwrap(),
        ("api_route", "control_plane")
    );
    assert_eq!(
        control_plane.status_label.as_metric_label().unwrap(),
        ("status_class", "3xx")
    );
    assert_eq!(
        health.route_label.as_metric_label().unwrap(),
        ("api_route", "health")
    );
    assert_eq!(
        health.status_label.as_metric_label().unwrap(),
        ("status_class", "1xx")
    );
    for metric in [responses, compact, websocket, control_plane, health] {
        assert!(!metric.route_label.key.contains("tenant"));
        assert!(!metric.route_label.key.contains("request"));
        assert!(!metric.route_label.key.contains("path"));
        assert!(!metric.status_label.key.contains("status_code"));
    }
}

#[test]
fn api_admission_metric_uses_closed_route_and_result_labels() {
    let responses =
        plan_api_admission_metric(ApiRouteKind::Responses, ApiAdmissionResult::Accepted).unwrap();
    let compact =
        plan_api_admission_metric(ApiRouteKind::Compact, ApiAdmissionResult::QueueFull).unwrap();
    let websocket = plan_api_admission_metric(
        ApiRouteKind::Websocket,
        ApiAdmissionResult::RouteLimitReached,
    )
    .unwrap();
    let control_plane = plan_api_admission_metric(
        ApiRouteKind::ControlPlane,
        ApiAdmissionResult::GlobalLimitReached,
    )
    .unwrap();
    let health =
        plan_api_admission_metric(ApiRouteKind::Health, ApiAdmissionResult::Draining).unwrap();

    assert_eq!(
        responses.metric_name,
        "prodex_api_admission_decisions_total"
    );
    assert_eq!(responses.increment, 1);
    assert_eq!(
        responses.route_label.as_metric_label().unwrap(),
        ("api_admission_route", "responses")
    );
    assert_eq!(
        responses.result_label.as_metric_label().unwrap(),
        ("api_admission_result", "accepted")
    );
    assert_eq!(
        compact.route_label.as_metric_label().unwrap(),
        ("api_admission_route", "compact")
    );
    assert_eq!(
        compact.result_label.as_metric_label().unwrap(),
        ("api_admission_result", "queue_full")
    );
    assert_eq!(
        websocket.route_label.as_metric_label().unwrap(),
        ("api_admission_route", "websocket")
    );
    assert_eq!(
        websocket.result_label.as_metric_label().unwrap(),
        ("api_admission_result", "route_limit_reached")
    );
    assert_eq!(
        control_plane.route_label.as_metric_label().unwrap(),
        ("api_admission_route", "control_plane")
    );
    assert_eq!(
        control_plane.result_label.as_metric_label().unwrap(),
        ("api_admission_result", "global_limit_reached")
    );
    assert_eq!(
        health.route_label.as_metric_label().unwrap(),
        ("api_admission_route", "health")
    );
    assert_eq!(
        health.result_label.as_metric_label().unwrap(),
        ("api_admission_result", "draining")
    );

    for metric in [responses, compact, websocket, control_plane, health] {
        assert!(!metric.route_label.key.contains("tenant"));
        assert!(!metric.route_label.key.contains("request"));
        assert!(!metric.route_label.key.contains("path"));
        assert!(!metric.route_label.key.contains("connection_id"));
        assert!(!metric.result_label.key.contains("capacity"));
        assert!(!metric.result_label.key.contains("queue_depth"));
    }
}

#[test]
fn api_schema_validation_metric_uses_closed_surface_and_result_labels() {
    let request = plan_api_schema_validation_metric(
        ApiSchemaSurface::Request,
        ApiSchemaValidationResult::Valid,
    )
    .unwrap();
    let response = plan_api_schema_validation_metric(
        ApiSchemaSurface::Response,
        ApiSchemaValidationResult::Invalid,
    )
    .unwrap();
    let openapi = plan_api_schema_validation_metric(
        ApiSchemaSurface::OpenApi,
        ApiSchemaValidationResult::MissingSchema,
    )
    .unwrap();
    let error_envelope = plan_api_schema_validation_metric(
        ApiSchemaSurface::ErrorEnvelope,
        ApiSchemaValidationResult::Incompatible,
    )
    .unwrap();

    assert_eq!(request.metric_name, "prodex_api_schema_validation_total");
    assert_eq!(request.increment, 1);
    assert_eq!(
        request.surface_label.as_metric_label().unwrap(),
        ("api_schema_surface", "request")
    );
    assert_eq!(
        request.result_label.as_metric_label().unwrap(),
        ("api_schema_result", "valid")
    );
    assert_eq!(
        response.surface_label.as_metric_label().unwrap(),
        ("api_schema_surface", "response")
    );
    assert_eq!(
        response.result_label.as_metric_label().unwrap(),
        ("api_schema_result", "invalid")
    );
    assert_eq!(
        openapi.surface_label.as_metric_label().unwrap(),
        ("api_schema_surface", "openapi")
    );
    assert_eq!(
        openapi.result_label.as_metric_label().unwrap(),
        ("api_schema_result", "missing_schema")
    );
    assert_eq!(
        error_envelope.surface_label.as_metric_label().unwrap(),
        ("api_schema_surface", "error_envelope")
    );
    assert_eq!(
        error_envelope.result_label.as_metric_label().unwrap(),
        ("api_schema_result", "incompatible")
    );

    for metric in [request, response, openapi, error_envelope] {
        assert!(!metric.surface_label.key.contains("tenant"));
        assert!(!metric.surface_label.key.contains("route"));
        assert!(!metric.surface_label.key.contains("schema_path"));
        assert!(!metric.result_label.key.contains("field"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn api_deprecation_metric_uses_closed_surface_and_signal_labels() {
    let data_plane = plan_api_deprecation_metric(
        ApiDeprecationSurface::DataPlane,
        ApiDeprecationSignal::Notice,
    )
    .unwrap();
    let control_plane = plan_api_deprecation_metric(
        ApiDeprecationSurface::ControlPlane,
        ApiDeprecationSignal::Sunset,
    )
    .unwrap();
    let scim =
        plan_api_deprecation_metric(ApiDeprecationSurface::Scim, ApiDeprecationSignal::Rejected)
            .unwrap();
    let health =
        plan_api_deprecation_metric(ApiDeprecationSurface::Health, ApiDeprecationSignal::Notice)
            .unwrap();

    assert_eq!(
        data_plane.metric_name,
        "prodex_api_deprecation_events_total"
    );
    assert_eq!(data_plane.increment, 1);
    assert_eq!(
        data_plane.surface_label.as_metric_label().unwrap(),
        ("api_deprecation_surface", "data_plane")
    );
    assert_eq!(
        data_plane.signal_label.as_metric_label().unwrap(),
        ("api_deprecation_signal", "notice")
    );
    assert_eq!(
        control_plane.surface_label.as_metric_label().unwrap(),
        ("api_deprecation_surface", "control_plane")
    );
    assert_eq!(
        control_plane.signal_label.as_metric_label().unwrap(),
        ("api_deprecation_signal", "sunset")
    );
    assert_eq!(
        scim.surface_label.as_metric_label().unwrap(),
        ("api_deprecation_surface", "scim")
    );
    assert_eq!(
        scim.signal_label.as_metric_label().unwrap(),
        ("api_deprecation_signal", "rejected")
    );
    assert_eq!(
        health.surface_label.as_metric_label().unwrap(),
        ("api_deprecation_surface", "health")
    );

    for metric in [data_plane, control_plane, scim, health] {
        assert!(!metric.surface_label.key.contains("tenant"));
        assert!(!metric.surface_label.key.contains("route"));
        assert!(!metric.surface_label.key.contains("path"));
        assert!(!metric.signal_label.key.contains("version"));
        assert!(!metric.signal_label.key.contains("client"));
    }
}

#[test]
fn api_pagination_metric_uses_closed_surface_and_result_labels() {
    let control_plane = plan_api_pagination_metric(
        ApiPaginationSurface::ControlPlane,
        ApiPaginationResult::PageReturned,
    )
    .unwrap();
    let scim =
        plan_api_pagination_metric(ApiPaginationSurface::Scim, ApiPaginationResult::EmptyPage)
            .unwrap();
    let audit = plan_api_pagination_metric(
        ApiPaginationSurface::AuditExport,
        ApiPaginationResult::InvalidCursor,
    )
    .unwrap();
    let quota = plan_api_pagination_metric(
        ApiPaginationSurface::Quota,
        ApiPaginationResult::ExpiredCursor,
    )
    .unwrap();

    assert_eq!(
        control_plane.metric_name,
        "prodex_api_pagination_events_total"
    );
    assert_eq!(control_plane.increment, 1);
    assert_eq!(
        control_plane.surface_label.as_metric_label().unwrap(),
        ("api_pagination_surface", "control_plane")
    );
    assert_eq!(
        control_plane.result_label.as_metric_label().unwrap(),
        ("api_pagination_result", "page_returned")
    );
    assert_eq!(
        scim.surface_label.as_metric_label().unwrap(),
        ("api_pagination_surface", "scim")
    );
    assert_eq!(
        scim.result_label.as_metric_label().unwrap(),
        ("api_pagination_result", "empty_page")
    );
    assert_eq!(
        audit.surface_label.as_metric_label().unwrap(),
        ("api_pagination_surface", "audit_export")
    );
    assert_eq!(
        audit.result_label.as_metric_label().unwrap(),
        ("api_pagination_result", "invalid_cursor")
    );
    assert_eq!(
        quota.surface_label.as_metric_label().unwrap(),
        ("api_pagination_surface", "quota")
    );
    assert_eq!(
        quota.result_label.as_metric_label().unwrap(),
        ("api_pagination_result", "expired_cursor")
    );

    for metric in [control_plane, scim, audit, quota] {
        assert!(!metric.surface_label.key.contains("tenant"));
        assert!(!metric.surface_label.key.contains("route"));
        assert!(!metric.surface_label.key.contains("path"));
        assert!(!metric.result_label.key.contains("cursor"));
        assert!(!metric.result_label.key.contains("query"));
    }
}

#[test]
fn api_precondition_metric_uses_closed_surface_and_result_labels() {
    let tenant = plan_api_precondition_metric(
        ApiPreconditionSurface::Tenant,
        ApiPreconditionResult::Matched,
    )
    .unwrap();
    let principal = plan_api_precondition_metric(
        ApiPreconditionSurface::Principal,
        ApiPreconditionResult::Missing,
    )
    .unwrap();
    let virtual_key = plan_api_precondition_metric(
        ApiPreconditionSurface::VirtualKey,
        ApiPreconditionResult::Mismatched,
    )
    .unwrap();
    let policy = plan_api_precondition_metric(
        ApiPreconditionSurface::Policy,
        ApiPreconditionResult::Invalid,
    )
    .unwrap();

    assert_eq!(tenant.metric_name, "prodex_api_precondition_events_total");
    assert_eq!(tenant.increment, 1);
    assert_eq!(
        tenant.surface_label.as_metric_label().unwrap(),
        ("api_precondition_surface", "tenant")
    );
    assert_eq!(
        tenant.result_label.as_metric_label().unwrap(),
        ("api_precondition_result", "matched")
    );
    assert_eq!(
        principal.surface_label.as_metric_label().unwrap(),
        ("api_precondition_surface", "principal")
    );
    assert_eq!(
        principal.result_label.as_metric_label().unwrap(),
        ("api_precondition_result", "missing")
    );
    assert_eq!(
        virtual_key.surface_label.as_metric_label().unwrap(),
        ("api_precondition_surface", "virtual_key")
    );
    assert_eq!(
        virtual_key.result_label.as_metric_label().unwrap(),
        ("api_precondition_result", "mismatched")
    );
    assert_eq!(
        policy.surface_label.as_metric_label().unwrap(),
        ("api_precondition_surface", "policy")
    );
    assert_eq!(
        policy.result_label.as_metric_label().unwrap(),
        ("api_precondition_result", "invalid")
    );

    for metric in [tenant, principal, virtual_key, policy] {
        assert!(!metric.surface_label.key.contains("etag"));
        assert!(!metric.surface_label.key.contains("resource_id"));
        assert!(!metric.result_label.key.contains("version"));
        assert!(!metric.result_label.key.contains("request"));
        assert!(!metric.result_label.key.contains("tenant_id"));
    }
}

#[test]
fn api_idempotency_metric_uses_closed_surface_and_result_labels() {
    let tenant = plan_api_idempotency_metric(
        ApiIdempotencySurface::TenantMutation,
        ApiIdempotencyResult::Accepted,
    )
    .unwrap();
    let principal = plan_api_idempotency_metric(
        ApiIdempotencySurface::PrincipalMutation,
        ApiIdempotencyResult::Replayed,
    )
    .unwrap();
    let virtual_key = plan_api_idempotency_metric(
        ApiIdempotencySurface::VirtualKeyMutation,
        ApiIdempotencyResult::Conflict,
    )
    .unwrap();
    let policy_missing = plan_api_idempotency_metric(
        ApiIdempotencySurface::PolicyMutation,
        ApiIdempotencyResult::Missing,
    )
    .unwrap();
    let policy_invalid = plan_api_idempotency_metric(
        ApiIdempotencySurface::PolicyMutation,
        ApiIdempotencyResult::Invalid,
    )
    .unwrap();

    assert_eq!(tenant.metric_name, "prodex_api_idempotency_events_total");
    assert_eq!(tenant.increment, 1);
    assert_eq!(
        tenant.surface_label.as_metric_label().unwrap(),
        ("api_idempotency_surface", "tenant_mutation")
    );
    assert_eq!(
        tenant.result_label.as_metric_label().unwrap(),
        ("api_idempotency_result", "accepted")
    );
    assert_eq!(
        principal.surface_label.as_metric_label().unwrap(),
        ("api_idempotency_surface", "principal_mutation")
    );
    assert_eq!(
        principal.result_label.as_metric_label().unwrap(),
        ("api_idempotency_result", "replayed")
    );
    assert_eq!(
        virtual_key.surface_label.as_metric_label().unwrap(),
        ("api_idempotency_surface", "virtual_key_mutation")
    );
    assert_eq!(
        virtual_key.result_label.as_metric_label().unwrap(),
        ("api_idempotency_result", "conflict")
    );
    assert_eq!(
        policy_missing.surface_label.as_metric_label().unwrap(),
        ("api_idempotency_surface", "policy_mutation")
    );
    assert_eq!(
        policy_missing.result_label.as_metric_label().unwrap(),
        ("api_idempotency_result", "missing")
    );
    assert_eq!(
        policy_invalid.result_label.as_metric_label().unwrap(),
        ("api_idempotency_result", "invalid")
    );

    for metric in [
        tenant,
        principal,
        virtual_key,
        policy_missing,
        policy_invalid,
    ] {
        assert!(!metric.surface_label.key.contains("idempotency_key"));
        assert!(!metric.surface_label.key.contains("fingerprint"));
        assert!(!metric.result_label.key.contains("tenant_id"));
        assert!(!metric.result_label.key.contains("request"));
        assert!(!metric.result_label.key.contains("resource_id"));
    }
}

#[test]
fn idempotency_record_metric_uses_closed_backend_operation_and_result_labels() {
    let pending = plan_idempotency_record_metric(
        IdempotencyRecordBackend::Postgres,
        IdempotencyRecordOperation::PendingInsert,
        IdempotencyRecordResult::Recorded,
    )
    .unwrap();
    let complete = plan_idempotency_record_metric(
        IdempotencyRecordBackend::Sqlite,
        IdempotencyRecordOperation::Complete,
        IdempotencyRecordResult::Failed,
    )
    .unwrap();
    let lookup_replayed = plan_idempotency_record_metric(
        IdempotencyRecordBackend::Postgres,
        IdempotencyRecordOperation::Lookup,
        IdempotencyRecordResult::Replayed,
    )
    .unwrap();
    let lookup_conflict = plan_idempotency_record_metric(
        IdempotencyRecordBackend::Sqlite,
        IdempotencyRecordOperation::Lookup,
        IdempotencyRecordResult::Conflict,
    )
    .unwrap();
    let lookup_missing = plan_idempotency_record_metric(
        IdempotencyRecordBackend::Postgres,
        IdempotencyRecordOperation::Lookup,
        IdempotencyRecordResult::NotFound,
    )
    .unwrap();

    assert_eq!(
        pending.metric_name,
        "prodex_idempotency_record_events_total"
    );
    assert_eq!(pending.increment, 1);
    assert_eq!(
        pending.backend_label.as_metric_label().unwrap(),
        ("idempotency_record_backend", "postgres")
    );
    assert_eq!(
        pending.operation_label.as_metric_label().unwrap(),
        ("idempotency_record_operation", "pending_insert")
    );
    assert_eq!(
        pending.result_label.as_metric_label().unwrap(),
        ("idempotency_record_result", "recorded")
    );
    assert_eq!(
        complete.backend_label.as_metric_label().unwrap(),
        ("idempotency_record_backend", "sqlite")
    );
    assert_eq!(
        complete.operation_label.as_metric_label().unwrap(),
        ("idempotency_record_operation", "complete")
    );
    assert_eq!(
        complete.result_label.as_metric_label().unwrap(),
        ("idempotency_record_result", "failed")
    );
    assert_eq!(
        lookup_replayed.operation_label.as_metric_label().unwrap(),
        ("idempotency_record_operation", "lookup")
    );
    assert_eq!(
        lookup_replayed.result_label.as_metric_label().unwrap(),
        ("idempotency_record_result", "replayed")
    );
    assert_eq!(
        lookup_conflict.result_label.as_metric_label().unwrap(),
        ("idempotency_record_result", "conflict")
    );
    assert_eq!(
        lookup_missing.result_label.as_metric_label().unwrap(),
        ("idempotency_record_result", "not_found")
    );

    for metric in [
        pending,
        complete,
        lookup_replayed,
        lookup_conflict,
        lookup_missing,
    ] {
        assert!(!metric.backend_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("idempotency_key"));
        assert!(!metric.operation_label.key.contains("fingerprint"));
        assert!(!metric.result_label.key.contains("request"));
        assert!(!metric.result_label.key.contains("response_body"));
    }
}

#[test]
fn api_compatibility_metric_uses_closed_surface_and_result_labels() {
    let data_plane = plan_api_compatibility_metric(
        ApiCompatibilitySurface::DataPlane,
        ApiCompatibilityResult::Compatible,
    )
    .unwrap();
    let control_plane = plan_api_compatibility_metric(
        ApiCompatibilitySurface::ControlPlane,
        ApiCompatibilityResult::AdditiveChange,
    )
    .unwrap();
    let scim = plan_api_compatibility_metric(
        ApiCompatibilitySurface::Scim,
        ApiCompatibilityResult::DeprecatedChange,
    )
    .unwrap();
    let error_envelope = plan_api_compatibility_metric(
        ApiCompatibilitySurface::ErrorEnvelope,
        ApiCompatibilityResult::BreakingChange,
    )
    .unwrap();

    assert_eq!(
        data_plane.metric_name,
        "prodex_api_compatibility_events_total"
    );
    assert_eq!(data_plane.increment, 1);
    assert_eq!(
        data_plane.surface_label.as_metric_label().unwrap(),
        ("api_compatibility_surface", "data_plane")
    );
    assert_eq!(
        data_plane.result_label.as_metric_label().unwrap(),
        ("api_compatibility_result", "compatible")
    );
    assert_eq!(
        control_plane.surface_label.as_metric_label().unwrap(),
        ("api_compatibility_surface", "control_plane")
    );
    assert_eq!(
        control_plane.result_label.as_metric_label().unwrap(),
        ("api_compatibility_result", "additive_change")
    );
    assert_eq!(
        scim.surface_label.as_metric_label().unwrap(),
        ("api_compatibility_surface", "scim")
    );
    assert_eq!(
        scim.result_label.as_metric_label().unwrap(),
        ("api_compatibility_result", "deprecated_change")
    );
    assert_eq!(
        error_envelope.surface_label.as_metric_label().unwrap(),
        ("api_compatibility_surface", "error_envelope")
    );
    assert_eq!(
        error_envelope.result_label.as_metric_label().unwrap(),
        ("api_compatibility_result", "breaking_change")
    );

    for metric in [data_plane, control_plane, scim, error_envelope] {
        assert!(!metric.surface_label.key.contains("tenant"));
        assert!(!metric.surface_label.key.contains("route"));
        assert!(!metric.surface_label.key.contains("schema_hash"));
        assert!(!metric.result_label.key.contains("version"));
        assert!(!metric.result_label.key.contains("request"));
    }
}

#[test]
fn api_mutation_audit_metric_uses_closed_surface_and_result_labels() {
    let tenant = plan_api_mutation_audit_metric(
        ApiMutationAuditSurface::Tenant,
        ApiMutationAuditResult::Required,
    )
    .unwrap();
    let principal = plan_api_mutation_audit_metric(
        ApiMutationAuditSurface::Principal,
        ApiMutationAuditResult::Persisted,
    )
    .unwrap();
    let virtual_key = plan_api_mutation_audit_metric(
        ApiMutationAuditSurface::VirtualKey,
        ApiMutationAuditResult::Missing,
    )
    .unwrap();
    let policy = plan_api_mutation_audit_metric(
        ApiMutationAuditSurface::Policy,
        ApiMutationAuditResult::Failed,
    )
    .unwrap();

    assert_eq!(tenant.metric_name, "prodex_api_mutation_audit_events_total");
    assert_eq!(tenant.increment, 1);
    assert_eq!(
        tenant.surface_label.as_metric_label().unwrap(),
        ("api_mutation_audit_surface", "tenant")
    );
    assert_eq!(
        tenant.result_label.as_metric_label().unwrap(),
        ("api_mutation_audit_result", "required")
    );
    assert_eq!(
        principal.surface_label.as_metric_label().unwrap(),
        ("api_mutation_audit_surface", "principal")
    );
    assert_eq!(
        principal.result_label.as_metric_label().unwrap(),
        ("api_mutation_audit_result", "persisted")
    );
    assert_eq!(
        virtual_key.surface_label.as_metric_label().unwrap(),
        ("api_mutation_audit_surface", "virtual_key")
    );
    assert_eq!(
        virtual_key.result_label.as_metric_label().unwrap(),
        ("api_mutation_audit_result", "missing")
    );
    assert_eq!(
        policy.surface_label.as_metric_label().unwrap(),
        ("api_mutation_audit_surface", "policy")
    );
    assert_eq!(
        policy.result_label.as_metric_label().unwrap(),
        ("api_mutation_audit_result", "failed")
    );

    for metric in [tenant, principal, virtual_key, policy] {
        assert!(!metric.surface_label.key.contains("audit_event_id"));
        assert!(!metric.surface_label.key.contains("resource_id"));
        assert!(!metric.result_label.key.contains("tenant_id"));
        assert!(!metric.result_label.key.contains("request"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn api_version_metric_uses_closed_surface_and_result_labels() {
    let data_plane =
        plan_api_version_metric(ApiVersionSurface::DataPlane, ApiVersionResult::Accepted).unwrap();
    let control_plane =
        plan_api_version_metric(ApiVersionSurface::ControlPlane, ApiVersionResult::Defaulted)
            .unwrap();
    let scim =
        plan_api_version_metric(ApiVersionSurface::Scim, ApiVersionResult::Deprecated).unwrap();
    let health =
        plan_api_version_metric(ApiVersionSurface::Health, ApiVersionResult::Unsupported).unwrap();

    assert_eq!(
        data_plane.metric_name,
        "prodex_api_version_negotiation_events_total"
    );
    assert_eq!(data_plane.increment, 1);
    assert_eq!(
        data_plane.surface_label.as_metric_label().unwrap(),
        ("api_version_surface", "data_plane")
    );
    assert_eq!(
        data_plane.result_label.as_metric_label().unwrap(),
        ("api_version_result", "accepted")
    );
    assert_eq!(
        control_plane.surface_label.as_metric_label().unwrap(),
        ("api_version_surface", "control_plane")
    );
    assert_eq!(
        control_plane.result_label.as_metric_label().unwrap(),
        ("api_version_result", "defaulted")
    );
    assert_eq!(
        scim.surface_label.as_metric_label().unwrap(),
        ("api_version_surface", "scim")
    );
    assert_eq!(
        scim.result_label.as_metric_label().unwrap(),
        ("api_version_result", "deprecated")
    );
    assert_eq!(
        health.surface_label.as_metric_label().unwrap(),
        ("api_version_surface", "health")
    );
    assert_eq!(
        health.result_label.as_metric_label().unwrap(),
        ("api_version_result", "unsupported")
    );

    for metric in [data_plane, control_plane, scim, health] {
        assert!(!metric.surface_label.key.contains("route"));
        assert!(!metric.surface_label.key.contains("raw_version"));
        assert!(!metric.result_label.key.contains("path"));
        assert!(!metric.result_label.key.contains("tenant"));
        assert!(!metric.result_label.key.contains("request"));
    }
}

#[test]
fn api_spec_publication_metric_uses_closed_surface_and_result_labels() {
    let gateway = plan_api_spec_publication_metric(
        ApiSpecSurface::GatewayOpenApi,
        ApiSpecPublicationResult::Generated,
    )
    .unwrap();
    let control_plane = plan_api_spec_publication_metric(
        ApiSpecSurface::ControlPlaneOpenApi,
        ApiSpecPublicationResult::Validated,
    )
    .unwrap();
    let scim = plan_api_spec_publication_metric(
        ApiSpecSurface::ScimSchema,
        ApiSpecPublicationResult::Published,
    )
    .unwrap();
    let error_envelope = plan_api_spec_publication_metric(
        ApiSpecSurface::ErrorEnvelope,
        ApiSpecPublicationResult::Rejected,
    )
    .unwrap();

    assert_eq!(
        gateway.metric_name,
        "prodex_api_spec_publication_events_total"
    );
    assert_eq!(gateway.increment, 1);
    assert_eq!(
        gateway.surface_label.as_metric_label().unwrap(),
        ("api_spec_surface", "gateway_openapi")
    );
    assert_eq!(
        gateway.result_label.as_metric_label().unwrap(),
        ("api_spec_publication_result", "generated")
    );
    assert_eq!(
        control_plane.surface_label.as_metric_label().unwrap(),
        ("api_spec_surface", "control_plane_openapi")
    );
    assert_eq!(
        control_plane.result_label.as_metric_label().unwrap(),
        ("api_spec_publication_result", "validated")
    );
    assert_eq!(
        scim.surface_label.as_metric_label().unwrap(),
        ("api_spec_surface", "scim_schema")
    );
    assert_eq!(
        scim.result_label.as_metric_label().unwrap(),
        ("api_spec_publication_result", "published")
    );
    assert_eq!(
        error_envelope.surface_label.as_metric_label().unwrap(),
        ("api_spec_surface", "error_envelope")
    );
    assert_eq!(
        error_envelope.result_label.as_metric_label().unwrap(),
        ("api_spec_publication_result", "rejected")
    );

    for metric in [gateway, control_plane, scim, error_envelope] {
        assert!(!metric.surface_label.key.contains("schema_hash"));
        assert!(!metric.surface_label.key.contains("path"));
        assert!(!metric.result_label.key.contains("tenant"));
        assert!(!metric.result_label.key.contains("payload"));
        assert!(!metric.result_label.key.contains("request"));
    }
}

#[test]
fn api_error_envelope_metric_uses_closed_surface_and_result_labels() {
    let data_plane = plan_api_error_envelope_metric(
        ApiErrorEnvelopeSurface::DataPlane,
        ApiErrorEnvelopeResult::Emitted,
    )
    .unwrap();
    let control_plane = plan_api_error_envelope_metric(
        ApiErrorEnvelopeSurface::ControlPlane,
        ApiErrorEnvelopeResult::Redacted,
    )
    .unwrap();
    let scim = plan_api_error_envelope_metric(
        ApiErrorEnvelopeSurface::Scim,
        ApiErrorEnvelopeResult::ValidationFailed,
    )
    .unwrap();
    let health = plan_api_error_envelope_metric(
        ApiErrorEnvelopeSurface::Health,
        ApiErrorEnvelopeResult::CompatibilityRejected,
    )
    .unwrap();

    assert_eq!(
        data_plane.metric_name,
        "prodex_api_error_envelope_events_total"
    );
    assert_eq!(data_plane.increment, 1);
    assert_eq!(
        data_plane.surface_label.as_metric_label().unwrap(),
        ("api_error_envelope_surface", "data_plane")
    );
    assert_eq!(
        data_plane.result_label.as_metric_label().unwrap(),
        ("api_error_envelope_result", "emitted")
    );
    assert_eq!(
        control_plane.surface_label.as_metric_label().unwrap(),
        ("api_error_envelope_surface", "control_plane")
    );
    assert_eq!(
        control_plane.result_label.as_metric_label().unwrap(),
        ("api_error_envelope_result", "redacted")
    );
    assert_eq!(
        scim.surface_label.as_metric_label().unwrap(),
        ("api_error_envelope_surface", "scim")
    );
    assert_eq!(
        scim.result_label.as_metric_label().unwrap(),
        ("api_error_envelope_result", "validation_failed")
    );
    assert_eq!(
        health.surface_label.as_metric_label().unwrap(),
        ("api_error_envelope_surface", "health")
    );
    assert_eq!(
        health.result_label.as_metric_label().unwrap(),
        ("api_error_envelope_result", "compatibility_rejected")
    );

    for metric in [data_plane, control_plane, scim, health] {
        assert!(!metric.surface_label.key.contains("tenant"));
        assert!(!metric.surface_label.key.contains("request"));
        assert!(!metric.result_label.key.contains("error_text"));
        assert!(!metric.result_label.key.contains("provider_secret"));
        assert!(!metric.result_label.key.contains("raw_body"));
    }
}

#[test]
fn api_body_limit_metric_uses_closed_surface_and_result_labels() {
    let data_plane =
        plan_api_body_limit_metric(ApiBodyLimitSurface::DataPlane, ApiBodyLimitResult::Accepted)
            .unwrap();
    let control_plane = plan_api_body_limit_metric(
        ApiBodyLimitSurface::ControlPlane,
        ApiBodyLimitResult::RejectedTooLarge,
    )
    .unwrap();
    let scim =
        plan_api_body_limit_metric(ApiBodyLimitSurface::Scim, ApiBodyLimitResult::UnknownLength)
            .unwrap();
    let upload =
        plan_api_body_limit_metric(ApiBodyLimitSurface::Upload, ApiBodyLimitResult::Truncated)
            .unwrap();

    assert_eq!(data_plane.metric_name, "prodex_api_body_limit_events_total");
    assert_eq!(data_plane.increment, 1);
    assert_eq!(
        data_plane.surface_label.as_metric_label().unwrap(),
        ("api_body_limit_surface", "data_plane")
    );
    assert_eq!(
        data_plane.result_label.as_metric_label().unwrap(),
        ("api_body_limit_result", "accepted")
    );
    assert_eq!(
        control_plane.surface_label.as_metric_label().unwrap(),
        ("api_body_limit_surface", "control_plane")
    );
    assert_eq!(
        control_plane.result_label.as_metric_label().unwrap(),
        ("api_body_limit_result", "rejected_too_large")
    );
    assert_eq!(
        scim.surface_label.as_metric_label().unwrap(),
        ("api_body_limit_surface", "scim")
    );
    assert_eq!(
        scim.result_label.as_metric_label().unwrap(),
        ("api_body_limit_result", "unknown_length")
    );
    assert_eq!(
        upload.surface_label.as_metric_label().unwrap(),
        ("api_body_limit_surface", "upload")
    );
    assert_eq!(
        upload.result_label.as_metric_label().unwrap(),
        ("api_body_limit_result", "truncated")
    );

    for metric in [data_plane, control_plane, scim, upload] {
        assert!(!metric.surface_label.key.contains("tenant"));
        assert!(!metric.surface_label.key.contains("route"));
        assert!(!metric.result_label.key.contains("content_length"));
        assert!(!metric.result_label.key.contains("payload"));
        assert!(!metric.result_label.key.contains("request"));
    }
}

#[test]
fn api_timeout_budget_metric_uses_closed_surface_and_result_labels() {
    let data_plane = plan_api_timeout_budget_metric(
        ApiTimeoutBudgetSurface::DataPlane,
        ApiTimeoutBudgetResult::Accepted,
    )
    .unwrap();
    let control_plane = plan_api_timeout_budget_metric(
        ApiTimeoutBudgetSurface::ControlPlane,
        ApiTimeoutBudgetResult::Expired,
    )
    .unwrap();
    let provider = plan_api_timeout_budget_metric(
        ApiTimeoutBudgetSurface::Provider,
        ApiTimeoutBudgetResult::Exhausted,
    )
    .unwrap();
    let persistence = plan_api_timeout_budget_metric(
        ApiTimeoutBudgetSurface::Persistence,
        ApiTimeoutBudgetResult::Cancelled,
    )
    .unwrap();

    assert_eq!(
        data_plane.metric_name,
        "prodex_api_timeout_budget_events_total"
    );
    assert_eq!(data_plane.increment, 1);
    assert_eq!(
        data_plane.surface_label.as_metric_label().unwrap(),
        ("api_timeout_budget_surface", "data_plane")
    );
    assert_eq!(
        data_plane.result_label.as_metric_label().unwrap(),
        ("api_timeout_budget_result", "accepted")
    );
    assert_eq!(
        control_plane.surface_label.as_metric_label().unwrap(),
        ("api_timeout_budget_surface", "control_plane")
    );
    assert_eq!(
        control_plane.result_label.as_metric_label().unwrap(),
        ("api_timeout_budget_result", "expired")
    );
    assert_eq!(
        provider.surface_label.as_metric_label().unwrap(),
        ("api_timeout_budget_surface", "provider")
    );
    assert_eq!(
        provider.result_label.as_metric_label().unwrap(),
        ("api_timeout_budget_result", "exhausted")
    );
    assert_eq!(
        persistence.surface_label.as_metric_label().unwrap(),
        ("api_timeout_budget_surface", "persistence")
    );
    assert_eq!(
        persistence.result_label.as_metric_label().unwrap(),
        ("api_timeout_budget_result", "cancelled")
    );

    for metric in [data_plane, control_plane, provider, persistence] {
        assert!(!metric.surface_label.key.contains("tenant"));
        assert!(!metric.surface_label.key.contains("request"));
        assert!(!metric.result_label.key.contains("route"));
        assert!(!metric.result_label.key.contains("deadline"));
        assert!(!metric.result_label.key.contains("duration_ms"));
    }
}

#[test]
fn api_cancellation_metric_uses_closed_surface_and_source_labels() {
    let data_plane = plan_api_cancellation_metric(
        ApiCancellationSurface::DataPlane,
        ApiCancellationSource::ClientDisconnect,
    )
    .unwrap();
    let control_plane = plan_api_cancellation_metric(
        ApiCancellationSurface::ControlPlane,
        ApiCancellationSource::TimeoutBudget,
    )
    .unwrap();
    let provider = plan_api_cancellation_metric(
        ApiCancellationSurface::ProviderStream,
        ApiCancellationSource::ShutdownDrain,
    )
    .unwrap();
    let persistence = plan_api_cancellation_metric(
        ApiCancellationSurface::Persistence,
        ApiCancellationSource::UpstreamAbort,
    )
    .unwrap();

    assert_eq!(
        data_plane.metric_name,
        "prodex_api_cancellation_events_total"
    );
    assert_eq!(data_plane.increment, 1);
    assert_eq!(
        data_plane.surface_label.as_metric_label().unwrap(),
        ("api_cancellation_surface", "data_plane")
    );
    assert_eq!(
        data_plane.source_label.as_metric_label().unwrap(),
        ("api_cancellation_source", "client_disconnect")
    );
    assert_eq!(
        control_plane.surface_label.as_metric_label().unwrap(),
        ("api_cancellation_surface", "control_plane")
    );
    assert_eq!(
        control_plane.source_label.as_metric_label().unwrap(),
        ("api_cancellation_source", "timeout_budget")
    );
    assert_eq!(
        provider.surface_label.as_metric_label().unwrap(),
        ("api_cancellation_surface", "provider_stream")
    );
    assert_eq!(
        provider.source_label.as_metric_label().unwrap(),
        ("api_cancellation_source", "shutdown_drain")
    );
    assert_eq!(
        persistence.surface_label.as_metric_label().unwrap(),
        ("api_cancellation_surface", "persistence")
    );
    assert_eq!(
        persistence.source_label.as_metric_label().unwrap(),
        ("api_cancellation_source", "upstream_abort")
    );

    for metric in [data_plane, control_plane, provider, persistence] {
        assert!(!metric.surface_label.key.contains("tenant"));
        assert!(!metric.surface_label.key.contains("request"));
        assert!(!metric.source_label.key.contains("route"));
        assert!(!metric.source_label.key.contains("session"));
        assert!(!metric.source_label.key.contains("connection_id"));
    }
}

#[test]
fn api_stream_backpressure_metric_uses_closed_surface_and_state_labels() {
    let data_plane = plan_api_stream_backpressure_metric(
        ApiStreamBackpressureSurface::DataPlaneStream,
        ApiStreamBackpressureState::Ready,
    )
    .unwrap();
    let provider = plan_api_stream_backpressure_metric(
        ApiStreamBackpressureSurface::ProviderStream,
        ApiStreamBackpressureState::Paused,
    )
    .unwrap();
    let websocket = plan_api_stream_backpressure_metric(
        ApiStreamBackpressureSurface::Websocket,
        ApiStreamBackpressureState::Dropped,
    )
    .unwrap();
    let audit = plan_api_stream_backpressure_metric(
        ApiStreamBackpressureSurface::AuditExport,
        ApiStreamBackpressureState::Closed,
    )
    .unwrap();

    assert_eq!(
        data_plane.metric_name,
        "prodex_api_stream_backpressure_events_total"
    );
    assert_eq!(data_plane.increment, 1);
    assert_eq!(
        data_plane.surface_label.as_metric_label().unwrap(),
        ("api_stream_backpressure_surface", "data_plane_stream")
    );
    assert_eq!(
        data_plane.state_label.as_metric_label().unwrap(),
        ("api_stream_backpressure_state", "ready")
    );
    assert_eq!(
        provider.surface_label.as_metric_label().unwrap(),
        ("api_stream_backpressure_surface", "provider_stream")
    );
    assert_eq!(
        provider.state_label.as_metric_label().unwrap(),
        ("api_stream_backpressure_state", "paused")
    );
    assert_eq!(
        websocket.surface_label.as_metric_label().unwrap(),
        ("api_stream_backpressure_surface", "websocket")
    );
    assert_eq!(
        websocket.state_label.as_metric_label().unwrap(),
        ("api_stream_backpressure_state", "dropped")
    );
    assert_eq!(
        audit.surface_label.as_metric_label().unwrap(),
        ("api_stream_backpressure_surface", "audit_export")
    );
    assert_eq!(
        audit.state_label.as_metric_label().unwrap(),
        ("api_stream_backpressure_state", "closed")
    );

    for metric in [data_plane, provider, websocket, audit] {
        assert!(!metric.surface_label.key.contains("tenant"));
        assert!(!metric.surface_label.key.contains("request"));
        assert!(!metric.state_label.key.contains("session"));
        assert!(!metric.state_label.key.contains("stream_id"));
        assert!(!metric.state_label.key.contains("connection_id"));
    }
}

#[test]
fn provider_metric_uses_closed_provider_and_result_labels() {
    let openai =
        plan_provider_metric(ProviderKind::OpenAi, ProviderResultClass::Success, 80).unwrap();
    let anthropic = plan_provider_metric(
        ProviderKind::Anthropic,
        ProviderResultClass::RateLimited,
        120,
    )
    .unwrap();
    let gemini =
        plan_provider_metric(ProviderKind::Gemini, ProviderResultClass::Overloaded, 64).unwrap();
    let local =
        plan_provider_metric(ProviderKind::Local, ProviderResultClass::ProviderError, 9).unwrap();
    let other = plan_provider_metric(
        ProviderKind::Other,
        ProviderResultClass::TransportError,
        250,
    )
    .unwrap();

    assert_eq!(
        openai.request_count_metric_name,
        "prodex_provider_requests_total"
    );
    assert_eq!(
        openai.duration_metric_name,
        "prodex_provider_request_duration_ms"
    );
    assert_eq!(openai.increment, 1);
    assert_eq!(openai.duration_ms, 80);
    assert_eq!(
        openai.provider_label.as_metric_label().unwrap(),
        ("provider", "openai")
    );
    assert_eq!(
        openai.result_label.as_metric_label().unwrap(),
        ("provider_result", "success")
    );
    assert_eq!(
        anthropic.provider_label.as_metric_label().unwrap(),
        ("provider", "anthropic")
    );
    assert_eq!(
        anthropic.result_label.as_metric_label().unwrap(),
        ("provider_result", "rate_limited")
    );
    assert_eq!(
        gemini.provider_label.as_metric_label().unwrap(),
        ("provider", "gemini")
    );
    assert_eq!(
        gemini.result_label.as_metric_label().unwrap(),
        ("provider_result", "overloaded")
    );
    assert_eq!(
        local.provider_label.as_metric_label().unwrap(),
        ("provider", "local")
    );
    assert_eq!(
        local.result_label.as_metric_label().unwrap(),
        ("provider_result", "provider_error")
    );
    assert_eq!(
        other.provider_label.as_metric_label().unwrap(),
        ("provider", "other")
    );
    assert_eq!(
        other.result_label.as_metric_label().unwrap(),
        ("provider_result", "transport_error")
    );
    for metric in [openai, anthropic, gemini, local, other] {
        assert!(!metric.provider_label.key.contains("tenant"));
        assert!(!metric.provider_label.key.contains("model"));
        assert!(!metric.provider_label.key.contains("endpoint"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn provider_capability_negotiation_metric_uses_closed_provider_capability_and_result_labels() {
    let responses = plan_provider_capability_negotiation_metric(
        ProviderKind::OpenAi,
        ProviderCapabilityKind::ResponsesApi,
        ProviderCapabilityResult::Compatible,
    )
    .unwrap();
    let streaming = plan_provider_capability_negotiation_metric(
        ProviderKind::Anthropic,
        ProviderCapabilityKind::Streaming,
        ProviderCapabilityResult::Incompatible,
    )
    .unwrap();
    let tools = plan_provider_capability_negotiation_metric(
        ProviderKind::Gemini,
        ProviderCapabilityKind::Tools,
        ProviderCapabilityResult::NoCandidate,
    )
    .unwrap();
    let vision = plan_provider_capability_negotiation_metric(
        ProviderKind::Local,
        ProviderCapabilityKind::Vision,
        ProviderCapabilityResult::Compatible,
    )
    .unwrap();
    let json = plan_provider_capability_negotiation_metric(
        ProviderKind::Other,
        ProviderCapabilityKind::JsonMode,
        ProviderCapabilityResult::Compatible,
    )
    .unwrap();
    let compact = plan_provider_capability_negotiation_metric(
        ProviderKind::OpenAi,
        ProviderCapabilityKind::RemoteCompact,
        ProviderCapabilityResult::Compatible,
    )
    .unwrap();
    let websocket = plan_provider_capability_negotiation_metric(
        ProviderKind::Anthropic,
        ProviderCapabilityKind::WebSocket,
        ProviderCapabilityResult::Compatible,
    )
    .unwrap();

    assert_eq!(
        responses.metric_name,
        "prodex_provider_capability_negotiation_events_total"
    );
    assert_eq!(responses.increment, 1);
    assert_eq!(
        responses.provider_label.as_metric_label().unwrap(),
        ("provider", "openai")
    );
    assert_eq!(
        responses.capability_label.as_metric_label().unwrap(),
        ("provider_capability", "responses_api")
    );
    assert_eq!(
        responses.result_label.as_metric_label().unwrap(),
        ("provider_capability_result", "compatible")
    );
    assert_eq!(
        streaming.capability_label.as_metric_label().unwrap(),
        ("provider_capability", "streaming")
    );
    assert_eq!(
        streaming.result_label.as_metric_label().unwrap(),
        ("provider_capability_result", "incompatible")
    );
    assert_eq!(
        tools.capability_label.as_metric_label().unwrap(),
        ("provider_capability", "tools")
    );
    assert_eq!(
        tools.result_label.as_metric_label().unwrap(),
        ("provider_capability_result", "no_candidate")
    );
    assert_eq!(
        vision.capability_label.as_metric_label().unwrap(),
        ("provider_capability", "vision")
    );
    assert_eq!(
        json.capability_label.as_metric_label().unwrap(),
        ("provider_capability", "json_mode")
    );
    assert_eq!(
        compact.capability_label.as_metric_label().unwrap(),
        ("provider_capability", "remote_compact")
    );
    assert_eq!(
        websocket.capability_label.as_metric_label().unwrap(),
        ("provider_capability", "websocket")
    );

    for metric in [
        responses, streaming, tools, vision, json, compact, websocket,
    ] {
        assert!(!metric.provider_label.key.contains("tenant"));
        assert!(!metric.provider_label.key.contains("model"));
        assert!(!metric.capability_label.key.contains("endpoint"));
        assert!(!metric.capability_label.key.contains("request"));
        assert!(!metric.result_label.key.contains("model"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn provider_retry_metric_uses_closed_provider_stage_and_outcome_labels() {
    let before_dispatch = plan_provider_retry_metric(
        ProviderKind::OpenAi,
        ProviderRetryAttemptStage::BeforeDispatch,
        ProviderRetryOutcome::Allowed,
    )
    .unwrap();
    let before_first_byte = plan_provider_retry_metric(
        ProviderKind::Anthropic,
        ProviderRetryAttemptStage::BeforeFirstByte,
        ProviderRetryOutcome::Allowed,
    )
    .unwrap();
    let after_first_byte = plan_provider_retry_metric(
        ProviderKind::Gemini,
        ProviderRetryAttemptStage::AfterFirstByte,
        ProviderRetryOutcome::DeniedCommitted,
    )
    .unwrap();
    let after_cancellation = plan_provider_retry_metric(
        ProviderKind::Other,
        ProviderRetryAttemptStage::AfterCancellation,
        ProviderRetryOutcome::DeniedBudgetExhausted,
    )
    .unwrap();

    assert_eq!(
        before_dispatch.metric_name,
        "prodex_provider_retry_events_total"
    );
    assert_eq!(before_dispatch.increment, 1);
    assert_eq!(
        before_dispatch.provider_label.as_metric_label().unwrap(),
        ("provider", "openai")
    );
    assert_eq!(
        before_dispatch.stage_label.as_metric_label().unwrap(),
        ("provider_retry_stage", "before_dispatch")
    );
    assert_eq!(
        before_dispatch.outcome_label.as_metric_label().unwrap(),
        ("provider_retry_outcome", "allowed")
    );
    assert_eq!(
        before_first_byte.stage_label.as_metric_label().unwrap(),
        ("provider_retry_stage", "before_first_byte")
    );
    assert_eq!(
        before_first_byte.outcome_label.as_metric_label().unwrap(),
        ("provider_retry_outcome", "allowed")
    );
    assert_eq!(
        after_first_byte.stage_label.as_metric_label().unwrap(),
        ("provider_retry_stage", "after_first_byte")
    );
    assert_eq!(
        after_first_byte.outcome_label.as_metric_label().unwrap(),
        ("provider_retry_outcome", "denied_committed")
    );
    assert_eq!(
        after_cancellation.stage_label.as_metric_label().unwrap(),
        ("provider_retry_stage", "after_cancellation")
    );
    assert_eq!(
        after_cancellation.outcome_label.as_metric_label().unwrap(),
        ("provider_retry_outcome", "denied_budget_exhausted")
    );

    for metric in [
        before_dispatch,
        before_first_byte,
        after_first_byte,
        after_cancellation,
    ] {
        assert!(!metric.provider_label.key.contains("tenant"));
        assert!(!metric.provider_label.key.contains("model"));
        assert!(!metric.stage_label.key.contains("request"));
        assert!(!metric.stage_label.key.contains("endpoint"));
        assert!(!metric.outcome_label.key.contains("error_text"));
        assert!(!metric.outcome_label.key.contains("retry_after"));
    }
}

#[test]
fn provider_circuit_breaker_metric_uses_closed_provider_decision_and_event_labels() {
    let closed = plan_provider_circuit_breaker_metric(
        ProviderKind::OpenAi,
        ProviderCircuitBreakerDecision::Closed,
        ProviderCircuitBreakerEvent::Success,
    )
    .unwrap();
    let open = plan_provider_circuit_breaker_metric(
        ProviderKind::Anthropic,
        ProviderCircuitBreakerDecision::Open,
        ProviderCircuitBreakerEvent::Failure,
    )
    .unwrap();
    let half_open = plan_provider_circuit_breaker_metric(
        ProviderKind::Gemini,
        ProviderCircuitBreakerDecision::HalfOpenProbe,
        ProviderCircuitBreakerEvent::Probe,
    )
    .unwrap();

    assert_eq!(
        closed.metric_name,
        "prodex_provider_circuit_breaker_events_total"
    );
    assert_eq!(closed.increment, 1);
    assert_eq!(
        closed.provider_label.as_metric_label().unwrap(),
        ("provider", "openai")
    );
    assert_eq!(
        closed.decision_label.as_metric_label().unwrap(),
        ("provider_circuit_breaker_decision", "closed")
    );
    assert_eq!(
        closed.event_label.as_metric_label().unwrap(),
        ("provider_circuit_breaker_event", "success")
    );
    assert_eq!(
        open.provider_label.as_metric_label().unwrap(),
        ("provider", "anthropic")
    );
    assert_eq!(
        open.decision_label.as_metric_label().unwrap(),
        ("provider_circuit_breaker_decision", "open")
    );
    assert_eq!(
        open.event_label.as_metric_label().unwrap(),
        ("provider_circuit_breaker_event", "failure")
    );
    assert_eq!(
        half_open.provider_label.as_metric_label().unwrap(),
        ("provider", "gemini")
    );
    assert_eq!(
        half_open.decision_label.as_metric_label().unwrap(),
        ("provider_circuit_breaker_decision", "half_open_probe")
    );
    assert_eq!(
        half_open.event_label.as_metric_label().unwrap(),
        ("provider_circuit_breaker_event", "probe")
    );

    for metric in [closed, open, half_open] {
        assert!(!metric.provider_label.key.contains("tenant"));
        assert!(!metric.provider_label.key.contains("model"));
        assert!(!metric.decision_label.key.contains("endpoint"));
        assert!(!metric.decision_label.key.contains("retry_after"));
        assert!(!metric.event_label.key.contains("request"));
        assert!(!metric.event_label.key.contains("error_text"));
    }
}

#[test]
fn provider_degradation_metric_uses_closed_provider_signal_and_severity_labels() {
    let error_rate = plan_provider_degradation_metric(
        ProviderKind::OpenAi,
        ProviderDegradationSignal::ErrorRate,
        ProviderDegradationSeverity::Warning,
    )
    .unwrap();
    let latency = plan_provider_degradation_metric(
        ProviderKind::Anthropic,
        ProviderDegradationSignal::Latency,
        ProviderDegradationSeverity::Critical,
    )
    .unwrap();
    let overload = plan_provider_degradation_metric(
        ProviderKind::Gemini,
        ProviderDegradationSignal::Overload,
        ProviderDegradationSeverity::Recovered,
    )
    .unwrap();
    let transport = plan_provider_degradation_metric(
        ProviderKind::Local,
        ProviderDegradationSignal::Transport,
        ProviderDegradationSeverity::Critical,
    )
    .unwrap();
    let circuit = plan_provider_degradation_metric(
        ProviderKind::Other,
        ProviderDegradationSignal::CircuitOpen,
        ProviderDegradationSeverity::Warning,
    )
    .unwrap();

    assert_eq!(
        error_rate.metric_name,
        "prodex_provider_degradation_events_total"
    );
    assert_eq!(error_rate.increment, 1);
    assert_eq!(
        error_rate.provider_label.as_metric_label().unwrap(),
        ("provider", "openai")
    );
    assert_eq!(
        error_rate.signal_label.as_metric_label().unwrap(),
        ("provider_degradation_signal", "error_rate")
    );
    assert_eq!(
        error_rate.severity_label.as_metric_label().unwrap(),
        ("provider_degradation_severity", "warning")
    );
    assert_eq!(
        latency.provider_label.as_metric_label().unwrap(),
        ("provider", "anthropic")
    );
    assert_eq!(
        latency.signal_label.as_metric_label().unwrap(),
        ("provider_degradation_signal", "latency")
    );
    assert_eq!(
        latency.severity_label.as_metric_label().unwrap(),
        ("provider_degradation_severity", "critical")
    );
    assert_eq!(
        overload.provider_label.as_metric_label().unwrap(),
        ("provider", "gemini")
    );
    assert_eq!(
        overload.signal_label.as_metric_label().unwrap(),
        ("provider_degradation_signal", "overload")
    );
    assert_eq!(
        overload.severity_label.as_metric_label().unwrap(),
        ("provider_degradation_severity", "recovered")
    );
    assert_eq!(
        transport.signal_label.as_metric_label().unwrap(),
        ("provider_degradation_signal", "transport")
    );
    assert_eq!(
        circuit.signal_label.as_metric_label().unwrap(),
        ("provider_degradation_signal", "circuit_open")
    );
    for metric in [error_rate, latency, overload, transport, circuit] {
        assert!(!metric.provider_label.key.contains("tenant"));
        assert!(!metric.provider_label.key.contains("model"));
        assert!(!metric.signal_label.key.contains("endpoint"));
        assert!(!metric.signal_label.key.contains("request"));
        assert!(!metric.severity_label.key.contains("error_text"));
    }
}

#[test]
fn streaming_lifecycle_metric_uses_closed_transport_and_outcome_labels() {
    let completed = plan_streaming_lifecycle_metric(
        StreamTransportKind::Responses,
        StreamOutcome::Completed,
        1_500,
    )
    .unwrap();
    let cancelled = plan_streaming_lifecycle_metric(
        StreamTransportKind::Responses,
        StreamOutcome::Cancelled,
        300,
    )
    .unwrap();
    let interrupted = plan_streaming_lifecycle_metric(
        StreamTransportKind::Websocket,
        StreamOutcome::Interrupted,
        700,
    )
    .unwrap();
    let blocked = plan_streaming_lifecycle_metric(
        StreamTransportKind::Websocket,
        StreamOutcome::GuardrailBlocked,
        120,
    )
    .unwrap();

    assert_eq!(
        completed.event_count_metric_name,
        "prodex_streaming_lifecycle_total"
    );
    assert_eq!(
        completed.duration_metric_name,
        "prodex_streaming_lifecycle_duration_ms"
    );
    assert_eq!(completed.increment, 1);
    assert_eq!(completed.duration_ms, 1_500);
    assert_eq!(
        completed.transport_label.as_metric_label().unwrap(),
        ("stream_transport", "responses")
    );
    assert_eq!(
        completed.outcome_label.as_metric_label().unwrap(),
        ("stream_outcome", "completed")
    );
    assert_eq!(
        cancelled.outcome_label.as_metric_label().unwrap(),
        ("stream_outcome", "cancelled")
    );
    assert_eq!(
        interrupted.transport_label.as_metric_label().unwrap(),
        ("stream_transport", "websocket")
    );
    assert_eq!(
        interrupted.outcome_label.as_metric_label().unwrap(),
        ("stream_outcome", "interrupted")
    );
    assert_eq!(
        blocked.outcome_label.as_metric_label().unwrap(),
        ("stream_outcome", "guardrail_blocked")
    );
    for metric in [completed, cancelled, interrupted, blocked] {
        assert!(!metric.transport_label.key.contains("tenant"));
        assert!(!metric.transport_label.key.contains("request"));
        assert!(!metric.outcome_label.key.contains("provider"));
        assert!(!metric.outcome_label.key.contains("error_text"));
    }
}

#[test]
fn routing_decision_metric_uses_closed_lane_and_outcome_labels() {
    let selected =
        plan_routing_decision_metric(RoutingLaneKind::Responses, RoutingDecisionOutcome::Selected)
            .unwrap();
    let fallback =
        plan_routing_decision_metric(RoutingLaneKind::Compact, RoutingDecisionOutcome::Fallback)
            .unwrap();
    let rejected =
        plan_routing_decision_metric(RoutingLaneKind::Websocket, RoutingDecisionOutcome::Rejected)
            .unwrap();
    let no_candidate = plan_routing_decision_metric(
        RoutingLaneKind::ControlPlane,
        RoutingDecisionOutcome::NoCandidate,
    )
    .unwrap();

    assert_eq!(selected.metric_name, "prodex_routing_decisions_total");
    assert_eq!(selected.increment, 1);
    assert_eq!(
        selected.lane_label.as_metric_label().unwrap(),
        ("routing_lane", "responses")
    );
    assert_eq!(
        selected.outcome_label.as_metric_label().unwrap(),
        ("routing_outcome", "selected")
    );
    assert_eq!(
        fallback.lane_label.as_metric_label().unwrap(),
        ("routing_lane", "compact")
    );
    assert_eq!(
        fallback.outcome_label.as_metric_label().unwrap(),
        ("routing_outcome", "fallback")
    );
    assert_eq!(
        rejected.lane_label.as_metric_label().unwrap(),
        ("routing_lane", "websocket")
    );
    assert_eq!(
        rejected.outcome_label.as_metric_label().unwrap(),
        ("routing_outcome", "rejected")
    );
    assert_eq!(
        no_candidate.lane_label.as_metric_label().unwrap(),
        ("routing_lane", "control_plane")
    );
    assert_eq!(
        no_candidate.outcome_label.as_metric_label().unwrap(),
        ("routing_outcome", "no_candidate")
    );
    for metric in [selected, fallback, rejected, no_candidate] {
        assert!(!metric.lane_label.key.contains("tenant"));
        assert!(!metric.lane_label.key.contains("profile"));
        assert!(!metric.outcome_label.key.contains("request"));
        assert!(!metric.outcome_label.key.contains("error_text"));
        assert!(!metric.outcome_label.key.contains("model"));
    }
}

#[test]
fn audit_metric_uses_closed_operation_and_result_labels() {
    let emitted = plan_audit_metric(AuditOperation::Emit, AuditResult::Success).unwrap();
    let persisted = plan_audit_metric(AuditOperation::Persist, AuditResult::Failure).unwrap();
    let exported = plan_audit_metric(AuditOperation::Export, AuditResult::Dropped).unwrap();

    assert_eq!(emitted.metric_name, "prodex_audit_events_total");
    assert_eq!(emitted.increment, 1);
    assert_eq!(
        emitted.operation_label.as_metric_label().unwrap(),
        ("audit_operation", "emit")
    );
    assert_eq!(
        emitted.result_label.as_metric_label().unwrap(),
        ("audit_result", "success")
    );
    assert_eq!(
        persisted.operation_label.as_metric_label().unwrap(),
        ("audit_operation", "persist")
    );
    assert_eq!(
        persisted.result_label.as_metric_label().unwrap(),
        ("audit_result", "failure")
    );
    assert_eq!(
        exported.operation_label.as_metric_label().unwrap(),
        ("audit_operation", "export")
    );
    assert_eq!(
        exported.result_label.as_metric_label().unwrap(),
        ("audit_result", "dropped")
    );
    for metric in [emitted, persisted, exported] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("audit_event_id"));
        assert!(!metric.result_label.key.contains("audit_event_id"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn audit_query_lifecycle_metric_uses_closed_operation_and_result_labels() {
    let query = plan_audit_query_lifecycle_metric(
        AuditQueryLifecycleOperation::PlanQuery,
        AuditQueryLifecycleResult::Planned,
    )
    .unwrap();
    let page = plan_audit_query_lifecycle_metric(
        AuditQueryLifecycleOperation::PageQuery,
        AuditQueryLifecycleResult::PageReturned,
    )
    .unwrap();
    let empty = plan_audit_query_lifecycle_metric(
        AuditQueryLifecycleOperation::PageQuery,
        AuditQueryLifecycleResult::Empty,
    )
    .unwrap();
    let export = plan_audit_query_lifecycle_metric(
        AuditQueryLifecycleOperation::PlanExport,
        AuditQueryLifecycleResult::Denied,
    )
    .unwrap();
    let serialize = plan_audit_query_lifecycle_metric(
        AuditQueryLifecycleOperation::SerializeExport,
        AuditQueryLifecycleResult::Failed,
    )
    .unwrap();

    assert_eq!(
        query.metric_name,
        "prodex_audit_query_lifecycle_events_total"
    );
    assert_eq!(query.increment, 1);
    assert_eq!(
        query.operation_label.as_metric_label().unwrap(),
        ("audit_query_operation", "plan_query")
    );
    assert_eq!(
        query.result_label.as_metric_label().unwrap(),
        ("audit_query_result", "planned")
    );
    assert_eq!(
        page.operation_label.as_metric_label().unwrap(),
        ("audit_query_operation", "page_query")
    );
    assert_eq!(
        page.result_label.as_metric_label().unwrap(),
        ("audit_query_result", "page_returned")
    );
    assert_eq!(
        empty.result_label.as_metric_label().unwrap(),
        ("audit_query_result", "empty")
    );
    assert_eq!(
        export.operation_label.as_metric_label().unwrap(),
        ("audit_query_operation", "plan_export")
    );
    assert_eq!(
        export.result_label.as_metric_label().unwrap(),
        ("audit_query_result", "denied")
    );
    assert_eq!(
        serialize.operation_label.as_metric_label().unwrap(),
        ("audit_query_operation", "serialize_export")
    );
    assert_eq!(
        serialize.result_label.as_metric_label().unwrap(),
        ("audit_query_result", "failed")
    );

    for metric in [query, page, empty, export, serialize] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("principal"));
        assert!(!metric.operation_label.key.contains("audit_event_id"));
        assert!(!metric.operation_label.key.contains("cursor"));
        assert!(!metric.result_label.key.contains("time_range"));
        assert!(!metric.result_label.key.contains("format"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn audit_chain_metric_uses_closed_operation_and_result_labels() {
    let append =
        plan_audit_chain_metric(AuditChainOperation::Append, AuditChainResult::Success).unwrap();
    let link = plan_audit_chain_metric(AuditChainOperation::VerifyLink, AuditChainResult::Conflict)
        .unwrap();
    let range = plan_audit_chain_metric(
        AuditChainOperation::VerifyRange,
        AuditChainResult::GapDetected,
    )
    .unwrap();
    let proof = plan_audit_chain_metric(
        AuditChainOperation::ExportProof,
        AuditChainResult::DigestInvalid,
    )
    .unwrap();
    let failed =
        plan_audit_chain_metric(AuditChainOperation::VerifyRange, AuditChainResult::Failed)
            .unwrap();

    assert_eq!(append.metric_name, "prodex_audit_chain_events_total");
    assert_eq!(append.increment, 1);
    assert_eq!(
        append.operation_label.as_metric_label().unwrap(),
        ("audit_chain_operation", "append")
    );
    assert_eq!(
        append.result_label.as_metric_label().unwrap(),
        ("audit_chain_result", "success")
    );
    assert_eq!(
        link.operation_label.as_metric_label().unwrap(),
        ("audit_chain_operation", "verify_link")
    );
    assert_eq!(
        link.result_label.as_metric_label().unwrap(),
        ("audit_chain_result", "conflict")
    );
    assert_eq!(
        range.operation_label.as_metric_label().unwrap(),
        ("audit_chain_operation", "verify_range")
    );
    assert_eq!(
        range.result_label.as_metric_label().unwrap(),
        ("audit_chain_result", "gap_detected")
    );
    assert_eq!(
        proof.operation_label.as_metric_label().unwrap(),
        ("audit_chain_operation", "export_proof")
    );
    assert_eq!(
        proof.result_label.as_metric_label().unwrap(),
        ("audit_chain_result", "digest_invalid")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("audit_chain_result", "failed")
    );
    for metric in [append, link, range, proof, failed] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("audit_event_id"));
        assert!(!metric.operation_label.key.contains("digest"));
        assert!(!metric.operation_label.key.contains("principal"));
        assert!(!metric.result_label.key.contains("resource_id"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn audit_retention_purge_metric_uses_closed_operation_and_result_labels() {
    let selected = plan_audit_retention_purge_metric(
        AuditRetentionPurgeOperation::SelectCandidates,
        AuditRetentionPurgeResult::Success,
    )
    .unwrap();
    let legal_hold = plan_audit_retention_purge_metric(
        AuditRetentionPurgeOperation::ApplyLegalHold,
        AuditRetentionPurgeResult::Protected,
    )
    .unwrap();
    let deleted = plan_audit_retention_purge_metric(
        AuditRetentionPurgeOperation::DeleteBatch,
        AuditRetentionPurgeResult::Failed,
    )
    .unwrap();
    let verified = plan_audit_retention_purge_metric(
        AuditRetentionPurgeOperation::VerifyChain,
        AuditRetentionPurgeResult::Empty,
    )
    .unwrap();

    assert_eq!(
        selected.metric_name,
        "prodex_audit_retention_purge_events_total"
    );
    assert_eq!(selected.increment, 1);
    assert_eq!(
        selected.operation_label.as_metric_label().unwrap(),
        ("audit_retention_operation", "select_candidates")
    );
    assert_eq!(
        selected.result_label.as_metric_label().unwrap(),
        ("audit_retention_result", "success")
    );
    assert_eq!(
        legal_hold.operation_label.as_metric_label().unwrap(),
        ("audit_retention_operation", "apply_legal_hold")
    );
    assert_eq!(
        legal_hold.result_label.as_metric_label().unwrap(),
        ("audit_retention_result", "protected")
    );
    assert_eq!(
        deleted.operation_label.as_metric_label().unwrap(),
        ("audit_retention_operation", "delete_batch")
    );
    assert_eq!(
        deleted.result_label.as_metric_label().unwrap(),
        ("audit_retention_result", "failed")
    );
    assert_eq!(
        verified.operation_label.as_metric_label().unwrap(),
        ("audit_retention_operation", "verify_chain")
    );
    assert_eq!(
        verified.result_label.as_metric_label().unwrap(),
        ("audit_retention_result", "empty")
    );
    for metric in [selected, legal_hold, deleted, verified] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("audit_event_id"));
        assert!(!metric.operation_label.key.contains("batch_id"));
        assert!(!metric.result_label.key.contains("storage_error"));
        assert!(!metric.result_label.key.contains("cursor"));
    }
}

#[test]
fn security_decision_metric_uses_closed_decision_and_result_labels() {
    let authentication = plan_security_decision_metric(
        SecurityDecisionKind::Authentication,
        SecurityDecisionResult::Allowed,
    )
    .unwrap();
    let tenant_resolution = plan_security_decision_metric(
        SecurityDecisionKind::TenantResolution,
        SecurityDecisionResult::Denied,
    )
    .unwrap();
    let authorization = plan_security_decision_metric(
        SecurityDecisionKind::Authorization,
        SecurityDecisionResult::Error,
    )
    .unwrap();
    let credential_scope = plan_security_decision_metric(
        SecurityDecisionKind::CredentialScope,
        SecurityDecisionResult::Denied,
    )
    .unwrap();

    assert_eq!(
        authentication.metric_name,
        "prodex_security_decisions_total"
    );
    assert_eq!(authentication.increment, 1);
    assert_eq!(
        authentication.decision_label.as_metric_label().unwrap(),
        ("security_decision", "authentication")
    );
    assert_eq!(
        authentication.result_label.as_metric_label().unwrap(),
        ("security_result", "allowed")
    );
    assert_eq!(
        tenant_resolution.decision_label.as_metric_label().unwrap(),
        ("security_decision", "tenant_resolution")
    );
    assert_eq!(
        tenant_resolution.result_label.as_metric_label().unwrap(),
        ("security_result", "denied")
    );
    assert_eq!(
        authorization.decision_label.as_metric_label().unwrap(),
        ("security_decision", "authorization")
    );
    assert_eq!(
        authorization.result_label.as_metric_label().unwrap(),
        ("security_result", "error")
    );
    assert_eq!(
        credential_scope.decision_label.as_metric_label().unwrap(),
        ("security_decision", "credential_scope")
    );
    for metric in [
        authentication,
        tenant_resolution,
        authorization,
        credential_scope,
    ] {
        assert!(!metric.decision_label.key.contains("tenant"));
        assert!(!metric.decision_label.key.contains("principal"));
        assert!(!metric.result_label.key.contains("role"));
        assert!(!metric.result_label.key.contains("claim"));
        assert!(!metric.result_label.key.contains("token"));
    }
}

#[test]
fn authn_token_validation_metric_uses_closed_stage_and_result_labels() {
    let accepted = plan_authn_token_validation_metric(
        AuthnTokenValidationStage::Decode,
        AuthnTokenValidationResult::Accepted,
    )
    .unwrap();
    let malformed = plan_authn_token_validation_metric(
        AuthnTokenValidationStage::Decode,
        AuthnTokenValidationResult::Malformed,
    )
    .unwrap();
    let invalid_signature = plan_authn_token_validation_metric(
        AuthnTokenValidationStage::Signature,
        AuthnTokenValidationResult::InvalidSignature,
    )
    .unwrap();
    let expired = plan_authn_token_validation_metric(
        AuthnTokenValidationStage::Claims,
        AuthnTokenValidationResult::Expired,
    )
    .unwrap();
    let unknown_key = plan_authn_token_validation_metric(
        AuthnTokenValidationStage::JwksCache,
        AuthnTokenValidationResult::UnknownKey,
    )
    .unwrap();
    let missing_tenant = plan_authn_token_validation_metric(
        AuthnTokenValidationStage::TenantClaim,
        AuthnTokenValidationResult::MissingTenant,
    )
    .unwrap();
    let role_denied = plan_authn_token_validation_metric(
        AuthnTokenValidationStage::RoleClaim,
        AuthnTokenValidationResult::RoleDenied,
    )
    .unwrap();
    let cache_unavailable = plan_authn_token_validation_metric(
        AuthnTokenValidationStage::JwksCache,
        AuthnTokenValidationResult::CacheUnavailable,
    )
    .unwrap();

    assert_eq!(
        accepted.metric_name,
        "prodex_authn_token_validation_events_total"
    );
    assert_eq!(accepted.increment, 1);
    assert_eq!(
        accepted.stage_label.as_metric_label().unwrap(),
        ("authn_validation_stage", "decode")
    );
    assert_eq!(
        accepted.result_label.as_metric_label().unwrap(),
        ("authn_validation_result", "accepted")
    );
    assert_eq!(
        malformed.result_label.as_metric_label().unwrap(),
        ("authn_validation_result", "malformed")
    );
    assert_eq!(
        invalid_signature.stage_label.as_metric_label().unwrap(),
        ("authn_validation_stage", "signature")
    );
    assert_eq!(
        invalid_signature.result_label.as_metric_label().unwrap(),
        ("authn_validation_result", "invalid_signature")
    );
    assert_eq!(
        expired.stage_label.as_metric_label().unwrap(),
        ("authn_validation_stage", "claims")
    );
    assert_eq!(
        expired.result_label.as_metric_label().unwrap(),
        ("authn_validation_result", "expired")
    );
    assert_eq!(
        unknown_key.stage_label.as_metric_label().unwrap(),
        ("authn_validation_stage", "jwks_cache")
    );
    assert_eq!(
        unknown_key.result_label.as_metric_label().unwrap(),
        ("authn_validation_result", "unknown_key")
    );
    assert_eq!(
        missing_tenant.stage_label.as_metric_label().unwrap(),
        ("authn_validation_stage", "tenant_claim")
    );
    assert_eq!(
        missing_tenant.result_label.as_metric_label().unwrap(),
        ("authn_validation_result", "missing_tenant")
    );
    assert_eq!(
        role_denied.stage_label.as_metric_label().unwrap(),
        ("authn_validation_stage", "role_claim")
    );
    assert_eq!(
        role_denied.result_label.as_metric_label().unwrap(),
        ("authn_validation_result", "role_denied")
    );
    assert_eq!(
        cache_unavailable.result_label.as_metric_label().unwrap(),
        ("authn_validation_result", "cache_unavailable")
    );
    for metric in [
        accepted,
        malformed,
        invalid_signature,
        expired,
        unknown_key,
        missing_tenant,
        role_denied,
        cache_unavailable,
    ] {
        assert!(!metric.stage_label.key.contains("tenant_id"));
        assert!(!metric.stage_label.key.contains("principal_id"));
        assert!(!metric.stage_label.key.contains("issuer"));
        assert!(!metric.stage_label.key.contains("kid"));
        assert!(!metric.result_label.key.contains("token"));
        assert!(!metric.result_label.key.contains("raw_claim"));
        assert!(!metric.result_label.key.contains("jwks_key"));
    }
}

#[test]
fn authz_decision_metric_uses_closed_boundary_and_result_labels() {
    let inference = plan_authz_decision_metric(
        AuthzBoundaryKind::DataPlaneInference,
        AuthzDecisionResult::Allowed,
    )
    .unwrap();
    let quota = plan_authz_decision_metric(
        AuthzBoundaryKind::DataPlaneQuota,
        AuthzDecisionResult::CredentialScopeDenied,
    )
    .unwrap();
    let read = plan_authz_decision_metric(
        AuthzBoundaryKind::ControlPlaneRead,
        AuthzDecisionResult::RoleDenied,
    )
    .unwrap();
    let mutation = plan_authz_decision_metric(
        AuthzBoundaryKind::ControlPlaneMutation,
        AuthzDecisionResult::TenantDenied,
    )
    .unwrap();
    let billing = plan_authz_decision_metric(
        AuthzBoundaryKind::ControlPlaneBilling,
        AuthzDecisionResult::ResourceDenied,
    )
    .unwrap();
    let break_glass =
        plan_authz_decision_metric(AuthzBoundaryKind::BreakGlass, AuthzDecisionResult::Failed)
            .unwrap();

    assert_eq!(inference.metric_name, "prodex_authz_decisions_total");
    assert_eq!(inference.increment, 1);
    assert_eq!(
        inference.boundary_label.as_metric_label().unwrap(),
        ("authz_boundary", "data_plane_inference")
    );
    assert_eq!(
        inference.result_label.as_metric_label().unwrap(),
        ("authz_result", "allowed")
    );
    assert_eq!(
        quota.boundary_label.as_metric_label().unwrap(),
        ("authz_boundary", "data_plane_quota")
    );
    assert_eq!(
        quota.result_label.as_metric_label().unwrap(),
        ("authz_result", "credential_scope_denied")
    );
    assert_eq!(
        read.boundary_label.as_metric_label().unwrap(),
        ("authz_boundary", "control_plane_read")
    );
    assert_eq!(
        read.result_label.as_metric_label().unwrap(),
        ("authz_result", "role_denied")
    );
    assert_eq!(
        mutation.boundary_label.as_metric_label().unwrap(),
        ("authz_boundary", "control_plane_mutation")
    );
    assert_eq!(
        mutation.result_label.as_metric_label().unwrap(),
        ("authz_result", "tenant_denied")
    );
    assert_eq!(
        billing.boundary_label.as_metric_label().unwrap(),
        ("authz_boundary", "control_plane_billing")
    );
    assert_eq!(
        billing.result_label.as_metric_label().unwrap(),
        ("authz_result", "resource_denied")
    );
    assert_eq!(
        break_glass.boundary_label.as_metric_label().unwrap(),
        ("authz_boundary", "break_glass")
    );
    assert_eq!(
        break_glass.result_label.as_metric_label().unwrap(),
        ("authz_result", "failed")
    );
    for metric in [inference, quota, read, mutation, billing, break_glass] {
        assert!(!metric.boundary_label.key.contains("tenant"));
        assert!(!metric.boundary_label.key.contains("principal"));
        assert!(!metric.boundary_label.key.contains("role_name"));
        assert!(!metric.boundary_label.key.contains("resource_id"));
        assert!(!metric.result_label.key.contains("action"));
        assert!(!metric.result_label.key.contains("scope"));
        assert!(!metric.result_label.key.contains("policy"));
    }
}

#[test]
fn credential_scope_mismatch_metric_uses_closed_direction_and_result_labels() {
    let data_to_control = plan_credential_scope_mismatch_metric(
        CredentialScopeMismatchDirection::DataPlaneToControlPlane,
        CredentialScopeMismatchResult::Rejected,
    )
    .unwrap();
    let control_to_data = plan_credential_scope_mismatch_metric(
        CredentialScopeMismatchDirection::ControlPlaneToDataPlane,
        CredentialScopeMismatchResult::Audited,
    )
    .unwrap();
    let break_to_data = plan_credential_scope_mismatch_metric(
        CredentialScopeMismatchDirection::BreakGlassToDataPlane,
        CredentialScopeMismatchResult::Rejected,
    )
    .unwrap();
    let break_to_control = plan_credential_scope_mismatch_metric(
        CredentialScopeMismatchDirection::BreakGlassToControlPlane,
        CredentialScopeMismatchResult::Failed,
    )
    .unwrap();
    let missing = plan_credential_scope_mismatch_metric(
        CredentialScopeMismatchDirection::MissingCredential,
        CredentialScopeMismatchResult::Rejected,
    )
    .unwrap();

    assert_eq!(
        data_to_control.metric_name,
        "prodex_credential_scope_mismatch_events_total"
    );
    assert_eq!(data_to_control.increment, 1);
    assert_eq!(
        data_to_control.direction_label.as_metric_label().unwrap(),
        ("credential_scope_direction", "data_plane_to_control_plane")
    );
    assert_eq!(
        data_to_control.result_label.as_metric_label().unwrap(),
        ("credential_scope_result", "rejected")
    );
    assert_eq!(
        control_to_data.direction_label.as_metric_label().unwrap(),
        ("credential_scope_direction", "control_plane_to_data_plane")
    );
    assert_eq!(
        control_to_data.result_label.as_metric_label().unwrap(),
        ("credential_scope_result", "audited")
    );
    assert_eq!(
        break_to_data.direction_label.as_metric_label().unwrap(),
        ("credential_scope_direction", "break_glass_to_data_plane")
    );
    assert_eq!(
        break_to_control.direction_label.as_metric_label().unwrap(),
        ("credential_scope_direction", "break_glass_to_control_plane")
    );
    assert_eq!(
        break_to_control.result_label.as_metric_label().unwrap(),
        ("credential_scope_result", "failed")
    );
    assert_eq!(
        missing.direction_label.as_metric_label().unwrap(),
        ("credential_scope_direction", "missing_credential")
    );
    for metric in [
        data_to_control,
        control_to_data,
        break_to_data,
        break_to_control,
        missing,
    ] {
        assert!(!metric.direction_label.key.contains("tenant"));
        assert!(!metric.direction_label.key.contains("principal"));
        assert!(!metric.direction_label.key.contains("token"));
        assert!(!metric.direction_label.key.contains("route"));
        assert!(!metric.result_label.key.contains("credential_id"));
        assert!(!metric.result_label.key.contains("path"));
        assert!(!metric.result_label.key.contains("request"));
    }
}

#[test]
fn tenant_isolation_metric_uses_closed_surface_and_result_labels() {
    let authn = plan_tenant_isolation_metric(
        TenantIsolationSurface::Authentication,
        TenantIsolationResult::MissingTenantDenied,
    )
    .unwrap();
    let authz = plan_tenant_isolation_metric(
        TenantIsolationSurface::Authorization,
        TenantIsolationResult::CrossTenantDenied,
    )
    .unwrap();
    let storage = plan_tenant_isolation_metric(
        TenantIsolationSurface::StoragePredicate,
        TenantIsolationResult::MismatchRejected,
    )
    .unwrap();
    let cache = plan_tenant_isolation_metric(
        TenantIsolationSurface::CacheKey,
        TenantIsolationResult::Enforced,
    )
    .unwrap();
    let audit = plan_tenant_isolation_metric(
        TenantIsolationSurface::AuditQuery,
        TenantIsolationResult::Failed,
    )
    .unwrap();

    assert_eq!(authn.metric_name, "prodex_tenant_isolation_events_total");
    assert_eq!(authn.increment, 1);
    assert_eq!(
        authn.surface_label.as_metric_label().unwrap(),
        ("tenant_isolation_surface", "authentication")
    );
    assert_eq!(
        authn.result_label.as_metric_label().unwrap(),
        ("tenant_isolation_result", "missing_tenant_denied")
    );
    assert_eq!(
        authz.surface_label.as_metric_label().unwrap(),
        ("tenant_isolation_surface", "authorization")
    );
    assert_eq!(
        authz.result_label.as_metric_label().unwrap(),
        ("tenant_isolation_result", "cross_tenant_denied")
    );
    assert_eq!(
        storage.surface_label.as_metric_label().unwrap(),
        ("tenant_isolation_surface", "storage_predicate")
    );
    assert_eq!(
        storage.result_label.as_metric_label().unwrap(),
        ("tenant_isolation_result", "mismatch_rejected")
    );
    assert_eq!(
        cache.surface_label.as_metric_label().unwrap(),
        ("tenant_isolation_surface", "cache_key")
    );
    assert_eq!(
        cache.result_label.as_metric_label().unwrap(),
        ("tenant_isolation_result", "enforced")
    );
    assert_eq!(
        audit.surface_label.as_metric_label().unwrap(),
        ("tenant_isolation_surface", "audit_query")
    );
    assert_eq!(
        audit.result_label.as_metric_label().unwrap(),
        ("tenant_isolation_result", "failed")
    );
    for metric in [authn, authz, storage, cache, audit] {
        assert!(!metric.surface_label.key.contains("tenant_id"));
        assert!(!metric.surface_label.key.contains("principal_id"));
        assert!(!metric.surface_label.key.contains("resource_id"));
        assert!(!metric.surface_label.key.contains("storage_key"));
        assert!(!metric.result_label.key.contains("tenant_id"));
        assert!(!metric.result_label.key.contains("query"));
        assert!(!metric.result_label.key.contains("cache_key"));
    }
}

#[test]
fn postgres_tenant_context_metric_uses_closed_operation_and_result_labels() {
    let set_context = plan_postgres_tenant_context_metric(
        PostgresTenantContextOperation::SetContext,
        PostgresTenantContextResult::Applied,
    )
    .unwrap();
    let verify = plan_postgres_tenant_context_metric(
        PostgresTenantContextOperation::VerifyContext,
        PostgresTenantContextResult::Missing,
    )
    .unwrap();
    let rls = plan_postgres_tenant_context_metric(
        PostgresTenantContextOperation::ApplyRlsPolicy,
        PostgresTenantContextResult::RlsDenied,
    )
    .unwrap();
    let dml = plan_postgres_tenant_context_metric(
        PostgresTenantContextOperation::ExecuteTenantDml,
        PostgresTenantContextResult::MismatchRejected,
    )
    .unwrap();
    let failed = plan_postgres_tenant_context_metric(
        PostgresTenantContextOperation::ExecuteTenantDml,
        PostgresTenantContextResult::Failed,
    )
    .unwrap();

    assert_eq!(
        set_context.metric_name,
        "prodex_postgres_tenant_context_events_total"
    );
    assert_eq!(set_context.increment, 1);
    assert_eq!(
        set_context.operation_label.as_metric_label().unwrap(),
        ("postgres_tenant_context_operation", "set_context")
    );
    assert_eq!(
        set_context.result_label.as_metric_label().unwrap(),
        ("postgres_tenant_context_result", "applied")
    );
    assert_eq!(
        verify.operation_label.as_metric_label().unwrap(),
        ("postgres_tenant_context_operation", "verify_context")
    );
    assert_eq!(
        verify.result_label.as_metric_label().unwrap(),
        ("postgres_tenant_context_result", "missing")
    );
    assert_eq!(
        rls.operation_label.as_metric_label().unwrap(),
        ("postgres_tenant_context_operation", "apply_rls_policy")
    );
    assert_eq!(
        rls.result_label.as_metric_label().unwrap(),
        ("postgres_tenant_context_result", "rls_denied")
    );
    assert_eq!(
        dml.operation_label.as_metric_label().unwrap(),
        ("postgres_tenant_context_operation", "execute_tenant_dml")
    );
    assert_eq!(
        dml.result_label.as_metric_label().unwrap(),
        ("postgres_tenant_context_result", "mismatch_rejected")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("postgres_tenant_context_result", "failed")
    );
    for metric in [set_context, verify, rls, dml, failed] {
        assert!(!metric.operation_label.key.contains("tenant_id"));
        assert!(!metric.operation_label.key.contains("statement"));
        assert!(!metric.operation_label.key.contains("query"));
        assert!(!metric.result_label.key.contains("tenant_id"));
        assert!(!metric.result_label.key.contains("database"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn identity_context_metric_uses_closed_surface_and_result_labels() {
    let authn = plan_identity_context_metric(
        IdentityContextSurface::Authentication,
        IdentityContextResult::MissingPrincipal,
    )
    .unwrap();
    let authz = plan_identity_context_metric(
        IdentityContextSurface::Authorization,
        IdentityContextResult::MissingTenant,
    )
    .unwrap();
    let audit = plan_identity_context_metric(
        IdentityContextSurface::Audit,
        IdentityContextResult::CorrelationMissing,
    )
    .unwrap();
    let control = plan_identity_context_metric(
        IdentityContextSurface::ControlPlane,
        IdentityContextResult::TenantMismatch,
    )
    .unwrap();
    let data = plan_identity_context_metric(
        IdentityContextSurface::DataPlane,
        IdentityContextResult::Consistent,
    )
    .unwrap();
    let failed =
        plan_identity_context_metric(IdentityContextSurface::Audit, IdentityContextResult::Failed)
            .unwrap();

    assert_eq!(authn.metric_name, "prodex_identity_context_events_total");
    assert_eq!(authn.increment, 1);
    assert_eq!(
        authn.surface_label.as_metric_label().unwrap(),
        ("identity_context_surface", "authentication")
    );
    assert_eq!(
        authn.result_label.as_metric_label().unwrap(),
        ("identity_context_result", "missing_principal")
    );
    assert_eq!(
        authz.surface_label.as_metric_label().unwrap(),
        ("identity_context_surface", "authorization")
    );
    assert_eq!(
        authz.result_label.as_metric_label().unwrap(),
        ("identity_context_result", "missing_tenant")
    );
    assert_eq!(
        audit.surface_label.as_metric_label().unwrap(),
        ("identity_context_surface", "audit")
    );
    assert_eq!(
        audit.result_label.as_metric_label().unwrap(),
        ("identity_context_result", "correlation_missing")
    );
    assert_eq!(
        control.surface_label.as_metric_label().unwrap(),
        ("identity_context_surface", "control_plane")
    );
    assert_eq!(
        control.result_label.as_metric_label().unwrap(),
        ("identity_context_result", "tenant_mismatch")
    );
    assert_eq!(
        data.surface_label.as_metric_label().unwrap(),
        ("identity_context_surface", "data_plane")
    );
    assert_eq!(
        data.result_label.as_metric_label().unwrap(),
        ("identity_context_result", "consistent")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("identity_context_result", "failed")
    );
    for metric in [authn, authz, audit, control, data, failed] {
        assert!(!metric.surface_label.key.contains("tenant_id"));
        assert!(!metric.surface_label.key.contains("principal_id"));
        assert!(!metric.surface_label.key.contains("request_id"));
        assert!(!metric.surface_label.key.contains("audit_event_id"));
        assert!(!metric.result_label.key.contains("trace_id"));
        assert!(!metric.result_label.key.contains("raw_context"));
        assert!(!metric.result_label.key.contains("claim"));
    }
}

#[test]
fn break_glass_lifecycle_metric_uses_closed_operation_and_result_labels() {
    let request = plan_break_glass_lifecycle_metric(
        BreakGlassLifecycleOperation::Request,
        BreakGlassLifecycleResult::Authorized,
    )
    .unwrap();
    let approve = plan_break_glass_lifecycle_metric(
        BreakGlassLifecycleOperation::Approve,
        BreakGlassLifecycleResult::Persisted,
    )
    .unwrap();
    let activate = plan_break_glass_lifecycle_metric(
        BreakGlassLifecycleOperation::Activate,
        BreakGlassLifecycleResult::Denied,
    )
    .unwrap();
    let revoke = plan_break_glass_lifecycle_metric(
        BreakGlassLifecycleOperation::Revoke,
        BreakGlassLifecycleResult::Failed,
    )
    .unwrap();
    let expire = plan_break_glass_lifecycle_metric(
        BreakGlassLifecycleOperation::Expire,
        BreakGlassLifecycleResult::Expired,
    )
    .unwrap();

    assert_eq!(
        request.metric_name,
        "prodex_break_glass_lifecycle_events_total"
    );
    assert_eq!(request.increment, 1);
    assert_eq!(
        request.operation_label.as_metric_label().unwrap(),
        ("break_glass_operation", "request")
    );
    assert_eq!(
        request.result_label.as_metric_label().unwrap(),
        ("break_glass_result", "authorized")
    );
    assert_eq!(
        approve.operation_label.as_metric_label().unwrap(),
        ("break_glass_operation", "approve")
    );
    assert_eq!(
        approve.result_label.as_metric_label().unwrap(),
        ("break_glass_result", "persisted")
    );
    assert_eq!(
        activate.operation_label.as_metric_label().unwrap(),
        ("break_glass_operation", "activate")
    );
    assert_eq!(
        activate.result_label.as_metric_label().unwrap(),
        ("break_glass_result", "denied")
    );
    assert_eq!(
        revoke.operation_label.as_metric_label().unwrap(),
        ("break_glass_operation", "revoke")
    );
    assert_eq!(
        revoke.result_label.as_metric_label().unwrap(),
        ("break_glass_result", "failed")
    );
    assert_eq!(
        expire.operation_label.as_metric_label().unwrap(),
        ("break_glass_operation", "expire")
    );
    assert_eq!(
        expire.result_label.as_metric_label().unwrap(),
        ("break_glass_result", "expired")
    );
    for metric in [request, approve, activate, revoke, expire] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("principal_id"));
        assert!(!metric.operation_label.key.contains("token"));
        assert!(!metric.operation_label.key.contains("ttl"));
        assert!(!metric.result_label.key.contains("request"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn user_lifecycle_metric_uses_closed_operation_and_result_labels() {
    let invite = plan_user_lifecycle_metric(
        UserLifecycleOperation::Invite,
        UserLifecycleResult::Authorized,
    )
    .unwrap();
    let create = plan_user_lifecycle_metric(
        UserLifecycleOperation::ScimCreate,
        UserLifecycleResult::Persisted,
    )
    .unwrap();
    let update = plan_user_lifecycle_metric(
        UserLifecycleOperation::ScimUpdate,
        UserLifecycleResult::Failed,
    )
    .unwrap();
    let delete = plan_user_lifecycle_metric(
        UserLifecycleOperation::ScimDelete,
        UserLifecycleResult::Denied,
    )
    .unwrap();

    assert_eq!(invite.metric_name, "prodex_user_lifecycle_events_total");
    assert_eq!(invite.increment, 1);
    assert_eq!(
        invite.operation_label.as_metric_label().unwrap(),
        ("user_lifecycle_operation", "invite")
    );
    assert_eq!(
        invite.result_label.as_metric_label().unwrap(),
        ("user_lifecycle_result", "authorized")
    );
    assert_eq!(
        create.operation_label.as_metric_label().unwrap(),
        ("user_lifecycle_operation", "scim_create")
    );
    assert_eq!(
        create.result_label.as_metric_label().unwrap(),
        ("user_lifecycle_result", "persisted")
    );
    assert_eq!(
        update.operation_label.as_metric_label().unwrap(),
        ("user_lifecycle_operation", "scim_update")
    );
    assert_eq!(
        update.result_label.as_metric_label().unwrap(),
        ("user_lifecycle_result", "failed")
    );
    assert_eq!(
        delete.operation_label.as_metric_label().unwrap(),
        ("user_lifecycle_operation", "scim_delete")
    );
    assert_eq!(
        delete.result_label.as_metric_label().unwrap(),
        ("user_lifecycle_result", "denied")
    );
    for metric in [invite, create, update, delete] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("user_id"));
        assert!(!metric.operation_label.key.contains("email"));
        assert!(!metric.result_label.key.contains("scim_id"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn service_identity_lifecycle_metric_uses_closed_operation_and_result_labels() {
    let create = plan_service_identity_lifecycle_metric(
        ServiceIdentityLifecycleOperation::Create,
        ServiceIdentityLifecycleResult::Authorized,
    )
    .unwrap();
    let rotate = plan_service_identity_lifecycle_metric(
        ServiceIdentityLifecycleOperation::RotateSecret,
        ServiceIdentityLifecycleResult::Persisted,
    )
    .unwrap();
    let disable = plan_service_identity_lifecycle_metric(
        ServiceIdentityLifecycleOperation::Disable,
        ServiceIdentityLifecycleResult::Denied,
    )
    .unwrap();
    let failed = plan_service_identity_lifecycle_metric(
        ServiceIdentityLifecycleOperation::Create,
        ServiceIdentityLifecycleResult::Failed,
    )
    .unwrap();

    assert_eq!(
        create.metric_name,
        "prodex_service_identity_lifecycle_events_total"
    );
    assert_eq!(create.increment, 1);
    assert_eq!(
        create.operation_label.as_metric_label().unwrap(),
        ("service_identity_operation", "create")
    );
    assert_eq!(
        create.result_label.as_metric_label().unwrap(),
        ("service_identity_result", "authorized")
    );
    assert_eq!(
        rotate.operation_label.as_metric_label().unwrap(),
        ("service_identity_operation", "rotate_secret")
    );
    assert_eq!(
        rotate.result_label.as_metric_label().unwrap(),
        ("service_identity_result", "persisted")
    );
    assert_eq!(
        disable.operation_label.as_metric_label().unwrap(),
        ("service_identity_operation", "disable")
    );
    assert_eq!(
        disable.result_label.as_metric_label().unwrap(),
        ("service_identity_result", "denied")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("service_identity_result", "failed")
    );
    for metric in [create, rotate, disable, failed] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("principal_id"));
        assert!(!metric.operation_label.key.contains("client_id"));
        assert!(!metric.result_label.key.contains("secret"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn role_binding_lifecycle_metric_uses_closed_operation_and_result_labels() {
    let grant = plan_role_binding_lifecycle_metric(
        RoleBindingLifecycleOperation::Grant,
        RoleBindingLifecycleResult::Authorized,
    )
    .unwrap();
    let persisted = plan_role_binding_lifecycle_metric(
        RoleBindingLifecycleOperation::Grant,
        RoleBindingLifecycleResult::Persisted,
    )
    .unwrap();
    let revoke = plan_role_binding_lifecycle_metric(
        RoleBindingLifecycleOperation::Revoke,
        RoleBindingLifecycleResult::Denied,
    )
    .unwrap();
    let failed = plan_role_binding_lifecycle_metric(
        RoleBindingLifecycleOperation::Revoke,
        RoleBindingLifecycleResult::Failed,
    )
    .unwrap();

    assert_eq!(
        grant.metric_name,
        "prodex_role_binding_lifecycle_events_total"
    );
    assert_eq!(grant.increment, 1);
    assert_eq!(
        grant.operation_label.as_metric_label().unwrap(),
        ("role_binding_operation", "grant")
    );
    assert_eq!(
        grant.result_label.as_metric_label().unwrap(),
        ("role_binding_result", "authorized")
    );
    assert_eq!(
        persisted.result_label.as_metric_label().unwrap(),
        ("role_binding_result", "persisted")
    );
    assert_eq!(
        revoke.operation_label.as_metric_label().unwrap(),
        ("role_binding_operation", "revoke")
    );
    assert_eq!(
        revoke.result_label.as_metric_label().unwrap(),
        ("role_binding_result", "denied")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("role_binding_result", "failed")
    );
    for metric in [grant, persisted, revoke, failed] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("principal_id"));
        assert!(!metric.operation_label.key.contains("role_name"));
        assert!(!metric.result_label.key.contains("binding_id"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn provider_credential_lifecycle_metric_uses_closed_operation_and_result_labels() {
    let rotate = plan_provider_credential_lifecycle_metric(
        ProviderCredentialLifecycleOperation::Rotate,
        ProviderCredentialLifecycleResult::Authorized,
    )
    .unwrap();
    let validate = plan_provider_credential_lifecycle_metric(
        ProviderCredentialLifecycleOperation::ValidateReference,
        ProviderCredentialLifecycleResult::Denied,
    )
    .unwrap();
    let persist = plan_provider_credential_lifecycle_metric(
        ProviderCredentialLifecycleOperation::PersistReference,
        ProviderCredentialLifecycleResult::Persisted,
    )
    .unwrap();
    let failed = plan_provider_credential_lifecycle_metric(
        ProviderCredentialLifecycleOperation::PersistReference,
        ProviderCredentialLifecycleResult::Failed,
    )
    .unwrap();

    assert_eq!(
        rotate.metric_name,
        "prodex_provider_credential_lifecycle_events_total"
    );
    assert_eq!(rotate.increment, 1);
    assert_eq!(
        rotate.operation_label.as_metric_label().unwrap(),
        ("provider_credential_operation", "rotate")
    );
    assert_eq!(
        rotate.result_label.as_metric_label().unwrap(),
        ("provider_credential_result", "authorized")
    );
    assert_eq!(
        validate.operation_label.as_metric_label().unwrap(),
        ("provider_credential_operation", "validate_reference")
    );
    assert_eq!(
        validate.result_label.as_metric_label().unwrap(),
        ("provider_credential_result", "denied")
    );
    assert_eq!(
        persist.operation_label.as_metric_label().unwrap(),
        ("provider_credential_operation", "persist_reference")
    );
    assert_eq!(
        persist.result_label.as_metric_label().unwrap(),
        ("provider_credential_result", "persisted")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("provider_credential_result", "failed")
    );
    for metric in [rotate, validate, persist, failed] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("credential_id"));
        assert!(!metric.operation_label.key.contains("provider_account"));
        assert!(!metric.result_label.key.contains("secret"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn virtual_key_lifecycle_metric_uses_closed_operation_and_result_labels() {
    let create = plan_virtual_key_lifecycle_metric(
        VirtualKeyLifecycleOperation::Create,
        VirtualKeyLifecycleResult::Authorized,
    )
    .unwrap();
    let rotate = plan_virtual_key_lifecycle_metric(
        VirtualKeyLifecycleOperation::RotateSecret,
        VirtualKeyLifecycleResult::Denied,
    )
    .unwrap();
    let persist = plan_virtual_key_lifecycle_metric(
        VirtualKeyLifecycleOperation::PersistReference,
        VirtualKeyLifecycleResult::Persisted,
    )
    .unwrap();
    let failed = plan_virtual_key_lifecycle_metric(
        VirtualKeyLifecycleOperation::PersistReference,
        VirtualKeyLifecycleResult::Failed,
    )
    .unwrap();

    assert_eq!(
        create.metric_name,
        "prodex_virtual_key_lifecycle_events_total"
    );
    assert_eq!(create.increment, 1);
    assert_eq!(
        create.operation_label.as_metric_label().unwrap(),
        ("credential_lifecycle_operation", "create")
    );
    assert_eq!(
        create.result_label.as_metric_label().unwrap(),
        ("credential_lifecycle_result", "authorized")
    );
    assert_eq!(
        rotate.operation_label.as_metric_label().unwrap(),
        ("credential_lifecycle_operation", "rotate_secret")
    );
    assert_eq!(
        rotate.result_label.as_metric_label().unwrap(),
        ("credential_lifecycle_result", "denied")
    );
    assert_eq!(
        persist.operation_label.as_metric_label().unwrap(),
        ("credential_lifecycle_operation", "persist_reference")
    );
    assert_eq!(
        persist.result_label.as_metric_label().unwrap(),
        ("credential_lifecycle_result", "persisted")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("credential_lifecycle_result", "failed")
    );
    for metric in [create, rotate, persist, failed] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("virtual_key_id"));
        assert!(!metric.operation_label.key.contains("key_name"));
        assert!(!metric.result_label.key.contains("secret"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn budget_policy_lifecycle_metric_uses_closed_operation_and_result_labels() {
    let update = plan_budget_policy_lifecycle_metric(
        BudgetPolicyLifecycleOperation::Update,
        BudgetPolicyLifecycleResult::Authorized,
    )
    .unwrap();
    let validate = plan_budget_policy_lifecycle_metric(
        BudgetPolicyLifecycleOperation::ValidateScope,
        BudgetPolicyLifecycleResult::Denied,
    )
    .unwrap();
    let persist = plan_budget_policy_lifecycle_metric(
        BudgetPolicyLifecycleOperation::PersistPolicy,
        BudgetPolicyLifecycleResult::Persisted,
    )
    .unwrap();
    let failed = plan_budget_policy_lifecycle_metric(
        BudgetPolicyLifecycleOperation::PersistPolicy,
        BudgetPolicyLifecycleResult::Failed,
    )
    .unwrap();

    assert_eq!(
        update.metric_name,
        "prodex_budget_policy_lifecycle_events_total"
    );
    assert_eq!(update.increment, 1);
    assert_eq!(
        update.operation_label.as_metric_label().unwrap(),
        ("budget_policy_operation", "update")
    );
    assert_eq!(
        update.result_label.as_metric_label().unwrap(),
        ("budget_policy_result", "authorized")
    );
    assert_eq!(
        validate.operation_label.as_metric_label().unwrap(),
        ("budget_policy_operation", "validate_scope")
    );
    assert_eq!(
        validate.result_label.as_metric_label().unwrap(),
        ("budget_policy_result", "denied")
    );
    assert_eq!(
        persist.operation_label.as_metric_label().unwrap(),
        ("budget_policy_operation", "persist_policy")
    );
    assert_eq!(
        persist.result_label.as_metric_label().unwrap(),
        ("budget_policy_result", "persisted")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("budget_policy_result", "failed")
    );
    for metric in [update, validate, persist, failed] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("scope_id"));
        assert!(!metric.operation_label.key.contains("amount"));
        assert!(!metric.result_label.key.contains("limit"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn policy_lifecycle_metric_uses_closed_operation_and_result_labels() {
    let create = plan_policy_lifecycle_metric(
        PolicyLifecycleOperation::Create,
        PolicyLifecycleResult::Authorized,
    )
    .unwrap();
    let update = plan_policy_lifecycle_metric(
        PolicyLifecycleOperation::Update,
        PolicyLifecycleResult::Persisted,
    )
    .unwrap();
    let publish = plan_policy_lifecycle_metric(
        PolicyLifecycleOperation::Publish,
        PolicyLifecycleResult::Published,
    )
    .unwrap();
    let invalidate = plan_policy_lifecycle_metric(
        PolicyLifecycleOperation::Invalidate,
        PolicyLifecycleResult::Denied,
    )
    .unwrap();
    let failed = plan_policy_lifecycle_metric(
        PolicyLifecycleOperation::Publish,
        PolicyLifecycleResult::Failed,
    )
    .unwrap();

    assert_eq!(create.metric_name, "prodex_policy_lifecycle_events_total");
    assert_eq!(create.increment, 1);
    assert_eq!(
        create.operation_label.as_metric_label().unwrap(),
        ("policy_lifecycle_operation", "create")
    );
    assert_eq!(
        create.result_label.as_metric_label().unwrap(),
        ("policy_lifecycle_result", "authorized")
    );
    assert_eq!(
        update.operation_label.as_metric_label().unwrap(),
        ("policy_lifecycle_operation", "update")
    );
    assert_eq!(
        update.result_label.as_metric_label().unwrap(),
        ("policy_lifecycle_result", "persisted")
    );
    assert_eq!(
        publish.operation_label.as_metric_label().unwrap(),
        ("policy_lifecycle_operation", "publish")
    );
    assert_eq!(
        publish.result_label.as_metric_label().unwrap(),
        ("policy_lifecycle_result", "published")
    );
    assert_eq!(
        invalidate.operation_label.as_metric_label().unwrap(),
        ("policy_lifecycle_operation", "invalidate")
    );
    assert_eq!(
        invalidate.result_label.as_metric_label().unwrap(),
        ("policy_lifecycle_result", "denied")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("policy_lifecycle_result", "failed")
    );

    for metric in [create, update, publish, invalidate, failed] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("policy_revision"));
        assert!(!metric.operation_label.key.contains("policy_id"));
        assert!(!metric.result_label.key.contains("digest"));
        assert!(!metric.result_label.key.contains("payload"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn tenant_lifecycle_metric_uses_closed_operation_and_result_labels() {
    let create = plan_tenant_lifecycle_metric(
        TenantLifecycleOperation::Create,
        TenantLifecycleResult::Authorized,
    )
    .unwrap();
    let update = plan_tenant_lifecycle_metric(
        TenantLifecycleOperation::Update,
        TenantLifecycleResult::Persisted,
    )
    .unwrap();
    let denied = plan_tenant_lifecycle_metric(
        TenantLifecycleOperation::Update,
        TenantLifecycleResult::Denied,
    )
    .unwrap();
    let failed = plan_tenant_lifecycle_metric(
        TenantLifecycleOperation::Create,
        TenantLifecycleResult::Failed,
    )
    .unwrap();

    assert_eq!(create.metric_name, "prodex_tenant_lifecycle_events_total");
    assert_eq!(create.increment, 1);
    assert_eq!(
        create.operation_label.as_metric_label().unwrap(),
        ("account_lifecycle_operation", "create")
    );
    assert_eq!(
        create.result_label.as_metric_label().unwrap(),
        ("account_lifecycle_result", "authorized")
    );
    assert_eq!(
        update.operation_label.as_metric_label().unwrap(),
        ("account_lifecycle_operation", "update")
    );
    assert_eq!(
        update.result_label.as_metric_label().unwrap(),
        ("account_lifecycle_result", "persisted")
    );
    assert_eq!(
        denied.result_label.as_metric_label().unwrap(),
        ("account_lifecycle_result", "denied")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("account_lifecycle_result", "failed")
    );
    for metric in [create, update, denied, failed] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("tenant_id"));
        assert!(!metric.operation_label.key.contains("display_name"));
        assert!(!metric.result_label.key.contains("tenant"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn reservation_recovery_metric_uses_closed_operation_and_result_labels() {
    let scan = plan_reservation_recovery_metric(
        ReservationRecoveryOperation::ScanExpired,
        ReservationRecoveryResult::Recovered,
    )
    .unwrap();
    let lease = plan_reservation_recovery_metric(
        ReservationRecoveryOperation::AcquireLease,
        ReservationRecoveryResult::LeaseUnavailable,
    )
    .unwrap();
    let release = plan_reservation_recovery_metric(
        ReservationRecoveryOperation::ReleaseBudget,
        ReservationRecoveryResult::Skipped,
    )
    .unwrap();
    let ledger = plan_reservation_recovery_metric(
        ReservationRecoveryOperation::WriteLedger,
        ReservationRecoveryResult::Failed,
    )
    .unwrap();

    assert_eq!(scan.metric_name, "prodex_reservation_recovery_events_total");
    assert_eq!(scan.increment, 1);
    assert_eq!(
        scan.operation_label.as_metric_label().unwrap(),
        ("reservation_recovery_operation", "scan_expired")
    );
    assert_eq!(
        scan.result_label.as_metric_label().unwrap(),
        ("reservation_recovery_result", "recovered")
    );
    assert_eq!(
        lease.operation_label.as_metric_label().unwrap(),
        ("reservation_recovery_operation", "acquire_lease")
    );
    assert_eq!(
        lease.result_label.as_metric_label().unwrap(),
        ("reservation_recovery_result", "lease_unavailable")
    );
    assert_eq!(
        release.operation_label.as_metric_label().unwrap(),
        ("reservation_recovery_operation", "release_budget")
    );
    assert_eq!(
        release.result_label.as_metric_label().unwrap(),
        ("reservation_recovery_result", "skipped")
    );
    assert_eq!(
        ledger.operation_label.as_metric_label().unwrap(),
        ("reservation_recovery_operation", "write_ledger")
    );
    assert_eq!(
        ledger.result_label.as_metric_label().unwrap(),
        ("reservation_recovery_result", "failed")
    );
    for metric in [scan, lease, release, ledger] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("reservation_id"));
        assert!(!metric.operation_label.key.contains("lease_key"));
        assert!(!metric.result_label.key.contains("amount"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn accounting_metric_uses_closed_operation_and_result_labels() {
    let reservation =
        plan_accounting_metric(AccountingOperation::Reservation, AccountingResult::Accepted)
            .unwrap();
    let commit =
        plan_accounting_metric(AccountingOperation::Commit, AccountingResult::Committed).unwrap();
    let release =
        plan_accounting_metric(AccountingOperation::Release, AccountingResult::Released).unwrap();
    let expire =
        plan_accounting_metric(AccountingOperation::Expire, AccountingResult::Expired).unwrap();
    let reconciliation = plan_accounting_metric(
        AccountingOperation::Reconciliation,
        AccountingResult::Reconciled,
    )
    .unwrap();
    let budget_rejection = plan_accounting_metric(
        AccountingOperation::BudgetRejection,
        AccountingResult::Rejected,
    )
    .unwrap();
    let failed =
        plan_accounting_metric(AccountingOperation::Commit, AccountingResult::Failed).unwrap();

    assert_eq!(reservation.metric_name, "prodex_accounting_events_total");
    assert_eq!(reservation.increment, 1);
    assert_eq!(
        reservation.operation_label.as_metric_label().unwrap(),
        ("accounting_operation", "reservation")
    );
    assert_eq!(
        reservation.result_label.as_metric_label().unwrap(),
        ("accounting_result", "accepted")
    );
    assert_eq!(
        commit.operation_label.as_metric_label().unwrap(),
        ("accounting_operation", "commit")
    );
    assert_eq!(
        commit.result_label.as_metric_label().unwrap(),
        ("accounting_result", "committed")
    );
    assert_eq!(
        release.operation_label.as_metric_label().unwrap(),
        ("accounting_operation", "release")
    );
    assert_eq!(
        release.result_label.as_metric_label().unwrap(),
        ("accounting_result", "released")
    );
    assert_eq!(
        expire.operation_label.as_metric_label().unwrap(),
        ("accounting_operation", "expire")
    );
    assert_eq!(
        expire.result_label.as_metric_label().unwrap(),
        ("accounting_result", "expired")
    );
    assert_eq!(
        reconciliation.operation_label.as_metric_label().unwrap(),
        ("accounting_operation", "reconciliation")
    );
    assert_eq!(
        reconciliation.result_label.as_metric_label().unwrap(),
        ("accounting_result", "reconciled")
    );
    assert_eq!(
        budget_rejection.operation_label.as_metric_label().unwrap(),
        ("accounting_operation", "budget_rejection")
    );
    assert_eq!(
        budget_rejection.result_label.as_metric_label().unwrap(),
        ("accounting_result", "rejected")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("accounting_result", "failed")
    );
    for metric in [
        reservation,
        commit,
        release,
        expire,
        reconciliation,
        budget_rejection,
        failed,
    ] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("reservation_id"));
        assert!(!metric.result_label.key.contains("call_id"));
        assert!(!metric.result_label.key.contains("virtual_key"));
    }
}

#[test]
fn billing_ledger_metric_uses_closed_operation_and_result_labels() {
    let reserve = plan_billing_ledger_metric(
        BillingLedgerOperation::ReserveAppend,
        BillingLedgerResult::Written,
    )
    .unwrap();
    let commit = plan_billing_ledger_metric(
        BillingLedgerOperation::CommitAppend,
        BillingLedgerResult::Failed,
    )
    .unwrap();
    let release = plan_billing_ledger_metric(
        BillingLedgerOperation::ReleaseAppend,
        BillingLedgerResult::Skipped,
    )
    .unwrap();
    let reconciliation = plan_billing_ledger_metric(
        BillingLedgerOperation::ReconciliationAppend,
        BillingLedgerResult::Written,
    )
    .unwrap();
    let query =
        plan_billing_ledger_metric(BillingLedgerOperation::Query, BillingLedgerResult::Read)
            .unwrap();

    assert_eq!(reserve.metric_name, "prodex_billing_ledger_events_total");
    assert_eq!(reserve.increment, 1);
    assert_eq!(
        reserve.operation_label.as_metric_label().unwrap(),
        ("billing_ledger_operation", "reserve_append")
    );
    assert_eq!(
        reserve.result_label.as_metric_label().unwrap(),
        ("billing_ledger_result", "written")
    );
    assert_eq!(
        commit.operation_label.as_metric_label().unwrap(),
        ("billing_ledger_operation", "commit_append")
    );
    assert_eq!(
        commit.result_label.as_metric_label().unwrap(),
        ("billing_ledger_result", "failed")
    );
    assert_eq!(
        release.operation_label.as_metric_label().unwrap(),
        ("billing_ledger_operation", "release_append")
    );
    assert_eq!(
        release.result_label.as_metric_label().unwrap(),
        ("billing_ledger_result", "skipped")
    );
    assert_eq!(
        reconciliation.operation_label.as_metric_label().unwrap(),
        ("billing_ledger_operation", "reconciliation_append")
    );
    assert_eq!(
        query.operation_label.as_metric_label().unwrap(),
        ("billing_ledger_operation", "query")
    );
    assert_eq!(
        query.result_label.as_metric_label().unwrap(),
        ("billing_ledger_result", "read")
    );
    for metric in [reserve, commit, release, reconciliation, query] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("reservation_id"));
        assert!(!metric.operation_label.key.contains("call_id"));
        assert!(!metric.result_label.key.contains("ledger_id"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn budget_rejection_metric_uses_closed_reason_labels() {
    let tenant = plan_budget_rejection_metric(BudgetRejectionReason::TenantBudgetExceeded).unwrap();
    let key =
        plan_budget_rejection_metric(BudgetRejectionReason::VirtualKeyBudgetExceeded).unwrap();
    let rate_limited = plan_budget_rejection_metric(BudgetRejectionReason::RateLimited).unwrap();
    let unavailable =
        plan_budget_rejection_metric(BudgetRejectionReason::ReservationUnavailable).unwrap();
    let policy = plan_budget_rejection_metric(BudgetRejectionReason::PolicyDenied).unwrap();

    assert_eq!(tenant.metric_name, "prodex_budget_rejections_total");
    assert_eq!(tenant.increment, 1);
    assert_eq!(
        tenant.reason_label.as_metric_label().unwrap(),
        ("budget_rejection_reason", "tenant_budget_exceeded")
    );
    assert_eq!(
        key.reason_label.as_metric_label().unwrap(),
        ("budget_rejection_reason", "virtual_key_budget_exceeded")
    );
    assert_eq!(
        rate_limited.reason_label.as_metric_label().unwrap(),
        ("budget_rejection_reason", "rate_limited")
    );
    assert_eq!(
        unavailable.reason_label.as_metric_label().unwrap(),
        ("budget_rejection_reason", "reservation_unavailable")
    );
    assert_eq!(
        policy.reason_label.as_metric_label().unwrap(),
        ("budget_rejection_reason", "policy_denied")
    );
    for metric in [tenant, key, rate_limited, unavailable, policy] {
        assert!(!metric.reason_label.key.contains("tenant_id"));
        assert!(!metric.reason_label.key.contains("virtual_key_id"));
        assert!(!metric.reason_label.key.contains("request"));
        assert!(!metric.reason_label.key.contains("prompt"));
        assert!(!metric.reason_label.key.contains("raw"));
    }
}

#[test]
fn rate_limit_decision_metric_uses_closed_scope_and_decision_labels() {
    let tenant =
        plan_rate_limit_decision_metric(RateLimitScope::Tenant, RateLimitDecision::Allowed)
            .unwrap();
    let virtual_key =
        plan_rate_limit_decision_metric(RateLimitScope::VirtualKey, RateLimitDecision::Delayed)
            .unwrap();
    let principal =
        plan_rate_limit_decision_metric(RateLimitScope::Principal, RateLimitDecision::Rejected)
            .unwrap();
    let provider =
        plan_rate_limit_decision_metric(RateLimitScope::Provider, RateLimitDecision::Unavailable)
            .unwrap();

    assert_eq!(tenant.metric_name, "prodex_rate_limit_decisions_total");
    assert_eq!(tenant.increment, 1);
    assert_eq!(
        tenant.scope_label.as_metric_label().unwrap(),
        ("rate_limit_scope", "tenant")
    );
    assert_eq!(
        tenant.decision_label.as_metric_label().unwrap(),
        ("rate_limit_decision", "allowed")
    );
    assert_eq!(
        virtual_key.scope_label.as_metric_label().unwrap(),
        ("rate_limit_scope", "virtual_key")
    );
    assert_eq!(
        virtual_key.decision_label.as_metric_label().unwrap(),
        ("rate_limit_decision", "delayed")
    );
    assert_eq!(
        principal.scope_label.as_metric_label().unwrap(),
        ("rate_limit_scope", "principal")
    );
    assert_eq!(
        principal.decision_label.as_metric_label().unwrap(),
        ("rate_limit_decision", "rejected")
    );
    assert_eq!(
        provider.scope_label.as_metric_label().unwrap(),
        ("rate_limit_scope", "provider")
    );
    assert_eq!(
        provider.decision_label.as_metric_label().unwrap(),
        ("rate_limit_decision", "unavailable")
    );
    for metric in [tenant, virtual_key, principal, provider] {
        assert!(!metric.scope_label.key.contains("tenant_id"));
        assert!(!metric.scope_label.key.contains("virtual_key_id"));
        assert!(!metric.scope_label.key.contains("principal_id"));
        assert!(!metric.decision_label.key.contains("request"));
        assert!(!metric.decision_label.key.contains("error_text"));
    }
}

#[test]
fn redis_coordination_metric_uses_closed_operation_and_result_labels() {
    let rate_limit_check = plan_redis_coordination_metric(
        RedisCoordinationOperation::RateLimitCheck,
        RedisCoordinationResult::Limited,
    )
    .unwrap();
    let rate_limit_commit = plan_redis_coordination_metric(
        RedisCoordinationOperation::RateLimitCommit,
        RedisCoordinationResult::Success,
    )
    .unwrap();
    let lease_acquire = plan_redis_coordination_metric(
        RedisCoordinationOperation::RecoveryLeaseAcquire,
        RedisCoordinationResult::LeaseUnavailable,
    )
    .unwrap();
    let lease_release = plan_redis_coordination_metric(
        RedisCoordinationOperation::RecoveryLeaseRelease,
        RedisCoordinationResult::Unavailable,
    )
    .unwrap();
    let cache_read = plan_redis_coordination_metric(
        RedisCoordinationOperation::CacheRead,
        RedisCoordinationResult::CacheMiss,
    )
    .unwrap();
    let cache_write = plan_redis_coordination_metric(
        RedisCoordinationOperation::CacheWrite,
        RedisCoordinationResult::Failed,
    )
    .unwrap();

    assert_eq!(
        rate_limit_check.metric_name,
        "prodex_redis_coordination_events_total"
    );
    assert_eq!(rate_limit_check.increment, 1);
    assert_eq!(
        rate_limit_check.operation_label.as_metric_label().unwrap(),
        ("redis_coordination_operation", "rate_limit_check")
    );
    assert_eq!(
        rate_limit_check.result_label.as_metric_label().unwrap(),
        ("redis_coordination_result", "limited")
    );
    assert_eq!(
        rate_limit_commit.operation_label.as_metric_label().unwrap(),
        ("redis_coordination_operation", "rate_limit_commit")
    );
    assert_eq!(
        rate_limit_commit.result_label.as_metric_label().unwrap(),
        ("redis_coordination_result", "success")
    );
    assert_eq!(
        lease_acquire.operation_label.as_metric_label().unwrap(),
        ("redis_coordination_operation", "recovery_lease_acquire")
    );
    assert_eq!(
        lease_acquire.result_label.as_metric_label().unwrap(),
        ("redis_coordination_result", "lease_unavailable")
    );
    assert_eq!(
        lease_release.operation_label.as_metric_label().unwrap(),
        ("redis_coordination_operation", "recovery_lease_release")
    );
    assert_eq!(
        lease_release.result_label.as_metric_label().unwrap(),
        ("redis_coordination_result", "unavailable")
    );
    assert_eq!(
        cache_read.operation_label.as_metric_label().unwrap(),
        ("redis_coordination_operation", "cache_read")
    );
    assert_eq!(
        cache_read.result_label.as_metric_label().unwrap(),
        ("redis_coordination_result", "cache_miss")
    );
    assert_eq!(
        cache_write.operation_label.as_metric_label().unwrap(),
        ("redis_coordination_operation", "cache_write")
    );
    assert_eq!(
        cache_write.result_label.as_metric_label().unwrap(),
        ("redis_coordination_result", "failed")
    );

    for metric in [
        rate_limit_check,
        rate_limit_commit,
        lease_acquire,
        lease_release,
        cache_read,
        cache_write,
    ] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("key"));
        assert!(!metric.operation_label.key.contains("lease_owner"));
        assert!(!metric.operation_label.key.contains("endpoint"));
        assert!(!metric.operation_label.key.contains("script"));
        assert!(!metric.result_label.key.contains("request"));
        assert!(!metric.result_label.key.contains("storage_error"));
    }
}

#[test]
fn quota_correctness_metric_uses_closed_event_labels() {
    let overshoot =
        plan_quota_correctness_metric(QuotaCorrectnessEvent::ReservationOvershoot).unwrap();
    let duplicate =
        plan_quota_correctness_metric(QuotaCorrectnessEvent::DuplicateChargePrevented).unwrap();
    let missing_commit =
        plan_quota_correctness_metric(QuotaCorrectnessEvent::MissingCommitRecovered).unwrap();
    let missing_release =
        plan_quota_correctness_metric(QuotaCorrectnessEvent::MissingReleaseRecovered).unwrap();
    let ledger_mismatch =
        plan_quota_correctness_metric(QuotaCorrectnessEvent::LedgerMismatchDetected).unwrap();

    assert_eq!(
        overshoot.metric_name,
        "prodex_quota_correctness_events_total"
    );
    assert_eq!(overshoot.increment, 1);
    assert_eq!(
        overshoot.event_label.as_metric_label().unwrap(),
        ("quota_correctness_event", "reservation_overshoot")
    );
    assert_eq!(
        duplicate.event_label.as_metric_label().unwrap(),
        ("quota_correctness_event", "duplicate_charge_prevented")
    );
    assert_eq!(
        missing_commit.event_label.as_metric_label().unwrap(),
        ("quota_correctness_event", "missing_commit_recovered")
    );
    assert_eq!(
        missing_release.event_label.as_metric_label().unwrap(),
        ("quota_correctness_event", "missing_release_recovered")
    );
    assert_eq!(
        ledger_mismatch.event_label.as_metric_label().unwrap(),
        ("quota_correctness_event", "ledger_mismatch_detected")
    );
    for metric in [
        overshoot,
        duplicate,
        missing_commit,
        missing_release,
        ledger_mismatch,
    ] {
        assert!(!metric.event_label.key.contains("tenant"));
        assert!(!metric.event_label.key.contains("reservation_id"));
        assert!(!metric.event_label.key.contains("call_id"));
        assert!(!metric.event_label.key.contains("ledger_id"));
        assert!(!metric.event_label.key.contains("amount"));
    }
}

#[test]
fn slo_alert_metric_uses_closed_sli_and_severity_labels() {
    let availability =
        plan_slo_alert_metric(SloAlertSli::Availability, SloAlertSeverity::Critical).unwrap();
    let latency =
        plan_slo_alert_metric(SloAlertSli::LatencyP95, SloAlertSeverity::Warning).unwrap();
    let error_rate =
        plan_slo_alert_metric(SloAlertSli::ErrorRate, SloAlertSeverity::Warning).unwrap();
    let quota =
        plan_slo_alert_metric(SloAlertSli::QuotaCorrectness, SloAlertSeverity::Critical).unwrap();
    let provider =
        plan_slo_alert_metric(SloAlertSli::ProviderDegradation, SloAlertSeverity::Warning).unwrap();
    let persistence =
        plan_slo_alert_metric(SloAlertSli::PersistenceFailure, SloAlertSeverity::Critical).unwrap();

    assert_eq!(availability.metric_name, "prodex_slo_alert_events_total");
    assert_eq!(availability.increment, 1);
    assert_eq!(
        availability.sli_label.as_metric_label().unwrap(),
        ("slo_sli", "availability")
    );
    assert_eq!(
        availability.severity_label.as_metric_label().unwrap(),
        ("slo_severity", "critical")
    );
    assert_eq!(
        latency.sli_label.as_metric_label().unwrap(),
        ("slo_sli", "latency_p95")
    );
    assert_eq!(
        latency.severity_label.as_metric_label().unwrap(),
        ("slo_severity", "warning")
    );
    assert_eq!(
        error_rate.sli_label.as_metric_label().unwrap(),
        ("slo_sli", "error_rate")
    );
    assert_eq!(
        quota.sli_label.as_metric_label().unwrap(),
        ("slo_sli", "quota_correctness")
    );
    assert_eq!(
        provider.sli_label.as_metric_label().unwrap(),
        ("slo_sli", "provider_degradation")
    );
    assert_eq!(
        persistence.sli_label.as_metric_label().unwrap(),
        ("slo_sli", "persistence_failure")
    );

    for metric in [
        availability,
        latency,
        error_rate,
        quota,
        provider,
        persistence,
    ] {
        assert!(!metric.sli_label.key.contains("tenant"));
        assert!(!metric.sli_label.key.contains("objective_name"));
        assert!(!metric.severity_label.key.contains("observed"));
        assert!(!metric.severity_label.key.contains("target"));
        assert!(!metric.severity_label.key.contains("request"));
    }
}

#[test]
fn shutdown_lifecycle_metric_uses_closed_event_and_result_labels() {
    let signal = plan_shutdown_lifecycle_metric(
        ShutdownLifecycleEvent::SignalReceived,
        ShutdownLifecycleResult::Success,
    )
    .unwrap();
    let draining = plan_shutdown_lifecycle_metric(
        ShutdownLifecycleEvent::DrainingStarted,
        ShutdownLifecycleResult::Success,
    )
    .unwrap();
    let readiness = plan_shutdown_lifecycle_metric(
        ShutdownLifecycleEvent::ReadinessDisabled,
        ShutdownLifecycleResult::Success,
    )
    .unwrap();
    let inflight = plan_shutdown_lifecycle_metric(
        ShutdownLifecycleEvent::InflightDrained,
        ShutdownLifecycleResult::Failed,
    )
    .unwrap();
    let timeout = plan_shutdown_lifecycle_metric(
        ShutdownLifecycleEvent::TimeoutElapsed,
        ShutdownLifecycleResult::Timeout,
    )
    .unwrap();
    let completed = plan_shutdown_lifecycle_metric(
        ShutdownLifecycleEvent::Completed,
        ShutdownLifecycleResult::Forced,
    )
    .unwrap();

    assert_eq!(signal.metric_name, "prodex_shutdown_lifecycle_total");
    assert_eq!(signal.increment, 1);
    assert_eq!(
        signal.event_label.as_metric_label().unwrap(),
        ("shutdown_event", "signal_received")
    );
    assert_eq!(
        signal.result_label.as_metric_label().unwrap(),
        ("shutdown_result", "success")
    );
    assert_eq!(
        draining.event_label.as_metric_label().unwrap(),
        ("shutdown_event", "draining_started")
    );
    assert_eq!(
        readiness.event_label.as_metric_label().unwrap(),
        ("shutdown_event", "readiness_disabled")
    );
    assert_eq!(
        inflight.event_label.as_metric_label().unwrap(),
        ("shutdown_event", "inflight_drained")
    );
    assert_eq!(
        inflight.result_label.as_metric_label().unwrap(),
        ("shutdown_result", "failed")
    );
    assert_eq!(
        timeout.event_label.as_metric_label().unwrap(),
        ("shutdown_event", "timeout_elapsed")
    );
    assert_eq!(
        timeout.result_label.as_metric_label().unwrap(),
        ("shutdown_result", "timeout")
    );
    assert_eq!(
        completed.event_label.as_metric_label().unwrap(),
        ("shutdown_event", "completed")
    );
    assert_eq!(
        completed.result_label.as_metric_label().unwrap(),
        ("shutdown_result", "forced")
    );
    for metric in [signal, draining, readiness, inflight, timeout, completed] {
        assert!(!metric.event_label.key.contains("pod"));
        assert!(!metric.event_label.key.contains("signal"));
        assert!(!metric.event_label.key.contains("request"));
        assert!(!metric.result_label.key.contains("tenant"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn health_probe_metric_uses_closed_probe_and_result_labels() {
    let live = plan_health_probe_metric(HealthProbeKind::Live, HealthProbeResult::Passing).unwrap();
    let ready =
        plan_health_probe_metric(HealthProbeKind::Ready, HealthProbeResult::Degraded).unwrap();
    let startup =
        plan_health_probe_metric(HealthProbeKind::Startup, HealthProbeResult::Failing).unwrap();
    let draining =
        plan_health_probe_metric(HealthProbeKind::Ready, HealthProbeResult::Draining).unwrap();

    assert_eq!(live.metric_name, "prodex_health_probe_results_total");
    assert_eq!(live.increment, 1);
    assert_eq!(
        live.probe_label.as_metric_label().unwrap(),
        ("health_probe", "live")
    );
    assert_eq!(
        live.result_label.as_metric_label().unwrap(),
        ("health_result", "passing")
    );
    assert_eq!(
        ready.probe_label.as_metric_label().unwrap(),
        ("health_probe", "ready")
    );
    assert_eq!(
        ready.result_label.as_metric_label().unwrap(),
        ("health_result", "degraded")
    );
    assert_eq!(
        startup.probe_label.as_metric_label().unwrap(),
        ("health_probe", "startup")
    );
    assert_eq!(
        startup.result_label.as_metric_label().unwrap(),
        ("health_result", "failing")
    );
    assert_eq!(
        draining.result_label.as_metric_label().unwrap(),
        ("health_result", "draining")
    );
    for metric in [live, ready, startup, draining] {
        assert!(!metric.probe_label.key.contains("path"));
        assert!(!metric.probe_label.key.contains("tenant"));
        assert!(!metric.result_label.key.contains("dependency"));
        assert!(!metric.result_label.key.contains("revision"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn secret_provider_metric_uses_closed_backend_operation_and_result_labels() {
    let file_read = plan_secret_provider_metric(
        SecretProviderBackend::File,
        SecretProviderOperation::Read,
        SecretProviderResult::Success,
    )
    .unwrap();
    let keyring_write = plan_secret_provider_metric(
        SecretProviderBackend::Keyring,
        SecretProviderOperation::Write,
        SecretProviderResult::Unsupported,
    )
    .unwrap();
    let external_delete = plan_secret_provider_metric(
        SecretProviderBackend::ExternalManager,
        SecretProviderOperation::Delete,
        SecretProviderResult::NotFound,
    )
    .unwrap();
    let revision_lookup = plan_secret_provider_metric(
        SecretProviderBackend::ExternalManager,
        SecretProviderOperation::RevisionLookup,
        SecretProviderResult::Failed,
    )
    .unwrap();

    assert_eq!(
        file_read.metric_name,
        "prodex_secret_provider_operations_total"
    );
    assert_eq!(file_read.increment, 1);
    assert_eq!(
        file_read.backend_label.as_metric_label().unwrap(),
        ("secret_backend", "file")
    );
    assert_eq!(
        file_read.operation_label.as_metric_label().unwrap(),
        ("secret_operation", "read")
    );
    assert_eq!(
        file_read.result_label.as_metric_label().unwrap(),
        ("secret_result", "success")
    );
    assert_eq!(
        keyring_write.backend_label.as_metric_label().unwrap(),
        ("secret_backend", "keyring")
    );
    assert_eq!(
        keyring_write.operation_label.as_metric_label().unwrap(),
        ("secret_operation", "write")
    );
    assert_eq!(
        keyring_write.result_label.as_metric_label().unwrap(),
        ("secret_result", "unsupported")
    );
    assert_eq!(
        external_delete.backend_label.as_metric_label().unwrap(),
        ("secret_backend", "external_manager")
    );
    assert_eq!(
        external_delete.operation_label.as_metric_label().unwrap(),
        ("secret_operation", "delete")
    );
    assert_eq!(
        external_delete.result_label.as_metric_label().unwrap(),
        ("secret_result", "not_found")
    );
    assert_eq!(
        revision_lookup.operation_label.as_metric_label().unwrap(),
        ("secret_operation", "revision_lookup")
    );
    assert_eq!(
        revision_lookup.result_label.as_metric_label().unwrap(),
        ("secret_result", "failed")
    );

    for metric in [file_read, keyring_write, external_delete, revision_lookup] {
        assert!(!metric.backend_label.key.contains("tenant"));
        assert!(!metric.backend_label.key.contains("secret_id"));
        assert!(!metric.operation_label.key.contains("path"));
        assert!(!metric.operation_label.key.contains("account"));
        assert!(!metric.result_label.key.contains("value"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn secret_rotation_metric_uses_closed_scope_and_result_labels() {
    let provider = plan_secret_rotation_metric(
        SecretRotationScope::ProviderCredential,
        SecretRotationResult::Success,
    )
    .unwrap();
    let oidc = plan_secret_rotation_metric(
        SecretRotationScope::OidcClient,
        SecretRotationResult::Failed,
    )
    .unwrap();
    let signing = plan_secret_rotation_metric(
        SecretRotationScope::SigningKey,
        SecretRotationResult::Skipped,
    )
    .unwrap();
    let storage = plan_secret_rotation_metric(
        SecretRotationScope::StorageCredential,
        SecretRotationResult::Rollback,
    )
    .unwrap();
    let webhook = plan_secret_rotation_metric(
        SecretRotationScope::WebhookSecret,
        SecretRotationResult::Success,
    )
    .unwrap();

    assert_eq!(provider.metric_name, "prodex_secret_rotation_events_total");
    assert_eq!(provider.increment, 1);
    assert_eq!(
        provider.scope_label.as_metric_label().unwrap(),
        ("secret_scope", "provider_credential")
    );
    assert_eq!(
        provider.result_label.as_metric_label().unwrap(),
        ("secret_rotation_result", "success")
    );
    assert_eq!(
        oidc.scope_label.as_metric_label().unwrap(),
        ("secret_scope", "oidc_client")
    );
    assert_eq!(
        oidc.result_label.as_metric_label().unwrap(),
        ("secret_rotation_result", "failed")
    );
    assert_eq!(
        signing.scope_label.as_metric_label().unwrap(),
        ("secret_scope", "signing_key")
    );
    assert_eq!(
        signing.result_label.as_metric_label().unwrap(),
        ("secret_rotation_result", "skipped")
    );
    assert_eq!(
        storage.scope_label.as_metric_label().unwrap(),
        ("secret_scope", "storage_credential")
    );
    assert_eq!(
        storage.result_label.as_metric_label().unwrap(),
        ("secret_rotation_result", "rollback")
    );
    assert_eq!(
        webhook.scope_label.as_metric_label().unwrap(),
        ("secret_scope", "webhook_secret")
    );
    for metric in [provider, oidc, signing, storage, webhook] {
        assert!(!metric.scope_label.key.contains("tenant"));
        assert!(!metric.scope_label.key.contains("secret_id"));
        assert!(!metric.scope_label.key.contains("path"));
        assert!(!metric.result_label.key.contains("value"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn backup_restore_metric_uses_closed_operation_and_result_labels() {
    let backup =
        plan_backup_restore_metric(BackupRestoreOperation::Backup, BackupRestoreResult::Success)
            .unwrap();
    let restore =
        plan_backup_restore_metric(BackupRestoreOperation::Restore, BackupRestoreResult::Failed)
            .unwrap();
    let verify =
        plan_backup_restore_metric(BackupRestoreOperation::Verify, BackupRestoreResult::Partial)
            .unwrap();
    let drill =
        plan_backup_restore_metric(BackupRestoreOperation::Drill, BackupRestoreResult::Skipped)
            .unwrap();

    assert_eq!(backup.metric_name, "prodex_backup_restore_events_total");
    assert_eq!(backup.increment, 1);
    assert_eq!(
        backup.operation_label.as_metric_label().unwrap(),
        ("backup_restore_operation", "backup")
    );
    assert_eq!(
        backup.result_label.as_metric_label().unwrap(),
        ("backup_restore_result", "success")
    );
    assert_eq!(
        restore.operation_label.as_metric_label().unwrap(),
        ("backup_restore_operation", "restore")
    );
    assert_eq!(
        restore.result_label.as_metric_label().unwrap(),
        ("backup_restore_result", "failed")
    );
    assert_eq!(
        verify.operation_label.as_metric_label().unwrap(),
        ("backup_restore_operation", "verify")
    );
    assert_eq!(
        verify.result_label.as_metric_label().unwrap(),
        ("backup_restore_result", "partial")
    );
    assert_eq!(
        drill.operation_label.as_metric_label().unwrap(),
        ("backup_restore_operation", "drill")
    );
    assert_eq!(
        drill.result_label.as_metric_label().unwrap(),
        ("backup_restore_result", "skipped")
    );
    for metric in [backup, restore, verify, drill] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("backup_id"));
        assert!(!metric.operation_label.key.contains("path"));
        assert!(!metric.result_label.key.contains("checksum"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn deployment_rollout_metric_uses_closed_operation_and_result_labels() {
    let apply = plan_deployment_rollout_metric(
        DeploymentRolloutOperation::Apply,
        DeploymentRolloutResult::Success,
    )
    .unwrap();
    let verify = plan_deployment_rollout_metric(
        DeploymentRolloutOperation::Verify,
        DeploymentRolloutResult::Degraded,
    )
    .unwrap();
    let promote = plan_deployment_rollout_metric(
        DeploymentRolloutOperation::Promote,
        DeploymentRolloutResult::Failed,
    )
    .unwrap();
    let rollback = plan_deployment_rollout_metric(
        DeploymentRolloutOperation::Rollback,
        DeploymentRolloutResult::Skipped,
    )
    .unwrap();

    assert_eq!(apply.metric_name, "prodex_deployment_rollout_events_total");
    assert_eq!(apply.increment, 1);
    assert_eq!(
        apply.operation_label.as_metric_label().unwrap(),
        ("deployment_rollout_operation", "apply")
    );
    assert_eq!(
        apply.result_label.as_metric_label().unwrap(),
        ("deployment_rollout_result", "success")
    );
    assert_eq!(
        verify.operation_label.as_metric_label().unwrap(),
        ("deployment_rollout_operation", "verify")
    );
    assert_eq!(
        verify.result_label.as_metric_label().unwrap(),
        ("deployment_rollout_result", "degraded")
    );
    assert_eq!(
        promote.operation_label.as_metric_label().unwrap(),
        ("deployment_rollout_operation", "promote")
    );
    assert_eq!(
        promote.result_label.as_metric_label().unwrap(),
        ("deployment_rollout_result", "failed")
    );
    assert_eq!(
        rollback.operation_label.as_metric_label().unwrap(),
        ("deployment_rollout_operation", "rollback")
    );
    assert_eq!(
        rollback.result_label.as_metric_label().unwrap(),
        ("deployment_rollout_result", "skipped")
    );
    for metric in [apply, verify, promote, rollback] {
        assert!(!metric.operation_label.key.contains("namespace"));
        assert!(!metric.operation_label.key.contains("pod"));
        assert!(!metric.operation_label.key.contains("image"));
        assert!(!metric.result_label.key.contains("revision"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn load_soak_metric_uses_closed_scenario_and_result_labels() {
    let load =
        plan_load_soak_metric(LoadSoakScenarioKind::Load, LoadSoakResult::Passed, 60_000).unwrap();
    let soak =
        plan_load_soak_metric(LoadSoakScenarioKind::Soak, LoadSoakResult::Failed, 600_000).unwrap();
    let spike = plan_load_soak_metric(LoadSoakScenarioKind::Spike, LoadSoakResult::Aborted, 15_000)
        .unwrap();
    let recovery = plan_load_soak_metric(
        LoadSoakScenarioKind::Recovery,
        LoadSoakResult::ThresholdBreached,
        120_000,
    )
    .unwrap();

    assert_eq!(
        load.event_count_metric_name,
        "prodex_load_soak_events_total"
    );
    assert_eq!(load.duration_metric_name, "prodex_load_soak_duration_ms");
    assert_eq!(load.increment, 1);
    assert_eq!(load.duration_ms, 60_000);
    assert_eq!(
        load.scenario_label.as_metric_label().unwrap(),
        ("load_soak_scenario", "load")
    );
    assert_eq!(
        load.result_label.as_metric_label().unwrap(),
        ("load_soak_result", "passed")
    );
    assert_eq!(
        soak.scenario_label.as_metric_label().unwrap(),
        ("load_soak_scenario", "soak")
    );
    assert_eq!(
        soak.result_label.as_metric_label().unwrap(),
        ("load_soak_result", "failed")
    );
    assert_eq!(
        spike.scenario_label.as_metric_label().unwrap(),
        ("load_soak_scenario", "spike")
    );
    assert_eq!(
        spike.result_label.as_metric_label().unwrap(),
        ("load_soak_result", "aborted")
    );
    assert_eq!(
        recovery.scenario_label.as_metric_label().unwrap(),
        ("load_soak_scenario", "recovery")
    );
    assert_eq!(
        recovery.result_label.as_metric_label().unwrap(),
        ("load_soak_result", "threshold_breached")
    );
    for metric in [load, soak, spike, recovery] {
        assert!(!metric.scenario_label.key.contains("tenant"));
        assert!(!metric.scenario_label.key.contains("profile"));
        assert!(!metric.scenario_label.key.contains("run_id"));
        assert!(!metric.result_label.key.contains("threshold_value"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn fault_injection_metric_uses_closed_target_and_result_labels() {
    let postgres = plan_fault_injection_metric(
        FaultInjectionTarget::Postgres,
        FaultInjectionResult::Injected,
    )
    .unwrap();
    let redis =
        plan_fault_injection_metric(FaultInjectionTarget::Redis, FaultInjectionResult::Recovered)
            .unwrap();
    let idp = plan_fault_injection_metric(FaultInjectionTarget::Idp, FaultInjectionResult::Failed)
        .unwrap();
    let provider = plan_fault_injection_metric(
        FaultInjectionTarget::Provider,
        FaultInjectionResult::Skipped,
    )
    .unwrap();

    assert_eq!(postgres.metric_name, "prodex_fault_injection_events_total");
    assert_eq!(postgres.increment, 1);
    assert_eq!(
        postgres.target_label.as_metric_label().unwrap(),
        ("fault_injection_target", "postgres")
    );
    assert_eq!(
        postgres.result_label.as_metric_label().unwrap(),
        ("fault_injection_result", "injected")
    );
    assert_eq!(
        redis.target_label.as_metric_label().unwrap(),
        ("fault_injection_target", "redis")
    );
    assert_eq!(
        redis.result_label.as_metric_label().unwrap(),
        ("fault_injection_result", "recovered")
    );
    assert_eq!(
        idp.target_label.as_metric_label().unwrap(),
        ("fault_injection_target", "idp")
    );
    assert_eq!(
        idp.result_label.as_metric_label().unwrap(),
        ("fault_injection_result", "failed")
    );
    assert_eq!(
        provider.target_label.as_metric_label().unwrap(),
        ("fault_injection_target", "provider")
    );
    assert_eq!(
        provider.result_label.as_metric_label().unwrap(),
        ("fault_injection_result", "skipped")
    );
    for metric in [postgres, redis, idp, provider] {
        assert!(!metric.target_label.key.contains("tenant"));
        assert!(!metric.target_label.key.contains("endpoint"));
        assert!(!metric.target_label.key.contains("provider_id"));
        assert!(!metric.result_label.key.contains("run_id"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn migration_lifecycle_metric_uses_closed_operation_and_result_labels() {
    let status = plan_migration_lifecycle_metric(
        MigrationLifecycleOperation::StatusCheck,
        MigrationLifecycleResult::Compatible,
    )
    .unwrap();
    let compatibility = plan_migration_lifecycle_metric(
        MigrationLifecycleOperation::CompatibilityCheck,
        MigrationLifecycleResult::Blocked,
    )
    .unwrap();
    let apply = plan_migration_lifecycle_metric(
        MigrationLifecycleOperation::Apply,
        MigrationLifecycleResult::Applied,
    )
    .unwrap();
    let rollback = plan_migration_lifecycle_metric(
        MigrationLifecycleOperation::Rollback,
        MigrationLifecycleResult::RolledBack,
    )
    .unwrap();
    let failed = plan_migration_lifecycle_metric(
        MigrationLifecycleOperation::Apply,
        MigrationLifecycleResult::Failed,
    )
    .unwrap();

    assert_eq!(
        status.metric_name,
        "prodex_migration_lifecycle_events_total"
    );
    assert_eq!(status.increment, 1);
    assert_eq!(
        status.operation_label.as_metric_label().unwrap(),
        ("migration_operation", "status_check")
    );
    assert_eq!(
        status.result_label.as_metric_label().unwrap(),
        ("migration_result", "compatible")
    );
    assert_eq!(
        compatibility.operation_label.as_metric_label().unwrap(),
        ("migration_operation", "compatibility_check")
    );
    assert_eq!(
        compatibility.result_label.as_metric_label().unwrap(),
        ("migration_result", "blocked")
    );
    assert_eq!(
        apply.operation_label.as_metric_label().unwrap(),
        ("migration_operation", "apply")
    );
    assert_eq!(
        apply.result_label.as_metric_label().unwrap(),
        ("migration_result", "applied")
    );
    assert_eq!(
        rollback.operation_label.as_metric_label().unwrap(),
        ("migration_operation", "rollback")
    );
    assert_eq!(
        rollback.result_label.as_metric_label().unwrap(),
        ("migration_result", "rolled_back")
    );
    assert_eq!(
        failed.result_label.as_metric_label().unwrap(),
        ("migration_result", "failed")
    );
    for metric in [status, compatibility, apply, rollback, failed] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("version"));
        assert!(!metric.operation_label.key.contains("lock_owner"));
        assert!(!metric.result_label.key.contains("endpoint"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}

#[test]
fn persistence_metric_uses_closed_operation_and_result_labels() {
    let read =
        plan_persistence_metric(PersistenceOperation::Read, PersistenceResult::Success).unwrap();
    let write =
        plan_persistence_metric(PersistenceOperation::Write, PersistenceResult::Conflict).unwrap();
    let commit =
        plan_persistence_metric(PersistenceOperation::Commit, PersistenceResult::Timeout).unwrap();
    let rollback = plan_persistence_metric(
        PersistenceOperation::Rollback,
        PersistenceResult::Unavailable,
    )
    .unwrap();
    let health =
        plan_persistence_metric(PersistenceOperation::HealthCheck, PersistenceResult::Failed)
            .unwrap();

    assert_eq!(read.metric_name, "prodex_persistence_operations_total");
    assert_eq!(read.increment, 1);
    assert_eq!(
        read.operation_label.as_metric_label().unwrap(),
        ("persistence_operation", "read")
    );
    assert_eq!(
        read.result_label.as_metric_label().unwrap(),
        ("persistence_result", "success")
    );
    assert_eq!(
        write.operation_label.as_metric_label().unwrap(),
        ("persistence_operation", "write")
    );
    assert_eq!(
        write.result_label.as_metric_label().unwrap(),
        ("persistence_result", "conflict")
    );
    assert_eq!(
        commit.operation_label.as_metric_label().unwrap(),
        ("persistence_operation", "commit")
    );
    assert_eq!(
        commit.result_label.as_metric_label().unwrap(),
        ("persistence_result", "timeout")
    );
    assert_eq!(
        rollback.operation_label.as_metric_label().unwrap(),
        ("persistence_operation", "rollback")
    );
    assert_eq!(
        rollback.result_label.as_metric_label().unwrap(),
        ("persistence_result", "unavailable")
    );
    assert_eq!(
        health.operation_label.as_metric_label().unwrap(),
        ("persistence_operation", "health_check")
    );
    assert_eq!(
        health.result_label.as_metric_label().unwrap(),
        ("persistence_result", "failed")
    );
    for metric in [read, write, commit, rollback, health] {
        assert!(!metric.operation_label.key.contains("tenant"));
        assert!(!metric.operation_label.key.contains("resource"));
        assert!(!metric.result_label.key.contains("storage_key"));
        assert!(!metric.result_label.key.contains("endpoint"));
        assert!(!metric.result_label.key.contains("error_text"));
    }
}
