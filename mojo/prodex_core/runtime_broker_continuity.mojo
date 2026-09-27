from std.memory import Pointer

from rich_text import rich_view_valid
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
