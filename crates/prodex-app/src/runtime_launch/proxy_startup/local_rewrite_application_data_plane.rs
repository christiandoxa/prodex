use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_router::runtime_gateway_http_request_meta;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use super::local_rewrite_provider_registry::{
    RuntimeGatewayGovernedProviderRegistrySnapshot, RuntimeGatewayProviderRuntimeSignals,
    RuntimeGatewayProviderRuntimeSnapshot,
};
use super::local_rewrite_response_guardrails::runtime_gateway_response_inspection_coverage;
use super::provider_bridge::{RuntimeProviderGatewaySpendEvent, runtime_provider_model_from_body};
use crate::{
    RuntimeProxyRequest, runtime_profile_in_selection_backoff, runtime_profile_inflight_sort_key,
    runtime_profile_route_circuit_open_until, runtime_profile_route_health_key,
    runtime_profile_route_health_score, runtime_proxy_log,
};
use prodex_application::{
    ApplicationAuthorizedRequestContext, ApplicationDataPlaneError, ApplicationDataPlanePlan,
    ApplicationDataPlaneRequest, ApplicationExecutionApprovalDecision,
    ApplicationExecutionApprovalRequest, ApplicationExecutionApprovalService,
    ApplicationGovernancePlan, ApplicationGovernanceRequest, ApplicationGovernanceSnapshot,
    ApplicationInspectionPlan, ApplicationObligationContext, ApplicationObligationDisposition,
    ApplicationObligationExecutionPlan, ApplicationObligationMode, ApplicationProviderRetryRequest,
    ApplicationResponseObligationPlan, ApplicationResponseTransport,
    ApplicationUsageReconciliationBackend, ApplicationUsageReconciliationError,
    ApplicationUsageReconciliationExecutionPlan, ApplicationUsageReconciliationExecutionRequest,
    ApplicationUsageReconciliationPlan, ApplicationUsageReconciliationRequest,
    plan_application_data_plane, plan_application_data_plane_execution,
    plan_application_governance, plan_application_obligation_execution,
    plan_application_provider_retry, plan_application_usage_reconciliation,
    plan_application_usage_reconciliation_execution,
};
use prodex_domain::{
    ApprovalId, ApprovalState, AuditAction, AuditEvent, AuditEventId, AuditOutcome, AuditResource,
    CanonicalRoute, CapabilitySet, Channel, CredentialScope, DataClassification, DataModality,
    EnvironmentContext, ExecutionApprovalBinding, GovernedAction, ModelCapability, NetworkZone,
    PolicyEffect, Principal, PrincipalPolicyAttributes, QuotaContext, RequestId,
    RequestPolicyAttributes, RequestRisk, ReservationReconciliationReason, ReservationRecord,
    SecretRef, SessionPolicyContext, TenantContext, TenantId, TenantScopedResource, UsageAmount,
    compute_audit_chain_digest, execution_approval_id,
};
use prodex_gateway_core::{GatewayAdmissionRequest, GatewayUsageReconciliationRequest};
use prodex_gateway_http::{GatewayHttpExecutionPlan, GatewayHttpPolicy, GatewayHttpRouteKind};
use prodex_observability::{TraceContext, TraceContextError};
use prodex_provider_core::{
    ProviderCapabilityStatus, ProviderEndpoint, ProviderErrorClass, ProviderId, provider_adapter,
    provider_catalog_entries_for,
};
use prodex_provider_spi::{
    GovernedRoutingError, GovernedRoutingPlan, GovernedRoutingRequest,
    MAX_GOVERNED_ROUTING_FALLBACKS, ProviderInvocation, ProviderRetryCause, ProviderRetryDecision,
    ProviderRetryPolicy, ProviderRetryStage, ProviderRoute, ProviderRouteError, ProviderStreamMode,
    plan_governed_provider_route_with_model,
};
use prodex_quota::RuntimeQuotaWindowStatus;
use prodex_storage::{
    AppendOnlyAuditCommand, AtomicReservationCommand, AuditOutboxWriteCommand, DurableStoreKind,
    TenantStorageKey, UsageReconciliationCommand,
};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;
use std::sync::atomic::Ordering;

mod compatibility;
mod governance_decision;
mod planning;
mod provider_dispatch;
pub(super) use self::planning::{
    runtime_gateway_application_trace_context,
    runtime_gateway_buffered_response_is_locally_inspectable, runtime_gateway_governance_route,
    runtime_gateway_governed_action, runtime_gateway_normalized_load,
    runtime_gateway_provider_capability_is_executable, runtime_gateway_provider_credential_ref,
    runtime_gateway_provider_endpoint, runtime_gateway_provider_executable_capabilities,
    runtime_gateway_provider_stream_mode, runtime_gateway_quota_window_headroom,
    runtime_gateway_requested_capabilities, runtime_gateway_requested_modalities,
    runtime_gateway_requested_output_tokens, runtime_gateway_requested_tools,
    runtime_gateway_route_kind, runtime_gateway_route_uses_compact_dispatch,
    runtime_gateway_route_uses_models_dispatch,
};
pub(super) use self::provider_dispatch::{
    RuntimeGatewayApplicationProviderDispatch, RuntimeGatewayApplicationReconciliationInput,
    runtime_gateway_application_provider_dispatch,
    runtime_gateway_application_provider_dispatch_attempt,
    runtime_gateway_application_provider_retry_precommit,
    runtime_gateway_application_provider_stage_is_committed,
    runtime_gateway_application_reconciliation_execution,
    runtime_gateway_application_usage_reconciliation,
};
#[cfg(test)]
use compatibility::runtime_gateway_compatibility_http_route;
use governance_decision::runtime_gateway_governance_decision;
#[cfg(test)]
use planning::MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RuntimeGatewayTenantResource {
    pub(super) tenant_id: TenantId,
}

impl TenantScopedResource for RuntimeGatewayTenantResource {
    fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }
}

#[derive(Clone)]
pub(super) struct RuntimeGatewayApplicationAdmission(RuntimeGatewayApplicationAdmissionKind);

#[derive(Clone)]
enum RuntimeGatewayApplicationAdmissionKind {
    TenantBound {
        tenant: TenantContext,
        principal: Principal,
        plan: Box<ApplicationDataPlanePlan>,
        routing: Option<Box<GovernedRoutingPlan>>,
        obligations: Box<ApplicationObligationExecutionPlan>,
    },
    CompatibilityAnonymous {
        invocation: RuntimeGatewayCompatibilityProviderInvocation,
        inspection: ApplicationInspectionPlan,
    },
}

#[derive(Clone, Copy)]
struct RuntimeGatewayCompatibilityProviderInvocation {
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    stream_mode: ProviderStreamMode,
}

impl RuntimeGatewayApplicationAdmission {
    pub(super) fn audit_context(&self) -> Option<(TenantContext, Principal)> {
        match &self.0 {
            RuntimeGatewayApplicationAdmissionKind::TenantBound {
                tenant, principal, ..
            } => Some((*tenant, principal.clone())),
            RuntimeGatewayApplicationAdmissionKind::CompatibilityAnonymous { .. } => None,
        }
    }

    pub(super) fn tenant_bound(&self) -> Option<&ApplicationDataPlanePlan> {
        match &self.0 {
            RuntimeGatewayApplicationAdmissionKind::TenantBound { plan, .. } => Some(plan),
            RuntimeGatewayApplicationAdmissionKind::CompatibilityAnonymous { .. } => None,
        }
    }

    pub(super) fn inspection(&self) -> &ApplicationInspectionPlan {
        match &self.0 {
            RuntimeGatewayApplicationAdmissionKind::TenantBound { plan, .. } => &plan.inspection,
            RuntimeGatewayApplicationAdmissionKind::CompatibilityAnonymous {
                inspection, ..
            } => inspection,
        }
    }

    pub(super) fn governance(&self) -> Option<&ApplicationGovernancePlan> {
        self.tenant_bound().map(|plan| &plan.governance)
    }

    pub(super) fn routing(&self) -> Option<&GovernedRoutingPlan> {
        let RuntimeGatewayApplicationAdmissionKind::TenantBound { routing, .. } = &self.0 else {
            return None;
        };
        routing.as_deref()
    }

    pub(super) fn response_obligations(&self) -> Option<ApplicationResponseObligationPlan> {
        match &self.0 {
            RuntimeGatewayApplicationAdmissionKind::TenantBound { obligations, .. } => {
                Some(obligations.response)
            }
            RuntimeGatewayApplicationAdmissionKind::CompatibilityAnonymous { .. } => None,
        }
    }
}

#[derive(Debug)]
pub(super) enum RuntimeGatewayApplicationDataPlaneError {
    Execution(prodex_application::ApplicationDataPlaneExecutionError),
    MissingPrincipal,
    RouteUnavailable,
    ProviderRoute(ProviderRouteError),
    TraceContext(TraceContextError),
    Admission(ApplicationDataPlaneError),
    GovernanceDenied,
    GovernanceApprovalRequired {
        approval_id: ApprovalId,
        state: ApprovalState,
    },
    GovernanceSessionRequired,
    GovernanceUnavailable,
    NoEligibleProvider,
}

impl fmt::Display for RuntimeGatewayApplicationDataPlaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Execution(error) => error.fmt(f),
            Self::Admission(error) => error.fmt(f),
            Self::ProviderRoute(error) => error.fmt(f),
            Self::TraceContext(error) => error.fmt(f),
            Self::MissingPrincipal
            | Self::RouteUnavailable
            | Self::GovernanceDenied
            | Self::GovernanceApprovalRequired { .. }
            | Self::GovernanceSessionRequired
            | Self::GovernanceUnavailable
            | Self::NoEligibleProvider => {
                write!(f, "application data-plane request is invalid")
            }
        }
    }
}

impl Error for RuntimeGatewayApplicationDataPlaneError {}

pub(super) fn runtime_gateway_application_http_policy(
    shared: &RuntimeLocalRewriteProxyShared,
) -> GatewayHttpPolicy {
    let defaults = GatewayHttpPolicy::production_default();
    let stream_idle_timeout_ms = shared
        .runtime_shared
        .runtime_config
        .tuning
        .stream_idle_timeout_ms;
    GatewayHttpPolicy {
        max_body_bytes: usize::try_from(
            shared.runtime_shared.runtime_config.max_request_body_bytes,
        )
        .unwrap_or(usize::MAX),
        max_header_count: defaults.max_header_count,
        max_header_bytes: defaults.max_header_bytes,
        max_single_header_bytes: defaults.max_single_header_bytes,
        request_timeout_ms: defaults.request_timeout_ms.max(stream_idle_timeout_ms),
        stream_idle_timeout_ms,
        max_concurrent_streams: u32::try_from(shared.runtime_shared.active_request_limit)
            .unwrap_or(u32::MAX)
            .max(1),
        connection_drain_timeout_ms: defaults.connection_drain_timeout_ms,
        require_trace_context: false,
    }
}

pub(super) fn runtime_gateway_application_local_admission(
    authorized: &ApplicationAuthorizedRequestContext<'_>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<GatewayHttpExecutionPlan, RuntimeGatewayApplicationDataPlaneError> {
    plan_application_data_plane_execution(
        runtime_gateway_application_http_policy(shared),
        authorized,
    )
    .map_err(RuntimeGatewayApplicationDataPlaneError::Execution)
}

pub(super) fn runtime_gateway_application_data_plane_admission(
    authorized: &ApplicationAuthorizedRequestContext<'_>,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    network_zone: NetworkZone,
    principal_attributes: PrincipalPolicyAttributes,
    reservation: AtomicReservationCommand,
    inspection: ApplicationInspectionPlan,
) -> Result<RuntimeGatewayApplicationAdmission, RuntimeGatewayApplicationDataPlaneError> {
    let Some(tenant) = authorized.tenant_context() else {
        if !shared
            .runtime_shared
            .runtime_config
            .governance
            .mode
            .allows_anonymous_compatibility()
        {
            return Err(RuntimeGatewayApplicationDataPlaneError::MissingPrincipal);
        }
        return RuntimeGatewayApplicationAdmission::compatibility_anonymous(
            authorized.request().route(),
            captured,
            shared,
            inspection,
        );
    };
    let principal = authorized
        .principal()
        .cloned()
        .ok_or(RuntimeGatewayApplicationDataPlaneError::MissingPrincipal)?;
    let request_id = authorized.request().request_id();
    let trace_context = match authorized.request().trace_context() {
        Some(trace_context) => trace_context.clone(),
        None => runtime_gateway_application_trace_context(request_id)
            .map_err(RuntimeGatewayApplicationDataPlaneError::TraceContext)?,
    };
    let governance = runtime_gateway_governance_decision(
        authorized,
        captured,
        shared,
        network_zone,
        &principal_attributes,
        Some(&reservation),
        &inspection,
    )?;
    let provider_invocation =
        runtime_gateway_provider_invocation(RuntimeGatewayProviderInvocationInput {
            tenant,
            principal: principal.clone(),
            request_id,
            reservation: &reservation,
            route: authorized.request().route(),
            captured,
            shared,
            routing: governance.routing.as_ref(),
        })?;
    let http = runtime_gateway_http_request_meta(captured, captured.path_and_query.as_str());
    let request = ApplicationDataPlaneRequest {
        http,
        inspection,
        governance: governance.plan,
        admission: GatewayAdmissionRequest {
            tenant,
            principal: principal.clone(),
            resource: RuntimeGatewayTenantResource {
                tenant_id: tenant.tenant_id,
            },
            reservation,
            provider_invocation,
            trace_context,
        },
    };
    plan_application_data_plane(runtime_gateway_application_http_policy(shared), request)
        .map(|plan| {
            RuntimeGatewayApplicationAdmission(
                RuntimeGatewayApplicationAdmissionKind::TenantBound {
                    tenant,
                    principal,
                    plan: Box::new(plan),
                    routing: governance.routing.map(Box::new),
                    obligations: Box::new(governance.obligations),
                },
            )
        })
        .map_err(RuntimeGatewayApplicationDataPlaneError::Admission)
}

pub(super) fn runtime_gateway_application_websocket_governance(
    authorized: Option<&ApplicationAuthorizedRequestContext<'_>>,
    text: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    network_zone: NetworkZone,
    inspection: &ApplicationInspectionPlan,
) -> Result<Option<ApplicationResponseObligationPlan>, RuntimeGatewayApplicationDataPlaneError> {
    let requires_identity = !shared
        .runtime_shared
        .runtime_config
        .governance
        .mode
        .allows_anonymous_compatibility();
    let Some(authorized) = authorized else {
        return if requires_identity {
            Err(RuntimeGatewayApplicationDataPlaneError::MissingPrincipal)
        } else {
            Ok(None)
        };
    };
    if authorized.tenant_context().is_none() || authorized.principal().is_none() {
        return if requires_identity {
            Err(RuntimeGatewayApplicationDataPlaneError::MissingPrincipal)
        } else {
            Ok(None)
        };
    }
    let captured = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/realtime".to_string(),
        headers: vec![("upgrade".to_string(), "websocket".to_string())],
        body: text.as_bytes().to_vec(),
    };
    let decision = runtime_gateway_governance_decision(
        authorized,
        &captured,
        shared,
        network_zone,
        &PrincipalPolicyAttributes::default(),
        None,
        inspection,
    )?;
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_websocket_governance_decision",
            [
                runtime_proxy_log_field(
                    "classification",
                    decision.plan.classification.classification().as_str(),
                ),
                runtime_proxy_log_field(
                    "coverage",
                    decision.plan.classification.coverage().as_str(),
                ),
                runtime_proxy_log_field(
                    "provider",
                    decision
                        .routing
                        .as_ref()
                        .map(|routing| routing.primary.provider.label())
                        .unwrap_or("legacy-observe"),
                ),
            ],
        ),
    );
    Ok(Some(decision.obligations.response))
}

fn runtime_gateway_compatibility_provider_invocation(
    provider: ProviderId,
    route: GatewayHttpRouteKind,
    captured: &RuntimeProxyRequest,
) -> Result<RuntimeGatewayCompatibilityProviderInvocation, RuntimeGatewayApplicationDataPlaneError>
{
    let endpoint = runtime_gateway_provider_endpoint(route)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::RouteUnavailable)?;
    Ok(RuntimeGatewayCompatibilityProviderInvocation {
        provider,
        endpoint,
        stream_mode: runtime_gateway_provider_stream_mode(captured),
    })
}

struct RuntimeGatewayProviderInvocationInput<'a> {
    tenant: TenantContext,
    principal: Principal,
    request_id: RequestId,
    reservation: &'a AtomicReservationCommand,
    route: GatewayHttpRouteKind,
    captured: &'a RuntimeProxyRequest,
    shared: &'a RuntimeLocalRewriteProxyShared,
    routing: Option<&'a GovernedRoutingPlan>,
}

struct RuntimeGatewayGovernanceDecision {
    plan: ApplicationGovernancePlan,
    routing: Option<GovernedRoutingPlan>,
    obligations: ApplicationObligationExecutionPlan,
}

fn runtime_gateway_provider_runtime_snapshot(
    shared: &RuntimeLocalRewriteProxyShared,
    registry: &RuntimeGatewayGovernedProviderRegistrySnapshot,
    route: GatewayHttpRouteKind,
) -> Result<RuntimeGatewayProviderRuntimeSnapshot, RuntimeGatewayApplicationDataPlaneError> {
    let now = (runtime_gateway_unix_epoch_millis() / 1_000) as i64;
    let route_kind = runtime_gateway_route_kind(route);
    let admission = &shared.runtime_shared.lane_admission;
    let lane_active = admission.active_counter(route_kind).load(Ordering::Relaxed);
    let lane_limit = admission.limit(route_kind);
    let global_active = shared
        .runtime_shared
        .active_request_count
        .load(Ordering::Relaxed);
    let global_limit = shared.runtime_shared.active_request_limit;
    let lane_load = runtime_gateway_normalized_load(lane_active, lane_limit)
        .max(runtime_gateway_normalized_load(global_active, global_limit));
    let profile_limit = shared
        .runtime_shared
        .runtime_config
        .tuning
        .profile_inflight_hard_limit;
    let profile_inflight = admission.profile_inflight_snapshot();
    let runtime = shared
        .runtime_shared
        .lock_runtime_state()
        .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let mut snapshot = RuntimeGatewayProviderRuntimeSnapshot::default();
    for provider in registry.provider_ids() {
        let profile = registry.runtime_profile_name(provider);
        let profile_inflight = runtime_profile_inflight_sort_key(profile, &profile_inflight);
        let health_key = runtime_profile_route_health_key(profile, route_kind);
        let health = runtime.profile_health.contains_key(&health_key).then(|| {
            let penalty = runtime_profile_route_health_score(&runtime, profile, now, route_kind);
            prodex_provider_spi::ROUTING_SCORE_SCALE.saturating_sub(
                u16::try_from(penalty)
                    .unwrap_or(u16::MAX)
                    .saturating_mul(1_000)
                    .min(prodex_provider_spi::ROUTING_SCORE_SCALE),
            )
        });
        let quota_headroom = runtime
            .profile_usage_snapshots
            .get(profile)
            .and_then(|usage| {
                [
                    runtime_gateway_quota_window_headroom(
                        usage.five_hour_status,
                        usage.five_hour_remaining_percent,
                        usage.five_hour_reset_at,
                        now,
                    ),
                    runtime_gateway_quota_window_headroom(
                        usage.weekly_status,
                        usage.weekly_remaining_percent,
                        usage.weekly_reset_at,
                        now,
                    ),
                ]
                .into_iter()
                .flatten()
                .min()
            });
        snapshot.insert(
            provider,
            RuntimeGatewayProviderRuntimeSignals {
                health,
                load: lane_load.max(runtime_gateway_normalized_load(
                    profile_inflight,
                    profile_limit,
                )),
                quota_headroom,
                circuit_open: runtime_profile_in_selection_backoff(
                    &runtime, profile, route_kind, now,
                ) || runtime_profile_route_circuit_open_until(
                    &runtime, profile, route_kind, now,
                )
                .is_some(),
                quota_available: quota_headroom != Some(0),
                inflight_cap_reached: profile_limit == 0 || profile_inflight >= profile_limit,
            },
        );
    }
    Ok(snapshot)
}

fn runtime_gateway_execution_approval(
    shared: &RuntimeLocalRewriteProxyShared,
    tenant: TenantContext,
    principal: &Principal,
    captured: &RuntimeProxyRequest,
    route_kind: GatewayHttpRouteKind,
    session: super::local_rewrite_governance_session::RuntimeGatewayGovernanceSessionSnapshot,
    governance: &ApplicationGovernancePlan,
) -> Result<ApplicationExecutionApprovalDecision, RuntimeGatewayApplicationDataPlaneError> {
    let session_id_hash = session
        .session_id_hash()
        .ok_or(RuntimeGatewayApplicationDataPlaneError::GovernanceSessionRequired)?;
    let tools = if runtime_gateway_requested_capabilities(route_kind, captured)
        .contains(ModelCapability::Tools)
    {
        runtime_gateway_requested_tools(&captured.body)
            .ok_or(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?
    } else {
        Vec::new()
    };
    let model = runtime_provider_model_from_body(&captured.body);
    let request_body_digest: [u8; 32] = Sha256::digest(&captured.body).into();
    let fingerprint = ExecutionApprovalBinding {
        tenant_id: tenant.tenant_id,
        principal_id: principal.id,
        session_id_hash: &session_id_hash,
        action: runtime_gateway_governed_action(route_kind),
        model: model.as_deref(),
        tools: &tools,
        request_body_digest: &request_body_digest,
        policy_revision_id: governance.policy.policy_revision,
    }
    .fingerprint()
    .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let approval_id = execution_approval_id(&fingerprint)
        .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let repository =
        super::local_rewrite_gateway_admin_policies::runtime_governance_repository(shared)
            .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let now_unix_ms = runtime_gateway_unix_epoch_millis();
    let create_audit_outbox = runtime_gateway_execution_approval_audit(
        &repository,
        tenant,
        principal,
        &approval_id,
        "gateway.governance.execution_approval.create",
        now_unix_ms,
    )?;
    let consume_audit_outbox = runtime_gateway_execution_approval_audit(
        &repository,
        tenant,
        principal,
        &approval_id,
        "gateway.governance.execution_approval.consume",
        now_unix_ms,
    )?;
    ApplicationExecutionApprovalService::new(&repository)
        .enforce(ApplicationExecutionApprovalRequest {
            tenant_id: tenant.tenant_id,
            principal: principal.clone(),
            policy_effect: governance.policy.effect,
            fingerprint,
            now_unix_ms,
            create_audit_outbox,
            consume_audit_outbox,
        })
        .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)
}

fn runtime_gateway_execution_approval_audit(
    repository: &super::local_rewrite_gateway_admin_policies::RuntimeGovernanceRepository<'_>,
    tenant: TenantContext,
    principal: &Principal,
    approval_id: &ApprovalId,
    action: &'static str,
    occurred_at_unix_ms: u64,
) -> Result<AuditOutboxWriteCommand, RuntimeGatewayApplicationDataPlaneError> {
    let event = AuditEvent::new(
        occurred_at_unix_ms,
        tenant,
        principal,
        AuditAction::new(action),
        AuditResource::new(
            "execution_approval",
            Some(approval_id.as_str().to_string()),
            Some(tenant.tenant_id),
        ),
        AuditOutcome::Success,
        None::<String>,
    );
    let previous_digest = repository
        .latest_audit_digest(tenant.tenant_id)
        .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let event_digest = compute_audit_chain_digest(previous_digest.as_ref(), &event);
    Ok(AuditOutboxWriteCommand {
        outbox_event_id: AuditEventId::new(),
        audit: AppendOnlyAuditCommand {
            storage_key: TenantStorageKey::tenant(tenant.tenant_id),
            event,
            previous_digest,
            event_digest,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn runtime_gateway_mandatory_governance_audit(
    shared: &RuntimeLocalRewriteProxyShared,
    tenant: TenantContext,
    principal: &Principal,
    request_id: RequestId,
    action: &str,
    outcome: AuditOutcome,
    governance: &ApplicationGovernancePlan,
    routing: Option<&GovernedRoutingPlan>,
    reason: &str,
) -> Result<(), RuntimeGatewayApplicationDataPlaneError> {
    if !shared
        .runtime_shared
        .runtime_config
        .governance
        .mandatory_audit
    {
        return Ok(());
    }
    let decision_context = format!(
        "p:{}:r:{}:s:{}:c:{}:v:{}:q:{}:i:{}:e:{}",
        governance.policy.policy_revision,
        routing
            .map(|routing| routing.registry_revision.to_string())
            .unwrap_or_else(|| "none".to_string()),
        routing
            .map(|routing| routing.score_revision.to_string())
            .unwrap_or_else(|| "none".to_string()),
        governance.classification.classification().as_str(),
        routing
            .map(|routing| routing.primary.provider.label())
            .unwrap_or("none"),
        request_id,
        governance.classification.coverage().as_str(),
        match governance.policy.effect {
            PolicyEffect::Allow => "allow",
            PolicyEffect::Deny => "deny",
            PolicyEffect::RequireApproval => "require_approval",
        },
    );
    super::local_rewrite_governance_audit::persist_runtime_governance_decision_audit(
        shared,
        tenant,
        principal,
        action,
        outcome,
        reason,
        &decision_context,
    )
    .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)
}

fn runtime_gateway_governance_error_code(
    error: &RuntimeGatewayApplicationDataPlaneError,
) -> &'static str {
    match error {
        RuntimeGatewayApplicationDataPlaneError::GovernanceDenied => "policy_denied",
        RuntimeGatewayApplicationDataPlaneError::GovernanceApprovalRequired { .. } => {
            "approval_required"
        }
        RuntimeGatewayApplicationDataPlaneError::GovernanceSessionRequired => {
            "approval_session_required"
        }
        RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider => "no_compliant_provider",
        RuntimeGatewayApplicationDataPlaneError::Execution(_)
        | RuntimeGatewayApplicationDataPlaneError::MissingPrincipal
        | RuntimeGatewayApplicationDataPlaneError::RouteUnavailable
        | RuntimeGatewayApplicationDataPlaneError::ProviderRoute(_)
        | RuntimeGatewayApplicationDataPlaneError::TraceContext(_)
        | RuntimeGatewayApplicationDataPlaneError::Admission(_)
        | RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable => {
            "governance_unavailable"
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn runtime_gateway_obligation_execution(
    governance: &ApplicationGovernancePlan,
    inspection: &ApplicationInspectionPlan,
    capabilities: &CapabilitySet,
    route_kind: GatewayHttpRouteKind,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    session: SessionPolicyContext,
    environment: EnvironmentContext,
    governance_mode: prodex_config::GovernanceMode,
) -> prodex_application::ApplicationObligationExecutionPlan {
    let estimated_input_tokens =
        u32::try_from(captured.body.len().saturating_add(3) / 4).unwrap_or(u32::MAX);
    let findings = inspection
        .result
        .findings()
        .iter()
        .map(|finding| finding.kind())
        .collect::<Vec<_>>();
    let tools = runtime_gateway_requested_tools(&captured.body);
    let tool_refs = tools
        .as_ref()
        .map(|tools| tools.iter().map(String::as_str).collect::<Vec<_>>());
    let requested_modalities = runtime_gateway_requested_modalities(route_kind, capabilities);
    let streaming = runtime_gateway_provider_stream_mode(captured) == ProviderStreamMode::Streaming;
    let websocket = captured.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("upgrade") && value.eq_ignore_ascii_case("websocket")
    });
    let response_transport = if websocket {
        ApplicationResponseTransport::WebSocket
    } else if streaming {
        ApplicationResponseTransport::ServerSentEvents
    } else {
        ApplicationResponseTransport::Unary
    };
    let keyword_inspection = !shared.gateway_guardrails.blocked_output_keywords.is_empty();
    let response_inspection_coverage = runtime_gateway_response_inspection_coverage(
        governance_mode,
        websocket,
        streaming,
        keyword_inspection,
        runtime_gateway_buffered_response_is_locally_inspectable(route_kind),
        shared.gateway_guardrail_webhook.enabled_for("post"),
    );
    plan_application_obligation_execution(
        &governance.policy,
        ApplicationObligationContext {
            mode: if governance_mode == prodex_config::GovernanceMode::BankEnforce {
                ApplicationObligationMode::BankEnforce
            } else if governance_mode.is_enforcing() {
                ApplicationObligationMode::Enforce
            } else {
                ApplicationObligationMode::Observe
            },
            classification: governance.classification.classification(),
            inspection_coverage: governance.classification.coverage(),
            detected_findings: &findings,
            masked_findings: &inspection.masked_findings,
            requested_capabilities: capabilities,
            requested_model: runtime_provider_model_from_body(&captured.body).as_deref(),
            requested_tools: capabilities
                .contains(ModelCapability::Tools)
                .then_some(tool_refs.as_deref())
                .flatten(),
            requested_modalities: &requested_modalities,
            estimated_input_tokens,
            estimated_context_tokens: estimated_input_tokens,
            requested_output_tokens: runtime_gateway_requested_output_tokens(&captured.body),
            session,
            environment,
            response_transport,
            response_inspection_coverage,
        },
    )
}

fn runtime_gateway_provider_invocation(
    input: RuntimeGatewayProviderInvocationInput<'_>,
) -> Result<ProviderInvocation, RuntimeGatewayApplicationDataPlaneError> {
    let provider = input
        .routing
        .map(|routing| routing.primary.provider)
        .unwrap_or_else(|| input.shared.provider.bridge_kind().provider_id());
    let endpoint = runtime_gateway_provider_endpoint(input.route)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::RouteUnavailable)?;
    let wire_format = provider_adapter(provider).upstream_request_format();
    let model = runtime_provider_model_from_body(&input.captured.body)
        .unwrap_or_else(|| "unknown".to_string());
    let route = ProviderRoute::new(provider, endpoint, wire_format, model)
        .map_err(RuntimeGatewayApplicationDataPlaneError::ProviderRoute)?;
    Ok(ProviderInvocation {
        tenant: input.tenant,
        principal: input.principal,
        request_id: input.request_id,
        call_id: input.reservation.request.call_id,
        route,
        credential_ref: input
            .routing
            .map(|routing| routing.primary.credential_ref.clone())
            .unwrap_or_else(|| {
                runtime_gateway_provider_credential_ref(
                    input
                        .shared
                        .provider_credential
                        .as_ref()
                        .map(|credential| credential.reference()),
                    provider,
                )
            }),
        stream_mode: runtime_gateway_provider_stream_mode(input.captured),
        estimated_usage: input.reservation.request.estimate,
    })
}

#[cfg(test)]
mod provider_tests;

#[cfg(test)]
#[path = "local_rewrite_application_data_plane/tests.rs"]
mod tests;
