use super::*;

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
fn runtime_gateway_admin_write_authorization_uses_shared_control_plane_decision() {
    let http = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/v1/prodex/gateway/keys".to_string(),
        body_len: 0,
        headers: Vec::new(),
    };
    let viewer = RuntimeGatewayAdminAuth {
        name: "viewer".to_string(),
        role: RuntimeGatewayAdminRole::Viewer,
        tenant_id: None,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
    };
    let admin = RuntimeGatewayAdminAuth {
        name: "admin".to_string(),
        role: RuntimeGatewayAdminRole::Admin,
        tenant_id: None,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
    };

    assert!(!runtime_gateway_admin_write_authorized(&http, &viewer));
    assert!(runtime_gateway_admin_write_authorized(&http, &admin));

    let explain_http = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/v1/prodex/gateway/routes/explain".to_string(),
        body_len: 2,
        headers: Vec::new(),
    };
    assert!(runtime_gateway_admin_route_explain_plan(&explain_http, &viewer).is_some());
    assert!(runtime_gateway_admin_route_explain_plan(&explain_http, &admin).is_some());
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
fn admin_control_plane_action_does_not_synthesize_random_ids() {
    let source = include_str!("local_rewrite_gateway_admin_router.rs");
    for fallback in [
        ["let tenant_id = ", "TenantId::new();"].join(""),
        ["PrincipalId", "::new()"].join(""),
    ] {
        assert!(!source.contains(&fallback));
    }
}
