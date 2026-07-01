//! Prodex control-plane entrypoint.
//!
//! This binary stays a thin composition root. It now exposes a one-shot
//! configuration-publication delivery command while long-lived admin serving
//! remains gated until adapters are wired behind the enterprise boundaries.

use std::path::PathBuf;

use prodex::deliver_config_publication_event_to_gateway_runtime;
use prodex_config::{ConfigPublicationEventPlan, ConfigPublicationEventTarget};
use prodex_control_plane::{
    ConfigurationPublicationDecision, ConfigurationPublicationErrorStatus,
    ConfigurationPublicationRequest, ControlPlaneActionRequest, ControlPlaneOperation,
    ControlPlaneResourceRef, decide_configuration_publication,
    plan_configuration_publication_error_response,
};
use prodex_domain::{PolicyRevisionId, Principal, ResourceKind, TenantId};
use serde::Deserialize;

const HELP: &str = "prodex-control-plane

Control-plane entrypoint.

USAGE:
    prodex-control-plane --help
    prodex-control-plane --version
    prodex-control-plane plan-config-publication --request <path>
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

fn configuration_publication_error_status_label(
    status: ConfigurationPublicationErrorStatus,
) -> &'static str {
    match status {
        ConfigurationPublicationErrorStatus::BadRequest => "bad_request",
        ConfigurationPublicationErrorStatus::Conflict => "conflict",
        ConfigurationPublicationErrorStatus::Forbidden => "forbidden",
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
