use prodex_application::plan_application_control_plane_http_route;
use prodex_control_plane::ControlPlaneOperation;
use prodex_gateway_http::{
    GatewayControlPlaneOperation, GatewayHttpMethod, GatewayHttpRequestMeta,
};

#[test]
fn route_explain_maps_through_the_application_control_plane_boundary() {
    let explain = plan_application_control_plane_http_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/v1/prodex/gateway/routes/explain".to_string(),
        body_len: 128,
        headers: Vec::new(),
    })
    .unwrap();

    assert_eq!(explain.operation, ControlPlaneOperation::RouteExplain);
    assert_eq!(
        explain.http.operation,
        GatewayControlPlaneOperation::RouteExplain
    );
    assert!(!explain.http.requires_idempotency);
    assert!(explain.http.requires_audit);
}
