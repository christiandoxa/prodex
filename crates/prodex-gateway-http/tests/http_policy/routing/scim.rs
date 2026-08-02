use super::*;

#[test]
fn control_plane_route_planner_maps_scim_paths_and_methods() {
    let read = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Get,
        path: "/prodex/gateway/scim/v2/Users/user-1".to_string(),
        body_len: 0,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(read.operation, GatewayControlPlaneOperation::ScimUserRead);
    assert!(!read.requires_idempotency);
    assert!(read.requires_audit);

    let create = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Post,
        path: "/prodex/gateway/scim/v2/Users".to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(
        create.operation,
        GatewayControlPlaneOperation::ScimUserCreate
    );
    assert!(create.requires_idempotency);
    assert!(create.requires_audit);

    let update = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Patch,
        path: "/v1/scim/v2/Users/user-1".to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(
        update.operation,
        GatewayControlPlaneOperation::ScimUserUpdate
    );
    assert!(update.requires_idempotency);
    assert!(update.requires_audit);

    let replace = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Put,
        path: "/v1/scim/v2/Users/user-1".to_string(),
        body_len: 128,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(
        replace.operation,
        GatewayControlPlaneOperation::ScimUserUpdate
    );
    assert!(replace.requires_idempotency);
    assert!(replace.requires_audit);

    let delete = plan_control_plane_route(&GatewayHttpRequestMeta {
        method: GatewayHttpMethod::Delete,
        path: "/scim/v2/Users/user-1".to_string(),
        body_len: 0,
        headers: vec![traceparent()],
    })
    .unwrap();
    assert_eq!(
        delete.operation,
        GatewayControlPlaneOperation::ScimUserDelete
    );
    assert!(delete.requires_idempotency);
    assert!(delete.requires_audit);
}
