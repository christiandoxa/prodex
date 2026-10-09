from std.memory import Pointer

comptime WEBSOCKET_FAILURE_RATE_LIMITED: Int64 = 0
comptime WEBSOCKET_FAILURE_AUTH_FAILED: Int64 = 1
comptime WEBSOCKET_FAILURE_OVERLOADED: Int64 = 2
comptime WEBSOCKET_FAILURE_LOCAL_SELECTION_BLOCKED: Int64 = 3


@export("prodex_runtime_websocket_failure_state_plan_v1")
def prodex_runtime_websocket_failure_state_plan_v1(
    failure_kind: Int64,
    affinity_releasable: Int64,
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if (
        failure_kind < WEBSOCKET_FAILURE_RATE_LIMITED
        or failure_kind > WEBSOCKET_FAILURE_LOCAL_SELECTION_BLOCKED
        or (affinity_releasable != 0 and affinity_releasable != 1)
    ):
        return 1

    # output:
    # 0 clear_affinity
    # 1 store_last_failure
    # 2 last_failure_retryable
    # 3 record_rate_limit_failure
    # 4 record_overload_failure
    for index in range(5):
        output[unsafe_offset=index] = 0

    var releasable = affinity_releasable == 1

    if failure_kind == WEBSOCKET_FAILURE_RATE_LIMITED:
        output[unsafe_offset=1] = 1
        output[unsafe_offset=3] = 1
        return 0

    if failure_kind == WEBSOCKET_FAILURE_AUTH_FAILED:
        output[unsafe_offset=0] = Int64(releasable)
        output[unsafe_offset=1] = 1
        output[unsafe_offset=2] = 1
        return 0

    if failure_kind == WEBSOCKET_FAILURE_OVERLOADED:
        output[unsafe_offset=1] = 1
        output[unsafe_offset=4] = 1
        return 0

    # A local in-flight block is pressure, not a profile failure. The existing
    # disposition plan owns continuation/backoff/exclusion precedence.
    output[unsafe_offset=0] = Int64(releasable)
    return 0
