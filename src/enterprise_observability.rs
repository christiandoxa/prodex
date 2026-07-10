use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

#[derive(Clone, Debug, PartialEq, Eq)]
struct OtlpHttpLogExportConfig {
    endpoint: String,
    headers: Vec<(String, String)>,
    timeout: Option<Duration>,
}

pub fn export_otlp_http_log_if_configured(
    service_name: &str,
    scope_name: &str,
    event_name: &str,
    attributes: Vec<OtlpLogAttribute>,
) -> Result<bool, String> {
    let Some(config) = otlp_http_log_export_config_from_env()? else {
        return Ok(false);
    };
    export_otlp_http_log(&config, service_name, scope_name, event_name, attributes)?;
    Ok(true)
}

pub fn otlp_http_log_export_status(
    service_name: &str,
    scope_name: &str,
    event_name: &str,
    attributes: Vec<OtlpLogAttribute>,
) -> &'static str {
    match export_otlp_http_log_if_configured(service_name, scope_name, event_name, attributes) {
        Ok(true) => "exported",
        Ok(false) => "disabled",
        Err(_) => "failed",
    }
}

fn otlp_http_log_export_config_from_env() -> Result<Option<OtlpHttpLogExportConfig>, String> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("{}/v1/logs", value.trim_end_matches('/')))
        });
    let Some(endpoint) = endpoint else {
        return Ok(None);
    };
    let mut headers = Vec::new();
    if let Ok(raw_headers) = std::env::var("OTEL_EXPORTER_OTLP_HEADERS") {
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
    let timeout = std::env::var("OTEL_EXPORTER_OTLP_LOGS_TIMEOUT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("OTEL_EXPORTER_OTLP_TIMEOUT")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .map(|value| {
            value
                .trim()
                .parse::<u64>()
                .map(Duration::from_millis)
                .map_err(|_| "invalid OTLP timeout: expected integer milliseconds".to_string())
        })
        .transpose()?;
    Ok(Some(OtlpHttpLogExportConfig {
        endpoint,
        headers,
        timeout,
    }))
}

fn export_otlp_http_log(
    config: &OtlpHttpLogExportConfig,
    service_name: &str,
    scope_name: &str,
    event_name: &str,
    attributes: Vec<OtlpLogAttribute>,
) -> Result<(), String> {
    let mut client = Client::builder();
    if let Some(timeout) = config.timeout {
        client = client.timeout(timeout);
    }
    let client = client
        .build()
        .map_err(|err| format!("failed to build OTLP HTTP client: {err}"))?;
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
    client
        .post(&config.endpoint)
        .headers(headers)
        .json(&otlp_http_log_payload(
            service_name,
            scope_name,
            event_name,
            attributes,
        ))
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|err| format!("OTLP log export failed: {err}"))?;
    Ok(())
}

fn otlp_http_log_payload(
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

fn otlp_log_trace_id(attributes: &[OtlpLogAttribute]) -> Option<String> {
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

fn otlp_log_span_id(attributes: &[OtlpLogAttribute]) -> Option<String> {
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

fn otlp_log_trace_flags(attributes: &[OtlpLogAttribute]) -> Option<u32> {
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

fn otlp_any_value(value: Value) -> Value {
    match value {
        Value::String(value) => json!({ "stringValue": value }),
        Value::Bool(value) => json!({ "boolValue": value }),
        Value::Number(value) if value.is_u64() => json!({ "intValue": value.to_string() }),
        Value::Number(value) if value.is_i64() => json!({ "intValue": value.to_string() }),
        Value::Number(value) => json!({ "doubleValue": value.as_f64().unwrap_or_default() }),
        other => json!({ "stringValue": other.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Mutex, OnceLock};
    use std::thread;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_env_vars<T>(updates: &[(&str, Option<&str>)], f: impl FnOnce() -> T) -> T {
        let _guard = env_lock().lock().expect("env lock");
        let previous = updates
            .iter()
            .map(|(key, _)| ((*key).to_string(), std::env::var(key).ok()))
            .collect::<Vec<_>>();
        for (key, value) in updates {
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
        let result = f();
        for (key, value) in previous {
            match value {
                Some(value) => unsafe { std::env::set_var(&key, value) },
                None => unsafe { std::env::remove_var(&key) },
            }
        }
        result
    }

    #[test]
    fn otlp_http_log_payload_uses_service_scope_body_and_attributes() {
        let payload = otlp_http_log_payload(
            "prodex-gateway",
            "prodex.gateway",
            "config_publication.consume",
            vec![
                OtlpLogAttribute::u64("delivered_event_count", 1),
                OtlpLogAttribute::bool("gateway_cache_refreshed", true),
                OtlpLogAttribute::string("trace_id", "4bf92f3577b34da6a3ce929d0e0e4736"),
                OtlpLogAttribute::string("span_id", "00f067aa0ba902b7"),
                OtlpLogAttribute::string("trace_flags", "01"),
            ],
        );

        assert_eq!(
            payload["resourceLogs"][0]["resource"]["attributes"][0]["value"]["stringValue"],
            "prodex-gateway"
        );
        assert_eq!(
            payload["resourceLogs"][0]["resource"]["attributes"][1]["value"]["stringValue"],
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["scope"]["name"],
            "prodex.gateway"
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["scope"]["version"],
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["body"]["stringValue"],
            "config_publication.consume"
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["traceId"],
            "4bf92f3577b34da6a3ce929d0e0e4736"
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["spanId"],
            "00f067aa0ba902b7"
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["flags"],
            1
        );
        let time_unix_nano =
            &payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["timeUnixNano"];
        assert!(time_unix_nano.as_str().unwrap().parse::<u128>().unwrap() > 0);
        assert_eq!(
            &payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["observedTimeUnixNano"],
            time_unix_nano
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["severityNumber"],
            9
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["attributes"][0]["key"],
            "delivered_event_count"
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["attributes"][1]["value"]["boolValue"],
            true
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["attributes"][5]["key"],
            "event.name"
        );
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["attributes"][5]["value"]["stringValue"],
            "config_publication.consume"
        );
    }

    #[test]
    fn export_otlp_http_log_posts_json_payload() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP test listener");
        let addr = listener.local_addr().expect("resolve listener addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept OTLP export");
            let mut header_bytes = Vec::new();
            let mut byte = [0_u8; 1];
            while !header_bytes.ends_with(b"\r\n\r\n") {
                stream
                    .read_exact(&mut byte)
                    .expect("read request header byte");
                header_bytes.push(byte[0]);
            }
            let headers = String::from_utf8(header_bytes).expect("header bytes should be utf8");
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length").then(|| {
                        value
                            .trim()
                            .parse::<usize>()
                            .expect("content-length number")
                    })
                })
                .expect("content-length header");
            let mut body_bytes = vec![0_u8; content_length];
            stream
                .read_exact(&mut body_bytes)
                .expect("read OTLP request body");
            let request = format!(
                "{}{}",
                headers,
                String::from_utf8(body_bytes).expect("body bytes should be utf8")
            );
            assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("content-type: application/json")
            );
            assert!(request.contains("authorization: Bearer test-token"));
            assert!(request.contains("\"key\":\"service.name\""));
            assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
            assert!(request.contains("\"stringValue\":\"config_publication.deliver\""));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write OTLP response");
        });

        export_otlp_http_log(
            &OtlpHttpLogExportConfig {
                endpoint: format!("http://{addr}/v1/logs"),
                headers: vec![("authorization".to_string(), "Bearer test-token".to_string())],
                timeout: None,
            },
            "prodex-control-plane",
            "prodex.control-plane",
            "config_publication.deliver",
            vec![OtlpLogAttribute::u64("runtime_policy_version", 1)],
        )
        .expect("OTLP export should succeed");

        server.join().expect("OTLP server should join");
    }

    #[test]
    fn otlp_http_log_export_config_uses_endpoint_fallback_with_logs_suffix() {
        with_env_vars(
            &[
                ("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
                (
                    "OTEL_EXPORTER_OTLP_ENDPOINT",
                    Some("https://otel.example.test/root/"),
                ),
            ],
            || {
                assert_eq!(
                    otlp_http_log_export_config_from_env(),
                    Ok(Some(OtlpHttpLogExportConfig {
                        endpoint: "https://otel.example.test/root/v1/logs".to_string(),
                        headers: Vec::new(),
                        timeout: None,
                    }))
                );
            },
        );
    }

    #[test]
    fn otlp_http_log_export_config_ignores_empty_endpoint() {
        with_env_vars(
            &[
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
                ("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", Some("   ")),
            ],
            || {
                assert_eq!(otlp_http_log_export_config_from_env(), Ok(None));
            },
        );
    }

    #[test]
    fn otlp_http_log_export_config_falls_back_when_logs_endpoint_is_empty() {
        with_env_vars(
            &[
                ("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", Some("   ")),
                (
                    "OTEL_EXPORTER_OTLP_ENDPOINT",
                    Some("https://otel.example.test/root/"),
                ),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
            ],
            || {
                assert_eq!(
                    otlp_http_log_export_config_from_env(),
                    Ok(Some(OtlpHttpLogExportConfig {
                        endpoint: "https://otel.example.test/root/v1/logs".to_string(),
                        headers: Vec::new(),
                        timeout: None,
                    }))
                );
            },
        );
    }

    #[test]
    fn otlp_http_log_export_config_prefers_logs_endpoint_and_parses_headers() {
        with_env_vars(
            &[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some("https://logs.example.test/custom"),
                ),
                (
                    "OTEL_EXPORTER_OTLP_ENDPOINT",
                    Some("https://fallback.example.test/root"),
                ),
                (
                    "OTEL_EXPORTER_OTLP_HEADERS",
                    Some("authorization=Bearer test-token, empty= , x-tenant = prodex "),
                ),
            ],
            || {
                assert_eq!(
                    otlp_http_log_export_config_from_env(),
                    Ok(Some(OtlpHttpLogExportConfig {
                        endpoint: "https://logs.example.test/custom".to_string(),
                        headers: vec![
                            ("authorization".to_string(), "Bearer test-token".to_string(),),
                            ("x-tenant".to_string(), "prodex".to_string()),
                        ],
                        timeout: None,
                    }))
                );
            },
        );
    }

    #[test]
    fn otlp_http_log_export_config_rejects_malformed_header_entry() {
        with_env_vars(
            &[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some("https://logs.example.test/custom"),
                ),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", Some("authorization")),
            ],
            || {
                let err = otlp_http_log_export_config_from_env()
                    .expect_err("malformed header entry should fail");
                assert!(err.contains("invalid OTLP header entry"));
            },
        );
    }

    #[test]
    fn otlp_http_log_export_config_uses_logs_timeout_before_generic_timeout() {
        with_env_vars(
            &[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some("https://logs.example.test/custom"),
                ),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
                ("OTEL_EXPORTER_OTLP_TIMEOUT", Some("2500")),
                ("OTEL_EXPORTER_OTLP_LOGS_TIMEOUT", Some("15")),
            ],
            || {
                assert_eq!(
                    otlp_http_log_export_config_from_env(),
                    Ok(Some(OtlpHttpLogExportConfig {
                        endpoint: "https://logs.example.test/custom".to_string(),
                        headers: Vec::new(),
                        timeout: Some(Duration::from_millis(15)),
                    }))
                );
            },
        );
    }

    #[test]
    fn otlp_http_log_export_config_rejects_invalid_timeout() {
        with_env_vars(
            &[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some("https://logs.example.test/custom"),
                ),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
                ("OTEL_EXPORTER_OTLP_LOGS_TIMEOUT", Some("slow")),
            ],
            || {
                let err = otlp_http_log_export_config_from_env()
                    .expect_err("invalid timeout should fail");
                assert!(err.contains("invalid OTLP timeout"));
            },
        );
    }

    #[test]
    fn export_otlp_http_log_if_configured_returns_false_when_unconfigured() {
        with_env_vars(
            &[
                ("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
            ],
            || {
                assert_eq!(
                    export_otlp_http_log_if_configured(
                        "prodex-control-plane",
                        "prodex.control-plane",
                        "config_publication.plan",
                        vec![OtlpLogAttribute::bool("authorized", true)],
                    ),
                    Ok(false)
                );
            },
        );
    }

    #[test]
    fn export_otlp_http_log_if_configured_returns_true_when_configured() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP test listener");
        let addr = listener.local_addr().expect("resolve listener addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept OTLP export");
            let mut header_bytes = Vec::new();
            let mut byte = [0_u8; 1];
            while !header_bytes.ends_with(b"\r\n\r\n") {
                stream
                    .read_exact(&mut byte)
                    .expect("read request header byte");
                header_bytes.push(byte[0]);
            }
            let headers = String::from_utf8(header_bytes).expect("header bytes should be utf8");
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length").then(|| {
                        value
                            .trim()
                            .parse::<usize>()
                            .expect("content-length number")
                    })
                })
                .expect("content-length header");
            let mut body_bytes = vec![0_u8; content_length];
            stream
                .read_exact(&mut body_bytes)
                .expect("read OTLP request body");
            let request = format!(
                "{}{}",
                headers,
                String::from_utf8(body_bytes).expect("body bytes should be utf8")
            );
            assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
            assert!(request.contains("\"stringValue\":\"config_publication.plan\""));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write OTLP response");
        });

        with_env_vars(
            &[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some(&format!("http://{addr}/v1/logs")),
                ),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
            ],
            || {
                assert_eq!(
                    export_otlp_http_log_if_configured(
                        "prodex-control-plane",
                        "prodex.control-plane",
                        "config_publication.plan",
                        vec![OtlpLogAttribute::bool("authorized", true)],
                    ),
                    Ok(true)
                );
            },
        );

        server.join().expect("OTLP server should join");
    }

    #[test]
    fn otlp_http_log_export_status_returns_disabled_when_unconfigured() {
        with_env_vars(
            &[
                ("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
            ],
            || {
                assert_eq!(
                    otlp_http_log_export_status(
                        "prodex-control-plane",
                        "prodex.control-plane",
                        "config_publication.plan",
                        vec![OtlpLogAttribute::bool("authorized", true)],
                    ),
                    "disabled"
                );
            },
        );
    }

    #[test]
    fn otlp_http_log_export_status_returns_exported_when_configured() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP test listener");
        let addr = listener.local_addr().expect("resolve listener addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept OTLP export");
            let mut header_bytes = Vec::new();
            let mut byte = [0_u8; 1];
            while !header_bytes.ends_with(b"\r\n\r\n") {
                stream
                    .read_exact(&mut byte)
                    .expect("read request header byte");
                header_bytes.push(byte[0]);
            }
            let headers = String::from_utf8(header_bytes).expect("header bytes should be utf8");
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length").then(|| {
                        value
                            .trim()
                            .parse::<usize>()
                            .expect("content-length number")
                    })
                })
                .expect("content-length header");
            let mut body_bytes = vec![0_u8; content_length];
            stream
                .read_exact(&mut body_bytes)
                .expect("read OTLP request body");
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write OTLP response");
        });

        with_env_vars(
            &[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some(&format!("http://{addr}/v1/logs")),
                ),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
            ],
            || {
                assert_eq!(
                    otlp_http_log_export_status(
                        "prodex-control-plane",
                        "prodex.control-plane",
                        "config_publication.plan",
                        vec![OtlpLogAttribute::bool("authorized", true)],
                    ),
                    "exported"
                );
            },
        );

        server.join().expect("OTLP server should join");
    }

    #[test]
    fn otlp_http_log_export_status_returns_failed_when_export_fails() {
        with_env_vars(
            &[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some("http://127.0.0.1:9/v1/logs"),
                ),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
            ],
            || {
                assert_eq!(
                    otlp_http_log_export_status(
                        "prodex-control-plane",
                        "prodex.control-plane",
                        "config_publication.plan",
                        vec![OtlpLogAttribute::bool("authorized", true)],
                    ),
                    "failed"
                );
            },
        );
    }

    #[test]
    fn export_otlp_http_log_rejects_invalid_header_name() {
        let err = export_otlp_http_log(
            &OtlpHttpLogExportConfig {
                endpoint: "http://127.0.0.1:9/v1/logs".to_string(),
                headers: vec![("bad header".to_string(), "value".to_string())],
                timeout: None,
            },
            "prodex-control-plane",
            "prodex.control-plane",
            "config_publication.plan",
            vec![OtlpLogAttribute::bool("authorized", true)],
        )
        .expect_err("invalid header name should fail");

        assert!(err.contains("invalid OTLP header name"));
    }

    #[test]
    fn export_otlp_http_log_rejects_invalid_header_value() {
        let err = export_otlp_http_log(
            &OtlpHttpLogExportConfig {
                endpoint: "http://127.0.0.1:9/v1/logs".to_string(),
                headers: vec![("authorization".to_string(), "bad\nvalue".to_string())],
                timeout: None,
            },
            "prodex-control-plane",
            "prodex.control-plane",
            "config_publication.plan",
            vec![OtlpLogAttribute::bool("authorized", true)],
        )
        .expect_err("invalid header value should fail");

        assert!(err.contains("invalid OTLP header value"));
    }

    #[test]
    fn export_otlp_http_log_reports_http_error_status() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP test listener");
        let addr = listener.local_addr().expect("resolve listener addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept OTLP export");
            let mut header_bytes = Vec::new();
            let mut byte = [0_u8; 1];
            while !header_bytes.ends_with(b"\r\n\r\n") {
                stream
                    .read_exact(&mut byte)
                    .expect("read request header byte");
                header_bytes.push(byte[0]);
            }
            let headers = String::from_utf8(header_bytes).expect("header bytes should be utf8");
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length").then(|| {
                        value
                            .trim()
                            .parse::<usize>()
                            .expect("content-length number")
                    })
                })
                .expect("content-length header");
            let mut body_bytes = vec![0_u8; content_length];
            stream
                .read_exact(&mut body_bytes)
                .expect("read OTLP request body");
            stream
                .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n")
                .expect("write OTLP error response");
        });

        let err = export_otlp_http_log(
            &OtlpHttpLogExportConfig {
                endpoint: format!("http://{addr}/v1/logs"),
                headers: Vec::new(),
                timeout: None,
            },
            "prodex-control-plane",
            "prodex.control-plane",
            "config_publication.plan",
            vec![OtlpLogAttribute::bool("authorized", true)],
        )
        .expect_err("non-200 response should fail");

        assert!(err.contains("OTLP log export failed"));
        assert!(err.contains("503"));
        server.join().expect("OTLP server should join");
    }

    #[test]
    fn export_otlp_http_log_reports_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP test listener");
        let addr = listener.local_addr().expect("resolve listener addr");
        let server = thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("accept OTLP export");
            std::thread::sleep(Duration::from_millis(100));
        });

        let err = export_otlp_http_log(
            &OtlpHttpLogExportConfig {
                endpoint: format!("http://{addr}/v1/logs"),
                headers: Vec::new(),
                timeout: Some(Duration::from_millis(10)),
            },
            "prodex-control-plane",
            "prodex.control-plane",
            "config_publication.plan",
            vec![OtlpLogAttribute::bool("authorized", true)],
        )
        .expect_err("timeout should fail");

        assert!(err.contains("OTLP log export failed"));
        server.join().expect("OTLP server should join");
    }

    #[test]
    fn export_otlp_http_log_if_configured_rejects_invalid_header_value_from_env() {
        with_env_vars(
            &[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some("http://127.0.0.1:9/v1/logs"),
                ),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                (
                    "OTEL_EXPORTER_OTLP_HEADERS",
                    Some("authorization=bad\nvalue"),
                ),
            ],
            || {
                let err = export_otlp_http_log_if_configured(
                    "prodex-control-plane",
                    "prodex.control-plane",
                    "config_publication.plan",
                    vec![OtlpLogAttribute::bool("authorized", true)],
                )
                .expect_err("invalid header value should fail");

                assert!(err.contains("invalid OTLP header value"));
            },
        );
    }

    #[test]
    fn export_otlp_http_log_if_configured_rejects_invalid_header_name_from_env() {
        with_env_vars(
            &[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some("http://127.0.0.1:9/v1/logs"),
                ),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", Some("bad header=value")),
            ],
            || {
                let err = export_otlp_http_log_if_configured(
                    "prodex-control-plane",
                    "prodex.control-plane",
                    "config_publication.plan",
                    vec![OtlpLogAttribute::bool("authorized", true)],
                )
                .expect_err("invalid header name should fail");

                assert!(err.contains("invalid OTLP header name"));
            },
        );
    }

    #[test]
    fn export_otlp_http_log_if_configured_reports_transport_failure_when_configured() {
        with_env_vars(
            &[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some("http://127.0.0.1:9/v1/logs"),
                ),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
            ],
            || {
                let err = export_otlp_http_log_if_configured(
                    "prodex-control-plane",
                    "prodex.control-plane",
                    "config_publication.plan",
                    vec![OtlpLogAttribute::bool("authorized", true)],
                )
                .expect_err("unreachable configured endpoint should fail");

                assert!(err.contains("OTLP log export failed"));
            },
        );
    }

    #[test]
    fn export_otlp_http_log_if_configured_rejects_malformed_header_entry_from_env() {
        with_env_vars(
            &[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some("http://127.0.0.1:9/v1/logs"),
                ),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", Some("authorization")),
            ],
            || {
                let err = export_otlp_http_log_if_configured(
                    "prodex-control-plane",
                    "prodex.control-plane",
                    "config_publication.plan",
                    vec![OtlpLogAttribute::bool("authorized", true)],
                )
                .expect_err("malformed header entry should fail");

                assert!(err.contains("invalid OTLP header entry"));
            },
        );
    }
}
