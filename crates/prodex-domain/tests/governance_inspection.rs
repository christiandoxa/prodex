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

fn finding_at(
    kind: FindingKind,
    path: &str,
    start_byte: usize,
    end_byte: usize,
    confidence_basis_points: u16,
    detector_id: &str,
) -> InspectionFinding {
    InspectionFinding::new(
        kind,
        ContentLocation::new(path, start_byte, end_byte).unwrap(),
        confidence_basis_points,
        DetectorId::new(detector_id).unwrap(),
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
fn inspection_result_ordering_uses_mojo_key_and_deduplicates() {
    let result = InspectionResult::new(
        InspectionCoverage::Full,
        DataClassification::Restricted,
        vec![
            finding_at(FindingKind::ApiKey, "$.input[1]", 1, 2, 9_000, "detector.b"),
            finding_at(FindingKind::ApiKey, "$.input[0]", 2, 5, 9_000, "detector.b"),
            finding_at(
                FindingKind::EmailAddress,
                "$.input[0]",
                2,
                5,
                8_000,
                "detector.a",
            ),
            finding_at(FindingKind::ApiKey, "$.input[0]", 2, 5, 9_000, "detector.a"),
            finding_at(FindingKind::ApiKey, "$.input[0]", 2, 5, 7_000, "detector.a"),
        ],
        vec![
            InspectionTag::new("secret").unwrap(),
            InspectionTag::new("alpha").unwrap(),
            InspectionTag::new("secret").unwrap(),
            InspectionTag::new("beta").unwrap(),
            InspectionTag::new("alpha").unwrap(),
        ],
        vec![
            InspectionReasonCode::new("zeta").unwrap(),
            InspectionReasonCode::new("alpha").unwrap(),
            InspectionReasonCode::new("zeta").unwrap(),
        ],
        DetectorRevisionId::new("detectors-v1").unwrap(),
        InspectionLimits::default(),
    )
    .unwrap();

    let findings = result
        .findings()
        .iter()
        .map(|finding| {
            (
                finding.location().field_path(),
                finding.kind(),
                finding.detector_id().as_str(),
                finding.confidence_basis_points(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        findings,
        vec![
            ("$.input[0]", FindingKind::EmailAddress, "detector.a", 8_000),
            ("$.input[0]", FindingKind::ApiKey, "detector.a", 7_000),
            ("$.input[0]", FindingKind::ApiKey, "detector.a", 9_000),
            ("$.input[0]", FindingKind::ApiKey, "detector.b", 9_000),
            ("$.input[1]", FindingKind::ApiKey, "detector.b", 9_000),
        ]
    );
    assert_eq!(
        result
            .tags()
            .iter()
            .map(InspectionTag::as_str)
            .collect::<Vec<_>>(),
        ["alpha", "beta", "secret"]
    );
    assert_eq!(
        result
            .reason_codes()
            .iter()
            .map(InspectionReasonCode::as_str)
            .collect::<Vec<_>>(),
        ["alpha", "zeta"]
    );
}

#[test]
fn governance_order_plan_returns_sorted_original_indices() {
    let findings = [
        prodex_mojo_core::policy::GovernanceInspectionFindingOrderKey {
            field_path: "$.input[1]",
            start_byte: 4,
            end_byte: 20,
            kind: FindingKind::ApiKey as u8,
            detector_id: "detector.b",
            confidence_basis_points: 9_000,
        },
        prodex_mojo_core::policy::GovernanceInspectionFindingOrderKey {
            field_path: "$.input[0]",
            start_byte: 4,
            end_byte: 20,
            kind: FindingKind::EmailAddress as u8,
            detector_id: "detector.a",
            confidence_basis_points: 9_000,
        },
    ];
    let tags = ["zeta", "alpha"];
    let reason_codes = ["reason.z", "reason.a"];
    let order =
        prodex_mojo_core::policy::governance_inspection_order(&findings, &tags, &reason_codes)
            .unwrap();

    assert_eq!(order.finding_indices, [1, 0]);
    assert_eq!(order.tag_indices, [1, 0]);
    assert_eq!(order.reason_code_indices, [1, 0]);
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
#[test]
fn finding_minimum_classifications_follow_the_mojo_policy() {
    let expected = [
        DataClassification::Confidential,
        DataClassification::Confidential,
        DataClassification::Confidential,
        DataClassification::Confidential,
        DataClassification::Restricted,
        DataClassification::Restricted,
        DataClassification::Restricted,
        DataClassification::Restricted,
        DataClassification::Restricted,
        DataClassification::Restricted,
        DataClassification::Restricted,
        DataClassification::Confidential,
    ];
    for (kind, expected) in FindingKind::ALL.into_iter().zip(expected) {
        assert_eq!(kind.minimum_classification(), expected, "kind={kind:?}");
    }
}

#[test]
fn governance_labels_and_validation_boundaries_stay_stable() {
    assert_eq!(DataClassification::Public.as_str(), "public");
    assert_eq!(DataClassification::Restricted.as_str(), "restricted");
    assert_eq!(InspectionCoverage::Full.as_str(), "full");
    assert_eq!(InspectionCoverage::Unsupported.as_str(), "unsupported");

    assert!(ContentLocation::new("$.input[0].content", 0, 1).is_ok());
    assert_eq!(
        ContentLocation::new("$.bad value", 0, 1),
        Err(InspectionModelError::InvalidLocation)
    );
    assert_eq!(
        ContentLocation::new("", 0, 1),
        Err(InspectionModelError::InvalidLocation)
    );
    assert!(DetectorId::new("detector.v1:local/foo").is_ok());
    assert_eq!(
        DetectorId::new("detector secret"),
        Err(InspectionModelError::InvalidToken)
    );
    assert!(InspectionLimits::new(8, 256, 32, 32).is_ok());
    assert_eq!(
        InspectionLimits::new(9, 256, 32, 32),
        Err(InspectionModelError::InvalidLimits)
    );
}
