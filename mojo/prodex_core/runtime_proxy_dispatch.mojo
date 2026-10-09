from rich_text import rich_trim_bounds, rich_view_ptr
from rich_types import ProdexRichStringView
from std.memory import Pointer


comptime RUNTIME_PROXY_DISPATCH_ABI_VERSION: Int64 = 1
comptime RUNTIME_PROXY_DISPATCH_OK: Int64 = 0
comptime RUNTIME_PROXY_DISPATCH_INVALID: Int64 = 1
comptime RUNTIME_PROXY_DISPATCH_ABI: Int64 = 4
comptime RUNTIME_PROXY_DISPATCH_MAX_HEADER_BYTES: Int64 = 65_536

comptime MODE_CONTENT_LENGTH: Int64 = 0
comptime MODE_ADMISSION: Int64 = 1
comptime MODE_ERROR_RESPONSE: Int64 = 2

comptime REJECTION_NONE: Int64 = 0
comptime REJECTION_GLOBAL: Int64 = 1
comptime REJECTION_LANE: Int64 = 2

comptime LANE_RESPONSES: Int64 = 0
comptime LANE_COMPACT: Int64 = 1
comptime LANE_WEBSOCKET: Int64 = 2
comptime LANE_STANDARD: Int64 = 3

comptime ADMISSION_ALLOW: UInt64 = 0
comptime ADMISSION_REJECT: UInt64 = 1
comptime ADMISSION_CAPTURE_AND_RETRY: UInt64 = 2

comptime ERROR_CAPTURE_TOO_LARGE: Int64 = 0
comptime ERROR_CAPTURE_OTHER: Int64 = 1
comptime ERROR_UPSTREAM_TRANSPORT: Int64 = 2
comptime ERROR_UPSTREAM_REWRITE: Int64 = 3

comptime RESPONSE_BODY_TOO_LARGE: UInt64 = 0
comptime RESPONSE_CAPTURE_FAILED: UInt64 = 1
comptime RESPONSE_TRANSPORT_FAILED: UInt64 = 2
comptime RESPONSE_REWRITE_FAILED: UInt64 = 3

comptime U64_MAX: UInt64 = 18_446_744_073_709_551_615


def runtime_proxy_dispatch_header_value(
    input_address: UInt,
    input_length: Int64,
    output: Pointer[mut=True, UInt64, MutUntrackedOrigin],
) -> Bool:
    var view = ProdexRichStringView(input_address, UInt(input_length))
    var bounds = rich_trim_bounds(view)
    var start = bounds[0]
    var end = bounds[1]
    var source = rich_view_ptr(view)
    if start == end:
        return False
    if source[unsafe_offset=start] == 43:
        start += 1
    if start == end:
        return False

    var value: UInt64 = 0
    for index in range(start, end):
        var byte = source[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt64(byte - 48)
        if value > U64_MAX // UInt64(10) or (
            value == U64_MAX // UInt64(10) and digit > U64_MAX % UInt64(10)
        ):
            return False
        value = value * UInt64(10) + digit
    output[unsafe_offset=1] = value
    return True


@export("prodex_runtime_proxy_dispatch_policy_v1")
def prodex_runtime_proxy_dispatch_policy_v1(
    abi_version: Int64,
    mode: Int64,
    rejection_kind: Int64,
    lane_kind: Int64,
    websocket: Int64,
    capture_attempted: Int64,
    error_kind: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_PROXY_DISPATCH_ABI_VERSION:
        return RUNTIME_PROXY_DISPATCH_ABI
    if mode < MODE_CONTENT_LENGTH or mode > MODE_ERROR_RESPONSE or output_address == 0:
        return RUNTIME_PROXY_DISPATCH_INVALID
    if (
        rejection_kind < REJECTION_NONE
        or rejection_kind > REJECTION_LANE
        or lane_kind < LANE_RESPONSES
        or lane_kind > LANE_STANDARD
        or websocket < 0
        or websocket > 1
        or capture_attempted < 0
        or capture_attempted > 1
        or error_kind < ERROR_CAPTURE_TOO_LARGE
        or error_kind > ERROR_UPSTREAM_REWRITE
    ):
        return RUNTIME_PROXY_DISPATCH_INVALID
    if input_length < 0 or input_length > RUNTIME_PROXY_DISPATCH_MAX_HEADER_BYTES or (
        input_length > 0 and input_address == 0
    ):
        return RUNTIME_PROXY_DISPATCH_INVALID

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    output[unsafe_offset=2] = 0
    output[unsafe_offset=3] = 0

    if mode == MODE_CONTENT_LENGTH:
        if input_length == 0:
            return RUNTIME_PROXY_DISPATCH_OK
        output[unsafe_offset=0] = UInt64(
            runtime_proxy_dispatch_header_value(input_address, input_length, output)
        )
        return RUNTIME_PROXY_DISPATCH_OK

    if mode == MODE_ADMISSION:
        if rejection_kind == REJECTION_NONE:
            output[unsafe_offset=0] = ADMISSION_ALLOW
        elif rejection_kind == REJECTION_LANE and websocket == 0 and capture_attempted == 0:
            output[unsafe_offset=0] = ADMISSION_CAPTURE_AND_RETRY
        else:
            output[unsafe_offset=0] = ADMISSION_REJECT
        output[unsafe_offset=1] = UInt64(
            rejection_kind == REJECTION_GLOBAL
            or (rejection_kind == REJECTION_LANE and lane_kind == LANE_RESPONSES)
        )
        return RUNTIME_PROXY_DISPATCH_OK

    if error_kind == ERROR_CAPTURE_TOO_LARGE:
        output[unsafe_offset=0] = RESPONSE_BODY_TOO_LARGE
    elif error_kind == ERROR_CAPTURE_OTHER:
        output[unsafe_offset=0] = RESPONSE_CAPTURE_FAILED
    elif error_kind == ERROR_UPSTREAM_TRANSPORT:
        output[unsafe_offset=0] = RESPONSE_TRANSPORT_FAILED
    else:
        output[unsafe_offset=0] = RESPONSE_REWRITE_FAILED
    return RUNTIME_PROXY_DISPATCH_OK
