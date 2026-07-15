use super::session::emit_mandatory_governance_audit;
use super::{
    ApplicationExecutionApprovalDecision, ApplicationGovernancePlan, ApplicationInspectionPlan,
    ApplicationObligationDisposition, AuditOutcome, GovernanceDecisionContext,
    GovernancePolicyExecution, RuntimeGatewayApplicationDataPlaneError,
    RuntimeLocalRewriteProxyShared, RuntimeProxyRequest, runtime_gateway_execution_approval,
    runtime_gateway_obligation_execution, runtime_gateway_unix_epoch_millis, runtime_proxy_log,
    runtime_proxy_log_field, runtime_proxy_structured_log_message,
};

pub(super) fn enforce_session_admission(
    shared: &RuntimeLocalRewriteProxyShared,
    context: &GovernanceDecisionContext<'_>,
    governance: &ApplicationGovernancePlan,
    governance_config: prodex_config::GovernanceConfig,
    enforcing: bool,
) -> Result<(), RuntimeGatewayApplicationDataPlaneError> {
    let violation = shared
        .governance_sessions
        .configured_violation(
            context.session_snapshot,
            context.tenant,
            context.principal,
            context.now_seconds,
            governance_config.session,
        )
        .or_else(|| {
            context
                .session_snapshot
                .policy_revision_mismatch(governance.policy.policy_revision)
                .then_some("session_policy_revision_mismatch")
        });
    if enforcing && let Some(reason) = violation {
        emit_mandatory_governance_audit(
            shared,
            context,
            governance,
            None,
            "session_admission",
            AuditOutcome::Denied,
            reason,
        )?;
        return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceDenied);
    }
    Ok(())
}

pub(super) fn resolve_execution_approval(
    shared: &RuntimeLocalRewriteProxyShared,
    context: &GovernanceDecisionContext<'_>,
    captured: &RuntimeProxyRequest,
    inspection: &ApplicationInspectionPlan,
    governance: &ApplicationGovernancePlan,
    governance_config: prodex_config::GovernanceConfig,
    enforcing: bool,
) -> Result<GovernancePolicyExecution, RuntimeGatewayApplicationDataPlaneError> {
    let obligations = runtime_gateway_obligation_execution(
        governance,
        inspection,
        &context.capabilities,
        context.route_kind,
        captured,
        shared,
        context.session_snapshot.policy,
        context.environment,
        governance_config.mode,
    );
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_obligation_execution",
            [
                runtime_proxy_log_field(
                    "disposition",
                    match obligations.disposition {
                        ApplicationObligationDisposition::Proceed => "proceed",
                        ApplicationObligationDisposition::Reject => "reject",
                    },
                ),
                runtime_proxy_log_field(
                    "violation",
                    obligations
                        .violations
                        .first()
                        .map(|violation| violation.code())
                        .unwrap_or("none"),
                ),
                runtime_proxy_log_field(
                    "response_inspection",
                    obligations.response.inspection_coverage.as_str(),
                ),
            ],
        ),
    );
    if enforcing && governance.policy.valid_until_unix_ms <= runtime_gateway_unix_epoch_millis() {
        emit_mandatory_governance_audit(
            shared,
            context,
            governance,
            None,
            "policy_decision",
            AuditOutcome::Denied,
            "policy_expired",
        )?;
        return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable);
    }

    let mut approval_authorized = false;
    if enforcing && obligations.disposition == ApplicationObligationDisposition::Reject {
        let reason = obligations
            .violations
            .first()
            .map(|violation| violation.code())
            .unwrap_or("policy_denied");
        if reason == "approval_required" {
            match runtime_gateway_execution_approval(
                shared,
                context.tenant,
                context.principal,
                captured,
                context.route_kind,
                context.session_snapshot,
                governance,
            )? {
                ApplicationExecutionApprovalDecision::Authorized(_) => {
                    approval_authorized = true;
                }
                ApplicationExecutionApprovalDecision::Pending(approval)
                | ApplicationExecutionApprovalDecision::Denied(approval) => {
                    emit_mandatory_governance_audit(
                        shared,
                        context,
                        governance,
                        None,
                        "policy_decision",
                        AuditOutcome::Denied,
                        reason,
                    )?;
                    return Err(
                        RuntimeGatewayApplicationDataPlaneError::GovernanceApprovalRequired {
                            approval_id: approval.id,
                            state: approval.state,
                        },
                    );
                }
            }
        } else {
            emit_mandatory_governance_audit(
                shared,
                context,
                governance,
                None,
                "policy_decision",
                AuditOutcome::Denied,
                reason,
            )?;
            return Err(RuntimeGatewayApplicationDataPlaneError::GovernanceDenied);
        }
    }

    Ok(GovernancePolicyExecution {
        obligations,
        approval_authorized,
    })
}
