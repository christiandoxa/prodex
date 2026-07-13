use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const MAX_INSPECTION_DETECTORS: usize = 8;
pub const MAX_INSPECTION_FINDINGS: usize = 256;
pub const MAX_INSPECTION_TAGS: usize = 32;
pub const MAX_INSPECTION_REASON_CODES: usize = 32;
pub const MAX_INSPECTION_TOKEN_BYTES: usize = 128;
pub const MAX_CONTENT_LOCATION_PATH_BYTES: usize = 256;

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
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
            Self::Confidential => "confidential",
            Self::Restricted => "restricted",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionCoverage {
    Full,
    Partial,
    Unsupported,
}

impl InspectionCoverage {
    pub fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Full, Self::Full) => Self::Full,
            (Self::Unsupported, Self::Unsupported) => Self::Unsupported,
            _ => Self::Partial,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Partial => "partial",
            Self::Unsupported => "unsupported",
        }
    }
}

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
        match self {
            Self::EmailAddress
            | Self::PhoneNumber
            | Self::PersonName
            | Self::PhysicalAddress
            | Self::TenantSensitive => DataClassification::Confidential,
            Self::GovernmentId
            | Self::FinancialAccount
            | Self::PaymentCard
            | Self::AccessToken
            | Self::ApiKey
            | Self::PrivateKey
            | Self::Password => DataClassification::Restricted,
        }
    }
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
        if field_path.is_empty()
            || field_path.len() > MAX_CONTENT_LOCATION_PATH_BYTES
            || !field_path.bytes().all(content_location_path_byte)
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

fn content_location_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'$' | b'.' | b'_' | b'-' | b'*' | b'[' | b']')
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
    !value.is_empty()
        && value.len() <= MAX_INSPECTION_TOKEN_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
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
        if max_detectors == 0
            || max_detectors > MAX_INSPECTION_DETECTORS
            || max_findings == 0
            || max_findings > MAX_INSPECTION_FINDINGS
            || max_tags == 0
            || max_tags > MAX_INSPECTION_TAGS
            || max_reason_codes == 0
            || max_reason_codes > MAX_INSPECTION_REASON_CODES
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
        mut findings: Vec<InspectionFinding>,
        mut tags: Vec<InspectionTag>,
        mut reason_codes: Vec<InspectionReasonCode>,
        detector_revision: DetectorRevisionId,
        limits: InspectionLimits,
    ) -> Result<Self, InspectionModelError> {
        if findings.len() > limits.max_findings
            || tags.len() > limits.max_tags
            || reason_codes.len() > limits.max_reason_codes
        {
            return Err(InspectionModelError::LimitExceeded);
        }
        if findings
            .iter()
            .any(|finding| classification < finding.kind.minimum_classification())
        {
            return Err(InspectionModelError::ClassificationTooLow);
        }

        findings.sort_by(|left, right| {
            left.location
                .cmp(&right.location)
                .then_with(|| left.kind.cmp(&right.kind))
                .then_with(|| left.detector_id.cmp(&right.detector_id))
                .then_with(|| {
                    left.confidence_basis_points
                        .cmp(&right.confidence_basis_points)
                })
        });
        tags.sort();
        tags.dedup();
        reason_codes.sort();
        reason_codes.dedup();

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
