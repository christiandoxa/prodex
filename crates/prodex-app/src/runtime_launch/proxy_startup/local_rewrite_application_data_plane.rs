#[cfg(test)]
use super::local_rewrite::RuntimeLocalRewriteProviderOptions;
use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_router::runtime_gateway_http_request_meta;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use super::provider_bridge::{RuntimeProviderGatewaySpendEvent, runtime_provider_model_from_body};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use prodex_application::{
    ApplicationAuthorizedRequestContext, ApplicationDataPlaneError, ApplicationDataPlanePlan,
    ApplicationDataPlaneRequest, ApplicationGovernancePlan, ApplicationGovernanceRequest,
    ApplicationGovernanceSnapshot, ApplicationInspectionPlan, ApplicationObligationContext,
    ApplicationObligationDisposition, ApplicationObligationExecutionPlan,
    ApplicationObligationMode, ApplicationProviderRetryRequest, ApplicationResponseObligationPlan,
    ApplicationResponseTransport, ApplicationUsageReconciliationBackend,
    ApplicationUsageReconciliationError, ApplicationUsageReconciliationExecutionPlan,
    ApplicationUsageReconciliationExecutionRequest, ApplicationUsageReconciliationPlan,
    ApplicationUsageReconciliationRequest, plan_application_data_plane,
    plan_application_data_plane_execution, plan_application_governance,
    plan_application_obligation_execution, plan_application_provider_retry,
    plan_application_usage_reconciliation, plan_application_usage_reconciliation_execution,
};
use prodex_domain::{
    AuditOutcome, CanonicalRoute, CapabilitySet, Channel, CredentialScope, DataClassification,
    DataModality, EnvironmentContext, GovernedAction, InspectionCoverage, ModelCapability,
    NetworkZone, PolicyEffect, Principal, QuotaContext, RequestId, RequestRisk,
    ReservationReconciliationReason, ReservationRecord, SecretRef, SessionPolicyContext,
    TenantContext, TenantId, TenantScopedResource, UsageAmount,
};
use prodex_gateway_core::{GatewayAdmissionRequest, GatewayUsageReconciliationRequest};
use prodex_gateway_http::{GatewayHttpExecutionPlan, GatewayHttpPolicy, GatewayHttpRouteKind};
use prodex_observability::{TraceContext, TraceContextError};
use prodex_provider_core::{
    ProviderAdapterContract, ProviderCapabilityStatus, ProviderEndpoint, ProviderErrorClass,
    ProviderId, provider_adapter, provider_catalog_entries_for,
};
#[cfg(test)]
use prodex_provider_spi::GovernedRoutingWeights;
use prodex_provider_spi::{
    GovernedRoutingError, GovernedRoutingPlan, GovernedRoutingRequest, ProviderInvocation,
    ProviderRetryCause, ProviderRetryDecision, ProviderRetryPolicy, ProviderRetryStage,
    ProviderRoute, ProviderRouteError, ProviderStreamMode, plan_governed_provider_route,
};
use prodex_storage::{
    AtomicReservationCommand, DurableStoreKind, TenantStorageKey, UsageReconciliationCommand,
};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::error::Error;
use std::fmt;

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
    GovernanceApprovalRequired,
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
            | Self::GovernanceApprovalRequired
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
    reservation: AtomicReservationCommand,
    inspection: ApplicationInspectionPlan,
) -> Result<RuntimeGatewayApplicationAdmission, RuntimeGatewayApplicationDataPlaneError> {
    let Some(tenant) = authorized.tenant_context() else {
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
        tenant,
        &principal,
        request_id,
        authorized.request().route(),
        captured,
        shared,
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
            principal,
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
    inspection: &ApplicationInspectionPlan,
) -> Result<Option<ApplicationResponseObligationPlan>, RuntimeGatewayApplicationDataPlaneError> {
    let enforcing = shared
        .runtime_shared
        .runtime_config
        .governance
        .mode
        .is_enforcing();
    let Some(authorized) = authorized else {
        return if enforcing {
            Err(RuntimeGatewayApplicationDataPlaneError::MissingPrincipal)
        } else {
            Ok(None)
        };
    };
    let (Some(tenant), Some(principal)) = (authorized.tenant_context(), authorized.principal())
    else {
        return if enforcing {
            Err(RuntimeGatewayApplicationDataPlaneError::MissingPrincipal)
        } else {
            Ok(None)
        };
    };
    let captured = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/realtime".to_string(),
        headers: vec![("upgrade".to_string(), "websocket".to_string())],
        body: text.as_bytes().to_vec(),
    };
    let decision = runtime_gateway_governance_decision(
        tenant,
        principal,
        authorized.request().request_id(),
        GatewayHttpRouteKind::DataPlaneWebSocket,
        &captured,
        shared,
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

fn runtime_gateway_governance_decision(
    tenant: TenantContext,
    principal: &Principal,
    request_id: RequestId,
    route_kind: GatewayHttpRouteKind,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    inspection: &ApplicationInspectionPlan,
) -> Result<RuntimeGatewayGovernanceDecision, RuntimeGatewayApplicationDataPlaneError> {
    let snapshot = shared
        .governance_snapshot
        .load_full()
        .snapshot_for(tenant.tenant_id)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let classification = shared
        .classification_rules
        .load_full()
        .snapshot_for(tenant.tenant_id)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let governance_config = snapshot.config;
    let application_snapshot = ApplicationGovernanceSnapshot {
        classification_rules: classification.classification_rules().clone(),
        policy: snapshot.application.policy.clone(),
    };
    let route = runtime_gateway_governance_route(route_kind)?;
    let capabilities = runtime_gateway_requested_capabilities(route_kind, captured);
    let tools_requested = capabilities.contains(ModelCapability::Tools);
    let request_risk = if tools_requested {
        RequestRisk::High
    } else if inspection.result.coverage() != prodex_domain::InspectionCoverage::Full {
        RequestRisk::Elevated
    } else {
        RequestRisk::Low
    };
    let environment = EnvironmentContext {
        network_zone: NetworkZone::Unknown,
        authentication_strength: 1,
        mfa_satisfied: false,
    };
    let channel = Channel::Api;
    let now_seconds = runtime_gateway_unix_epoch_millis() / 1_000;
    let session_snapshot =
        shared
            .governance_sessions
            .snapshot(captured, tenant, principal, channel, now_seconds);
    let session = session_snapshot.policy;
    let governance = plan_application_governance(
        &application_snapshot,
        ApplicationGovernanceRequest {
            inspection,
            trusted_label: None,
            untrusted_label: None,
            prior_classification: Some(session.retained_classification),
            session_floor: session.retained_classification,
            route_floor: DataClassification::Public,
            request_risk_floor: if tools_requested {
                DataClassification::Internal
            } else {
                DataClassification::Public
            },
            tenant,
            principal,
            channel,
            credential_scope: CredentialScope::DataPlane,
            session,
            action: runtime_gateway_governed_action(route_kind),
            route: &route,
            request_risk,
            requested_capabilities: &capabilities,
            quota: QuotaContext {
                has_headroom: true,
                reservation_required: true,
            },
            environment,
        },
    )
    .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let enforcing = governance_config.mode.is_enforcing();
    let session_violation = shared
        .governance_sessions
        .configured_violation(
            session_snapshot,
            tenant,
            principal,
            now_seconds,
            governance_config.session,
        )
        .or_else(|| {
            session_snapshot
                .policy_revision_mismatch(governance.policy.policy_revision)
                .then_some("session_policy_revision_mismatch")
        });
    if enforcing && let Some(reason) = session_violation {
        runtime_gateway_mandatory_governance_audit(
            shared,
            tenant,
            principal,
            request_id,
            "session_admission",
            AuditOutcome::Denied,
            &governance,
            None,
            reason,
        )?;
        return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceDenied);
    }
    let obligation_execution = runtime_gateway_obligation_execution(
        &governance,
        inspection,
        &capabilities,
        captured,
        shared,
        session,
        environment,
        governance_config.mode,
    );
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_obligation_execution",
            [
                runtime_proxy_log_field(
                    "disposition",
                    match obligation_execution.disposition {
                        ApplicationObligationDisposition::Proceed => "proceed",
                        ApplicationObligationDisposition::Reject => "reject",
                    },
                ),
                runtime_proxy_log_field(
                    "violation",
                    obligation_execution
                        .violations
                        .first()
                        .map(|violation| violation.code())
                        .unwrap_or("none"),
                ),
                runtime_proxy_log_field(
                    "response_inspection",
                    obligation_execution.response.inspection_coverage.as_str(),
                ),
            ],
        ),
    );
    if enforcing && governance.policy.valid_until_unix_ms <= runtime_gateway_unix_epoch_millis() {
        runtime_gateway_mandatory_governance_audit(
            shared,
            tenant,
            principal,
            request_id,
            "policy_decision",
            AuditOutcome::Denied,
            &governance,
            None,
            "policy_expired",
        )?;
        return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable);
    }
    if enforcing && obligation_execution.disposition == ApplicationObligationDisposition::Reject {
        let reason = obligation_execution
            .violations
            .first()
            .map(|violation| violation.code())
            .unwrap_or("policy_denied");
        let error = if reason == "approval_required" {
            RuntimeGatewayApplicationDataPlaneError::GovernanceApprovalRequired
        } else {
            RuntimeGatewayApplicationDataPlaneError::GovernanceDenied
        };
        runtime_gateway_mandatory_governance_audit(
            shared,
            tenant,
            principal,
            request_id,
            "policy_decision",
            AuditOutcome::Denied,
            &governance,
            None,
            reason,
        )?;
        return Err(error);
    }
    let endpoint = runtime_gateway_provider_endpoint(route_kind)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::RouteUnavailable)?;
    let provider_registry = shared
        .governed_provider_registry
        .load_full()
        .snapshot_for(tenant.tenant_id)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let routing_scores = shared
        .governed_routing_scores
        .load_full()
        .snapshot_for(tenant.tenant_id)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let registry = provider_registry.for_tenant(tenant, endpoint);
    let routing = plan_governed_provider_route(&GovernedRoutingRequest {
        tenant,
        classification: governance.classification.classification(),
        required_capabilities: &capabilities,
        policy: &governance.policy,
        registry: &registry,
        score_revision: routing_scores.revision,
        weights: routing_scores.weights,
        affinity_provider: session_snapshot.affinity_provider,
        max_fallbacks: 0,
    });
    let routing = match routing {
        Ok(plan) => Some(plan),
        Err(_error) if !enforcing => None,
        Err(error) => {
            let error = match error {
                GovernedRoutingError::PolicyDenied => {
                    RuntimeGatewayApplicationDataPlaneError::GovernanceDenied
                }
                GovernedRoutingError::ApprovalRequired => {
                    RuntimeGatewayApplicationDataPlaneError::GovernanceApprovalRequired
                }
                GovernedRoutingError::NoEligibleProvider => {
                    RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider
                }
                _ => RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable,
            };
            runtime_gateway_mandatory_governance_audit(
                shared,
                tenant,
                principal,
                request_id,
                "provider_selection",
                AuditOutcome::Denied,
                &governance,
                None,
                runtime_gateway_governance_error_code(&error),
            )?;
            return Err(error);
        }
    };
    if enforcing
        && let Some(routing) = routing.as_ref()
        && session_snapshot.provider_revision_mismatch(
            routing.registry_revision,
            routing.primary.descriptor_revision,
        )
    {
        runtime_gateway_mandatory_governance_audit(
            shared,
            tenant,
            principal,
            request_id,
            "session_admission",
            AuditOutcome::Denied,
            &governance,
            Some(routing),
            "session_provider_revision_mismatch",
        )?;
        return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceDenied);
    }
    let selected_provider = routing
        .as_ref()
        .map(|routing| routing.primary.provider)
        .unwrap_or_else(|| shared.provider.bridge_kind().provider_id());
    if let Err(error) = shared.governance_sessions.remember(
        session_snapshot,
        tenant,
        principal,
        channel,
        now_seconds,
        governance.classification.classification(),
        governance.policy.policy_revision,
        routing
            .as_ref()
            .map(|routing| routing.registry_revision)
            .unwrap_or_default(),
        routing
            .as_ref()
            .map(|routing| routing.primary.descriptor_revision)
            .unwrap_or_default(),
        selected_provider,
        governance_config.session,
    ) {
        let (reason, error) = match error {
            super::local_rewrite_governance_session::RuntimeGatewayGovernanceSessionPersistError::ConcurrentLimitReached => (
                "session_concurrency_limit",
                RuntimeGatewayApplicationDataPlaneError::GovernanceDenied,
            ),
            super::local_rewrite_governance_session::RuntimeGatewayGovernanceSessionPersistError::Unavailable => (
                "session_store_unavailable",
                RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable,
            ),
        };
        runtime_gateway_mandatory_governance_audit(
            shared,
            tenant,
            principal,
            request_id,
            "session_admission",
            AuditOutcome::Denied,
            &governance,
            routing.as_ref(),
            reason,
        )?;
        return Err(error);
    }
    runtime_gateway_mandatory_governance_audit(
        shared,
        tenant,
        principal,
        request_id,
        "provider_selection",
        AuditOutcome::Success,
        &governance,
        routing.as_ref(),
        "policy_allow",
    )?;
    Ok(RuntimeGatewayGovernanceDecision {
        plan: governance,
        routing,
        obligations: obligation_execution,
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
        RuntimeGatewayApplicationDataPlaneError::GovernanceApprovalRequired => "approval_required",
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
    let mut requested_modalities = vec![DataModality::Text];
    if capabilities.contains(ModelCapability::Vision) {
        requested_modalities.push(DataModality::Image);
    }
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
mod provider_credential_tests {
    use super::*;

    #[test]
    fn configured_provider_reference_reaches_application_invocation() {
        let configured = SecretRef::new("external", "provider-key", Some("v2"));

        let selected =
            runtime_gateway_provider_credential_ref(Some(&configured), ProviderId::OpenAi);

        assert_eq!(selected, configured);
        assert_ne!(selected.provider(), "runtime-provider");
    }
}

#[cfg(test)]
mod provider_dispatch_tests {
    use super::*;

    fn captured(stream: bool) -> RuntimeProxyRequest {
        RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/v1/responses".to_string(),
            headers: Vec::new(),
            body: serde_json::to_vec(&serde_json::json!({ "stream": stream })).unwrap(),
        }
    }

    #[test]
    fn anonymous_compatibility_owns_typed_provider_dispatch() {
        let invocation = runtime_gateway_compatibility_provider_invocation(
            ProviderId::Gemini,
            GatewayHttpRouteKind::DataPlaneResponses,
            &captured(true),
        )
        .unwrap();
        assert_eq!(invocation.provider, ProviderId::Gemini);
        assert_eq!(invocation.endpoint, ProviderEndpoint::Responses);
        assert_eq!(invocation.stream_mode, ProviderStreamMode::Streaming);

        assert!(
            runtime_gateway_compatibility_provider_invocation(
                ProviderId::Gemini,
                GatewayHttpRouteKind::Unknown,
                &captured(false),
            )
            .is_err()
        );
    }

    #[test]
    fn application_retry_uses_nonzero_bounded_candidate_counts() {
        for (attempt_index, candidate_count, expected) in [
            (0, 0, false),
            (0, 1, false),
            (0, 2, true),
            (1, 2, false),
            (2, 2, false),
            (254, 257, true),
            (255, 257, false),
        ] {
            assert_eq!(
                runtime_gateway_application_provider_retry_precommit(
                    ProviderRetryCause::NextModel,
                    ProviderErrorClass::Transient,
                    attempt_index,
                    candidate_count,
                ),
                expected,
                "attempt_index={attempt_index} candidate_count={candidate_count}",
            );
        }
        assert!(!runtime_gateway_application_provider_retry_precommit(
            ProviderRetryCause::RotateCredential,
            ProviderErrorClass::NotFound,
            0,
            2,
        ));
    }
}

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
}

enum RuntimeGatewayApplicationProviderDispatchKind<'a> {
    Application(&'a ProviderInvocation),
    CompatibilityAnonymous(&'a RuntimeGatewayCompatibilityProviderInvocation),
}

impl RuntimeGatewayApplicationProviderDispatch<'_> {
    pub(super) fn provider(&self) -> ProviderId {
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
}

pub(super) fn runtime_gateway_application_provider_dispatch<'a>(
    admission: &'a RuntimeGatewayApplicationAdmission,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<RuntimeGatewayApplicationProviderDispatch<'a>, RuntimeGatewayApplicationDataPlaneError>
{
    match &admission.0 {
        RuntimeGatewayApplicationAdmissionKind::TenantBound { plan, routing, .. } => {
            let invocation = &plan.admission.provider_invocation;
            if let Some(routing) = routing.as_ref() {
                let provider_registry = shared.governed_provider_registry.load_full();
                let routing_scores = shared.governed_routing_scores.load_full();
                if invocation.route.provider != routing.primary.provider
                    || routing.primary.provider != shared.provider.bridge_kind().provider_id()
                    || !provider_registry
                        .snapshot_for(routing.tenant.tenant_id)
                        .is_some_and(|snapshot| {
                            snapshot.matches_route(routing, invocation.route.endpoint)
                        })
                    || !routing_scores
                        .snapshot_for(routing.tenant.tenant_id)
                        .is_some_and(|snapshot| snapshot.revision == routing.score_revision)
                {
                    return Err(RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider);
                }
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
            })
        }
        RuntimeGatewayApplicationAdmissionKind::CompatibilityAnonymous { invocation, .. } => {
            if shared
                .runtime_shared
                .runtime_config
                .governance
                .mode
                .is_enforcing()
            {
                return Err(RuntimeGatewayApplicationDataPlaneError::MissingPrincipal);
            }
            Ok(RuntimeGatewayApplicationProviderDispatch {
                kind: RuntimeGatewayApplicationProviderDispatchKind::CompatibilityAnonymous(
                    invocation,
                ),
                inspection: admission.inspection(),
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
    use super::*;
    use prodex_domain::{
        CallId, ReservationId, ReservationReconciliationReason, ReservationRequest,
    };

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
        let source = include_str!("local_rewrite_application_data_plane.rs");
        let hot_path = source.split("\n#[cfg(test)]\nmod tests").next().unwrap();
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
        let responses = snapshot.for_tenant(tenant, ProviderEndpoint::Responses);
        assert!(responses.providers[0].enabled);
        assert!(
            !snapshot
                .for_tenant(tenant, ProviderEndpoint::ResponsesCompact)
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
        let registry = snapshot.for_tenant(tenant, ProviderEndpoint::Responses);
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
