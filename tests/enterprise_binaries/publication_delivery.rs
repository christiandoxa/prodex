use super::*;

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
