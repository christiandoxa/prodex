use super::*;

#[test]
fn control_plane_binary_plans_http_control_plane_gateway_admin_read_route() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-gateway-admin-read-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane gateway admin read root");
    let request_path = root.join("http-plan-request-gateway-admin-read.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000073",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "GET",
            "path": "/prodex/gateway/admin",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write gateway admin read request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway admin read http plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], true);
    assert_eq!(stdout["operation"], "gateway_admin_read");
    assert_eq!(stdout["resource_kind"], "configuration");
    assert_eq!(stdout["requires_idempotency"], false);
    assert_eq!(stdout["requires_audit"], true);
    assert_eq!(stdout["audit"]["required"], true);
    assert_eq!(
        stdout["audit"]["audit_action"],
        "control_plane.gateway_admin.read"
    );
    assert_eq!(stdout["idempotency"]["required"], false);
    assert_eq!(stdout["page_request"]["limit"], 50);
    assert_eq!(stdout["page_request"]["cursor"], serde_json::Value::Null);
    assert_eq!(stdout["precondition"]["present"], false);
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_audit_retention_purge_missing_idempotency_key_error()
 {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-retention-purge-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root)
        .expect("create control-plane audit retention purge idempotency error root");
    let request_path = root.join("http-plan-request-audit-retention-purge-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000072",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "retention-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/audit/retention",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:audit-retention-purge-1"
        })
        .to_string(),
    )
    .expect("write audit retention purge idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run audit retention purge idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "audit_retention_purge");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_gateway_admin_read_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-gateway-admin-read-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane gateway admin read route error root");
    let request_path = root.join("http-plan-request-gateway-admin-read-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000073",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "viewer",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/prodex/gateway/admin",
            "body_len": 32,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:gateway-admin-read-route-error"
        })
        .to_string(),
    )
    .expect("write gateway admin read route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run gateway admin read route error plan");
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
fn control_plane_binary_reports_http_control_plane_audit_export_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-export-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane audit export route error root");
    let request_path = root.join("http-plan-request-audit-export-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000071",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "export-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/audit/exports",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write audit export route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run audit export route error plan");
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
fn control_plane_binary_reports_http_control_plane_audit_retention_purge_method_not_allowed_error()
{
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-audit-retention-purge-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane audit retention purge route error root");
    let request_path = root.join("http-plan-request-audit-retention-purge-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000072",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "retention-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/audit/retention",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write audit retention purge route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run audit retention purge route error plan");
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
fn control_plane_binary_reports_http_control_plane_policy_publish_missing_idempotency_key_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-policy-publish-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane policy publish idempotency error root");
    let request_path = root.join("http-plan-request-policy-publish-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000060",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/policies/revisions",
            "body_len": 96,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:policy-publish-1"
        })
        .to_string(),
    )
    .expect("write policy publish idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run policy publish idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "policy_publish");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_user_invite_missing_idempotency_key_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-user-invite-idempotency-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane user invite idempotency error root");
    let request_path = root.join("http-plan-request-user-invite-idempotency-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000059",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "POST",
            "path": "/admin/users/invites",
            "body_len": 84,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": "sha256:user-invite-1"
        })
        .to_string(),
    )
    .expect("write user invite idempotency error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run user invite idempotency error plan");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["planned"], false);
    assert_eq!(stdout["operation"], "user_invite");
    assert_eq!(stdout["code"], "control_plane_idempotency_key_required");
    assert_eq!(stdout["otlp_log_export"], "disabled");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_http_control_plane_policy_publish_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-policy-publish-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane policy publish route error root");
    let request_path = root.join("http-plan-request-policy-publish-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000060",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": "revision-1",
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/policies/revisions",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write policy publish route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run policy publish route error plan");
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
fn control_plane_binary_reports_http_control_plane_user_invite_method_not_allowed_error() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-http-user-invite-route-error-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane user invite route error root");
    let request_path = root.join("http-plan-request-user-invite-route-error.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000059",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "resource_id": serde_json::Value::Null,
            "occurred_at_unix_ms": 1700000000000u64,
            "method": "DELETE",
            "path": "/admin/users/invites",
            "body_len": 0,
            "headers": [
                {"name": "traceparent", "value": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"}
            ],
            "body_digest": serde_json::Value::Null
        })
        .to_string(),
    )
    .expect("write user invite route error request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-http-control-plane",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run user invite route error plan");
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
