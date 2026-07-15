use super::*;

mod context;
mod policy;
mod routing;
mod session;

use context::*;
use policy::*;
use routing::*;
use session::*;

struct GovernanceDecisionContext<'a> {
    tenant: TenantContext,
    principal: &'a Principal,
    request_id: RequestId,
    route_kind: GatewayHttpRouteKind,
    route: CanonicalRoute,
    capabilities: CapabilitySet,
    tools_requested: bool,
    request_attributes: RequestPolicyAttributes,
    request_risk: RequestRisk,
    environment: EnvironmentContext,
    channel: Channel,
    now_seconds: u64,
    session_snapshot:
        super::super::local_rewrite_governance_session::RuntimeGatewayGovernanceSessionSnapshot,
}

struct GovernancePolicyExecution {
    obligations: ApplicationObligationExecutionPlan,
    approval_authorized: bool,
}

pub(super) fn runtime_gateway_governance_decision(
    authorized: &ApplicationAuthorizedRequestContext<'_>,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    network_zone: NetworkZone,
    principal_attributes: &PrincipalPolicyAttributes,
    reservation: Option<&AtomicReservationCommand>,
    inspection: &ApplicationInspectionPlan,
) -> Result<RuntimeGatewayGovernanceDecision, RuntimeGatewayApplicationDataPlaneError> {
    let tenant = authorized
        .tenant_context()
        .ok_or(RuntimeGatewayApplicationDataPlaneError::MissingPrincipal)?;
    let principal = authorized
        .principal()
        .ok_or(RuntimeGatewayApplicationDataPlaneError::MissingPrincipal)?;
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

    let context = build_governance_context(
        authorized,
        captured,
        shared,
        tenant,
        principal,
        network_zone,
        inspection,
    )?;
    let governance = evaluate_policy(
        &context,
        &application_snapshot,
        principal_attributes,
        reservation,
        inspection,
    )?;
    let enforcing = governance_config.mode.is_enforcing();
    enforce_session_admission(shared, &context, &governance, governance_config, enforcing)?;
    let execution = resolve_execution_approval(
        shared,
        &context,
        captured,
        inspection,
        &governance,
        governance_config,
        enforcing,
    )?;
    let routing = plan_provider_route(
        shared,
        &context,
        &governance,
        execution.approval_authorized,
        enforcing,
    )?;
    enforce_provider_revision(shared, &context, &governance, routing.as_ref(), enforcing)?;
    persist_governance_session(
        shared,
        &context,
        &governance,
        routing.as_ref(),
        governance_config,
    )?;
    emit_mandatory_governance_audit(
        shared,
        &context,
        &governance,
        routing.as_ref(),
        "provider_selection",
        AuditOutcome::Success,
        "policy_allow",
    )?;

    Ok(RuntimeGatewayGovernanceDecision {
        plan: governance,
        routing,
        obligations: execution.obligations,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn orchestrator_keeps_security_phase_order_explicit() {
        let source = include_str!("governance_decision.rs");
        let order = [
            "build_governance_context(",
            "evaluate_policy(",
            "enforce_session_admission(",
            "resolve_execution_approval(",
            "plan_provider_route(",
            "enforce_provider_revision(",
            "persist_governance_session(",
            "emit_mandatory_governance_audit(",
        ];
        let mut offset = 0;
        for phase in order {
            let found = source[offset..].find(phase).expect(phase);
            offset += found + phase.len();
        }
    }
}
