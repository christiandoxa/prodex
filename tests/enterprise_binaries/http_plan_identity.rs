use super::*;

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
    assert_scoped_idempotency_key!(&stdout, 55, "service-identity-create-1");
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
    assert_scoped_idempotency_key!(&stdout, 56, "provider-credential-rotate-1");
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
