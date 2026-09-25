use prodex_observability::{
    ApiAdmissionResult, ApiRouteKind, ApiStatusClass, InspectionCoverageClass,
    InspectionFindingCategory, InspectionMaskingAction, InspectionOutcome, InspectionStage,
    ProviderKind, ProviderResultClass, SecretProviderBackend, SecretProviderOperation,
    SecretProviderResult, plan_api_admission_metric, plan_api_red_metric, plan_inspection_metric,
    plan_provider_metric, plan_secret_provider_metric,
};

fn assert_safe_label(label: &prodex_observability::TelemetryAttribute) {
    let (key, value) = label
        .as_metric_label()
        .expect("planned labels must satisfy the observability metric-label boundary");
    assert!(!key.is_empty());
    assert!(!value.is_empty());
}

#[test]
fn api_metrics_are_bounded_and_low_cardinality() {
    let red = plan_api_red_metric(ApiRouteKind::Responses, ApiStatusClass::Success, 42)
        .expect("API RED metric should plan");
    assert_eq!(red.increment, 1);
    assert_eq!(red.duration_ms, 42);
    assert!(!red.request_count_metric_name.is_empty());
    assert!(!red.duration_metric_name.is_empty());
    assert_safe_label(&red.route_label);
    assert_safe_label(&red.status_label);

    let admission =
        plan_api_admission_metric(ApiRouteKind::Websocket, ApiAdmissionResult::QueueFull)
            .expect("API admission metric should plan");
    assert_eq!(admission.increment, 1);
    assert!(!admission.metric_name.is_empty());
    assert_safe_label(&admission.route_label);
    assert_safe_label(&admission.result_label);
}

#[test]
fn provider_and_secret_metrics_keep_only_typed_labels() {
    let provider =
        plan_provider_metric(ProviderKind::Gemini, ProviderResultClass::RateLimited, 123)
            .expect("provider metric should plan");
    assert_eq!(provider.increment, 1);
    assert_eq!(provider.duration_ms, 123);
    assert_safe_label(&provider.provider_label);
    assert_safe_label(&provider.result_label);

    let secret = plan_secret_provider_metric(
        SecretProviderBackend::Keyring,
        SecretProviderOperation::Read,
        SecretProviderResult::Success,
    )
    .expect("secret provider metric should plan");
    assert_eq!(secret.increment, 1);
    assert_safe_label(&secret.backend_label);
    assert_safe_label(&secret.operation_label);
    assert_safe_label(&secret.result_label);
}

#[test]
fn inspection_metric_clamps_duration_and_keeps_labels_valid() {
    let plan = plan_inspection_metric(
        InspectionStage::RequestEnforcement,
        InspectionCoverageClass::Partial,
        InspectionFindingCategory::Credential,
        InspectionMaskingAction::Masked,
        InspectionOutcome::Allowed,
        u64::MAX,
    )
    .expect("inspection metric should plan");

    assert_eq!(plan.increment, 1);
    assert_eq!(plan.duration_micros, 120_000_000);
    assert_safe_label(&plan.stage_label);
    assert_safe_label(&plan.coverage_label);
    assert_safe_label(&plan.finding_category_label);
    assert_safe_label(&plan.masking_action_label);
    assert_safe_label(&plan.outcome_label);
}

#[test]
fn planned_metric_debug_output_does_not_expose_raw_secret_material() {
    let plan = plan_secret_provider_metric(
        SecretProviderBackend::ExternalManager,
        SecretProviderOperation::RevisionLookup,
        SecretProviderResult::NotFound,
    )
    .expect("secret provider metric should plan");
    let rendered = format!("{plan:?}");
    assert!(!rendered.contains("raw-secret"));
    assert!(rendered.contains("<redacted>"));
}
