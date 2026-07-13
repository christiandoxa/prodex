use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_router::runtime_gateway_http_request_meta;
use super::local_rewrite_gateway_config::RuntimeGatewayStateStore;
use super::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;
use super::provider_bridge::{RuntimeProviderGatewaySpendEvent, runtime_provider_model_from_body};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use prodex_application::{
    ApplicationAuthorizedRequestContext, ApplicationDataPlaneError, ApplicationDataPlanePlan,
    ApplicationDataPlaneRequest, ApplicationGovernancePlan, ApplicationGovernanceRequest,
    ApplicationInspectionPlan, ApplicationProviderRetryRequest,
    ApplicationUsageReconciliationBackend, ApplicationUsageReconciliationError,
    ApplicationUsageReconciliationExecutionPlan, ApplicationUsageReconciliationExecutionRequest,
    ApplicationUsageReconciliationPlan, ApplicationUsageReconciliationRequest,
    plan_application_data_plane, plan_application_data_plane_execution,
    plan_application_governance, plan_application_provider_retry,
    plan_application_usage_reconciliation, plan_application_usage_reconciliation_execution,
};
use prodex_domain::{
    CanonicalRoute, CapabilitySet, Channel, CredentialScope, DataClassification,
    EnvironmentContext, GovernanceObligation, GovernedAction, ModelCapability, NetworkZone,
    PolicyEffect, PolicySelector, Principal, ProviderTrustTier, QuotaContext, RequestId,
    RequestRisk, ReservationReconciliationReason, ReservationRecord, SecretRef,
    SessionPolicyContext, TenantContext, TenantId, TenantScopedResource, UsageAmount,
};
use prodex_gateway_core::{GatewayAdmissionRequest, GatewayUsageReconciliationRequest};
use prodex_gateway_http::{GatewayHttpExecutionPlan, GatewayHttpPolicy, GatewayHttpRouteKind};
use prodex_observability::{TraceContext, TraceContextError};
use prodex_provider_core::{
    ProviderAdapterContract, ProviderEndpoint, ProviderErrorClass, ProviderId, provider_adapter,
};
use prodex_provider_spi::{
    GovernedProviderDescriptor, GovernedProviderRegistry, GovernedRoutingError,
    GovernedRoutingPlan, GovernedRoutingRequest, GovernedRoutingSignals, GovernedRoutingWeights,
    ProviderInvocation, ProviderRetryCause, ProviderRetryDecision, ProviderRetryPolicy,
    ProviderRetryStage, ProviderRoute, ProviderRouteError, ProviderStreamMode,
    plan_governed_provider_route,
};
use prodex_storage::{
    AtomicReservationCommand, DurableStoreKind, TenantStorageKey, UsageReconciliationCommand,
};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::error::Error;
use std::fmt;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeGatewayTenantResource {
    tenant_id: TenantId,
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
        routing: Option<GovernedRoutingPlan>,
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
        match &self.0 {
            RuntimeGatewayApplicationAdmissionKind::TenantBound { routing, .. } => routing.as_ref(),
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
                    routing: governance.routing,
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
) -> Result<(), RuntimeGatewayApplicationDataPlaneError> {
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
            Ok(())
        };
    };
    let (Some(tenant), Some(principal)) = (authorized.tenant_context(), authorized.principal())
    else {
        return if enforcing {
            Err(RuntimeGatewayApplicationDataPlaneError::MissingPrincipal)
        } else {
            Ok(())
        };
    };
    let captured = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/realtime".to_string(),
        headers: Vec::new(),
        body: text.as_bytes().to_vec(),
    };
    let decision = runtime_gateway_governance_decision(
        tenant,
        principal,
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
    Ok(())
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
}

struct RuntimeGatewayGovernanceDecision {
    plan: ApplicationGovernancePlan,
    routing: Option<GovernedRoutingPlan>,
}

fn runtime_gateway_governance_decision(
    tenant: TenantContext,
    principal: &Principal,
    route_kind: GatewayHttpRouteKind,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    inspection: &ApplicationInspectionPlan,
) -> Result<RuntimeGatewayGovernanceDecision, RuntimeGatewayApplicationDataPlaneError> {
    let snapshot = shared
        .runtime_shared
        .runtime_config
        .governance_snapshot
        .as_ref()
        .ok_or(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
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
    let governance = plan_application_governance(
        snapshot,
        ApplicationGovernanceRequest {
            inspection,
            trusted_label: None,
            untrusted_label: None,
            prior_classification: None,
            session_floor: DataClassification::Public,
            route_floor: DataClassification::Public,
            request_risk_floor: if tools_requested {
                DataClassification::Internal
            } else {
                DataClassification::Public
            },
            tenant,
            principal,
            channel: Channel::Api,
            credential_scope: CredentialScope::DataPlane,
            session: SessionPolicyContext {
                age_seconds: 0,
                idle_seconds: 0,
                revoked: false,
                mfa_satisfied: false,
                retained_classification: DataClassification::Public,
            },
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
    let enforcing = shared
        .runtime_shared
        .runtime_config
        .governance
        .mode
        .is_enforcing();
    if enforcing
        && let Err(error) =
            runtime_gateway_enforce_policy_decision(&governance, &capabilities, captured, shared)
    {
        runtime_gateway_mandatory_governance_audit(
            shared,
            "policy_decision",
            "denied",
            &governance,
            None,
            runtime_gateway_governance_error_code(&error),
        )?;
        return Err(error);
    }
    let registry = runtime_gateway_governed_registry(tenant, shared, &capabilities)?;
    let routing = plan_governed_provider_route(&GovernedRoutingRequest {
        tenant,
        classification: governance.classification.classification(),
        required_capabilities: &capabilities,
        policy: &governance.policy,
        registry: &registry,
        score_revision: shared
            .runtime_shared
            .runtime_config
            .governance_policy
            .routing_score_revision
            .unwrap_or(1),
        weights: GovernedRoutingWeights::default(),
        affinity_provider: None,
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
                "provider_selection",
                "denied",
                &governance,
                None,
                runtime_gateway_governance_error_code(&error),
            )?;
            return Err(error);
        }
    };
    runtime_gateway_mandatory_governance_audit(
        shared,
        "provider_selection",
        "allowed",
        &governance,
        routing.as_ref(),
        "policy_allow",
    )?;
    Ok(RuntimeGatewayGovernanceDecision {
        plan: governance,
        routing,
    })
}

fn runtime_gateway_mandatory_governance_audit(
    shared: &RuntimeLocalRewriteProxyShared,
    action: &str,
    outcome: &str,
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
    let default_log_dir = shared
        .runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    prodex_audit_log::append_audit_event(
        &path,
        "gateway_governance",
        action,
        outcome,
        serde_json::json!({
            "classification": governance.classification.classification().as_str(),
            "inspection_coverage": governance.classification.coverage().as_str(),
            "policy_revision": governance.policy.policy_revision.to_string(),
            "policy_effect": match governance.policy.effect {
                PolicyEffect::Allow => "allow",
                PolicyEffect::Deny => "deny",
                PolicyEffect::RequireApproval => "require_approval",
            },
            "obligation_count": governance.policy.obligations.len(),
            "provider": routing.map(|routing| routing.primary.provider.label()),
            "registry_revision": routing.map(|routing| routing.registry_revision),
            "score_revision": routing.map(|routing| routing.score_revision),
            "reason": reason,
        }),
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

fn runtime_gateway_enforce_policy_decision(
    governance: &ApplicationGovernancePlan,
    capabilities: &CapabilitySet,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<(), RuntimeGatewayApplicationDataPlaneError> {
    if governance.policy.valid_until_unix_ms <= runtime_gateway_unix_epoch_millis() {
        return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable);
    }
    match governance.policy.effect {
        PolicyEffect::Deny => {
            return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceDenied);
        }
        PolicyEffect::RequireApproval => {
            return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceApprovalRequired);
        }
        PolicyEffect::Allow => {}
    }
    if governance
        .policy
        .obligations
        .contains(&GovernanceObligation::RequireHumanApproval)
    {
        return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceApprovalRequired);
    }
    if governance
        .policy
        .obligations
        .contains(&GovernanceObligation::RequireMfa)
        || governance
            .policy
            .obligations
            .contains(&GovernanceObligation::RequireReauthentication)
        || (governance
            .policy
            .obligations
            .contains(&GovernanceObligation::DisableTools)
            && capabilities.contains(ModelCapability::Tools))
    {
        return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceDenied);
    }
    let estimated_input_tokens =
        u32::try_from(captured.body.len().saturating_add(3) / 4).unwrap_or(u32::MAX);
    if governance.policy.obligations.iter().any(
        |obligation| matches!(obligation, GovernanceObligation::MaxInputTokens(limit) if estimated_input_tokens > *limit),
    ) {
        return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceDenied);
    }
    if governance
        .policy
        .obligations
        .contains(&GovernanceObligation::RequireResponseInspection)
    {
        let streaming =
            runtime_gateway_provider_stream_mode(captured) == ProviderStreamMode::Streaming;
        let incremental_inspection = !shared.gateway_guardrails.blocked_output_keywords.is_empty();
        let buffered_inspection = shared.gateway_guardrail_webhook.enabled_for("post");
        if (streaming && !incremental_inspection)
            || (!streaming && !incremental_inspection && !buffered_inspection)
        {
            return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable);
        }
    }
    Ok(())
}

fn runtime_gateway_governed_registry(
    tenant: TenantContext,
    shared: &RuntimeLocalRewriteProxyShared,
    capabilities: &CapabilitySet,
) -> Result<GovernedProviderRegistry, RuntimeGatewayApplicationDataPlaneError> {
    let settings = &shared.runtime_shared.runtime_config.governance_policy;
    let provider_settings = settings.provider.as_ref();
    let provider = shared.provider.bridge_kind().provider_id();
    let regions = provider_settings
        .map(|settings| settings.regions.as_slice())
        .unwrap_or(&[])
        .iter()
        .map(|region| PolicySelector::new(region.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let regions = if regions.is_empty() {
        vec![
            PolicySelector::new("*")
                .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?,
        ]
    } else {
        regions
    };
    let trust_tier = match provider_settings.map(|settings| settings.trust_tier) {
        Some(prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::Enterprise) => {
            ProviderTrustTier::Enterprise
        }
        Some(prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::RestrictedApproved) => {
            ProviderTrustTier::RestrictedApproved
        }
        Some(prodex_runtime_policy::RuntimeGovernanceProviderTrustTier::Standard) | None => {
            ProviderTrustTier::Standard
        }
    };
    let maximum_classification = match provider_settings
        .map(|settings| settings.maximum_classification)
        .unwrap_or(prodex_runtime_policy::RuntimeGovernanceDataClassification::Internal)
    {
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Public => {
            DataClassification::Public
        }
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Internal => {
            DataClassification::Internal
        }
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Confidential => {
            DataClassification::Confidential
        }
        prodex_runtime_policy::RuntimeGovernanceDataClassification::Restricted => {
            DataClassification::Restricted
        }
    };
    Ok(GovernedProviderRegistry {
        revision: settings.provider_registry_revision.unwrap_or(1),
        providers: vec![GovernedProviderDescriptor {
            revision: provider_settings
                .map(|settings| settings.descriptor_revision)
                .unwrap_or(1),
            tenant,
            provider,
            credential_ref: runtime_gateway_provider_credential_ref(
                shared
                    .provider_credential
                    .as_ref()
                    .map(|credential| credential.reference()),
                provider,
            ),
            credential_available: true,
            enabled: provider_settings.is_none_or(|settings| settings.enabled),
            revoked: false,
            circuit_open: false,
            quota_available: true,
            local_execution: provider_settings.is_some_and(|settings| settings.local_execution),
            trust_tier,
            maximum_classification,
            capabilities: capabilities.clone(),
            regions,
            retention_seconds: provider_settings
                .map(|settings| settings.retention_seconds)
                .unwrap_or(u32::MAX),
            training_use: provider_settings.is_none_or(|settings| settings.training_use),
            signals: GovernedRoutingSignals {
                health: 10_000,
                load: 0,
                cost: 5_000,
                latency: 5_000,
                risk: match trust_tier {
                    ProviderTrustTier::Standard => 8_000,
                    ProviderTrustTier::Enterprise => 4_000,
                    ProviderTrustTier::RestrictedApproved => 1_000,
                },
                priority: 5_000,
            },
        }],
    })
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
    let provider = input.shared.provider.bridge_kind().provider_id();
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
        credential_ref: runtime_gateway_provider_credential_ref(
            input
                .shared
                .provider_credential
                .as_ref()
                .map(|credential| credential.reference()),
            provider,
        ),
        stream_mode: runtime_gateway_provider_stream_mode(input.captured),
        estimated_usage: input.reservation.request.estimate,
    })
}

fn runtime_gateway_provider_credential_ref(
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

pub(super) fn runtime_gateway_application_provider_dispatch(
    admission: &RuntimeGatewayApplicationAdmission,
) -> RuntimeGatewayApplicationProviderDispatch<'_> {
    match &admission.0 {
        RuntimeGatewayApplicationAdmissionKind::TenantBound { plan, .. } => {
            RuntimeGatewayApplicationProviderDispatch {
                kind: RuntimeGatewayApplicationProviderDispatchKind::Application(
                    &plan.admission.provider_invocation,
                ),
                inspection: admission.inspection(),
            }
        }
        RuntimeGatewayApplicationAdmissionKind::CompatibilityAnonymous { invocation, .. } => {
            RuntimeGatewayApplicationProviderDispatch {
                kind: RuntimeGatewayApplicationProviderDispatchKind::CompatibilityAnonymous(
                    invocation,
                ),
                inspection: admission.inspection(),
            }
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
