use crate::MojoError;

const RUNTIME_PROFILE_ROUTE_CIRCUIT_ABI_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProfileRouteCircuitActionInput {
    pub circuit_present: bool,
    pub circuit_open: bool,
    pub health_score_zero: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfileRouteCircuitAction {
    AllowNoCircuit,
    RejectOpen,
    ClearAndAllow,
    ReserveAndAllow,
}

unsafe extern "C" {
    fn prodex_runtime_profile_route_circuit_action_v1(
        abi_version: i64,
        circuit_present: i64,
        circuit_open: i64,
        health_score_zero: i64,
    ) -> i64;
}

/// Plans route-circuit admission from facts gathered by the runtime host.
pub fn runtime_profile_route_circuit_action(
    input: RuntimeProfileRouteCircuitActionInput,
) -> Result<RuntimeProfileRouteCircuitAction, MojoError> {
    let result = unsafe {
        prodex_runtime_profile_route_circuit_action_v1(
            RUNTIME_PROFILE_ROUTE_CIRCUIT_ABI_VERSION,
            i64::from(input.circuit_present),
            i64::from(input.circuit_open),
            i64::from(input.health_score_zero),
        )
    };
    match result {
        0 => Ok(RuntimeProfileRouteCircuitAction::AllowNoCircuit),
        1 => Ok(RuntimeProfileRouteCircuitAction::RejectOpen),
        2 => Ok(RuntimeProfileRouteCircuitAction::ClearAndAllow),
        3 => Ok(RuntimeProfileRouteCircuitAction::ReserveAndAllow),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_circuit_action_covers_all_state_boundaries() {
        let cases = [
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
                    health_score_zero: false,
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
        ];
        for (input, expected) in cases {
            assert_eq!(runtime_profile_route_circuit_action(input), Ok(expected));
        }
    }

    #[test]
    fn route_circuit_action_rejects_abi_and_boolean_errors() {
        assert_eq!(
            unsafe { prodex_runtime_profile_route_circuit_action_v1(0, 0, 0, 0) },
            -4
        );
        assert_eq!(
            unsafe { prodex_runtime_profile_route_circuit_action_v1(1, 2, 0, 0) },
            -1
        );
        assert_eq!(
            unsafe { prodex_runtime_profile_route_circuit_action_v1(1, 0, 0, 2) },
            -1
        );
    }
}
