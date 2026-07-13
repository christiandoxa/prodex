use prodex_domain::{
    ContentLocation, DataClassification, DetectorId, DetectorRevisionId, FindingKind,
    InspectionCoverage, InspectionFinding, InspectionLimits, InspectionModelError,
    InspectionReasonCode, InspectionResult, InspectionTag,
};

fn finding(kind: FindingKind, path: &str) -> InspectionFinding {
    InspectionFinding::new(
        kind,
        ContentLocation::new(path, 4, 20).unwrap(),
        9_900,
        DetectorId::new("local.secret-v1").unwrap(),
    )
    .unwrap()
}

#[test]
fn inspection_result_is_bounded_deterministic_and_content_free() {
    let result = InspectionResult::new(
        InspectionCoverage::Full,
        DataClassification::Restricted,
        vec![
            finding(FindingKind::ApiKey, "$.input[1].content"),
            finding(
                FindingKind::EmailAddress,
                "$.tools[0].function.arguments.*.content",
            ),
        ],
        vec![
            InspectionTag::new("secret").unwrap(),
            InspectionTag::new("secret").unwrap(),
        ],
        vec![InspectionReasonCode::new("detector.finding").unwrap()],
        DetectorRevisionId::new("detectors-2026-07-13").unwrap(),
        InspectionLimits::default(),
    )
    .unwrap();

    assert_eq!(result.classification(), DataClassification::Restricted);
    assert_eq!(result.findings().len(), 2);
    assert_eq!(
        result.findings()[0].location().field_path(),
        "$.input[1].content"
    );
    assert_eq!(result.tags().len(), 1);
    let debug = format!("{result:?}");
    assert!(!debug.contains("$.input"));
    assert!(!debug.contains("local.secret-v1"));
}

#[test]
fn inspection_result_rejects_weak_classification_and_excess_findings() {
    let error = InspectionResult::new(
        InspectionCoverage::Full,
        DataClassification::Confidential,
        vec![finding(FindingKind::PrivateKey, "$.input")],
        Vec::new(),
        Vec::new(),
        DetectorRevisionId::new("detectors-v1").unwrap(),
        InspectionLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error, InspectionModelError::ClassificationTooLow);

    let limits = InspectionLimits::new(1, 1, 1, 1).unwrap();
    let error = InspectionResult::new(
        InspectionCoverage::Partial,
        DataClassification::Restricted,
        vec![
            finding(FindingKind::ApiKey, "$.input[0]"),
            finding(FindingKind::ApiKey, "$.input[1]"),
        ],
        Vec::new(),
        Vec::new(),
        DetectorRevisionId::new("detectors-v1").unwrap(),
        limits,
    )
    .unwrap_err();
    assert_eq!(error, InspectionModelError::LimitExceeded);
}

#[test]
fn classification_and_coverage_only_move_conservatively() {
    assert_eq!(
        DataClassification::Internal.raised_to(DataClassification::Restricted),
        DataClassification::Restricted
    );
    assert_eq!(
        InspectionCoverage::Full.combine(InspectionCoverage::Unsupported),
        InspectionCoverage::Partial
    );
    assert_eq!(
        InspectionCoverage::Unsupported.combine(InspectionCoverage::Unsupported),
        InspectionCoverage::Unsupported
    );
}
