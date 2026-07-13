use super::await_runtime_proxy_async_task;
use crate::presidio_runtime::PresidioLanguageMode;
use crate::runtime_core_shared::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use crate::runtime_proxy_log;
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use crate::shared_types::RuntimeProxyRequest;
use crate::{RuntimePresidioRedactionConfig, read_async_response_body_with_limit};
use anyhow::{Context, Result, anyhow};
use prodex_application::{
    ApplicationInspectionPlan, ApplicationInspectionRequest, ApplicationInspectionSource,
    plan_application_request_inspection,
};
use prodex_domain::{
    ContentLocation, DataClassification, DetectorId, DetectorRevisionId, FindingKind,
    InspectionCoverage, InspectionFinding, InspectionLimits, InspectionReasonCode, InspectionTag,
    MAX_INSPECTION_FINDINGS,
};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

mod analyzer;
mod json_body;

use analyzer::{detect_presidio_language, merge_presidio_analyzer_results};
use json_body::{
    PresidioJsonString, collect_json_content, presidio_json_value_separator,
    replace_json_string_values,
};

const PRESIDIO_HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const PRESIDIO_RESPONSE_MAX_BYTES: usize = 64 * 1024 * 1024;

static RUNTIME_PRESIDIO_REDACTION_BY_LOG_PATH: OnceLock<
    Mutex<BTreeMap<PathBuf, RuntimePresidioRedactionConfig>>,
> = OnceLock::new();

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct PresidioAnalyzerResult {
    start: usize,
    end: usize,
    score: f64,
    entity_type: String,
    #[serde(default)]
    language: String,
}

#[derive(Debug, serde::Deserialize)]
struct PresidioAnonymizeResponse {
    text: String,
}

struct RuntimePresidioTextRedaction {
    text: String,
    findings: Vec<PresidioAnalyzerResult>,
}

struct RuntimePresidioRedaction {
    body: Vec<u8>,
    source: ApplicationInspectionSource,
}

pub(crate) struct RuntimePresidioWebSocketInspection<'a> {
    pub(crate) text: Cow<'a, str>,
    pub(crate) inspection: ApplicationInspectionPlan,
}

pub(crate) fn register_runtime_presidio_redaction_proxy_state(
    log_path: &Path,
    config: Option<RuntimePresidioRedactionConfig>,
) {
    let registry =
        RUNTIME_PRESIDIO_REDACTION_BY_LOG_PATH.get_or_init(|| Mutex::new(BTreeMap::new()));
    let Ok(mut registry) = registry.lock() else {
        return;
    };
    if let Some(config) = config {
        registry.insert(log_path.to_path_buf(), config);
    } else {
        registry.remove(log_path);
    }
}

pub(crate) fn apply_runtime_presidio_redaction_to_request(
    request_id: u64,
    request: &mut RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<ApplicationInspectionPlan> {
    let Some(config) = runtime_presidio_redaction_for_log_path(&shared.log_path) else {
        return runtime_presidio_inspection_plan(Vec::new());
    };
    if request.body.is_empty() {
        return runtime_presidio_inspection_plan(vec![runtime_presidio_inspection_source(
            InspectionCoverage::Full,
            Vec::new(),
        )?]);
    }
    if std::str::from_utf8(&request.body).is_err() {
        if config.fail_closed {
            return Err(anyhow!("presidio_unsupported_encoding"));
        }
        return runtime_presidio_inspection_plan(vec![runtime_presidio_unavailable_source(
            "inspection.unsupported_encoding",
        )?]);
    }

    let redaction = await_runtime_proxy_async_task(
        shared,
        "presidio_redact_request_body",
        runtime_presidio_redact_body(request.body.clone(), config.clone()),
    );
    match redaction {
        Ok(redaction) => {
            if redaction.body != request.body {
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "presidio_redaction_applied",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "http"),
                            runtime_proxy_log_field(
                                "original_bytes",
                                request.body.len().to_string(),
                            ),
                            runtime_proxy_log_field(
                                "redacted_bytes",
                                redaction.body.len().to_string(),
                            ),
                        ],
                    ),
                );
                request.body = redaction.body;
            }
            runtime_presidio_inspection_plan(vec![redaction.source])
        }
        Err(_err) => {
            let fail_closed = config.fail_closed;
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "presidio_redaction_error",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field(
                            "fail_mode",
                            if fail_closed { "closed" } else { "open" },
                        ),
                        runtime_proxy_log_field("reason", "presidio_redaction_failed"),
                    ],
                ),
            );
            if fail_closed {
                Err(anyhow!("presidio_redaction_failed"))
            } else {
                runtime_presidio_inspection_plan(vec![runtime_presidio_unavailable_source(
                    "presidio.unavailable",
                )?])
            }
        }
    }
}

pub(crate) fn apply_runtime_presidio_redaction_to_websocket_text<'a>(
    request_id: u64,
    text: &'a str,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimePresidioWebSocketInspection<'a>> {
    let Some(config) = runtime_presidio_redaction_for_log_path(&shared.log_path) else {
        let result = RuntimePresidioWebSocketInspection {
            text: Cow::Borrowed(text),
            inspection: runtime_presidio_inspection_plan(Vec::new())?,
        };
        runtime_presidio_log_websocket_inspection(request_id, shared, &result.inspection);
        return Ok(result);
    };
    let redaction = await_runtime_proxy_async_task(
        shared,
        "presidio_redact_websocket_text",
        runtime_presidio_redact_body(text.as_bytes().to_vec(), config.clone()),
    );
    match redaction {
        Ok(redaction) => {
            let redacted = String::from_utf8(redaction.body)
                .context("Presidio returned non-UTF-8 websocket text")?;
            if redacted != text {
                runtime_proxy_log(
                    shared,
                    runtime_proxy_structured_log_message(
                        "presidio_redaction_applied",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "websocket"),
                            runtime_proxy_log_field("original_bytes", text.len().to_string()),
                            runtime_proxy_log_field("redacted_bytes", redacted.len().to_string()),
                        ],
                    ),
                );
                let result = RuntimePresidioWebSocketInspection {
                    text: Cow::Owned(redacted),
                    inspection: runtime_presidio_inspection_plan(vec![redaction.source])?,
                };
                runtime_presidio_log_websocket_inspection(request_id, shared, &result.inspection);
                Ok(result)
            } else {
                let result = RuntimePresidioWebSocketInspection {
                    text: Cow::Borrowed(text),
                    inspection: runtime_presidio_inspection_plan(vec![redaction.source])?,
                };
                runtime_presidio_log_websocket_inspection(request_id, shared, &result.inspection);
                Ok(result)
            }
        }
        Err(_err) => {
            runtime_proxy_log(
                shared,
                runtime_proxy_structured_log_message(
                    "presidio_redaction_error",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "websocket"),
                        runtime_proxy_log_field(
                            "fail_mode",
                            if config.fail_closed { "closed" } else { "open" },
                        ),
                        runtime_proxy_log_field("reason", "presidio_redaction_failed"),
                    ],
                ),
            );
            if config.fail_closed {
                Err(anyhow!("presidio_redaction_failed"))
            } else {
                let result = RuntimePresidioWebSocketInspection {
                    text: Cow::Borrowed(text),
                    inspection: runtime_presidio_inspection_plan(vec![
                        runtime_presidio_unavailable_source("presidio.unavailable")?,
                    ])?,
                };
                runtime_presidio_log_websocket_inspection(request_id, shared, &result.inspection);
                Ok(result)
            }
        }
    }
}

fn runtime_presidio_log_websocket_inspection(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    inspection: &ApplicationInspectionPlan,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "gateway_request_inspection",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "websocket"),
                runtime_proxy_log_field("coverage", inspection.result.coverage().as_str()),
                runtime_proxy_log_field(
                    "classification",
                    inspection.result.classification().as_str(),
                ),
                runtime_proxy_log_field(
                    "finding_count",
                    inspection.result.findings().len().to_string(),
                ),
            ],
        ),
    );
}

fn runtime_presidio_redaction_for_log_path(
    log_path: &Path,
) -> Option<RuntimePresidioRedactionConfig> {
    RUNTIME_PRESIDIO_REDACTION_BY_LOG_PATH
        .get()
        .and_then(|registry| registry.lock().ok()?.get(log_path).cloned())
}

async fn runtime_presidio_redact_body(
    body: Vec<u8>,
    config: RuntimePresidioRedactionConfig,
) -> Result<RuntimePresidioRedaction> {
    let text = String::from_utf8(body).context("request body is not UTF-8")?;
    let client = reqwest::Client::builder()
        .timeout(PRESIDIO_HTTP_TIMEOUT)
        .build()
        .context("failed to build Presidio runtime HTTP client")?;

    if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&text) {
        let content = collect_json_content(&json)?;
        if content.values.is_empty() {
            return Ok(RuntimePresidioRedaction {
                body: text.into_bytes(),
                source: runtime_presidio_inspection_source(content.coverage, Vec::new())?,
            });
        }

        let separator = presidio_json_value_separator(&content.values);
        let combined = content
            .values
            .iter()
            .map(|value| value.text.as_str())
            .collect::<Vec<_>>()
            .join(&separator);
        let redacted = runtime_presidio_redact_text(&client, &combined, &config).await?;
        let findings = runtime_presidio_findings(&content.values, &separator, &redacted.findings)?;
        if redacted.text == combined {
            return Ok(RuntimePresidioRedaction {
                body: text.into_bytes(),
                source: runtime_presidio_inspection_source(content.coverage, findings)?,
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
        return Ok(RuntimePresidioRedaction {
            body: serde_json::to_vec(&json)
                .context("failed to serialize redacted JSON request body")?,
            source: runtime_presidio_inspection_source(content.coverage, findings)?,
        });
    }

    let redacted = runtime_presidio_redact_text(&client, &text, &config).await?;
    let findings = runtime_presidio_findings(
        &[PresidioJsonString {
            path: "$".to_string(),
            text,
        }],
        "",
        &redacted.findings,
    )?;
    Ok(RuntimePresidioRedaction {
        body: redacted.text.into_bytes(),
        source: runtime_presidio_inspection_source(InspectionCoverage::Full, findings)?,
    })
}

async fn runtime_presidio_redact_text(
    client: &reqwest::Client,
    text: &str,
    config: &RuntimePresidioRedactionConfig,
) -> Result<RuntimePresidioTextRedaction> {
    let languages = &config.languages;
    let language_mode = config.language_mode;

    let mut all_analyzer_results = Vec::new();

    match language_mode {
        PresidioLanguageMode::Fixed => {
            let results =
                presidio_analyze_async(client, &config.analyzer_url, text, &languages[0]).await?;
            all_analyzer_results = results;
        }
        PresidioLanguageMode::Auto => {
            let detected_lang =
                detect_presidio_language(text, languages).unwrap_or_else(|| languages[0].clone());
            let results =
                presidio_analyze_async(client, &config.analyzer_url, text, &detected_lang).await?;
            all_analyzer_results = results;
        }
        PresidioLanguageMode::Multi => {
            for lang in languages {
                let results =
                    presidio_analyze_async(client, &config.analyzer_url, text, lang).await?;
                all_analyzer_results.extend(results.into_iter().map(|mut r| {
                    r.language = lang.clone();
                    r
                }));
            }
            all_analyzer_results = merge_presidio_analyzer_results(all_analyzer_results);
        }
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
        PRESIDIO_RESPONSE_MAX_BYTES,
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

fn runtime_presidio_inspection_source(
    coverage: InspectionCoverage,
    findings: Vec<InspectionFinding>,
) -> Result<ApplicationInspectionSource> {
    let has_findings = !findings.is_empty();
    Ok(ApplicationInspectionSource {
        coverage,
        findings,
        tags: has_findings
            .then(|| InspectionTag::new("sensitive-data"))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .context("invalid inspection tag")?,
        reason_codes: has_findings
            .then(|| InspectionReasonCode::new("presidio.finding"))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .context("invalid inspection reason code")?,
    })
}

fn runtime_presidio_unavailable_source(
    reason: &'static str,
) -> Result<ApplicationInspectionSource> {
    Ok(ApplicationInspectionSource {
        coverage: InspectionCoverage::Unsupported,
        findings: Vec::new(),
        tags: Vec::new(),
        reason_codes: vec![
            InspectionReasonCode::new(reason).context("invalid inspection reason code")?,
        ],
    })
}

fn runtime_presidio_inspection_plan(
    sources: Vec<ApplicationInspectionSource>,
) -> Result<ApplicationInspectionPlan> {
    plan_application_request_inspection(ApplicationInspectionRequest {
        sources,
        default_classification: DataClassification::Internal,
        trusted_label: None,
        prior_classification: None,
        detector_revision: DetectorRevisionId::new("runtime-inspection-v1")
            .context("invalid detector revision")?,
        limits: InspectionLimits::default(),
    })
    .context("failed to build inspection plan")
}

fn runtime_presidio_findings(
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

fn unicode_scalar_offset_to_byte(value: &str, offset: usize) -> Option<usize> {
    if offset == value.chars().count() {
        return Some(value.len());
    }
    value.char_indices().nth(offset).map(|(index, _)| index)
}

fn presidio_confidence_basis_points(score: f64) -> Result<u16> {
    if !score.is_finite() || !(0.0..=1.0).contains(&score) {
        anyhow::bail!("Presidio returned an invalid confidence score");
    }
    Ok((score * 10_000.0).round() as u16)
}

fn presidio_finding_kind(entity_type: &str) -> FindingKind {
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

async fn presidio_analyze_async(
    client: &reqwest::Client,
    analyzer_url: &str,
    text: &str,
    language: &str,
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
        PRESIDIO_RESPONSE_MAX_BYTES,
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
    use super::{
        PresidioAnalyzerResult, PresidioJsonString, PresidioLanguageMode,
        RuntimePresidioRedactionConfig, runtime_presidio_findings, runtime_presidio_redact_body,
    };
    use std::thread;
    use tiny_http::{Header as TinyHeader, Response as TinyResponse, Server as TinyServer};
    use tokio::runtime::Builder as TokioRuntimeBuilder;

    fn start_presidio_fixture(
        response_body: &'static str,
        expected_path: &'static str,
        expected_snippet: &'static str,
    ) -> (String, thread::JoinHandle<()>) {
        let server = TinyServer::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_ip().unwrap();
        let handle = thread::spawn(move || {
            let mut request = server.recv().unwrap();
            assert_eq!(request.url(), expected_path);
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).unwrap();
            assert!(body.contains(expected_snippet), "{body}");
            let response = TinyResponse::from_string(response_body)
                .with_header(TinyHeader::from_bytes("Content-Type", "application/json").unwrap());
            request.respond(response).unwrap();
        });
        (format!("http://{addr}"), handle)
    }

    #[test]
    fn runtime_presidio_redact_body_anonymizes_request_payload() {
        let (analyzer_url, analyzer_handle) = start_presidio_fixture(
            r#"[{"start":8,"end":24,"score":0.99,"entity_type":"EMAIL_ADDRESS"}]"#,
            "/analyze",
            "user@example.com",
        );
        let (anonymizer_url, anonymizer_handle) = start_presidio_fixture(
            r#"{"text":"contact <EMAIL_ADDRESS>"}"#,
            "/anonymize",
            "EMAIL_ADDRESS",
        );
        let config = RuntimePresidioRedactionConfig {
            analyzer_url,
            anonymizer_url,
            languages: vec!["en".to_string()],
            language_mode: PresidioLanguageMode::Fixed,
            fail_closed: true,
            trusted_hosts: Vec::new(),
        };
        let runtime = TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let redacted = runtime
            .block_on(runtime_presidio_redact_body(
                br#"{"type":"response.create","input":"contact user@example.com"}"#.to_vec(),
                config,
            ))
            .unwrap();
        assert_eq!(redacted.source.findings.len(), 1);
        let text = String::from_utf8(redacted.body).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(!text.contains("user@example.com"));
        assert_eq!(json["type"], "response.create");
        assert_eq!(json["input"], "contact <EMAIL_ADDRESS>");
        analyzer_handle.join().unwrap();
        anonymizer_handle.join().unwrap();
    }

    #[test]
    fn presidio_unicode_scalar_offsets_become_field_byte_offsets() {
        let findings = runtime_presidio_findings(
            &[PresidioJsonString {
                path: "$.tools[0].arguments.*".to_string(),
                text: "é user@example.com".to_string(),
            }],
            "",
            &[PresidioAnalyzerResult {
                start: 2,
                end: 18,
                score: 0.99,
                entity_type: "EMAIL_ADDRESS".to_string(),
                language: "en".to_string(),
            }],
        )
        .unwrap();

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].location().byte_range(), 3..19);
        assert_eq!(
            findings[0].location().field_path(),
            "$.tools[0].arguments.*"
        );
    }

    #[test]
    fn presidio_rejects_malformed_finding_metadata() {
        let error = runtime_presidio_findings(
            &[PresidioJsonString {
                path: "$.input".to_string(),
                text: "safe".to_string(),
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
