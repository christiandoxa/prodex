use super::*;

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
