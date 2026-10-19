
from std.memory import Pointer

comptime SSE_LINE_BLANK: Int64 = 0
comptime SSE_LINE_IGNORE: Int64 = 1
comptime SSE_LINE_DATA: Int64 = 2
comptime SSE_UINT64_MAX: UInt64 = 18_446_744_073_709_551_615


# Identify newline boundaries in a bounded chunk of streaming SSE bytes.
# Rust owns transport I/O and buffered line data, but no longer scans bytes
# for LF or duplicates its chunk framing in separate inspection paths.
comptime SSE_CHUNK_MAX_BOUNDARIES: Int64 = 256

@export("prodex_runtime_sse_chunk_line_offsets_v1")
def prodex_runtime_sse_chunk_line_offsets_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    offsets_address: UInt,
    offsets_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (
        input_length < 0
        or (input_length > 0 and input_address == 0)
        or offsets_address == 0
        or result_address == 0
        or offsets_capacity < 1
        or offsets_capacity > SSE_CHUNK_MAX_BOUNDARIES
    ):
        return 1
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var offsets = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(offsets_address)
    )
    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    var cursor: Int64 = 0
    var count: Int64 = 0
    while cursor < input_length:
        if source[unsafe_offset=cursor] == 10:
            offsets[unsafe_offset=count] = UInt64(cursor)
            count += 1
            cursor += 1
            if count == offsets_capacity:
                break
        else:
            cursor += 1
    result[unsafe_offset=0] = UInt64(cursor)
    result[unsafe_offset=1] = UInt64(count)
    return 0


# SSE data payload assembly is a Mojo-owned wire operation: join data:
# line bodies using LF and remove repeated leading UTF-8 BOM sequences.
# The host owns String data and serde_json decoding of the resulting bytes.
@export("prodex_runtime_sse_payload_join_v1")
def prodex_runtime_sse_payload_join_v1(
    abi_version: Int64,
    views_address: UInt,
    line_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (
        line_count < 1
        or views_address == 0
        or output_address == 0
        or written_address == 0
        or output_capacity < 0
    ):
        return 1
    var views = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(views_address)
    )
    var first_address = views[unsafe_offset=0]
    var first_length = views[unsafe_offset=1]
    if first_length > UInt64(9223372036854775807):
        return 1
    if first_length > 0 and first_address == 0:
        return 1
    var first_skip: Int64 = 0
    if first_length >= 3:
        var first = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(first_address)
        )
        while first_skip + 3 <= Int64(first_length):
            if (
                first[unsafe_offset=first_skip] != 239
                or first[unsafe_offset=first_skip + 1] != 187
                or first[unsafe_offset=first_skip + 2] != 191
            ):
                break
            first_skip += 3

    # Check all lengths and required capacity before touching caller output.
    var required: Int64 = 0
    for index in range(line_count):
        var address = views[unsafe_offset=index * 2]
        var length = views[unsafe_offset=index * 2 + 1]
        if length > UInt64(9223372036854775807):
            return 1
        if length > 0 and address == 0:
            return 1
        var portion = Int64(length)
        if index == 0:
            portion -= first_skip
        else:
            if portion == 9223372036854775807:
                return 1
            portion += 1
        if portion > output_capacity - required:
            return 3
        required += portion

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var cursor: Int64 = 0
    for index in range(line_count):
        if index > 0:
            output[unsafe_offset=cursor] = 10
            cursor += 1
        var address = views[unsafe_offset=index * 2]
        var length = Int64(views[unsafe_offset=index * 2 + 1])
        if length > 0:
            var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
                unsafe_from_address=Int(address)
            )
            var offset: Int64 = first_skip if index == 0 else 0
            while offset < length:
                output[unsafe_offset=cursor] = source[unsafe_offset=offset]
                cursor += 1
                offset += 1
    written[] = cursor
    return 0


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


# Interpret an already Mojo-classified upstream error action into the
# SSE pre-commit retry flags. An action of passthrough, including after
# local response commitment, must never demand another profile attempt.
# Class: 0 other, 1 quota, 2 rate-limit, 3 unavailable,
#        4 overload, 5 transient-server.
# Action: 0 passthrough, 1 rotate-profile, 2 retry-profile.
comptime SSE_FAILURE_QUOTA_BLOCKED: UInt64 = 1
comptime SSE_FAILURE_RATE_LIMITED: UInt64 = 2
comptime SSE_FAILURE_OVERLOADED: UInt64 = 4

@export("prodex_runtime_sse_failure_flags_v1")
def prodex_runtime_sse_failure_flags_v1(
    abi_version: Int64,
    class_tag: Int64,
    action_tag: Int64,
    flags_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if class_tag < 0 or class_tag > 5 or action_tag < 0 or action_tag > 2 or flags_address == 0:
        return 1
    var flags = UInt64(0)
    if action_tag == 1 and class_tag == 1:
        flags |= SSE_FAILURE_QUOTA_BLOCKED
    elif action_tag == 2 and class_tag == 2:
        flags |= SSE_FAILURE_RATE_LIMITED
    elif (
        (action_tag == 2 and (class_tag == 4 or class_tag == 5))
        or (action_tag == 1 and class_tag == 3)
    ):
        flags |= SSE_FAILURE_OVERLOADED
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(flags_address)
    )
    output[] = flags
    return 0


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


@export("prodex_runtime_sse_event_byte_plan_v1")
def prodex_runtime_sse_event_byte_plan_v1(
    current_bytes: UInt64,
    data_bytes: UInt64,
    max_bytes: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if output_address == 0:
        return 1
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var next_bytes: UInt64
    if SSE_UINT64_MAX - current_bytes < data_bytes:
        next_bytes = SSE_UINT64_MAX
    else:
        next_bytes = current_bytes + data_bytes
    if next_bytes != SSE_UINT64_MAX:
        if SSE_UINT64_MAX - next_bytes < 1:
            next_bytes = SSE_UINT64_MAX
        else:
            next_bytes += 1
    output[unsafe_offset=0] = next_bytes
    output[unsafe_offset=1] = UInt64(next_bytes > max_bytes)
    return 0


# A polling slice or metadata prefix is never evidence of model output.
# The caller invokes this only while event inspection still reports Hold.
@export("prodex_runtime_sse_precommit_boundary_v1")
def prodex_runtime_sse_precommit_boundary_v1(
    abi_version: Int64,
    buffered_bytes: UInt64,
    byte_limit: UInt64,
    elapsed_ms: UInt64,
    deadline_ms: UInt64,
    upstream_eof: Int64,
) abi("C") -> Int64:
    if abi_version != 1:
        return -4
    if byte_limit == 0 or deadline_ms == 0 or upstream_eof < 0 or upstream_eof > 1:
        return -1
    if upstream_eof == 1:
        return 3
    if buffered_bytes >= byte_limit:
        return 2
    if elapsed_ms >= deadline_ms:
        return 1
    return 0


# Shared provider SSE JSON framing precedence. Serde owns JSON syntax parsing
# and Rust owns stream buffers; the compiled SSE owner alone chooses between
# a valid joined event, separate complete JSON lines, bounded accumulation,
# and a hard size-limit error for incomplete fragments.
comptime SSE_JSON_FRAME_ABI_VERSION: Int64 = 1
comptime SSE_JSON_FRAME_JOINED: Int64 = 0
comptime SSE_JSON_FRAME_SEPARATE: Int64 = 1
comptime SSE_JSON_FRAME_ACCUMULATE: Int64 = 2
comptime SSE_JSON_FRAME_LIMIT_EXCEEDED: Int64 = 3


@export("prodex_runtime_sse_json_frame_plan_v1")
def prodex_runtime_sse_json_frame_plan_v1(
    abi_version: Int64,
    joined_json_valid: Int64,
    separate_json_valid: Int64,
    accumulated_bytes: UInt64,
    fragment_bytes: UInt64,
    accumulated_max_bytes: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != SSE_JSON_FRAME_ABI_VERSION:
        return 4
    if (
        (joined_json_valid != 0 and joined_json_valid != 1)
        or (separate_json_valid != 0 and separate_json_valid != 1)
        or result_address == 0
    ):
        return 1
    var decision = SSE_JSON_FRAME_JOINED
    if joined_json_valid == 0:
        decision = SSE_JSON_FRAME_SEPARATE
        if separate_json_valid == 0:
            decision = SSE_JSON_FRAME_ACCUMULATE
            if (
                fragment_bytes > accumulated_max_bytes
                or accumulated_bytes > accumulated_max_bytes - fragment_bytes
            ):
                decision = SSE_JSON_FRAME_LIMIT_EXCEEDED
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = decision
    return 0
