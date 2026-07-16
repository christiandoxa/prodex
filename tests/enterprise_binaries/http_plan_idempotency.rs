use super::*;

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
