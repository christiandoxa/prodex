//! Deterministic, bounded data-classification rules.

use std::error::Error;
use std::fmt;

use serde::Serialize;

use super::{DataClassification, FindingKind, InspectionCoverage, InspectionResult};

pub const MAX_CLASSIFICATION_RULES: usize = 128;
pub const MAX_CLASSIFICATION_REASON_CODES: usize = 32;
const MAX_CLASSIFICATION_TOKEN_BYTES: usize = 128;

macro_rules! classification_token {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ClassificationError> {
                let value = value.into();
                if !classification_token_is_valid(&value) {
                    return Err(ClassificationError::InvalidToken);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&"<redacted>")
                    .finish()
            }
        }
    };
}

classification_token!(ClassificationRuleSetRevisionId);
classification_token!(ClassificationRuleSetChecksum);
classification_token!(ClassificationReasonCode);

fn classification_token_is_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CLASSIFICATION_TOKEN_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassificationRule {
    pub finding_kind: FindingKind,
    pub classification: DataClassification,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ClassificationRuleSet {
    pub revision: ClassificationRuleSetRevisionId,
    pub checksum: ClassificationRuleSetChecksum,
    pub unsupported_coverage_floor: DataClassification,
    pub rules: Vec<ClassificationRule>,
}

impl fmt::Debug for ClassificationRuleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClassificationRuleSet")
            .field("revision", &self.revision)
            .field("checksum", &self.checksum)
            .field(
                "unsupported_coverage_floor",
                &self.unsupported_coverage_floor,
            )
            .field("rule_count", &self.rules.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CompiledClassificationRuleSet {
    revision: ClassificationRuleSetRevisionId,
    checksum: ClassificationRuleSetChecksum,
    unsupported_coverage_floor: DataClassification,
    rules: Vec<ClassificationRule>,
}

impl fmt::Debug for CompiledClassificationRuleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledClassificationRuleSet")
            .field("revision", &self.revision)
            .field("checksum", &self.checksum)
            .field(
                "unsupported_coverage_floor",
                &self.unsupported_coverage_floor,
            )
            .field("rule_count", &self.rules.len())
            .finish()
    }
}

pub fn compile_classification_rule_set(
    mut rule_set: ClassificationRuleSet,
) -> Result<CompiledClassificationRuleSet, ClassificationError> {
    if rule_set.rules.len() > MAX_CLASSIFICATION_RULES {
        return Err(ClassificationError::RuleLimitExceeded);
    }
    rule_set
        .rules
        .sort_by_key(|rule| (rule.finding_kind, rule.classification));
    if rule_set
        .rules
        .windows(2)
        .any(|rules| rules[0].finding_kind == rules[1].finding_kind)
    {
        return Err(ClassificationError::DuplicateRule);
    }
    if rule_set
        .rules
        .iter()
        .any(|rule| rule.classification < rule.finding_kind.minimum_classification())
    {
        return Err(ClassificationError::ClassificationTooLow);
    }
    Ok(CompiledClassificationRuleSet {
        revision: rule_set.revision,
        checksum: rule_set.checksum,
        unsupported_coverage_floor: rule_set.unsupported_coverage_floor,
        rules: rule_set.rules,
    })
}

#[derive(Clone, Copy)]
pub struct ClassificationRequest<'a> {
    pub inspection: &'a InspectionResult,
    pub trusted_label: Option<DataClassification>,
    pub untrusted_label: Option<DataClassification>,
    pub prior_classification: Option<DataClassification>,
    pub session_floor: DataClassification,
    pub route_floor: DataClassification,
    pub request_risk_floor: DataClassification,
}

impl fmt::Debug for ClassificationRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClassificationRequest")
            .field("inspection", &self.inspection)
            .field("trusted_label", &self.trusted_label)
            .field("untrusted_label", &self.untrusted_label)
            .field("prior_classification", &self.prior_classification)
            .field("session_floor", &self.session_floor)
            .field("route_floor", &self.route_floor)
            .field("request_risk_floor", &self.request_risk_floor)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct ClassificationDecision {
    classification: DataClassification,
    coverage: InspectionCoverage,
    reason_codes: Vec<ClassificationReasonCode>,
    revision: ClassificationRuleSetRevisionId,
    checksum: ClassificationRuleSetChecksum,
}

impl fmt::Debug for ClassificationDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClassificationDecision")
            .field("classification", &self.classification)
            .field("coverage", &self.coverage)
            .field("reason_code_count", &self.reason_codes.len())
            .field("revision", &self.revision)
            .field("checksum", &self.checksum)
            .finish()
    }
}

impl ClassificationDecision {
    pub fn classification(&self) -> DataClassification {
        self.classification
    }

    pub fn coverage(&self) -> InspectionCoverage {
        self.coverage
    }

    pub fn reason_codes(&self) -> &[ClassificationReasonCode] {
        &self.reason_codes
    }

    pub fn revision(&self) -> &ClassificationRuleSetRevisionId {
        &self.revision
    }

    pub fn checksum(&self) -> &ClassificationRuleSetChecksum {
        &self.checksum
    }
}

pub fn classify_inspection(
    rules: &CompiledClassificationRuleSet,
    request: ClassificationRequest<'_>,
) -> Result<ClassificationDecision, ClassificationError> {
    let mut classification = request
        .inspection
        .classification()
        .raised_to(request.session_floor)
        .raised_to(request.route_floor)
        .raised_to(request.request_risk_floor);
    let mut reasons = vec![ClassificationReasonCode::new("inspection.classification")?];

    for (label, reason) in [
        (request.trusted_label, "label.trusted"),
        (request.untrusted_label, "label.untrusted_raise_only"),
        (request.prior_classification, "context.prior_monotonic"),
    ] {
        if let Some(label) = label {
            let raised = classification.raised_to(label);
            if raised != classification {
                reasons.push(ClassificationReasonCode::new(reason)?);
                classification = raised;
            }
        }
    }

    let coverage_reason = match request.inspection.coverage() {
        InspectionCoverage::Full => None,
        InspectionCoverage::Partial => Some("coverage.partial"),
        InspectionCoverage::Unsupported => Some("coverage.unsupported"),
    };
    if let Some(reason) = coverage_reason {
        classification = classification.raised_to(rules.unsupported_coverage_floor);
        reasons.push(ClassificationReasonCode::new(reason)?);
    }

    for finding in request.inspection.findings() {
        if let Ok(index) = rules
            .rules
            .binary_search_by_key(&finding.kind(), |rule| rule.finding_kind)
        {
            classification = classification.raised_to(rules.rules[index].classification);
        }
    }
    if !request.inspection.findings().is_empty() {
        reasons.push(ClassificationReasonCode::new("detector.finding")?);
    }

    reasons.sort();
    reasons.dedup();
    if reasons.len() > MAX_CLASSIFICATION_REASON_CODES {
        return Err(ClassificationError::ReasonLimitExceeded);
    }
    Ok(ClassificationDecision {
        classification,
        coverage: request.inspection.coverage(),
        reason_codes: reasons,
        revision: rules.revision.clone(),
        checksum: rules.checksum.clone(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassificationError {
    InvalidToken,
    RuleLimitExceeded,
    DuplicateRule,
    ClassificationTooLow,
    ReasonLimitExceeded,
}

impl fmt::Display for ClassificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "classification rules are invalid")
    }
}

impl Error for ClassificationError {}
