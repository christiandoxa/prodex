use super::{
    ApplicationGovernancePlan, AuditOutcome, GovernanceDecisionContext, GovernedRoutingPlan,
    RuntimeGatewayApplicationDataPlaneError, RuntimeLocalRewriteProxyShared,
    runtime_gateway_mandatory_governance_audit,
};

pub(super) fn persist_governance_session(
    shared: &RuntimeLocalRewriteProxyShared,
    context: &GovernanceDecisionContext<'_>,
    governance: &ApplicationGovernancePlan,
    routing: Option<&GovernedRoutingPlan>,
    governance_config: prodex_config::GovernanceConfig,
) -> Result<(), RuntimeGatewayApplicationDataPlaneError> {
    let selected_provider = routing
        .map(|routing| routing.primary.provider)
        .unwrap_or_else(|| shared.provider.bridge_kind().provider_id());
    if let Err(error) = shared.governance_sessions.remember(
        context.session_snapshot,
        context.tenant,
        context.principal,
        context.channel,
        context.now_seconds,
        governance.classification.classification(),
        governance.policy.policy_revision,
        routing
            .map(|routing| routing.registry_revision)
            .unwrap_or_default(),
        routing
            .map(|routing| routing.primary.descriptor_revision)
            .unwrap_or_default(),
        selected_provider,
        governance_config.session,
    ) {
        let (reason, error) = match error {
            super::super::super::local_rewrite_governance_session::RuntimeGatewayGovernanceSessionPersistError::ConcurrentLimitReached => (
                "session_concurrency_limit",
                RuntimeGatewayApplicationDataPlaneError::GovernanceDenied,
            ),
            super::super::super::local_rewrite_governance_session::RuntimeGatewayGovernanceSessionPersistError::Unavailable => (
                "session_store_unavailable",
                RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable,
            ),
        };
        emit_mandatory_governance_audit(
            shared,
            context,
            governance,
            routing,
            "session_admission",
            AuditOutcome::Denied,
            reason,
        )?;
        return Err(error);
    }
    Ok(())
}

pub(super) fn emit_mandatory_governance_audit(
    shared: &RuntimeLocalRewriteProxyShared,
    context: &GovernanceDecisionContext<'_>,
    governance: &ApplicationGovernancePlan,
    routing: Option<&GovernedRoutingPlan>,
    action: &str,
    outcome: AuditOutcome,
    reason: &str,
) -> Result<(), RuntimeGatewayApplicationDataPlaneError> {
    runtime_gateway_mandatory_governance_audit(
        shared,
        context.tenant,
        context.principal,
        context.request_id,
        action,
        outcome,
        governance,
        routing,
        reason,
    )
}
