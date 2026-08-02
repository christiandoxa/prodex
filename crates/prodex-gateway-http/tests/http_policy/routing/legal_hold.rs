use super::*;

#[test]
fn control_plane_route_planner_maps_legal_hold_paths_to_explicit_operations() {
    for (method, path, operation, requires_idempotency) in [
        (
            GatewayHttpMethod::Get,
            "/prodex/gateway/audit/retention/holds",
            GatewayControlPlaneOperation::AuditLegalHoldRead,
            false,
        ),
        (
            GatewayHttpMethod::Post,
            "/prodex/gateway/audit/retention/holds",
            GatewayControlPlaneOperation::AuditLegalHoldUpsert,
            true,
        ),
        (
            GatewayHttpMethod::Delete,
            "/prodex/gateway/audit/retention/holds/audit-event-1",
            GatewayControlPlaneOperation::AuditLegalHoldDelete,
            true,
        ),
    ] {
        let plan = plan_control_plane_route(&GatewayHttpRequestMeta {
            method,
            path: path.to_string(),
            body_len: 128,
            headers: vec![traceparent()],
        })
        .unwrap();
        assert_eq!(plan.operation, operation);
        assert_eq!(plan.requires_idempotency, requires_idempotency);
        assert!(plan.requires_audit);
    }
}
