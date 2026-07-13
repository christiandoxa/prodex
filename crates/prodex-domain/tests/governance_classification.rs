use prodex_domain::{
    ClassificationRequest, ClassificationRule, ClassificationRuleSet,
    ClassificationRuleSetChecksum, ClassificationRuleSetRevisionId, ContentLocation,
    DataClassification, DetectorId, DetectorRevisionId, FindingKind, InspectionCoverage,
    InspectionFinding, InspectionLimits, InspectionResult, classify_inspection,
    compile_classification_rule_set,
};

fn inspection(coverage: InspectionCoverage) -> InspectionResult {
    InspectionResult::new(
        coverage,
        DataClassification::Confidential,
        vec![
            InspectionFinding::new(
                FindingKind::EmailAddress,
                ContentLocation::new("$.input", 0, 4).unwrap(),
                9_000,
                DetectorId::new("fixture.detector").unwrap(),
            )
            .unwrap(),
        ],
        Vec::new(),
        Vec::new(),
        DetectorRevisionId::new("detectors-v1").unwrap(),
        InspectionLimits::default(),
    )
    .unwrap()
}

fn rules(rules: Vec<ClassificationRule>) -> prodex_domain::CompiledClassificationRuleSet {
    compile_classification_rule_set(ClassificationRuleSet {
        revision: ClassificationRuleSetRevisionId::new("classification-v1").unwrap(),
        checksum: ClassificationRuleSetChecksum::new("sha256:fixture").unwrap(),
        unsupported_coverage_floor: DataClassification::Restricted,
        rules,
    })
    .unwrap()
}

#[test]
fn classification_is_deterministic_and_monotonic() {
    let compiled = rules(vec![ClassificationRule {
        finding_kind: FindingKind::EmailAddress,
        classification: DataClassification::Restricted,
    }]);
    let inspected = inspection(InspectionCoverage::Full);
    let request = ClassificationRequest {
        inspection: &inspected,
        trusted_label: Some(DataClassification::Public),
        untrusted_label: Some(DataClassification::Internal),
        prior_classification: Some(DataClassification::Confidential),
        session_floor: DataClassification::Internal,
        route_floor: DataClassification::Public,
        request_risk_floor: DataClassification::Internal,
    };

    let first = classify_inspection(&compiled, request).unwrap();
    let second = classify_inspection(&compiled, request).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.classification(), DataClassification::Restricted);
    assert_eq!(first.coverage(), InspectionCoverage::Full);
}

#[test]
fn incomplete_coverage_applies_configured_floor() {
    let compiled = rules(Vec::new());
    let inspected = inspection(InspectionCoverage::Unsupported);
    let decision = classify_inspection(
        &compiled,
        ClassificationRequest {
            inspection: &inspected,
            trusted_label: None,
            untrusted_label: None,
            prior_classification: None,
            session_floor: DataClassification::Public,
            route_floor: DataClassification::Public,
            request_risk_floor: DataClassification::Public,
        },
    )
    .unwrap();

    assert_eq!(decision.classification(), DataClassification::Restricted);
    assert!(
        decision
            .reason_codes()
            .iter()
            .any(|reason| reason.as_str() == "coverage.unsupported")
    );
}

#[test]
fn rule_publication_rejects_duplicate_or_weakened_rules() {
    let duplicate = compile_classification_rule_set(ClassificationRuleSet {
        revision: ClassificationRuleSetRevisionId::new("classification-v1").unwrap(),
        checksum: ClassificationRuleSetChecksum::new("sha256:fixture").unwrap(),
        unsupported_coverage_floor: DataClassification::Internal,
        rules: vec![
            ClassificationRule {
                finding_kind: FindingKind::EmailAddress,
                classification: DataClassification::Confidential,
            },
            ClassificationRule {
                finding_kind: FindingKind::EmailAddress,
                classification: DataClassification::Restricted,
            },
        ],
    })
    .unwrap_err();
    assert_eq!(duplicate, prodex_domain::ClassificationError::DuplicateRule);

    let weakened = compile_classification_rule_set(ClassificationRuleSet {
        revision: ClassificationRuleSetRevisionId::new("classification-v1").unwrap(),
        checksum: ClassificationRuleSetChecksum::new("sha256:fixture").unwrap(),
        unsupported_coverage_floor: DataClassification::Internal,
        rules: vec![ClassificationRule {
            finding_kind: FindingKind::ApiKey,
            classification: DataClassification::Confidential,
        }],
    })
    .unwrap_err();
    assert_eq!(
        weakened,
        prodex_domain::ClassificationError::ClassificationTooLow
    );
}
