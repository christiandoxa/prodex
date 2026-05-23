use super::await_runtime_proxy_async_task;
use crate::runtime_core_shared::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use crate::runtime_proxy_log;
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use crate::shared_types::RuntimeProxyRequest;
use anyhow::{Context, Result};
use prodex_core::AppPaths;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const PRODEX_PRESIDIO_FILE_NAME: &str = "presidio.toml";
const DEFAULT_PRESIDIO_ANALYZER_URL: &str = "http://localhost:5002";
const DEFAULT_PRESIDIO_ANONYMIZER_URL: &str = "http://localhost:5001";
const DEFAULT_PRESIDIO_LANGUAGE: &str = "en";
const PRESIDIO_HTTP_TIMEOUT: Duration = Duration::from_secs(10);

static RUNTIME_PRESIDIO_REDACTION_BY_LOG_PATH: OnceLock<
    Mutex<BTreeMap<PathBuf, RuntimePresidioRedactionConfig>>,
> = OnceLock::new();

#[derive(Debug, Clone, serde::Deserialize)]
struct ProdexPresidioRuntimeFileConfig {
    #[serde(default = "default_presidio_analyzer_url")]
    analyzer_url: String,
    #[serde(default = "default_presidio_anonymizer_url")]
    anonymizer_url: String,
    #[serde(default = "default_presidio_language")]
    language: String,
    #[serde(default = "default_presidio_fail_mode")]
    fail_mode: String,
}

impl Default for ProdexPresidioRuntimeFileConfig {
    fn default() -> Self {
        Self {
            analyzer_url: default_presidio_analyzer_url(),
            anonymizer_url: default_presidio_anonymizer_url(),
            language: default_presidio_language(),
            fail_mode: default_presidio_fail_mode(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimePresidioRedactionConfig {
    analyzer_url: String,
    anonymizer_url: String,
    language: String,
    fail_closed: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct PresidioAnalyzerResult {
    start: usize,
    end: usize,
    score: f64,
    entity_type: String,
}

#[derive(Debug, serde::Deserialize)]
struct PresidioAnonymizeResponse {
    text: String,
}

fn default_presidio_analyzer_url() -> String {
    DEFAULT_PRESIDIO_ANALYZER_URL.to_string()
}

fn default_presidio_anonymizer_url() -> String {
    DEFAULT_PRESIDIO_ANONYMIZER_URL.to_string()
}

fn default_presidio_language() -> String {
    DEFAULT_PRESIDIO_LANGUAGE.to_string()
}

fn default_presidio_fail_mode() -> String {
    "open".to_string()
}

pub(crate) fn runtime_presidio_redaction_config(
    paths: &AppPaths,
) -> Result<RuntimePresidioRedactionConfig> {
    let path = paths.root.join(PRODEX_PRESIDIO_FILE_NAME);
    let file_config = if path.exists() {
        toml::from_str::<ProdexPresidioRuntimeFileConfig>(
            &fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?,
        )
        .with_context(|| format!("failed to parse {}", path.display()))?
    } else {
        ProdexPresidioRuntimeFileConfig::default()
    };
    Ok(RuntimePresidioRedactionConfig {
        analyzer_url: file_config.analyzer_url,
        anonymizer_url: file_config.anonymizer_url,
        language: file_config.language,
        fail_closed: file_config.fail_mode.eq_ignore_ascii_case("closed"),
    })
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
    let analyzer_results = client
        .post(presidio_endpoint(&config.analyzer_url, "analyze"))
        .json(&serde_json::json!({
            "text": text,
            "language": config.language,
        }))
        .send()
        .await
        .context("failed to call Presidio Analyzer")?
        .error_for_status()
        .context("Presidio Analyzer returned an error")?
        .json::<Vec<PresidioAnalyzerResult>>()
        .await
        .context("failed to parse Presidio Analyzer response")?;
    if analyzer_results.is_empty() {
        return Ok(text.into_bytes());
    }
    let anonymized = client
        .post(presidio_endpoint(&config.anonymizer_url, "anonymize"))
        .json(&serde_json::json!({
            "text": text,
            "analyzer_results": analyzer_results,
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

fn presidio_endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{}", base_url.trim_end_matches('/'), path)
}

#[cfg(test)]
mod tests {
    use super::{RuntimePresidioRedactionConfig, runtime_presidio_redact_body};
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
            language: "en".to_string(),
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
