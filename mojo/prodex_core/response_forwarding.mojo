
from std.memory import Pointer


comptime RESPONSE_FORWARDING_SKIP_HEADER: Int64 = 0
comptime RESPONSE_FORWARDING_CONTENT_TYPE_SSE: Int64 = 1
comptime RESPONSE_FORWARDING_USAGE_EVENT_LOGGABLE: Int64 = 2
comptime RESPONSE_FORWARDING_GENERATION_START: Int64 = 3


def response_ascii_whitespace(value: UInt8) -> Bool:
    return value == 9 or value == 10 or value == 13 or value == 32


def response_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def response_text_ptr(address: UInt) -> Pointer[mut=False, UInt8, ImmUntrackedOrigin]:
    return Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(address))


def response_equals(
    address: UInt,
    length: Int64,
    literal: StringSlice,
) -> Bool:
    var expected = Int64(literal.byte_length())
    if length != expected:
        return False
    if length == 0:
        return True
    if address == 0:
        return False
    var source = response_text_ptr(address)
    var target = literal.unsafe_ptr()
    for index in range(length):
        if source[unsafe_offset=index] != target[unsafe_offset=index]:
            return False
    return True


def response_equals_ci_range(
    address: UInt,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var expected = Int64(literal.byte_length())
    if end - start != expected:
        return False
    if expected == 0:
        return True
    if address == 0:
        return False
    var source = response_text_ptr(address)
    var target = literal.unsafe_ptr()
    for offset in range(expected):
        if response_ascii_lower(source[unsafe_offset=start + offset]) != target[unsafe_offset=offset]:
            return False
    return True


def response_contains_ci(
    address: UInt,
    length: Int64,
    literal: StringSlice,
) -> Bool:
    var expected = Int64(literal.byte_length())
    if expected == 0:
        return True
    if address == 0 or length < expected:
        return False
    var source = response_text_ptr(address)
    var target = literal.unsafe_ptr()
    var start: Int64 = 0
    while start + expected <= length:
        var matched = True
        for offset in range(expected):
            if response_ascii_lower(source[unsafe_offset=start + offset]) != target[unsafe_offset=offset]:
                matched = False
                break
        if matched:
            return True
        start += 1
    return False


def response_ends_with(
    address: UInt,
    length: Int64,
    literal: StringSlice,
) -> Bool:
    var expected = Int64(literal.byte_length())
    if address == 0 or length < expected:
        return False
    var source = response_text_ptr(address)
    var target = literal.unsafe_ptr()
    var start = length - expected
    for offset in range(expected):
        if source[unsafe_offset=start + offset] != target[unsafe_offset=offset]:
            return False
    return True


def response_generation_start(address: UInt, length: Int64) -> Bool:
    return (
        response_equals(address, length, StringSlice("response.output_text.delta"))
        or response_equals(address, length, StringSlice("response.refusal.delta"))
        or response_equals(
            address, length, StringSlice("response.reasoning_summary_text.delta")
        )
        or response_equals(address, length, StringSlice("response.reasoning_text.delta"))
        or response_equals(
            address, length, StringSlice("response.function_call_arguments.delta")
        )
        or response_equals(
            address, length, StringSlice("response.mcp_call_arguments.delta")
        )
        or response_equals(
            address, length, StringSlice("response.custom_tool_call_input.delta")
        )
    )


@export("prodex_runtime_response_forwarding_classify_v1")
def prodex_runtime_response_forwarding_classify_v1(
    operation: Int64,
    address: UInt,
    length: Int64,
    present: Int64,
    _numeric: UInt64,
) abi("C") -> Int64:
    if (
        operation < RESPONSE_FORWARDING_SKIP_HEADER
        or operation > RESPONSE_FORWARDING_GENERATION_START
        or present < 0
        or present > 1
        or length < 0
        or (present == 1 and length > 0 and address == 0)
    ):
        return -1

    if operation == RESPONSE_FORWARDING_SKIP_HEADER:
        if present == 0:
            return 0
        var start: Int64 = 0
        var end = length
        var ptr = response_text_ptr(address)
        while start < end and response_ascii_whitespace(ptr[unsafe_offset=start]):
            start += 1
        while end > start and response_ascii_whitespace(ptr[unsafe_offset=end - 1]):
            end -= 1
        if (
            response_equals_ci_range(address, start, end, StringSlice("connection"))
            or response_equals_ci_range(address, start, end, StringSlice("content-length"))
            or response_equals_ci_range(address, start, end, StringSlice("keep-alive"))
            or response_equals_ci_range(
                address, start, end, StringSlice("proxy-authenticate")
            )
            or response_equals_ci_range(
                address, start, end, StringSlice("proxy-authorization")
            )
            or response_equals_ci_range(address, start, end, StringSlice("te"))
            or response_equals_ci_range(address, start, end, StringSlice("trailer"))
            or response_equals_ci_range(
                address, start, end, StringSlice("transfer-encoding")
            )
            or response_equals_ci_range(address, start, end, StringSlice("upgrade"))
        ):
            return 1
        return 0

    if operation == RESPONSE_FORWARDING_CONTENT_TYPE_SSE:
        if present == 0:
            return 0
        return Int64(
            response_contains_ci(address, length, StringSlice("text/event-stream"))
        )

    if operation == RESPONSE_FORWARDING_USAGE_EVENT_LOGGABLE:
        if present == 0:
            return 1
        return Int64(
            response_equals(address, length, StringSlice("response.completed"))
            or response_equals(address, length, StringSlice("response.failed"))
            or response_ends_with(address, length, StringSlice(".completed"))
        )

    return Int64(present == 1 and response_generation_start(address, length))
