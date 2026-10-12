comptime RUNTIME_PROFILE_ROUTE_CIRCUIT_ABI_VERSION: Int64 = 1

comptime RUNTIME_PROFILE_ROUTE_CIRCUIT_ALLOW_NO_CIRCUIT: Int64 = 0
comptime RUNTIME_PROFILE_ROUTE_CIRCUIT_REJECT_OPEN: Int64 = 1
comptime RUNTIME_PROFILE_ROUTE_CIRCUIT_CLEAR_AND_ALLOW: Int64 = 2
comptime RUNTIME_PROFILE_ROUTE_CIRCUIT_RESERVE_AND_ALLOW: Int64 = 3


def runtime_profile_route_circuit_bool_is_valid(value: Int64) -> Bool:
    return value == 0 or value == 1


# Owns the side-effect-free route-circuit admission action. Rust supplies facts
# read while holding the runtime lock and applies the selected state mutation.
@export("prodex_runtime_profile_route_circuit_action_v1")
def prodex_runtime_profile_route_circuit_action_v1(
    abi_version: Int64,
    circuit_present: Int64,
    circuit_open: Int64,
    health_score_zero: Int64,
) abi("C") -> Int64:
    if abi_version != RUNTIME_PROFILE_ROUTE_CIRCUIT_ABI_VERSION:
        return -4
    if (
        not runtime_profile_route_circuit_bool_is_valid(circuit_present)
        or not runtime_profile_route_circuit_bool_is_valid(circuit_open)
        or not runtime_profile_route_circuit_bool_is_valid(health_score_zero)
    ):
        return -1
    if circuit_present == 0:
        return RUNTIME_PROFILE_ROUTE_CIRCUIT_ALLOW_NO_CIRCUIT
    if circuit_open == 1:
        return RUNTIME_PROFILE_ROUTE_CIRCUIT_REJECT_OPEN
    if health_score_zero == 1:
        return RUNTIME_PROFILE_ROUTE_CIRCUIT_CLEAR_AND_ALLOW
    return RUNTIME_PROFILE_ROUTE_CIRCUIT_RESERVE_AND_ALLOW
