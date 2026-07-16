use super::*;

#[test]
fn control_plane_binary_plans_http_control_plane_budget_update_audit_required() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane http audit root");
    let request_path = root.join("http-plan-request-audit.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000046",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "budget-default",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:budget-update-audit",
            "method": "PATCH",
            "path": "/admin/budgets/default",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "budget-update-1"},
                {"name": "if-match", "value": "W/\"7\""}
            ],
            "body_digest": "sha256:budget-update-1"
        })
        .to_string(),
    )
    .expect("write http audit request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run http audit plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "budget_update");
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.budget.update"
    );
    assert_eq!(stdout["audit"]["audit_outcome"], "success");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_budget_update_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-budget-update-route-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane budget update root");
    let request_path = root.join("http-plan-request-budget-update-route.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000046",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "budget-default",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:budget-update-audit",
            "method": "PATCH",
            "path": "/admin/budgets/default",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "budget-update-1"},
                {"name": "if-match", "value": "W/\"7\""}
            ],
            "body_digest": "sha256:budget-update-1"
        })
        .to_string(),
    )
    .expect("write budget update route request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run budget update route plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "budget_update");
    assert_eq!(stdout["resource_kind"], "budget");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.budget.update"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_scoped_idempotency_key!(&stdout, 46, "budget-update-1");
    assert_eq!(stdout["precondition"]["present"], true);
    assert_eq!(stdout["precondition"]["entity_tag"], "W/\"7\"");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_budget_update_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-budget-update-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane budget update route error root");
    let request_path = root.join("http-plan-request-budget-update-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000046",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "budget-default",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/budgets/default",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:budget-update-route-error"
        })
        .to_string(),
    )
    .expect("write budget update route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run budget update route error plan");
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
fn control_plane_binary_plans_http_control_plane_scim_user_create_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-create-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim create root");
    let request_path = root.join("http-plan-request-scim-create.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000050",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:scim-create-audit",
            "method": "POST",
            "path": "/scim/v2/Users",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "scim-create-1"}
            ],
            "body_digest": "sha256:scim-create-1"
        })
        .to_string(),
    )
    .expect("write scim create request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim create http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "scim_user_create");
    assert_eq!(stdout["resource_kind"], "user");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.scim_user.create"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_scoped_idempotency_key!(&stdout, 50, "scim-create-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_scim_user_read_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-read-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim read root");
    let request_path = root.join("http-plan-request-scim-read.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000050",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "user-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:scim-read-audit",
            "method": "GET",
            "path": "/scim/v2/Users/user-1",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:virtual-key-update-route-error"
        })
        .to_string(),
    )
    .expect("write scim read request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim read http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "scim_user_read");
    assert_eq!(stdout["resource_kind"], "user");
    assert_eq!(stdout["requires_idempotency"], false);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.scim_user.read"
    );
    assert_eq!(stdout["idempotency"]["required"], false);
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_scim_user_update_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-update-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim update root");
    let request_path = root.join("http-plan-request-scim-update.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000051",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "user-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "postgres",
            "event_digest": "sha256:scim-update-audit",
            "method": "PATCH",
            "path": "/scim/v2/Users/user-1",
            "body_len": 64,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "scim-update-1"},
                {"name": "if-match", "value": "W/\"9\""}
            ],
            "body_digest": "sha256:scim-update-1"
        })
        .to_string(),
    )
    .expect("write scim update request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim update http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "scim_user_update");
    assert_eq!(stdout["resource_kind"], "user");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "postgres");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.scim_user.update"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_scoped_idempotency_key!(&stdout, 51, "scim-update-1");
    assert_eq!(stdout["precondition"]["present"], true);
    assert_eq!(stdout["precondition"]["entity_tag"], "W/\"9\"");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_http_control_plane_scim_user_delete_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-delete-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane scim delete root");
    let request_path = root.join("http-plan-request-scim-delete.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000052",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "user-2",
            "occurred_at_unix_ms": 1700000000000u64,
            "durable_store": "sqlite",
            "event_digest": "sha256:scim-delete-audit",
            "method": "DELETE",
            "path": "/scim/v2/Users/user-2",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"},
                {"name": "idempotency-key", "value": "scim-delete-1"}
            ],
            "body_digest": "sha256:scim-delete-1"
        })
        .to_string(),
    )
    .expect("write scim delete request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim delete http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "scim_user_delete");
    assert_eq!(stdout["resource_kind"], "user");
    assert_eq!(stdout["requires_idempotency"], true);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(stdout["audit"]["storage_backend"], "sqlite");
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.scim_user.delete"
    );
    assert_eq!(stdout["idempotency"]["required"], true);
    assert_scoped_idempotency_key!(&stdout, 52, "scim-delete-1");
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_scim_user_create_missing_idempotency_key_error()
{
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-scim-idempotency-error-{}-{}",
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
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run scim idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "scim_user_create");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}
