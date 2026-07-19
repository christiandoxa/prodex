//! Shared local/remote Presidio inspection and redaction engine.

use super::analyzer::{detect_presidio_language, merge_presidio_analyzer_results};
use super::findings::{
    PresidioAnalyzerResult, runtime_presidio_findings, runtime_presidio_inspection_source,
};
use super::json_body::{
    PresidioJsonString, collect_json_content, presidio_json_value_separator,
    replace_json_string_values,
};
use super::registry::RuntimePresidioRedactionState;
use crate::presidio_runtime::PresidioLanguageMode;
use crate::{RuntimePresidioRedactionConfig, read_async_response_body_with_limit};
use anyhow::{Context, Result, anyhow};
use prodex_application::ApplicationInspectionSource;
use prodex_domain::{InspectionCoverage, MAX_INSPECTION_FINDINGS};
use std::sync::Arc;

#[derive(Debug, serde::Deserialize)]
struct PresidioAnonymizeResponse {
    text: String,
}

struct RuntimePresidioTextRedaction {
    text: String,
    findings: Vec<PresidioAnalyzerResult>,
}

pub(super) struct RedactionOutcome {
    pub(super) body: Vec<u8>,
    pub(super) source: ApplicationInspectionSource,
}

#[derive(Debug)]
pub(super) struct InspectionExecutionFailure {
    pub(super) body: Vec<u8>,
    pub(super) error: anyhow::Error,
}

pub(super) enum InspectionExecutionOutcome {
    Redacted(RedactionOutcome),
    Failed(InspectionExecutionFailure),
}

pub(super) const fn runtime_local_inspection_required(
    rollout: prodex_config::GovernanceRolloutMode,
    legacy_local_enabled: bool,
    configured_detector_enabled: bool,
) -> bool {
    !matches!(rollout, prodex_config::GovernanceRolloutMode::Off)
        || legacy_local_enabled
        || configured_detector_enabled
}

pub(super) const fn runtime_local_inspection_fail_closed(
    rollout: prodex_config::GovernanceRolloutMode,
    legacy_local_enabled: bool,
    tenant_detector_enabled: bool,
    presidio_fail_closed: Option<bool>,
) -> bool {
    matches!(rollout, prodex_config::GovernanceRolloutMode::Enforce)
        || legacy_local_enabled
        || tenant_detector_enabled
        || matches!(presidio_fail_closed, Some(true))
}

pub(super) async fn runtime_presidio_redact_body(
    body: Vec<u8>,
    state: Arc<RuntimePresidioRedactionState>,
) -> Result<InspectionExecutionOutcome> {
    let _permit = match state.slots.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return Ok(InspectionExecutionOutcome::Failed(
                InspectionExecutionFailure {
                    body,
                    error: anyhow!("presidio_concurrency_limit_reached"),
                },
            ));
        }
    };
    let text = match String::from_utf8(body) {
        Ok(text) => text,
        Err(error) => {
            return Ok(InspectionExecutionOutcome::Failed(
                InspectionExecutionFailure {
                    body: error.into_bytes(),
                    error: anyhow!("request body is not UTF-8"),
                },
            ));
        }
    };
    Ok(
        match runtime_presidio_redact_text_body(&text, &state).await {
            Ok(redaction) => InspectionExecutionOutcome::Redacted(redaction),
            Err(error) => InspectionExecutionOutcome::Failed(InspectionExecutionFailure {
                body: text.into_bytes(),
                error,
            }),
        },
    )
}

async fn runtime_presidio_redact_text_body(
    text: &str,
    state: &RuntimePresidioRedactionState,
) -> Result<RedactionOutcome> {
    let config = &state.config;
    let client = &state.client;

    if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(text) {
        let content = collect_json_content(&json)?;
        if content.values.is_empty() {
            return Ok(RedactionOutcome {
                body: text.as_bytes().to_vec(),
                source: runtime_presidio_inspection_source(content.coverage, Vec::new(), false)?,
            });
        }

        let separator = presidio_json_value_separator(&content.values);
        let combined = content
            .values
            .iter()
            .map(|value| value.text.as_str())
            .collect::<Vec<_>>()
            .join(&separator);
        let redacted = runtime_presidio_redact_text(
            client,
            &combined,
            config,
            Some((&content.values, &separator)),
        )
        .await?;
        let findings = runtime_presidio_findings(&content.values, &separator, &redacted.findings)?;
        if redacted.text == combined {
            return Ok(RedactionOutcome {
                body: text.as_bytes().to_vec(),
                source: runtime_presidio_inspection_source(content.coverage, findings, false)?,
            });
        }

        let redacted_values = redacted.text.split(&separator).collect::<Vec<_>>();
        if redacted_values.len() != content.values.len() {
            anyhow::bail!(
                "Presidio changed JSON value separator count: expected {}, got {}",
                content.values.len(),
                redacted_values.len()
            );
        }
        let mut redacted_values = redacted_values.into_iter();
        replace_json_string_values(&mut json, false, &mut redacted_values);
        return Ok(RedactionOutcome {
            body: serde_json::to_vec(&json)
                .context("failed to serialize redacted JSON request body")?,
            source: runtime_presidio_inspection_source(content.coverage, findings, true)?,
        });
    }

    let redacted = runtime_presidio_redact_text(client, text, config, None).await?;
    let findings = runtime_presidio_findings(
        &[PresidioJsonString {
            path: "$".to_string(),
            text: text.to_string(),
            sensitive_kind: None,
        }],
        "",
        &redacted.findings,
    )?;
    let changed = redacted.text != text;
    Ok(RedactionOutcome {
        body: redacted.text.into_bytes(),
        source: runtime_presidio_inspection_source(InspectionCoverage::Full, findings, changed)?,
    })
}

async fn runtime_presidio_redact_text(
    client: &reqwest::Client,
    text: &str,
    config: &RuntimePresidioRedactionConfig,
    json_content: Option<(&[PresidioJsonString], &str)>,
) -> Result<RuntimePresidioTextRedaction> {
    let languages = &config.languages;
    let language_mode = config.language_mode;

    let mut all_analyzer_results = Vec::new();

    match language_mode {
        PresidioLanguageMode::Fixed => {
            let results = presidio_analyze_async(
                client,
                &config.analyzer_url,
                text,
                &languages[0],
                config.max_response_bytes,
            )
            .await?;
            all_analyzer_results = results;
        }
        PresidioLanguageMode::Auto => {
            let detected_lang =
                detect_presidio_language(text, languages).unwrap_or_else(|| languages[0].clone());
            let results = presidio_analyze_async(
                client,
                &config.analyzer_url,
                text,
                &detected_lang,
                config.max_response_bytes,
            )
            .await?;
            all_analyzer_results = results;
        }
        PresidioLanguageMode::Multi => {
            for lang in languages {
                let results = presidio_analyze_async(
                    client,
                    &config.analyzer_url,
                    text,
                    lang,
                    config.max_response_bytes,
                )
                .await?;
                all_analyzer_results.extend(results.into_iter().map(|mut r| {
                    r.language = lang.clone();
                    r
                }));
            }
            all_analyzer_results = merge_presidio_analyzer_results(all_analyzer_results);
        }
    }

    if let Some((values, separator)) = json_content {
        retain_findings_within_json_values(&mut all_analyzer_results, values, separator);
    }
    if all_analyzer_results.is_empty() {
        return Ok(RuntimePresidioTextRedaction {
            text: text.to_string(),
            findings: Vec::new(),
        });
    }
    if all_analyzer_results.len() > MAX_INSPECTION_FINDINGS {
        anyhow::bail!("Presidio finding count exceeded safe limit");
    }

    let anonymized_response = client
        .post(presidio_endpoint(&config.anonymizer_url, "anonymize"))
        .json(&serde_json::json!({
            "text": text,
            "analyzer_results": all_analyzer_results,
        }))
        .send()
        .await
        .context("failed to call Presidio Anonymizer")?
        .error_for_status()
        .context("Presidio Anonymizer returned an error")?;
    let anonymized_body = read_async_response_body_with_limit(
        anonymized_response,
        config.max_response_bytes,
        "failed to read Presidio Anonymizer response",
    )
    .await?;
    let anonymized = serde_json::from_slice::<PresidioAnonymizeResponse>(&anonymized_body)
        .context("failed to parse Presidio Anonymizer response")?;
    Ok(RuntimePresidioTextRedaction {
        text: anonymized.text,
        findings: all_analyzer_results,
    })
}

fn retain_findings_within_json_values(
    findings: &mut Vec<PresidioAnalyzerResult>,
    values: &[PresidioJsonString],
    separator: &str,
) {
    let separator_chars = separator.chars().count();
    let mut cursor = 0usize;
    let ranges = values
        .iter()
        .map(|value| {
            let start = cursor;
            let end = start.saturating_add(value.text.chars().count());
            cursor = end.saturating_add(separator_chars);
            (start, end)
        })
        .collect::<Vec<_>>();
    findings.retain(|finding| {
        ranges
            .iter()
            .any(|(start, end)| finding.start >= *start && finding.end <= *end)
    });
}

async fn presidio_analyze_async(
    client: &reqwest::Client,
    analyzer_url: &str,
    text: &str,
    language: &str,
    max_response_bytes: usize,
) -> Result<Vec<PresidioAnalyzerResult>> {
    let response = client
        .post(presidio_endpoint(analyzer_url, "analyze"))
        .json(&serde_json::json!({
            "text": text,
            "language": language,
        }))
        .send()
        .await
        .context("failed to call Presidio Analyzer")?
        .error_for_status()
        .context("Presidio Analyzer returned an error")?;
    let body = read_async_response_body_with_limit(
        response,
        max_response_bytes,
        "failed to read Presidio Analyzer response",
    )
    .await?;
    let results = serde_json::from_slice::<Vec<PresidioAnalyzerResult>>(&body)
        .context("failed to parse Presidio Analyzer response")?;
    Ok(results)
}

fn presidio_endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{}", base_url.trim_end_matches('/'), path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_field_findings_are_removed_before_anonymization() {
        let values = [
            PresidioJsonString {
                path: "$.input[0]".to_string(),
                text: "alpha".to_string(),
                sensitive_kind: None,
            },
            PresidioJsonString {
                path: "$.input[1]".to_string(),
                text: "user@example.com".to_string(),
                sensitive_kind: None,
            },
        ];
        let separator = presidio_json_value_separator(&values);
        let second_start = values[0].text.chars().count() + separator.chars().count();
        let mut findings = vec![
            PresidioAnalyzerResult {
                start: 3,
                end: second_start + 4,
                score: 0.9,
                entity_type: "PERSON".to_string(),
                language: "en".to_string(),
            },
            PresidioAnalyzerResult {
                start: second_start,
                end: second_start + values[1].text.chars().count(),
                score: 0.9,
                entity_type: "EMAIL_ADDRESS".to_string(),
                language: "en".to_string(),
            },
        ];

        retain_findings_within_json_values(&mut findings, &values, &separator);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].entity_type, "EMAIL_ADDRESS");
    }
}
