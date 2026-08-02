use prodex_application::plan_application_control_plane_http_route;
use prodex_control_plane::ControlPlaneOperation;
use prodex_gateway_http::{GatewayHttpHeader, GatewayHttpMethod, GatewayHttpRequestMeta};

fn request(method: GatewayHttpMethod, path: &str, body_len: usize) -> GatewayHttpRequestMeta {
    GatewayHttpRequestMeta {
        method,
        path: path.to_string(),
        body_len,
        headers: vec![GatewayHttpHeader::new(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )],
    }
}

#[test]
fn application_control_plane_http_route_maps_legal_hold_operations() {
    for (method, path, body_len, operation, requires_idempotency) in [
        (
            GatewayHttpMethod::Get,
            "/admin/audit/retention/holds",
            0,
            ControlPlaneOperation::AuditLegalHoldRead,
            false,
        ),
        (
            GatewayHttpMethod::Post,
            "/admin/audit/retention/holds",
            128,
            ControlPlaneOperation::AuditLegalHoldUpsert,
            true,
        ),
        (
            GatewayHttpMethod::Delete,
            "/admin/audit/retention/holds/audit-event-1",
            0,
            ControlPlaneOperation::AuditLegalHoldDelete,
            true,
        ),
    ] {
        let plan =
            plan_application_control_plane_http_route(&request(method, path, body_len)).unwrap();
        assert_eq!(plan.operation, operation);
        assert_eq!(plan.http.requires_idempotency, requires_idempotency);
    }
}
