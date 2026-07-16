use std::path::Path;

use prodex::{
    OtlpLogAttribute, compact_config_publication_transport,
    deliver_config_publication_event_to_gateway_runtime, otlp_http_log_export_status,
    publish_config_publication_event_to_gateway_transport,
};
use prodex_config::{ConfigPublicationEventPlan, ConfigPublicationEventTarget};
use prodex_control_plane::{
    ConfigurationPublicationDecision, ConfigurationPublicationErrorStatus,
    ConfigurationPublicationPlan, ConfigurationPublicationRequest, ControlPlaneActionRequest,
    ControlPlaneOperation, ControlPlaneResourceRef, decide_configuration_publication,
    plan_configuration_publication_error_response,
};
use prodex_domain::{PolicyRevisionId, Principal, ResourceAction, ResourceKind, Role, TenantId};
use serde::Deserialize;

use super::{
    CONTROL_PLANE_CONFIGURATION_PUBLICATION_COMPACT_EVENT_NAME,
    CONTROL_PLANE_CONFIGURATION_PUBLICATION_DELIVER_EVENT_NAME,
    CONTROL_PLANE_CONFIGURATION_PUBLICATION_PLAN_EVENT_NAME,
    CONTROL_PLANE_CONFIGURATION_PUBLICATION_PUBLISH_EVENT_NAME, CONTROL_PLANE_SCOPE_NAME,
    CONTROL_PLANE_SERVICE_NAME,
};

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

pub(super) fn plan(request_path: &Path) -> Result<String, String> {
    let request = load_config_publication_request(request_path)?;
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

pub(super) fn deliver(event_path: &Path, root: &Path) -> Result<String, String> {
    let event = load_config_publication_event(event_path)?;
    let delivery = deliver_config_publication_event_to_gateway_runtime(&event, root)
        .map_err(|err| err.to_string())?;
    encode_configuration_publication_delivery_summary(&delivery)
}

pub(super) fn publish(event_path: &Path, transport: &Path) -> Result<String, String> {
    let event = load_config_publication_event(event_path)?;
    let publication = publish_config_publication_event_to_gateway_transport(&event, transport)
        .map_err(|err| err.to_string())?;
    encode_configuration_publication_publish_summary(
        &publication.transport_root,
        &publication.event_path,
        publication.event_id.as_str(),
    )
}

pub(super) fn compact(transport: &Path, retain: usize) -> Result<String, String> {
    let compaction =
        compact_config_publication_transport(transport, retain).map_err(|err| err.to_string())?;
    encode_configuration_publication_compaction_summary(&compaction)
}

fn load_config_publication_request(
    path: &Path,
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

fn load_config_publication_event(path: &Path) -> Result<ConfigPublicationEventPlan, String> {
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
