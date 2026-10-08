from std.memory import Pointer

from rich_text import rich_view_valid, rich_views_equal
from rich_types import ProdexRichStringView, rich_view_ptr

comptime BROKER_CONTINUITY_ABI_VERSION: Int64 = 1
comptime BROKER_CONTINUITY_INVALID: Int64 = -1
comptime INT64_MAX: Int64 = 9223372036854775807
comptime INT64_MIN: Int64 = -9223372036854775808
comptime U32_MAX_I64: Int64 = 4294967295


def broker_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


def broker_ascii_whitespace(value: UInt8) -> Bool:
    return value == 9 or value == 10 or value == 11 or value == 12 or value == 13 or value == 32


def broker_trim_event_punctuation(value: UInt8) -> Bool:
    return (
        value == 34
        or value == 39
        or value == 44
        or value == 91
        or value == 93
        or value == 123
        or value == 125
        or value == 40
        or value == 41
    )


def broker_literal_equal(
    source: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    if end - start != Int64(literal.byte_length()):
        return False
    var target = literal.unsafe_ptr()
    for index in range(end - start):
        if source[unsafe_offset=start + index] != target[unsafe_offset=index]:
            return False
    return True


def broker_known_event(
    source: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var lower = start
    var upper = end
    while lower < upper and broker_trim_event_punctuation(source[unsafe_offset=lower]):
        lower += 1
    while upper > lower and broker_trim_event_punctuation(source[unsafe_offset=upper - 1]):
        upper -= 1
    if broker_literal_equal(source, lower, upper, StringSlice("chain_retried_owner")):
        return 1
    if broker_literal_equal(source, lower, upper, StringSlice("chain_dead_upstream_confirmed")):
        return 2
    if broker_literal_equal(source, lower, upper, StringSlice("stale_continuation")):
        return 3
    return 0


def broker_scan_event(view: ProdexRichStringView) -> Int64:
    var source = rich_view_ptr(view)
    var length = Int64(view.len)
    var index: Int64 = 0
    while index < length:
        while index < length and broker_ascii_whitespace(source[unsafe_offset=index]):
            index += 1
        var start = index
        while index < length and not broker_ascii_whitespace(source[unsafe_offset=index]):
            index += 1
        if start < index:
            var event = broker_known_event(source, start, index)
            if event != 0:
                return event
    return 0


def broker_find_reason(
    view: ProdexRichStringView,
    output_start: Pointer[mut=True, Int64, _],
    output_length: Pointer[mut=True, Int64, _],
) -> Bool:
    var source = rich_view_ptr(view)
    var length = Int64(view.len)
    var index: Int64 = 0
    while index < length:
        while index < length and broker_ascii_whitespace(source[unsafe_offset=index]):
            index += 1
        if index >= length:
            break
        var key_start = index
        while (
            index < length
            and not broker_ascii_whitespace(source[unsafe_offset=index])
            and source[unsafe_offset=index] != 61
        ):
            index += 1
        if index >= length or source[unsafe_offset=index] != 61:
            while index < length and not broker_ascii_whitespace(source[unsafe_offset=index]):
                index += 1
            continue
        var key_end = index
        index += 1
        var value_start = index
        if broker_literal_equal(source, key_start, key_end, StringSlice("reason")):
            if value_start < length and source[unsafe_offset=value_start] == 34:
                index += 1
                var escaped = False
                while index < length:
                    var value = source[unsafe_offset=index]
                    if escaped:
                        escaped = False
                        index += 1
                    elif value == 92:
                        escaped = True
                        index += 1
                    elif value == 34:
                        index += 1
                        break
                    else:
                        index += 1
            else:
                while index < length and not broker_ascii_whitespace(source[unsafe_offset=index]):
                    index += 1
            output_start[] = value_start
            output_length[] = index - value_start
            return True
        if value_start < length and source[unsafe_offset=value_start] == 34:
            index += 1
            var escaped = False
            while index < length:
                var value = source[unsafe_offset=index]
                if escaped:
                    escaped = False
                    index += 1
                elif value == 92:
                    escaped = True
                    index += 1
                elif value == 34:
                    index += 1
                    break
                else:
                    index += 1
        else:
            while index < length and not broker_ascii_whitespace(source[unsafe_offset=index]):
                index += 1
    return False


def broker_valid_optional_view(
    present: Int64, address: UInt, length: Int64
) -> Bool:
    if present != 0 and present != 1 or length < 0:
        return False
    if present == 0:
        return length == 0
    if length > 0 and address == 0:
        return False
    return rich_view_valid(broker_view(address, length), length)


@export("prodex_runtime_broker_continuity_line_v1")
def prodex_runtime_broker_continuity_line_v1(
    abi_version: Int64,
    raw_address: UInt,
    raw_length: Int64,
    event_present: Int64,
    event_address: UInt,
    event_length: Int64,
    reason_present: Int64,
    reason_address: UInt,
    reason_length: Int64,
    message_present: Int64,
    message_address: UInt,
    message_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or output_address == 0
        or raw_length < 0
        or (raw_length > 0 and raw_address == 0)
        or not broker_valid_optional_view(event_present, event_address, event_length)
        or not broker_valid_optional_view(reason_present, reason_address, reason_length)
        or not broker_valid_optional_view(message_present, message_address, message_length)
    ):
        return BROKER_CONTINUITY_INVALID
    var raw = broker_view(raw_address, raw_length)
    if not rich_view_valid(raw, raw_length):
        return BROKER_CONTINUITY_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(4):
        output[unsafe_offset=index] = 0

    var event: Int64 = 0
    if event_present == 1:
        var event_view = broker_view(event_address, event_length)
        event = broker_known_event(rich_view_ptr(event_view), 0, event_length)
    if event == 0 and message_present == 1:
        event = broker_scan_event(broker_view(message_address, message_length))
    if event == 0:
        event = broker_scan_event(raw)
    output[0] = event

    if reason_present == 1:
        output[1] = 1
        output[2] = 0
        output[3] = reason_length
        return 0

    var reason_start: Int64 = 0
    var reason_len: Int64 = 0
    if (
        message_present == 1
        and broker_find_reason(
            broker_view(message_address, message_length),
            Pointer(to=reason_start),
            Pointer(to=reason_len),
        )
    ):
        output[1] = 2
        output[2] = reason_start
        output[3] = reason_len
        return 0

    if broker_find_reason(raw, Pointer(to=reason_start), Pointer(to=reason_len)):
        output[1] = 3
        output[2] = reason_start
        output[3] = reason_len
    return 0


def broker_saturating_sub(left: Int64, right: Int64) -> Int64:
    if right > 0 and left < INT64_MIN + right:
        return INT64_MIN
    if right < 0 and left > INT64_MAX + right:
        return INT64_MAX
    return left - right


@export("prodex_runtime_broker_effective_score_v1")
def prodex_runtime_broker_effective_score_v1(
    abi_version: Int64,
    score: Int64,
    updated_at: Int64,
    now: Int64,
    decay_seconds: Int64,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or score < 0
        or score > U32_MAX_I64
    ):
        return BROKER_CONTINUITY_INVALID
    var divisor = max(decay_seconds, Int64(1))
    var decay = broker_saturating_sub(now, updated_at) // divisor
    decay = min(max(decay, Int64(0)), U32_MAX_I64)
    return max(score - decay, Int64(0))


@export("prodex_runtime_broker_stale_verified_v1")
def prodex_runtime_broker_stale_verified_v1(
    abi_version: Int64,
    verified: Int64,
    not_found_present: Int64,
    last_not_found_at: Int64,
    verified_present: Int64,
    last_verified_at: Int64,
    touched_present: Int64,
    last_touched_at: Int64,
    now: Int64,
    stale_verified_seconds: Int64,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or (verified != 0 and verified != 1)
        or (not_found_present != 0 and not_found_present != 1)
        or (verified_present != 0 and verified_present != 1)
        or (touched_present != 0 and touched_present != 1)
    ):
        return BROKER_CONTINUITY_INVALID
    if verified == 0:
        return 0
    var present = False
    var last: Int64 = INT64_MIN
    if not_found_present == 1:
        present = True
        last = max(last, last_not_found_at)
    if verified_present == 1:
        present = True
        last = max(last, last_verified_at)
    if touched_present == 1:
        present = True
        last = max(last, last_touched_at)
    if not present:
        return 0
    return Int64(broker_saturating_sub(now, last) >= stale_verified_seconds)


@export("prodex_runtime_broker_route_kind_v1")
def prodex_runtime_broker_route_kind_v1(
    abi_version: Int64,
    route_address: UInt,
    route_length: Int64,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or route_length < 0
        or (route_length > 0 and route_address == 0)
    ):
        return BROKER_CONTINUITY_INVALID
    var view = broker_view(route_address, route_length)
    if not rich_view_valid(view, route_length):
        return BROKER_CONTINUITY_INVALID
    var ptr = rich_view_ptr(view)
    if broker_literal_equal(ptr, 0, route_length, StringSlice("responses")):
        return 1
    if broker_literal_equal(ptr, 0, route_length, StringSlice("compact")):
        return 2
    if broker_literal_equal(ptr, 0, route_length, StringSlice("websocket")):
        return 3
    if broker_literal_equal(ptr, 0, route_length, StringSlice("standard")):
        return 4
    return 0


@export("prodex_runtime_broker_health_key_kind_v1")
def prodex_runtime_broker_health_key_kind_v1(
    abi_version: Int64,
    key_address: UInt,
    key_length: Int64,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or key_length < 0
        or (key_length > 0 and key_address == 0)
    ):
        return BROKER_CONTINUITY_INVALID
    var view = broker_view(key_address, key_length)
    if not rich_view_valid(view, key_length):
        return BROKER_CONTINUITY_INVALID
    var ptr = rich_view_ptr(view)
    if key_length >= 17 and broker_literal_equal(
        ptr, 0, 17, StringSlice("__route_health__:")
    ):
        return 1
    if key_length >= 2 and ptr[0] == 95 and ptr[1] == 95:
        return 0
    return 2


comptime BROKER_REGISTRY_STATUS_NOT_LEGACY: Int64 = 0
comptime BROKER_REGISTRY_STATUS_VALID_LEGACY: Int64 = 1
comptime BROKER_REGISTRY_STATUS_MALFORMED: Int64 = 2
comptime BROKER_REGISTRY_STATUS_TOO_LARGE: Int64 = 3


@export("prodex_runtime_broker_registry_store_plan_v1")
def prodex_runtime_broker_registry_store_plan_v1(
    abi_version: Int64,
    primary_exists: Int64,
    primary_status: Int64,
    backup_status: Int64,
    primary_current: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or (primary_exists != 0 and primary_exists != 1)
        or primary_status < BROKER_REGISTRY_STATUS_NOT_LEGACY
        or primary_status > BROKER_REGISTRY_STATUS_TOO_LARGE
        or backup_status < BROKER_REGISTRY_STATUS_NOT_LEGACY
        or backup_status > BROKER_REGISTRY_STATUS_TOO_LARGE
        or (primary_current != 0 and primary_current != 1)
        or output_address == 0
    ):
        return BROKER_CONTINUITY_INVALID

    var action: Int64 = 0
    if (
        backup_status == BROKER_REGISTRY_STATUS_VALID_LEGACY
        and primary_status == BROKER_REGISTRY_STATUS_NOT_LEGACY
        and primary_current == 1
    ):
        action = 2
    elif (
        primary_status == BROKER_REGISTRY_STATUS_VALID_LEGACY
        or (
            backup_status == BROKER_REGISTRY_STATUS_VALID_LEGACY
            and primary_exists == 0
        )
    ):
        action = 1

    var error_source: Int64 = 0
    if primary_status == BROKER_REGISTRY_STATUS_TOO_LARGE:
        error_source = 1
    elif backup_status == BROKER_REGISTRY_STATUS_TOO_LARGE:
        error_source = 2

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = action
    output[unsafe_offset=1] = error_source
    return 0


@export("prodex_runtime_broker_registry_identity_match_v1")
def prodex_runtime_broker_registry_identity_match_v1(
    abi_version: Int64,
    left_address: UInt,
    left_length: Int64,
    right_address: UInt,
    right_length: Int64,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or left_length < 0
        or right_length < 0
        or (left_length > 0 and left_address == 0)
        or (right_length > 0 and right_address == 0)
    ):
        return BROKER_CONTINUITY_INVALID
    var left = broker_view(left_address, left_length)
    var right = broker_view(right_address, right_length)
    if not rich_view_valid(left, left_length) or not rich_view_valid(right, right_length):
        return BROKER_CONTINUITY_INVALID
    if left_length != right_length:
        return 0
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    for index in range(left_length):
        if left_ptr[unsafe_offset=index] != right_ptr[unsafe_offset=index]:
            return 0
    return 1


@export("prodex_runtime_broker_registry_reuse_plan_v1")
def prodex_runtime_broker_registry_reuse_plan_v1(
    abi_version: Int64,
    registry_upstream_address: UInt,
    registry_upstream_length: Int64,
    registry_include_code_review: Int64,
    registry_upstream_no_proxy: Int64,
    registry_smart_context_enabled: Int64,
    launch_upstream_address: UInt,
    launch_upstream_length: Int64,
    launch_include_code_review: Int64,
    launch_upstream_no_proxy: Int64,
    launch_smart_context_enabled: Int64,
    health_present: Int64,
    health_matches: Int64,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or registry_upstream_length < 0
        or launch_upstream_length < 0
        or (registry_upstream_length > 0 and registry_upstream_address == 0)
        or (launch_upstream_length > 0 and launch_upstream_address == 0)
        or not rich_view_valid(
            broker_view(registry_upstream_address, registry_upstream_length),
            registry_upstream_length,
        )
        or not rich_view_valid(
            broker_view(launch_upstream_address, launch_upstream_length),
            launch_upstream_length,
        )
        or (registry_include_code_review != 0 and registry_include_code_review != 1)
        or (registry_upstream_no_proxy != 0 and registry_upstream_no_proxy != 1)
        or (registry_smart_context_enabled != 0 and registry_smart_context_enabled != 1)
        or (launch_include_code_review != 0 and launch_include_code_review != 1)
        or (launch_upstream_no_proxy != 0 and launch_upstream_no_proxy != 1)
        or (launch_smart_context_enabled != 0 and launch_smart_context_enabled != 1)
        or (health_present != 0 and health_present != 1)
        or (health_matches != 0 and health_matches != 1)
        or (health_matches == 1 and health_present == 0)
    ):
        return BROKER_CONTINUITY_INVALID

    var registry_upstream = broker_view(
        registry_upstream_address, registry_upstream_length
    )
    var launch_upstream = broker_view(launch_upstream_address, launch_upstream_length)
    if (
        not rich_views_equal(registry_upstream, launch_upstream)
        or registry_include_code_review != launch_include_code_review
        or registry_upstream_no_proxy != launch_upstream_no_proxy
        or registry_smart_context_enabled != launch_smart_context_enabled
    ):
        return 1
    if health_present == 1 and health_matches == 1:
        return 0
    return 2


@export("prodex_runtime_broker_startup_grace_seconds_v1")
def prodex_runtime_broker_startup_grace_seconds_v1(
    abi_version: Int64,
    ready_timeout_ms: UInt64,
    idle_grace_seconds: Int64,
) abi("C") -> Int64:
    if abi_version != BROKER_CONTINUITY_ABI_VERSION:
        return BROKER_CONTINUITY_INVALID
    var ready_timeout_seconds = ready_timeout_ms // UInt64(1_000)
    if ready_timeout_ms % UInt64(1_000) != 0:
        ready_timeout_seconds += 1
    return max(Int64(ready_timeout_seconds) + 1, idle_grace_seconds)

comptime BROKER_ID_VERSION: Int64 = 1
comptime BROKER_ID_PATH: Int64 = 2
comptime BROKER_ID_SHA: Int64 = 4

def broker_identity_valid(
    flags: Int64,
    version_address: UInt,
    version_length: Int64,
    sha_address: UInt,
    sha_length: Int64,
) -> Bool:
    if flags < 0 or flags > 7 or version_length < 0 or sha_length < 0:
        return False
    if (flags & BROKER_ID_VERSION) != 0:
        if not rich_view_valid(broker_view(version_address, version_length), version_length):
            return False
    elif version_length != 0:
        return False
    if (flags & BROKER_ID_SHA) != 0:
        if not rich_view_valid(broker_view(sha_address, sha_length), sha_length):
            return False
    elif sha_length != 0:
        return False
    return True

def broker_equal_bytes(
    left_address: UInt,
    left_length: Int64,
    right_address: UInt,
    right_length: Int64,
) -> Bool:
    if left_length != right_length:
        return False
    var left = rich_view_ptr(broker_view(left_address, left_length))
    var right = rich_view_ptr(broker_view(right_address, right_length))
    for index in range(left_length):
        if left[unsafe_offset=index] != right[unsafe_offset=index]:
            return False
    return True

def broker_identity_matches(
    current_flags: Int64,
    current_version_address: UInt,
    current_version_length: Int64,
    current_sha_address: UInt,
    current_sha_length: Int64,
    other_flags: Int64,
    other_version_address: UInt,
    other_version_length: Int64,
    other_sha_address: UInt,
    other_sha_length: Int64,
) -> Bool:
    if (current_flags & BROKER_ID_SHA) != 0 and (other_flags & BROKER_ID_SHA) != 0:
        return broker_equal_bytes(
            current_sha_address,
            current_sha_length,
            other_sha_address,
            other_sha_length,
        )
    if (
        (current_flags & BROKER_ID_VERSION) != 0
        and (other_flags & BROKER_ID_VERSION) != 0
    ):
        return broker_equal_bytes(
            current_version_address,
            current_version_length,
            other_version_address,
            other_version_length,
        )
    return False

def broker_identity_version_mismatch(
    current_flags: Int64,
    current_version_address: UInt,
    current_version_length: Int64,
    observed_flags: Int64,
    observed_version_address: UInt,
    observed_version_length: Int64,
) -> Bool:
    if (
        (current_flags & BROKER_ID_VERSION) == 0
        or (observed_flags & BROKER_ID_VERSION) == 0
    ):
        return False
    return not broker_equal_bytes(
        current_version_address,
        current_version_length,
        observed_version_address,
        observed_version_length,
    )

def broker_identity_replacement_reason(
    current_flags: Int64,
    current_version_address: UInt,
    current_version_length: Int64,
    current_sha_address: UInt,
    current_sha_length: Int64,
    observed_flags: Int64,
    observed_version_address: UInt,
    observed_version_length: Int64,
    observed_sha_address: UInt,
    observed_sha_length: Int64,
) -> Int64:
    if (
        (current_flags & BROKER_ID_SHA) != 0
        and (observed_flags & BROKER_ID_SHA) != 0
        and not broker_equal_bytes(
            current_sha_address,
            current_sha_length,
            observed_sha_address,
            observed_sha_length,
        )
    ):
        return 1
    if (
        (current_flags & BROKER_ID_VERSION) != 0
        and (observed_flags & BROKER_ID_VERSION) != 0
        and not broker_equal_bytes(
            current_version_address,
            current_version_length,
            observed_version_address,
            observed_version_length,
        )
    ):
        return 2
    return 3 if observed_flags != 0 else 4

@export("prodex_runtime_broker_identity_policy_v1")
def prodex_runtime_broker_identity_policy_v1(
    abi_version: Int64,
    mode: Int64,
    left_flags: Int64,
    left_version_address: UInt,
    left_version_length: Int64,
    left_sha_address: UInt,
    left_sha_length: Int64,
    right_flags: Int64,
    right_version_address: UInt,
    right_version_length: Int64,
    right_sha_address: UInt,
    right_sha_length: Int64,
) abi("C") -> Int64:
    if abi_version != BROKER_CONTINUITY_ABI_VERSION or mode < 0 or mode > 3:
        return BROKER_CONTINUITY_INVALID
    if not broker_identity_valid(
        left_flags,
        left_version_address,
        left_version_length,
        left_sha_address,
        left_sha_length,
    ):
        return BROKER_CONTINUITY_INVALID
    if mode != 0 and not broker_identity_valid(
        right_flags,
        right_version_address,
        right_version_length,
        right_sha_address,
        right_sha_length,
    ):
        return BROKER_CONTINUITY_INVALID
    if mode == 0:
        return Int64(left_flags != 0)
    if mode == 1:
        return Int64(
            broker_identity_matches(
                left_flags,
                left_version_address,
                left_version_length,
                left_sha_address,
                left_sha_length,
                right_flags,
                right_version_address,
                right_version_length,
                right_sha_address,
                right_sha_length,
            )
        )
    if mode == 2:
        return broker_identity_replacement_reason(
            left_flags,
            left_version_address,
            left_version_length,
            left_sha_address,
            left_sha_length,
            right_flags,
            right_version_address,
            right_version_length,
            right_sha_address,
            right_sha_length,
        )
    return Int64(
        broker_identity_version_mismatch(
            left_flags,
            left_version_address,
            left_version_length,
            right_flags,
            right_version_address,
            right_version_length,
        )
    )

@export("prodex_runtime_broker_guard_plan_v1")
def prodex_runtime_broker_guard_plan_v1(
    abi_version: Int64,
    process_alive: Int64,
    binary_flags: Int64,
    binary_version_address: UInt,
    binary_version_length: Int64,
    binary_sha_address: UInt,
    binary_sha_length: Int64,
    version_flags: Int64,
    version_version_address: UInt,
    version_version_length: Int64,
    version_sha_address: UInt,
    version_sha_length: Int64,
    observed_flags: Int64,
    observed_version_address: UInt,
    observed_version_length: Int64,
    observed_sha_address: UInt,
    observed_sha_length: Int64,
    active_requests: UInt64,
    live_leases: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or (process_alive != 0 and process_alive != 1)
        or output_address == 0
        or not broker_identity_valid(
            binary_flags,
            binary_version_address,
            binary_version_length,
            binary_sha_address,
            binary_sha_length,
        )
        or not broker_identity_valid(
            version_flags,
            version_version_address,
            version_version_length,
            version_sha_address,
            version_sha_length,
        )
        or not broker_identity_valid(
            observed_flags,
            observed_version_address,
            observed_version_length,
            observed_sha_address,
            observed_sha_length,
        )
    ):
        return BROKER_CONTINUITY_INVALID

    var use_version = broker_identity_version_mismatch(
        version_flags,
        version_version_address,
        version_version_length,
        observed_flags,
        observed_version_address,
        observed_version_length,
    )

    var current_flags = binary_flags
    var current_version_address = binary_version_address
    var current_version_length = binary_version_length
    var current_sha_address = binary_sha_address
    var current_sha_length = binary_sha_length
    if use_version:
        current_flags = version_flags
        current_version_address = version_version_address
        current_version_length = version_version_length
        current_sha_address = version_sha_address
        current_sha_length = version_sha_length

    var outcome: Int64 = 2
    var reason: Int64 = 0
    if process_alive == 0 or (
        observed_flags != 0
        and broker_identity_matches(
            current_flags,
            current_version_address,
            current_version_length,
            current_sha_address,
            current_sha_length,
            observed_flags,
            observed_version_address,
            observed_version_length,
            observed_sha_address,
            observed_sha_length,
        )
    ):
        outcome = 0
    elif active_requests > 0 or live_leases > 0:
        outcome = 1
    else:
        reason = broker_identity_replacement_reason(
            current_flags,
            current_version_address,
            current_version_length,
            current_sha_address,
            current_sha_length,
            observed_flags,
            observed_version_address,
            observed_version_length,
            observed_sha_address,
            observed_sha_length,
        )

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = outcome
    output[unsafe_offset=1] = Int64(use_version)
    output[unsafe_offset=2] = reason
    return 0

@export("prodex_runtime_broker_parse_version_v1")
def prodex_runtime_broker_parse_version_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or length < 0
        or (length > 0 and address == 0)
        or output_address == 0
    ):
        return BROKER_CONTINUITY_INVALID
    var view = broker_view(address, length)
    if not rich_view_valid(view, length):
        return BROKER_CONTINUITY_INVALID
    var ptr = rich_view_ptr(view)
    var cursor: Int64 = 0
    while cursor < length and broker_ascii_whitespace(ptr[unsafe_offset=cursor]):
        cursor += 1
    var name_start = cursor
    while cursor < length and not broker_ascii_whitespace(ptr[unsafe_offset=cursor]):
        cursor += 1
    var name_end = cursor
    if not broker_literal_equal(ptr, name_start, name_end, StringSlice("prodex")):
        return 0
    while cursor < length and broker_ascii_whitespace(ptr[unsafe_offset=cursor]):
        cursor += 1
    var version_start = cursor
    while cursor < length and not broker_ascii_whitespace(ptr[unsafe_offset=cursor]):
        cursor += 1
    var version_end = cursor
    if version_end <= version_start:
        return 0

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = version_start
    output[unsafe_offset=1] = version_end
    return 1


def broker_duration_less(
    left_seconds: UInt64,
    left_nanoseconds: UInt64,
    right_seconds: UInt64,
    right_nanoseconds: UInt64,
) -> Bool:
    return (
        left_seconds < right_seconds
        or (
            left_seconds == right_seconds
            and left_nanoseconds < right_nanoseconds
        )
    )


@export("prodex_runtime_broker_log_cache_relation_v1")
def prodex_runtime_broker_log_cache_relation_v1(
    abi_version: Int64,
    current_len: UInt64,
    current_modified_seconds: UInt64,
    current_modified_nanoseconds: UInt64,
    previous_present: Int64,
    previous_len: UInt64,
    previous_modified_seconds: UInt64,
    previous_modified_nanoseconds: UInt64,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or (previous_present != 0 and previous_present != 1)
        or current_modified_nanoseconds >= UInt64(1_000_000_000)
        or previous_modified_nanoseconds >= UInt64(1_000_000_000)
    ):
        return BROKER_CONTINUITY_INVALID
    if previous_present == 0:
        return 0

    var modified_equal = (
        current_modified_seconds == previous_modified_seconds
        and current_modified_nanoseconds == previous_modified_nanoseconds
    )
    if current_len == previous_len and modified_equal:
        return 1

    var modified_before = broker_duration_less(
        current_modified_seconds,
        current_modified_nanoseconds,
        previous_modified_seconds,
        previous_modified_nanoseconds,
    )
    if current_len < previous_len or modified_before:
        return 3

    if current_len > previous_len:
        return 2

    return 0


@export("prodex_runtime_broker_lru_evict_index_v1")
def prodex_runtime_broker_lru_evict_index_v1(
    abi_version: Int64,
    touches_address: UInt,
    count: Int64,
    keep_index: Int64,
) abi("C") -> Int64:
    if (
        abi_version != BROKER_CONTINUITY_ABI_VERSION
        or count < 0
        or count > 1_000_000
        or (count > 0 and touches_address == 0)
        or keep_index < -1
        or keep_index >= count
    ):
        return BROKER_CONTINUITY_INVALID
    if count == 0:
        return -2

    var touches = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(touches_address)
    )
    var selected: Int64 = -1
    var selected_touch: UInt64 = 0
    for index in range(count):
        if index == keep_index:
            continue
        var touch = touches[unsafe_offset=index]
        if selected < 0 or touch < selected_touch:
            selected = index
            selected_touch = touch

    if selected >= 0:
        return selected

    # Preserve the Rust fallback that may evict the kept entry when it is the
    # only entry left above a pathological limit.
    return 0
