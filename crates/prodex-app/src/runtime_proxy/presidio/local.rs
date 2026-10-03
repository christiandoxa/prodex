use super::json_body::{
    MAX_PRESIDIO_JSON_TEXT_BYTES, PresidioJsonString, collect_json_content,
    replace_json_string_values,
};
use anyhow::{Context, Result, anyhow};
use prodex_domain::{
    ContentLocation, DetectorId, FindingKind, InspectionCoverage, InspectionFinding,
    MAX_INSPECTION_FINDINGS, TenantId,
};
use prodex_mojo_core::{MojoError, redaction::LocalInspectionFindingKind};

const LOCAL_DETECTOR_ID: &str = "local-bounded-v1";

#[derive(Clone, Default)]
pub(crate) struct RuntimeTenantDetectorPatterns;

impl RuntimeTenantDetectorPatterns {
    pub(crate) fn has_for_tenant(&self, _tenant_id: Option<TenantId>) -> bool {
        false
    }
}

pub(crate) struct RuntimeLocalInspection {
    pub(crate) body: Vec<u8>,
    pub(crate) coverage: InspectionCoverage,
    pub(crate) findings: Vec<InspectionFinding>,
    pub(crate) changed: bool,
}

#[derive(Debug)]
pub(crate) struct RuntimeLocalInspectionFailure {
    pub(crate) body: Vec<u8>,
    pub(crate) error: anyhow::Error,
}

struct RuntimeLocalInspectionResult {
    body: Option<Vec<u8>>,
    coverage: InspectionCoverage,
    findings: Vec<InspectionFinding>,
    changed: bool,
}

#[cfg(test)]
pub(crate) fn runtime_local_inspect_and_mask(body: Vec<u8>) -> Result<RuntimeLocalInspection> {
    runtime_local_inspect_and_mask_for_tenant(body, &RuntimeTenantDetectorPatterns, None)
        .map_err(|failure| failure.error)
}

pub(crate) fn runtime_local_inspect_and_mask_for_tenant(
    body: Vec<u8>,
    _patterns: &RuntimeTenantDetectorPatterns,
    _tenant_id: Option<TenantId>,
) -> std::result::Result<RuntimeLocalInspection, RuntimeLocalInspectionFailure> {
    let text = match String::from_utf8(body) {
        Ok(text) => text,
        Err(error) => {
            let source = error.utf8_error();
            return Err(RuntimeLocalInspectionFailure {
                body: error.into_bytes(),
                error: anyhow!(source).context("request body is not UTF-8"),
            });
        }
    };
    match runtime_local_inspect_and_mask_text(&text) {
        Ok(result) => Ok(RuntimeLocalInspection {
            body: result.body.unwrap_or_else(|| text.into_bytes()),
            coverage: result.coverage,
            findings: result.findings,
            changed: result.changed,
        }),
        Err(error) => Err(RuntimeLocalInspectionFailure {
            body: text.into_bytes(),
            error,
        }),
    }
}

fn runtime_local_inspect_and_mask_text(text: &str) -> Result<RuntimeLocalInspectionResult> {
    if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(text) {
        let content = collect_json_content(&json)?;
        let mut findings = Vec::new();
        let mut masked_values = Vec::with_capacity(content.values.len());
        for value in &content.values {
            let remaining_findings = MAX_INSPECTION_FINDINGS
                .checked_sub(findings.len())
                .context("local inspection finding count exceeded safe limit")?;
            let (masked, value_findings) = inspect_and_mask_value(value, remaining_findings)?;
            findings.extend(value_findings);
            masked_values.push(masked);
        }
        if findings.is_empty() {
            return Ok(RuntimeLocalInspectionResult {
                body: None,
                coverage: content.coverage,
                findings,
                changed: false,
            });
        }
        let mut masked_values = masked_values.iter().map(String::as_str);
        replace_json_string_values(&mut json, false, &mut masked_values);
        return Ok(RuntimeLocalInspectionResult {
            body: Some(
                serde_json::to_vec(&json).context("failed to serialize masked JSON request")?,
            ),
            coverage: content.coverage,
            findings,
            changed: true,
        });
    }

    let value = PresidioJsonString {
        path: "$".to_string(),
        text: text.to_string(),
        sensitive_kind: None,
    };
    if value.text.len() > MAX_PRESIDIO_JSON_TEXT_BYTES {
        anyhow::bail!("request content exceeds inspection limits");
    }
    let (text, findings) = inspect_and_mask_value(&value, MAX_INSPECTION_FINDINGS)?;
    let changed = !findings.is_empty();
    Ok(RuntimeLocalInspectionResult {
        body: changed.then(|| text.into_bytes()),
        coverage: InspectionCoverage::Full,
        changed,
        findings,
    })
}

fn inspect_and_mask_value(
    value: &PresidioJsonString,
    max_matches: usize,
) -> Result<(String, Vec<InspectionFinding>)> {
    let sensitive_kind = value.sensitive_kind.map(|kind| match kind {
        FindingKind::EmailAddress => LocalInspectionFindingKind::EmailAddress,
        FindingKind::PhoneNumber => LocalInspectionFindingKind::PhoneNumber,
        FindingKind::PersonName => LocalInspectionFindingKind::PersonName,
        FindingKind::PhysicalAddress => LocalInspectionFindingKind::PhysicalAddress,
        FindingKind::GovernmentId => LocalInspectionFindingKind::GovernmentId,
        FindingKind::FinancialAccount => LocalInspectionFindingKind::FinancialAccount,
        FindingKind::PaymentCard => LocalInspectionFindingKind::PaymentCard,
        FindingKind::AccessToken => LocalInspectionFindingKind::AccessToken,
        FindingKind::ApiKey => LocalInspectionFindingKind::ApiKey,
        FindingKind::PrivateKey => LocalInspectionFindingKind::PrivateKey,
        FindingKind::Password => LocalInspectionFindingKind::Password,
        FindingKind::TenantSensitive => LocalInspectionFindingKind::TenantSensitive,
    });
    let redaction = prodex_mojo_core::redaction::local_inspect_and_redact(
        &value.text,
        sensitive_kind,
        max_matches,
    )
    .map_err(|error| match error {
        MojoError::Capacity => anyhow!("local inspection finding count exceeded safe limit"),
        error => anyhow!("{error:?}").context("Mojo local inspection failed"),
    })?;
    let detector_id = DetectorId::new(LOCAL_DETECTOR_ID)?;
    let findings = redaction
        .matches
        .iter()
        .map(|finding| {
            let kind = match finding.kind {
                LocalInspectionFindingKind::EmailAddress => FindingKind::EmailAddress,
                LocalInspectionFindingKind::PhoneNumber => FindingKind::PhoneNumber,
                LocalInspectionFindingKind::PersonName => FindingKind::PersonName,
                LocalInspectionFindingKind::PhysicalAddress => FindingKind::PhysicalAddress,
                LocalInspectionFindingKind::GovernmentId => FindingKind::GovernmentId,
                LocalInspectionFindingKind::FinancialAccount => FindingKind::FinancialAccount,
                LocalInspectionFindingKind::PaymentCard => FindingKind::PaymentCard,
                LocalInspectionFindingKind::AccessToken => FindingKind::AccessToken,
                LocalInspectionFindingKind::ApiKey => FindingKind::ApiKey,
                LocalInspectionFindingKind::PrivateKey => FindingKind::PrivateKey,
                LocalInspectionFindingKind::Password => FindingKind::Password,
                LocalInspectionFindingKind::TenantSensitive => FindingKind::TenantSensitive,
            };
            InspectionFinding::new(
                kind,
                ContentLocation::new(&value.path, finding.start, finding.end)?,
                10_000,
                detector_id.clone(),
            )
            .map_err(anyhow::Error::from)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok((redaction.text, findings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_inspection_masks_supported_nested_content_and_preserves_structure() {
        let private_key = concat!(
            "-----BEGIN PRIVATE ",
            "KEY-----abc-----END PRIVATE KEY-----"
        );
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gpt-5",
            "input": [
                "héllo user@example.com",
                "Bearer access-token-value",
                {"arguments": {
                    "api_key": "tenant-api-key-value",
                    "private_key": private_key,
                    "note": "sk-proj-1234567890 card 4111-1111-1111-1111"
                }}
            ]
        }))
        .unwrap();

        let inspected = runtime_local_inspect_and_mask(body).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&inspected.body).unwrap();
        let rendered = value.to_string();

        assert_eq!(value["model"], "gpt-5");
        assert_eq!(value["input"][0], "héllo <redacted>");
        assert_eq!(value["input"][1], "Bearer <redacted>");
        assert_eq!(value["input"][2]["arguments"]["api_key"], "<redacted>");
        assert_eq!(value["input"][2]["arguments"]["private_key"], "<redacted>");
        assert_eq!(
            value["input"][2]["arguments"]["note"],
            "<redacted> card <redacted>"
        );
        assert_eq!(rendered.matches("<redacted>").count(), 6);
        assert!(!rendered.contains("user@example.com"));
        assert_eq!(inspected.coverage, InspectionCoverage::Full);
        for expected in [
            FindingKind::EmailAddress,
            FindingKind::AccessToken,
            FindingKind::ApiKey,
            FindingKind::PrivateKey,
            FindingKind::FinancialAccount,
        ] {
            assert!(
                inspected
                    .findings
                    .iter()
                    .any(|finding| finding.kind() == expected)
            );
        }
        assert!(
            inspected
                .findings
                .iter()
                .any(|finding| finding.kind() == FindingKind::EmailAddress)
        );
    }

    #[test]
    fn local_inspection_rejects_deep_and_match_flood_inputs() {
        let mut deep = serde_json::json!("value");
        for _ in 0..=super::super::json_body::MAX_PRESIDIO_JSON_DEPTH {
            deep = serde_json::json!({"input": deep});
        }
        assert!(runtime_local_inspect_and_mask(serde_json::to_vec(&deep).unwrap()).is_err());

        let flood = (0..=MAX_INSPECTION_FINDINGS)
            .map(|index| format!("user{index}@example.com"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(runtime_local_inspect_and_mask(flood.into_bytes()).is_err());
    }

    #[test]
    fn malformed_private_key_is_masked_through_end_of_value() {
        let secret = concat!(
            "-----BEGIN PRIVATE ",
            "KEY-----malformed-secret-without-footer"
        );
        let body = serde_json::to_vec(&serde_json::json!({"input": secret})).unwrap();

        let inspected = runtime_local_inspect_and_mask(body).unwrap();
        let rendered = String::from_utf8(inspected.body).unwrap();

        assert!(!rendered.contains("malformed-secret-without-footer"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn local_inspection_preserves_short_and_malformed_candidates() {
        let text = "12 digits 123456789012, 20 digits 12345678901234567890, short sk-1234567, malformed a@.test";
        let inspected = runtime_local_inspect_and_mask(text.as_bytes().to_vec()).unwrap();
        let rendered = String::from_utf8(inspected.body).unwrap();

        assert_eq!(rendered, text);
        assert!(inspected.findings.is_empty());
        assert!(!inspected.changed);
    }
}
