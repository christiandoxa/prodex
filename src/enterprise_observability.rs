mod http;

pub(crate) use http::OtlpHttpLogSink;
#[cfg(test)]
use http::*;
pub use http::{OtlpLogAttribute, export_otlp_http_log_if_configured, otlp_http_log_export_status};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    fn config_from(
        values: &[(&str, Option<&str>)],
    ) -> Result<Option<OtlpHttpLogExportConfig>, String> {
        otlp_http_log_export_config_from(|key| {
            values
                .iter()
                .find(|(candidate, _)| *candidate == key)
                .and_then(|(_, value)| value.map(str::to_string))
        })
    }

    #[test]
    fn otlp_config_debug_redacts_and_zeroize_clears_header_values() {
        let mut config = OtlpHttpLogExportConfig {
            endpoint: "https://otel.example.test/v1/logs?api_key=otlp-endpoint-secret".to_string(),
            headers: vec![(
                "authorization".to_string(),
                "Bearer otlp-header-secret".to_string(),
            )],
            timeout: Some(Duration::from_secs(1)),
        };

        let rendered = format!("{config:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("otlp-endpoint-secret"));
        assert!(!rendered.contains("otlp-header-secret"));

        zeroize::Zeroize::zeroize(&mut config);
        assert!(config.headers.is_empty());
    }

    #[test]
    fn otlp_endpoint_validation_rejects_credentials_query_fragment_and_whitespace_without_echo() {
        for endpoint in [
            "https://user:otlp-url-secret@otel.example.test/v1/logs",
            "https://otel.example.test/v1/logs?api_key=otlp-url-secret",
            "https://otel.example.test/v1/logs#otlp-url-secret",
            " https://otel.example.test/v1/logs",
        ] {
            let error = config_from(&[
                ("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", Some(endpoint)),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
            ])
            .expect_err("credential-bearing OTLP endpoint should fail");
            assert_eq!(error, INVALID_OTLP_HTTP_ENDPOINT);
            assert!(!error.contains("otlp-url-secret"));
        }
    }

    #[test]
    fn otlp_export_revalidates_endpoint_before_network_use() {
        let error = export_otlp_http_log(
            &OtlpHttpLogExportConfig {
                endpoint: "https://otel.example.test/v1/logs?token=otlp-url-secret".to_string(),
                headers: Vec::new(),
                timeout: None,
            },
            "prodex-control-plane",
            "prodex.control-plane",
            "config_publication.deliver",
            Vec::new(),
        )
        .expect_err("credential-bearing OTLP endpoint should fail before export");

        assert_eq!(error, INVALID_OTLP_HTTP_ENDPOINT);
        assert!(!error.contains("otlp-url-secret"));
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
        assert_eq!(
            config_from(&[
                ("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
                (
                    "OTEL_EXPORTER_OTLP_ENDPOINT",
                    Some("https://otel.example.test/root/"),
                ),
            ]),
            Ok(Some(OtlpHttpLogExportConfig {
                endpoint: "https://otel.example.test/root/v1/logs".to_string(),
                headers: Vec::new(),
                timeout: None,
            }))
        );
    }

    #[test]
    fn otlp_http_log_export_config_ignores_empty_endpoint() {
        assert_eq!(
            config_from(&[
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
                ("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", Some("   ")),
            ]),
            Ok(None)
        );
    }

    #[test]
    fn otlp_http_log_export_config_falls_back_when_logs_endpoint_is_empty() {
        assert_eq!(
            config_from(&[
                ("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", Some("   ")),
                (
                    "OTEL_EXPORTER_OTLP_ENDPOINT",
                    Some("https://otel.example.test/root/"),
                ),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
            ]),
            Ok(Some(OtlpHttpLogExportConfig {
                endpoint: "https://otel.example.test/root/v1/logs".to_string(),
                headers: Vec::new(),
                timeout: None,
            }))
        );
    }

    #[test]
    fn otlp_http_log_export_config_prefers_logs_endpoint_and_parses_headers() {
        assert_eq!(
            config_from(&[
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
            ]),
            Ok(Some(OtlpHttpLogExportConfig {
                endpoint: "https://logs.example.test/custom".to_string(),
                headers: vec![
                    ("authorization".to_string(), "Bearer test-token".to_string()),
                    ("x-tenant".to_string(), "prodex".to_string()),
                ],
                timeout: None,
            }))
        );
    }

    #[test]
    fn otlp_http_log_export_config_rejects_malformed_header_entry() {
        let err = config_from(&[
            (
                "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                Some("https://logs.example.test/custom"),
            ),
            ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
            ("OTEL_EXPORTER_OTLP_HEADERS", Some("authorization")),
        ])
        .expect_err("malformed header entry should fail");
        assert!(err.contains("invalid OTLP header entry"));
    }

    #[test]
    fn otlp_http_log_export_config_uses_logs_timeout_before_generic_timeout() {
        assert_eq!(
            config_from(&[
                (
                    "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                    Some("https://logs.example.test/custom"),
                ),
                ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
                ("OTEL_EXPORTER_OTLP_HEADERS", None),
                ("OTEL_EXPORTER_OTLP_TIMEOUT", Some("2500")),
                ("OTEL_EXPORTER_OTLP_LOGS_TIMEOUT", Some("15")),
            ]),
            Ok(Some(OtlpHttpLogExportConfig {
                endpoint: "https://logs.example.test/custom".to_string(),
                headers: Vec::new(),
                timeout: Some(Duration::from_millis(15)),
            }))
        );
    }

    #[test]
    fn otlp_http_log_export_config_rejects_invalid_timeout() {
        let err = config_from(&[
            (
                "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                Some("https://logs.example.test/custom"),
            ),
            ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
            ("OTEL_EXPORTER_OTLP_HEADERS", None),
            ("OTEL_EXPORTER_OTLP_LOGS_TIMEOUT", Some("slow")),
        ])
        .expect_err("invalid timeout should fail");
        assert!(err.contains("invalid OTLP timeout"));
    }

    #[test]
    fn export_otlp_http_log_if_configured_returns_false_when_unconfigured() {
        assert_eq!(
            export_otlp_http_log_with_config(
                None,
                "prodex-control-plane",
                "prodex.control-plane",
                "config_publication.plan",
                vec![OtlpLogAttribute::bool("authorized", true)],
            ),
            Ok(false)
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

        let config = OtlpHttpLogExportConfig {
            endpoint: format!("http://{addr}/v1/logs"),
            headers: Vec::new(),
            timeout: None,
        };
        assert_eq!(
            export_otlp_http_log_with_config(
                Some(&config),
                "prodex-control-plane",
                "prodex.control-plane",
                "config_publication.plan",
                vec![OtlpLogAttribute::bool("authorized", true)],
            ),
            Ok(true)
        );

        server.join().expect("OTLP server should join");
    }

    #[test]
    fn otlp_http_log_export_status_returns_disabled_when_unconfigured() {
        assert_eq!(otlp_http_log_export_status_for(Ok(false)), "disabled");
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

        let config = OtlpHttpLogExportConfig {
            endpoint: format!("http://{addr}/v1/logs"),
            headers: Vec::new(),
            timeout: None,
        };
        let result = export_otlp_http_log_with_config(
            Some(&config),
            "prodex-control-plane",
            "prodex.control-plane",
            "config_publication.plan",
            vec![OtlpLogAttribute::bool("authorized", true)],
        );
        assert_eq!(otlp_http_log_export_status_for(result), "exported");

        server.join().expect("OTLP server should join");
    }

    #[test]
    fn otlp_http_log_export_status_returns_failed_when_export_fails() {
        assert_eq!(
            otlp_http_log_export_status_for(Err("failed".to_string())),
            "failed"
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
    fn configured_export_rejects_invalid_header_value() {
        let config = config_from(&[
            (
                "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                Some("http://127.0.0.1:9/v1/logs"),
            ),
            ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
            (
                "OTEL_EXPORTER_OTLP_HEADERS",
                Some("authorization=bad\nvalue"),
            ),
        ])
        .expect("config should parse");
        let err = export_otlp_http_log_with_config(
            config.as_ref(),
            "prodex-control-plane",
            "prodex.control-plane",
            "config_publication.plan",
            vec![OtlpLogAttribute::bool("authorized", true)],
        )
        .expect_err("invalid header value should fail");

        assert!(err.contains("invalid OTLP header value"));
    }

    #[test]
    fn configured_export_rejects_invalid_header_name() {
        let config = config_from(&[
            (
                "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                Some("http://127.0.0.1:9/v1/logs"),
            ),
            ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
            ("OTEL_EXPORTER_OTLP_HEADERS", Some("bad header=value")),
        ])
        .expect("config should parse");
        let err = export_otlp_http_log_with_config(
            config.as_ref(),
            "prodex-control-plane",
            "prodex.control-plane",
            "config_publication.plan",
            vec![OtlpLogAttribute::bool("authorized", true)],
        )
        .expect_err("invalid header name should fail");

        assert!(err.contains("invalid OTLP header name"));
    }

    #[test]
    fn configured_export_reports_transport_failure() {
        let config = config_from(&[
            (
                "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                Some("http://127.0.0.1:9/v1/logs"),
            ),
            ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
            ("OTEL_EXPORTER_OTLP_HEADERS", None),
        ])
        .expect("config should parse");
        let err = export_otlp_http_log_with_config(
            config.as_ref(),
            "prodex-control-plane",
            "prodex.control-plane",
            "config_publication.plan",
            vec![OtlpLogAttribute::bool("authorized", true)],
        )
        .expect_err("unreachable configured endpoint should fail");

        assert!(err.contains("OTLP log export failed"));
    }

    #[test]
    fn configured_export_rejects_malformed_header_entry() {
        let err = config_from(&[
            (
                "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
                Some("http://127.0.0.1:9/v1/logs"),
            ),
            ("OTEL_EXPORTER_OTLP_ENDPOINT", None),
            ("OTEL_EXPORTER_OTLP_HEADERS", Some("authorization")),
        ])
        .expect_err("malformed header entry should fail");

        assert!(err.contains("invalid OTLP header entry"));
    }

    #[test]
    fn live_otlp_sink_exports_off_the_request_thread() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP test listener");
        let addr = listener.local_addr().expect("resolve listener addr");
        let (request_tx, request_rx) = std::sync::mpsc::channel();
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
            let mut body = vec![0_u8; content_length];
            stream.read_exact(&mut body).expect("read OTLP body");
            request_tx.send(body).expect("record OTLP body");
            thread::sleep(Duration::from_millis(500));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .expect("write OTLP response");
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
        let body = request_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("OTLP body should arrive");
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["body"]["stringValue"],
            "gateway.request"
        );

        server.join().expect("OTLP server should join");
        drop(sink);
    }
}
