use super::await_runtime_proxy_async_task;
use crate::presidio_runtime::PresidioLanguageMode;
use crate::runtime_core_shared::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use crate::runtime_proxy_log;
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use crate::shared_types::RuntimeProxyRequest;
use crate::{RuntimePresidioRedactionConfig, read_async_response_body_with_limit};
use anyhow::{Context, Result, anyhow};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

mod analyzer;
mod json_body;

use analyzer::{detect_presidio_language, merge_presidio_analyzer_results};
use json_body::{
    collect_json_string_values, presidio_json_value_separator, replace_json_string_values,
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
) -> Result<()> {
    let Some(config) = runtime_presidio_redaction_for_log_path(&shared.log_path) else {
        return Ok(());
    };
    if request.body.is_empty() || std::str::from_utf8(&request.body).is_err() {
        return Ok(());
    }

    let redaction = await_runtime_proxy_async_task(
        shared,
        "presidio_redact_request_body",
        runtime_presidio_redact_body(request.body.clone(), config.clone()),
    );
    match redaction {
        Ok(redacted) => {
            if redacted != request.body {
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
                            runtime_proxy_log_field("redacted_bytes", redacted.len().to_string()),
                        ],
                    ),
                );
                request.body = redacted;
            }
            Ok(())
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
                Ok(())
            }
        }
    }
}

pub(crate) fn apply_runtime_presidio_redaction_to_websocket_text<'a>(
    request_id: u64,
    text: &'a str,
    shared: &RuntimeRotationProxyShared,
) -> Result<Cow<'a, str>> {
    let Some(config) = runtime_presidio_redaction_for_log_path(&shared.log_path) else {
        return Ok(Cow::Borrowed(text));
    };
    let redaction = await_runtime_proxy_async_task(
        shared,
        "presidio_redact_websocket_text",
        runtime_presidio_redact_body(text.as_bytes().to_vec(), config.clone()),
    );
    match redaction {
        Ok(redacted) => {
            let redacted = String::from_utf8(redacted)
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
                Ok(Cow::Owned(redacted))
            } else {
                Ok(Cow::Borrowed(text))
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
                Ok(Cow::Borrowed(text))
            }
        }
    }
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
) -> Result<Vec<u8>> {
    let text = String::from_utf8(body).context("request body is not UTF-8")?;
    let client = reqwest::Client::builder()
        .timeout(PRESIDIO_HTTP_TIMEOUT)
        .build()
        .context("failed to build Presidio runtime HTTP client")?;

    if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&text) {
        let mut values = Vec::new();
        collect_json_string_values(&json, false, &mut values);
        if values.is_empty() {
            return Ok(text.into_bytes());
        }

        let separator = presidio_json_value_separator(&values);
        let combined = values.join(&separator);
        let redacted = runtime_presidio_redact_text(&client, &combined, &config).await?;
        if redacted == combined {
            return Ok(text.into_bytes());
        }

        let redacted_values = redacted.split(&separator).collect::<Vec<_>>();
        if redacted_values.len() != values.len() {
            anyhow::bail!(
                "Presidio changed JSON value separator count: expected {}, got {}",
                values.len(),
                redacted_values.len()
            );
        }
        let mut redacted_values = redacted_values.into_iter();
        replace_json_string_values(&mut json, false, &mut redacted_values);
        return serde_json::to_vec(&json).context("failed to serialize redacted JSON request body");
    }

    Ok(runtime_presidio_redact_text(&client, &text, &config)
        .await?
        .into_bytes())
}

async fn runtime_presidio_redact_text(
    client: &reqwest::Client,
    text: &str,
    config: &RuntimePresidioRedactionConfig,
) -> Result<String> {
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
        return Ok(text.to_string());
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
    Ok(anonymized.text)
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
        PresidioLanguageMode, RuntimePresidioRedactionConfig, runtime_presidio_redact_body,
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
        let text = String::from_utf8(redacted).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(!text.contains("user@example.com"));
        assert_eq!(json["type"], "response.create");
        assert_eq!(json["input"], "contact <EMAIL_ADDRESS>");
        analyzer_handle.join().unwrap();
        anonymizer_handle.join().unwrap();
    }
}
