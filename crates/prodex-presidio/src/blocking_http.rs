use anyhow::{Context, Result};
use std::io::Read;
use std::time::Duration;

use super::{
    DEFAULT_PRESIDIO_MAX_RESPONSE_BYTES, DEFAULT_PRESIDIO_TIMEOUT_MS, PresidioAnalyzerResult,
    PresidioAnonymizeResponse, PresidioHealth, ProdexPresidioRuntimeFileConfig, presidio_endpoint,
    presidio_redacted_message, validate_presidio_file_config,
};

pub struct PresidioBlockingClient {
    client: reqwest::blocking::Client,
    max_response_bytes: usize,
}

impl PresidioBlockingClient {
    pub fn from_config(config: &ProdexPresidioRuntimeFileConfig) -> Result<Self> {
        validate_presidio_file_config(config)?;
        let client = build_presidio_http_client(config.timeout_ms)?;
        Ok(Self {
            client,
            max_response_bytes: config.max_response_bytes,
        })
    }

    pub fn analyze(
        &self,
        analyzer_url: &str,
        text: &str,
        language: &str,
    ) -> Result<Vec<PresidioAnalyzerResult>> {
        presidio_analyze_with_limit(
            &self.client,
            analyzer_url,
            text,
            language,
            self.max_response_bytes,
        )
    }

    pub fn anonymize(
        &self,
        anonymizer_url: &str,
        text: &str,
        analyzer_results: Vec<PresidioAnalyzerResult>,
    ) -> Result<PresidioAnonymizeResponse> {
        presidio_anonymize_with_limit(
            &self.client,
            anonymizer_url,
            text,
            analyzer_results,
            self.max_response_bytes,
        )
    }

    pub fn probe_health(&self, base_url: &str) -> PresidioHealth {
        probe_presidio_health_with_limit(&self.client, base_url, self.max_response_bytes)
    }
}

pub fn presidio_http_client() -> Result<reqwest::blocking::Client> {
    build_presidio_http_client(DEFAULT_PRESIDIO_TIMEOUT_MS)
}

fn build_presidio_http_client(timeout_ms: u64) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("failed to build Presidio HTTP client")
}

pub fn presidio_analyze(
    client: &reqwest::blocking::Client,
    analyzer_url: &str,
    text: &str,
    language: &str,
) -> Result<Vec<PresidioAnalyzerResult>> {
    presidio_analyze_with_limit(
        client,
        analyzer_url,
        text,
        language,
        DEFAULT_PRESIDIO_MAX_RESPONSE_BYTES,
    )
}

fn presidio_analyze_with_limit(
    client: &reqwest::blocking::Client,
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
        .context("failed to call Presidio Analyzer")?;
    let status = response.status();
    if !status.is_success() {
        let body = read_presidio_text_response(response, max_response_bytes)?;
        anyhow::bail!(
            "Presidio Analyzer returned {status}: {}",
            presidio_redacted_message(body.trim())
        );
    }
    read_presidio_json_response(response, max_response_bytes)
        .context("failed to parse Presidio Analyzer response")
}

pub fn presidio_anonymize(
    client: &reqwest::blocking::Client,
    anonymizer_url: &str,
    text: &str,
    analyzer_results: Vec<PresidioAnalyzerResult>,
) -> Result<PresidioAnonymizeResponse> {
    presidio_anonymize_with_limit(
        client,
        anonymizer_url,
        text,
        analyzer_results,
        DEFAULT_PRESIDIO_MAX_RESPONSE_BYTES,
    )
}

fn presidio_anonymize_with_limit(
    client: &reqwest::blocking::Client,
    anonymizer_url: &str,
    text: &str,
    analyzer_results: Vec<PresidioAnalyzerResult>,
    max_response_bytes: usize,
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
        let body = read_presidio_text_response(response, max_response_bytes)?;
        anyhow::bail!(
            "Presidio Anonymizer returned {status}: {}",
            presidio_redacted_message(body.trim())
        );
    }
    read_presidio_json_response(response, max_response_bytes)
        .context("failed to parse Presidio Anonymizer response")
}

pub fn probe_presidio_health(client: &reqwest::blocking::Client, base_url: &str) -> PresidioHealth {
    probe_presidio_health_with_limit(client, base_url, DEFAULT_PRESIDIO_MAX_RESPONSE_BYTES)
}

fn probe_presidio_health_with_limit(
    client: &reqwest::blocking::Client,
    base_url: &str,
    max_response_bytes: usize,
) -> PresidioHealth {
    match client.get(presidio_endpoint(base_url, "health")).send() {
        Ok(response) => {
            let status = response.status();
            match read_presidio_text_response(response, max_response_bytes) {
                Ok(message) => PresidioHealth {
                    ok: status.is_success(),
                    message: if message.trim().is_empty() {
                        status.to_string()
                    } else {
                        format!("{status} {}", presidio_redacted_message(message.trim()))
                    },
                },
                Err(error) => PresidioHealth {
                    ok: false,
                    message: presidio_redacted_message(&error.to_string()),
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
    max_response_bytes: usize,
) -> Result<T> {
    let body = read_presidio_response_body(response, max_response_bytes)?;
    serde_json::from_slice(&body).context("invalid Presidio JSON response")
}

fn read_presidio_text_response(
    response: reqwest::blocking::Response,
    max_response_bytes: usize,
) -> Result<String> {
    let body = read_presidio_response_body(response, max_response_bytes)?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

fn read_presidio_response_body(
    mut response: reqwest::blocking::Response,
    max_response_bytes: usize,
) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    response
        .by_ref()
        .take((max_response_bytes as u64).saturating_add(1))
        .read_to_end(&mut body)
        .context("failed to read Presidio response")?;
    if body.len() > max_response_bytes {
        anyhow::bail!(
            "Presidio response exceeded safe size limit ({})",
            max_response_bytes
        );
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::{
        PresidioBlockingClient, ProdexPresidioRuntimeFileConfig, presidio_analyze,
        presidio_http_client,
    };
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    #[test]
    fn presidio_client_does_not_follow_redirects_with_inspected_content() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });

        let error = presidio_analyze(
            &presidio_http_client().unwrap(),
            &format!("http://{address}"),
            "synthetic-sensitive-input",
            "en",
        )
        .expect_err("redirects must not receive inspected content")
        .to_string();

        assert!(error.contains("302"), "{error}");
        server.join().unwrap();
    }

    #[test]
    fn config_aware_blocking_client_enforces_timeout_and_response_limit() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).unwrap();
            let result = r#"{"start":0,"end":1,"score":1.0,"entity_type":"PERSON"}"#;
            let body = format!("[{}]", vec![result; 20].join(","));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let config = ProdexPresidioRuntimeFileConfig {
            analyzer_url: format!("http://{address}"),
            max_response_bytes: 1_024,
            ..Default::default()
        };
        let client = PresidioBlockingClient::from_config(&config).unwrap();
        let error = client
            .analyze(&config.analyzer_url, "synthetic-input", "en")
            .unwrap_err();
        let error = format!("{error:#}");
        assert!(error.contains("1,024") || error.contains("1024"), "{error}");
        server.join().unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).unwrap();
            std::thread::sleep(Duration::from_millis(300));
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
        });
        let config = ProdexPresidioRuntimeFileConfig {
            analyzer_url: format!("http://{address}"),
            timeout_ms: 100,
            ..Default::default()
        };
        let client = PresidioBlockingClient::from_config(&config).unwrap();
        let health = client.probe_health(&config.analyzer_url);
        assert!(
            !health.ok,
            "health probe should honor the configured timeout"
        );
        server.join().unwrap();
    }
}
