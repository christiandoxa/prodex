from std.memory import Pointer

comptime WEBSOCKET_FAILURE_ABI_VERSION: Int64 = 2

comptime WEBSOCKET_FAILURE_RATE_LIMITED: Int64 = 0
comptime WEBSOCKET_FAILURE_AUTH_FAILED: Int64 = 1
comptime WEBSOCKET_FAILURE_OVERLOADED: Int64 = 2
comptime WEBSOCKET_FAILURE_LOCAL_SELECTION_BLOCKED: Int64 = 3

comptime WEBSOCKET_FAILURE_QUOTA: Int64 = 4
comptime WEBSOCKET_FAILURE_TRANSPORT: Int64 = 5
comptime WEBSOCKET_FAILURE_PREVIOUS_RESPONSE: Int64 = 6
comptime WEBSOCKET_FAILURE_REJECTED: Int64 = 7

comptime WEBSOCKET_FAILURE_ACTION_PASS_THROUGH: Int64 = 0
comptime WEBSOCKET_FAILURE_ACTION_ROTATE: Int64 = 1
comptime WEBSOCKET_FAILURE_ACTION_FULL_CONTEXT_RETRY: Int64 = 2
comptime WEBSOCKET_FAILURE_ACTION_RETRY_TRANSPORT: Int64 = 3
comptime WEBSOCKET_FAILURE_ACTION_REUSE_WATCHDOG: Int64 = 4
comptime WEBSOCKET_FAILURE_ACTION_CONTINUE: Int64 = 5
comptime WEBSOCKET_FAILURE_ACTION_ERROR: Int64 = 6

comptime WEBSOCKET_FRAME_RETRY_NONE: Int64 = 0
comptime WEBSOCKET_FRAME_RETRY_CONNECTION_LIMIT: Int64 = 1
comptime WEBSOCKET_FRAME_RETRY_QUOTA: Int64 = 2
comptime WEBSOCKET_FRAME_RETRY_RATE_LIMITED: Int64 = 3
comptime WEBSOCKET_FRAME_RETRY_OVERLOADED: Int64 = 4
comptime WEBSOCKET_FRAME_RETRY_PREVIOUS_RESPONSE: Int64 = 5


def websocket_failure_bool(value: Int64) -> Bool:
    return value == 0 or value == 1


@export("prodex_runtime_websocket_failure_decision_v2")
def prodex_runtime_websocket_failure_decision_v2(
    abi_version: Int64,
    failure_kind: Int64,
    stream_committed: Int64,
    hard_affinity: Int64,
    affinity_releasable: Int64,
    inflight_saturated: Int64,
    full_context_retry_available: Int64,
    quota_fallback_available: Int64,
    direct_current_fallback: Int64,
    reuse_existing_session: Int64,
    precommit_transport_retry_allowed: Int64,
    reset_retry_index: Int64,
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if abi_version != WEBSOCKET_FAILURE_ABI_VERSION:
        return 4
    if (
        failure_kind < WEBSOCKET_FAILURE_RATE_LIMITED
        or failure_kind > WEBSOCKET_FAILURE_REJECTED
        or not websocket_failure_bool(stream_committed)
        or not websocket_failure_bool(hard_affinity)
        or not websocket_failure_bool(affinity_releasable)
        or not websocket_failure_bool(inflight_saturated)
        or not websocket_failure_bool(full_context_retry_available)
        or not websocket_failure_bool(quota_fallback_available)
        or not websocket_failure_bool(direct_current_fallback)
        or not websocket_failure_bool(reuse_existing_session)
        or not websocket_failure_bool(precommit_transport_retry_allowed)
        or not websocket_failure_bool(reset_retry_index)
        or (hard_affinity == 1 and affinity_releasable == 1)
        or (hard_affinity == 0 and affinity_releasable == 0)
    ):
        return 1

    # output:
    # 0 action, 1 mark_backoff, 2 exclude_profile, 3 clear_affinity,
    # 4 retryable_failure, 5 record_rate_limit, 6 record_overload,
    # 7 store_last_failure, 8 last_failure_retryable, 9 release_affinity,
    # 10 terminal, 11 reset_retry_index
    for index in range(12):
        output[unsafe_offset=index] = 0

    var action = WEBSOCKET_FAILURE_ACTION_PASS_THROUGH
    var retryable = False
    var stateful_failure = False
    var clear_affinity = False
    var mark_backoff = False
    var release_affinity = False
    var record_rate_limit = False
    var record_overload = False
    var last_failure_retryable = False

    if stream_committed == 1:
        if failure_kind == WEBSOCKET_FAILURE_TRANSPORT:
            action = WEBSOCKET_FAILURE_ACTION_ERROR
        else:
            action = WEBSOCKET_FAILURE_ACTION_PASS_THROUGH
        output[unsafe_offset=0] = action
        output[unsafe_offset=10] = 1
        return 0

    if failure_kind == WEBSOCKET_FAILURE_TRANSPORT:
        if reuse_existing_session == 1:
            action = WEBSOCKET_FAILURE_ACTION_REUSE_WATCHDOG
        elif precommit_transport_retry_allowed == 1:
            action = WEBSOCKET_FAILURE_ACTION_RETRY_TRANSPORT
        elif hard_affinity == 1 and full_context_retry_available == 1:
            action = WEBSOCKET_FAILURE_ACTION_FULL_CONTEXT_RETRY
            clear_affinity = True
            release_affinity = True
        elif hard_affinity == 1:
            action = WEBSOCKET_FAILURE_ACTION_PASS_THROUGH
        else:
            action = WEBSOCKET_FAILURE_ACTION_ROTATE
            stateful_failure = True
            retryable = True
            last_failure_retryable = True
        output[unsafe_offset=0] = action
        output[unsafe_offset=2] = Int64(stateful_failure)
        output[unsafe_offset=4] = Int64(retryable)
        output[unsafe_offset=7] = Int64(stateful_failure)
        output[unsafe_offset=8] = Int64(last_failure_retryable)
        output[unsafe_offset=9] = Int64(release_affinity)
        output[unsafe_offset=10] = Int64(action == WEBSOCKET_FAILURE_ACTION_PASS_THROUGH)
        return 0

    if failure_kind == WEBSOCKET_FAILURE_REJECTED or failure_kind == WEBSOCKET_FAILURE_PREVIOUS_RESPONSE:
        output[unsafe_offset=0] = WEBSOCKET_FAILURE_ACTION_PASS_THROUGH
        output[unsafe_offset=10] = 1
        return 0

    if failure_kind == WEBSOCKET_FAILURE_LOCAL_SELECTION_BLOCKED:
        if inflight_saturated == 1:
            if hard_affinity == 1:
                action = WEBSOCKET_FAILURE_ACTION_PASS_THROUGH
            else:
                action = WEBSOCKET_FAILURE_ACTION_CONTINUE
                clear_affinity = True
                release_affinity = True
        elif affinity_releasable == 1:
            action = WEBSOCKET_FAILURE_ACTION_ROTATE
            stateful_failure = True
            clear_affinity = failure_kind == WEBSOCKET_FAILURE_QUOTA or failure_kind == WEBSOCKET_FAILURE_AUTH_FAILED
            release_affinity = clear_affinity
            mark_backoff = True
        elif full_context_retry_available == 1:
            action = WEBSOCKET_FAILURE_ACTION_FULL_CONTEXT_RETRY
            clear_affinity = True
            release_affinity = True
        else:
            action = WEBSOCKET_FAILURE_ACTION_PASS_THROUGH
        output[unsafe_offset=0] = action
        output[unsafe_offset=1] = Int64(mark_backoff)
        output[unsafe_offset=2] = Int64(action == WEBSOCKET_FAILURE_ACTION_ROTATE)
        output[unsafe_offset=3] = Int64(clear_affinity)
        output[unsafe_offset=9] = Int64(release_affinity)
        output[unsafe_offset=10] = Int64(action == WEBSOCKET_FAILURE_ACTION_PASS_THROUGH)
        output[unsafe_offset=11] = Int64(clear_affinity and reset_retry_index == 1)
        return 0

    if affinity_releasable == 1 and (
        failure_kind == WEBSOCKET_FAILURE_QUOTA
        or failure_kind == WEBSOCKET_FAILURE_RATE_LIMITED
        or failure_kind == WEBSOCKET_FAILURE_AUTH_FAILED
        or failure_kind == WEBSOCKET_FAILURE_OVERLOADED
    ):
        if failure_kind == WEBSOCKET_FAILURE_QUOTA and quota_fallback_available == 0:
            action = WEBSOCKET_FAILURE_ACTION_PASS_THROUGH
        else:
            action = WEBSOCKET_FAILURE_ACTION_ROTATE
            stateful_failure = True
            clear_affinity = True
            release_affinity = True
            mark_backoff = (
                failure_kind == WEBSOCKET_FAILURE_RATE_LIMITED
                or failure_kind == WEBSOCKET_FAILURE_OVERLOADED
                or direct_current_fallback == 1
            )
            retryable = True
            # Preserve rate-limit/overload failures after candidate exhaustion;
            # quota/auth failures with an available alternate remain retryable.
            last_failure_retryable = (
                failure_kind == WEBSOCKET_FAILURE_QUOTA
                or failure_kind == WEBSOCKET_FAILURE_AUTH_FAILED
            )
            record_rate_limit = failure_kind == WEBSOCKET_FAILURE_RATE_LIMITED
            record_overload = failure_kind == WEBSOCKET_FAILURE_OVERLOADED
    elif full_context_retry_available == 1:
        action = WEBSOCKET_FAILURE_ACTION_FULL_CONTEXT_RETRY
        clear_affinity = True
        release_affinity = True
    else:
        action = WEBSOCKET_FAILURE_ACTION_PASS_THROUGH

    output[unsafe_offset=0] = action
    output[unsafe_offset=1] = Int64(mark_backoff)
    output[unsafe_offset=2] = Int64(action == WEBSOCKET_FAILURE_ACTION_ROTATE)
    output[unsafe_offset=3] = Int64(clear_affinity)
    output[unsafe_offset=4] = Int64(retryable)
    output[unsafe_offset=5] = Int64(record_rate_limit)
    output[unsafe_offset=6] = Int64(record_overload)
    output[unsafe_offset=7] = Int64(stateful_failure)
    output[unsafe_offset=8] = Int64(last_failure_retryable)
    output[unsafe_offset=9] = Int64(release_affinity)
    output[unsafe_offset=10] = Int64(action == WEBSOCKET_FAILURE_ACTION_PASS_THROUGH)
    return 0


@export("prodex_runtime_websocket_failure_frame_classification_v1")
def prodex_runtime_websocket_failure_frame_classification_v1(
    abi_version: Int64,
    http_class: Int64,
    http_action: Int64,
    connection_limit: Int64,
    previous_response_not_found: Int64,
    stream_committed: Int64,
) abi("C") -> Int64:
    if abi_version != 1:
        return -4
    if (
        http_class < 0
        or http_class > 5
        or http_action < 0
        or http_action > 2
        or not websocket_failure_bool(connection_limit)
        or not websocket_failure_bool(previous_response_not_found)
        or not websocket_failure_bool(stream_committed)
    ):
        return -1
    if connection_limit == 1:
        return WEBSOCKET_FRAME_RETRY_CONNECTION_LIMIT
    if previous_response_not_found == 1:
        return WEBSOCKET_FRAME_RETRY_PREVIOUS_RESPONSE
    if stream_committed == 1:
        return WEBSOCKET_FRAME_RETRY_NONE
    # RuntimeHttpErrorClass: quota=0, rate=1, profile=2, overload=3,
    # transient=4, other=5. RuntimeHttpErrorAction: pass=0, rotate=1,
    # retry=2. Explicit action always wins over the class label.
    if http_action == 2 and http_class == 1:
        return WEBSOCKET_FRAME_RETRY_RATE_LIMITED
    if http_action == 2 and (http_class == 3 or http_class == 4):
        return WEBSOCKET_FRAME_RETRY_OVERLOADED
    if http_action == 1 and http_class == 2:
        return WEBSOCKET_FRAME_RETRY_OVERLOADED
    if http_action == 1 and http_class == 0:
        return WEBSOCKET_FRAME_RETRY_QUOTA
    return WEBSOCKET_FRAME_RETRY_NONE


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
