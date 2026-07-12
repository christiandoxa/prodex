//! Prodex control-plane entrypoint.
//!
//! This binary stays a thin composition root. It exposes one-shot publication
//! commands and a route-isolated async in-process control-plane listener.

use std::path::PathBuf;

use prodex::{
    DedicatedServerMode, OtlpLogAttribute, compact_config_publication_transport,
    deliver_config_publication_event_to_gateway_runtime, otlp_http_log_export_status,
    publish_config_publication_event_to_gateway_transport, run_enterprise_serve_or_exit,
};
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
use prodex_config::{ConfigPublicationEventPlan, ConfigPublicationEventTarget};
use prodex_control_plane::{
    ConfigurationPublicationDecision, ConfigurationPublicationErrorStatus,
    ConfigurationPublicationPlan, ConfigurationPublicationRequest, ControlPlaneActionRequest,
    ControlPlaneOperation, ControlPlaneResourceRef, decide_configuration_publication,
    plan_configuration_publication_error_response,
};
use prodex_domain::{
    AuditDigest, CallId, PolicyRevisionId, Principal, RequestId, ResourceAction, ResourceKind,
    Role, TenantId,
};
use prodex_gateway_http::{
    GatewayHttpHeader, GatewayHttpMethod, GatewayHttpPolicy, GatewayHttpRequestMeta,
};
use prodex_storage::DurableStoreKind;
use serde::Deserialize;

const HELP: &str = "prodex-control-plane

Control-plane entrypoint.

USAGE:
    prodex-control-plane --help
    prodex-control-plane --version
    prodex-control-plane serve [--listen <ADDR>]
    prodex-control-plane plan-config-publication --request <path>
    prodex-control-plane plan-http-control-plane --request <path>
    prodex-control-plane deliver-config-publication --event <path> --root <path>
    prodex-control-plane publish-config-publication --event <path> --transport <path>
    prodex-control-plane compact-config-publication --transport <path> [--retain <n>]

STATUS:
    The dedicated control-plane composition root uses an async listener that
    rejects data-plane routes and dispatches through the in-process application.
";

const CONTROL_PLANE_SERVICE_NAME: &str = "prodex-control-plane";
const CONTROL_PLANE_SCOPE_NAME: &str = "prodex.control-plane";
const CONTROL_PLANE_HTTP_ROUTE_PLAN_EVENT_NAME: &str = "control_plane.http_route.plan";
const CONTROL_PLANE_CONFIGURATION_PUBLICATION_PLAN_EVENT_NAME: &str =
    "control_plane.configuration_publication.plan";
const CONTROL_PLANE_CONFIGURATION_PUBLICATION_PUBLISH_EVENT_NAME: &str =
    "control_plane.configuration_publication.publish";
const CONTROL_PLANE_CONFIGURATION_PUBLICATION_DELIVER_EVENT_NAME: &str =
    "config_publication.deliver";
const CONTROL_PLANE_CONFIGURATION_PUBLICATION_COMPACT_EVENT_NAME: &str =
    "control_plane.configuration_publication.compact";

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
            run_enterprise_serve_or_exit(DedicatedServerMode::ControlPlane, args, HELP);
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
        Some("publish-config-publication") => match run_publish_config_publication(args) {
            Ok(output) => println!("{output}"),
            Err(err) => {
                eprintln!(
                    "{err}

{HELP}"
                );
                std::process::exit(2);
            }
        },
        Some("compact-config-publication") => match run_compact_config_publication(args) {
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
    let (path, query) = split_path_and_query(&request.path);
    let http = GatewayHttpRequestMeta {
        method: parse_gateway_http_method(&request.method)?,
        path,
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
            encode_configuration_publication_plan_success(plan)
        }
        ConfigurationPublicationDecision::Denied {
            error,
            audit_write,
            audit_event,
        } => {
            let response = plan_configuration_publication_error_response(&error);
            encode_configuration_publication_plan_failure(
                configuration_publication_error_status_label(response.status),
                response.code,
                response.message,
                audit_event.action.as_str(),
                serde_json::to_value(audit_event.outcome).unwrap_or(serde_json::Value::Null),
                audit_event.reason_code,
                audit_write.tenant_partition_key,
            )
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
    encode_configuration_publication_delivery_summary(&delivery)
}

fn run_publish_config_publication(args: impl Iterator<Item = String>) -> Result<String, String> {
    let mut event_path = None;
    let mut transport = None;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--event" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --event".to_string())?;
                event_path = Some(PathBuf::from(value));
            }
            "--transport" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --transport".to_string())?;
                transport = Some(PathBuf::from(value));
            }
            other => {
                return Err(format!(
                    "unknown publish-config-publication argument: {other}"
                ));
            }
        }
    }

    let event_path = event_path
        .ok_or_else(|| "publish-config-publication requires --event <path>".to_string())?;
    let transport = transport
        .ok_or_else(|| "publish-config-publication requires --transport <path>".to_string())?;
    let event = load_config_publication_event(&event_path)?;
    let publication = publish_config_publication_event_to_gateway_transport(&event, &transport)
        .map_err(|err| err.to_string())?;
    encode_configuration_publication_publish_summary(
        &publication.transport_root,
        &publication.event_path,
        publication.event_id.as_str(),
    )
}

fn run_compact_config_publication(args: impl Iterator<Item = String>) -> Result<String, String> {
    let mut transport = None;
    let mut retain = 0usize;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--transport" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --transport".to_string())?;
                transport = Some(PathBuf::from(value));
            }
            "--retain" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value for --retain".to_string())?;
                retain = value.parse::<usize>().map_err(|_| {
                    "compact-config-publication requires numeric --retain".to_string()
                })?;
            }
            other => {
                return Err(format!(
                    "unknown compact-config-publication argument: {other}"
                ));
            }
        }
    }

    let transport = transport
        .ok_or_else(|| "compact-config-publication requires --transport <path>".to_string())?;
    let compaction =
        compact_config_publication_transport(&transport, retain).map_err(|err| err.to_string())?;
    encode_configuration_publication_compaction_summary(&compaction)
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

fn encode_configuration_publication_plan_failure(
    status: &str,
    code: &str,
    message: &str,
    audit_action: &str,
    audit_outcome: serde_json::Value,
    audit_reason_code: Option<String>,
    audit_partition_tenant_id: TenantId,
) -> Result<String, String> {
    let otlp_log_export = otlp_http_log_export_status(
        CONTROL_PLANE_SERVICE_NAME,
        CONTROL_PLANE_SCOPE_NAME,
        CONTROL_PLANE_CONFIGURATION_PUBLICATION_PLAN_EVENT_NAME,
        vec![
            OtlpLogAttribute::bool("authorized", false),
            OtlpLogAttribute::string("status", status),
            OtlpLogAttribute::string("code", code),
        ],
    );
    serde_json::to_string_pretty(&serde_json::json!({
        "authorized": false,
        "status": status,
        "code": code,
        "message": message,
        "audit_action": audit_action,
        "audit_outcome": audit_outcome,
        "audit_reason_code": audit_reason_code,
        "audit_partition_tenant_id": audit_partition_tenant_id,
        "otlp_log_export": otlp_log_export,
    }))
    .map_err(|err| format!("failed to encode publication denial: {err}"))
}

fn encode_configuration_publication_plan_success(
    plan: ConfigurationPublicationPlan<serde_json::Value>,
) -> Result<String, String> {
    let audit_outcome =
        serde_json::to_value(plan.action.audit_event.outcome).unwrap_or(serde_json::Value::Null);
    let otlp_log_export = otlp_http_log_export_status(
        CONTROL_PLANE_SERVICE_NAME,
        CONTROL_PLANE_SCOPE_NAME,
        CONTROL_PLANE_CONFIGURATION_PUBLICATION_PLAN_EVENT_NAME,
        vec![
            OtlpLogAttribute::bool("authorized", true),
            OtlpLogAttribute::string(
                "required_role",
                role_label(plan.action.requirement.required_role),
            ),
            OtlpLogAttribute::string(
                "resource_action",
                resource_action_label(plan.action.requirement.action),
            ),
        ],
    );
    serde_json::to_string_pretty(&serde_json::json!({
        "authorized": true,
        "tenant_id": plan.action.tenant.tenant_id,
        "revision_id": plan.candidate.revision_id,
        "required_role": plan.action.requirement.required_role,
        "resource_kind": plan.action.requirement.resource,
        "resource_action": plan.action.requirement.action,
        "audit_action": plan.action.audit_event.action,
        "audit_outcome": audit_outcome,
        "audit_partition_tenant_id": plan.action.audit_write.tenant_partition_key,
        "otlp_log_export": otlp_log_export,
    }))
    .map_err(|err| format!("failed to encode publication plan: {err}"))
}

fn encode_configuration_publication_publish_summary(
    transport_root: &std::path::Path,
    event_path: &std::path::Path,
    event_id: &str,
) -> Result<String, String> {
    let otlp_log_export = otlp_http_log_export_status(
        CONTROL_PLANE_SERVICE_NAME,
        CONTROL_PLANE_SCOPE_NAME,
        CONTROL_PLANE_CONFIGURATION_PUBLICATION_PUBLISH_EVENT_NAME,
        vec![OtlpLogAttribute::bool("published", true)],
    );
    serde_json::to_string_pretty(&serde_json::json!({
        "transport_root": transport_root,
        "event_path": event_path,
        "event_id": event_id,
        "otlp_log_export": otlp_log_export,
    }))
    .map_err(|err| format!("failed to encode publication transport summary: {err}"))
}

fn encode_configuration_publication_delivery_summary(
    delivery: &prodex::RuntimePolicyPublicationDeliveryPlan,
) -> Result<String, String> {
    let otlp_log_export = otlp_http_log_export_status(
        CONTROL_PLANE_SERVICE_NAME,
        CONTROL_PLANE_SCOPE_NAME,
        CONTROL_PLANE_CONFIGURATION_PUBLICATION_DELIVER_EVENT_NAME,
        {
            let mut attributes = vec![OtlpLogAttribute::bool(
                "gateway_cache_refreshed",
                delivery.gateway_cache_refreshed,
            )];
            if let Some(version) = delivery.runtime_policy_version {
                attributes.push(OtlpLogAttribute::u64(
                    "runtime_policy_version",
                    version as u64,
                ));
            }
            attributes
        },
    );
    serde_json::to_string_pretty(&serde_json::json!({
        "root": delivery.root,
        "gateway_cache_refreshed": delivery.gateway_cache_refreshed,
        "runtime_policy_cached_version": delivery.runtime_policy_invalidation.cached_policy_version,
        "runtime_policy_cache_had_entry": delivery.runtime_policy_invalidation.had_cached_entry,
        "runtime_policy_version": delivery.runtime_policy_version,
        "otlp_log_export": otlp_log_export,
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

fn encode_configuration_publication_compaction_summary(
    compaction: &prodex::ConfigPublicationTransportCompactionPlan,
) -> Result<String, String> {
    let otlp_log_export = otlp_http_log_export_status(
        CONTROL_PLANE_SERVICE_NAME,
        CONTROL_PLANE_SCOPE_NAME,
        CONTROL_PLANE_CONFIGURATION_PUBLICATION_COMPACT_EVENT_NAME,
        vec![
            OtlpLogAttribute::u64("replica_count", compaction.replica_count as u64),
            OtlpLogAttribute::u64(
                "eligible_event_count",
                compaction.eligible_event_count as u64,
            ),
            OtlpLogAttribute::u64("removed_event_count", compaction.removed_event_count as u64),
            OtlpLogAttribute::u64(
                "retained_event_count",
                compaction.retained_event_count as u64,
            ),
        ],
    );
    serde_json::to_string_pretty(&serde_json::json!({
        "transport_root": compaction.transport_root,
        "replica_count": compaction.replica_count,
        "eligible_event_count": compaction.eligible_event_count,
        "removed_event_count": compaction.removed_event_count,
        "retained_event_count": compaction.retained_event_count,
        "otlp_log_export": otlp_log_export,
    }))
    .map_err(|err| format!("failed to encode compaction summary: {err}"))
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

fn role_label(role: Role) -> &'static str {
    match role {
        Role::Viewer => "viewer",
        Role::Operator => "operator",
        Role::Admin => "admin",
    }
}

fn resource_action_label(action: ResourceAction) -> &'static str {
    match action {
        ResourceAction::Read => "read",
        ResourceAction::Create => "create",
        ResourceAction::Update => "update",
        ResourceAction::Delete => "delete",
        ResourceAction::RotateSecret => "rotate_secret",
        ResourceAction::PublishRevision => "publish_revision",
        ResourceAction::Export => "export",
    }
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
