use prodex_application::{
    ApplicationInspectionError, ApplicationInspectionRequest, ApplicationInspectionSource,
    plan_application_request_inspection,
};
use prodex_domain::{
    ContentLocation, DataClassification, DetectorId, DetectorRevisionId, FindingKind,
    InspectionCoverage, InspectionFinding, InspectionLimits, InspectionReasonCode, InspectionTag,
};

fn finding(kind: FindingKind, path: &str) -> InspectionFinding {
    InspectionFinding::new(
        kind,
        ContentLocation::new(path, 0, 16).unwrap(),
        9_500,
        DetectorId::new("presidio-v1").unwrap(),
    )
    .unwrap()
}

#[test]
fn application_inspection_combines_sources_monotonically() {
    let plan = plan_application_request_inspection(ApplicationInspectionRequest {
        sources: vec![
            ApplicationInspectionSource {
                coverage: InspectionCoverage::Full,
                findings: vec![finding(FindingKind::EmailAddress, "$.input[0].content")],
                tags: vec![InspectionTag::new("pii").unwrap()],
                reason_codes: vec![InspectionReasonCode::new("presidio.finding").unwrap()],
            },
            ApplicationInspectionSource {
                coverage: InspectionCoverage::Unsupported,
                findings: vec![finding(FindingKind::ApiKey, "$.tools[0].arguments")],
                tags: vec![InspectionTag::new("secret").unwrap()],
                reason_codes: vec![InspectionReasonCode::new("local.secret").unwrap()],
            },
        ],
        default_classification: DataClassification::Internal,
        trusted_label: Some(DataClassification::Confidential),
        prior_classification: Some(DataClassification::Internal),
        detector_revision: DetectorRevisionId::new("combined-v1").unwrap(),
        limits: InspectionLimits::default(),
    })
    .unwrap();

    assert_eq!(plan.result.coverage(), InspectionCoverage::Partial);
    assert_eq!(plan.result.classification(), DataClassification::Restricted);
    assert_eq!(plan.result.findings().len(), 2);
}

#[test]
fn application_inspection_is_unsupported_without_detector_evidence() {
    let plan = plan_application_request_inspection(ApplicationInspectionRequest {
        sources: Vec::new(),
        default_classification: DataClassification::Internal,
        trusted_label: None,
        prior_classification: Some(DataClassification::Confidential),
        detector_revision: DetectorRevisionId::new("combined-v1").unwrap(),
        limits: InspectionLimits::default(),
    })
    .unwrap();

    assert_eq!(plan.result.coverage(), InspectionCoverage::Unsupported);
    assert_eq!(
        plan.result.classification(),
        DataClassification::Confidential
    );
}

#[test]
fn application_inspection_rejects_unbounded_detector_sources() {
    let source = ApplicationInspectionSource {
        coverage: InspectionCoverage::Full,
        findings: Vec::new(),
        tags: Vec::new(),
        reason_codes: Vec::new(),
    };
    let limits = InspectionLimits::new(1, 1, 1, 1).unwrap();
    let error = plan_application_request_inspection(ApplicationInspectionRequest {
        sources: vec![source.clone(), source],
        default_classification: DataClassification::Internal,
        trusted_label: None,
        prior_classification: None,
        detector_revision: DetectorRevisionId::new("combined-v1").unwrap(),
        limits,
    })
    .unwrap_err();

    assert_eq!(error, ApplicationInspectionError::TooManyDetectors);
}
