use super::*;

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
    assert_scoped_idempotency_key!(&stdout, 48, "virtual-key-create-1");
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
    assert_scoped_idempotency_key!(&stdout, 49, "virtual-key-rotate-1");
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
    assert_scoped_idempotency_key!(&stdout, 62, "virtual-key-delete-1");
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
