use super::{
    ApplicationDataPlanePlan, ApplicationInspectionPlan, ApplicationProviderRetryRequest,
    ApplicationUsageReconciliationBackend, ApplicationUsageReconciliationError,
    ApplicationUsageReconciliationExecutionPlan, ApplicationUsageReconciliationExecutionRequest,
    ApplicationUsageReconciliationPlan, ApplicationUsageReconciliationRequest, DurableStoreKind,
    Error, GatewayUsageReconciliationRequest, GovernedRoutingPlan, PolicyEffect, ProviderEndpoint,
    ProviderErrorClass, ProviderId, ProviderInvocation, ProviderRetryCause, ProviderRetryDecision,
    ProviderRetryPolicy, ProviderRetryStage, ProviderStreamMode, RequestId,
    ReservationReconciliationReason, ReservationRecord, RuntimeGatewayApplicationAdmission,
    RuntimeGatewayApplicationAdmissionKind, RuntimeGatewayApplicationDataPlaneError,
    RuntimeGatewayCompatibilityProviderInvocation, RuntimeGatewayStateStore,
    RuntimeLocalRewriteProxyShared, RuntimeProviderGatewaySpendEvent, TenantContext,
    TenantStorageKey, TraceContextError, UsageAmount, UsageReconciliationCommand, fmt,
    plan_application_provider_retry, plan_application_usage_reconciliation,
    plan_application_usage_reconciliation_execution, runtime_gateway_application_trace_context,
};

pub(in crate::runtime_launch::proxy_startup) struct RuntimeGatewayApplicationProviderDispatch<'a> {
    kind: RuntimeGatewayApplicationProviderDispatchKind<'a>,
    inspection: &'a ApplicationInspectionPlan,
    execution:
        Option<super::super::local_rewrite_provider_registry::RuntimeGatewayProviderExecution>,
    pricing: Option<super::super::local_rewrite_provider_registry::RuntimeGatewayProviderPricing>,
    provider_override: Option<ProviderId>,
}

enum RuntimeGatewayApplicationProviderDispatchKind<'a> {
    Application(&'a ProviderInvocation),
    CompatibilityAnonymous(&'a RuntimeGatewayCompatibilityProviderInvocation),
}

impl RuntimeGatewayApplicationProviderDispatch<'_> {
    pub(in crate::runtime_launch::proxy_startup) fn provider(&self) -> ProviderId {
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

    pub(in crate::runtime_launch::proxy_startup) fn endpoint(&self) -> ProviderEndpoint {
        match &self.kind {
            RuntimeGatewayApplicationProviderDispatchKind::Application(invocation) => {
                invocation.route.endpoint
            }
            RuntimeGatewayApplicationProviderDispatchKind::CompatibilityAnonymous(invocation) => {
                invocation.endpoint
            }
        }
    }

    pub(in crate::runtime_launch::proxy_startup) fn stream_mode(&self) -> ProviderStreamMode {
        match &self.kind {
            RuntimeGatewayApplicationProviderDispatchKind::Application(invocation) => {
                invocation.stream_mode
            }
            RuntimeGatewayApplicationProviderDispatchKind::CompatibilityAnonymous(invocation) => {
                invocation.stream_mode
            }
        }
    }

    pub(in crate::runtime_launch::proxy_startup) fn inspection(
        &self,
    ) -> &ApplicationInspectionPlan {
        self.inspection
    }

    pub(in crate::runtime_launch::proxy_startup) fn selected_shared(
        &self,
        shared: &RuntimeLocalRewriteProxyShared,
    ) -> RuntimeLocalRewriteProxyShared {
        let selected = if let Some(execution) = self.execution.as_ref() {
            shared.with_selected_upstream(
                execution.provider.clone(),
                execution.credential.clone(),
                execution.upstream_base_url.clone(),
            )
        } else {
            shared.clone()
        };
        selected.with_governed_pricing(self.pricing.clone())
    }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_application_provider_dispatch<
    'a,
>(
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

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_application_provider_dispatch_attempt<
    'a,
>(
    admission: &'a RuntimeGatewayApplicationAdmission,
    shared: &RuntimeLocalRewriteProxyShared,
    attempt_index: usize,
) -> Result<RuntimeGatewayApplicationProviderDispatch<'a>, RuntimeGatewayApplicationDataPlaneError>
{
    match &admission.0 {
        RuntimeGatewayApplicationAdmissionKind::TenantBound { plan, routing, .. } => {
            runtime_gateway_application_tenant_provider_dispatch_attempt(
                admission,
                plan,
                routing,
                shared,
                attempt_index,
            )
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
                pricing: None,
                provider_override: None,
            })
        }
    }
}

fn runtime_gateway_application_tenant_provider_dispatch_attempt<'a>(
    admission: &'a RuntimeGatewayApplicationAdmission,
    plan: &'a ApplicationDataPlanePlan,
    routing: &'a Option<Box<GovernedRoutingPlan>>,
    shared: &RuntimeLocalRewriteProxyShared,
    attempt_index: usize,
) -> Result<RuntimeGatewayApplicationProviderDispatch<'a>, RuntimeGatewayApplicationDataPlaneError>
{
    let (execution, pricing, provider_override) = runtime_gateway_application_route_execution(
        &plan.admission.provider_invocation,
        routing,
        shared,
        attempt_index,
    )?;
    if shared
        .runtime_shared
        .runtime_config
        .governance
        .mode
        .is_enforcing()
        && (!shared
            .runtime_shared
            .runtime_config
            .governance
            .mandatory_audit
            || plan.governance.policy.effect != PolicyEffect::Allow
            || routing.is_none())
    {
        return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable);
    }
    Ok(RuntimeGatewayApplicationProviderDispatch {
        kind: RuntimeGatewayApplicationProviderDispatchKind::Application(
            &plan.admission.provider_invocation,
        ),
        inspection: admission.inspection(),
        execution,
        pricing,
        provider_override,
    })
}

type RuntimeGatewayApplicationRouteExecution = (
    Option<super::super::local_rewrite_provider_registry::RuntimeGatewayProviderExecution>,
    Option<super::super::local_rewrite_provider_registry::RuntimeGatewayProviderPricing>,
    Option<ProviderId>,
);

fn runtime_gateway_application_route_execution(
    invocation: &ProviderInvocation,
    routing: &Option<Box<GovernedRoutingPlan>>,
    shared: &RuntimeLocalRewriteProxyShared,
    attempt_index: usize,
) -> Result<RuntimeGatewayApplicationRouteExecution, RuntimeGatewayApplicationDataPlaneError> {
    let Some(routing) = routing.as_deref() else {
        return Ok((None, None, None));
    };
    let selected_route = std::iter::once(&routing.primary)
        .chain(routing.fallbacks.iter())
        .nth(attempt_index)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider)?;
    let snapshots = shared
        .governance
        .snapshot_for(routing.tenant.tenant_id)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider)?;
    let provider_registry = snapshots.provider_registry;
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
        || snapshots.routing_scores.revision != routing.score_revision
    {
        return Err(RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider);
    }
    let execution = if selected_route.provider == shared.provider.bridge_kind().provider_id() {
        None
    } else {
        Some(
            provider_registry
                .execution_for_route(selected_route, invocation.route.endpoint)
                .ok_or(RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider)?,
        )
    };
    Ok((
        execution,
        provider_registry.pricing_for_route(selected_route, invocation.route.endpoint),
        Some(selected_route.provider),
    ))
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_application_provider_retry_precommit(
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

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_application_provider_stage_is_committed(
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

pub(in crate::runtime_launch::proxy_startup) struct RuntimeGatewayApplicationReconciliationInput<'a>
{
    pub(in crate::runtime_launch::proxy_startup) state_store: &'a RuntimeGatewayStateStore,
    pub(in crate::runtime_launch::proxy_startup) storage_key: TenantStorageKey,
    pub(in crate::runtime_launch::proxy_startup) record: ReservationRecord,
    pub(in crate::runtime_launch::proxy_startup) actual: UsageAmount,
    pub(in crate::runtime_launch::proxy_startup) event: &'a RuntimeProviderGatewaySpendEvent,
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_application_reconciliation_execution(
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

pub(in crate::runtime_launch::proxy_startup) struct RuntimeGatewayApplicationReconciliationPlan {
    pub(in crate::runtime_launch::proxy_startup) application: ApplicationUsageReconciliationPlan,
    pub(in crate::runtime_launch::proxy_startup) command: UsageReconciliationCommand,
}

#[derive(Debug)]
pub(in crate::runtime_launch::proxy_startup) enum RuntimeGatewayApplicationReconciliationError {
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

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_application_usage_reconciliation(
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
