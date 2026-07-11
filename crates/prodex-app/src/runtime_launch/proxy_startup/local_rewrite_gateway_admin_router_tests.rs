use super::super::local_rewrite_application_boundary::{
    runtime_gateway_admin_control_plane_action, runtime_gateway_admin_control_plane_tenant_id,
    runtime_gateway_application_control_plane_authorization,
    runtime_gateway_application_request_context,
};
use super::super::local_rewrite_gateway_admin_auth::{
    RuntimeGatewayAdminAuthentication, RuntimeGatewayAdminCredentialEvidence,
};
use super::{
    ControlPlaneDecision, GatewayHttpMethod, GatewayHttpRequestMeta, RuntimeGatewayAdminAuth,
    RuntimeGatewayAdminRole, RuntimeProxyRequest, plan_application_control_plane,
    runtime_gateway_admin_idempotency_replay_decision, runtime_gateway_admin_route_explain_plan,
    runtime_gateway_http_headers, runtime_gateway_request_body_sha256,
};
use prodex_application::{
    ApplicationRequestContext, ApplicationRequestDeadline,
    plan_application_control_plane_audit_from_http,
    plan_application_control_plane_idempotency_from_http_digest,
    plan_application_control_plane_idempotency_replay,
};
use prodex_domain::{IdempotencyEntry, IdempotencyReplayDecision, RequestId, TenantId};
use prodex_gateway_http::{CanonicalRequestTarget, GatewayHttpHeader};
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
    assert!(
        runtime_gateway_application_control_plane_authorization(&request, &http, &admin).is_ok()
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
fn admin_idempotency_replay_adapter_matches_application_pending_semantics() {
    let admin = admin_auth_named("admin", None);
    let http = GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/v1/prodex/gateway/keys".to_string(),
        body_len: 2,
        headers: vec![GatewayHttpHeader::new("Idempotency-Key", "mutation-1")],
    };
    let action = runtime_gateway_admin_control_plane_action(&http, &admin).unwrap();
    let operation = plan_application_control_plane_idempotency_from_http_digest(
        action,
        &http,
        runtime_gateway_request_body_sha256(b"{}"),
    )
    .unwrap()
    .operation
    .unwrap();

    assert_eq!(
        runtime_gateway_admin_idempotency_replay_decision(&operation, None).unwrap(),
        plan_application_control_plane_idempotency_replay::<()>(&operation, None).unwrap()
    );
    let existing = IdempotencyEntry::<()>::pending(operation.clone(), 0);
    assert_eq!(
        runtime_gateway_admin_idempotency_replay_decision(
            &operation,
            Some(operation.request_fingerprint.as_str()),
        )
        .unwrap(),
        plan_application_control_plane_idempotency_replay(&operation, Some(&existing)).unwrap()
    );
    assert!(matches!(
        runtime_gateway_admin_idempotency_replay_decision(
            &operation,
            Some(operation.request_fingerprint.as_str()),
        )
        .unwrap(),
        IdempotencyReplayDecision::AlreadyInProgress {
            started_at_unix_ms: 0
        }
    ));

    let mut conflicting_operation = operation.clone();
    conflicting_operation.request_fingerprint = "sha256:different-mutation".to_string();
    let conflicting = IdempotencyEntry::<()>::pending(conflicting_operation, 0);
    assert_eq!(
        runtime_gateway_admin_idempotency_replay_decision(
            &operation,
            Some("sha256:different-mutation"),
        ),
        plan_application_control_plane_idempotency_replay(&operation, Some(&conflicting))
    );
}
