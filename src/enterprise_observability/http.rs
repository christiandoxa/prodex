use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use std::fmt;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OtlpLogAttribute {
    key: String,
    value: Value,
}

impl OtlpLogAttribute {
    pub fn string(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: Value::String(value.into()),
        }
    }

    pub fn bool(key: impl Into<String>, value: bool) -> Self {
        Self {
            key: key.into(),
            value: Value::Bool(value),
        }
    }

    pub fn u64(key: impl Into<String>, value: u64) -> Self {
        Self {
            key: key.into(),
            value: json!(value),
        }
    }
}

#[derive(PartialEq, Eq)]
pub(super) struct OtlpHttpLogExportConfig {
    pub(super) endpoint: String,
    pub(super) headers: Vec<(String, String)>,
    pub(super) timeout: Option<Duration>,
}

impl fmt::Debug for OtlpHttpLogExportConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OtlpHttpLogExportConfig")
            .field("endpoint", &"<redacted>")
            .field("header_count", &self.headers.len())
            .field("headers", &"<redacted>")
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl Zeroize for OtlpHttpLogExportConfig {
    fn zeroize(&mut self) {
        self.endpoint.zeroize();
        for (_, value) in &mut self.headers {
            value.zeroize();
        }
        self.headers.clear();
    }
}

impl Drop for OtlpHttpLogExportConfig {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for OtlpHttpLogExportConfig {}

pub(super) const INVALID_OTLP_HTTP_ENDPOINT: &str = "invalid OTLP HTTP endpoint";
const MAX_OTLP_HTTP_ENDPOINT_BYTES: usize = 4 * 1024;

pub(super) fn validate_otlp_http_endpoint(endpoint: &str) -> Result<reqwest::Url, String> {
    if endpoint.is_empty()
        || endpoint.len() > MAX_OTLP_HTTP_ENDPOINT_BYTES
        || endpoint.chars().any(char::is_whitespace)
    {
        return Err(INVALID_OTLP_HTTP_ENDPOINT.to_string());
    }
    let parsed =
        reqwest::Url::parse(endpoint).map_err(|_| INVALID_OTLP_HTTP_ENDPOINT.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(INVALID_OTLP_HTTP_ENDPOINT.to_string());
    }
    Ok(parsed)
}

pub fn export_otlp_http_log_if_configured(
    service_name: &str,
    scope_name: &str,
    event_name: &str,
    attributes: Vec<OtlpLogAttribute>,
) -> Result<bool, String> {
    let config = otlp_http_log_export_config_from_env()?;
    export_otlp_http_log_with_config(
        config.as_ref(),
        service_name,
        scope_name,
        event_name,
        attributes,
    )
}

pub(super) fn export_otlp_http_log_with_config(
    config: Option<&OtlpHttpLogExportConfig>,
    service_name: &str,
    scope_name: &str,
    event_name: &str,
    attributes: Vec<OtlpLogAttribute>,
) -> Result<bool, String> {
    let Some(config) = config else {
        return Ok(false);
    };
    export_otlp_http_log(config, service_name, scope_name, event_name, attributes)?;
    Ok(true)
}

pub fn otlp_http_log_export_status(
    service_name: &str,
    scope_name: &str,
    event_name: &str,
    attributes: Vec<OtlpLogAttribute>,
) -> &'static str {
    let result = otlp_http_log_export_config_from_env().and_then(|config| {
        export_otlp_http_log_with_config(
            config.as_ref(),
            service_name,
            scope_name,
            event_name,
            attributes,
        )
    });
    otlp_http_log_export_status_for(result)
}

pub(super) fn otlp_http_log_export_status_for(result: Result<bool, String>) -> &'static str {
    match result {
        Ok(true) => "exported",
        Ok(false) => "disabled",
        Err(_) => "failed",
    }
}

pub(super) fn otlp_http_log_export_config_from_env()
-> Result<Option<OtlpHttpLogExportConfig>, String> {
    otlp_http_log_export_config_from(|key| std::env::var(key).ok())
}

pub(super) fn otlp_http_log_export_config_from(
    mut value: impl FnMut(&str) -> Option<String>,
) -> Result<Option<OtlpHttpLogExportConfig>, String> {
    let endpoint = value("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            value("OTEL_EXPORTER_OTLP_ENDPOINT")
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("{}/v1/logs", value.trim_end_matches('/')))
        });
    let Some(endpoint) = endpoint else {
        return Ok(None);
    };
    let endpoint = Zeroizing::new(endpoint);
    validate_otlp_http_endpoint(&endpoint)?;
    let mut headers = Vec::new();
    if let Some(raw_headers) = value("OTEL_EXPORTER_OTLP_HEADERS") {
        let raw_headers = Zeroizing::new(raw_headers);
        for entry in raw_headers.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let (key, value) = entry
                .split_once('=')
                .ok_or_else(|| "invalid OTLP header entry: expected key=value".to_string())?;
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                continue;
            }
            headers.push((key.to_string(), value.to_string()));
        }
    }
    let timeout = value("OTEL_EXPORTER_OTLP_LOGS_TIMEOUT")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| value("OTEL_EXPORTER_OTLP_TIMEOUT").filter(|value| !value.trim().is_empty()))
        .map(|value| {
            value
                .trim()
                .parse::<u64>()
                .map(Duration::from_millis)
                .map_err(|_| "invalid OTLP timeout: expected integer milliseconds".to_string())
        })
        .transpose()?;
    Ok(Some(OtlpHttpLogExportConfig {
        endpoint: endpoint.to_string(),
        headers,
        timeout,
    }))
}

fn otlp_http_client() -> Result<&'static Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .build()
                .map_err(|err| format!("failed to build OTLP HTTP client: {err}"))
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(super) fn export_otlp_http_log(
    config: &OtlpHttpLogExportConfig,
    service_name: &str,
    scope_name: &str,
    event_name: &str,
    attributes: Vec<OtlpLogAttribute>,
) -> Result<(), String> {
    let endpoint = validate_otlp_http_endpoint(&config.endpoint)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    for (name, value) in &config.headers {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|err| format!("invalid OTLP header name: {err}"))?;
        let header_value = HeaderValue::from_str(value)
            .map_err(|err| format!("invalid OTLP header value: {err}"))?;
        headers.insert(header_name, header_value);
    }
    let mut request =
        otlp_http_client()?
            .post(endpoint)
            .headers(headers)
            .json(&otlp_http_log_payload(
                service_name,
                scope_name,
                event_name,
                attributes,
            ));
    if let Some(timeout) = config.timeout {
        request = request.timeout(timeout);
    }
    request
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|err| format!("OTLP log export failed: {err}"))?;
    Ok(())
}

pub(super) fn otlp_http_log_payload(
    service_name: &str,
    scope_name: &str,
    event_name: &str,
    attributes: Vec<OtlpLogAttribute>,
) -> Value {
    let time_unix_nano = current_unix_time_nanos();
    let trace_id = otlp_log_trace_id(&attributes);
    let span_id = otlp_log_span_id(&attributes);
    let trace_flags = otlp_log_trace_flags(&attributes);
    let mut attributes = attributes;
    attributes.push(OtlpLogAttribute::string("event.name", event_name));
    let attributes = attributes
        .into_iter()
        .map(|attribute| {
            json!({
                "key": attribute.key,
                "value": otlp_any_value(attribute.value),
            })
        })
        .collect::<Vec<_>>();
    let mut log_record = json!({
        "timeUnixNano": &time_unix_nano,
        "observedTimeUnixNano": &time_unix_nano,
        "severityNumber": 9,
        "severityText": "INFO",
        "body": {"stringValue": event_name},
        "attributes": attributes,
    });
    if let Some(trace_id) = trace_id {
        log_record["traceId"] = Value::String(trace_id);
    }
    if let Some(span_id) = span_id {
        log_record["spanId"] = Value::String(span_id);
    }
    if let Some(trace_flags) = trace_flags {
        log_record["flags"] = json!(trace_flags);
    }
    json!({
        "resourceLogs": [{
            "resource": {
                "attributes": [
                    {
                        "key": "service.name",
                        "value": {"stringValue": service_name}
                    },
                    {
                        "key": "service.version",
                        "value": {"stringValue": env!("CARGO_PKG_VERSION")}
                    }
                ]
            },
            "scopeLogs": [{
                "scope": {
                    "name": scope_name,
                    "version": env!("CARGO_PKG_VERSION")
                },
                "logRecords": [log_record]
            }]
        }]
    })
}

pub(super) fn otlp_log_trace_id(attributes: &[OtlpLogAttribute]) -> Option<String> {
    attributes
        .iter()
        .find(|attribute| attribute.key == "trace_id")
        .and_then(|attribute| attribute.value.as_str())
        .filter(|value| is_valid_otlp_trace_id(value))
        .map(str::to_string)
}

fn is_valid_otlp_trace_id(value: &str) -> bool {
    value.len() == 32
        && value.bytes().any(|byte| byte != b'0')
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn otlp_log_span_id(attributes: &[OtlpLogAttribute]) -> Option<String> {
    attributes
        .iter()
        .find(|attribute| attribute.key == "span_id")
        .and_then(|attribute| attribute.value.as_str())
        .filter(|value| is_valid_otlp_span_id(value))
        .map(str::to_string)
}

fn is_valid_otlp_span_id(value: &str) -> bool {
    value.len() == 16
        && value.bytes().any(|byte| byte != b'0')
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn otlp_log_trace_flags(attributes: &[OtlpLogAttribute]) -> Option<u32> {
    attributes
        .iter()
        .find(|attribute| attribute.key == "trace_flags")
        .and_then(|attribute| attribute.value.as_str())
        .filter(|value| value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .and_then(|value| u32::from_str_radix(value, 16).ok())
}

fn current_unix_time_nanos() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}

pub(super) fn otlp_any_value(value: Value) -> Value {
    match value {
        Value::String(value) => json!({ "stringValue": value }),
        Value::Bool(value) => json!({ "boolValue": value }),
        Value::Number(value) if value.is_u64() => json!({ "intValue": value.to_string() }),
        Value::Number(value) if value.is_i64() => json!({ "intValue": value.to_string() }),
        Value::Number(value) => json!({ "doubleValue": value.as_f64().unwrap_or_default() }),
        other => json!({ "stringValue": other.to_string() }),
    }
}
