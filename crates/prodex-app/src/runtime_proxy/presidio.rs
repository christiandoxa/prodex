use super::await_runtime_proxy_async_task;
use crate::RuntimePresidioRedactionConfig;
use crate::presidio_runtime::PresidioLanguageMode;
use crate::runtime_core_shared::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use crate::runtime_proxy_log;
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use crate::shared_types::RuntimeProxyRequest;
use anyhow::{Context, Result};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const PRESIDIO_HTTP_TIMEOUT: Duration = Duration::from_secs(10);

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
        Err(err) => {
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
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
            if fail_closed {
                Err(err.context("Presidio request redaction failed"))
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
        Err(err) => {
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
                        runtime_proxy_log_field("error", err.to_string()),
                    ],
                ),
            );
            if config.fail_closed {
                Err(err.context("Presidio websocket redaction failed"))
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

    let languages = config.languages;
    let language_mode = config.language_mode;

    let mut all_analyzer_results = Vec::new();

    match language_mode {
        PresidioLanguageMode::Fixed => {
            let results =
                presidio_analyze_async(&client, &config.analyzer_url, &text, &languages[0]).await?;
            all_analyzer_results = results;
        }
        PresidioLanguageMode::Auto => {
            let detected_lang =
                detect_presidio_language(&text, &languages).unwrap_or_else(|| languages[0].clone());
            let results =
                presidio_analyze_async(&client, &config.analyzer_url, &text, &detected_lang)
                    .await?;
            all_analyzer_results = results;
        }
        PresidioLanguageMode::Multi => {
            for lang in &languages {
                let results =
                    presidio_analyze_async(&client, &config.analyzer_url, &text, lang).await?;
                all_analyzer_results.extend(results.into_iter().map(|mut r| {
                    r.language = lang.clone();
                    r
                }));
            }
            all_analyzer_results = merge_presidio_analyzer_results(all_analyzer_results);
        }
    }

    if all_analyzer_results.is_empty() {
        return Ok(text.into_bytes());
    }

    let anonymized = client
        .post(presidio_endpoint(&config.anonymizer_url, "anonymize"))
        .json(&serde_json::json!({
            "text": text,
            "analyzer_results": all_analyzer_results,
        }))
        .send()
        .await
        .context("failed to call Presidio Anonymizer")?
        .error_for_status()
        .context("Presidio Anonymizer returned an error")?
        .json::<PresidioAnonymizeResponse>()
        .await
        .context("failed to parse Presidio Anonymizer response")?;
    Ok(anonymized.text.into_bytes())
}

async fn presidio_analyze_async(
    client: &reqwest::Client,
    analyzer_url: &str,
    text: &str,
    language: &str,
) -> Result<Vec<PresidioAnalyzerResult>> {
    let results = client
        .post(presidio_endpoint(analyzer_url, "analyze"))
        .json(&serde_json::json!({
            "text": text,
            "language": language,
        }))
        .send()
        .await
        .context("failed to call Presidio Analyzer")?
        .error_for_status()
        .context("Presidio Analyzer returned an error")?
        .json::<Vec<PresidioAnalyzerResult>>()
        .await
        .context("failed to parse Presidio Analyzer response")?;
    Ok(results)
}

fn presidio_endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{}", base_url.trim_end_matches('/'), path)
}

fn detect_presidio_language(text: &str, candidates: &[String]) -> Option<String> {
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }

    let id_keywords = [
        "yang", "dan", "di", "ke", "dari", "saya", "kami", "anda", "nomor", "nama", "alamat",
        "tanggal", "lahir", "dengan", "untuk",
    ];
    let en_keywords = [
        "the", "and", "to", "from", "my", "name", "phone", "email", "address", "with", "for",
        "birth",
    ];

    let mut id_score = 0;
    let mut en_score = 0;

    let lower_text = text.to_lowercase();

    for keyword in id_keywords.iter() {
        if lower_text.contains(keyword) {
            id_score += 1;
        }
    }
    for keyword in en_keywords.iter() {
        if lower_text.contains(keyword) {
            en_score += 1;
        }
    }

    if id_score > en_score && candidates.contains(&"id".to_string()) {
        Some("id".to_string())
    } else if en_score > id_score && candidates.contains(&"en".to_string()) {
        Some("en".to_string())
    } else {
        candidates.first().cloned()
    }
}

fn merge_presidio_analyzer_results(
    mut results: Vec<PresidioAnalyzerResult>,
) -> Vec<PresidioAnalyzerResult> {
    results.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| a.end.cmp(&b.end))
            .then_with(|| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| b.entity_type.cmp(&a.entity_type))
    });

    let mut merged: Vec<PresidioAnalyzerResult> = Vec::new();
    for result in results {
        if let Some(last) = merged.last_mut() {
            if last.start == result.start
                && last.end == result.end
                && last.entity_type == result.entity_type
            {
                if result.score > last.score {
                    *last = result;
                }
                continue;
            }

            let overlaps = result.start < last.end && result.end > last.start;
            if overlaps {
                if result.score > last.score
                    || (result.score == last.score
                        && (result.end - result.start) > (last.end - last.start))
                {
                    if (result.start >= last.start && result.end <= last.end)
                        || (last.start >= result.start && last.end <= result.end)
                    {
                        if result.score > last.score {
                            *last = result;
                        }
                        continue;
                    } else if result.score > last.score {
                        last.start = last.start.min(result.start);
                        last.end = last.end.max(result.end);
                        last.score = result.score;
                        last.entity_type = result.entity_type;
                        last.language = result.language;
                        continue;
                    }
                }
            }
        }
        merged.push(result);
    }
    merged
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
            r#"[{"start":22,"end":38,"score":0.99,"entity_type":"EMAIL_ADDRESS"}]"#,
            "/analyze",
            "user@example.com",
        );
        let (anonymizer_url, anonymizer_handle) = start_presidio_fixture(
            r#"{"text":"{\"input\":\"contact <EMAIL_ADDRESS>\"}"}"#,
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
                br#"{"input":"contact user@example.com"}"#.to_vec(),
                config,
            ))
            .unwrap();
        let text = String::from_utf8(redacted).unwrap();
        assert!(!text.contains("user@example.com"));
        assert!(text.contains("<EMAIL_ADDRESS>"));
        analyzer_handle.join().unwrap();
        anonymizer_handle.join().unwrap();
    }
}
