use super::*;

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
    assert_scoped_idempotency_key!(&stdout, 74, "virtual-key-update-1");
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
