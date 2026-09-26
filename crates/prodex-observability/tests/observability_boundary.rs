use prodex_observability::{
    ApiAdmissionResult, ApiRouteKind, ApiStatusClass, InspectionCoverageClass,
    InspectionFindingCategory, InspectionMaskingAction, InspectionOutcome, InspectionStage,
    ProviderKind, ProviderResultClass, SecretProviderBackend, SecretProviderOperation,
    SecretProviderResult, TelemetryAttribute, plan_api_admission_metric, plan_api_red_metric,
    plan_inspection_metric, plan_provider_metric, plan_secret_provider_metric,
};

fn assert_label(label: &TelemetryAttribute, key: &str, value: &str) {
    assert_eq!(label.as_metric_label(), Ok((key, value)));
}

#[test]
fn api_metric_names_and_labels_match_the_public_contract() {
    let routes = [
        (ApiRouteKind::Responses, "responses"),
        (ApiRouteKind::Compact, "compact"),
        (ApiRouteKind::Websocket, "websocket"),
        (ApiRouteKind::ControlPlane, "control_plane"),
        (ApiRouteKind::Health, "health"),
    ];

    for &(route, route_value) in &routes {
        let red = plan_api_red_metric(route, ApiStatusClass::Success, 42)
            .expect("API RED metric should plan");
        assert_eq!(red.request_count_metric_name, "prodex_api_requests_total");
        assert_eq!(red.duration_metric_name, "prodex_api_request_duration_ms");
        assert_eq!(red.increment, 1);
        assert_eq!(red.duration_ms, 42);
        assert_label(&red.route_label, "api_route", route_value);
        assert_label(&red.status_label, "status_class", "2xx");

        let admission = plan_api_admission_metric(route, ApiAdmissionResult::Accepted)
            .expect("API admission metric should plan");
        assert_eq!(
            admission.metric_name,
            "prodex_api_admission_decisions_total"
        );
        assert_eq!(admission.increment, 1);
        assert_label(&admission.route_label, "api_admission_route", route_value);
        assert_label(&admission.result_label, "api_admission_result", "accepted");
    }

    for (status, value) in [
        (ApiStatusClass::Informational, "1xx"),
        (ApiStatusClass::Success, "2xx"),
        (ApiStatusClass::Redirection, "3xx"),
        (ApiStatusClass::ClientError, "4xx"),
        (ApiStatusClass::ServerError, "5xx"),
    ] {
        let metric = plan_api_red_metric(ApiRouteKind::Responses, status, 0)
            .expect("API RED metric should plan");
        assert_label(&metric.status_label, "status_class", value);
    }

    for (result, value) in [
        (ApiAdmissionResult::Accepted, "accepted"),
        (
            ApiAdmissionResult::GlobalLimitReached,
            "global_limit_reached",
        ),
        (ApiAdmissionResult::RouteLimitReached, "route_limit_reached"),
        (ApiAdmissionResult::QueueFull, "queue_full"),
        (ApiAdmissionResult::Draining, "draining"),
    ] {
        let metric = plan_api_admission_metric(ApiRouteKind::Responses, result)
            .expect("API admission metric should plan");
        assert_label(&metric.result_label, "api_admission_result", value);
    }
}

#[test]
fn provider_metric_names_and_labels_match_the_public_contract() {
    for (provider, value) in [
        (ProviderKind::OpenAi, "openai"),
        (ProviderKind::Anthropic, "anthropic"),
        (ProviderKind::Gemini, "gemini"),
        (ProviderKind::Local, "local"),
        (ProviderKind::Other, "other"),
    ] {
        let metric = plan_provider_metric(provider, ProviderResultClass::RateLimited, 123)
            .expect("provider metric should plan");
        assert_eq!(
            metric.request_count_metric_name,
            "prodex_provider_requests_total"
        );
        assert_eq!(
            metric.duration_metric_name,
            "prodex_provider_request_duration_ms"
        );
        assert_eq!(metric.increment, 1);
        assert_eq!(metric.duration_ms, 123);
        assert_label(&metric.provider_label, "provider", value);
        assert_label(&metric.result_label, "provider_result", "rate_limited");
    }

    for (result, value) in [
        (ProviderResultClass::Success, "success"),
        (ProviderResultClass::RateLimited, "rate_limited"),
        (ProviderResultClass::Overloaded, "overloaded"),
        (ProviderResultClass::ProviderError, "provider_error"),
        (ProviderResultClass::TransportError, "transport_error"),
    ] {
        let metric = plan_provider_metric(ProviderKind::Gemini, result, 0)
            .expect("provider metric should plan");
        assert_label(&metric.result_label, "provider_result", value);
    }
}

#[test]
fn secret_provider_metric_names_and_labels_match_the_public_contract() {
    for (backend, value) in [
        (SecretProviderBackend::File, "file"),
        (SecretProviderBackend::Keyring, "keyring"),
        (SecretProviderBackend::ExternalManager, "external_manager"),
    ] {
        let metric = plan_secret_provider_metric(
            backend,
            SecretProviderOperation::Read,
            SecretProviderResult::Success,
        )
        .expect("secret-provider metric should plan");
        assert_eq!(
            metric.metric_name,
            "prodex_secret_provider_operations_total"
        );
        assert_eq!(metric.increment, 1);
        assert_label(&metric.backend_label, "secret_backend", value);
        assert_label(&metric.operation_label, "secret_operation", "read");
        assert_label(&metric.result_label, "secret_result", "success");
    }

    for (operation, value) in [
        (SecretProviderOperation::Read, "read"),
        (SecretProviderOperation::Write, "write"),
        (SecretProviderOperation::Delete, "delete"),
        (SecretProviderOperation::RevisionLookup, "revision_lookup"),
    ] {
        let metric = plan_secret_provider_metric(
            SecretProviderBackend::Keyring,
            operation,
            SecretProviderResult::Success,
        )
        .expect("secret-provider metric should plan");
        assert_label(&metric.operation_label, "secret_operation", value);
    }

    for (result, value) in [
        (SecretProviderResult::Success, "success"),
        (SecretProviderResult::NotFound, "not_found"),
        (SecretProviderResult::Unsupported, "unsupported"),
        (SecretProviderResult::Failed, "failed"),
    ] {
        let metric = plan_secret_provider_metric(
            SecretProviderBackend::Keyring,
            SecretProviderOperation::Read,
            result,
        )
        .expect("secret-provider metric should plan");
        assert_label(&metric.result_label, "secret_result", value);
    }
}

#[test]
fn inspection_metric_names_and_labels_match_the_public_contract() {
    let metric = plan_inspection_metric(
        InspectionStage::RequestEnforcement,
        InspectionCoverageClass::Partial,
        InspectionFindingCategory::Credential,
        InspectionMaskingAction::Masked,
        InspectionOutcome::Allowed,
        u64::MAX,
    )
    .expect("inspection metric should plan");
    assert_eq!(metric.event_metric_name, "prodex_inspection_events_total");
    assert_eq!(
        metric.duration_metric_name,
        "prodex_inspection_duration_microseconds"
    );
    assert_eq!(metric.increment, 1);
    assert_eq!(metric.duration_micros, 120_000_000);
    assert_label(
        &metric.stage_label,
        "inspection_stage",
        "request_enforcement",
    );
    assert_label(&metric.coverage_label, "inspection_coverage", "partial");
    assert_label(
        &metric.finding_category_label,
        "inspection_finding_category",
        "credential",
    );
    assert_label(
        &metric.masking_action_label,
        "inspection_masking_action",
        "masked",
    );
    assert_label(&metric.outcome_label, "inspection_outcome", "allowed");

    for (stage, value) in [
        (InspectionStage::Local, "local"),
        (InspectionStage::External, "external"),
        (InspectionStage::Merge, "merge"),
        (InspectionStage::RequestEnforcement, "request_enforcement"),
        (InspectionStage::ResponseEnforcement, "response_enforcement"),
    ] {
        let metric = plan_inspection_metric(
            stage,
            InspectionCoverageClass::Partial,
            InspectionFindingCategory::Credential,
            InspectionMaskingAction::Masked,
            InspectionOutcome::Allowed,
            0,
        )
        .expect("inspection metric should plan");
        assert_label(&metric.stage_label, "inspection_stage", value);
    }

    for (coverage, value) in [
        (InspectionCoverageClass::Full, "full"),
        (InspectionCoverageClass::Partial, "partial"),
        (InspectionCoverageClass::Unsupported, "unsupported"),
    ] {
        let metric = plan_inspection_metric(
            InspectionStage::Local,
            coverage,
            InspectionFindingCategory::Credential,
            InspectionMaskingAction::Masked,
            InspectionOutcome::Allowed,
            0,
        )
        .expect("inspection metric should plan");
        assert_label(&metric.coverage_label, "inspection_coverage", value);
    }

    for (category, value) in [
        (InspectionFindingCategory::None, "none"),
        (InspectionFindingCategory::PersonalData, "personal_data"),
        (InspectionFindingCategory::Credential, "credential"),
        (InspectionFindingCategory::Financial, "financial"),
        (InspectionFindingCategory::Multiple, "multiple"),
    ] {
        let metric = plan_inspection_metric(
            InspectionStage::Local,
            InspectionCoverageClass::Partial,
            category,
            InspectionMaskingAction::Masked,
            InspectionOutcome::Allowed,
            0,
        )
        .expect("inspection metric should plan");
        assert_label(
            &metric.finding_category_label,
            "inspection_finding_category",
            value,
        );
    }

    for (action, value) in [
        (InspectionMaskingAction::None, "none"),
        (InspectionMaskingAction::Masked, "masked"),
        (InspectionMaskingAction::Denied, "denied"),
    ] {
        let metric = plan_inspection_metric(
            InspectionStage::Local,
            InspectionCoverageClass::Partial,
            InspectionFindingCategory::Credential,
            action,
            InspectionOutcome::Allowed,
            0,
        )
        .expect("inspection metric should plan");
        assert_label(
            &metric.masking_action_label,
            "inspection_masking_action",
            value,
        );
    }

    for (outcome, value) in [
        (InspectionOutcome::Allowed, "allowed"),
        (InspectionOutcome::Denied, "denied"),
        (InspectionOutcome::Timeout, "timeout"),
        (InspectionOutcome::Error, "error"),
    ] {
        let metric = plan_inspection_metric(
            InspectionStage::Local,
            InspectionCoverageClass::Partial,
            InspectionFindingCategory::Credential,
            InspectionMaskingAction::Masked,
            outcome,
            0,
        )
        .expect("inspection metric should plan");
        assert_label(&metric.outcome_label, "inspection_outcome", value);
    }
}

#[test]
fn planned_metric_debug_output_redacts_secret_material() {
    let plan = plan_secret_provider_metric(
        SecretProviderBackend::ExternalManager,
        SecretProviderOperation::RevisionLookup,
        SecretProviderResult::NotFound,
    )
    .expect("secret-provider metric should plan");
    let rendered = format!("{plan:?}");
    assert!(!rendered.contains("raw-secret"));
    assert!(!rendered.contains("external_manager"));
    assert!(!rendered.contains("revision_lookup"));
    assert!(!rendered.contains("not_found"));
    assert!(rendered.contains("<redacted>"));
}
