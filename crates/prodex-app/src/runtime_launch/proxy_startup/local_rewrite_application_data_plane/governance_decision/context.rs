use super::{
    ApplicationAuthorizedRequestContext, ApplicationGovernancePlan, ApplicationGovernanceRequest,
    ApplicationGovernanceSnapshot, ApplicationInspectionPlan, AtomicReservationCommand, Channel,
    CredentialScope, DataClassification, EnvironmentContext, GovernanceDecisionContext,
    ModelCapability, NetworkZone, Principal, PrincipalPolicyAttributes, QuotaContext,
    RequestPolicyAttributes, RequestRisk, RuntimeGatewayApplicationDataPlaneError,
    RuntimeLocalRewriteProxyShared, RuntimeProxyRequest, TenantContext,
    plan_application_governance, runtime_gateway_governance_route, runtime_gateway_governed_action,
    runtime_gateway_requested_capabilities, runtime_gateway_requested_modalities,
    runtime_gateway_requested_tools, runtime_gateway_unix_epoch_millis,
    runtime_provider_model_from_body,
};

pub(super) fn build_governance_context<'a>(
    authorized: &ApplicationAuthorizedRequestContext<'_>,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    tenant: TenantContext,
    principal: &'a Principal,
    network_zone: NetworkZone,
    inspection: &ApplicationInspectionPlan,
) -> Result<GovernanceDecisionContext<'a>, RuntimeGatewayApplicationDataPlaneError> {
    let request_id = authorized.request().request_id();
    let route_kind = authorized.request().route();
    let route = runtime_gateway_governance_route(route_kind)?;
    let capabilities = runtime_gateway_requested_capabilities(route_kind, captured);
    let tools_requested = capabilities.contains(ModelCapability::Tools);
    let requested_tools = if tools_requested {
        runtime_gateway_requested_tools(&captured.body)
            .ok_or(RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?
    } else {
        Vec::new()
    };
    let request_attributes = RequestPolicyAttributes::new(
        runtime_provider_model_from_body(&captured.body).as_deref(),
        &requested_tools,
        runtime_gateway_requested_modalities(route_kind, &capabilities),
        None,
        runtime_gateway_unix_epoch_millis(),
    )
    .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)?;
    let request_risk = if tools_requested {
        RequestRisk::High
    } else if inspection.result.coverage() != prodex_domain::InspectionCoverage::Full {
        RequestRisk::Elevated
    } else {
        RequestRisk::Low
    };
    let environment = EnvironmentContext {
        network_zone,
        authentication_strength: 1,
        mfa_satisfied: false,
    };
    let channel = Channel::Api;
    let now_seconds = runtime_gateway_unix_epoch_millis() / 1_000;
    let session_snapshot =
        shared
            .governance_sessions
            .snapshot(captured, tenant, principal, channel, now_seconds);

    Ok(GovernanceDecisionContext {
        tenant,
        principal,
        request_id,
        route_kind,
        route,
        capabilities,
        tools_requested,
        request_attributes,
        request_risk,
        environment,
        channel,
        now_seconds,
        session_snapshot,
    })
}

pub(super) fn evaluate_policy(
    context: &GovernanceDecisionContext<'_>,
    application_snapshot: &ApplicationGovernanceSnapshot,
    principal_attributes: &PrincipalPolicyAttributes,
    reservation: Option<&AtomicReservationCommand>,
    inspection: &ApplicationInspectionPlan,
) -> Result<ApplicationGovernancePlan, RuntimeGatewayApplicationDataPlaneError> {
    let session = context.session_snapshot.policy;
    plan_application_governance(
        application_snapshot,
        ApplicationGovernanceRequest {
            inspection,
            trusted_label: None,
            untrusted_label: None,
            prior_classification: Some(session.retained_classification),
            session_floor: session.retained_classification,
            route_floor: DataClassification::Public,
            request_risk_floor: if context.tools_requested {
                DataClassification::Internal
            } else {
                DataClassification::Public
            },
            tenant: context.tenant,
            principal: context.principal,
            principal_attributes,
            channel: context.channel,
            credential_scope: CredentialScope::DataPlane,
            session,
            action: runtime_gateway_governed_action(context.route_kind),
            route: &context.route,
            request_risk: context.request_risk,
            requested_capabilities: &context.capabilities,
            request_attributes: &context.request_attributes,
            quota: QuotaContext {
                has_headroom: reservation.is_some_and(|command| {
                    !command
                        .request
                        .estimate
                        .exceeds(command.snapshot.available(command.limit))
                }),
                reservation_required: true,
            },
            environment: context.environment,
        },
    )
    .map_err(|_| RuntimeGatewayApplicationDataPlaneError::GovernanceUnavailable)
}
