use std::{fmt, path::Path};

use prodex::{OtlpLogAttribute, otlp_http_log_export_status};
use prodex_application::{
    ApplicationControlPlaneAuditCorrelationErrorStatus,
    ApplicationControlPlaneAuditEmissionSpanErrorStatus, ApplicationControlPlaneAuditErrorStatus,
    ApplicationControlPlaneAuditPersistenceSpanErrorStatus,
    ApplicationControlPlaneAuditStoragePlan, ApplicationControlPlaneHttpRouteErrorStatus,
    plan_application_control_plane_audit_correlation_error_response,
    plan_application_control_plane_audit_correlation_from_http,
    plan_application_control_plane_audit_emission_span,
    plan_application_control_plane_audit_emission_span_error_response,
    plan_application_control_plane_audit_error_response,
    plan_application_control_plane_audit_persistence_span,
    plan_application_control_plane_audit_persistence_span_error_response,
    plan_application_control_plane_http_route,
    plan_application_control_plane_http_route_error_response,
    plan_application_control_plane_idempotency_error_response,
    plan_application_control_plane_idempotency_from_http_digest,
    plan_application_control_plane_page_request_error_response,
    plan_application_control_plane_page_request_from_http_query,
    plan_application_control_plane_precondition_error_response,
    plan_application_control_plane_precondition_from_http,
    plan_application_control_plane_with_audit_storage_from_http,
};
use prodex_control_plane::{
    ControlPlaneActionRequest, ControlPlaneOperation, ControlPlaneResourceRef,
};
use prodex_domain::{AuditDigest, CallId, Principal, RequestId, ResourceKind, TenantId};
use prodex_gateway_http::{
    GatewayHttpHeader, GatewayHttpMethod, GatewayHttpPolicy, GatewayHttpRequestMeta,
};
use prodex_storage::DurableStoreKind;
use serde::Deserialize;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::{
    CONTROL_PLANE_HTTP_ROUTE_PLAN_EVENT_NAME, CONTROL_PLANE_SCOPE_NAME, CONTROL_PLANE_SERVICE_NAME,
};

#[derive(Debug, Deserialize)]
struct ControlPlaneHttpPlanFile {
    principal: Principal,
    tenant_id: TenantId,
    resource_id: Option<String>,
    occurred_at_unix_ms: u64,
    request_id: Option<RequestId>,
    call_id: Option<CallId>,
    durable_store: Option<String>,
    previous_digest: Option<AuditDigest>,
    event_digest: Option<AuditDigest>,
    method: String,
    path: String,
    body_len: usize,
    headers: Vec<ControlPlaneHttpHeaderFile>,
    body_digest: Option<String>,
}

#[derive(Deserialize)]
struct ControlPlaneHttpHeaderFile {
    name: String,
    value: String,
}

impl fmt::Debug for ControlPlaneHttpHeaderFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlPlaneHttpHeaderFile")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .finish()
    }
}

impl Zeroize for ControlPlaneHttpHeaderFile {
    fn zeroize(&mut self) {
        self.value.zeroize();
    }
}

impl Drop for ControlPlaneHttpHeaderFile {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for ControlPlaneHttpHeaderFile {}

pub(super) fn run(request_path: &Path) -> Result<String, String> {
    let request = load_control_plane_http_plan_request(request_path)?;
    let (path, query) = split_path_and_query(&request.path);
    let headers = control_plane_plan_headers(request.headers)?;
    let http = GatewayHttpRequestMeta {
        method: parse_gateway_http_method(&request.method)?,
        path,
        body_len: request.body_len,
        headers,
    };
    let route = match plan_application_control_plane_http_route(&http) {
        Ok(route) => route,
        Err(error) => {
            let response = plan_application_control_plane_http_route_error_response(&error);
            return encode_control_plane_http_plan_route_failure(
                gateway_control_plane_route_status_label(response.status),
                response.code,
                response.message,
                &http,
            );
        }
    };
    let action = ControlPlaneActionRequest {
        principal: request.principal,
        operation: route.operation,
        resource: ControlPlaneResourceRef::new(
            request.tenant_id,
            route.operation.requirement().resource,
            request.resource_id,
        ),
        occurred_at_unix_ms: request.occurred_at_unix_ms,
    };
    let audit_plan = if route.http.requires_audit {
        Some(
            match plan_application_control_plane_with_audit_storage_from_http(
                prodex_application::ApplicationControlPlaneAuditRequest {
                    durable_store: parse_durable_store_kind(request.durable_store.as_deref())?,
                    action: action.clone(),
                    previous_digest: request.previous_digest.clone(),
                    event_digest: request
                        .event_digest
                        .clone()
                        .unwrap_or_else(default_control_plane_event_digest),
                },
                &http,
            ) {
                Ok(plan) => plan,
                Err(error) => {
                    let response = plan_application_control_plane_audit_error_response(&error);
                    return encode_control_plane_http_plan_operation_failure(
                        route.operation,
                        application_control_plane_audit_status_label(response.status),
                        response.code,
                        response.message,
                        "control-plane audit error",
                        &http,
                    );
                }
            },
        )
    } else {
        None
    };
    let audit = if let Some(plan) = audit_plan.as_ref() {
        let (audit_outcome, audit_action, audit_partition_tenant_id) = match &plan.decision {
            prodex_control_plane::ControlPlaneDecision::Authorized(action) => (
                action.audit_event.outcome,
                action.audit_event.action.as_str(),
                action.audit_write.tenant_partition_key,
            ),
            prodex_control_plane::ControlPlaneDecision::Denied {
                audit_event,
                audit_write,
                ..
            } => (
                audit_event.outcome,
                audit_event.action.as_str(),
                audit_write.tenant_partition_key,
            ),
        };
        serde_json::json!({
            "required": true,
            "storage_backend": application_control_plane_audit_storage_backend_label(&plan.audit_storage),
            "audit_action": audit_action,
            "audit_outcome": audit_outcome,
            "audit_partition_tenant_id": audit_partition_tenant_id,
        })
    } else {
        serde_json::json!({ "required": false })
    };
    let audit_correlation = if let Some(audit_plan) = audit_plan.clone() {
        let request_id = request.request_id.unwrap_or_default();
        let call_id = request.call_id;
        let correlation_plan = match plan_application_control_plane_audit_correlation_from_http(
            prodex_application::ApplicationControlPlaneAuditCorrelationRequest {
                request_id,
                call_id,
                http_policy: GatewayHttpPolicy::production_default(),
                http: http.clone(),
                audit: audit_plan.clone(),
            },
        ) {
            Ok(plan) => plan,
            Err(error) => {
                let response =
                    plan_application_control_plane_audit_correlation_error_response(&error);
                return encode_control_plane_http_plan_operation_failure(
                    route.operation,
                    application_control_plane_audit_correlation_status_label(response.status),
                    response.code,
                    response.message,
                    "control-plane audit correlation error",
                    &http,
                );
            }
        };
        let emission = match plan_application_control_plane_audit_emission_span(
            prodex_application::ApplicationControlPlaneAuditEmissionSpanRequest {
                correlation: correlation_plan.correlation.clone(),
            },
        ) {
            Ok(plan) => serde_json::json!({
                "name": plan.span.descriptor.name,
                "kind": format!("{:?}", plan.span.descriptor.kind).to_ascii_lowercase(),
            }),
            Err(error) => {
                let response =
                    plan_application_control_plane_audit_emission_span_error_response(&error);
                return encode_control_plane_http_plan_operation_failure(
                    route.operation,
                    application_control_plane_audit_emission_span_status_label(response.status),
                    response.code,
                    response.message,
                    "control-plane audit emission span error",
                    &http,
                );
            }
        };
        let persistence = match plan_application_control_plane_audit_persistence_span(
            prodex_application::ApplicationControlPlaneAuditPersistenceSpanRequest {
                correlation: correlation_plan.correlation.clone(),
                audit_storage: audit_plan.audit_storage,
            },
        ) {
            Ok(plan) => serde_json::json!({
                "name": plan.span.descriptor.name,
                "kind": format!("{:?}", plan.span.descriptor.kind).to_ascii_lowercase(),
            }),
            Err(error) => {
                let response =
                    plan_application_control_plane_audit_persistence_span_error_response(&error);
                return encode_control_plane_http_plan_operation_failure(
                    route.operation,
                    application_control_plane_audit_persistence_span_status_label(response.status),
                    response.code,
                    response.message,
                    "control-plane audit persistence span error",
                    &http,
                );
            }
        };
        serde_json::json!({
            "request_id": correlation_plan.correlation.request_id,
            "call_id": correlation_plan.correlation.call_id,
            "trace_id": correlation_plan.correlation.trace_id.as_ref().map(|trace_id| trace_id.as_str()),
            "tenant_id": correlation_plan.correlation.tenant_id,
            "audit_event_id": correlation_plan.correlation.audit_event_id,
            "emission_span": emission,
            "persistence_span": persistence,
        })
    } else {
        serde_json::Value::Null
    };
    let idempotency = if route.http.requires_idempotency {
        let body_digest = request.body_digest.ok_or_else(|| {
            "plan-http-control-plane requires body_digest for mutating control-plane routes"
                .to_string()
        })?;
        match plan_application_control_plane_idempotency_from_http_digest(
            action.clone(),
            &http,
            body_digest,
        ) {
            Ok(plan) => serde_json::json!({
                "required": true,
                "tenant_id": plan.operation.as_ref().map(|operation| operation.tenant_id),
                "key": plan.operation.as_ref().map(|operation| operation.key.as_str()),
                "request_fingerprint": plan.operation.as_ref().map(|operation| operation.request_fingerprint.as_str()),
            }),
            Err(error) => {
                let response = plan_application_control_plane_idempotency_error_response(&error);
                return encode_control_plane_http_plan_operation_failure(
                    route.operation,
                    application_control_plane_idempotency_status_label(response.status),
                    response.code,
                    response.message,
                    "control-plane idempotency error",
                    &http,
                );
            }
        }
    } else {
        serde_json::json!({
            "required": false,
        })
    };
    let precondition =
        match plan_application_control_plane_precondition_from_http(action.clone(), &http) {
            Ok(plan) => serde_json::json!({
                "present": plan.entity_tag.is_some(),
                "entity_tag": plan.entity_tag.as_ref().map(|tag| tag.as_str()),
            }),
            Err(error) => {
                let response = plan_application_control_plane_precondition_error_response(&error);
                return encode_control_plane_http_plan_operation_failure(
                    route.operation,
                    application_control_plane_precondition_status_label(response.status),
                    response.code,
                    response.message,
                    "control-plane precondition error",
                    &http,
                );
            }
        };
    let page_request = match plan_application_control_plane_page_request_from_http_query(
        action.clone(),
        &http,
        query,
    ) {
        Ok(plan) => serde_json::json!({
            "limit": plan.page_request.limit,
            "cursor": plan.page_request.cursor.as_ref().map(|cursor| cursor.as_str()),
        }),
        Err(error) => {
            let response = plan_application_control_plane_page_request_error_response(&error);
            return encode_control_plane_http_plan_operation_failure(
                route.operation,
                application_control_plane_page_request_status_label(response.status),
                response.code,
                response.message,
                "control-plane page request error",
                &http,
            );
        }
    };
    let mut otlp_attributes = vec![
        OtlpLogAttribute::bool("planned", true),
        OtlpLogAttribute::string("operation", control_plane_operation_label(route.operation)),
        OtlpLogAttribute::bool("requires_idempotency", route.http.requires_idempotency),
        OtlpLogAttribute::bool("requires_audit", route.http.requires_audit),
    ];
    let has_audit_trace_id = audit_correlation
        .get("trace_id")
        .and_then(|value| value.as_str())
        .is_some();
    append_control_plane_http_correlation_otlp_attributes(&mut otlp_attributes, &audit_correlation);
    append_control_plane_http_traceparent_otlp_attributes(
        &mut otlp_attributes,
        &http,
        !has_audit_trace_id,
    );
    let otlp_log_export = otlp_http_log_export_status(
        CONTROL_PLANE_SERVICE_NAME,
        CONTROL_PLANE_SCOPE_NAME,
        CONTROL_PLANE_HTTP_ROUTE_PLAN_EVENT_NAME,
        otlp_attributes,
    );
    serde_json::to_string_pretty(&serde_json::json!({
        "planned": true,
        "operation": control_plane_operation_label(route.operation),
        "resource_kind": resource_kind_label(route.operation.requirement().resource),
        "requires_idempotency": route.http.requires_idempotency,
        "requires_audit": route.http.requires_audit,
        "audit": audit,
        "audit_correlation": audit_correlation,
        "idempotency": idempotency,
        "precondition": precondition,
        "page_request": page_request,
        "otlp_log_export": otlp_log_export,
    }))
    .map_err(|err| format!("failed to encode control-plane HTTP plan: {err}"))
}

fn load_control_plane_http_plan_request(path: &Path) -> Result<ControlPlaneHttpPlanFile, String> {
    let bytes = std::fs::read(path).map_err(|err| {
        format!(
            "failed to read control-plane HTTP request {}: {err}",
            path.display()
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse control-plane HTTP request {}: {err}",
            path.display()
        )
    })
}

fn encode_control_plane_http_plan_operation_failure(
    operation: ControlPlaneOperation,
    status: &str,
    code: &str,
    message: &str,
    error_context: &str,
    http: &GatewayHttpRequestMeta,
) -> Result<String, String> {
    let mut otlp_attributes = vec![
        OtlpLogAttribute::bool("planned", false),
        OtlpLogAttribute::string("operation", control_plane_operation_label(operation)),
        OtlpLogAttribute::string("status", status),
        OtlpLogAttribute::string("code", code),
    ];
    append_control_plane_http_traceparent_otlp_attributes(&mut otlp_attributes, http, true);
    let otlp_log_export = otlp_http_log_export_status(
        CONTROL_PLANE_SERVICE_NAME,
        CONTROL_PLANE_SCOPE_NAME,
        CONTROL_PLANE_HTTP_ROUTE_PLAN_EVENT_NAME,
        otlp_attributes,
    );
    serde_json::to_string_pretty(&serde_json::json!({
        "planned": false,
        "operation": control_plane_operation_label(operation),
        "status": status,
        "code": code,
        "message": message,
        "otlp_log_export": otlp_log_export,
    }))
    .map_err(|err| format!("failed to encode {error_context}: {err}"))
}

fn encode_control_plane_http_plan_route_failure(
    status: &str,
    code: &str,
    message: &str,
    http: &GatewayHttpRequestMeta,
) -> Result<String, String> {
    let mut otlp_attributes = vec![
        OtlpLogAttribute::bool("planned", false),
        OtlpLogAttribute::string("status", status),
        OtlpLogAttribute::string("code", code),
    ];
    append_control_plane_http_traceparent_otlp_attributes(&mut otlp_attributes, http, true);
    let otlp_log_export = otlp_http_log_export_status(
        CONTROL_PLANE_SERVICE_NAME,
        CONTROL_PLANE_SCOPE_NAME,
        CONTROL_PLANE_HTTP_ROUTE_PLAN_EVENT_NAME,
        otlp_attributes,
    );
    serde_json::to_string_pretty(&serde_json::json!({
        "planned": false,
        "status": status,
        "code": code,
        "message": message,
        "otlp_log_export": otlp_log_export,
    }))
    .map_err(|err| format!("failed to encode control-plane route error: {err}"))
}

fn append_control_plane_http_correlation_otlp_attributes(
    attributes: &mut Vec<OtlpLogAttribute>,
    audit_correlation: &serde_json::Value,
) {
    for key in [
        "request_id",
        "call_id",
        "trace_id",
        "tenant_id",
        "audit_event_id",
    ] {
        if let Some(value) = audit_correlation.get(key).and_then(|value| value.as_str()) {
            attributes.push(OtlpLogAttribute::string(key, value));
        }
    }
}

fn append_control_plane_http_traceparent_otlp_attributes(
    attributes: &mut Vec<OtlpLogAttribute>,
    http: &GatewayHttpRequestMeta,
    include_trace_id: bool,
) {
    if include_trace_id && let Some(trace_id) = control_plane_http_traceparent_trace_id(http) {
        attributes.push(OtlpLogAttribute::string("trace_id", trace_id));
    }
    if let Some(span_id) = control_plane_http_traceparent_span_id(http) {
        attributes.push(OtlpLogAttribute::string("span_id", span_id));
    }
    if let Some(trace_flags) = control_plane_http_traceparent_flags(http) {
        attributes.push(OtlpLogAttribute::string("trace_flags", trace_flags));
    }
}

fn control_plane_http_traceparent_trace_id(http: &GatewayHttpRequestMeta) -> Option<String> {
    let trace_id = http
        .headers
        .iter()
        .find(|header| header.normalized_name() == "traceparent")?
        .value
        .split('-')
        .nth(1)?;
    is_valid_w3c_trace_id(trace_id).then(|| trace_id.to_string())
}

fn is_valid_w3c_trace_id(value: &str) -> bool {
    value.len() == 32
        && value.bytes().any(|byte| byte != b'0')
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn control_plane_http_traceparent_span_id(http: &GatewayHttpRequestMeta) -> Option<String> {
    let span_id = http
        .headers
        .iter()
        .find(|header| header.normalized_name() == "traceparent")?
        .value
        .split('-')
        .nth(2)?;
    is_valid_w3c_span_id(span_id).then(|| span_id.to_string())
}

fn is_valid_w3c_span_id(value: &str) -> bool {
    value.len() == 16
        && value.bytes().any(|byte| byte != b'0')
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn control_plane_http_traceparent_flags(http: &GatewayHttpRequestMeta) -> Option<String> {
    let flags = http
        .headers
        .iter()
        .find(|header| header.normalized_name() == "traceparent")?
        .value
        .split('-')
        .nth(3)?;
    is_valid_w3c_trace_flags(flags).then(|| flags.to_string())
}

fn is_valid_w3c_trace_flags(value: &str) -> bool {
    value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn gateway_control_plane_route_status_label(
    status: ApplicationControlPlaneHttpRouteErrorStatus,
) -> &'static str {
    match status {
        ApplicationControlPlaneHttpRouteErrorStatus::BadRequest => "bad_request",
        ApplicationControlPlaneHttpRouteErrorStatus::MethodNotAllowed => "method_not_allowed",
    }
}

fn application_control_plane_idempotency_status_label(
    status: prodex_application::ApplicationControlPlaneIdempotencyErrorStatus,
) -> &'static str {
    match status {
        prodex_application::ApplicationControlPlaneIdempotencyErrorStatus::BadRequest => {
            "bad_request"
        }
        prodex_application::ApplicationControlPlaneIdempotencyErrorStatus::Conflict => "conflict",
        prodex_application::ApplicationControlPlaneIdempotencyErrorStatus::MethodNotAllowed => {
            "method_not_allowed"
        }
        prodex_application::ApplicationControlPlaneIdempotencyErrorStatus::ServiceUnavailable => {
            "service_unavailable"
        }
    }
}

fn application_control_plane_precondition_status_label(
    status: prodex_application::ApplicationControlPlanePreconditionErrorStatus,
) -> &'static str {
    match status {
        prodex_application::ApplicationControlPlanePreconditionErrorStatus::BadRequest => {
            "bad_request"
        }
        prodex_application::ApplicationControlPlanePreconditionErrorStatus::MethodNotAllowed => {
            "method_not_allowed"
        }
    }
}

fn application_control_plane_page_request_status_label(
    status: prodex_application::ApplicationControlPlanePageRequestErrorStatus,
) -> &'static str {
    match status {
        prodex_application::ApplicationControlPlanePageRequestErrorStatus::BadRequest => {
            "bad_request"
        }
        prodex_application::ApplicationControlPlanePageRequestErrorStatus::MethodNotAllowed => {
            "method_not_allowed"
        }
    }
}

fn application_control_plane_audit_status_label(
    status: ApplicationControlPlaneAuditErrorStatus,
) -> &'static str {
    match status {
        ApplicationControlPlaneAuditErrorStatus::BadRequest => "bad_request",
        ApplicationControlPlaneAuditErrorStatus::MethodNotAllowed => "method_not_allowed",
        ApplicationControlPlaneAuditErrorStatus::ServiceUnavailable => "service_unavailable",
    }
}

fn application_control_plane_audit_storage_backend_label(
    audit_storage: &ApplicationControlPlaneAuditStoragePlan,
) -> &'static str {
    match audit_storage {
        ApplicationControlPlaneAuditStoragePlan::Postgres(_) => "postgres",
        ApplicationControlPlaneAuditStoragePlan::Sqlite(_) => "sqlite",
    }
}

fn application_control_plane_audit_correlation_status_label(
    status: ApplicationControlPlaneAuditCorrelationErrorStatus,
) -> &'static str {
    match status {
        ApplicationControlPlaneAuditCorrelationErrorStatus::BadRequest => "bad_request",
        ApplicationControlPlaneAuditCorrelationErrorStatus::MethodNotAllowed => {
            "method_not_allowed"
        }
        ApplicationControlPlaneAuditCorrelationErrorStatus::PayloadTooLarge => "payload_too_large",
        ApplicationControlPlaneAuditCorrelationErrorStatus::RequestHeaderFieldsTooLarge => {
            "request_header_fields_too_large"
        }
        ApplicationControlPlaneAuditCorrelationErrorStatus::ServiceUnavailable => {
            "service_unavailable"
        }
    }
}

fn application_control_plane_audit_emission_span_status_label(
    status: ApplicationControlPlaneAuditEmissionSpanErrorStatus,
) -> &'static str {
    match status {
        ApplicationControlPlaneAuditEmissionSpanErrorStatus::ServiceUnavailable => {
            "service_unavailable"
        }
    }
}

fn application_control_plane_audit_persistence_span_status_label(
    status: ApplicationControlPlaneAuditPersistenceSpanErrorStatus,
) -> &'static str {
    match status {
        ApplicationControlPlaneAuditPersistenceSpanErrorStatus::ServiceUnavailable => {
            "service_unavailable"
        }
    }
}

fn parse_durable_store_kind(value: Option<&str>) -> Result<DurableStoreKind, String> {
    match value.unwrap_or("postgres") {
        "postgres" => Ok(DurableStoreKind::Postgres),
        "sqlite" => Ok(DurableStoreKind::Sqlite),
        other => Err(format!("unsupported durable store: {other}")),
    }
}

fn default_control_plane_event_digest() -> AuditDigest {
    AuditDigest::new("sha256:control-plane-http-plan")
        .expect("static control-plane event digest should be valid")
}

fn split_path_and_query(path_and_query: &str) -> (String, &str) {
    match path_and_query.split_once('?') {
        Some((path, query)) => (path.to_string(), query),
        None => (path_and_query.to_string(), ""),
    }
}

fn parse_gateway_http_method(value: &str) -> Result<GatewayHttpMethod, String> {
    match value {
        "GET" | "get" => Ok(GatewayHttpMethod::Get),
        "POST" | "post" => Ok(GatewayHttpMethod::Post),
        "PATCH" | "patch" => Ok(GatewayHttpMethod::Patch),
        "DELETE" | "delete" => Ok(GatewayHttpMethod::Delete),
        "OPTIONS" | "options" => Ok(GatewayHttpMethod::Options),
        "OTHER" | "other" => Ok(GatewayHttpMethod::Other),
        _ => Err(format!("unsupported HTTP method: {value}")),
    }
}

fn control_plane_plan_headers(
    headers: Vec<ControlPlaneHttpHeaderFile>,
) -> Result<Vec<GatewayHttpHeader>, String> {
    if headers
        .iter()
        .any(|header| control_plane_plan_credential_header(&header.name))
    {
        return Err("control-plane plan input must not include credential headers".to_string());
    }
    Ok(headers
        .into_iter()
        .map(|mut header| {
            GatewayHttpHeader::new(
                std::mem::take(&mut header.name),
                std::mem::take(&mut header.value),
            )
        })
        .collect())
}

fn control_plane_plan_credential_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "proxy-authorization" | "x-api-key" | "api-key"
    )
}

fn control_plane_operation_label(operation: ControlPlaneOperation) -> &'static str {
    match operation {
        ControlPlaneOperation::GatewayAdminRead => "gateway_admin_read",
        ControlPlaneOperation::RouteExplain => "route_explain",
        ControlPlaneOperation::TenantCreate => "tenant_create",
        ControlPlaneOperation::TenantUpdate => "tenant_update",
        ControlPlaneOperation::UserInvite => "user_invite",
        ControlPlaneOperation::ScimUserRead => "scim_user_read",
        ControlPlaneOperation::ScimUserCreate => "scim_user_create",
        ControlPlaneOperation::ScimUserUpdate => "scim_user_update",
        ControlPlaneOperation::ScimUserDelete => "scim_user_delete",
        ControlPlaneOperation::RoleBindingGrant => "role_binding_grant",
        ControlPlaneOperation::RoleBindingRevoke => "role_binding_revoke",
        ControlPlaneOperation::ServiceIdentityCreate => "service_identity_create",
        ControlPlaneOperation::VirtualKeyRead => "virtual_key_read",
        ControlPlaneOperation::VirtualKeyCreate => "virtual_key_create",
        ControlPlaneOperation::VirtualKeyUpdate => "virtual_key_update",
        ControlPlaneOperation::VirtualKeyDelete => "virtual_key_delete",
        ControlPlaneOperation::VirtualKeyRotateSecret => "virtual_key_rotate_secret",
        ControlPlaneOperation::PolicyPublish => "policy_publish",
        ControlPlaneOperation::ProviderCredentialRotate => "provider_credential_rotate",
        ControlPlaneOperation::BudgetUpdate => "budget_update",
        ControlPlaneOperation::BillingRead => "billing_read",
        ControlPlaneOperation::AuditExport => "audit_export",
        ControlPlaneOperation::AuditRetentionPurge => "audit_retention_purge",
        ControlPlaneOperation::ConfigurationPublish => "configuration_publish",
    }
}

fn resource_kind_label(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Tenant => "tenant",
        ResourceKind::User => "user",
        ResourceKind::RoleBinding => "role_binding",
        ResourceKind::ServiceIdentity => "service_identity",
        ResourceKind::VirtualKey => "virtual_key",
        ResourceKind::Policy => "policy",
        ResourceKind::ProviderCredential => "provider_credential",
        ResourceKind::Budget => "budget",
        ResourceKind::Billing => "billing",
        ResourceKind::AuditLog => "audit_log",
        ResourceKind::Configuration => "configuration",
    }
}

#[cfg(test)]
#[path = "http_plan/tests/security.rs"]
mod tests;
