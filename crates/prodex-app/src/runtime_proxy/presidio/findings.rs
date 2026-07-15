//! Presidio finding normalization and domain inspection conversion.

use anyhow::{Context, Result};
use prodex_application::{
    ApplicationInspectionPlan, ApplicationInspectionRequest, ApplicationInspectionSource,
    plan_application_request_inspection,
};
use prodex_domain::{
    ContentLocation, DataClassification, DetectorId, DetectorRevisionId, FindingKind,
    InspectionCoverage, InspectionFinding, InspectionLimits, InspectionReasonCode, InspectionTag,
};

use super::json_body::PresidioJsonString;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(super) struct PresidioAnalyzerResult {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) score: f64,
    pub(super) entity_type: String,
    #[serde(default)]
    pub(super) language: String,
}

pub(super) fn runtime_presidio_inspection_source(
    coverage: InspectionCoverage,
    findings: Vec<InspectionFinding>,
    masked: bool,
) -> Result<ApplicationInspectionSource> {
    runtime_inspection_source(coverage, findings, masked, "presidio.finding")
}

pub(super) fn runtime_local_inspection_source(
    coverage: InspectionCoverage,
    findings: Vec<InspectionFinding>,
    masked: bool,
) -> Result<ApplicationInspectionSource> {
    runtime_inspection_source(coverage, findings, masked, "local.finding")
}

fn runtime_inspection_source(
    coverage: InspectionCoverage,
    findings: Vec<InspectionFinding>,
    masked: bool,
    finding_reason: &'static str,
) -> Result<ApplicationInspectionSource> {
    let has_findings = !findings.is_empty();
    let masked_findings = if masked {
        let mut kinds = findings
            .iter()
            .map(|finding| finding.kind())
            .collect::<Vec<_>>();
        kinds.sort();
        kinds.dedup();
        kinds
    } else {
        Vec::new()
    };
    Ok(ApplicationInspectionSource {
        coverage,
        findings,
        masked_findings,
        tags: has_findings
            .then(|| InspectionTag::new("sensitive-data"))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .context("invalid inspection tag")?,
        reason_codes: has_findings
            .then(|| InspectionReasonCode::new(finding_reason))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .context("invalid inspection reason code")?,
    })
}

pub(super) fn runtime_presidio_unavailable_source(
    reason: &'static str,
) -> Result<ApplicationInspectionSource> {
    Ok(ApplicationInspectionSource {
        coverage: InspectionCoverage::Unsupported,
        findings: Vec::new(),
        masked_findings: Vec::new(),
        tags: Vec::new(),
        reason_codes: vec![
            InspectionReasonCode::new(reason).context("invalid inspection reason code")?,
        ],
    })
}

pub(super) fn runtime_presidio_inspection_plan(
    sources: Vec<ApplicationInspectionSource>,
    default_classification: DataClassification,
    detector_revision: &DetectorRevisionId,
) -> Result<ApplicationInspectionPlan> {
    plan_application_request_inspection(ApplicationInspectionRequest {
        sources,
        default_classification,
        trusted_label: None,
        prior_classification: None,
        detector_revision: detector_revision.clone(),
        limits: InspectionLimits::default(),
    })
    .context("failed to build inspection plan")
}

pub(super) fn runtime_presidio_findings(
    values: &[PresidioJsonString],
    separator: &str,
    findings: &[PresidioAnalyzerResult],
) -> Result<Vec<InspectionFinding>> {
    let separator_chars = separator.chars().count();
    let mut ranges = Vec::with_capacity(values.len());
    let mut cursor = 0usize;
    for value in values {
        let end = cursor
            .checked_add(value.text.chars().count())
            .context("Presidio content offset overflow")?;
        ranges.push((cursor, end));
        cursor = end
            .checked_add(separator_chars)
            .context("Presidio content offset overflow")?;
    }

    findings
        .iter()
        .map(|finding| {
            let (value_index, (value_start, _)) = ranges
                .iter()
                .copied()
                .enumerate()
                .find(|(_, (start, end))| finding.start >= *start && finding.end <= *end)
                .context("Presidio finding crossed a content-field boundary")?;
            let value = &values[value_index];
            let start_char = finding.start - value_start;
            let end_char = finding.end - value_start;
            let start_byte = unicode_scalar_offset_to_byte(&value.text, start_char)
                .context("Presidio finding start offset was invalid")?;
            let end_byte = unicode_scalar_offset_to_byte(&value.text, end_char)
                .context("Presidio finding end offset was invalid")?;
            let confidence = presidio_confidence_basis_points(finding.score)?;
            InspectionFinding::new(
                presidio_finding_kind(&finding.entity_type),
                ContentLocation::new(&value.path, start_byte, end_byte)?,
                confidence,
                DetectorId::new("presidio")?,
            )
            .map_err(anyhow::Error::from)
        })
        .collect()
}

pub(super) fn unicode_scalar_offset_to_byte(value: &str, offset: usize) -> Option<usize> {
    if offset == value.chars().count() {
        return Some(value.len());
    }
    value.char_indices().nth(offset).map(|(index, _)| index)
}

pub(super) fn presidio_confidence_basis_points(score: f64) -> Result<u16> {
    if !score.is_finite() || !(0.0..=1.0).contains(&score) {
        anyhow::bail!("Presidio returned an invalid confidence score");
    }
    Ok((score * 10_000.0).round() as u16)
}

pub(super) fn presidio_finding_kind(entity_type: &str) -> FindingKind {
    match entity_type {
        "EMAIL_ADDRESS" => FindingKind::EmailAddress,
        "PHONE_NUMBER" => FindingKind::PhoneNumber,
        "PERSON" => FindingKind::PersonName,
        "LOCATION" => FindingKind::PhysicalAddress,
        "CREDIT_CARD" => FindingKind::PaymentCard,
        "IBAN_CODE" | "CRYPTO" => FindingKind::FinancialAccount,
        "API_KEY" => FindingKind::ApiKey,
        "ACCESS_TOKEN" => FindingKind::AccessToken,
        "PRIVATE_KEY" => FindingKind::PrivateKey,
        "PASSWORD" => FindingKind::Password,
        value if value.ends_with("_ID") || value.ends_with("_SSN") => FindingKind::GovernmentId,
        _ => FindingKind::TenantSensitive,
    }
}
