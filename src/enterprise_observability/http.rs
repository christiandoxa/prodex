use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use std::fmt;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{SyncSender, TrySendError, sync_channel},
};
use std::thread::JoinHandle;
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
const DEFAULT_OTLP_HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const OTLP_HTTP_LOG_QUEUE_CAPACITY: usize = 256;

struct OtlpHttpLogEvent {
    event_name: &'static str,
    attributes: Vec<OtlpLogAttribute>,
}

#[derive(Default)]
struct OtlpHttpLogSinkStats {
    queue_full: AtomicU64,
    disconnected: AtomicU64,
    export_failed: AtomicU64,
    shutdown_dropped: AtomicU64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct OtlpHttpLogSinkStatsSnapshot {
    queue_full: u64,
    disconnected: u64,
    export_failed: u64,
    shutdown_dropped: u64,
}

pub(crate) struct OtlpHttpLogSink {
    sender: Option<SyncSender<OtlpHttpLogEvent>>,
    shutdown: Arc<AtomicBool>,
    stats: Arc<OtlpHttpLogSinkStats>,
    worker: Option<JoinHandle<()>>,
}

impl OtlpHttpLogSink {
    pub(crate) fn from_env(
        service_name: &'static str,
        scope_name: &'static str,
    ) -> Result<Option<Self>, String> {
        let Some(config) = otlp_http_log_export_config_from_env()? else {
            return Ok(None);
        };
        Self::start(config, service_name, scope_name).map(Some)
    }

    pub(super) fn start(
        config: OtlpHttpLogExportConfig,
        service_name: &'static str,
        scope_name: &'static str,
    ) -> Result<Self, String> {
        let (sender, receiver) = sync_channel::<OtlpHttpLogEvent>(OTLP_HTTP_LOG_QUEUE_CAPACITY);
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = Arc::clone(&shutdown);
        let stats = Arc::new(OtlpHttpLogSinkStats::default());
        let worker_stats = Arc::clone(&stats);
        let worker = std::thread::Builder::new()
            .name("prodex-otlp-log-export".to_string())
            .spawn(move || {
                let mut next_queue_full_report = 1;
                while let Ok(event) = receiver.recv() {
                    let failed = export_otlp_http_log(
                        &config,
                        service_name,
                        scope_name,
                        event.event_name,
                        event.attributes,
                    )
                    .is_err();
                    if failed {
                        let count = worker_stats.export_failed.fetch_add(1, Ordering::Relaxed) + 1;
                        if count.is_power_of_two() {
                            eprintln!(
                                "prodex_otlp_log_export_degraded reason=exporter_unavailable count={count}"
                            );
                        }
                    }
                    let queue_full = worker_stats.queue_full.load(Ordering::Relaxed);
                    if queue_full >= next_queue_full_report {
                        eprintln!(
                            "prodex_otlp_log_export_degraded reason=queue_full count={queue_full}"
                        );
                        while next_queue_full_report <= queue_full {
                            let next = next_queue_full_report.saturating_mul(2);
                            if next == next_queue_full_report {
                                break;
                            }
                            next_queue_full_report = next;
                        }
                    }
                    if failed && worker_shutdown.load(Ordering::Relaxed) {
                        let dropped = receiver.try_iter().count() as u64;
                        worker_stats
                            .shutdown_dropped
                            .fetch_add(dropped, Ordering::Relaxed);
                        break;
                    }
                }
            })
            .map_err(|err| format!("failed to start OTLP log exporter: {err}"))?;
        Ok(Self {
            sender: Some(sender),
            shutdown,
            stats,
            worker: Some(worker),
        })
    }

    pub(crate) fn try_export(&self, event_name: &'static str, attributes: Vec<OtlpLogAttribute>) {
        if let Some(sender) = self.sender.as_ref() {
            match sender.try_send(OtlpHttpLogEvent {
                event_name,
                attributes,
            }) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    self.stats.queue_full.fetch_add(1, Ordering::Relaxed);
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.stats.disconnected.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    fn stats(&self) -> OtlpHttpLogSinkStatsSnapshot {
        OtlpHttpLogSinkStatsSnapshot {
            queue_full: self.stats.queue_full.load(Ordering::Relaxed),
            disconnected: self.stats.disconnected.load(Ordering::Relaxed),
            export_failed: self.stats.export_failed.load(Ordering::Relaxed),
            shutdown_dropped: self.stats.shutdown_dropped.load(Ordering::Relaxed),
        }
    }
}

impl Drop for OtlpHttpLogSink {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let stats = self.stats();
        if stats != OtlpHttpLogSinkStatsSnapshot::default() {
            eprintln!(
                "prodex_otlp_log_export_summary queue_full={} disconnected={} export_failed={} shutdown_dropped={}",
                stats.queue_full, stats.disconnected, stats.export_failed, stats.shutdown_dropped
            );
        }
    }
}

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
    request = request.timeout(config.timeout.unwrap_or(DEFAULT_OTLP_HTTP_TIMEOUT));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn sink_counts_queue_pressure_and_failed_shutdown_drain() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            accepted_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            drop(stream);
        });
        let sink = OtlpHttpLogSink::start(
            OtlpHttpLogExportConfig {
                endpoint: format!("http://{addr}/v1/logs"),
                headers: Vec::new(),
                timeout: Some(Duration::from_secs(1)),
            },
            "prodex-gateway",
            "prodex.gateway",
        )
        .unwrap();

        sink.try_export("gateway.first", Vec::new());
        accepted_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        for _ in 0..OTLP_HTTP_LOG_QUEUE_CAPACITY + 32 {
            sink.try_export("gateway.queued", Vec::new());
        }
        let stats = Arc::clone(&sink.stats);
        assert!(stats.queue_full.load(Ordering::Relaxed) > 0);

        release_tx.send(()).unwrap();
        drop(sink);
        server.join().unwrap();
        assert!(stats.export_failed.load(Ordering::Relaxed) > 0);
        assert!(stats.shutdown_dropped.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn live_sink_exports_off_thread_and_drains_on_shutdown() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (request_tx, request_rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut header_bytes = Vec::new();
                let mut byte = [0_u8; 1];
                while !header_bytes.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    header_bytes.push(byte[0]);
                }
                let headers = String::from_utf8(header_bytes).unwrap();
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                let mut body = vec![0_u8; content_length];
                stream.read_exact(&mut body).unwrap();
                request_tx.send(body).unwrap();
                std::thread::sleep(Duration::from_millis(100));
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 0\r\n\r\n")
                    .unwrap();
            }
        });
        let sink = OtlpHttpLogSink::start(
            OtlpHttpLogExportConfig {
                endpoint: format!("http://{addr}/v1/logs"),
                headers: Vec::new(),
                timeout: Some(Duration::from_secs(1)),
            },
            "prodex-gateway",
            "prodex.gateway",
        )
        .unwrap();

        let started_at = std::time::Instant::now();
        sink.try_export(
            "gateway.request",
            vec![OtlpLogAttribute::string("gateway.route", "responses")],
        );
        assert!(started_at.elapsed() < Duration::from_millis(250));
        let body = request_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["body"]["stringValue"],
            "gateway.request"
        );

        sink.try_export(
            "gateway.shutdown",
            vec![OtlpLogAttribute::string("gateway.route", "shutdown")],
        );
        drop(sink);
        let body = request_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["body"]["stringValue"],
            "gateway.shutdown"
        );
        server.join().unwrap();
    }
}
