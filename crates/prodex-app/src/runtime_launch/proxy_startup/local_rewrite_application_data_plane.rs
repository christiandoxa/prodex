use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_router::runtime_gateway_http_request_meta;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use super::local_rewrite_provider_registry::{
    RuntimeGatewayGovernedProviderRegistrySnapshot, RuntimeGatewayProviderRuntimeSignals,
    RuntimeGatewayProviderRuntimeSnapshot,
};
use super::provider_bridge::{RuntimeProviderGatewaySpendEvent, runtime_provider_model_from_body};
use crate::{
    RuntimeProxyRequest, RuntimeRouteKind, runtime_profile_in_selection_backoff,
    runtime_profile_inflight_count, runtime_profile_route_circuit_open_until,
    runtime_profile_route_health_key, runtime_profile_route_health_score, runtime_proxy_log,
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
    EnvironmentContext, ExecutionApprovalBinding, GovernedAction, InspectionCoverage,
    ModelCapability, NetworkZone, PolicyEffect, Principal, PrincipalPolicyAttributes, QuotaContext,
    RequestId, RequestPolicyAttributes, RequestRisk, ReservationReconciliationReason,
    ReservationRecord, SecretRef, SessionPolicyContext, TenantContext, TenantId,
    TenantScopedResource, UsageAmount, compute_audit_chain_digest, execution_approval_id,
};
use prodex_gateway_core::{GatewayAdmissionRequest, GatewayUsageReconciliationRequest};
use prodex_gateway_http::{GatewayHttpExecutionPlan, GatewayHttpPolicy, GatewayHttpRouteKind};
use prodex_observability::{TraceContext, TraceContextError};
use prodex_provider_core::{
    ProviderAdapterContract, ProviderCapabilityStatus, ProviderEndpoint, ProviderErrorClass,
    ProviderId, provider_adapter, provider_catalog_entries_for,
};
use prodex_provider_spi::{
    GovernedRoutingError, GovernedRoutingPlan, GovernedRoutingRequest,
    MAX_GOVERNED_ROUTING_FALLBACKS, ProviderInvocation, ProviderRetryCause, ProviderRetryDecision,
    ProviderRetryPolicy, ProviderRetryStage, ProviderRoute, ProviderRouteError, ProviderStreamMode,
    plan_governed_provider_route,
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

mod governance_decision;
use governance_decision::runtime_gateway_governance_decision;

const MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeGatewayTenantResource {
    tenant_id: TenantId,
}

pub(super) fn runtime_gateway_provider_capability_is_executable(
    status: ProviderCapabilityStatus,
) -> bool {
    !matches!(
        status,
        ProviderCapabilityStatus::Unsupported | ProviderCapabilityStatus::Untested
    )
}

pub(super) fn runtime_gateway_provider_executable_capabilities(
    provider: ProviderId,
) -> CapabilitySet {
    let adapter = provider_adapter(provider);
    let response_executable = runtime_gateway_provider_capability_is_executable(
        adapter.capability_status(ProviderEndpoint::Responses),
    );
    let compact_executable = runtime_gateway_provider_capability_is_executable(
        adapter.capability_status(ProviderEndpoint::ResponsesCompact),
    );
    let image_executable = runtime_gateway_provider_capability_is_executable(
        adapter.capability_status(ProviderEndpoint::Images),
    );
    let catalog = provider_catalog_entries_for(provider);
    let mut capabilities = Vec::new();
    if response_executable {
        capabilities.push(ModelCapability::ResponsesApi);
    }
    if response_executable && adapter.supports_streaming() {
        capabilities.push(ModelCapability::Streaming);
    }
    if compact_executable {
        capabilities.push(ModelCapability::RemoteCompact);
    }
    if image_executable || catalog.iter().any(|entry| entry.feature_flags.vision) {
        capabilities.push(ModelCapability::Vision);
    }
    if catalog.iter().any(|entry| entry.feature_flags.tools) {
        capabilities.push(ModelCapability::Tools);
    }
    if catalog.iter().any(|entry| entry.feature_flags.json_schema) {
        capabilities.push(ModelCapability::JsonMode);
    }
    if provider == ProviderId::Gemini {
        capabilities.push(ModelCapability::WebSocket);
    }
    CapabilitySet::new(capabilities)
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

    /// Compatibility is owned here until intentionally open anonymous data-plane access is
    /// removed; callers cannot forge provider routing fields outside this adapter.
    pub(super) fn compatibility_anonymous(
        route: GatewayHttpRouteKind,
        captured: &RuntimeProxyRequest,
        shared: &RuntimeLocalRewriteProxyShared,
        inspection: ApplicationInspectionPlan,
    ) -> Result<Self, RuntimeGatewayApplicationDataPlaneError> {
        runtime_gateway_compatibility_provider_invocation(
            shared.provider.bridge_kind().provider_id(),
            route,
            captured,
        )
        .map(|invocation| {
            Self(
                RuntimeGatewayApplicationAdmissionKind::CompatibilityAnonymous {
                    invocation,
                    inspection,
                },
            )
        })
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

fn runtime_gateway_route_kind(route: GatewayHttpRouteKind) -> RuntimeRouteKind {
    match route {
        GatewayHttpRouteKind::DataPlaneResponses => RuntimeRouteKind::Responses,
        GatewayHttpRouteKind::DataPlaneWebSocket => RuntimeRouteKind::Websocket,
        GatewayHttpRouteKind::DataPlaneCompact => RuntimeRouteKind::Compact,
        _ => RuntimeRouteKind::Standard,
    }
}

fn runtime_gateway_normalized_load(active: usize, limit: usize) -> u16 {
    if limit == 0 {
        return prodex_provider_spi::ROUTING_SCORE_SCALE;
    }
    active
        .saturating_mul(usize::from(prodex_provider_spi::ROUTING_SCORE_SCALE))
        .checked_div(limit)
        .unwrap_or(usize::from(prodex_provider_spi::ROUTING_SCORE_SCALE))
        .min(usize::from(prodex_provider_spi::ROUTING_SCORE_SCALE)) as u16
}

fn runtime_gateway_quota_window_headroom(
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> Option<u16> {
    match status {
        RuntimeQuotaWindowStatus::Ready
        | RuntimeQuotaWindowStatus::Thin
        | RuntimeQuotaWindowStatus::Critical => Some(
            remaining_percent.clamp(0, 100) as u16
                * (prodex_provider_spi::ROUTING_SCORE_SCALE / 100),
        ),
        RuntimeQuotaWindowStatus::Exhausted if reset_at > now => Some(0),
        RuntimeQuotaWindowStatus::Exhausted | RuntimeQuotaWindowStatus::Unknown => None,
    }
}

fn runtime_gateway_provider_runtime_snapshot(
    shared: &RuntimeLocalRewriteProxyShared,
    registry: &RuntimeGatewayGovernedProviderRegistrySnapshot,
    route: GatewayHttpRouteKind,
) -> Result<RuntimeGatewayProviderRuntimeSnapshot, RuntimeGatewayApplicationDataPlaneError> {
    let now = (runtime_gateway_unix_epoch_millis() / 1_000) as i64;
    let route_kind = runtime_gateway_route_kind(route);
    let lane_active = shared
        .runtime_shared
        .lane_admission
        .active_counter(route_kind)
        .load(Ordering::Relaxed);
    let lane_limit = shared.runtime_shared.lane_admission.limit(route_kind);
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
    let runtime = shared
        .runtime_shared
        .lock_runtime_state()
        .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let mut snapshot = RuntimeGatewayProviderRuntimeSnapshot::default();
    for provider in registry.provider_ids() {
        let profile = registry.runtime_profile_name(provider);
        let profile_inflight = runtime_profile_inflight_count(&runtime, profile);
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
    let response_inspection_coverage = if websocket && keyword_inspection {
        InspectionCoverage::Partial
    } else if websocket {
        InspectionCoverage::Unsupported
    } else if !streaming
        && (keyword_inspection
            || shared.gateway_guardrail_webhook.enabled_for("post")
            || runtime_gateway_buffered_response_is_locally_inspectable(captured))
    {
        InspectionCoverage::Full
    } else if keyword_inspection {
        InspectionCoverage::Partial
    } else {
        InspectionCoverage::Unsupported
    };
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

fn runtime_gateway_buffered_response_is_locally_inspectable(
    captured: &RuntimeProxyRequest,
) -> bool {
    let path = runtime_proxy_crate::path_without_query(&captured.path_and_query);
    !path.contains("/images/") && !path.contains("/audio/")
}

fn runtime_gateway_requested_modalities(
    route: GatewayHttpRouteKind,
    capabilities: &CapabilitySet,
) -> Vec<DataModality> {
    let mut modalities = match route {
        GatewayHttpRouteKind::DataPlaneImagesGenerations => {
            vec![DataModality::Text, DataModality::Image]
        }
        GatewayHttpRouteKind::DataPlaneImagesEdits
        | GatewayHttpRouteKind::DataPlaneImagesVariations => {
            vec![DataModality::Text, DataModality::Image, DataModality::File]
        }
        GatewayHttpRouteKind::DataPlaneAudioSpeech => {
            vec![DataModality::Text, DataModality::Audio]
        }
        GatewayHttpRouteKind::DataPlaneAudioTranscriptions
        | GatewayHttpRouteKind::DataPlaneAudioTranslations => {
            vec![DataModality::Text, DataModality::Audio, DataModality::File]
        }
        GatewayHttpRouteKind::DataPlaneResponses
        | GatewayHttpRouteKind::DataPlaneCompact
        | GatewayHttpRouteKind::DataPlaneWebSocket
        | GatewayHttpRouteKind::DataPlaneQuota
        | GatewayHttpRouteKind::DataPlaneChatCompletions
        | GatewayHttpRouteKind::DataPlaneEmbeddings
        | GatewayHttpRouteKind::DataPlaneBatches
        | GatewayHttpRouteKind::DataPlaneBatch
        | GatewayHttpRouteKind::DataPlaneRerank
        | GatewayHttpRouteKind::DataPlaneA2a
        | GatewayHttpRouteKind::DataPlaneMessages
        | GatewayHttpRouteKind::DataPlaneModels
        | GatewayHttpRouteKind::DataPlaneModel
        | GatewayHttpRouteKind::ControlPlane
        | GatewayHttpRouteKind::HealthLive
        | GatewayHttpRouteKind::HealthReady
        | GatewayHttpRouteKind::HealthStartup
        | GatewayHttpRouteKind::Unknown => vec![DataModality::Text],
    };
    if capabilities.contains(ModelCapability::Vision) && !modalities.contains(&DataModality::Image)
    {
        modalities.push(DataModality::Image);
    }
    modalities.sort();
    modalities.dedup();
    modalities
}

fn runtime_gateway_requested_tools(body: &[u8]) -> Option<Vec<String>> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    let tools = value.get("tools")?.as_array()?;
    if tools.len() > MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS {
        return None;
    }
    tools
        .iter()
        .map(|tool| {
            tool.get("name")
                .or_else(|| {
                    tool.get("function")
                        .and_then(|function| function.get("name"))
                })
                .or_else(|| tool.get("type"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

fn runtime_gateway_requested_output_tokens(body: &[u8]) -> Option<u32> {
    let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
    ["max_output_tokens", "max_completion_tokens", "max_tokens"]
        .into_iter()
        .find_map(|field| value.get(field)?.as_u64()?.try_into().ok())
}

fn runtime_gateway_governance_route(
    route: GatewayHttpRouteKind,
) -> Result<CanonicalRoute, RuntimeGatewayApplicationDataPlaneError> {
    CanonicalRoute::new(match route {
        GatewayHttpRouteKind::DataPlaneResponses => "responses",
        GatewayHttpRouteKind::DataPlaneCompact => "responses/compact",
        GatewayHttpRouteKind::DataPlaneWebSocket => "responses/websocket",
        GatewayHttpRouteKind::DataPlaneChatCompletions => "chat/completions",
        GatewayHttpRouteKind::DataPlaneEmbeddings => "embeddings",
        GatewayHttpRouteKind::DataPlaneImagesGenerations => "images/generations",
        GatewayHttpRouteKind::DataPlaneImagesEdits => "images/edits",
        GatewayHttpRouteKind::DataPlaneImagesVariations => "images/variations",
        GatewayHttpRouteKind::DataPlaneAudioSpeech => "audio/speech",
        GatewayHttpRouteKind::DataPlaneAudioTranscriptions => "audio/transcriptions",
        GatewayHttpRouteKind::DataPlaneAudioTranslations => "audio/translations",
        GatewayHttpRouteKind::DataPlaneBatches => "batches",
        GatewayHttpRouteKind::DataPlaneBatch => "batch",
        GatewayHttpRouteKind::DataPlaneRerank => "rerank",
        GatewayHttpRouteKind::DataPlaneA2a => "a2a",
        GatewayHttpRouteKind::DataPlaneMessages => "messages",
        GatewayHttpRouteKind::DataPlaneModels => "models",
        GatewayHttpRouteKind::DataPlaneModel => "model",
        _ => return Err(RuntimeGatewayApplicationDataPlaneError::RouteUnavailable),
    })
    .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)
}

fn runtime_gateway_governed_action(route: GatewayHttpRouteKind) -> GovernedAction {
    match route {
        GatewayHttpRouteKind::DataPlaneCompact => GovernedAction::CompactContext,
        GatewayHttpRouteKind::DataPlaneImagesEdits
        | GatewayHttpRouteKind::DataPlaneImagesVariations
        | GatewayHttpRouteKind::DataPlaneAudioTranscriptions
        | GatewayHttpRouteKind::DataPlaneAudioTranslations => GovernedAction::UploadContent,
        _ => GovernedAction::InvokeModel,
    }
}

fn runtime_gateway_requested_capabilities(
    route: GatewayHttpRouteKind,
    captured: &RuntimeProxyRequest,
) -> CapabilitySet {
    let body = serde_json::from_slice::<serde_json::Value>(&captured.body).ok();
    let tools = body.as_ref().is_some_and(|body| {
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|tools| !tools.is_empty())
    });
    let mut capabilities = Vec::new();
    if matches!(
        route,
        GatewayHttpRouteKind::DataPlaneResponses
            | GatewayHttpRouteKind::DataPlaneCompact
            | GatewayHttpRouteKind::DataPlaneWebSocket
    ) {
        capabilities.push(ModelCapability::ResponsesApi);
    }
    if route == GatewayHttpRouteKind::DataPlaneCompact {
        capabilities.push(ModelCapability::RemoteCompact);
    }
    if runtime_gateway_provider_stream_mode(captured) == ProviderStreamMode::Streaming {
        capabilities.push(ModelCapability::Streaming);
    }
    if tools {
        capabilities.push(ModelCapability::Tools);
    }
    if matches!(
        route,
        GatewayHttpRouteKind::DataPlaneImagesGenerations
            | GatewayHttpRouteKind::DataPlaneImagesEdits
            | GatewayHttpRouteKind::DataPlaneImagesVariations
    ) {
        capabilities.push(ModelCapability::Vision);
    }
    if route == GatewayHttpRouteKind::DataPlaneWebSocket {
        capabilities.push(ModelCapability::WebSocket);
    }
    CapabilitySet::new(capabilities)
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

pub(super) fn runtime_gateway_provider_credential_ref(
    configured: Option<&SecretRef>,
    provider: ProviderId,
) -> SecretRef {
    configured
        .cloned()
        .unwrap_or_else(|| SecretRef::new("runtime-provider", provider.label(), None::<String>))
}

#[cfg(test)]
mod provider_tests;

fn runtime_gateway_provider_endpoint(route: GatewayHttpRouteKind) -> Option<ProviderEndpoint> {
    match route {
        GatewayHttpRouteKind::DataPlaneResponses | GatewayHttpRouteKind::DataPlaneWebSocket => {
            Some(ProviderEndpoint::Responses)
        }
        GatewayHttpRouteKind::DataPlaneCompact => Some(ProviderEndpoint::ResponsesCompact),
        GatewayHttpRouteKind::DataPlaneChatCompletions => Some(ProviderEndpoint::ChatCompletions),
        GatewayHttpRouteKind::DataPlaneMessages => Some(ProviderEndpoint::Messages),
        GatewayHttpRouteKind::DataPlaneEmbeddings => Some(ProviderEndpoint::Embeddings),
        GatewayHttpRouteKind::DataPlaneModels | GatewayHttpRouteKind::DataPlaneModel => {
            Some(ProviderEndpoint::Models)
        }
        GatewayHttpRouteKind::DataPlaneImagesGenerations
        | GatewayHttpRouteKind::DataPlaneImagesEdits
        | GatewayHttpRouteKind::DataPlaneImagesVariations => Some(ProviderEndpoint::Images),
        GatewayHttpRouteKind::DataPlaneAudioSpeech
        | GatewayHttpRouteKind::DataPlaneAudioTranscriptions
        | GatewayHttpRouteKind::DataPlaneAudioTranslations => Some(ProviderEndpoint::Audio),
        GatewayHttpRouteKind::DataPlaneBatches | GatewayHttpRouteKind::DataPlaneBatch => {
            Some(ProviderEndpoint::Batches)
        }
        GatewayHttpRouteKind::DataPlaneRerank => Some(ProviderEndpoint::Rerank),
        GatewayHttpRouteKind::DataPlaneA2a => Some(ProviderEndpoint::A2a),
        GatewayHttpRouteKind::DataPlaneQuota
        | GatewayHttpRouteKind::ControlPlane
        | GatewayHttpRouteKind::HealthLive
        | GatewayHttpRouteKind::HealthReady
        | GatewayHttpRouteKind::HealthStartup
        | GatewayHttpRouteKind::Unknown => None,
    }
}

fn runtime_gateway_provider_stream_mode(captured: &RuntimeProxyRequest) -> ProviderStreamMode {
    let streaming = serde_json::from_slice::<serde_json::Value>(&captured.body)
        .ok()
        .and_then(|body| body.get("stream").and_then(serde_json::Value::as_bool))
        .unwrap_or(false);
    if streaming {
        ProviderStreamMode::Streaming
    } else {
        ProviderStreamMode::Unary
    }
}

fn runtime_gateway_application_trace_context(
    request_id: RequestId,
) -> Result<TraceContext, TraceContextError> {
    let trace_id = request_id.as_uuid().simple().to_string();
    TraceContext::new(&trace_id, &trace_id[..16], "01")
}

pub(super) struct RuntimeGatewayApplicationProviderDispatch<'a> {
    kind: RuntimeGatewayApplicationProviderDispatchKind<'a>,
    inspection: &'a ApplicationInspectionPlan,
    execution: Option<super::local_rewrite_provider_registry::RuntimeGatewayProviderExecution>,
    provider_override: Option<ProviderId>,
}

enum RuntimeGatewayApplicationProviderDispatchKind<'a> {
    Application(&'a ProviderInvocation),
    CompatibilityAnonymous(&'a RuntimeGatewayCompatibilityProviderInvocation),
}

impl RuntimeGatewayApplicationProviderDispatch<'_> {
    pub(super) fn provider(&self) -> ProviderId {
        if let Some(provider) = self.provider_override {
            return provider;
        }
        match &self.kind {
            RuntimeGatewayApplicationProviderDispatchKind::Application(invocation) => {
                invocation.route.provider
            }
            RuntimeGatewayApplicationProviderDispatchKind::CompatibilityAnonymous(invocation) => {
                invocation.provider
            }
        }
    }

    pub(super) fn endpoint(&self) -> ProviderEndpoint {
        match &self.kind {
            RuntimeGatewayApplicationProviderDispatchKind::Application(invocation) => {
                invocation.route.endpoint
            }
            RuntimeGatewayApplicationProviderDispatchKind::CompatibilityAnonymous(invocation) => {
                invocation.endpoint
            }
        }
    }

    pub(super) fn stream_mode(&self) -> ProviderStreamMode {
        match &self.kind {
            RuntimeGatewayApplicationProviderDispatchKind::Application(invocation) => {
                invocation.stream_mode
            }
            RuntimeGatewayApplicationProviderDispatchKind::CompatibilityAnonymous(invocation) => {
                invocation.stream_mode
            }
        }
    }

    pub(super) fn inspection(&self) -> &ApplicationInspectionPlan {
        self.inspection
    }

    pub(super) fn selected_shared(
        &self,
        shared: &RuntimeLocalRewriteProxyShared,
    ) -> RuntimeLocalRewriteProxyShared {
        let Some(execution) = self.execution.as_ref() else {
            return shared.clone();
        };
        shared.with_selected_upstream(
            execution.provider.clone(),
            execution.credential.clone(),
            execution.upstream_base_url.clone(),
        )
    }
}

pub(super) fn runtime_gateway_application_provider_dispatch<'a>(
    admission: &'a RuntimeGatewayApplicationAdmission,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeGatewayApplicationProviderDispatch<'a>, RuntimeGatewayApplicationDataPlaneError>
{
    match &admission.0 {
        RuntimeGatewayApplicationAdmissionKind::TenantBound { plan, routing, .. } => {
            let invocation = &plan.admission.provider_invocation;
            debug_assert!(
                routing
                    .as_ref()
                    .is_none_or(|routing| routing.primary.provider == invocation.route.provider)
            );
            runtime_gateway_application_provider_dispatch_attempt(admission, shared, 0)
        }
        RuntimeGatewayApplicationAdmissionKind::CompatibilityAnonymous { .. } => {
            runtime_gateway_application_provider_dispatch_attempt(admission, shared, 0)
        }
    }
}

pub(super) fn runtime_gateway_application_provider_dispatch_attempt<'a>(
    admission: &'a RuntimeGatewayApplicationAdmission,
    shared: &RuntimeLocalRewriteProxyShared,
    attempt_index: usize,
) -> Result<RuntimeGatewayApplicationProviderDispatch<'a>, RuntimeGatewayApplicationDataPlaneError>
{
    match &admission.0 {
        RuntimeGatewayApplicationAdmissionKind::TenantBound { plan, routing, .. } => {
            let invocation = &plan.admission.provider_invocation;
            let mut execution = None;
            let mut provider_override = None;
            if let Some(routing) = routing.as_ref() {
                let selected_route = std::iter::once(&routing.primary)
                    .chain(routing.fallbacks.iter())
                    .nth(attempt_index)
                    .ok_or(RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider)?;
                let provider_registry = shared.governed_provider_registry.load_full();
                let routing_scores = shared.governed_routing_scores.load_full();
                let provider_registry = provider_registry
                    .snapshot_for(routing.tenant.tenant_id)
                    .ok_or(RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider)?;
                let route_matches = if attempt_index == 0 {
                    provider_registry.matches_route(routing, invocation.route.endpoint)
                } else {
                    provider_registry.matches_governed_route(
                        routing.registry_revision,
                        selected_route,
                        invocation.route.endpoint,
                    )
                };
                if invocation.route.provider != routing.primary.provider
                    || !route_matches
                    || !routing_scores
                        .snapshot_for(routing.tenant.tenant_id)
                        .is_some_and(|snapshot| snapshot.revision == routing.score_revision)
                {
                    return Err(RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider);
                }
                if selected_route.provider != shared.provider.bridge_kind().provider_id() {
                    execution = Some(
                        provider_registry
                            .execution_for_route(selected_route, invocation.route.endpoint)
                            .ok_or(RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider)?,
                    );
                }
                provider_override = Some(selected_route.provider);
            }
            if shared
                .runtime_shared
                .runtime_config
                .governance
                .mode
                .is_enforcing()
            {
                if !shared
                    .runtime_shared
                    .runtime_config
                    .governance
                    .mandatory_audit
                    || plan.governance.policy.effect != PolicyEffect::Allow
                {
                    return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable);
                }
                if routing.is_none() {
                    return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable);
                }
            }
            Ok(RuntimeGatewayApplicationProviderDispatch {
                kind: RuntimeGatewayApplicationProviderDispatchKind::Application(
                    &plan.admission.provider_invocation,
                ),
                inspection: admission.inspection(),
                execution,
                provider_override,
            })
        }
        RuntimeGatewayApplicationAdmissionKind::CompatibilityAnonymous { invocation, .. } => {
            if !shared
                .runtime_shared
                .runtime_config
                .governance
                .mode
                .allows_anonymous_compatibility()
            {
                return Err(RuntimeGatewayApplicationDataPlaneError::MissingPrincipal);
            }
            Ok(RuntimeGatewayApplicationProviderDispatch {
                kind: RuntimeGatewayApplicationProviderDispatchKind::CompatibilityAnonymous(
                    invocation,
                ),
                inspection: admission.inspection(),
                execution: None,
                provider_override: None,
            })
        }
    }
}

pub(super) fn runtime_gateway_application_provider_retry_precommit(
    cause: ProviderRetryCause,
    error_class: ProviderErrorClass,
    attempt_index: usize,
    candidate_count: usize,
) -> bool {
    if candidate_count == 0 || attempt_index >= candidate_count {
        return false;
    }
    let policy = ProviderRetryPolicy::bounded(
        u8::try_from(candidate_count.saturating_sub(1)).unwrap_or(u8::MAX),
    );
    let plan = plan_application_provider_retry(ApplicationProviderRetryRequest {
        policy,
        stage: ProviderRetryStage::BeforeFirstByte,
        cause,
        error_class,
        attempted_precommit_retries: u8::try_from(attempt_index).unwrap_or(u8::MAX),
    });
    plan.retry.decision == ProviderRetryDecision::Allowed
}

pub(super) fn runtime_gateway_application_provider_stage_is_committed(
    stage: ProviderRetryStage,
) -> bool {
    debug_assert!(matches!(
        stage,
        ProviderRetryStage::AfterFirstByte | ProviderRetryStage::AfterCancellation
    ));
    // Provider adapters retain their heterogeneous precommit attempt budgets. The application
    // planner is authoritative here only after the response is irreversible, where retry denial
    // is independent of the adapter-specific attempt count.
    let plan = plan_application_provider_retry(ApplicationProviderRetryRequest {
        policy: ProviderRetryPolicy::single_retry(),
        stage,
        cause: ProviderRetryCause::NextModel,
        error_class: ProviderErrorClass::Transient,
        attempted_precommit_retries: 0,
    });
    plan.retry.decision == ProviderRetryDecision::DeniedCommitted
}

pub(super) struct RuntimeGatewayApplicationReconciliationInput<'a> {
    pub(super) state_store: &'a RuntimeGatewayStateStore,
    pub(super) storage_key: TenantStorageKey,
    pub(super) record: ReservationRecord,
    pub(super) actual: UsageAmount,
    pub(super) event: &'a RuntimeProviderGatewaySpendEvent,
}

pub(super) fn runtime_gateway_application_reconciliation_execution(
    state_store: &RuntimeGatewayStateStore,
    event: &RuntimeProviderGatewaySpendEvent,
) -> ApplicationUsageReconciliationExecutionPlan {
    let backend = match state_store {
        RuntimeGatewayStateStore::File { .. } => ApplicationUsageReconciliationBackend::File,
        RuntimeGatewayStateStore::Sqlite { .. } => ApplicationUsageReconciliationBackend::Sqlite,
        RuntimeGatewayStateStore::Postgres { .. } => {
            ApplicationUsageReconciliationBackend::Postgres
        }
        RuntimeGatewayStateStore::Redis { .. } => ApplicationUsageReconciliationBackend::Redis,
    };
    plan_application_usage_reconciliation_execution(
        ApplicationUsageReconciliationExecutionRequest {
            backend,
            reason: event
                .reconciliation_reason
                .unwrap_or(ReservationReconciliationReason::Completed),
        },
    )
}

pub(super) struct RuntimeGatewayApplicationReconciliationPlan {
    pub(super) application: ApplicationUsageReconciliationPlan,
    pub(super) command: UsageReconciliationCommand,
}

#[derive(Debug)]
pub(super) enum RuntimeGatewayApplicationReconciliationError {
    UnsupportedStore,
    InvalidRequestId,
    TraceContext(TraceContextError),
    Application(ApplicationUsageReconciliationError),
}

impl fmt::Display for RuntimeGatewayApplicationReconciliationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application(error) => error.fmt(f),
            Self::TraceContext(error) => error.fmt(f),
            Self::UnsupportedStore | Self::InvalidRequestId => {
                write!(f, "application usage reconciliation is unavailable")
            }
        }
    }
}

impl Error for RuntimeGatewayApplicationReconciliationError {}

pub(super) fn runtime_gateway_application_usage_reconciliation(
    input: RuntimeGatewayApplicationReconciliationInput<'_>,
) -> Result<RuntimeGatewayApplicationReconciliationPlan, RuntimeGatewayApplicationReconciliationError>
{
    let durable_store = match input.state_store {
        RuntimeGatewayStateStore::Sqlite { .. } => DurableStoreKind::Sqlite,
        RuntimeGatewayStateStore::Postgres { .. } => DurableStoreKind::Postgres,
        RuntimeGatewayStateStore::File { .. } | RuntimeGatewayStateStore::Redis { .. } => {
            return Err(RuntimeGatewayApplicationReconciliationError::UnsupportedStore);
        }
    };
    let request_id = input
        .event
        .request_id
        .strip_prefix("prodex-")
        .unwrap_or(&input.event.request_id)
        .parse::<RequestId>()
        .map_err(|_| RuntimeGatewayApplicationReconciliationError::InvalidRequestId)?;
    let command = UsageReconciliationCommand {
        storage_key: input.storage_key,
        snapshot: prodex_domain::BudgetSnapshot {
            reserved: input.record.reserved,
            committed: UsageAmount::ZERO,
        },
        record: input.record,
        actual: input.actual,
        reason: input
            .event
            .reconciliation_reason
            .unwrap_or(ReservationReconciliationReason::Completed),
    };
    let request = ApplicationUsageReconciliationRequest {
        durable_store,
        gateway: GatewayUsageReconciliationRequest {
            tenant: TenantContext {
                tenant_id: input.record.tenant_id,
            },
            request_id,
            reconciliation: command.clone(),
            trace_context: runtime_gateway_application_trace_context(request_id)
                .map_err(RuntimeGatewayApplicationReconciliationError::TraceContext)?,
        },
    };
    let application = plan_application_usage_reconciliation(request)
        .map_err(RuntimeGatewayApplicationReconciliationError::Application)?;
    Ok(RuntimeGatewayApplicationReconciliationPlan {
        application,
        command,
    })
}

#[cfg(test)]
mod tests {
    use super::super::local_rewrite::RuntimeLocalRewriteProviderOptions;
    use super::super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
    use super::super::provider_bridge::RuntimeProviderGatewaySpendEvent;
    use super::{
        MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS, RuntimeGatewayApplicationReconciliationInput,
        runtime_gateway_application_provider_stage_is_committed,
        runtime_gateway_application_reconciliation_execution,
        runtime_gateway_application_usage_reconciliation, runtime_gateway_provider_endpoint,
        runtime_gateway_provider_executable_capabilities, runtime_gateway_requested_modalities,
        runtime_gateway_requested_tools,
    };
    use prodex_domain::{
        CallId, CapabilitySet, DataClassification, DataModality, ModelCapability, PolicyEffect,
        RequestId, ReservationId, ReservationReconciliationReason, ReservationRecord,
        ReservationRequest, TenantContext, TenantId, UsageAmount,
    };
    use prodex_gateway_http::GatewayHttpRouteKind;
    use prodex_provider_core::{ProviderEndpoint, ProviderId};
    use prodex_provider_spi::{
        GovernedRoutingError, GovernedRoutingRequest, GovernedRoutingWeights, ProviderRetryStage,
        plan_governed_provider_route,
    };
    use prodex_storage::TenantStorageKey;

    #[test]
    fn provider_retry_boundary_marks_irreversible_stages_committed() {
        assert!(runtime_gateway_application_provider_stage_is_committed(
            ProviderRetryStage::AfterFirstByte
        ));
        assert!(runtime_gateway_application_provider_stage_is_committed(
            ProviderRetryStage::AfterCancellation
        ));
    }

    #[test]
    fn data_plane_governance_authority_is_memory_only() {
        let parent = include_str!("local_rewrite_application_data_plane.rs");
        let decision = include_str!("local_rewrite_application_data_plane/governance_decision.rs");
        let hot_path = format!(
            "{}\n{}",
            parent.split("\n#[cfg(test)]\nmod tests").next().unwrap(),
            decision.split("\n#[cfg(test)]\nmod tests").next().unwrap(),
        );
        assert!(hot_path.contains("governance_snapshot"));
        assert!(hot_path.contains(".load_full()"));
        assert!(!hot_path.contains("GovernanceSqliteRepository"));
        assert!(!hot_path.contains("governance_load_snapshot"));
        assert!(!hot_path.contains(".load_snapshot("));
    }

    #[test]
    fn provider_route_mapping_covers_forwarded_data_plane_routes() {
        for route in [
            GatewayHttpRouteKind::DataPlaneResponses,
            GatewayHttpRouteKind::DataPlaneCompact,
            GatewayHttpRouteKind::DataPlaneChatCompletions,
            GatewayHttpRouteKind::DataPlaneMessages,
            GatewayHttpRouteKind::DataPlaneEmbeddings,
            GatewayHttpRouteKind::DataPlaneImagesGenerations,
            GatewayHttpRouteKind::DataPlaneAudioSpeech,
            GatewayHttpRouteKind::DataPlaneBatches,
            GatewayHttpRouteKind::DataPlaneRerank,
            GatewayHttpRouteKind::DataPlaneA2a,
        ] {
            assert!(runtime_gateway_provider_endpoint(route).is_some());
        }
    }

    #[test]
    fn route_modalities_mark_audio_and_file_payloads_explicitly() {
        let capabilities = CapabilitySet::new(Vec::new());
        assert_eq!(
            runtime_gateway_requested_modalities(
                GatewayHttpRouteKind::DataPlaneAudioSpeech,
                &capabilities,
            ),
            vec![DataModality::Text, DataModality::Audio]
        );
        for route in [
            GatewayHttpRouteKind::DataPlaneAudioTranscriptions,
            GatewayHttpRouteKind::DataPlaneAudioTranslations,
        ] {
            assert_eq!(
                runtime_gateway_requested_modalities(route, &capabilities),
                vec![DataModality::Text, DataModality::Audio, DataModality::File]
            );
        }
        for route in [
            GatewayHttpRouteKind::DataPlaneImagesEdits,
            GatewayHttpRouteKind::DataPlaneImagesVariations,
        ] {
            assert_eq!(
                runtime_gateway_requested_modalities(route, &capabilities),
                vec![DataModality::Text, DataModality::Image, DataModality::File]
            );
        }
    }

    #[test]
    fn requested_tool_metadata_is_bounded() {
        let body = |count| {
            serde_json::to_vec(&serde_json::json!({
                "tools": (0..count)
                    .map(|index| serde_json::json!({"name": format!("tool-{index}")}))
                    .collect::<Vec<_>>()
            }))
            .unwrap()
        };
        assert_eq!(
            runtime_gateway_requested_tools(&body(MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS))
                .unwrap()
                .len(),
            MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS
        );
        assert!(
            runtime_gateway_requested_tools(&body(MAX_RUNTIME_GATEWAY_REQUESTED_TOOLS + 1))
                .is_none()
        );
    }

    #[test]
    fn governed_registry_advertises_only_executable_adapter_capabilities() {
        let anthropic = runtime_gateway_provider_executable_capabilities(ProviderId::Anthropic);
        assert!(!anthropic.contains(ModelCapability::RemoteCompact));
        assert!(!anthropic.contains(ModelCapability::Vision));
        assert!(!anthropic.contains(ModelCapability::WebSocket));

        let gemini = runtime_gateway_provider_executable_capabilities(ProviderId::Gemini);
        assert!(gemini.contains(ModelCapability::RemoteCompact));
        assert!(gemini.contains(ModelCapability::WebSocket));

        let options = RuntimeLocalRewriteProviderOptions::OpenAiResponses {
            api_keys: vec!["test-key".to_string()],
        };
        let snapshot = super::super::local_rewrite_provider_registry::runtime_gateway_bootstrap_provider_registry_snapshot(
            &prodex_runtime_policy::RuntimePolicyGovernanceSettings::default(),
            &options,
            None,
        )
        .unwrap();
        let tenant = TenantContext {
            tenant_id: TenantId::new(),
        };
        let responses =
            snapshot.for_tenant(tenant, ProviderEndpoint::Responses, &Default::default());
        assert!(responses.providers[0].enabled);
        assert!(
            !snapshot
                .for_tenant(
                    tenant,
                    ProviderEndpoint::ResponsesCompact,
                    &Default::default(),
                )
                .providers[0]
                .enabled
        );
        let required = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
        let policy = prodex_domain::PolicyDecision {
            effect: PolicyEffect::Allow,
            obligations: Vec::new(),
            reason_codes: Vec::new(),
            policy_revision: prodex_domain::PolicyRevisionId::new(),
            valid_until_unix_ms: u64::MAX,
        };
        let routing = plan_governed_provider_route(&GovernedRoutingRequest {
            tenant,
            classification: DataClassification::Internal,
            required_capabilities: &required,
            policy: &policy,
            registry: &responses,
            score_revision: 1,
            weights: GovernedRoutingWeights::default(),
            affinity_provider: None,
            max_fallbacks: 0,
        })
        .unwrap();
        assert!(snapshot.matches_route(&routing, ProviderEndpoint::Responses));
        let mut stale = routing;
        stale.registry_revision += 1;
        assert!(!snapshot.matches_route(&stale, ProviderEndpoint::Responses));
    }

    #[test]
    fn explicit_provider_revocation_overrides_continuation_affinity() {
        let settings = prodex_runtime_policy::RuntimePolicyGovernanceSettings {
            provider_registry_revision: Some(7),
            provider: Some(
                prodex_runtime_policy::RuntimePolicyGovernanceProviderSettings {
                    descriptor_revision: 9,
                    enabled: true,
                    revoked: true,
                    trust_tier: prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::Standard,
                    local_execution: false,
                    maximum_classification:
                        prodex_runtime_policy::RuntimeGovernanceDataClassification::Internal,
                    regions: vec!["*".to_string()],
                    retention_seconds: 0,
                    training_use: false,
                },
            ),
            ..Default::default()
        };
        let snapshot = super::super::local_rewrite_provider_registry::runtime_gateway_bootstrap_provider_registry_snapshot(
            &settings,
            &RuntimeLocalRewriteProviderOptions::OpenAiResponses {
                api_keys: vec!["test-key".to_string()],
            },
            None,
        )
        .unwrap();
        let tenant = TenantContext {
            tenant_id: TenantId::new(),
        };
        let registry =
            snapshot.for_tenant(tenant, ProviderEndpoint::Responses, &Default::default());
        let required = CapabilitySet::new(vec![ModelCapability::ResponsesApi]);
        let policy = prodex_domain::PolicyDecision {
            effect: PolicyEffect::Allow,
            obligations: Vec::new(),
            reason_codes: Vec::new(),
            policy_revision: prodex_domain::PolicyRevisionId::new(),
            valid_until_unix_ms: u64::MAX,
        };

        assert_eq!(
            plan_governed_provider_route(&GovernedRoutingRequest {
                tenant,
                classification: DataClassification::Internal,
                required_capabilities: &required,
                policy: &policy,
                registry: &registry,
                score_revision: 1,
                weights: GovernedRoutingWeights::default(),
                affinity_provider: Some(ProviderId::OpenAi),
                max_fallbacks: 0,
            }),
            Err(GovernedRoutingError::NoEligibleProvider)
        );
    }

    #[test]
    fn application_reconciliation_preserves_cancelled_and_partial_stream_usage() {
        let tenant_id = TenantId::new();
        let request = ReservationRequest {
            tenant_id,
            call_id: CallId::new(),
            reservation_id: ReservationId::new(),
            estimate: UsageAmount::new(100, 1_000),
        };
        let record = ReservationRecord::from_request(request, 1_000, 60_000).unwrap();
        let state_store = RuntimeGatewayStateStore::sqlite("/tmp/prodex-test.sqlite".into());

        for reason in [
            ReservationReconciliationReason::Cancelled,
            ReservationReconciliationReason::StreamInterrupted,
        ] {
            let event = RuntimeProviderGatewaySpendEvent {
                event: "gateway_spend",
                phase: "response",
                request: 7,
                key_name: Some("test-key".to_string()),
                tenant_id: Some(tenant_id.to_string()),
                request_id: format!("prodex-{}", RequestId::new()),
                legacy_request_sequence: 7,
                call_id: format!("prodex-{}", request.call_id),
                provider: "openai".to_string(),
                path: "/v1/responses".to_string(),
                model: "gpt-5.4".to_string(),
                status: 200,
                elapsed_ms: 1,
                request_bytes: 10,
                response_bytes: Some(5),
                input_tokens: Some(3),
                output_tokens: Some(2),
                cost_usd: None,
                reconciliation_reason: Some(reason),
                sink: "runtime-log".to_string(),
            };
            let plan = runtime_gateway_application_usage_reconciliation(
                RuntimeGatewayApplicationReconciliationInput {
                    state_store: &state_store,
                    storage_key: TenantStorageKey::tenant(tenant_id),
                    record,
                    actual: UsageAmount::new(5, 50),
                    event: &event,
                },
            )
            .unwrap();
            let execution =
                runtime_gateway_application_reconciliation_execution(&state_store, &event);
            let audit = execution
                .audit(prodex_application::ApplicationUsageReconciliationAuditOutcome::Success);

            assert_eq!(
                plan.application
                    .gateway
                    .reconciliation
                    .reconciliation
                    .reason,
                reason,
            );
            assert_eq!(
                plan.application
                    .gateway
                    .reconciliation
                    .reconciliation
                    .commit
                    .actual,
                UsageAmount::new(5, 50),
            );
            assert_eq!(
                plan.application
                    .gateway
                    .reconciliation
                    .reconciliation
                    .released_event
                    .expect("partial usage releases the unconsumed reservation")
                    .amount,
                UsageAmount::new(95, 950),
            );
            assert_eq!(audit.backend(), "sqlite");
            assert_eq!(
                audit.reason(),
                match reason {
                    ReservationReconciliationReason::Cancelled => "cancelled",
                    ReservationReconciliationReason::StreamInterrupted => "stream_interrupted",
                    ReservationReconciliationReason::Completed => "completed",
                }
            );
        }
    }
}
