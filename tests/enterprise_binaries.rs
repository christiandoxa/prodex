use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;

fn bin(name: &str) -> String {
    std::env::var(format!("CARGO_BIN_EXE_{name}")).expect("cargo should expose binary path")
}

fn start_otlp_capture_server() -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP test listener");
    let addr = listener.local_addr().expect("resolve OTLP listener addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept OTLP export");
        let mut header_bytes = Vec::new();
        let mut byte = [0_u8; 1];
        while !header_bytes.ends_with(b"\r\n\r\n") {
            stream
                .read_exact(&mut byte)
                .expect("read OTLP request header byte");
            header_bytes.push(byte[0]);
        }
        let headers = String::from_utf8(header_bytes).expect("OTLP headers should be utf8");
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length").then(|| {
                    value
                        .trim()
                        .parse::<usize>()
                        .expect("OTLP content-length should parse")
                })
            })
            .expect("OTLP content-length header");
        let mut body = vec![0_u8; content_length];
        stream
            .read_exact(&mut body)
            .expect("read OTLP request body");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .expect("write OTLP response");
        format!(
            "{}{}",
            headers,
            String::from_utf8(body).expect("OTLP body should be utf8")
        )
    });
    (format!("http://{addr}/v1/logs"), handle)
}

fn closed_otlp_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP closed-port listener");
    let addr = listener
        .local_addr()
        .expect("resolve OTLP closed-port listener addr");
    drop(listener);
    format!("http://{addr}/v1/logs")
}

fn hanging_otlp_endpoint(delay_ms: u64) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP hanging listener");
    let addr = listener
        .local_addr()
        .expect("resolve OTLP hanging listener addr");
    let handle = thread::spawn(move || {
        let (_stream, _) = listener.accept().expect("accept OTLP hanging export");
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    });
    (format!("http://{addr}/v1/logs"), handle)
}

#[test]
fn gateway_binary_exposes_dedicated_help_and_version() {
    let help = Command::new(bin("prodex-gateway"))
        .arg("--help")
        .output()
        .expect("run prodex-gateway --help");
    assert!(help.status.success());
    let stdout = String::from_utf8(help.stdout).unwrap();
    assert!(stdout.contains("Data-plane gateway entrypoint"));
    assert!(stdout.contains("composition root"));

    let version = Command::new(bin("prodex-gateway"))
        .arg("--version")
        .output()
        .expect("run prodex-gateway --version");
    assert!(version.status.success());
    assert!(
        String::from_utf8(version.stdout)
            .unwrap()
            .starts_with("prodex-gateway ")
    );
}

#[test]
fn control_plane_binary_exposes_dedicated_help_and_version() {
    let help = Command::new(bin("prodex-control-plane"))
        .arg("--help")
        .output()
        .expect("run prodex-control-plane --help");
    assert!(help.status.success());
    let stdout = String::from_utf8(help.stdout).unwrap();
    assert!(stdout.contains("Control-plane entrypoint"));
    assert!(stdout.contains("composition"));

    let version = Command::new(bin("prodex-control-plane"))
        .arg("--version")
        .output()
        .expect("run prodex-control-plane --version");
    assert!(version.status.success());
    assert!(
        String::from_utf8(version.stdout)
            .unwrap()
            .starts_with("prodex-control-plane ")
    );
}

#[test]
fn enterprise_serve_commands_are_explicitly_gated_until_adapters_are_ready() {
    for (name, expected_stderr) in [
        (
            "prodex-gateway",
            "prodex-gateway serve is not wired yet; use the legacy `prodex gateway` path until the async adapter migration is complete\n",
        ),
        (
            "prodex-control-plane",
            "prodex-control-plane serve is not wired yet; use the legacy `prodex gateway` admin path until control-plane adapter migration is complete\n",
        ),
    ] {
        let output = Command::new(bin(name))
            .arg("serve")
            .output()
            .expect("run gated serve command");
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8(output.stderr).unwrap() == expected_stderr);
    }
}

#[test]
fn gateway_binary_reports_migration_summary_json() {
    let path = std::env::temp_dir().join(format!(
        "prodex-gateway-migrate-binary-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));

    let output = Command::new(bin("prodex-gateway"))
        .args([
            "migrate",
            "--backend",
            "sqlite",
            "--path",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway migrate");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["migrated"], true);
    assert_eq!(stdout["backend"], "sqlite");
    assert_eq!(stdout["otlp_log_export"], "disabled");
    assert!(
        stdout["message"]
            .as_str()
            .expect("message string")
            .contains("ensured gateway compatibility schema")
    );

    let _ = fs::remove_file(path);
}

#[test]
fn gateway_binary_migration_exports_otlp_http_log_when_configured() {
    let path = std::env::temp_dir().join(format!(
        "prodex-gateway-migrate-otlp-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-gateway"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .env(
            "OTEL_EXPORTER_OTLP_HEADERS",
            "authorization=Bearer test-token",
        )
        .args([
            "migrate",
            "--backend",
            "sqlite",
            "--path",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway migrate with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["migrated"], true);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-token"));
    assert!(request.contains("\"stringValue\":\"prodex-gateway\""));
    assert!(request.contains("\"stringValue\":\"gateway.migrate\""));

    let _ = fs::remove_file(path);
}

#[test]
fn gateway_binary_migration_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset() {
    let path = std::env::temp_dir().join(format!(
        "prodex-gateway-migrate-otlp-endpoint-fallback-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-gateway"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "migrate",
            "--backend",
            "sqlite",
            "--path",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway migrate with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["migrated"], true);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-gateway\""));
    assert!(request.contains("\"stringValue\":\"gateway.migrate\""));

    let _ = fs::remove_file(path);
}

#[test]
fn gateway_binary_migration_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank() {
    let path = std::env::temp_dir().join(format!(
        "prodex-gateway-migrate-otlp-endpoint-blank-fallback-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-gateway"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "migrate",
            "--backend",
            "sqlite",
            "--path",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway migrate with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["migrated"], true);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-gateway\""));
    assert!(request.contains("\"stringValue\":\"gateway.migrate\""));

    let _ = fs::remove_file(path);
}

#[test]
fn gateway_binary_migration_reports_failed_otlp_http_log_when_export_fails() {
    let path = std::env::temp_dir().join(format!(
        "prodex-gateway-migrate-otlp-failed-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));

    let output = Command::new(bin("prodex-gateway"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "migrate",
            "--backend",
            "sqlite",
            "--path",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway migrate with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["migrated"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_file(path);
}

#[test]
fn gateway_binary_migration_reports_failed_otlp_http_log_when_headers_are_malformed() {
    let path = std::env::temp_dir().join(format!(
        "prodex-gateway-migrate-otlp-malformed-headers-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));

    let output = Command::new(bin("prodex-gateway"))
        .env(
            "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
            "http://127.0.0.1:9/v1/logs",
        )
        .env("OTEL_EXPORTER_OTLP_HEADERS", "authorization")
        .args([
            "migrate",
            "--backend",
            "sqlite",
            "--path",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway migrate with malformed OTLP headers");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["migrated"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_file(path);
}

#[test]
fn gateway_binary_migration_reports_failed_otlp_http_log_when_export_times_out() {
    let path = std::env::temp_dir().join(format!(
        "prodex-gateway-migrate-otlp-timeout-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let (endpoint, server) = hanging_otlp_endpoint(100);

    let output = Command::new(bin("prodex-gateway"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .env("OTEL_EXPORTER_OTLP_LOGS_TIMEOUT", "10")
        .args([
            "migrate",
            "--backend",
            "sqlite",
            "--path",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway migrate with timed out OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["migrated"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    server.join().expect("OTLP hanging server should join");
    let _ = fs::remove_file(path);
}

#[test]
fn gateway_binary_migration_uses_generic_otlp_timeout_with_endpoint_fallback() {
    let path = std::env::temp_dir().join(format!(
        "prodex-gateway-migrate-otlp-generic-timeout-fallback-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let (logs_endpoint, server) = hanging_otlp_endpoint(100);
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-gateway"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .env("OTEL_EXPORTER_OTLP_TIMEOUT", "10")
        .args([
            "migrate",
            "--backend",
            "sqlite",
            "--path",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway migrate with generic OTLP timeout fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["migrated"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    server.join().expect("OTLP hanging server should join");
    let _ = fs::remove_file(path);
}

#[test]
fn gateway_binary_migration_reports_failed_otlp_http_log_when_timeout_env_is_invalid() {
    let path = std::env::temp_dir().join(format!(
        "prodex-gateway-migrate-otlp-invalid-timeout-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));

    let output = Command::new(bin("prodex-gateway"))
        .env(
            "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
            "http://127.0.0.1:9/v1/logs",
        )
        .env("OTEL_EXPORTER_OTLP_LOGS_TIMEOUT", "slow")
        .args([
            "migrate",
            "--backend",
            "sqlite",
            "--path",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway migrate with invalid OTLP timeout");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["migrated"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_file(path);
}

#[test]
fn gateway_binary_migration_reports_failed_otlp_http_log_when_generic_timeout_env_is_invalid() {
    let path = std::env::temp_dir().join(format!(
        "prodex-gateway-migrate-otlp-invalid-generic-timeout-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP generic-timeout listener");
    let addr = listener
        .local_addr()
        .expect("resolve OTLP generic-timeout listener addr");
    drop(listener);

    let output = Command::new(bin("prodex-gateway"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", format!("http://{addr}"))
        .env("OTEL_EXPORTER_OTLP_TIMEOUT", "slow")
        .args([
            "migrate",
            "--backend",
            "sqlite",
            "--path",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway migrate with invalid generic OTLP timeout");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["migrated"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_file(path);
}

#[test]
fn control_plane_binary_delivers_config_publication_event() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-delivery-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane root");
    fs::write(
        root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 3
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "activated_revision_id": "00000000-0000-7000-8000-000000000002",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000003",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "deliver-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run config publication delivery");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["gateway_cache_refreshed"], true);
    assert_eq!(stdout["runtime_policy_version"], 1);
    assert_eq!(stdout["otlp_log_export"], "disabled");
    assert_eq!(
        stdout["delivery_metrics"][0]["target"],
        "gateway_cache_refresh"
    );
    assert_eq!(
        stdout["delivery_metrics"][1]["target"],
        "runtime_policy_reload"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_delivery_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-delivery-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane OTLP root");
    fs::write(
        root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 3
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "activated_revision_id": "00000000-0000-7000-8000-000000000002",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000003",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .env(
            "OTEL_EXPORTER_OTLP_HEADERS",
            "authorization=Bearer test-token",
        )
        .args([
            "deliver-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run control-plane delivery with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["gateway_cache_refreshed"], true);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-token"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"config_publication.deliver\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_delivery_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-delivery-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane OTLP fallback root");
    fs::write(
        root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 3
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000021",
            "activated_revision_id": "00000000-0000-7000-8000-000000000022",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000023",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "deliver-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run control-plane delivery with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["gateway_cache_refreshed"], true);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"config_publication.deliver\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_delivery_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-delivery-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane OTLP fallback root");
    fs::write(
        root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 3
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000021",
            "activated_revision_id": "00000000-0000-7000-8000-000000000022",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000023",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "deliver-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run control-plane delivery with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["gateway_cache_refreshed"], true);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"config_publication.deliver\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_delivery_reports_failed_otlp_http_log_when_export_fails() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-delivery-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane failed-OTLP root");
    fs::write(
        root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 3
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000011",
            "activated_revision_id": "00000000-0000-7000-8000-000000000012",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000013",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "deliver-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run control-plane delivery with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["gateway_cache_refreshed"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_delivery_reports_failed_otlp_http_log_when_headers_are_malformed() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-delivery-otlp-malformed-headers-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane malformed-OTLP root");
    fs::write(
        root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 3
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000011",
            "activated_revision_id": "00000000-0000-7000-8000-000000000012",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000013",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");

    let output = Command::new(bin("prodex-control-plane"))
        .env(
            "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
            "http://127.0.0.1:9/v1/logs",
        )
        .env("OTEL_EXPORTER_OTLP_HEADERS", "authorization")
        .args([
            "deliver-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run control-plane delivery with malformed OTLP headers");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["gateway_cache_refreshed"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_delivery_reports_failed_otlp_http_log_when_export_times_out() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-delivery-otlp-timeout-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane timeout-OTLP root");
    fs::write(
        root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 3
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000011",
            "activated_revision_id": "00000000-0000-7000-8000-000000000012",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000013",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let (endpoint, server) = hanging_otlp_endpoint(100);

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .env("OTEL_EXPORTER_OTLP_LOGS_TIMEOUT", "10")
        .args([
            "deliver-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run control-plane delivery with timed out OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["gateway_cache_refreshed"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    server.join().expect("OTLP hanging server should join");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_delivery_uses_generic_otlp_timeout_with_endpoint_fallback() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-delivery-otlp-generic-timeout-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane generic-timeout-OTLP root");
    fs::write(
        root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 3
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000011",
            "activated_revision_id": "00000000-0000-7000-8000-000000000012",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000013",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let (logs_endpoint, server) = hanging_otlp_endpoint(100);
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .env("OTEL_EXPORTER_OTLP_TIMEOUT", "10")
        .args([
            "deliver-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run control-plane delivery with generic OTLP timeout fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["gateway_cache_refreshed"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    server.join().expect("OTLP hanging server should join");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_delivery_reports_failed_otlp_http_log_when_timeout_env_is_invalid() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-delivery-otlp-invalid-timeout-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane invalid-timeout-OTLP root");
    fs::write(
        root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 3
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000011",
            "activated_revision_id": "00000000-0000-7000-8000-000000000012",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000013",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");

    let output = Command::new(bin("prodex-control-plane"))
        .env(
            "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
            "http://127.0.0.1:9/v1/logs",
        )
        .env("OTEL_EXPORTER_OTLP_LOGS_TIMEOUT", "slow")
        .args([
            "deliver-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run control-plane delivery with invalid OTLP timeout");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["gateway_cache_refreshed"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_delivery_reports_failed_otlp_http_log_when_generic_timeout_env_is_invalid()
{
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-delivery-otlp-invalid-generic-timeout-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane invalid-generic-timeout-OTLP root");
    fs::write(
        root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 3
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000011",
            "activated_revision_id": "00000000-0000-7000-8000-000000000012",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000013",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP generic-timeout listener");
    let addr = listener
        .local_addr()
        .expect("resolve OTLP generic-timeout listener addr");
    drop(listener);

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", format!("http://{addr}"))
        .env("OTEL_EXPORTER_OTLP_TIMEOUT", "slow")
        .args([
            "deliver-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run control-plane delivery with invalid generic OTLP timeout");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["gateway_cache_refreshed"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_and_gateway_binaries_use_replicated_config_publication_transport() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-transport-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    let gateway_root = root.join("gateway");
    fs::create_dir_all(&transport).expect("create transport root");
    fs::create_dir_all(&gateway_root).expect("create gateway root");
    fs::write(
        gateway_root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 4
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000101",
            "activated_revision_id": "00000000-0000-7000-8000-000000000102",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000103",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");

    let publish = Command::new(bin("prodex-control-plane"))
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event");
    assert!(
        publish.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&publish.stderr)
    );
    let publish_stdout: serde_json::Value =
        serde_json::from_slice(&publish.stdout).expect("json output");
    assert_eq!(
        publish_stdout["transport_root"],
        serde_json::Value::String(transport.display().to_string())
    );
    assert_eq!(publish_stdout["otlp_log_export"], "disabled");

    let first_consume = Command::new(bin("prodex-gateway"))
        .args([
            "consume-config-publication",
            "--transport",
            transport.to_str().unwrap(),
            "--replica",
            "gateway-a",
            "--root",
            gateway_root.to_str().unwrap(),
        ])
        .output()
        .expect("consume config publication event");
    assert!(
        first_consume.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_consume.stderr)
    );
    let first_stdout: serde_json::Value =
        serde_json::from_slice(&first_consume.stdout).expect("json output");
    assert_eq!(first_stdout["delivered_event_count"], 1);
    assert_eq!(first_stdout["runtime_policy_version"], 1);
    assert_eq!(first_stdout["otlp_log_export"], "disabled");
    assert_eq!(
        first_stdout["delivery_metrics"][0]["target"],
        "gateway_cache_refresh"
    );
    assert_eq!(first_stdout["delivery_metrics"][0]["increment"], 1);

    let repeat_consume = Command::new(bin("prodex-gateway"))
        .args([
            "consume-config-publication",
            "--transport",
            transport.to_str().unwrap(),
            "--replica",
            "gateway-a",
            "--root",
            gateway_root.to_str().unwrap(),
        ])
        .output()
        .expect("repeat config publication consume");
    assert!(
        repeat_consume.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&repeat_consume.stderr)
    );
    let repeat_stdout: serde_json::Value =
        serde_json::from_slice(&repeat_consume.stdout).expect("json output");
    assert_eq!(repeat_stdout["delivered_event_count"], 0);
    assert_eq!(repeat_stdout["delivery_metrics"][0]["increment"], 0);

    let second_replica_consume = Command::new(bin("prodex-gateway"))
        .args([
            "consume-config-publication",
            "--transport",
            transport.to_str().unwrap(),
            "--replica",
            "gateway-b",
            "--root",
            gateway_root.to_str().unwrap(),
        ])
        .output()
        .expect("second replica config publication consume");
    assert!(
        second_replica_consume.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_replica_consume.stderr)
    );
    let second_replica_stdout: serde_json::Value =
        serde_json::from_slice(&second_replica_consume.stdout).expect("json output");
    assert_eq!(second_replica_stdout["delivered_event_count"], 1);

    let compact = Command::new(bin("prodex-control-plane"))
        .args([
            "compact-config-publication",
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("compact config publication transport");
    assert!(
        compact.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&compact.stderr)
    );
    let compact_stdout: serde_json::Value =
        serde_json::from_slice(&compact.stdout).expect("json output");
    assert_eq!(compact_stdout["replica_count"], 2);
    assert_eq!(compact_stdout["eligible_event_count"], 1);
    assert_eq!(compact_stdout["removed_event_count"], 1);
    assert_eq!(compact_stdout["retained_event_count"], 0);
    assert_eq!(compact_stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_publish_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-publish-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    fs::create_dir_all(&transport).expect("create transport root");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000111",
            "activated_revision_id": "00000000-0000-7000-8000-000000000112",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000113",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(
        request.contains("\"stringValue\":\"control_plane.configuration_publication.publish\"")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_publish_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-publish-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    fs::create_dir_all(&transport).expect("create transport root");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000117",
            "activated_revision_id": "00000000-0000-7000-8000-000000000118",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000119",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(
        request.contains("\"stringValue\":\"control_plane.configuration_publication.publish\"")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_publish_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-publish-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    fs::create_dir_all(&transport).expect("create transport root");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000117",
            "activated_revision_id": "00000000-0000-7000-8000-000000000118",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000119",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(
        request.contains("\"stringValue\":\"control_plane.configuration_publication.publish\"")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_publish_reports_failed_otlp_http_log_when_export_fails() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-publish-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    fs::create_dir_all(&transport).expect("create transport root");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000114",
            "activated_revision_id": "00000000-0000-7000-8000-000000000115",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000116",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn gateway_binary_consume_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-consume-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    let gateway_root = root.join("gateway");
    fs::create_dir_all(&transport).expect("create transport root");
    fs::create_dir_all(&gateway_root).expect("create gateway root");
    fs::write(
        gateway_root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 4
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000121",
            "activated_revision_id": "00000000-0000-7000-8000-000000000122",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000123",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let publish = Command::new(bin("prodex-control-plane"))
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event");
    assert!(
        publish.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&publish.stderr)
    );
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-gateway"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .args([
            "consume-config-publication",
            "--transport",
            transport.to_str().unwrap(),
            "--replica",
            "gateway-a",
            "--root",
            gateway_root.to_str().unwrap(),
        ])
        .output()
        .expect("consume config publication event with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["delivered_event_count"], 1);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-gateway\""));
    assert!(request.contains("\"stringValue\":\"config_publication.consume\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn gateway_binary_consume_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-consume-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    let gateway_root = root.join("gateway");
    fs::create_dir_all(&transport).expect("create transport root");
    fs::create_dir_all(&gateway_root).expect("create gateway root");
    fs::write(
        gateway_root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 4
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000124",
            "activated_revision_id": "00000000-0000-7000-8000-000000000125",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000126",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let publish = Command::new(bin("prodex-control-plane"))
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event");
    assert!(
        publish.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&publish.stderr)
    );
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-gateway"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "consume-config-publication",
            "--transport",
            transport.to_str().unwrap(),
            "--replica",
            "gateway-a",
            "--root",
            gateway_root.to_str().unwrap(),
        ])
        .output()
        .expect("consume config publication event with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["delivered_event_count"], 1);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-gateway\""));
    assert!(request.contains("\"stringValue\":\"config_publication.consume\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn gateway_binary_consume_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-consume-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    let gateway_root = root.join("gateway");
    fs::create_dir_all(&transport).expect("create transport root");
    fs::create_dir_all(&gateway_root).expect("create gateway root");
    fs::write(
        gateway_root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 4
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000124",
            "activated_revision_id": "00000000-0000-7000-8000-000000000125",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000126",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let publish = Command::new(bin("prodex-control-plane"))
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event");
    assert!(
        publish.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&publish.stderr)
    );
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-gateway"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "consume-config-publication",
            "--transport",
            transport.to_str().unwrap(),
            "--replica",
            "gateway-a",
            "--root",
            gateway_root.to_str().unwrap(),
        ])
        .output()
        .expect("consume config publication event with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["delivered_event_count"], 1);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-gateway\""));
    assert!(request.contains("\"stringValue\":\"config_publication.consume\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn gateway_binary_consume_reports_failed_otlp_http_log_when_export_fails() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-consume-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    let gateway_root = root.join("gateway");
    fs::create_dir_all(&transport).expect("create transport root");
    fs::create_dir_all(&gateway_root).expect("create gateway root");
    fs::write(
        gateway_root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 4
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000124",
            "activated_revision_id": "00000000-0000-7000-8000-000000000125",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000126",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let publish = Command::new(bin("prodex-control-plane"))
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event");
    assert!(
        publish.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&publish.stderr)
    );

    let output = Command::new(bin("prodex-gateway"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "consume-config-publication",
            "--transport",
            transport.to_str().unwrap(),
            "--replica",
            "gateway-a",
            "--root",
            gateway_root.to_str().unwrap(),
        ])
        .output()
        .expect("consume config publication event with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["delivered_event_count"], 1);
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_compact_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-compact-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    let gateway_root = root.join("gateway");
    fs::create_dir_all(&transport).expect("create transport root");
    fs::create_dir_all(&gateway_root).expect("create gateway root");
    fs::write(
        gateway_root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 4
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000131",
            "activated_revision_id": "00000000-0000-7000-8000-000000000132",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000133",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let publish = Command::new(bin("prodex-control-plane"))
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event");
    assert!(
        publish.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&publish.stderr)
    );
    for replica in ["gateway-a", "gateway-b"] {
        let consume = Command::new(bin("prodex-gateway"))
            .args([
                "consume-config-publication",
                "--transport",
                transport.to_str().unwrap(),
                "--replica",
                replica,
                "--root",
                gateway_root.to_str().unwrap(),
            ])
            .output()
            .expect("consume config publication event");
        assert!(
            consume.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&consume.stderr)
        );
    }
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .args([
            "compact-config-publication",
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("compact config publication transport with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["replica_count"], 2);
    assert_eq!(stdout["eligible_event_count"], 1);
    assert_eq!(stdout["removed_event_count"], 1);
    assert_eq!(stdout["retained_event_count"], 0);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(
        request.contains("\"stringValue\":\"control_plane.configuration_publication.compact\"")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_compact_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-compact-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    let gateway_root = root.join("gateway");
    fs::create_dir_all(&transport).expect("create transport root");
    fs::create_dir_all(&gateway_root).expect("create gateway root");
    fs::write(
        gateway_root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 4
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000134",
            "activated_revision_id": "00000000-0000-7000-8000-000000000135",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000136",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let publish = Command::new(bin("prodex-control-plane"))
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event");
    assert!(
        publish.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&publish.stderr)
    );
    for replica in ["gateway-a", "gateway-b"] {
        let consume = Command::new(bin("prodex-gateway"))
            .args([
                "consume-config-publication",
                "--transport",
                transport.to_str().unwrap(),
                "--replica",
                replica,
                "--root",
                gateway_root.to_str().unwrap(),
            ])
            .output()
            .expect("consume config publication event");
        assert!(
            consume.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&consume.stderr)
        );
    }
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "compact-config-publication",
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("compact config publication transport with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["replica_count"], 2);
    assert_eq!(stdout["eligible_event_count"], 1);
    assert_eq!(stdout["removed_event_count"], 1);
    assert_eq!(stdout["retained_event_count"], 0);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(
        request.contains("\"stringValue\":\"control_plane.configuration_publication.compact\"")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_compact_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-compact-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    let gateway_root = root.join("gateway");
    fs::create_dir_all(&transport).expect("create transport root");
    fs::create_dir_all(&gateway_root).expect("create gateway root");
    fs::write(
        gateway_root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 4
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000134",
            "activated_revision_id": "00000000-0000-7000-8000-000000000135",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000136",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let publish = Command::new(bin("prodex-control-plane"))
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event");
    assert!(
        publish.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&publish.stderr)
    );
    for replica in ["gateway-a", "gateway-b"] {
        let consume = Command::new(bin("prodex-gateway"))
            .args([
                "consume-config-publication",
                "--transport",
                transport.to_str().unwrap(),
                "--replica",
                replica,
                "--root",
                gateway_root.to_str().unwrap(),
            ])
            .output()
            .expect("consume config publication event");
        assert!(
            consume.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&consume.stderr)
        );
    }
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "compact-config-publication",
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("compact config publication transport with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["replica_count"], 2);
    assert_eq!(stdout["eligible_event_count"], 1);
    assert_eq!(stdout["removed_event_count"], 1);
    assert_eq!(stdout["retained_event_count"], 0);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(
        request.contains("\"stringValue\":\"control_plane.configuration_publication.compact\"")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_compact_reports_failed_otlp_http_log_when_export_fails() {
    let root = std::env::temp_dir().join(format!(
        "prodex-config-publication-compact-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let transport = root.join("transport");
    let gateway_root = root.join("gateway");
    fs::create_dir_all(&transport).expect("create transport root");
    fs::create_dir_all(&gateway_root).expect("create gateway root");
    fs::write(
        gateway_root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 4
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000134",
            "activated_revision_id": "00000000-0000-7000-8000-000000000135",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000136",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");
    let publish = Command::new(bin("prodex-control-plane"))
        .args([
            "publish-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("publish config publication event");
    assert!(
        publish.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&publish.stderr)
    );
    for replica in ["gateway-a", "gateway-b"] {
        let consume = Command::new(bin("prodex-gateway"))
            .args([
                "consume-config-publication",
                "--transport",
                transport.to_str().unwrap(),
                "--replica",
                replica,
                "--root",
                gateway_root.to_str().unwrap(),
            ])
            .output()
            .expect("consume config publication event");
        assert!(
            consume.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&consume.stderr)
        );
    }

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "compact-config-publication",
            "--transport",
            transport.to_str().unwrap(),
        ])
        .output()
        .expect("compact config publication transport with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["replica_count"], 2);
    assert_eq!(stdout["eligible_event_count"], 1);
    assert_eq!(stdout["removed_event_count"], 1);
    assert_eq!(stdout["retained_event_count"], 0);
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_config_publication() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane plan root");
    let request_path = root.join("publication-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000010",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000011",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write publication request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run config publication planning");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], true);
    assert_eq!(stdout["required_role"], "admin");
    assert_eq!(stdout["resource_kind"], "configuration");
    assert_eq!(stdout["resource_action"], "publish_revision");
    assert_eq!(
        stdout["audit_action"],
        "control_plane.configuration.publish"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plan_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane plan root");
    let request_path = root.join("publication-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000010",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000011",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write publication request");
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run config publication planning with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], true);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.configuration_publication.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plan_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane plan root");
    let request_path = root.join("publication-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000010",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000011",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write publication request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run config publication planning with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], true);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.configuration_publication.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plan_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane plan root");
    let request_path = root.join("publication-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000010",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000011",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write publication request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run config publication planning with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], true);
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.configuration_publication.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_config_publication_denial() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-denied-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane denied root");
    let request_path = root.join("publication-request-denied.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000020",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "data_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000021",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write denied publication request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run denied config publication planning");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], false);
    assert_eq!(stdout["code"], "credential_scope_not_allowed");
    assert_eq!(stdout["audit_outcome"], "denied");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plan_reports_failed_otlp_http_log_when_export_fails() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane plan root");
    let request_path = root.join("publication-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000010",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000011",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write publication request");

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run config publication planning with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], true);
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_denied_plan_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-denied-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane denied root");
    let request_path = root.join("publication-request-denied.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000020",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "data_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000021",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write denied publication request");
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run denied config publication planning with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], false);
    assert_eq!(stdout["code"], "credential_scope_not_allowed");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.configuration_publication.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_denied_plan_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-denied-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane denied root");
    let request_path = root.join("publication-request-denied.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000020",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "data_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000021",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write denied publication request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run denied config publication planning with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], false);
    assert_eq!(stdout["code"], "credential_scope_not_allowed");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.configuration_publication.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_denied_plan_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-denied-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane denied root");
    let request_path = root.join("publication-request-denied.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000020",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "data_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000021",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write denied publication request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run denied config publication planning with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], false);
    assert_eq!(stdout["code"], "credential_scope_not_allowed");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.configuration_publication.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_denied_plan_reports_failed_otlp_http_log_when_export_fails() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-denied-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane denied root");
    let request_path = root.join("publication-request-denied.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000020",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "data_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000021",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write denied publication request");

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run denied config publication planning with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], false);
    assert_eq!(stdout["code"], "credential_scope_not_allowed");
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_configuration_publish_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-plan-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http root");
    let request_path = root.join("http-plan-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000030",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "request_id": "00000000-0000-7000-8000-000000000101",
            "call_id": "00000000-0000-7000-8000-000000000102",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/configuration/revisions",
            "body_len": 128,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "config-publish-1"}
            ],
            "body_digest": "sha256:config-publish-1"
        })
        .to_string(),
    )
    .expect("write http plan request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "configuration_publish");
    assert_eq!(stdout["resource_kind"], "configuration");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "postgres");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.configuration.publish"
    );
    assert_eq!(stdout["audit"]["audit_outcome"], "success");
    assert_eq!(
        stdout["audit_correlation"]["request_id"],
        "00000000-0000-7000-8000-000000000101"
    );
    assert_eq!(
        stdout["audit_correlation"]["call_id"],
        "00000000-0000-7000-8000-000000000102"
    );
    assert_eq!(
        stdout["audit_correlation"]["trace_id"],
        "4bf92f3577b34da6a3ce929d0e0e4736"
    );
    assert_eq!(
        stdout["audit_correlation"]["emission_span"]["name"],
        "prodex.control_plane.audit.emit"
    );
    assert_eq!(
        stdout["audit_correlation"]["persistence_span"]["name"],
        "prodex.control_plane.audit.persist"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "config-publish-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(
        stdout["precondition"]["entity_tag"],
        serde_json::Value::Null
    );
    assert_eq!(stdout["page_request"]["limit"], 50);
    assert_eq!(stdout["page_request"]["cursor"], serde_json::Value::Null);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-plan-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http root");
    let request_path = root.join("http-plan-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000030",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "request_id": "00000000-0000-7000-8000-000000000101",
            "call_id": "00000000-0000-7000-8000-000000000102",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/configuration/revisions",
            "body_len": 128,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "config-publish-1"}
            ],
            "body_digest": "sha256:config-publish-1"
        })
        .to_string(),
    )
    .expect("write http plan request");
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http plan with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "configuration_publish");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains(&format!(
        "\"key\":\"service.version\",\"value\":{{\"stringValue\":\"{}\"}}",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(request.contains(&format!(
        "\"scope\":{{\"name\":\"prodex.control-plane\",\"version\":\"{}\"}}",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));
    assert!(request.contains("\"key\":\"request_id\",\"value\":{\"stringValue\":\"00000000-0000-7000-8000-000000000101\"}"));
    assert!(request.contains(
        "\"key\":\"call_id\",\"value\":{\"stringValue\":\"00000000-0000-7000-8000-000000000102\"}"
    ));
    assert!(request.contains(
        "\"key\":\"trace_id\",\"value\":{\"stringValue\":\"4bf92f3577b34da6a3ce929d0e0e4736\"}"
    ));
    assert!(request.contains("\"traceId\":\"4bf92f3577b34da6a3ce929d0e0e4736\""));
    assert!(
        request.contains("\"key\":\"span_id\",\"value\":{\"stringValue\":\"00f067aa0ba902b7\"}")
    );
    assert!(request.contains("\"spanId\":\"00f067aa0ba902b7\""));
    assert!(request.contains("\"key\":\"trace_flags\",\"value\":{\"stringValue\":\"01\"}"));
    assert!(request.contains("\"flags\":1"));
    assert!(request.contains(
        "\"key\":\"event.name\",\"value\":{\"stringValue\":\"control_plane.http_route.plan\"}"
    ));
    assert!(request.contains(
        "\"key\":\"tenant_id\",\"value\":{\"stringValue\":\"00000000-0000-7000-8000-000000000001\"}"
    ));
    assert!(request.contains("\"key\":\"audit_event_id\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-plan-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http root");
    let request_path = root.join("http-plan-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000030",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "request_id": "00000000-0000-7000-8000-000000000101",
            "call_id": "00000000-0000-7000-8000-000000000102",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/configuration/revisions",
            "body_len": 128,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "config-publish-1"}
            ],
            "body_digest": "sha256:config-publish-1"
        })
        .to_string(),
    )
    .expect("write http plan request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http plan with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "configuration_publish");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-plan-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http root");
    let request_path = root.join("http-plan-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000030",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "request_id": "00000000-0000-7000-8000-000000000101",
            "call_id": "00000000-0000-7000-8000-000000000102",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/configuration/revisions",
            "body_len": 128,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "config-publish-1"}
            ],
            "body_digest": "sha256:config-publish-1"
        })
        .to_string(),
    )
    .expect("write http plan request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http plan with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "configuration_publish");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_budget_update_trace_context_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-correlation-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http audit correlation error root");
    let request_path = root.join("http-plan-request-audit-correlation-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000047",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "budget-default",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:budget-update-audit",
            "method": "PATCH",
            "path": "/admin/budgets/default",
            "body_len": 32,
            "headers": [
                {"name": "idempotency-key", "value": "budget-update-1"},
                {"name": "if-match", "value": "W/\"7\""}
            ],
            "body_digest": "sha256:budget-update-1"
        })
        .to_string(),
    )
    .expect("write http audit correlation error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http audit correlation error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "budget_update");
    assert_eq!(stdout["code"], "invalid_trace_context");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_reports_failed_otlp_http_log_when_export_fails() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-plan-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http root");
    let request_path = root.join("http-plan-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000030",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "request_id": "00000000-0000-7000-8000-000000000101",
            "call_id": "00000000-0000-7000-8000-000000000102",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/configuration/revisions",
            "body_len": 128,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "config-publish-1"}
            ],
            "body_digest": "sha256:config-publish-1"
        })
        .to_string(),
    )
    .expect("write http plan request");

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http plan with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "configuration_publish");
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_failure_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-correlation-error-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http audit correlation error root");
    let request_path = root.join("http-plan-request-audit-correlation-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000047",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "budget-default",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:budget-update-audit",
            "method": "PATCH",
            "path": "/admin/budgets/default",
            "body_len": 32,
            "headers": [
                {"name": "idempotency-key", "value": "budget-update-1"},
                {"name": "if-match", "value": "W/\"7\""}
            ],
            "body_digest": "sha256:budget-update-1"
        })
        .to_string(),
    )
    .expect("write http audit correlation error request");
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http audit correlation error plan with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "budget_update");
    assert_eq!(stdout["code"], "invalid_trace_context");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_failure_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset()
{
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-correlation-error-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http audit correlation error root");
    let request_path = root.join("http-plan-request-audit-correlation-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000047",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "budget-default",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:budget-update-audit",
            "method": "PATCH",
            "path": "/admin/budgets/default",
            "body_len": 32,
            "headers": [
                {"name": "idempotency-key", "value": "budget-update-1"},
                {"name": "if-match", "value": "W/\"7\""}
            ],
            "body_digest": "sha256:budget-update-1"
        })
        .to_string(),
    )
    .expect("write http audit correlation error request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http audit correlation error plan with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "budget_update");
    assert_eq!(stdout["code"], "invalid_trace_context");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_failure_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank()
{
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-correlation-error-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http audit correlation error root");
    let request_path = root.join("http-plan-request-audit-correlation-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000047",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "budget-default",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:budget-update-audit",
            "method": "PATCH",
            "path": "/admin/budgets/default",
            "body_len": 32,
            "headers": [
                {"name": "idempotency-key", "value": "budget-update-1"},
                {"name": "if-match", "value": "W/\"7\""}
            ],
            "body_digest": "sha256:budget-update-1"
        })
        .to_string(),
    )
    .expect("write http audit correlation error request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http audit correlation error plan with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "budget_update");
    assert_eq!(stdout["code"], "invalid_trace_context");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_failure_reports_failed_otlp_http_log_when_export_fails() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-correlation-error-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http audit correlation error root");
    let request_path = root.join("http-plan-request-audit-correlation-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000047",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "budget-default",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:budget-update-audit",
            "method": "PATCH",
            "path": "/admin/budgets/default",
            "body_len": 32,
            "headers": [
                {"name": "idempotency-key", "value": "budget-update-1"},
                {"name": "if-match", "value": "W/\"7\""}
            ],
            "body_digest": "sha256:budget-update-1"
        })
        .to_string(),
    )
    .expect("write http audit correlation error request");

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http audit correlation error plan with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "budget_update");
    assert_eq!(stdout["code"], "invalid_trace_context");
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_budget_update_audit_required() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http audit root");
    let request_path = root.join("http-plan-request-audit.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000046",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "budget-default",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:budget-update-audit",
            "method": "PATCH",
            "path": "/admin/budgets/default",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "budget-update-1"},
                {"name": "if-match", "value": "W/\"7\""}
            ],
            "body_digest": "sha256:budget-update-1"
        })
        .to_string(),
    )
    .expect("write http audit request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http audit plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "budget_update");
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.budget.update"
    );
    assert_eq!(stdout["audit"]["audit_outcome"], "success");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_budget_update_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-budget-update-route-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane budget update root");
    let request_path = root.join("http-plan-request-budget-update-route.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000046",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "budget-default",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:budget-update-audit",
            "method": "PATCH",
            "path": "/admin/budgets/default",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "budget-update-1"},
                {"name": "if-match", "value": "W/\"7\""}
            ],
            "body_digest": "sha256:budget-update-1"
        })
        .to_string(),
    )
    .expect("write budget update route request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run budget update route plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "budget_update");
    assert_eq!(stdout["resource_kind"], "budget");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.budget.update"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "budget-update-1");
    assert_eq!(stdout["precondition"]["present"], true);
    assert_eq!(stdout["precondition"]["entity_tag"], "W/\"7\"");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_budget_update_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-budget-update-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane budget update route error root");
    let request_path = root.join("http-plan-request-budget-update-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000046",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "budget-default",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/budgets/default",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:budget-update-route-error"
        })
        .to_string(),
    )
    .expect("write budget update route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run budget update route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_scim_user_create_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-create-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim create root");
    let request_path = root.join("http-plan-request-scim-create.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000050",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:scim-create-audit",
            "method": "POST",
            "path": "/scim/v2/Users",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "scim-create-1"}
            ],
            "body_digest": "sha256:scim-create-1"
        })
        .to_string(),
    )
    .expect("write scim create request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim create http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "scim_user_create");
    assert_eq!(stdout["resource_kind"], "user");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.scim_user.create"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "scim-create-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_scim_user_read_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-read-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim read root");
    let request_path = root.join("http-plan-request-scim-read.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000050",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "user-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:scim-read-audit",
            "method": "GET",
            "path": "/scim/v2/Users/user-1",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:virtual-key-update-route-error"
        })
        .to_string(),
    )
    .expect("write scim read request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim read http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "scim_user_read");
    assert_eq!(stdout["resource_kind"], "user");
    assert_eq!(stdout["requires_idempotency"], false);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.scim_user.read"
    );
    assert_eq!(stdout["idempotency"]["required"], false);
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_scim_user_update_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-update-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim update root");
    let request_path = root.join("http-plan-request-scim-update.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000051",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "user-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:scim-update-audit",
            "method": "PATCH",
            "path": "/scim/v2/Users/user-1",
            "body_len": 64,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "scim-update-1"},
                {"name": "if-match", "value": "W/\"9\""}
            ],
            "body_digest": "sha256:scim-update-1"
        })
        .to_string(),
    )
    .expect("write scim update request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim update http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "scim_user_update");
    assert_eq!(stdout["resource_kind"], "user");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "postgres");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.scim_user.update"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "scim-update-1");
    assert_eq!(stdout["precondition"]["present"], true);
    assert_eq!(stdout["precondition"]["entity_tag"], "W/\"9\"");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_scim_user_delete_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-delete-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim delete root");
    let request_path = root.join("http-plan-request-scim-delete.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000052",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "user-2",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:scim-delete-audit",
            "method": "DELETE",
            "path": "/scim/v2/Users/user-2",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "scim-delete-1"}
            ],
            "body_digest": "sha256:scim-delete-1"
        })
        .to_string(),
    )
    .expect("write scim delete request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim delete http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "scim_user_delete");
    assert_eq!(stdout["resource_kind"], "user");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.scim_user.delete"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "scim-delete-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_scim_user_create_missing_idempotency_key_error()
{
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim idempotency error root");
    let request_path = root.join("http-plan-request-scim-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000061",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/scim/v2/Users",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:scim-create-1"
        })
        .to_string(),
    )
    .expect("write scim idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "scim_user_create");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_idempotency_failure_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-idempotency-error-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim idempotency error root");
    let request_path = root.join("http-plan-request-scim-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000061",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/scim/v2/Users",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:scim-create-1"
        })
        .to_string(),
    )
    .expect("write scim idempotency error request");
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim idempotency error plan with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "scim_user_create");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));
    assert!(request.contains(
        "\"key\":\"trace_id\",\"value\":{\"stringValue\":\"4bf92f3577b34da6a3ce929d0e0e4736\"}"
    ));
    assert!(request.contains("\"traceId\":\"4bf92f3577b34da6a3ce929d0e0e4736\""));
    assert!(
        request.contains("\"key\":\"span_id\",\"value\":{\"stringValue\":\"00f067aa0ba902b7\"}")
    );
    assert!(request.contains("\"spanId\":\"00f067aa0ba902b7\""));
    assert!(request.contains("\"key\":\"trace_flags\",\"value\":{\"stringValue\":\"01\"}"));
    assert!(request.contains("\"flags\":1"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_idempotency_failure_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-idempotency-error-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim idempotency error root");
    let request_path = root.join("http-plan-request-scim-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000061",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/scim/v2/Users",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:scim-create-1"
        })
        .to_string(),
    )
    .expect("write scim idempotency error request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim idempotency error plan with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "scim_user_create");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_idempotency_failure_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-idempotency-error-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim idempotency error root");
    let request_path = root.join("http-plan-request-scim-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000061",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/scim/v2/Users",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:scim-create-1"
        })
        .to_string(),
    )
    .expect("write scim idempotency error request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim idempotency error plan with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "scim_user_create");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_idempotency_failure_reports_failed_otlp_http_log_when_export_fails()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-idempotency-error-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim idempotency error root");
    let request_path = root.join("http-plan-request-scim-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000061",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/scim/v2/Users",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:scim-create-1"
        })
        .to_string(),
    )
    .expect("write scim idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim idempotency error plan with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "scim_user_create");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_scim_user_delete_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim route error root");
    let request_path = root.join("http-plan-request-scim-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000062",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/scim/v2/Users",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:virtual-key-update-route-error"
        })
        .to_string(),
    )
    .expect("write scim route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_route_failure_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-route-error-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim route error root");
    let request_path = root.join("http-plan-request-scim-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000062",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/scim/v2/Users",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:virtual-key-update-route-error"
        })
        .to_string(),
    )
    .expect("write scim route error request");
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim route error plan with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));
    assert!(request.contains(
        "\"key\":\"trace_id\",\"value\":{\"stringValue\":\"4bf92f3577b34da6a3ce929d0e0e4736\"}"
    ));
    assert!(request.contains("\"traceId\":\"4bf92f3577b34da6a3ce929d0e0e4736\""));
    assert!(
        request.contains("\"key\":\"span_id\",\"value\":{\"stringValue\":\"00f067aa0ba902b7\"}")
    );
    assert!(request.contains("\"spanId\":\"00f067aa0ba902b7\""));
    assert!(request.contains("\"key\":\"trace_flags\",\"value\":{\"stringValue\":\"01\"}"));
    assert!(request.contains("\"flags\":1"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_route_failure_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-route-error-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim route error root");
    let request_path = root.join("http-plan-request-scim-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000062",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/scim/v2/Users",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:virtual-key-update-route-error"
        })
        .to_string(),
    )
    .expect("write scim route error request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim route error plan with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_route_failure_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-route-error-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim route error root");
    let request_path = root.join("http-plan-request-scim-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000062",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/scim/v2/Users",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:virtual-key-update-route-error"
        })
        .to_string(),
    )
    .expect("write scim route error request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim route error plan with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_route_failure_reports_failed_otlp_http_log_when_export_fails() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-route-error-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim route error root");
    let request_path = root.join("http-plan-request-scim-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000062",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/scim/v2/Users",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:virtual-key-update-route-error"
        })
        .to_string(),
    )
    .expect("write scim route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim route error plan with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_scim_user_read_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-read-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim read route error root");
    let request_path = root.join("http-plan-request-scim-read-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000050",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "user-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/scim/v2/Users/user-1",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:scim-read-route-error"
        })
        .to_string(),
    )
    .expect("write scim read route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim read route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_role_binding_grant_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-role-binding-grant-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane role binding grant root");
    let request_path = root.join("http-plan-request-role-binding-grant.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000053",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:role-binding-grant-audit",
            "method": "POST",
            "path": "/admin/role-bindings",
            "body_len": 80,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "role-binding-grant-1"}
            ],
            "body_digest": "sha256:role-binding-grant-1"
        })
        .to_string(),
    )
    .expect("write role binding grant request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run role binding grant http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "role_binding_grant");
    assert_eq!(stdout["resource_kind"], "role_binding");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.role_binding.grant"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "role-binding-grant-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_role_binding_revoke_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-role-binding-revoke-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane role binding revoke root");
    let request_path = root.join("http-plan-request-role-binding-revoke.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000054",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "binding-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:role-binding-revoke-audit",
            "method": "DELETE",
            "path": "/admin/role-bindings/binding-1",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "role-binding-revoke-1"}
            ],
            "body_digest": "sha256:role-binding-revoke-1"
        })
        .to_string(),
    )
    .expect("write role binding revoke request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run role binding revoke http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "role_binding_revoke");
    assert_eq!(stdout["resource_kind"], "role_binding");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "postgres");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.role_binding.revoke"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "role-binding-revoke-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_role_binding_grant_missing_idempotency_key_error()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-role-binding-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane role binding idempotency error root");
    let request_path = root.join("http-plan-request-role-binding-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000053",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/role-bindings",
            "body_len": 80,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:role-binding-grant-1"
        })
        .to_string(),
    )
    .expect("write role binding idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run role binding idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "role_binding_grant");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_role_binding_grant_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-role-binding-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane role binding route error root");
    let request_path = root.join("http-plan-request-role-binding-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000053",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/role-bindings",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write role binding route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run role binding route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_service_identity_create_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-service-identity-create-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane service identity root");
    let request_path = root.join("http-plan-request-service-identity-create.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000055",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:service-identity-create-audit",
            "method": "POST",
            "path": "/admin/service-identities",
            "body_len": 88,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "service-identity-create-1"}
            ],
            "body_digest": "sha256:service-identity-create-1"
        })
        .to_string(),
    )
    .expect("write service identity create request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run service identity create http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "service_identity_create");
    assert_eq!(stdout["resource_kind"], "service_identity");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.service_identity.create"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "service-identity-create-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_provider_credential_rotate_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-provider-credential-rotate-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane provider credential rotate root");
    let request_path = root.join("http-plan-request-provider-credential-rotate.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000056",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "provider-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:provider-credential-rotate-audit",
            "method": "POST",
            "path": "/admin/provider-credentials/provider-1/secret",
            "body_len": 72,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "provider-credential-rotate-1"}
            ],
            "body_digest": "sha256:provider-credential-rotate-1"
        })
        .to_string(),
    )
    .expect("write provider credential rotate request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run provider credential rotate http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "provider_credential_rotate");
    assert_eq!(stdout["resource_kind"], "provider_credential");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "postgres");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.provider_credential.rotate_secret"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "provider-credential-rotate-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_service_identity_create_missing_idempotency_key_error()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-service-identity-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root)
        .expect("create control-plane service identity idempotency error root");
    let request_path = root.join("http-plan-request-service-identity-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000055",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/service-identities",
            "body_len": 88,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:service-identity-create-1"
        })
        .to_string(),
    )
    .expect("write service identity idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run service identity idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "service_identity_create");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_service_identity_create_method_not_allowed_error()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-service-identity-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane service identity route error root");
    let request_path = root.join("http-plan-request-service-identity-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000055",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/service-identities",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write service identity route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run service identity route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_provider_credential_rotate_missing_idempotency_key_error()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-provider-credential-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root)
        .expect("create control-plane provider credential idempotency error root");
    let request_path = root.join("http-plan-request-provider-credential-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000056",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "provider-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/provider-credentials/provider-1/secret",
            "body_len": 72,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:provider-credential-rotate-1"
        })
        .to_string(),
    )
    .expect("write provider credential idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run provider credential idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "provider_credential_rotate");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_provider_credential_rotate_method_not_allowed_error()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-provider-credential-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane provider credential route error root");
    let request_path = root.join("http-plan-request-provider-credential-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000056",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "provider-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/provider-credentials/provider-1/secret",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write provider credential route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run provider credential route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_tenant_create_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-tenant-create-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane tenant create root");
    let request_path = root.join("http-plan-request-tenant-create.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000057",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:tenant-create-audit",
            "method": "POST",
            "path": "/admin/tenants",
            "body_len": 72,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "tenant-create-1"}
            ],
            "body_digest": "sha256:tenant-create-1"
        })
        .to_string(),
    )
    .expect("write tenant create request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run tenant create http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "tenant_create");
    assert_eq!(stdout["resource_kind"], "tenant");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.tenant.create"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "tenant-create-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_tenant_update_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-tenant-update-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane tenant update root");
    let request_path = root.join("http-plan-request-tenant-update.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000058",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "tenant-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:tenant-update-audit",
            "method": "PATCH",
            "path": "/v1/admin/tenants/tenant-1",
            "body_len": 64,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "tenant-update-1"},
                {"name": "if-match", "value": "W/\"3\""}
            ],
            "body_digest": "sha256:tenant-update-1"
        })
        .to_string(),
    )
    .expect("write tenant update request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run tenant update http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "tenant_update");
    assert_eq!(stdout["resource_kind"], "tenant");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "postgres");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.tenant.update"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "tenant-update-1");
    assert_eq!(stdout["precondition"]["present"], true);
    assert_eq!(stdout["precondition"]["entity_tag"], "W/\"3\"");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_tenant_create_missing_idempotency_key_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-tenant-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane tenant idempotency error root");
    let request_path = root.join("http-plan-request-tenant-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000057",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/tenants",
            "body_len": 72,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:tenant-create-1"
        })
        .to_string(),
    )
    .expect("write tenant idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run tenant idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "tenant_create");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_tenant_create_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-tenant-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane tenant route error root");
    let request_path = root.join("http-plan-request-tenant-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000057",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/tenants",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write tenant route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run tenant route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_user_invite_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-user-invite-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane user invite root");
    let request_path = root.join("http-plan-request-user-invite.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000059",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:user-invite-audit",
            "method": "POST",
            "path": "/admin/users/invites",
            "body_len": 84,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "user-invite-1"}
            ],
            "body_digest": "sha256:user-invite-1"
        })
        .to_string(),
    )
    .expect("write user invite request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run user invite http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "user_invite");
    assert_eq!(stdout["resource_kind"], "user");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(stdout["audit"]["audit_action"], "control_plane.user.invite");
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "user-invite-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_policy_publish_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-policy-publish-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane policy publish root");
    let request_path = root.join("http-plan-request-policy-publish.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000060",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:policy-publish-audit",
            "method": "POST",
            "path": "/admin/policies/revisions",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "policy-publish-1"}
            ],
            "body_digest": "sha256:policy-publish-1"
        })
        .to_string(),
    )
    .expect("write policy publish request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run policy publish http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "policy_publish");
    assert_eq!(stdout["resource_kind"], "policy");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "postgres");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.policy.publish"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "policy-publish-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_audit_retention_purge_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-retention-purge-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane audit retention purge root");
    let request_path = root.join("http-plan-request-audit-retention-purge.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000072",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "retention-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:audit-retention-purge-audit",
            "method": "DELETE",
            "path": "/admin/audit/retention",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "audit-retention-purge-1"}
            ],
            "body_digest": "sha256:audit-retention-purge-1"
        })
        .to_string(),
    )
    .expect("write audit retention purge request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run audit retention purge http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "audit_retention_purge");
    assert_eq!(stdout["resource_kind"], "audit_log");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.audit.retention_purge"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "audit-retention-purge-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_audit_export_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-export-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane audit export root");
    let request_path = root.join("http-plan-request-audit-export.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000071",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "export-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:audit-export-audit",
            "method": "POST",
            "path": "/admin/audit/exports?limit=25",
            "body_len": 64,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "audit-export-1"}
            ],
            "body_digest": "sha256:audit-export-1"
        })
        .to_string(),
    )
    .expect("write audit export request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run audit export http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "audit_export");
    assert_eq!(stdout["resource_kind"], "audit_log");
    assert_eq!(stdout["requires_idempotency"], false);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "postgres");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.audit.export"
    );
    assert_eq!(stdout["idempotency"]["required"], false);
    assert_eq!(stdout["page_request"]["limit"], 25);
    assert_eq!(stdout["page_request"]["cursor"], serde_json::Value::Null);
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_gateway_admin_read_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-gateway-admin-read-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane gateway admin read root");
    let request_path = root.join("http-plan-request-gateway-admin-read.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000073",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "GET",
            "path": "/prodex/gateway/admin",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write gateway admin read request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway admin read http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "gateway_admin_read");
    assert_eq!(stdout["resource_kind"], "configuration");
    assert_eq!(stdout["requires_idempotency"], false);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.gateway_admin.read"
    );
    assert_eq!(stdout["idempotency"]["required"], false);
    assert_eq!(stdout["page_request"]["limit"], 50);
    assert_eq!(stdout["page_request"]["cursor"], serde_json::Value::Null);
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_audit_retention_purge_missing_idempotency_key_error()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-retention-purge-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root)
        .expect("create control-plane audit retention purge idempotency error root");
    let request_path = root.join("http-plan-request-audit-retention-purge-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000072",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "retention-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/audit/retention",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:audit-retention-purge-1"
        })
        .to_string(),
    )
    .expect("write audit retention purge idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run audit retention purge idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "audit_retention_purge");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_gateway_admin_read_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-gateway-admin-read-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane gateway admin read route error root");
    let request_path = root.join("http-plan-request-gateway-admin-read-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000073",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/prodex/gateway/admin",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:gateway-admin-read-route-error"
        })
        .to_string(),
    )
    .expect("write gateway admin read route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway admin read route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_audit_export_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-export-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane audit export route error root");
    let request_path = root.join("http-plan-request-audit-export-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000071",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "export-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/audit/exports",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write audit export route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run audit export route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_audit_retention_purge_method_not_allowed_error()
{
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-retention-purge-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane audit retention purge route error root");
    let request_path = root.join("http-plan-request-audit-retention-purge-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000072",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "retention-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/audit/retention",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write audit retention purge route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run audit retention purge route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_policy_publish_missing_idempotency_key_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-policy-publish-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane policy publish idempotency error root");
    let request_path = root.join("http-plan-request-policy-publish-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000060",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/policies/revisions",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:policy-publish-1"
        })
        .to_string(),
    )
    .expect("write policy publish idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run policy publish idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "policy_publish");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_user_invite_missing_idempotency_key_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-user-invite-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane user invite idempotency error root");
    let request_path = root.join("http-plan-request-user-invite-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000059",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/users/invites",
            "body_len": 84,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:user-invite-1"
        })
        .to_string(),
    )
    .expect("write user invite idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run user invite idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "user_invite");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_policy_publish_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-policy-publish-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane policy publish route error root");
    let request_path = root.join("http-plan-request-policy-publish-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000060",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/policies/revisions",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write policy publish route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run policy publish route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_user_invite_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-user-invite-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane user invite route error root");
    let request_path = root.join("http-plan-request-user-invite-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000059",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/users/invites",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write user invite route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run user invite route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_virtual_key_create_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-virtual-key-create-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane virtual-key create root");
    let request_path = root.join("http-plan-request-virtual-key-create.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000048",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:virtual-key-create-audit",
            "method": "POST",
            "path": "/admin/keys",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "virtual-key-create-1"}
            ],
            "body_digest": "sha256:virtual-key-create-1"
        })
        .to_string(),
    )
    .expect("write virtual-key create request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run virtual-key create http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "virtual_key_create");
    assert_eq!(stdout["resource_kind"], "virtual_key");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.virtual_key.create"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "virtual-key-create-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_virtual_key_read_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-virtual-key-read-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane virtual-key read root");
    let request_path = root.join("http-plan-request-virtual-key-read.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000061",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:virtual-key-read-audit",
            "method": "GET",
            "path": "/prodex/gateway/keys?limit=25",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write virtual-key read request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run virtual-key read http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "virtual_key_read");
    assert_eq!(stdout["resource_kind"], "virtual_key");
    assert_eq!(stdout["requires_idempotency"], false);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.virtual_key.read"
    );
    assert_eq!(stdout["idempotency"]["required"], false);
    assert_eq!(stdout["page_request"]["limit"], 25);
    assert_eq!(stdout["page_request"]["cursor"], serde_json::Value::Null);
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_virtual_key_rotate_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-virtual-key-rotate-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane virtual-key rotate root");
    let request_path = root.join("http-plan-request-virtual-key-rotate.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000049",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "team-a-key",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:virtual-key-rotate-audit",
            "method": "POST",
            "path": "/admin/keys/team-a-key/secret",
            "body_len": 48,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "virtual-key-rotate-1"}
            ],
            "body_digest": "sha256:virtual-key-rotate-1"
        })
        .to_string(),
    )
    .expect("write virtual-key rotate request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run virtual-key rotate http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "virtual_key_rotate_secret");
    assert_eq!(stdout["resource_kind"], "virtual_key");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "postgres");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.virtual_key.rotate_secret"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "virtual-key-rotate-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_virtual_key_delete_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-virtual-key-delete-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane virtual-key delete root");
    let request_path = root.join("http-plan-request-virtual-key-delete.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000062",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "team-a-key",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:virtual-key-delete-audit",
            "method": "DELETE",
            "path": "/prodex/gateway/keys/team-a-key",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "virtual-key-delete-1"}
            ],
            "body_digest": "sha256:virtual-key-delete-1"
        })
        .to_string(),
    )
    .expect("write virtual-key delete request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run virtual-key delete http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "virtual_key_delete");
    assert_eq!(stdout["resource_kind"], "virtual_key");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "postgres");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.virtual_key.delete"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "virtual-key-delete-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_virtual_key_create_missing_idempotency_key_error()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-virtual-key-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane virtual-key idempotency error root");
    let request_path = root.join("http-plan-request-virtual-key-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000063",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/keys",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:virtual-key-create-1"
        })
        .to_string(),
    )
    .expect("write virtual-key idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run virtual-key idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "virtual_key_create");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_virtual_key_create_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-virtual-key-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane virtual-key route error root");
    let request_path = root.join("http-plan-request-virtual-key-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000064",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/keys",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write virtual-key route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run virtual-key route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_virtual_key_delete_missing_idempotency_key_error()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-virtual-key-delete-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root)
        .expect("create control-plane virtual-key delete idempotency error root");
    let request_path = root.join("http-plan-request-virtual-key-delete-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000062",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "team-a-key",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/prodex/gateway/keys/team-a-key",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:virtual-key-delete-1"
        })
        .to_string(),
    )
    .expect("write virtual-key delete idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run virtual-key delete idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "virtual_key_delete");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_virtual_key_read_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-virtual-key-read-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane virtual-key read route error root");
    let request_path = root.join("http-plan-request-virtual-key-read-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000061",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "PATCH",
            "path": "/prodex/gateway/keys",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:virtual-key-read-route-error"
        })
        .to_string(),
    )
    .expect("write virtual-key read route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run virtual-key read route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_configuration_publish_missing_idempotency_key_error()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-plan-denied-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http denied root");
    let request_path = root.join("http-plan-request-denied.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000040",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/configuration/revisions",
            "body_len": 128,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:config-publish-1"
        })
        .to_string(),
    )
    .expect("write denied http plan request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run denied http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "configuration_publish");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_virtual_key_update_precondition_required() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-precondition-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http precondition root");
    let request_path = root.join("http-plan-request-precondition.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000042",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "team-a-key",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "PATCH",
            "path": "/admin/keys/team-a-key",
            "body_len": 64,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "team-a-key-patch-1"},
                {"name": "if-match", "value": "W/\"42\""}
            ],
            "body_digest": "sha256:team-a-key-patch-1"
        })
        .to_string(),
    )
    .expect("write http precondition request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http precondition plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "virtual_key_update");
    assert_eq!(stdout["precondition"]["present"], true);
    assert_eq!(stdout["precondition"]["entity_tag"], "W/\"42\"");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_virtual_key_update_entity_tag_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-precondition-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http precondition error root");
    let request_path = root.join("http-plan-request-precondition-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000043",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "team-a-key",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "PATCH",
            "path": "/admin/keys/team-a-key",
            "body_len": 64,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "team-a-key-patch-1"},
                {"name": "if-match", "value": ""}
            ],
            "body_digest": "sha256:team-a-key-patch-1"
        })
        .to_string(),
    )
    .expect("write http precondition error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http precondition error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "virtual_key_update");
    assert_eq!(stdout["code"], "entity_tag_invalid");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_precondition_failure_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-precondition-error-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http precondition error root");
    let request_path = root.join("http-plan-request-precondition-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000043",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "team-a-key",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "PATCH",
            "path": "/admin/keys/team-a-key",
            "body_len": 64,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "team-a-key-patch-1"},
                {"name": "if-match", "value": ""}
            ],
            "body_digest": "sha256:team-a-key-patch-1"
        })
        .to_string(),
    )
    .expect("write http precondition error request");
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http precondition error plan with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "virtual_key_update");
    assert_eq!(stdout["code"], "entity_tag_invalid");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));
    assert!(request.contains(
        "\"key\":\"trace_id\",\"value\":{\"stringValue\":\"4bf92f3577b34da6a3ce929d0e0e4736\"}"
    ));
    assert!(request.contains("\"traceId\":\"4bf92f3577b34da6a3ce929d0e0e4736\""));
    assert!(
        request.contains("\"key\":\"span_id\",\"value\":{\"stringValue\":\"00f067aa0ba902b7\"}")
    );
    assert!(request.contains("\"spanId\":\"00f067aa0ba902b7\""));
    assert!(request.contains("\"key\":\"trace_flags\",\"value\":{\"stringValue\":\"01\"}"));
    assert!(request.contains("\"flags\":1"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_precondition_failure_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-precondition-error-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http precondition error root");
    let request_path = root.join("http-plan-request-precondition-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000043",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "team-a-key",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "PATCH",
            "path": "/admin/keys/team-a-key",
            "body_len": 64,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "team-a-key-patch-1"},
                {"name": "if-match", "value": ""}
            ],
            "body_digest": "sha256:team-a-key-patch-1"
        })
        .to_string(),
    )
    .expect("write http precondition error request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http precondition error plan with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "virtual_key_update");
    assert_eq!(stdout["code"], "entity_tag_invalid");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_precondition_failure_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-precondition-error-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http precondition error root");
    let request_path = root.join("http-plan-request-precondition-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000043",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "team-a-key",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "PATCH",
            "path": "/admin/keys/team-a-key",
            "body_len": 64,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "team-a-key-patch-1"},
                {"name": "if-match", "value": ""}
            ],
            "body_digest": "sha256:team-a-key-patch-1"
        })
        .to_string(),
    )
    .expect("write http precondition error request");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http precondition error plan with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "virtual_key_update");
    assert_eq!(stdout["code"], "entity_tag_invalid");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_precondition_failure_reports_failed_otlp_http_log_when_export_fails()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-precondition-error-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http precondition error root");
    let request_path = root.join("http-plan-request-precondition-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000043",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "team-a-key",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "PATCH",
            "path": "/admin/keys/team-a-key",
            "body_len": 64,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "team-a-key-patch-1"},
                {"name": "if-match", "value": ""}
            ],
            "body_digest": "sha256:team-a-key-patch-1"
        })
        .to_string(),
    )
    .expect("write http precondition error request");

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http precondition error plan with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "virtual_key_update");
    assert_eq!(stdout["code"], "entity_tag_invalid");
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_virtual_key_update_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-virtual-key-update-route-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane virtual-key update root");
    let request_path = root.join("http-plan-request-virtual-key-update-route.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000074",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "team-a-key",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:virtual-key-update-audit",
            "method": "PATCH",
            "path": "/prodex/gateway/keys/team-a-key",
            "body_len": 64,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "virtual-key-update-1"},
                {"name": "if-match", "value": "W/\"5\""}
            ],
            "body_digest": "sha256:virtual-key-update-1"
        })
        .to_string(),
    )
    .expect("write virtual-key update route request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run virtual-key update route plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "virtual_key_update");
    assert_eq!(stdout["resource_kind"], "virtual_key");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "postgres");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.virtual_key.update"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_eq!(stdout["idempotency"]["key"], "virtual-key-update-1");
    assert_eq!(stdout["precondition"]["present"], true);
    assert_eq!(stdout["precondition"]["entity_tag"], "W/\"5\"");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_virtual_key_update_route_invalid_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-virtual-key-update-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane virtual-key update route error root");
    let request_path = root.join("http-plan-request-virtual-key-update-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000074",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "team-a-key",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/prodex/gateway/keys/team-a-key",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:virtual-key-update-route-error"
        })
        .to_string(),
    )
    .expect("write virtual-key update route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run virtual-key update route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "bad_request");
    assert_eq!(stdout["code"], "control_plane_route_invalid");
    assert_eq!(stdout["message"], "control-plane route is invalid");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_billing_read_page_request_query() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-page-request-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http page request root");
    let request_path = root.join("http-plan-request-page-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000044",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "GET",
            "path": "/admin/billing?limit=25",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write http page request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http page request plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "billing_read");
    assert_eq!(stdout["page_request"]["limit"], 25);
    assert_eq!(stdout["page_request"]["cursor"], serde_json::Value::Null);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_billing_read_pagination_cursor_invalid_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-page-request-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http page request error root");
    let request_path = root.join("http-plan-request-page-request-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000045",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "GET",
            "path": "/admin/billing?cursor=",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write http page request error");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http page request error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "billing_read");
    assert_eq!(stdout["code"], "pagination_cursor_invalid");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_page_request_failure_exports_otlp_http_log_when_configured() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-page-request-error-otlp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http page request error root");
    let request_path = root.join("http-plan-request-page-request-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000045",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "GET",
            "path": "/admin/billing?cursor=",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write http page request error");
    let (endpoint, server) = start_otlp_capture_server();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http page request error plan with OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "billing_read");
    assert_eq!(stdout["code"], "pagination_cursor_invalid");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));
    assert!(request.contains(
        "\"key\":\"trace_id\",\"value\":{\"stringValue\":\"4bf92f3577b34da6a3ce929d0e0e4736\"}"
    ));
    assert!(request.contains("\"traceId\":\"4bf92f3577b34da6a3ce929d0e0e4736\""));
    assert!(
        request.contains("\"key\":\"span_id\",\"value\":{\"stringValue\":\"00f067aa0ba902b7\"}")
    );
    assert!(request.contains("\"spanId\":\"00f067aa0ba902b7\""));
    assert!(request.contains("\"key\":\"trace_flags\",\"value\":{\"stringValue\":\"01\"}"));
    assert!(request.contains("\"flags\":1"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_page_request_failure_uses_otlp_endpoint_fallback_when_logs_endpoint_is_unset()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-page-request-error-otlp-endpoint-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http page request error root");
    let request_path = root.join("http-plan-request-page-request-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000045",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "GET",
            "path": "/admin/billing?cursor=",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write http page request error");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env_remove("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http page request error plan with OTLP endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "billing_read");
    assert_eq!(stdout["code"], "pagination_cursor_invalid");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_page_request_failure_uses_otlp_endpoint_fallback_when_logs_endpoint_is_blank()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-page-request-error-otlp-endpoint-blank-fallback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http page request error root");
    let request_path = root.join("http-plan-request-page-request-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000045",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "GET",
            "path": "/admin/billing?cursor=",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write http page request error");
    let (logs_endpoint, server) = start_otlp_capture_server();
    let endpoint = logs_endpoint.trim_end_matches("/v1/logs").to_string();

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", "   ")
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http page request error plan with blank OTLP logs endpoint fallback");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "billing_read");
    assert_eq!(stdout["code"], "pagination_cursor_invalid");
    assert_eq!(stdout["otlp_log_export"], "exported");

    let request = server.join().expect("OTLP server should join");
    assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
    assert!(request.contains("\"stringValue\":\"prodex-control-plane\""));
    assert!(request.contains("\"stringValue\":\"control_plane.http_route.plan\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_http_plan_page_request_failure_reports_failed_otlp_http_log_when_export_fails()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-page-request-error-otlp-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http page request error root");
    let request_path = root.join("http-plan-request-page-request-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000045",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "GET",
            "path": "/admin/billing?cursor=",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write http page request error");

    let output = Command::new(bin("prodex-control-plane"))
        .env("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", closed_otlp_endpoint())
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http page request error plan with failed OTLP");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "billing_read");
    assert_eq!(stdout["code"], "pagination_cursor_invalid");
    assert_eq!(stdout["otlp_log_export"], "failed");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_billing_read_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-billing-read-route-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane billing read root");
    let request_path = root.join("http-plan-request-billing-read-route.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000044",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "GET",
            "path": "/admin/billing?limit=25",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write billing read route request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run billing read route plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "billing_read");
    assert_eq!(stdout["resource_kind"], "billing");
    assert_eq!(stdout["requires_idempotency"], false);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["page_request"]["limit"], 25);
    assert_eq!(stdout["page_request"]["cursor"], serde_json::Value::Null);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_billing_read_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-billing-read-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane billing read route error root");
    let request_path = root.join("http-plan-request-billing-read-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000044",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/billing",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:billing-read-route-error"
        })
        .to_string(),
    )
    .expect("write billing read route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run billing read route error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_configuration_publish_method_not_allowed_error()
{
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http route error root");
    let request_path = root.join("http-plan-request-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000041",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/configuration/revisions",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write route error http plan request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run route error http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["status"], "method_not_allowed");
    assert_eq!(stdout["code"], "control_plane_method_not_allowed");
    assert_eq!(
        stdout["message"],
        "HTTP method is not allowed for this control-plane route"
    );
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}
