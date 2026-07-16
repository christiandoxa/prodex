use super::*;

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
