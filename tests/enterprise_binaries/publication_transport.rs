use super::*;

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
