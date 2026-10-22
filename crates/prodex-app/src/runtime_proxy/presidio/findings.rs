//! Presidio finding normalization and domain inspection conversion.

use anyhow::{Context, Result};
use prodex_domain::{
    ContentLocation, DataClassification, DetectorId, DetectorRevisionId, FindingKind,
    InspectionCoverage, InspectionFinding, InspectionLimits, InspectionReasonCode,
    InspectionResult, InspectionTag,
};

use super::json_body::PresidioJsonString;
use prodex_mojo_core::redaction::{
    AnalyzerFinding, FindingLocalizationError, FindingLocalizationMode, FindingLocation,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ApplicationInspectionSource {
    pub(super) coverage: InspectionCoverage,
    pub(super) findings: Vec<InspectionFinding>,
    pub(super) masked_findings: Vec<FindingKind>,
    pub(super) tags: Vec<InspectionTag>,
    pub(super) reason_codes: Vec<InspectionReasonCode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ApplicationInspectionPlan {
    pub(super) result: InspectionResult,
    pub(super) masked_findings: Vec<FindingKind>,
}

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
    let limits = InspectionLimits::default();
    if sources.len() > limits.max_detectors {
        anyhow::bail!("too many inspection sources");
    }
    for source in &sources {
        if source.masked_findings.len() > FindingKind::ALL.len() {
            anyhow::bail!("too many masked finding kinds");
        }
    }
    let finding_kinds = sources
        .iter()
        .map(|source| {
            source
                .findings
                .iter()
                .map(|finding| finding.kind() as u8)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let masked_kinds = sources
        .iter()
        .map(|source| {
            source
                .masked_findings
                .iter()
                .map(|kind| *kind as u8)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let facts = sources
        .iter()
        .enumerate()
        .map(
            |(index, source)| prodex_mojo_core::policy::GovernanceInspectionSource {
                coverage: source.coverage as u8,
                finding_kinds: &finding_kinds[index],
                masked_kinds: &masked_kinds[index],
                tag_count: source.tags.len(),
                reason_count: source.reason_codes.len(),
            },
        )
        .collect::<Vec<_>>();
    let aggregate = prodex_mojo_core::policy::governance_inspection_aggregate(
        &facts,
        default_classification as u8,
    )
    .map_err(|error| match error {
        prodex_mojo_core::policy::GovernanceInspectionAggregationError::LimitExceeded => {
            anyhow::Error::new(prodex_domain::InspectionModelError::LimitExceeded)
                .context("failed to build inspection result")
        }
        prodex_mojo_core::policy::GovernanceInspectionAggregationError::Abi(error) => {
            anyhow::anyhow!("compiled Mojo inspection aggregation failed: {error:?}")
        }
    })?;
    let coverage = [
        InspectionCoverage::Full,
        InspectionCoverage::Partial,
        InspectionCoverage::Unsupported,
    ][usize::from(aggregate.coverage)];
    let classification = [
        DataClassification::Public,
        DataClassification::Internal,
        DataClassification::Confidential,
        DataClassification::Restricted,
    ][usize::from(aggregate.classification)];
    let masked_findings = aggregate
        .masked_kinds
        .into_iter()
        .map(|kind| FindingKind::ALL[usize::from(kind)])
        .collect();
    let mut findings = Vec::new();
    let mut tags = Vec::new();
    let mut reason_codes = Vec::new();
    for source in sources {
        findings.extend(source.findings);
        tags.extend(source.tags);
        reason_codes.extend(source.reason_codes);
    }
    let result = InspectionResult::new(
        coverage,
        classification,
        findings,
        tags,
        reason_codes,
        detector_revision.clone(),
        limits,
    )
    .context("failed to build inspection result")?;
    Ok(ApplicationInspectionPlan {
        result,
        masked_findings,
    })
}

pub(super) fn runtime_presidio_findings(
    values: &[PresidioJsonString],
    separator: &str,
    findings: &[PresidioAnalyzerResult],
) -> Result<Vec<InspectionFinding>> {
    runtime_presidio_localization(
        values,
        separator,
        findings,
        FindingLocalizationMode::Localize,
    )?
    .into_iter()
    .map(|location| {
        let value = &values[location.value_index as usize];
        InspectionFinding::new(
            FindingKind::ALL[location.kind as usize],
            ContentLocation::new(
                &value.path,
                location.start_byte as usize,
                location.end_byte as usize,
            )?,
            location.confidence as u16,
            DetectorId::new("presidio")?,
        )
        .map_err(anyhow::Error::from)
    })
    .collect()
}

pub(super) fn retain_findings_within_json_values(
    findings: &mut Vec<PresidioAnalyzerResult>,
    values: &[PresidioJsonString],
    separator: &str,
) -> Result<()> {
    let plan = runtime_presidio_localization(
        values,
        separator,
        findings,
        FindingLocalizationMode::RetainWithinValues,
    )?;
    *findings = plan
        .into_iter()
        .map(|item| findings[item.source as usize].clone())
        .collect();
    Ok(())
}

fn runtime_presidio_localization(
    values: &[PresidioJsonString],
    separator: &str,
    findings: &[PresidioAnalyzerResult],
    mode: FindingLocalizationMode,
) -> Result<Vec<FindingLocation>> {
    let text = values
        .iter()
        .map(|value| value.text.as_str())
        .collect::<Vec<_>>();
    let facts = findings
        .iter()
        .map(|finding| AnalyzerFinding {
            start: finding.start,
            end: finding.end,
            score: finding.score,
            entity: &finding.entity_type,
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::redaction::finding_localization_plan(&facts, &text, separator, mode).map_err(
        |error| match error {
            FindingLocalizationError::CrossFieldBoundary => {
                anyhow::anyhow!("Presidio finding crossed a content-field boundary")
            }
            FindingLocalizationError::InvalidConfidence => {
                anyhow::anyhow!("Presidio returned an invalid confidence score")
            }
            FindingLocalizationError::OffsetOverflow => {
                anyhow::anyhow!("Presidio content offset overflow")
            }
            FindingLocalizationError::Abi(error) => {
                anyhow::anyhow!("compiled Mojo Presidio finding localization failed: {error:?}")
            }
        },
    )
}

#[cfg(test)]
#[path = "findings/aggregation_tests.rs"]
mod aggregation_tests;

#[cfg(test)]
mod tests {
    use super::super::json_body::PresidioJsonString;
    use super::*;

    #[test]
    fn compiled_presidio_finding_localization_preserves_all_entity_kinds_and_rounding() {
        let value = PresidioJsonString {
            path: "$.input".to_string(),
            text: "🧭".to_string(),
            sensitive_kind: None,
        };
        for (entity, kind) in [
            ("EMAIL_ADDRESS", FindingKind::EmailAddress),
            ("PHONE_NUMBER", FindingKind::PhoneNumber),
            ("PERSON", FindingKind::PersonName),
            ("LOCATION", FindingKind::PhysicalAddress),
            ("CREDIT_CARD", FindingKind::PaymentCard),
            ("IBAN_CODE", FindingKind::FinancialAccount),
            ("CRYPTO", FindingKind::FinancialAccount),
            ("API_KEY", FindingKind::ApiKey),
            ("ACCESS_TOKEN", FindingKind::AccessToken),
            ("PRIVATE_KEY", FindingKind::PrivateKey),
            ("PASSWORD", FindingKind::Password),
            ("NATIONAL_ID", FindingKind::GovernmentId),
            ("CUSTOM_SSN", FindingKind::GovernmentId),
            ("custom_ssn", FindingKind::TenantSensitive),
        ] {
            let finding = PresidioAnalyzerResult {
                start: 0,
                end: 1,
                score: 0.00005_f64.next_down(),
                entity_type: entity.to_string(),
                language: "en".to_string(),
            };
            let result =
                runtime_presidio_findings(std::slice::from_ref(&value), "", &[finding]).unwrap();
            assert_eq!(result[0].kind(), kind, "{entity}");
            assert_eq!(result[0].location().byte_range(), 0..4);
        }
    }

    #[test]
    fn presidio_rejects_malformed_finding_metadata() {
        let error = runtime_presidio_findings(
            &[PresidioJsonString {
                path: "$.input".to_string(),
                text: "safe".to_string(),
                sensitive_kind: None,
            }],
            "",
            &[PresidioAnalyzerResult {
                start: 0,
                end: 4,
                score: f64::NAN,
                entity_type: "PERSON".to_string(),
                language: "en".to_string(),
            }],
        )
        .unwrap_err();

        assert!(error.to_string().contains("invalid confidence score"));
    }
}
