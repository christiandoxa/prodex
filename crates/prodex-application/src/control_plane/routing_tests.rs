#[cfg(feature = "mojo")]
use super::control_plane_route_validation_rust;
use super::{ControlPlaneRouteValidationMode, control_plane_route_validation};
use prodex_control_plane::ControlPlaneOperation;
use prodex_gateway_http::GatewayHttpMethod;

#[cfg(feature = "mojo")]
#[test]
fn mojo_control_plane_route_validation_matches_rust_oracle() {
    let methods = [
        GatewayHttpMethod::Get,
        GatewayHttpMethod::Post,
        GatewayHttpMethod::Put,
        GatewayHttpMethod::Patch,
        GatewayHttpMethod::Delete,
        GatewayHttpMethod::Options,
        GatewayHttpMethod::Other,
    ];
    let modes = [
        ControlPlaneRouteValidationMode::Exact,
        ControlPlaneRouteValidationMode::AllowAlias,
        ControlPlaneRouteValidationMode::AllowAliasAndMethodCheck,
    ];

    for route_operation in ControlPlaneOperation::ALL {
        for action_operation in ControlPlaneOperation::ALL {
            for method in methods {
                for mode in modes {
                    let expected = control_plane_route_validation_rust(
                        route_operation,
                        action_operation,
                        method,
                        mode,
                    );
                    let actual = control_plane_route_validation(
                        route_operation,
                        action_operation,
                        method,
                        mode,
                    )
                    .expect("valid operation and method tags must cross Mojo");
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}

#[cfg(not(feature = "mojo"))]
#[test]
fn feature_off_route_validation_preserves_method_rejection() {
    let decision = control_plane_route_validation(
        ControlPlaneOperation::VirtualKeyRead,
        ControlPlaneOperation::VirtualKeyRotateSecret,
        GatewayHttpMethod::Get,
        ControlPlaneRouteValidationMode::AllowAliasAndMethodCheck,
    )
    .expect("feature-off validation should not need Mojo");
    assert_eq!(
        decision,
        super::ControlPlaneRouteValidationDecision::MethodNotAllowed
    );
}
