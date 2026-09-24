
from std.memory import Pointer

comptime SSE_LINE_BLANK: Int64 = 0
comptime SSE_LINE_IGNORE: Int64 = 1
comptime SSE_LINE_DATA: Int64 = 2


@export("prodex_runtime_sse_line_plan_v1")
def prodex_runtime_sse_line_plan_v1(
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if length < 0 or output_address == 0 or (length > 0 and address == 0):
        return 1

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var end = length
    while end > 0 and (
        ptr[unsafe_offset=end - 1] == 13 or ptr[unsafe_offset=end - 1] == 10
    ):
        end -= 1

    output[unsafe_offset=0] = SSE_LINE_IGNORE
    output[unsafe_offset=1] = end
    output[unsafe_offset=2] = end

    if end == 0:
        output[unsafe_offset=0] = SSE_LINE_BLANK
        return 0
    if ptr[unsafe_offset=0] == 58:
        return 0

    var separator: Int64 = -1
    for index in range(end):
        if ptr[unsafe_offset=index] == 58:
            separator = index
            break

    var field_end = end if separator < 0 else separator
    if field_end != 4:
        return 0
    if (
        ptr[unsafe_offset=0] != 100
        or ptr[unsafe_offset=1] != 97
        or ptr[unsafe_offset=2] != 116
        or ptr[unsafe_offset=3] != 97
    ):
        return 0

    var value_start = end
    if separator >= 0:
        value_start = separator + 1
        if value_start < end and ptr[unsafe_offset=value_start] == 32:
            value_start += 1
    output[unsafe_offset=0] = SSE_LINE_DATA
    output[unsafe_offset=1] = value_start
    output[unsafe_offset=2] = end
    return 0


comptime SSE_INSPECTION_CONTINUE: Int64 = 0
comptime SSE_INSPECTION_QUOTA_BLOCKED: Int64 = 1
comptime SSE_INSPECTION_RATE_LIMITED: Int64 = 2
comptime SSE_INSPECTION_OVERLOADED: Int64 = 3
comptime SSE_INSPECTION_PREVIOUS_RESPONSE_NOT_FOUND: Int64 = 4


@export("prodex_runtime_sse_inspection_step_v1")
def prodex_runtime_sse_inspection_step_v1(
    committed: Int64,
    quota_blocked: Int64,
    rate_limited: Int64,
    overloaded: Int64,
    previous_response_not_found: Int64,
    precommit_hold: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        committed < 0
        or committed > 1
        or quota_blocked < 0
        or quota_blocked > 1
        or rate_limited < 0
        or rate_limited > 1
        or overloaded < 0
        or overloaded > 1
        or previous_response_not_found < 0
        or previous_response_not_found > 1
        or precommit_hold < 0
        or precommit_hold > 1
        or output_address == 0
    ):
        return 1

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var action = SSE_INSPECTION_CONTINUE
    if committed == 0:
        if quota_blocked == 1:
            action = SSE_INSPECTION_QUOTA_BLOCKED
        elif rate_limited == 1:
            action = SSE_INSPECTION_RATE_LIMITED
        elif overloaded == 1:
            action = SSE_INSPECTION_OVERLOADED
        elif previous_response_not_found == 1:
            action = SSE_INSPECTION_PREVIOUS_RESPONSE_NOT_FOUND

    output[unsafe_offset=0] = action
    output[unsafe_offset=1] = committed
    if action == SSE_INSPECTION_CONTINUE and precommit_hold == 0:
        output[unsafe_offset=1] = 1
    return 0
