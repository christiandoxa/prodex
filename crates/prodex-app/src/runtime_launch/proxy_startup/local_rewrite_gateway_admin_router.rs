use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_application_boundary::{
    RuntimeGatewayAdminPreauthorization, runtime_gateway_admin_control_plane_action,
};
use super::local_rewrite_gateway_admin_audit::{
    runtime_gateway_audit_admin_auth_event, runtime_gateway_audit_admin_request_denied_event,
    runtime_gateway_audit_admin_role_denied_event,
};
use super::local_rewrite_gateway_admin_auth::RuntimeGatewayAdminAuth;
use super::local_rewrite_gateway_admin_keys::{
    runtime_gateway_admin_create_key_response, runtime_gateway_admin_delete_key_response,
    runtime_gateway_admin_get_key_response, runtime_gateway_admin_key_etag,
    runtime_gateway_admin_update_key_response,
};
use super::local_rewrite_gateway_admin_ledger::{
    runtime_gateway_admin_ledger_csv_response, runtime_gateway_admin_ledger_response,
    runtime_gateway_admin_ledger_summary_csv_response,
    runtime_gateway_admin_ledger_summary_response,
};
use super::local_rewrite_gateway_admin_payloads::{
    runtime_gateway_admin_guardrails_payload, runtime_gateway_admin_observability_payload,
    runtime_gateway_admin_providers_payload, runtime_gateway_openapi_spec,
};
use super::local_rewrite_gateway_admin_response::runtime_gateway_admin_json_response;
use super::local_rewrite_gateway_admin_route_explain::runtime_gateway_admin_route_explain_response;
use super::local_rewrite_gateway_admin_scim::{
    runtime_gateway_admin_scim_create_user_response,
    runtime_gateway_admin_scim_delete_user_response, runtime_gateway_admin_scim_get_user_response,
    runtime_gateway_admin_scim_list_users_response,
    runtime_gateway_admin_scim_update_user_response,
};
use super::local_rewrite_gateway_dashboard::runtime_gateway_admin_dashboard_response;
use super::local_rewrite_gateway_key_payloads::runtime_gateway_admin_keys_payload;
use super::local_rewrite_gateway_metrics::runtime_gateway_prometheus_response;
use super::*;
use prodex_application::{
    ApplicationControlPlaneAuditErrorStatus, ApplicationControlPlaneHttpRouteErrorStatus,
    ApplicationControlPlaneIdempotencyError, ApplicationControlPlaneIdempotencyErrorStatus,
    ApplicationControlPlanePreconditionErrorStatus, plan_application_control_plane,
    plan_application_control_plane_audit_error_response,
    plan_application_control_plane_audit_from_http, plan_application_control_plane_http_route,
    plan_application_control_plane_http_route_error_response,
    plan_application_control_plane_idempotency_error_response,
    plan_application_control_plane_idempotency_from_http_digest,
    plan_application_control_plane_idempotency_replay,
    plan_application_control_plane_precondition_error_response,
    plan_application_control_plane_precondition_from_http,
};
use prodex_control_plane::ControlPlaneDecision;
use prodex_domain::{IdempotencyEntry, IdempotencyReplayDecision, IdempotentOperation};
use prodex_gateway_http::{GatewayHttpHeader, GatewayHttpMethod, GatewayHttpRequestMeta};
use sha2::{Digest, Sha256};

pub(super) fn runtime_gateway_request_path_is_admin(
    request_path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
) -> bool {
    let path = path_without_query(request_path);
    let admin_prefix = format!("{}/prodex/gateway", shared.mount_path.trim_end_matches('/'));
    path == format!("{admin_prefix}/admin")
        || path == format!("{admin_prefix}/openapi.json")
        || path == format!("{admin_prefix}/metrics")
        || path == format!("{admin_prefix}/providers")
        || path == format!("{admin_prefix}/observability")
        || path == format!("{admin_prefix}/guardrails")
        || path == format!("{admin_prefix}/routes/explain")
        || path == format!("{admin_prefix}/usage")
        || path == format!("{admin_prefix}/keys")
        || path.starts_with(&format!("{admin_prefix}/keys/"))
        || path == format!("{admin_prefix}/ledger")
        || path == format!("{admin_prefix}/ledger.csv")
        || path == format!("{admin_prefix}/ledger/summary")
        || path == format!("{admin_prefix}/ledger/summary.csv")
        || path == format!("{admin_prefix}/scim/v2/Users")
        || path.starts_with(&format!("{admin_prefix}/scim/v2/Users/"))
}

pub(super) fn runtime_gateway_request_path_is_route_explain(
    request_path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
) -> bool {
    let path = path_without_query(request_path);
    let admin_prefix = format!("{}/prodex/gateway", shared.mount_path.trim_end_matches('/'));
    path == format!("{admin_prefix}/routes/explain")
}

pub(super) fn runtime_gateway_request_path_requires_admin_auth(
    request_path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
) -> bool {
    runtime_gateway_request_path_is_admin(request_path, shared)
}

pub(super) fn runtime_gateway_admin_authorization_rejection_response(
    request_id: u64,
    method: &str,
    path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    runtime_gateway_audit_admin_role_denied_event(
        shared,
        &admin_auth.name,
        admin_auth.role.as_str(),
        method,
        path,
    );
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_admin_role_rejected",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("path", path),
                runtime_proxy_log_field("admin", admin_auth.name.as_str()),
                runtime_proxy_log_field("role", admin_auth.role.as_str()),
            ],
        ),
    );
    build_runtime_proxy_json_error_response(
        403,
        "gateway_admin_role_forbidden",
        "gateway admin role does not allow this mutation",
    )
}

pub(super) fn runtime_gateway_admin_response(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    _request_context: &prodex_application::ApplicationRequestContext<'_>,
    preauthorized: Option<RuntimeGatewayAdminPreauthorization<'_>>,
) -> Option<tiny_http::ResponseBox> {
    let path = path_without_query(&captured.path_and_query);
    let admin_prefix = format!("{}/prodex/gateway", shared.mount_path.trim_end_matches('/'));
    let keys_path = format!("{admin_prefix}/keys");
    let usage_path = format!("{admin_prefix}/usage");
    let ledger_path = format!("{admin_prefix}/ledger");
    let ledger_csv_path = format!("{ledger_path}.csv");
    let ledger_summary_path = format!("{ledger_path}/summary");
    let ledger_summary_csv_path = format!("{ledger_summary_path}.csv");
    let metrics_path = format!("{admin_prefix}/metrics");
    let providers_path = format!("{admin_prefix}/providers");
    let observability_path = format!("{admin_prefix}/observability");
    let guardrails_path = format!("{admin_prefix}/guardrails");
    let route_explain_path = format!("{admin_prefix}/routes/explain");
    let openapi_path = format!("{admin_prefix}/openapi.json");
    let admin_path = format!("{admin_prefix}/admin");
    let scim_users_path = format!("{admin_prefix}/scim/v2/Users");
    let key_name = path
        .strip_prefix(&(keys_path.clone() + "/"))
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let scim_user_id = path
        .strip_prefix(&(scim_users_path.clone() + "/"))
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if path != usage_path
        && path != ledger_path
        && path != ledger_csv_path
        && path != ledger_summary_path
        && path != ledger_summary_csv_path
        && path != metrics_path
        && path != providers_path
        && path != observability_path
        && path != guardrails_path
        && path != route_explain_path
        && path != admin_path
        && path != keys_path
        && path != openapi_path
        && path != scim_users_path
        && key_name.is_none()
        && scim_user_id.is_none()
    {
        return None;
    }
    let Some(preauthorized) = preauthorized else {
        if shared.gateway_admin_tokens.is_empty()
            && shared.gateway_sso.proxy_token_hash.is_none()
            && shared.gateway_sso.oidc.is_none()
        {
            runtime_gateway_audit_admin_auth_event(
                shared,
                "auth_failed",
                "failure",
                serde_json::json!({
                    "reason": "admin_auth_not_configured",
                    "method": captured.method,
                    "path": path,
                }),
            );
            return Some(build_runtime_proxy_json_error_response(
                403,
                "admin_auth_not_configured",
                "configure a gateway admin bearer token to use gateway admin endpoints",
            ));
        }
        runtime_gateway_audit_admin_auth_event(
            shared,
            "auth_failed",
            "failure",
            serde_json::json!({
                "reason": "admin_authentication_required",
                "method": captured.method,
                "path": path,
            }),
        );
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_admin_auth_rejected",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("path", path),
                ],
            ),
        );
        return Some(build_runtime_proxy_json_error_response(
            401,
            "invalid_admin_token",
            "missing or invalid gateway admin bearer token",
        ));
    };
    let admin_auth = &preauthorized.auth;
    let _application_tenant = preauthorized
        .application
        .as_ref()
        .and_then(|application| application.tenant_context());

    if path == route_explain_path && !captured.method.eq_ignore_ascii_case("POST") {
        runtime_gateway_audit_admin_request_denied_event(
            shared,
            &admin_auth.name,
            admin_auth.role.as_str(),
            "control_plane_method_not_allowed",
            &captured.method,
            path,
        );
    }
    if let Some(response) = runtime_gateway_admin_boundary_response(captured, path) {
        return Some(response);
    }

    if path == admin_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway admin dashboard endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_dashboard_response(shared));
    }
    let admin_method = captured.method.to_ascii_uppercase();
    let admin_http = runtime_gateway_http_request_meta(captured, path);
    if let Some(action) = runtime_gateway_admin_control_plane_action(&admin_http, admin_auth)
        .filter(|action| action.operation.requires_idempotency())
    {
        if let Some(response) = runtime_gateway_admin_audit_boundary_response(
            shared,
            admin_auth,
            &admin_method,
            path,
            action.clone(),
            &admin_http,
        ) {
            return Some(response);
        }
        if let Some(response) = runtime_gateway_admin_idempotency_response(
            captured,
            shared,
            admin_auth,
            &admin_method,
            path,
            action,
            &admin_http,
        ) {
            return Some(response);
        }
    }

    if path == openapi_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway OpenAPI endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_json_response(
            200,
            runtime_gateway_openapi_spec(shared),
        ));
    }

    if path == route_explain_path {
        return Some(runtime_gateway_admin_route_explain_response(
            captured, shared, admin_auth,
        ));
    }

    if path == usage_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway usage endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_json_response(
            200,
            runtime_gateway_admin_keys_payload(shared, "gateway.usage", Some(admin_auth)),
        ));
    }

    if path == ledger_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway ledger endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_ledger_response(
            &captured.path_and_query,
            shared,
            admin_auth,
        ));
    }

    if path == ledger_csv_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway ledger CSV endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_ledger_csv_response(
            shared, admin_auth,
        ));
    }

    if path == ledger_summary_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway ledger summary endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_ledger_summary_response(
            shared, admin_auth,
        ));
    }

    if path == ledger_summary_csv_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway ledger summary CSV endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_ledger_summary_csv_response(
            shared, admin_auth,
        ));
    }

    if path == metrics_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway metrics endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_prometheus_response(shared, admin_auth));
    }

    if path == providers_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway providers endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_json_response(
            200,
            runtime_gateway_admin_providers_payload(),
        ));
    }

    if path == observability_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway observability endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_json_response(
            200,
            runtime_gateway_admin_observability_payload(shared),
        ));
    }

    if path == guardrails_path {
        if !captured.method.eq_ignore_ascii_case("GET") {
            return Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway guardrails endpoint requires GET",
            ));
        }
        return Some(runtime_gateway_admin_json_response(
            200,
            runtime_gateway_admin_guardrails_payload(shared),
        ));
    }

    if path == keys_path {
        return match admin_method.as_str() {
            "GET" => Some(runtime_gateway_admin_json_response(
                200,
                runtime_gateway_admin_keys_payload(shared, "gateway.keys", Some(admin_auth)),
            )),
            "POST" => Some(runtime_gateway_admin_create_key_response(
                captured, shared, admin_auth,
            )),
            _ => Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway keys endpoint supports GET and POST",
            )),
        };
    }

    if path == scim_users_path {
        return match admin_method.as_str() {
            "GET" => Some(runtime_gateway_admin_scim_list_users_response(
                shared, admin_auth,
            )),
            "POST" => Some(runtime_gateway_admin_scim_create_user_response(
                captured, shared, admin_auth,
            )),
            _ => Some(build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway SCIM Users endpoint supports GET and POST",
            )),
        };
    }

    if let Some(scim_user_id) = scim_user_id {
        return Some(match admin_method.as_str() {
            "GET" => runtime_gateway_admin_scim_get_user_response(scim_user_id, shared, admin_auth),
            "PATCH" | "PUT" => runtime_gateway_admin_scim_update_user_response(
                scim_user_id,
                captured,
                shared,
                admin_auth,
            ),
            "DELETE" => {
                runtime_gateway_admin_scim_delete_user_response(scim_user_id, shared, admin_auth)
            }
            _ => build_runtime_proxy_json_error_response(
                405,
                "method_not_allowed",
                "gateway SCIM User endpoint supports GET, PATCH, PUT, and DELETE",
            ),
        });
    }

    if matches!(admin_method.as_str(), "PATCH" | "DELETE")
        && let Some(response) = runtime_gateway_admin_if_match_response(
            captured,
            shared,
            admin_auth,
            key_name,
            &admin_method,
            path,
        )
    {
        return Some(response);
    }

    let key_name = key_name.unwrap_or_default();
    Some(match admin_method.as_str() {
        "GET" => runtime_gateway_admin_get_key_response(key_name, shared, admin_auth),
        "PATCH" => {
            runtime_gateway_admin_update_key_response(key_name, captured, shared, admin_auth)
        }
        "DELETE" => runtime_gateway_admin_delete_key_response(key_name, shared, admin_auth),
        _ => build_runtime_proxy_json_error_response(
            405,
            "method_not_allowed",
            "gateway key endpoint supports GET, PATCH, and DELETE",
        ),
    })
}

fn runtime_gateway_admin_boundary_response(
    captured: &RuntimeProxyRequest,
    path: &str,
) -> Option<tiny_http::ResponseBox> {
    let http = GatewayHttpRequestMeta {
        method: gateway_http_method(&captured.method),
        path: path.to_string(),
        body_len: captured.body.len(),
        headers: captured
            .headers
            .iter()
            .map(|(name, value)| GatewayHttpHeader::new(name, value))
            .collect(),
    };
    let error = plan_application_control_plane_http_route(&http).err()?;
    let response = plan_application_control_plane_http_route_error_response(&error);
    let status = match response.status {
        ApplicationControlPlaneHttpRouteErrorStatus::BadRequest => 400,
        ApplicationControlPlaneHttpRouteErrorStatus::MethodNotAllowed => 405,
    };
    Some(build_runtime_proxy_json_error_response(
        status,
        response.code,
        response.message,
    ))
}

fn gateway_http_method(method: &str) -> GatewayHttpMethod {
    match method {
        value if value.eq_ignore_ascii_case("GET") => GatewayHttpMethod::Get,
        value if value.eq_ignore_ascii_case("POST") => GatewayHttpMethod::Post,
        value if value.eq_ignore_ascii_case("PATCH") => GatewayHttpMethod::Patch,
        value if value.eq_ignore_ascii_case("DELETE") => GatewayHttpMethod::Delete,
        value if value.eq_ignore_ascii_case("OPTIONS") => GatewayHttpMethod::Options,
        _ => GatewayHttpMethod::Other,
    }
}

fn runtime_gateway_admin_audit_boundary_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    method: &str,
    path: &str,
    action: prodex_control_plane::ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Option<tiny_http::ResponseBox> {
    let error = plan_application_control_plane_audit_from_http(action, http).err()?;
    let response = plan_application_control_plane_audit_error_response(&error);
    runtime_gateway_audit_admin_request_denied_event(
        shared,
        &admin_auth.name,
        admin_auth.role.as_str(),
        response.code,
        method,
        path,
    );
    Some(build_runtime_proxy_json_error_response(
        runtime_gateway_application_audit_status_code(response.status),
        response.code,
        response.message,
    ))
}

fn runtime_gateway_admin_idempotency_response(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    method: &str,
    path: &str,
    action: prodex_control_plane::ControlPlaneActionRequest,
    http: &GatewayHttpRequestMeta,
) -> Option<tiny_http::ResponseBox> {
    let operation = match plan_application_control_plane_idempotency_from_http_digest(
        action,
        http,
        runtime_gateway_request_body_sha256(&captured.body),
    ) {
        Ok(plan) => plan.operation?,
        Err(error) => {
            let response = plan_application_control_plane_idempotency_error_response(&error);
            runtime_gateway_audit_admin_request_denied_event(
                shared,
                &admin_auth.name,
                admin_auth.role.as_str(),
                response.code,
                method,
                path,
            );
            return Some(build_runtime_proxy_json_error_response(
                runtime_gateway_application_idempotency_status_code(response.status),
                response.code,
                response.message,
            ));
        }
    };
    let idempotency_key = operation.key.as_str();

    let cache_key_prefix = format!(
        "{} {method} {path} {idempotency_key}\x1e",
        runtime_gateway_admin_idempotency_scope_key(admin_auth)
    );
    let mut keys = shared
        .gateway_admin_idempotency_keys
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let existing_fingerprint = keys
        .range(cache_key_prefix.clone()..)
        .next()
        .and_then(|entry| entry.strip_prefix(&cache_key_prefix))
        .map(str::to_string);
    match runtime_gateway_admin_idempotency_replay_decision(
        &operation,
        existing_fingerprint.as_deref(),
    ) {
        Ok(IdempotencyReplayDecision::ExecuteAndRecordPending) => {
            keys.insert(format!(
                "{cache_key_prefix}{}",
                operation.request_fingerprint
            ));
            None
        }
        Ok(
            IdempotencyReplayDecision::AlreadyInProgress { .. }
            | IdempotencyReplayDecision::Replay(()),
        )
        | Err(_) => {
            runtime_gateway_audit_admin_request_denied_event(
                shared,
                &admin_auth.name,
                admin_auth.role.as_str(),
                "duplicate_idempotency_key",
                method,
                path,
            );
            Some(build_runtime_proxy_json_error_response(
                409,
                "duplicate_idempotency_key",
                "admin mutation with this Idempotency-Key was already accepted for this resource",
            ))
        }
    }
}

fn runtime_gateway_admin_idempotency_replay_decision(
    operation: &IdempotentOperation,
    existing_fingerprint: Option<&str>,
) -> Result<IdempotencyReplayDecision<()>, ApplicationControlPlaneIdempotencyError> {
    let existing = existing_fingerprint.map(|request_fingerprint| {
        let mut existing_operation = operation.clone();
        existing_operation.request_fingerprint = request_fingerprint.to_string();
        IdempotencyEntry::pending(existing_operation, 0)
    });
    plan_application_control_plane_idempotency_replay(operation, existing.as_ref())
}

fn runtime_gateway_admin_idempotency_scope_key(admin_auth: &RuntimeGatewayAdminAuth) -> String {
    [
        admin_auth.name.as_str(),
        admin_auth.role.as_str(),
        admin_auth.tenant_id.as_deref().unwrap_or("*"),
        admin_auth.team_id.as_deref().unwrap_or("*"),
        admin_auth.project_id.as_deref().unwrap_or("*"),
        admin_auth.user_id.as_deref().unwrap_or("*"),
        admin_auth.budget_id.as_deref().unwrap_or("*"),
        &admin_auth.allowed_key_prefixes.join(","),
    ]
    .join("\x1f")
}

fn runtime_gateway_admin_if_match_response(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
    key_name: Option<&str>,
    method: &str,
    path: &str,
) -> Option<tiny_http::ResponseBox> {
    let http = runtime_gateway_http_request_meta(captured, path);
    let action = runtime_gateway_admin_control_plane_action(&http, admin_auth)?;
    let expected_match = match plan_application_control_plane_precondition_from_http(action, &http)
    {
        Ok(plan) => plan.entity_tag?,
        Err(error) => {
            let response = plan_application_control_plane_precondition_error_response(&error);
            runtime_gateway_audit_admin_request_denied_event(
                shared,
                &admin_auth.name,
                admin_auth.role.as_str(),
                response.code,
                method,
                path,
            );
            return Some(build_runtime_proxy_json_error_response(
                runtime_gateway_application_precondition_status_code(response.status),
                response.code,
                response.message,
            ));
        }
    };
    if expected_match.as_str() == "*" {
        return None;
    }
    let key_name = key_name?;
    let entries = shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.clone())
        .unwrap_or_default();
    let entry = entries
        .iter()
        .find(|entry| entry.key.name.eq_ignore_ascii_case(key_name))?;
    let current =
        prodex_domain::EntityTag::new(runtime_gateway_admin_key_etag(entry.updated_at_epoch))
            .expect("generated gateway key etag should be valid");
    if expected_match != current {
        runtime_gateway_audit_admin_request_denied_event(
            shared,
            &admin_auth.name,
            admin_auth.role.as_str(),
            "precondition_failed",
            method,
            path,
        );
        return Some(build_runtime_proxy_json_error_response(
            412,
            "precondition_failed",
            "If-Match does not match the current gateway key ETag",
        ));
    }
    None
}

fn runtime_gateway_http_headers(captured: &RuntimeProxyRequest) -> Vec<GatewayHttpHeader> {
    captured
        .headers
        .iter()
        .map(|(name, value)| GatewayHttpHeader::new(name, value))
        .collect()
}

pub(super) fn runtime_gateway_http_request_meta(
    captured: &RuntimeProxyRequest,
    path: &str,
) -> GatewayHttpRequestMeta {
    GatewayHttpRequestMeta {
        method: gateway_http_method(&captured.method),
        path: path.to_string(),
        body_len: captured.body.len(),
        headers: runtime_gateway_http_headers(captured),
    }
}

pub(super) fn runtime_gateway_admin_route_explain_plan(
    http: &GatewayHttpRequestMeta,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> Option<prodex_control_plane::ControlPlaneActionPlan> {
    let action = runtime_gateway_admin_control_plane_action(http, admin_auth)?;
    if action.operation != prodex_control_plane::ControlPlaneOperation::RouteExplain {
        return None;
    }
    match plan_application_control_plane(action).decision {
        ControlPlaneDecision::Authorized(plan) => Some(plan),
        ControlPlaneDecision::Denied { .. } => None,
    }
}

fn runtime_gateway_request_body_sha256(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

fn runtime_gateway_application_idempotency_status_code(
    status: ApplicationControlPlaneIdempotencyErrorStatus,
) -> u16 {
    match status {
        ApplicationControlPlaneIdempotencyErrorStatus::BadRequest => 400,
        ApplicationControlPlaneIdempotencyErrorStatus::Conflict => 409,
        ApplicationControlPlaneIdempotencyErrorStatus::MethodNotAllowed => 405,
        ApplicationControlPlaneIdempotencyErrorStatus::ServiceUnavailable => 503,
    }
}

fn runtime_gateway_application_audit_status_code(
    status: ApplicationControlPlaneAuditErrorStatus,
) -> u16 {
    match status {
        ApplicationControlPlaneAuditErrorStatus::BadRequest => 400,
        ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => 405,
        ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => 503,
    }
}

fn runtime_gateway_application_precondition_status_code(
    status: ApplicationControlPlanePreconditionErrorStatus,
) -> u16 {
    match status {
        ApplicationControlPlanePreconditionErrorStatus::BadRequest => 400,
        ApplicationControlPlanePreconditionErrorStatus::MethodNotAllowed => 405,
    }
}

#[cfg(test)]
#[path = "local_rewrite_gateway_admin_router_tests.rs"]
mod tests;
