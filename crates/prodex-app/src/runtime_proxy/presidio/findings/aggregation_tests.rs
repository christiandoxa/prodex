use super::{
    ApplicationInspectionPlan, ApplicationInspectionSource, ContentLocation, DataClassification,
    DetectorId, DetectorRevisionId, FindingKind, InspectionCoverage, InspectionFinding,
    InspectionLimits, InspectionReasonCode, InspectionResult, InspectionTag,
    runtime_presidio_inspection_plan,
};

fn finding(kind: FindingKind) -> InspectionFinding {
    InspectionFinding::new(
        kind,
        ContentLocation::new("$.input", 0, 1).unwrap(),
        9000,
        DetectorId::new("fixture").unwrap(),
    )
    .unwrap()
}

fn source(
    coverage: InspectionCoverage,
    kinds: &[FindingKind],
    masked: &[FindingKind],
) -> ApplicationInspectionSource {
    ApplicationInspectionSource {
        coverage,
        findings: kinds.iter().copied().map(finding).collect(),
        masked_findings: masked.to_vec(),
        tags: vec![],
        reason_codes: vec![],
    }
}

#[test]
fn inspection_aggregation_keeps_empty_default_and_all_coverage_pairs() {
    let revision = DetectorRevisionId::new("fixture-revision").unwrap();
    for classification in [
        DataClassification::Public,
        DataClassification::Internal,
        DataClassification::Confidential,
        DataClassification::Restricted,
    ] {
        let plan = runtime_presidio_inspection_plan(vec![], classification, &revision).unwrap();
        assert_eq!(
            plan,
            ApplicationInspectionPlan {
                result: InspectionResult::new(
                    InspectionCoverage::Unsupported,
                    classification,
                    vec![],
                    vec![],
                    vec![],
                    revision.clone(),
                    InspectionLimits::default()
                )
                .unwrap(),
                masked_findings: vec![],
            }
        );
    }
    for (left, right, expected) in [
        (
            InspectionCoverage::Full,
            InspectionCoverage::Full,
            InspectionCoverage::Full,
        ),
        (
            InspectionCoverage::Full,
            InspectionCoverage::Partial,
            InspectionCoverage::Partial,
        ),
        (
            InspectionCoverage::Full,
            InspectionCoverage::Unsupported,
            InspectionCoverage::Partial,
        ),
        (
            InspectionCoverage::Partial,
            InspectionCoverage::Full,
            InspectionCoverage::Partial,
        ),
        (
            InspectionCoverage::Partial,
            InspectionCoverage::Partial,
            InspectionCoverage::Partial,
        ),
        (
            InspectionCoverage::Partial,
            InspectionCoverage::Unsupported,
            InspectionCoverage::Partial,
        ),
        (
            InspectionCoverage::Unsupported,
            InspectionCoverage::Full,
            InspectionCoverage::Partial,
        ),
        (
            InspectionCoverage::Unsupported,
            InspectionCoverage::Partial,
            InspectionCoverage::Partial,
        ),
        (
            InspectionCoverage::Unsupported,
            InspectionCoverage::Unsupported,
            InspectionCoverage::Unsupported,
        ),
    ] {
        let plan = runtime_presidio_inspection_plan(
            vec![source(left, &[], &[]), source(right, &[], &[])],
            DataClassification::Internal,
            &revision,
        )
        .unwrap();
        assert_eq!(plan.result.coverage(), expected);
        assert_eq!(plan.result.classification(), DataClassification::Internal);
    }
}

#[test]
fn inspection_aggregation_intersects_masks_with_each_source_before_union() {
    let revision = DetectorRevisionId::new("fixture-revision").unwrap();
    let plan = runtime_presidio_inspection_plan(
        vec![
            source(
                InspectionCoverage::Full,
                &[FindingKind::EmailAddress],
                &[FindingKind::ApiKey],
            ),
            source(
                InspectionCoverage::Unsupported,
                &[FindingKind::ApiKey],
                &[FindingKind::EmailAddress],
            ),
        ],
        DataClassification::Public,
        &revision,
    )
    .unwrap();
    assert_eq!(
        plan,
        ApplicationInspectionPlan {
            result: InspectionResult::new(
                InspectionCoverage::Partial,
                DataClassification::Restricted,
                vec![
                    finding(FindingKind::EmailAddress),
                    finding(FindingKind::ApiKey)
                ],
                vec![],
                vec![],
                revision,
                InspectionLimits::default()
            )
            .unwrap(),
            masked_findings: vec![],
        }
    );
}

#[test]
fn inspection_aggregation_keeps_all_kinds_order_labels_and_selected_revision() {
    let revision = DetectorRevisionId::new("selected-revision").unwrap();
    let reversed = FindingKind::ALL.into_iter().rev().collect::<Vec<_>>();
    let mut first = source(InspectionCoverage::Full, &reversed, &reversed);
    first.tags = vec![
        InspectionTag::new("z-tag").unwrap(),
        InspectionTag::new("a-tag").unwrap(),
    ];
    first.reason_codes = vec![InspectionReasonCode::new("z-reason").unwrap()];
    let mut second = source(
        InspectionCoverage::Full,
        &[FindingKind::EmailAddress],
        &[FindingKind::EmailAddress, FindingKind::EmailAddress],
    );
    second.tags = vec![InspectionTag::new("a-tag").unwrap()];
    second.reason_codes = vec![InspectionReasonCode::new("a-reason").unwrap()];
    let mut expected_findings = first.findings.clone();
    expected_findings.extend(second.findings.clone());
    let plan = runtime_presidio_inspection_plan(
        vec![first, second],
        DataClassification::Public,
        &revision,
    )
    .unwrap();
    assert_eq!(
        plan,
        ApplicationInspectionPlan {
            result: InspectionResult::new(
                InspectionCoverage::Full,
                DataClassification::Restricted,
                expected_findings,
                vec![
                    InspectionTag::new("a-tag").unwrap(),
                    InspectionTag::new("z-tag").unwrap()
                ],
                vec![
                    InspectionReasonCode::new("a-reason").unwrap(),
                    InspectionReasonCode::new("z-reason").unwrap()
                ],
                revision,
                InspectionLimits::default()
            )
            .unwrap(),
            masked_findings: FindingKind::ALL.to_vec(),
        }
    );
}

#[test]
fn inspection_aggregation_preserves_classification_minimum_and_higher_defaults() {
    let revision = DetectorRevisionId::new("fixture-revision").unwrap();
    for (kind, minimum) in [
        (FindingKind::EmailAddress, DataClassification::Confidential),
        (FindingKind::PhoneNumber, DataClassification::Confidential),
        (FindingKind::PersonName, DataClassification::Confidential),
        (
            FindingKind::PhysicalAddress,
            DataClassification::Confidential,
        ),
        (FindingKind::GovernmentId, DataClassification::Restricted),
        (
            FindingKind::FinancialAccount,
            DataClassification::Restricted,
        ),
        (FindingKind::PaymentCard, DataClassification::Restricted),
        (FindingKind::AccessToken, DataClassification::Restricted),
        (FindingKind::ApiKey, DataClassification::Restricted),
        (FindingKind::PrivateKey, DataClassification::Restricted),
        (FindingKind::Password, DataClassification::Restricted),
        (
            FindingKind::TenantSensitive,
            DataClassification::Confidential,
        ),
    ] {
        let plan = runtime_presidio_inspection_plan(
            vec![source(InspectionCoverage::Full, &[kind], &[kind])],
            DataClassification::Public,
            &revision,
        )
        .unwrap();
        assert_eq!(plan.result.classification(), minimum);
        assert_eq!(plan.masked_findings, vec![kind]);
        let higher = runtime_presidio_inspection_plan(
            vec![source(InspectionCoverage::Full, &[kind], &[])],
            DataClassification::Restricted,
            &revision,
        )
        .unwrap();
        assert_eq!(
            higher.result.classification(),
            DataClassification::Restricted
        );
    }
}

#[test]
fn inspection_aggregation_preserves_source_mask_and_final_count_error_precedence() {
    let revision = DetectorRevisionId::new("fixture-revision").unwrap();
    let over_mask = source(InspectionCoverage::Full, &[], &[FindingKind::ApiKey; 13]);
    let too_many_sources = runtime_presidio_inspection_plan(
        vec![over_mask.clone(); 9],
        DataClassification::Public,
        &revision,
    )
    .unwrap_err();
    assert_eq!(too_many_sources.to_string(), "too many inspection sources");
    let over_findings = source(InspectionCoverage::Full, &[FindingKind::ApiKey; 257], &[]);
    let mask_first = runtime_presidio_inspection_plan(
        vec![over_findings.clone(), over_mask],
        DataClassification::Public,
        &revision,
    )
    .unwrap_err();
    assert_eq!(mask_first.to_string(), "too many masked finding kinds");
    let count_error = runtime_presidio_inspection_plan(
        vec![over_findings],
        DataClassification::Public,
        &revision,
    )
    .unwrap_err();
    assert_eq!(count_error.to_string(), "failed to build inspection result");
    assert!(
        count_error
            .downcast_ref::<prodex_domain::InspectionModelError>()
            .is_some()
    );
}

#[test]
fn inspection_aggregation_keeps_domain_errors_for_oversized_finding_and_label_counts() {
    let revision = DetectorRevisionId::new("fixture-revision").unwrap();
    for oversized in [
        ApplicationInspectionSource {
            findings: vec![finding(FindingKind::ApiKey); 65_537],
            ..source(InspectionCoverage::Full, &[], &[])
        },
        ApplicationInspectionSource {
            tags: vec![InspectionTag::new("fixture-tag").unwrap(); 33],
            ..source(InspectionCoverage::Full, &[], &[])
        },
        ApplicationInspectionSource {
            reason_codes: vec![InspectionReasonCode::new("fixture-reason").unwrap(); 33],
            ..source(InspectionCoverage::Full, &[], &[])
        },
    ] {
        let error = runtime_presidio_inspection_plan(
            vec![oversized],
            DataClassification::Public,
            &revision,
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "failed to build inspection result");
        assert!(
            error
                .downcast_ref::<prodex_domain::InspectionModelError>()
                .is_some()
        );
    }
}
