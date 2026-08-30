from std.memory import Pointer

comptime CONTROL_PLANE_ROUTING_ABI_VERSION: Int64 = 1
comptime CONTROL_PLANE_OPERATION_COUNT: Int64 = 35

comptime CONTROL_PLANE_METHOD_GET: Int64 = 0
comptime CONTROL_PLANE_METHOD_POST: Int64 = 1
comptime CONTROL_PLANE_METHOD_PUT: Int64 = 2
comptime CONTROL_PLANE_METHOD_PATCH: Int64 = 3
comptime CONTROL_PLANE_METHOD_DELETE: Int64 = 4
comptime CONTROL_PLANE_METHOD_OPTIONS: Int64 = 5
comptime CONTROL_PLANE_METHOD_OTHER: Int64 = 6

comptime CONTROL_PLANE_VALIDATION_EXACT: Int64 = 0
comptime CONTROL_PLANE_VALIDATION_ALIAS: Int64 = 1
comptime CONTROL_PLANE_VALIDATION_ALIAS_AND_METHOD: Int64 = 2

comptime CONTROL_PLANE_DECISION_ALLOW: Int64 = 0
comptime CONTROL_PLANE_DECISION_MISMATCH: Int64 = 1
comptime CONTROL_PLANE_DECISION_METHOD_NOT_ALLOWED: Int64 = 2


def control_plane_operations_share_route_family(
    route_operation: Int64, action_operation: Int64
) -> Bool:
    if route_operation == action_operation:
        return True
    if (route_operation == 2 or route_operation == 3) and (
        action_operation == 2 or action_operation == 3
    ):
        return True
    if (route_operation >= 5 and route_operation <= 8) and (
        action_operation >= 5 and action_operation <= 8
    ):
        return True
    if (route_operation == 9 or route_operation == 10) and (
        action_operation == 9 or action_operation == 10
    ):
        return True
    if (route_operation >= 12 and route_operation <= 16) and (
        action_operation >= 12 and action_operation <= 16
    ):
        return True
    if (route_operation >= 17 and route_operation <= 25) and (
        action_operation >= 17 and action_operation <= 25
    ):
        return True
    if (route_operation == 29 or route_operation == 33) and (
        action_operation == 29 or action_operation == 33
    ):
        return True
    if (route_operation >= 30 and route_operation <= 32) and (
        action_operation >= 30 and action_operation <= 32
    ):
        return True
    return False


def control_plane_http_action_alias_allowed(
    route_operation: Int64, action_operation: Int64, method: Int64
) -> Bool:
    return (
        route_operation == 14
        and action_operation == 16
        and method == CONTROL_PLANE_METHOD_PATCH
    )


def control_plane_operation_allows_http_method(
    operation: Int64, method: Int64
) -> Bool:
    if (
        operation == 0
        or operation == 5
        or operation == 12
        or operation == 17
        or operation == 28
        or operation == 30
    ):
        return method == CONTROL_PLANE_METHOD_GET
    if (
        operation == 1
        or operation == 2
        or operation == 4
        or operation == 6
        or operation == 9
        or operation == 11
        or operation == 13
        or operation == 16
        or operation == 18
        or operation == 19
        or operation == 20
        or operation == 21
        or operation == 22
        or operation == 23
        or operation == 24
        or operation == 26
        or operation == 29
        or operation == 31
        or operation == 34
    ):
        return method == CONTROL_PLANE_METHOD_POST
    if operation == 25:
        return method == CONTROL_PLANE_METHOD_GET or method == CONTROL_PLANE_METHOD_POST
    if operation == 3 or operation == 14 or operation == 27:
        return method == CONTROL_PLANE_METHOD_PATCH
    if operation == 7:
        return method == CONTROL_PLANE_METHOD_PATCH or method == CONTROL_PLANE_METHOD_PUT
    if (
        operation == 8
        or operation == 10
        or operation == 15
        or operation == 32
        or operation == 33
    ):
        return method == CONTROL_PLANE_METHOD_DELETE
    return False


@export("prodex_mojo_control_plane_route_validation_v1")
def prodex_mojo_control_plane_route_validation_v1(
    abi_version: Int64,
    route_operation: Int64,
    action_operation: Int64,
    method: Int64,
    mode: Int64,
    decision_address: UInt,
) abi("C") -> Int64:
    if abi_version != CONTROL_PLANE_ROUTING_ABI_VERSION:
        return 2
    if (
        route_operation < 0
        or route_operation >= CONTROL_PLANE_OPERATION_COUNT
        or action_operation < 0
        or action_operation >= CONTROL_PLANE_OPERATION_COUNT
        or method < CONTROL_PLANE_METHOD_GET
        or method > CONTROL_PLANE_METHOD_OTHER
        or mode < CONTROL_PLANE_VALIDATION_EXACT
        or mode > CONTROL_PLANE_VALIDATION_ALIAS_AND_METHOD
        or decision_address == 0
    ):
        return 1

    var decision = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(decision_address)
    )
    if route_operation == action_operation:
        decision[] = CONTROL_PLANE_DECISION_ALLOW
    elif mode != CONTROL_PLANE_VALIDATION_EXACT and control_plane_http_action_alias_allowed(
        route_operation, action_operation, method
    ):
        decision[] = CONTROL_PLANE_DECISION_ALLOW
    elif (
        mode == CONTROL_PLANE_VALIDATION_ALIAS_AND_METHOD
        and control_plane_operations_share_route_family(route_operation, action_operation)
        and not control_plane_operation_allows_http_method(action_operation, method)
    ):
        decision[] = CONTROL_PLANE_DECISION_METHOD_NOT_ALLOWED
    else:
        decision[] = CONTROL_PLANE_DECISION_MISMATCH
    return 0
