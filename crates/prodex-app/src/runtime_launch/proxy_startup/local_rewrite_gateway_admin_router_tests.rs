use super::super::local_rewrite_application_boundary::{
    RuntimeGatewayAdminPreauthorization, runtime_gateway_admin_control_plane_action,
    runtime_gateway_admin_control_plane_tenant_id,
    runtime_gateway_application_control_plane_authorization,
    runtime_gateway_application_request_context,
};
use super::super::local_rewrite_gateway_admin_auth::{
    RuntimeGatewayAdminAuthentication, RuntimeGatewayAdminCredentialEvidence,
};
use super::super::local_rewrite_gateway_admin_execution::runtime_gateway_admin_mutation_execution;
use super::{
    ControlPlaneDecision, GatewayHttpMethod, GatewayHttpRequestMeta, RuntimeGatewayAdminAuth,
    RuntimeGatewayAdminRole, RuntimeProxyRequest, plan_application_control_plane,
    runtime_gateway_admin_route_explain_plan, runtime_gateway_http_headers,
    runtime_gateway_http_request_meta,
};
use prodex_application::{
    ApplicationRequestContext, ApplicationRequestDeadline,
    plan_application_control_plane_audit_from_http,
    plan_application_control_plane_idempotency_from_http_digest,
};
use prodex_control_plane::ControlPlaneOperation;
use prodex_domain::{RequestId, ResourceKind, TenantId};
use prodex_gateway_http::{CanonicalRequestTarget, GatewayHttpHeader};
use std::io::Read;
use std::time::{Duration, Instant};

fn application_request_context<'a>(
    target: &'a CanonicalRequestTarget,
) -> ApplicationRequestContext<'a> {
    runtime_gateway_application_request_context(
        target,
        RequestId::new(),
        ApplicationRequestDeadline::at(Instant::now() + Duration::from_secs(30)),
        &[],
    )
    .unwrap()
}

#[test]
fn runtime_gateway_http_headers_preserve_exact_header_order_and_values() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/prodex/gateway/keys".to_string(),
        headers: vec![
            ("Idempotency-Key".to_string(), "idem-1".to_string()),
            ("If-Match".to_string(), "\"gateway-key-1\"".to_string()),
        ],
        body: Vec::new(),
    };

    let headers = runtime_gateway_http_headers(&request);
    assert_eq!(headers.len(), 2);
    assert_eq!(headers[0].name, "Idempotency-Key");
    assert_eq!(headers[0].value, "idem-1");
    assert_eq!(headers[1].name, "If-Match");
    assert_eq!(headers[1].value, "\"gateway-key-1\"");
}

#[test]
fn runtime_gateway_admin_authorization_uses_the_application_boundary() {
    let http = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/v1/prodex/gateway/keys".to_string(),
        body_len: 0,
        headers: Vec::new(),
    };
    let viewer = RuntimeGatewayAdminAuthentication {
        evidence: RuntimeGatewayAdminCredentialEvidence::Principal,
        auth: RuntimeGatewayAdminAuth {
            name: "viewer".to_string(),
            role: RuntimeGatewayAdminRole::Viewer,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        },
    };
    let admin = RuntimeGatewayAdminAuthentication {
        evidence: RuntimeGatewayAdminCredentialEvidence::Principal,
        auth: RuntimeGatewayAdminAuth {
            name: "admin".to_string(),
            role: RuntimeGatewayAdminRole::Admin,
            tenant_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_key_prefixes: Vec::new(),
        },
    };

    let target = CanonicalRequestTarget::parse("/v1/prodex/gateway/keys").unwrap();
    let request = application_request_context(&target);
    assert!(
        runtime_gateway_application_control_plane_authorization(&request, &http, &viewer).is_err()
    );
    let authorized =
        runtime_gateway_application_control_plane_authorization(&request, &http, &admin)
            .expect("admin authorization should produce an application plan");
    let preauthorized = RuntimeGatewayAdminPreauthorization {
        auth: admin_auth_named("admin", None),
        application: Some(authorized),
    };
    let action = preauthorized
        .control_plane_action()
        .expect("preauthorization should retain the exact application action");
    assert_eq!(action.operation, ControlPlaneOperation::VirtualKeyCreate);
    assert_eq!(action.requirement.resource, ResourceKind::VirtualKey);
    assert_eq!(
        action.tenant.tenant_id,
        runtime_gateway_admin_control_plane_tenant_id(&preauthorized.auth)
    );

    let explain_http = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/v1/prodex/gateway/routes/explain".to_string(),
        body_len: 2,
        headers: Vec::new(),
    };
    assert!(runtime_gateway_admin_route_explain_plan(&explain_http, &viewer.auth).is_some());
    assert!(runtime_gateway_admin_route_explain_plan(&explain_http, &admin.auth).is_some());
}

fn admin_auth_named(name: &str, tenant_id: Option<String>) -> RuntimeGatewayAdminAuth {
    RuntimeGatewayAdminAuth {
        name: name.to_string(),
        role: RuntimeGatewayAdminRole::Admin,
        tenant_id,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
    }
}

#[test]
fn admin_control_plane_action_uses_stable_admin_identity() {
    let http = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/v1/prodex/gateway/keys".to_string(),
        body_len: 0,
        headers: Vec::new(),
    };
    let admin = admin_auth_named("admin", None);
    let first = runtime_gateway_admin_control_plane_action(&http, &admin)
        .expect("admin action should plan");
    let second = runtime_gateway_admin_control_plane_action(&http, &admin)
        .expect("admin action should plan");
    assert_eq!(first.principal, second.principal);
    assert_eq!(first.resource.tenant_id, second.resource.tenant_id);

    let other =
        runtime_gateway_admin_control_plane_action(&http, &admin_auth_named("other-admin", None))
            .expect("other admin action should plan");
    assert_ne!(first.principal, other.principal);
    assert_ne!(first.resource.tenant_id, other.resource.tenant_id);
}

#[test]
fn admin_control_plane_action_prefers_typed_tenant_scope() {
    let valid_tenant_id = TenantId::new();
    let admin = admin_auth_named("tenant-admin", Some(valid_tenant_id.to_string()));

    assert_eq!(
        runtime_gateway_admin_control_plane_tenant_id(&admin),
        valid_tenant_id
    );

    let explain = runtime_gateway_admin_control_plane_action(
        &GatewayHttpRequestMeta {
            method: GatewayHttpMethod::Post,
            path: "/v1/prodex/gateway/routes/explain".to_string(),
            body_len: 2,
            headers: Vec::new(),
        },
        &admin,
    )
    .expect("route explain action should plan");
    assert_eq!(explain.resource.tenant_id, valid_tenant_id);
    assert!(matches!(
        plan_application_control_plane(explain).decision,
        ControlPlaneDecision::Authorized(_)
    ));
}

#[test]
fn admin_control_plane_action_binds_mutation_resource_and_time() {
    let admin = admin_auth_named("admin", None);
    for (method, path, expected_resource_id) in [
        (
            GatewayHttpMethod::Patch,
            "/v1/prodex/gateway/keys/example-key",
            "example-key",
        ),
        (
            GatewayHttpMethod::Post,
            "/v1/prodex/gateway/keys/example-key/secret",
            "example-key",
        ),
        (
            GatewayHttpMethod::Delete,
            "/v1/prodex/gateway/scim/v2/Users/user-1",
            "user-1",
        ),
    ] {
        let action = runtime_gateway_admin_control_plane_action(
            &GatewayHttpRequestMeta {
                method,
                path: path.to_string(),
                body_len: 0,
                headers: Vec::new(),
            },
            &admin,
        )
        .expect("admin mutation should map to an application action");

        assert_eq!(action.resource.id.as_deref(), Some(expected_resource_id));
        assert!(action.occurred_at_unix_ms > 0);
    }
}

#[test]
fn admin_control_plane_action_does_not_synthesize_random_ids() {
    let source = include_str!("local_rewrite_application_boundary.rs");
    for fallback in [
        ["let tenant_id = ", "TenantId::new();"].join(""),
        ["PrincipalId", "::new()"].join(""),
    ] {
        assert!(!source.contains(&fallback));
    }
}

#[test]
fn admin_mutation_routes_match_application_idempotency_and_audit_planners() {
    let admin = admin_auth_named("admin", None);
    for (method, path) in [
        (GatewayHttpMethod::Post, "/v1/prodex/gateway/keys"),
        (
            GatewayHttpMethod::Patch,
            "/v1/prodex/gateway/keys/example-key",
        ),
        (
            GatewayHttpMethod::Delete,
            "/v1/prodex/gateway/keys/example-key",
        ),
        (GatewayHttpMethod::Post, "/v1/prodex/gateway/scim/v2/Users"),
        (
            GatewayHttpMethod::Patch,
            "/v1/prodex/gateway/scim/v2/Users/user-1",
        ),
        (
            GatewayHttpMethod::Delete,
            "/v1/prodex/gateway/scim/v2/Users/user-1",
        ),
    ] {
        let http = GatewayHttpRequestMeta {
            method,
            path: path.to_string(),
            body_len: 2,
            headers: vec![GatewayHttpHeader::new("Idempotency-Key", "mutation-1")],
        };
        let action = runtime_gateway_admin_control_plane_action(&http, &admin)
            .expect("legacy admin mutation should map to an application action");

        assert!(action.operation.requires_idempotency(), "path={path}");
        let audit = plan_application_control_plane_audit_from_http(action.clone(), &http)
            .expect("admin mutation should require canonical audit routing");
        assert!(audit.route.requires_audit, "path={path}");
        let idempotency = plan_application_control_plane_idempotency_from_http_digest(
            action,
            &http,
            "sha256:mutation",
        )
        .expect("admin mutation should accept canonical idempotency planning");
        assert!(idempotency.operation.is_some(), "path={path}");
    }
}

#[test]
fn admin_mutation_execution_preserves_retained_authorization() {
    let admin = admin_auth_named("admin", None);
    let captured = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/prodex/gateway/keys".to_string(),
        headers: vec![("Idempotency-Key".to_string(), "mutation-1".to_string())],
        body: b"{}".to_vec(),
    };
    let http = runtime_gateway_http_request_meta(&captured, "/v1/prodex/gateway/keys");
    let action = runtime_gateway_admin_control_plane_action(&http, &admin).unwrap();
    let base = match plan_application_control_plane(action).decision {
        ControlPlaneDecision::Authorized(base) => base,
        ControlPlaneDecision::Denied { .. } => panic!("admin mutation should be authorized"),
    };

    let execution = match runtime_gateway_admin_mutation_execution(
        &captured,
        "/v1/prodex/gateway/keys",
        &admin,
        &base,
        ControlPlaneOperation::VirtualKeyCreate,
    ) {
        Ok(execution) => execution,
        Err(_) => panic!("mutation execution should plan"),
    };

    assert_eq!(execution.authorized_action, base);
    assert_eq!(execution.atomic_write.audit_event, base.audit_event);
    assert_eq!(
        execution.atomic_write.operation.tenant_id,
        base.tenant.tenant_id
    );
    assert!(
        execution
            .atomic_write
            .operation
            .key
            .as_str()
            .starts_with("cp:v1:")
    );
    assert!(execution.governance.dimensions_are_unrestricted());

    let rendered = format!("{execution:?}");
    let event_id = base.audit_event.id.to_string();
    let tenant_id = base.tenant.tenant_id.to_string();
    for sensitive in [
        admin.name.as_str(),
        event_id.as_str(),
        tenant_id.as_str(),
        "mutation-1",
    ] {
        assert!(!rendered.contains(sensitive));
    }
}

#[test]
fn admin_mutation_execution_reauthorizes_only_legacy_rotate_alias() {
    let admin = admin_auth_named("admin", None);
    let path = "/v1/prodex/gateway/keys/example-key";
    let captured = RuntimeProxyRequest {
        method: "PATCH".to_string(),
        path_and_query: path.to_string(),
        headers: vec![
            ("Idempotency-Key".to_string(), "rotate-1".to_string()),
            ("If-Match".to_string(), "W/\"42\"".to_string()),
        ],
        body: br#"{"rotate_secret":true}"#.to_vec(),
    };
    let http = runtime_gateway_http_request_meta(&captured, path);
    let action = runtime_gateway_admin_control_plane_action(&http, &admin).unwrap();
    let base = match plan_application_control_plane(action).decision {
        ControlPlaneDecision::Authorized(base) => base,
        ControlPlaneDecision::Denied { .. } => panic!("admin mutation should be authorized"),
    };

    let execution = match runtime_gateway_admin_mutation_execution(
        &captured,
        path,
        &admin,
        &base,
        ControlPlaneOperation::VirtualKeyRotateSecret,
    ) {
        Ok(execution) => execution,
        Err(_) => panic!("rotate alias should reauthorize"),
    };

    assert_eq!(base.operation, ControlPlaneOperation::VirtualKeyUpdate);
    assert_eq!(
        execution.authorized_action.operation,
        ControlPlaneOperation::VirtualKeyRotateSecret
    );
    assert_eq!(
        execution.authorized_action.audit_event.action.as_str(),
        "control_plane.virtual_key.rotate_secret"
    );
    assert_eq!(
        execution
            .authorized_action
            .audit_event
            .resource
            .id
            .as_deref(),
        Some("example-key")
    );
    assert_eq!(
        execution.atomic_write.audit_event,
        execution.authorized_action.audit_event
    );
    assert_eq!(
        execution.entity_tag.as_ref().map(|tag| tag.as_str()),
        Some("W/\"42\"")
    );

    let rejected = runtime_gateway_admin_mutation_execution(
        &captured,
        path,
        &admin,
        &base,
        ControlPlaneOperation::VirtualKeyDelete,
    )
    .unwrap_err();
    assert_eq!(rejected.status_code().0, 400);
}

#[test]
fn admin_mutation_execution_rejects_missing_idempotency_key_without_leaks() {
    let admin = admin_auth_named("redacted-admin", None);
    let path = "/v1/prodex/gateway/keys";
    let captured = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: path.to_string(),
        headers: Vec::new(),
        body: br#"{"name":"sensitive-key"}"#.to_vec(),
    };
    let http = runtime_gateway_http_request_meta(&captured, path);
    let action = runtime_gateway_admin_control_plane_action(&http, &admin).unwrap();
    let base = match plan_application_control_plane(action).decision {
        ControlPlaneDecision::Authorized(base) => base,
        ControlPlaneDecision::Denied { .. } => panic!("admin mutation should be authorized"),
    };

    let response = runtime_gateway_admin_mutation_execution(
        &captured,
        path,
        &admin,
        &base,
        ControlPlaneOperation::VirtualKeyCreate,
    )
    .unwrap_err();
    assert_eq!(response.status_code().0, 400);
    let mut body = String::new();
    response.into_reader().read_to_string(&mut body).unwrap();
    assert!(body.contains("control_plane_idempotency_key_required"));
    assert!(!body.contains("redacted-admin"));
    assert!(!body.contains("sensitive-key"));
}

#[test]
fn admin_router_has_no_process_local_idempotency_preclaim() {
    let source = include_str!("local_rewrite_gateway_admin_router.rs");
    assert!(!source.contains("gateway_admin_idempotency_keys"));
    assert!(!source.contains("keys.insert"));
    assert!(!source.contains("runtime_gateway_admin_if_match_response"));
}
