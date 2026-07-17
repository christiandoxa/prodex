use super::{
    RuntimeGatewayApplicationAdmission, RuntimeGatewayApplicationAdmissionKind,
    RuntimeGatewayApplicationDataPlaneError, RuntimeLocalRewriteProxyShared,
    runtime_gateway_application_http_policy, runtime_gateway_compatibility_provider_invocation,
    runtime_gateway_http_request_meta,
};
use crate::RuntimeProxyRequest;
use prodex_application::{ApplicationDataPlaneError, ApplicationInspectionPlan};
use prodex_gateway_http::{GatewayHttpPolicy, GatewayHttpRouteKind, plan_gateway_http_request};

impl RuntimeGatewayApplicationAdmission {
    /// Keeps anonymous compatibility traffic on the expected provider data-plane route.
    pub(in super::super) fn compatibility_anonymous(
        expected_route: GatewayHttpRouteKind,
        captured: &RuntimeProxyRequest,
        shared: &RuntimeLocalRewriteProxyShared,
        inspection: ApplicationInspectionPlan,
    ) -> Result<Self, RuntimeGatewayApplicationDataPlaneError> {
        let route = runtime_gateway_compatibility_http_route(
            runtime_gateway_application_http_policy(shared),
            expected_route,
            captured,
        )?;
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

pub(super) fn runtime_gateway_compatibility_http_route(
    policy: GatewayHttpPolicy,
    expected_route: GatewayHttpRouteKind,
    captured: &RuntimeProxyRequest,
) -> Result<GatewayHttpRouteKind, RuntimeGatewayApplicationDataPlaneError> {
    let http = runtime_gateway_http_request_meta(captured, captured.path_and_query.as_str());
    let plan = plan_gateway_http_request(policy, http).map_err(|error| {
        RuntimeGatewayApplicationDataPlaneError::Admission(ApplicationDataPlaneError::Http(error))
    })?;
    if plan.route != expected_route || !plan.route.is_provider_data_plane() {
        return Err(RuntimeGatewayApplicationDataPlaneError::Admission(
            ApplicationDataPlaneError::WrongRoute(plan.route),
        ));
    }
    Ok(plan.route)
}
