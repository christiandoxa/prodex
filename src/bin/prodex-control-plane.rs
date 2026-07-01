//! Prodex control-plane entrypoint.
//!
//! This binary stays a thin composition root. It now exposes a one-shot
//! configuration-publication delivery command while long-lived admin serving
//! remains gated until adapters are wired behind the enterprise boundaries.

use std::path::PathBuf;

use prodex::deliver_config_publication_event_to_gateway_runtime;
use prodex_application::{
    plan_application_control_plane_http_route,
    plan_application_control_plane_idempotency_error_response,
    plan_application_control_plane_idempotency_from_http_digest,
};
use prodex_config::{ConfigPublicationEventPlan, ConfigPublicationEventTarget};
use prodex_control_plane::{
    ConfigurationPublicationDecision, ConfigurationPublicationErrorStatus,
    ConfigurationPublicationRequest, ControlPlaneActionRequest, ControlPlaneOperation,
    ControlPlaneResourceRef, decide_configuration_publication,
    plan_configuration_publication_error_response,
};
use prodex_domain::{PolicyRevisionId, Principal, ResourceKind, TenantId};
use prodex_gateway_http::{GatewayHttpHeader, GatewayHttpMethod, GatewayHttpRequestMeta};
use serde::Deserialize;

const HELP: &str = "prodex-control-plane

Control-plane entrypoint.

USAGE:
    prodex-control-plane --help
    prodex-control-plane --version
    prodex-control-plane plan-config-publication --request <path>
    prodex-control-plane plan-http-control-plane --request <path>
    prodex-control-plane deliver-config-publication --event <path> --root <path>

STATUS:
    The enterprise control-plane binary is present as a dedicated composition
    root. Long-lived admin serving remains intentionally gated until adapters
    are wired to prodex-application and prodex-control-plane.
";

#[derive(Debug, Deserialize)]
struct ConfigPublicationEventFile {
    tenant_id: TenantId,
    activated_revision_id: PolicyRevisionId,
    previous_active_revision_id: Option<PolicyRevisionId>,
    last_known_good_revision_id: Option<PolicyRevisionId>,
    targets: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigPublicationPlanFile {
    principal: Principal,
    occurred_at_unix_ms: u64,
    current_revision_id: Option<PolicyRevisionId>,
    candidate: ConfigRevisionFile,
}

#[derive(Debug, Deserialize)]
struct ConfigRevisionFile {
    tenant_id: TenantId,
    revision_id: PolicyRevisionId,
    published_at_unix_ms: u64,
    payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ControlPlaneHttpPlanFile {
    principal: Principal,
    tenant_id: TenantId,
    resource_id: Option<String>,
    occurred_at_unix_ms: u64,
    method: String,
    path: String,
    body_len: usize,
    headers: Vec<ControlPlaneHttpHeaderFile>,
    body_digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ControlPlaneHttpHeaderFile {
    name: String,
    value: String,
}

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("--help") | Some("-h") => {
            print!("{HELP}");
        }
        Some("--version") | Some("-V") => {
            println!("prodex-control-plane {}", env!("CARGO_PKG_VERSION"));
        }
        Some("serve") => {
            eprintln!(
                "prodex-control-plane serve is not wired yet; use the legacy `prodex gateway` admin path until control-plane adapter migration is complete"
            );
            std::process::exit(2);
        }
        Some("plan-config-publication") => match run_plan_config_publication(args) {
            Ok(output) => println!("{output}"),
            Err(err) => {
                eprintln!(
                    "{err}

{HELP}"
                );
                std::process::exit(2);
            }
        },
        Some("plan-http-control-plane") => match run_plan_http_control_plane(args) {
            Ok(output) => println!("{output}"),
            Err(err) => {
                eprintln!(
                    "{err}

{HELP}"
                );
                std::process::exit(2);
            }
        },
        Some("deliver-config-publication") => match run_deliver_config_publication(args) {
            Ok(output) => println!("{output}"),
            Err(err) => {
                eprintln!(
                    "{err}

{HELP}"
                );
                std::process::exit(2);
            }
        },
        Some(other) => {
            eprintln!(
                "unknown prodex-control-plane argument: {other}

{HELP}"
            );
            std::process::exit(2);
        }
    }
}

fn run_plan_http_control_plane(args: impl Iterator<Item = String>) -> Result<String, String> {
    let mut request_path = None;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--request" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --request".to_string())?;
                request_path = Some(PathBuf::from(value));
            }
            other => return Err(format!("unknown plan-http-control-plane argument: {other}")),
        }
    }

    let request_path = request_path
        .ok_or_else(|| "plan-http-control-plane requires --request <path>".to_string())?;
    let request = load_control_plane_http_plan_request(&request_path)?;
    let http = GatewayHttpRequestMeta {
        method: parse_gateway_http_method(&request.method)?,
        path: request.path,
        body_len: request.body_len,
        headers: request
            .headers
            .into_iter()
            .map(|header| GatewayHttpHeader::new(header.name, header.value))
            .collect(),
    };
    let route = match plan_application_control_plane_http_route(&http) {
        Ok(route) => route,
        Err(error) => {
            let response = prodex_gateway_http::plan_gateway_control_plane_route_error_response(
                match &error {
                    prodex_application::ApplicationControlPlaneHttpRouteError::Route(error) => {
                        error
                    }
                },
            );
            return serde_json::to_string_pretty(&serde_json::json!({
                "planned": false,
                "status": gateway_control_plane_route_status_label(response.status),
                "code": response.code,
                "message": response.message,
            }))
            .map_err(|err| format!("failed to encode control-plane route error: {err}"));
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
                return serde_json::to_string_pretty(&serde_json::json!({
                    "planned": false,
                    "operation": control_plane_operation_label(route.operation),
                    "status": application_control_plane_idempotency_status_label(response.status),
                    "code": response.code,
                    "message": response.message,
                }))
                .map_err(|err| format!("failed to encode control-plane idempotency error: {err}"));
            }
        }
    } else {
        serde_json::json!({
            "required": false,
        })
    };

    serde_json::to_string_pretty(&serde_json::json!({
        "planned": true,
        "operation": control_plane_operation_label(route.operation),
        "resource_kind": resource_kind_label(route.operation.requirement().resource),
        "requires_idempotency": route.http.requires_idempotency,
        "requires_audit": route.http.requires_audit,
        "idempotency": idempotency,
    }))
    .map_err(|err| format!("failed to encode control-plane HTTP plan: {err}"))
}

fn run_plan_config_publication(args: impl Iterator<Item = String>) -> Result<String, String> {
    let mut request_path = None;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--request" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --request".to_string())?;
                request_path = Some(PathBuf::from(value));
            }
            other => {
                return Err(format!("unknown plan-config-publication argument: {other}"));
            }
        }
    }

    let request_path = request_path
        .ok_or_else(|| "plan-config-publication requires --request <path>".to_string())?;
    let request = load_config_publication_request(&request_path)?;
    match decide_configuration_publication(request) {
        ConfigurationPublicationDecision::Authorized(plan) => {
            serde_json::to_string_pretty(&serde_json::json!({
                "authorized": true,
                "tenant_id": plan.action.tenant.tenant_id,
                "revision_id": plan.candidate.revision_id,
                "required_role": plan.action.requirement.required_role,
                "resource_kind": plan.action.requirement.resource,
                "resource_action": plan.action.requirement.action,
                "audit_action": plan.action.audit_event.action.as_str(),
                "audit_outcome": plan.action.audit_event.outcome,
                "audit_partition_tenant_id": plan.action.audit_write.tenant_partition_key,
            }))
            .map_err(|err| format!("failed to encode publication plan: {err}"))
        }
        ConfigurationPublicationDecision::Denied {
            error,
            audit_write,
            audit_event,
        } => {
            let response = plan_configuration_publication_error_response(&error);
            serde_json::to_string_pretty(&serde_json::json!({
                "authorized": false,
                "status": configuration_publication_error_status_label(response.status),
                "code": response.code,
                "message": response.message,
                "audit_action": audit_event.action.as_str(),
                "audit_outcome": audit_event.outcome,
                "audit_reason_code": audit_event.reason_code,
                "audit_partition_tenant_id": audit_write.tenant_partition_key,
            }))
            .map_err(|err| format!("failed to encode publication denial: {err}"))
        }
    }
}

fn run_deliver_config_publication(args: impl Iterator<Item = String>) -> Result<String, String> {
    let mut event_path = None;
    let mut root = None;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--event" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --event".to_string())?;
                event_path = Some(PathBuf::from(value));
            }
            "--root" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --root".to_string())?;
                root = Some(PathBuf::from(value));
            }
            other => {
                return Err(format!(
                    "unknown deliver-config-publication argument: {other}"
                ));
            }
        }
    }

    let event_path = event_path
        .ok_or_else(|| "deliver-config-publication requires --event <path>".to_string())?;
    let root =
        root.ok_or_else(|| "deliver-config-publication requires --root <path>".to_string())?;
    let event = load_config_publication_event(&event_path)?;
    let delivery = deliver_config_publication_event_to_gateway_runtime(&event, &root)
        .map_err(|err| err.to_string())?;
    serde_json::to_string_pretty(&serde_json::json!({
        "root": delivery.root,
        "gateway_cache_refreshed": delivery.gateway_cache_refreshed,
        "runtime_policy_cached_version": delivery.runtime_policy_invalidation.cached_policy_version,
        "runtime_policy_cache_had_entry": delivery.runtime_policy_invalidation.had_cached_entry,
        "runtime_policy_version": delivery.runtime_policy_version,
        "delivery_metrics": delivery.delivery_metrics.iter().map(|metric| {
            serde_json::json!({
                "metric_name": metric.metric_name,
                "target": metric.target_label.as_metric_label().map(|(_, value)| value).unwrap_or("invalid"),
                "result": metric.result_label.as_metric_label().map(|(_, value)| value).unwrap_or("invalid"),
                "increment": metric.increment,
            })
        }).collect::<Vec<_>>(),
    }))
    .map_err(|err| format!("failed to encode delivery summary: {err}"))
}

fn load_config_publication_request(
    path: &PathBuf,
) -> Result<ConfigurationPublicationRequest<serde_json::Value>, String> {
    let bytes = std::fs::read(path).map_err(|err| {
        format!(
            "failed to read publication request {}: {err}",
            path.display()
        )
    })?;
    let request: ConfigPublicationPlanFile = serde_json::from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse publication request {}: {err}",
            path.display()
        )
    })?;
    Ok(ConfigurationPublicationRequest {
        action: ControlPlaneActionRequest {
            principal: request.principal,
            operation: ControlPlaneOperation::ConfigurationPublish,
            resource: ControlPlaneResourceRef::new(
                request.candidate.tenant_id,
                ResourceKind::Configuration,
                Some(request.candidate.revision_id.to_string()),
            ),
            occurred_at_unix_ms: request.occurred_at_unix_ms,
        },
        current_revision_id: request.current_revision_id,
        candidate: prodex_config::ConfigRevision {
            tenant_id: request.candidate.tenant_id,
            revision_id: request.candidate.revision_id,
            published_at_unix_ms: request.candidate.published_at_unix_ms,
            payload: request.candidate.payload,
        },
    })
}

fn load_control_plane_http_plan_request(
    path: &PathBuf,
) -> Result<ControlPlaneHttpPlanFile, String> {
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

fn configuration_publication_error_status_label(
    status: ConfigurationPublicationErrorStatus,
) -> &'static str {
    match status {
        ConfigurationPublicationErrorStatus::BadRequest => "bad_request",
        ConfigurationPublicationErrorStatus::Conflict => "conflict",
        ConfigurationPublicationErrorStatus::Forbidden => "forbidden",
    }
}

fn gateway_control_plane_route_status_label(
    status: prodex_gateway_http::GatewayControlPlaneRouteErrorStatus,
) -> &'static str {
    match status {
        prodex_gateway_http::GatewayControlPlaneRouteErrorStatus::BadRequest => "bad_request",
        prodex_gateway_http::GatewayControlPlaneRouteErrorStatus::MethodNotAllowed => {
            "method_not_allowed"
        }
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

fn control_plane_operation_label(operation: ControlPlaneOperation) -> &'static str {
    match operation {
        ControlPlaneOperation::GatewayAdminRead => "gateway_admin_read",
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

fn load_config_publication_event(path: &PathBuf) -> Result<ConfigPublicationEventPlan, String> {
    let bytes = std::fs::read(path)
        .map_err(|err| format!("failed to read publication event {}: {err}", path.display()))?;
    let event: ConfigPublicationEventFile = serde_json::from_slice(&bytes).map_err(|err| {
        format!(
            "failed to parse publication event {}: {err}",
            path.display()
        )
    })?;
    Ok(ConfigPublicationEventPlan {
        tenant_id: event.tenant_id,
        activated_revision_id: event.activated_revision_id,
        previous_active_revision_id: event.previous_active_revision_id,
        last_known_good_revision_id: event.last_known_good_revision_id,
        targets: parse_config_publication_targets(event.targets)?,
    })
}

fn parse_config_publication_targets(
    targets: Vec<String>,
) -> Result<[ConfigPublicationEventTarget; 2], String> {
    let mut parsed = Vec::with_capacity(targets.len());
    for target in targets {
        let parsed_target = match target.as_str() {
            "gateway_cache_refresh" => ConfigPublicationEventTarget::GatewayCacheRefresh,
            "runtime_policy_reload" => ConfigPublicationEventTarget::RuntimePolicyReload,
            _ => return Err(format!("unknown publication target: {target}")),
        };
        if !parsed.contains(&parsed_target) {
            parsed.push(parsed_target);
        }
    }
    if parsed.len() != 2 {
        return Err(
            "publication targets must include gateway_cache_refresh and runtime_policy_reload"
                .to_string(),
        );
    }
    let mut gateway = None;
    let mut runtime = None;
    for target in parsed {
        match target {
            ConfigPublicationEventTarget::GatewayCacheRefresh => gateway = Some(target),
            ConfigPublicationEventTarget::RuntimePolicyReload => runtime = Some(target),
        }
    }
    Ok([
        gateway
            .ok_or_else(|| "publication targets must include gateway_cache_refresh".to_string())?,
        runtime
            .ok_or_else(|| "publication targets must include runtime_policy_reload".to_string())?,
    ])
}
