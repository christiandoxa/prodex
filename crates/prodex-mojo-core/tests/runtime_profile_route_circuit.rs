#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime::{
    RuntimeProfileRouteCircuitAction, RuntimeProfileRouteCircuitActionInput,
    runtime_profile_route_circuit_action,
};

#[test]
fn required_mojo_route_circuit_action_matrix_is_authoritative() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };

    for (input, expected) in [
        (
            RuntimeProfileRouteCircuitActionInput {
                circuit_present: false,
                circuit_open: false,
                health_score_zero: false,
            },
            RuntimeProfileRouteCircuitAction::AllowNoCircuit,
        ),
        (
            RuntimeProfileRouteCircuitActionInput {
                circuit_present: true,
                circuit_open: true,
                health_score_zero: true,
            },
            RuntimeProfileRouteCircuitAction::RejectOpen,
        ),
        (
            RuntimeProfileRouteCircuitActionInput {
                circuit_present: true,
                circuit_open: false,
                health_score_zero: true,
            },
            RuntimeProfileRouteCircuitAction::ClearAndAllow,
        ),
        (
            RuntimeProfileRouteCircuitActionInput {
                circuit_present: true,
                circuit_open: false,
                health_score_zero: false,
            },
            RuntimeProfileRouteCircuitAction::ReserveAndAllow,
        ),
    ] {
        assert_eq!(runtime_profile_route_circuit_action(input), Ok(expected));
    }
}
