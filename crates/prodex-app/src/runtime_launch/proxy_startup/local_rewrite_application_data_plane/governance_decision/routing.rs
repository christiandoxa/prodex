use super::session::emit_mandatory_governance_audit;
use super::{
    ApplicationGovernancePlan, AuditOutcome, GovernanceDecisionContext, GovernedRoutingError,
    GovernedRoutingPlan, GovernedRoutingRequest, MAX_GOVERNED_ROUTING_FALLBACKS, PolicyEffect,
    RuntimeGatewayApplicationDataPlaneError, RuntimeLocalRewriteProxyShared,
    plan_governed_provider_route, runtime_gateway_governance_error_code,
    runtime_gateway_provider_endpoint, runtime_gateway_provider_runtime_snapshot,
};

pub(super) fn plan_provider_route(
    shared: &RuntimeLocalRewriteProxyShared,
    context: &GovernanceDecisionContext<'_>,
    governance: &ApplicationGovernancePlan,
    approval_authorized: bool,
    enforcing: bool,
) -> Result<Option<GovernedRoutingPlan>, RuntimeGatewayApplicationDataPlaneError> {
    let endpoint = runtime_gateway_provider_endpoint(context.route_kind)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::RouteUnavailable)?;
    let provider_registry = shared
        .governed_provider_registry
        .load_full()
        .snapshot_for(context.tenant.tenant_id)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let routing_scores = shared
        .governed_routing_scores
        .load_full()
        .snapshot_for(context.tenant.tenant_id)
        .ok_or(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let runtime_snapshot =
        runtime_gateway_provider_runtime_snapshot(shared, &provider_registry, context.route_kind)?;
    let registry = provider_registry.for_tenant(context.tenant, endpoint, &runtime_snapshot);
    let mut routing_policy = governance.policy.clone();
    if approval_authorized {
        routing_policy.effect = PolicyEffect::Allow;
    }
    match plan_governed_provider_route(&GovernedRoutingRequest {
        tenant: context.tenant,
        classification: governance.classification.classification(),
        required_capabilities: &context.capabilities,
        policy: &routing_policy,
        registry: &registry,
        score_revision: routing_scores.revision,
        weights: routing_scores.weights,
        affinity_provider: context.session_snapshot.affinity_provider,
        max_fallbacks: MAX_GOVERNED_ROUTING_FALLBACKS,
    }) {
        Ok(plan) => Ok(Some(plan)),
        Err(_error) if !enforcing => Ok(None),
        Err(error) => {
            let error = match error {
                GovernedRoutingError::PolicyDenied | GovernedRoutingError::ApprovalRequired => {
                    RuntimeGatewayApplicationDataPlaneError::GovernanceDenied
                }
                GovernedRoutingError::NoEligibleProvider => {
                    RuntimeGatewayApplicationDataPlaneError::NoEligibleProvider
                }
                _ => RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable,
            };
            emit_mandatory_governance_audit(
                shared,
                context,
                governance,
                None,
                "provider_selection",
                AuditOutcome::Denied,
                runtime_gateway_governance_error_code(&error),
            )?;
            Err(error)
        }
    }
}

pub(super) fn enforce_provider_revision(
    shared: &RuntimeLocalRewriteProxyShared,
    context: &GovernanceDecisionContext<'_>,
    governance: &ApplicationGovernancePlan,
    routing: Option<&GovernedRoutingPlan>,
    enforcing: bool,
) -> Result<(), RuntimeGatewayApplicationDataPlaneError> {
    if enforcing
        && let Some(routing) = routing
        && context.session_snapshot.provider_revision_mismatch(
            routing.registry_revision,
            routing.primary.descriptor_revision,
        )
    {
        emit_mandatory_governance_audit(
            shared,
            context,
            governance,
            Some(routing),
            "session_admission",
            AuditOutcome::Denied,
            "session_provider_revision_mismatch",
        )?;
        return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceDenied);
    }
    Ok(())
}
