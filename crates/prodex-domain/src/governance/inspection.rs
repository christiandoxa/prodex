use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const MAX_INSPECTION_DETECTORS: usize = 8;
pub const MAX_INSPECTION_FINDINGS: usize = 256;
pub const MAX_INSPECTION_TAGS: usize = 32;
pub const MAX_INSPECTION_REASON_CODES: usize = 32;
pub const MAX_INSPECTION_TOKEN_BYTES: usize = 128;
pub const MAX_CONTENT_LOCATION_PATH_BYTES: usize = 256;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

impl DataClassification {
    pub fn raised_to(self, other: Self) -> Self {
        self.max(other)
    }

    pub fn as_str(self) -> &'static str {
        prodex_mojo_core::policy::governance_classification_label(self as u8)
            .expect("Mojo governance classification label returned invalid output")
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionCoverage {
    Full,
    Partial,
    Unsupported,
}

impl InspectionCoverage {
    pub fn combine(self, other: Self) -> Self {
        match prodex_mojo_core::policy::governance_coverage_combine(self as u8, other as u8)
            .expect("Mojo governance coverage combine returned invalid output")
        {
            0 => Self::Full,
            1 => Self::Partial,
            2 => Self::Unsupported,
            _ => unreachable!("validated Mojo governance coverage tag"),
        }
    }

    pub fn as_str(self) -> &'static str {
        prodex_mojo_core::policy::governance_coverage_label(self as u8)
            .expect("Mojo governance coverage label returned invalid output")
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    EmailAddress,
    PhoneNumber,
    PersonName,
    PhysicalAddress,
    GovernmentId,
    FinancialAccount,
    PaymentCard,
    AccessToken,
    ApiKey,
    PrivateKey,
    Password,
    TenantSensitive,
}

impl FindingKind {
    pub const ALL: [Self; 12] = [
        Self::EmailAddress,
        Self::PhoneNumber,
        Self::PersonName,
        Self::PhysicalAddress,
        Self::GovernmentId,
        Self::FinancialAccount,
        Self::PaymentCard,
        Self::AccessToken,
        Self::ApiKey,
        Self::PrivateKey,
        Self::Password,
        Self::TenantSensitive,
    ];

    pub fn minimum_classification(self) -> DataClassification {
        let classification =
            prodex_mojo_core::policy::governance_finding_minimum_classification(self as u8)
                .expect("Mojo governance finding classification returned invalid output");
        data_classification_from_tag(classification)
            .expect("Mojo governance finding classification returned an unknown tag")
    }
}

fn data_classification_from_tag(value: u8) -> Option<DataClassification> {
    [
        DataClassification::Public,
        DataClassification::Internal,
        DataClassification::Confidential,
        DataClassification::Restricted,
    ]
    .get(usize::from(value))
    .copied()
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ContentLocation {
    field_path: String,
    start_byte: u32,
    end_byte: u32,
}

impl fmt::Debug for ContentLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContentLocation")
            .field("field_path", &self.field_path)
            .field("start_byte", &self.start_byte)
            .field("end_byte", &self.end_byte)
            .finish()
    }
}

impl ContentLocation {
    pub fn new(
        field_path: impl Into<String>,
        start_byte: usize,
        end_byte: usize,
    ) -> Result<Self, InspectionModelError> {
        let field_path = field_path.into();
        if !prodex_mojo_core::policy::governance_content_location_path_valid(&field_path)
            .expect("Mojo governance content-location validation returned invalid output")
        {
            return Err(InspectionModelError::InvalidLocation);
        }
        if end_byte < start_byte {
            return Err(InspectionModelError::InvalidLocation);
        }
        let start_byte =
            u32::try_from(start_byte).map_err(|_| InspectionModelError::InvalidLocation)?;
        let end_byte =
            u32::try_from(end_byte).map_err(|_| InspectionModelError::InvalidLocation)?;
        Ok(Self {
            field_path,
            start_byte,
            end_byte,
        })
    }

    pub fn field_path(&self) -> &str {
        &self.field_path
    }

    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.start_byte as usize..self.end_byte as usize
    }
}

macro_rules! inspection_token {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, InspectionModelError> {
                let value = value.into();
                if !inspection_token_is_valid(&value) {
                    return Err(InspectionModelError::InvalidToken);
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

inspection_token!(DetectorId);
inspection_token!(DetectorRevisionId);
inspection_token!(InspectionTag);
inspection_token!(InspectionReasonCode);

fn inspection_token_is_valid(value: &str) -> bool {
    prodex_mojo_core::policy::governance_inspection_token_valid(value)
        .expect("Mojo governance inspection-token validation returned invalid output")
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct InspectionFinding {
    kind: FindingKind,
    location: ContentLocation,
    confidence_basis_points: u16,
    detector_id: DetectorId,
}

impl fmt::Debug for InspectionFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InspectionFinding")
            .field("kind", &self.kind)
            .field("location", &"<redacted>")
            .field("confidence_basis_points", &self.confidence_basis_points)
            .field("detector_id", &self.detector_id)
            .finish()
    }
}

impl InspectionFinding {
    pub fn new(
        kind: FindingKind,
        location: ContentLocation,
        confidence_basis_points: u16,
        detector_id: DetectorId,
    ) -> Result<Self, InspectionModelError> {
        if confidence_basis_points > 10_000 {
            return Err(InspectionModelError::InvalidConfidence);
        }
        Ok(Self {
            kind,
            location,
            confidence_basis_points,
            detector_id,
        })
    }

    pub fn kind(&self) -> FindingKind {
        self.kind
    }

    pub fn location(&self) -> &ContentLocation {
        &self.location
    }

    pub fn confidence_basis_points(&self) -> u16 {
        self.confidence_basis_points
    }

    pub fn detector_id(&self) -> &DetectorId {
        &self.detector_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InspectionLimits {
    pub max_detectors: usize,
    pub max_findings: usize,
    pub max_tags: usize,
    pub max_reason_codes: usize,
}

impl InspectionLimits {
    pub fn new(
        max_detectors: usize,
        max_findings: usize,
        max_tags: usize,
        max_reason_codes: usize,
    ) -> Result<Self, InspectionModelError> {
        if !prodex_mojo_core::policy::governance_inspection_limits_valid(
            max_detectors,
            max_findings,
            max_tags,
            max_reason_codes,
        )
        .expect("Mojo governance inspection-limits validation returned invalid output")
        {
            return Err(InspectionModelError::InvalidLimits);
        }
        Ok(Self {
            max_detectors,
            max_findings,
            max_tags,
            max_reason_codes,
        })
    }
}

impl Default for InspectionLimits {
    fn default() -> Self {
        Self {
            max_detectors: MAX_INSPECTION_DETECTORS,
            max_findings: MAX_INSPECTION_FINDINGS,
            max_tags: MAX_INSPECTION_TAGS,
            max_reason_codes: MAX_INSPECTION_REASON_CODES,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct InspectionResult {
    coverage: InspectionCoverage,
    classification: DataClassification,
    findings: Vec<InspectionFinding>,
    tags: Vec<InspectionTag>,
    reason_codes: Vec<InspectionReasonCode>,
    detector_revision: DetectorRevisionId,
}

impl fmt::Debug for InspectionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InspectionResult")
            .field("coverage", &self.coverage)
            .field("classification", &self.classification)
            .field("finding_count", &self.findings.len())
            .field("tag_count", &self.tags.len())
            .field("reason_code_count", &self.reason_codes.len())
            .field("detector_revision", &self.detector_revision)
            .finish()
    }
}

impl InspectionResult {
    pub fn new(
        coverage: InspectionCoverage,
        classification: DataClassification,
        findings: Vec<InspectionFinding>,
        tags: Vec<InspectionTag>,
        reason_codes: Vec<InspectionReasonCode>,
        detector_revision: DetectorRevisionId,
        limits: InspectionLimits,
    ) -> Result<Self, InspectionModelError> {
        if findings.len() > limits.max_findings
            || tags.len() > limits.max_tags
            || reason_codes.len() > limits.max_reason_codes
        {
            return Err(InspectionModelError::LimitExceeded);
        }
        let finding_kinds = findings
            .iter()
            .map(|finding| finding.kind as u8)
            .collect::<Vec<_>>();
        let classification_too_low =
            prodex_mojo_core::policy::governance_findings_exceed_classification(
                &finding_kinds,
                classification as u8,
            )
            .expect("Mojo governance finding classification returned invalid output");
        if classification_too_low {
            return Err(InspectionModelError::ClassificationTooLow);
        }

        let finding_keys = findings
            .iter()
            .map(|finding| {
                let location = &finding.location;
                prodex_mojo_core::policy::GovernanceInspectionFindingOrderKey {
                    field_path: &location.field_path,
                    start_byte: location.start_byte,
                    end_byte: location.end_byte,
                    kind: finding.kind as u8,
                    detector_id: finding.detector_id.as_str(),
                    confidence_basis_points: finding.confidence_basis_points,
                }
            })
            .collect::<Vec<_>>();
        let tag_values = tags.iter().map(InspectionTag::as_str).collect::<Vec<_>>();
        let reason_code_values = reason_codes
            .iter()
            .map(InspectionReasonCode::as_str)
            .collect::<Vec<_>>();
        let order = prodex_mojo_core::policy::governance_inspection_order(
            &finding_keys,
            &tag_values,
            &reason_code_values,
        )
        .expect("Mojo governance inspection ordering returned invalid output");
        let findings = apply_mojo_order(findings, &order.finding_indices);
        let tags = apply_mojo_order(tags, &order.tag_indices);
        let reason_codes = apply_mojo_order(reason_codes, &order.reason_code_indices);

        Ok(Self {
            coverage,
            classification,
            findings,
            tags,
            reason_codes,
            detector_revision,
        })
    }

    pub fn coverage(&self) -> InspectionCoverage {
        self.coverage
    }

    pub fn classification(&self) -> DataClassification {
        self.classification
    }

    pub fn findings(&self) -> &[InspectionFinding] {
        &self.findings
    }

    pub fn tags(&self) -> &[InspectionTag] {
        &self.tags
    }

    pub fn reason_codes(&self) -> &[InspectionReasonCode] {
        &self.reason_codes
    }

    pub fn detector_revision(&self) -> &DetectorRevisionId {
        &self.detector_revision
    }
}

fn apply_mojo_order<T>(values: Vec<T>, order: &[usize]) -> Vec<T> {
    let mut values = values.into_iter().map(Some).collect::<Vec<_>>();
    order
        .iter()
        .map(|index| {
            values[*index]
                .take()
                .expect("Mojo governance order indices are unique and in range")
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionModelError {
    InvalidLocation,
    InvalidToken,
    InvalidConfidence,
    InvalidLimits,
    LimitExceeded,
    ClassificationTooLow,
}

impl fmt::Display for InspectionModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inspection metadata is invalid")
    }
}

impl Error for InspectionModelError {}
