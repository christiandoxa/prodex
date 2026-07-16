use super::*;

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
    assert_scoped_idempotency_key!(&stdout, 30, "config-publish-1");
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
