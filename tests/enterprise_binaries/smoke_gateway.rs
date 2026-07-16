use super::*;

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
fn enterprise_serve_commands_reject_invalid_listen_addresses_before_startup() {
    for name in ["prodex-gateway", "prodex-control-plane"] {
        let output = Command::new(bin(name))
            .args(["serve", "--listen", "invalid"])
            .output()
            .expect("run serve command with invalid listen address");
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .starts_with("invalid serve listen address\n")
        );
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
