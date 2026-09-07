use super::*;

#[test]
fn legacy_admin_key_aliases_keep_typed_operation_and_method_behavior() {
    for (method, path, operation) in [
        (
            GatewayHttpMethod::Get,
            "/admin/keys",
            GatewayControlPlaneOperation::VirtualKeyRead,
        ),
        (
            GatewayHttpMethod::Post,
            "/v1/admin/keys",
            GatewayControlPlaneOperation::VirtualKeyCreate,
        ),
        (
            GatewayHttpMethod::Patch,
            "/admin/keys/key-1",
            GatewayControlPlaneOperation::VirtualKeyUpdate,
        ),
        (
            GatewayHttpMethod::Delete,
            "/v1/admin/keys/key-1",
            GatewayControlPlaneOperation::VirtualKeyDelete,
        ),
        (
            GatewayHttpMethod::Post,
            "/admin/keys/key-1/secret",
            GatewayControlPlaneOperation::VirtualKeyRotateSecret,
        ),
        (
            GatewayHttpMethod::Get,
            "/admin/keys/key-1/extra",
            GatewayControlPlaneOperation::VirtualKeyRead,
        ),
    ] {
        let plan = plan_control_plane_route(&GatewayHttpRequestMeta {
            method,
            path: path.to_string(),
            body_len: 128,
            headers: vec![traceparent()],
        })
        .expect(path);
        assert_eq!(plan.operation, operation, "{path}");
    }

    assert_eq!(
        plan_control_plane_route(&GatewayHttpRequestMeta {
            method: GatewayHttpMethod::Post,
            path: "/admin/keys/key-1".to_string(),
            body_len: 128,
            headers: vec![traceparent()],
        }),
        Err(GatewayControlPlaneRouteError::UnknownControlPlaneRoute)
    );
    assert_eq!(
        plan_control_plane_route(&GatewayHttpRequestMeta {
            method: GatewayHttpMethod::Get,
            path: "/admin/keys/key-1/secret".to_string(),
            body_len: 0,
            headers: vec![traceparent()],
        }),
        Err(GatewayControlPlaneRouteError::MethodNotAllowed {
            operation: GatewayControlPlaneOperation::VirtualKeyRotateSecret,
            method: GatewayHttpMethod::Get,
        })
    );
}
