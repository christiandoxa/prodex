use anyhow::{Context, Result};
pub use prodex_cli::PresidioLanguageMode;
use redaction::redaction_redact_secret_like_text;
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

const PRODEX_PRESIDIO_FILE_NAME: &str = "presidio.toml";
const DEFAULT_PRESIDIO_ANALYZER_URL: &str = "http://localhost:5002";
const DEFAULT_PRESIDIO_ANONYMIZER_URL: &str = "http://localhost:5001";
const DEFAULT_PRESIDIO_LANGUAGE: &str = "en";
const PRESIDIO_HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const PRESIDIO_RESPONSE_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, serde::Deserialize)]
struct ProdexPresidioRuntimeFileConfig {
    #[serde(default = "default_presidio_analyzer_url")]
    analyzer_url: String,
    #[serde(default = "default_presidio_anonymizer_url")]
    anonymizer_url: String,
    language: Option<String>,
    languages: Option<Vec<String>>,
    #[serde(
        default = "default_presidio_language_mode_str",
        deserialize_with = "deserialize_presidio_language_mode"
    )]
    language_mode: PresidioLanguageMode,
    #[serde(default = "default_presidio_fail_mode")]
    fail_mode: String,
}

impl Default for ProdexPresidioRuntimeFileConfig {
    fn default() -> Self {
        Self {
            analyzer_url: default_presidio_analyzer_url(),
            anonymizer_url: default_presidio_anonymizer_url(),
            language: None,
            languages: None,
            language_mode: PresidioLanguageMode::default(),
            fail_mode: default_presidio_fail_mode(),
        }
    }
}

#[derive(Clone)]
pub struct RuntimePresidioRedactionConfig {
    pub analyzer_url: String,
    pub anonymizer_url: String,
    pub languages: Vec<String>,
    pub language_mode: PresidioLanguageMode,
    pub fail_closed: bool,
}

impl fmt::Debug for RuntimePresidioRedactionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimePresidioRedactionConfig")
            .field("analyzer_url", &"<redacted>")
            .field("anonymizer_url", &"<redacted>")
            .field("languages", &self.languages)
            .field("language_mode", &self.language_mode)
            .field("fail_closed", &self.fail_closed)
            .finish()
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PresidioAnalyzerResult {
    pub start: usize,
    pub end: usize,
    pub score: f64,
    pub entity_type: String,
    #[serde(default)]
    pub language: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PresidioAnonymizeResponse {
    pub text: String,
    #[serde(default)]
    pub items: Vec<serde_json::Value>,
}

#[derive(Debug)]
pub struct PresidioHealth {
    pub ok: bool,
    pub message: String,
}

fn default_presidio_language_mode_str() -> PresidioLanguageMode {
    PresidioLanguageMode::Fixed
}

fn deserialize_presidio_language_mode<'de, D>(
    deserializer: D,
) -> Result<PresidioLanguageMode, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "fixed" => Ok(PresidioLanguageMode::Fixed),
        "auto" => Ok(PresidioLanguageMode::Auto),
        "multi" => Ok(PresidioLanguageMode::Multi),
        _ => Err(serde::de::Error::custom(format!(
            "unknown Presidio language mode: {}",
            s
        ))),
    }
}

fn default_presidio_analyzer_url() -> String {
    DEFAULT_PRESIDIO_ANALYZER_URL.to_string()
}

fn default_presidio_anonymizer_url() -> String {
    DEFAULT_PRESIDIO_ANONYMIZER_URL.to_string()
}

fn default_presidio_fail_mode() -> String {
    "open".to_string()
}

pub fn runtime_presidio_redaction_config(
    prodex_home: &Path,
) -> Result<RuntimePresidioRedactionConfig> {
    let path = prodex_home.join(PRODEX_PRESIDIO_FILE_NAME);
    let file_config = if path.exists() {
        toml::from_str::<ProdexPresidioRuntimeFileConfig>(
            &fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?,
        )
        .with_context(|| format!("failed to parse {}", path.display()))?
    } else {
        ProdexPresidioRuntimeFileConfig::default()
    };
    validate_presidio_url(&file_config.analyzer_url, "analyzer_url")?;
    validate_presidio_url(&file_config.anonymizer_url, "anonymizer_url")?;

    let languages = file_config.languages.unwrap_or_else(|| {
        file_config
            .language
            .map(|l| vec![l])
            .unwrap_or_else(|| vec![DEFAULT_PRESIDIO_LANGUAGE.to_string()])
    });

    let language_mode = file_config.language_mode;

    if language_mode == PresidioLanguageMode::Fixed && languages.len() != 1 {
        anyhow::bail!(
            "Fixed Presidio language mode requires exactly one language, found: {:?}",
            languages
        );
    }

    Ok(RuntimePresidioRedactionConfig {
        analyzer_url: file_config.analyzer_url,
        anonymizer_url: file_config.anonymizer_url,
        languages,
        language_mode,
        fail_closed: file_config.fail_mode.eq_ignore_ascii_case("closed"),
    })
}

pub fn presidio_http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(PRESIDIO_HTTP_TIMEOUT)
        .no_proxy()
        .build()
        .context("failed to build Presidio HTTP client")
}

pub fn presidio_analyze(
    client: &reqwest::blocking::Client,
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
        .context("failed to call Presidio Analyzer")?;
    let status = response.status();
    if !status.is_success() {
        let body = read_presidio_text_response(response).unwrap_or_default();
        anyhow::bail!(
            "Presidio Analyzer returned {status}: {}",
            presidio_redacted_message(body.trim())
        );
    }
    read_presidio_json_response(response).context("failed to parse Presidio Analyzer response")
}

pub fn presidio_anonymize(
    client: &reqwest::blocking::Client,
    anonymizer_url: &str,
    text: &str,
    analyzer_results: Vec<PresidioAnalyzerResult>,
) -> Result<PresidioAnonymizeResponse> {
    let response = client
        .post(presidio_endpoint(anonymizer_url, "anonymize"))
        .json(&serde_json::json!({
            "text": text,
            "analyzer_results": analyzer_results,
        }))
        .send()
        .context("failed to call Presidio Anonymizer")?;
    let status = response.status();
    if !status.is_success() {
        let body = read_presidio_text_response(response).unwrap_or_default();
        anyhow::bail!(
            "Presidio Anonymizer returned {status}: {}",
            presidio_redacted_message(body.trim())
        );
    }
    read_presidio_json_response(response).context("failed to parse Presidio Anonymizer response")
}

pub fn probe_presidio_health(client: &reqwest::blocking::Client, base_url: &str) -> PresidioHealth {
    match client.get(presidio_endpoint(base_url, "health")).send() {
        Ok(response) => {
            let status = response.status();
            let message = read_presidio_text_response(response).unwrap_or_default();
            PresidioHealth {
                ok: status.is_success(),
                message: if message.trim().is_empty() {
                    status.to_string()
                } else {
                    format!("{status} {}", presidio_redacted_message(message.trim()))
                },
            }
        }
        Err(err) => PresidioHealth {
            ok: false,
            message: presidio_redacted_message(&err.to_string()),
        },
    }
}

fn read_presidio_json_response<T: serde::de::DeserializeOwned>(
    response: reqwest::blocking::Response,
) -> Result<T> {
    let body = read_presidio_response_body(response)?;
    serde_json::from_slice(&body).context("invalid Presidio JSON response")
}

fn read_presidio_text_response(response: reqwest::blocking::Response) -> Result<String> {
    let body = read_presidio_response_body(response)?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

fn read_presidio_response_body(mut response: reqwest::blocking::Response) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    response
        .by_ref()
        .take((PRESIDIO_RESPONSE_MAX_BYTES as u64).saturating_add(1))
        .read_to_end(&mut body)
        .context("failed to read Presidio response")?;
    if body.len() > PRESIDIO_RESPONSE_MAX_BYTES {
        anyhow::bail!(
            "Presidio response exceeded safe size limit ({})",
            PRESIDIO_RESPONSE_MAX_BYTES
        );
    }
    Ok(body)
}

pub fn validate_presidio_url(url: &str, field: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).with_context(|| {
        format!(
            "invalid {field}: expected an http(s) URL with host and no credentials, query, or fragment"
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        anyhow::bail!(
            "invalid {field}: expected an http(s) URL with host and no credentials, query, or fragment"
        );
    }
    Ok(())
}

pub fn presidio_endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{}", base_url.trim_end_matches('/'), path)
}

fn presidio_redacted_message(value: &str) -> String {
    redaction_redact_secret_like_text(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn presidio_error_message_redacts_secret_like_material() {
        let message = presidio_redacted_message(
            "failed: Authorization: Bearer fixture-token-123 url=https://example.test?api_key=sk-fixture-123",
        );

        assert!(message.contains("Authorization: Bearer <redacted>"));
        assert!(message.contains("api_key=<redacted>"));
        assert!(!message.contains("fixture-token-123"));
        assert!(!message.contains("sk-fixture-123"));
    }

    #[test]
    fn presidio_urls_reject_credentials_query_and_fragment_without_echoing_values() {
        for value in [
            "https://presidio-user-secret-sentinel@example.test/analyze",
            "https://user:presidio-password-secret-sentinel@example.test/analyze",
            "https://example.test/analyze?token=presidio-query-secret-sentinel",
            "https://example.test/analyze#presidio-fragment-secret-sentinel",
        ] {
            let error = validate_presidio_url(value, "analyzer_url")
                .unwrap_err()
                .to_string();

            assert!(
                error.contains("no credentials, query, or fragment"),
                "{error}"
            );
            assert!(!error.contains("secret-sentinel"), "{error}");
        }

        validate_presidio_url("https://example.test/analyze/", "analyzer_url").unwrap();
    }

    #[test]
    fn runtime_presidio_config_debug_redacts_endpoint_values() {
        let config = RuntimePresidioRedactionConfig {
            analyzer_url: "https://example.test/analyzer-debug-sentinel".to_string(),
            anonymizer_url: "https://example.test/anonymizer-debug-sentinel".to_string(),
            languages: vec!["en".to_string()],
            language_mode: PresidioLanguageMode::Fixed,
            fail_closed: true,
        };

        let rendered = format!("{config:?}");
        assert!(rendered.contains("<redacted>"), "{rendered}");
        assert!(!rendered.contains("analyzer-debug-sentinel"), "{rendered}");
        assert!(
            !rendered.contains("anonymizer-debug-sentinel"),
            "{rendered}"
        );
    }

    #[test]
    fn runtime_presidio_config_rejects_endpoint_secrets_before_returning_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "prodex-presidio-url-boundary-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        fs::create_dir_all(&root).unwrap();
        let sentinel = "presidio-config-secret-sentinel";
        fs::write(
            root.join(PRODEX_PRESIDIO_FILE_NAME),
            format!(
                "analyzer_url = \"https://user:{sentinel}@example.test/analyze\"\n\
                 anonymizer_url = \"https://example.test/anonymize\"\n"
            ),
        )
        .unwrap();

        let error = runtime_presidio_redaction_config(&root)
            .unwrap_err()
            .to_string();

        assert!(error.contains("analyzer_url"), "{error}");
        assert!(!error.contains(sentinel), "{error}");
        let _ = fs::remove_dir_all(root);
    }
}
