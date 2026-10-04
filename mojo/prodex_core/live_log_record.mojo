from std.memory import Pointer

comptime LIVE_LOG_RECORD_ABI_VERSION: Int64 = 1
comptime LIVE_LOG_RECORD_OK: Int64 = 0
comptime LIVE_LOG_RECORD_INVALID: Int64 = 1
comptime LIVE_LOG_RECORD_CAPACITY: Int64 = 2
comptime LIVE_LOG_RECORD_ABI: Int64 = 4
comptime LIVE_LOG_RECORD_MAX_BYTES: Int64 = 128 * 1024
comptime LIVE_LOG_NESTED_STRING_MAX_BYTES: Int64 = 8 * 1024
comptime LIVE_LOG_RECORD_PREFIX_RESERVE: Int64 = 32


def live_log_utf8_prefix_end(
    input: Pointer[mut=False, UInt8, _], length: Int64, boundary: Int64
) -> Tuple[Int64, Bool]:
    var cursor: Int64 = 0
    var end: Int64 = 0
    while cursor < length and cursor < boundary:
        var first = input[unsafe_offset=cursor]
        var width: Int64
        if first < 128:
            width = 1
        elif first >= 194 and first <= 223:
            width = 2
        elif first >= 224 and first <= 239:
            width = 3
        elif first >= 240 and first <= 244:
            width = 4
        else:
            return (0, False)
        if width > length - cursor:
            return (0, False)
        var second: UInt8 = 0
        for offset in range(Int64(1), width):
            var next_byte = input[unsafe_offset=cursor + offset]
            if next_byte < 128 or next_byte > 191:
                return (0, False)
            if offset == 1:
                second = next_byte
        if (first == 224 and second < 160) or (first == 237 and second > 159):
            return (0, False)
        if (first == 240 and second < 144) or (first == 244 and second > 143):
            return (0, False)
        cursor += width
        end = cursor
    return (end, True)


@export("prodex_live_log_record_over_limit_v1")
def prodex_live_log_record_over_limit_v1(
    abi_version: Int64, record_length: Int64, output_address: UInt
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if record_length < 0 or output_address == 0:
        return LIVE_LOG_RECORD_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = Int64(record_length > LIVE_LOG_RECORD_MAX_BYTES)
    return LIVE_LOG_RECORD_OK


@export("prodex_live_log_string_clip_end_v1")
def prodex_live_log_string_clip_end_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if (
        input_length < 0
        or output_address == 0
        or (input_length > 0 and input_address == 0)
    ):
        return LIVE_LOG_RECORD_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if input_length <= LIVE_LOG_NESTED_STRING_MAX_BYTES:
        output[unsafe_offset=0] = input_length
        return LIVE_LOG_RECORD_OK
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var prefix_result = live_log_utf8_prefix_end(
        input, input_length, LIVE_LOG_NESTED_STRING_MAX_BYTES
    )
    var end = prefix_result[0]
    var valid = prefix_result[1]
    if not valid:
        return LIVE_LOG_RECORD_INVALID
    output[unsafe_offset=0] = end
    return LIVE_LOG_RECORD_OK


@export("prodex_live_log_json_plan_v1")
def prodex_live_log_json_plan_v1(
    abi_version: Int64, serialized_length: Int64, output_address: UInt
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if serialized_length < 0 or output_address == 0:
        return LIVE_LOG_RECORD_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var compact = serialized_length > LIVE_LOG_RECORD_MAX_BYTES
    output[unsafe_offset=0] = Int64(compact)
    output[unsafe_offset=1] = Int64(compact)
    output[unsafe_offset=2] = Int64(compact)
    output[unsafe_offset=3] = Int64(compact)
    return LIVE_LOG_RECORD_OK


@export("prodex_live_log_plain_text_truncate_v1")
def prodex_live_log_plain_text_truncate_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != LIVE_LOG_RECORD_ABI_VERSION:
        return LIVE_LOG_RECORD_ABI
    if (
        input_length <= LIVE_LOG_RECORD_MAX_BYTES
        or output_capacity < 0
        or output_address == 0
        or written_address == 0
        or input_address == 0
    ):
        return LIVE_LOG_RECORD_INVALID
    var input = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var prefix_result = live_log_utf8_prefix_end(
        input,
        input_length,
        LIVE_LOG_RECORD_MAX_BYTES - LIVE_LOG_RECORD_PREFIX_RESERVE,
    )
    var end = prefix_result[0]
    var valid = prefix_result[1]
    if not valid:
        return LIVE_LOG_RECORD_INVALID
    var tail = StringSlice(" …[truncated]\n")
    var tail_pointer = tail.unsafe_ptr()
    var tail_length = Int64(tail.byte_length())
    var required = end + tail_length
    if output_capacity < required:
        return LIVE_LOG_RECORD_CAPACITY
    for index in range(end):
        output[unsafe_offset=index] = input[unsafe_offset=index]
    for index in range(tail_length):
        output[unsafe_offset=end + index] = tail_pointer[unsafe_offset=index]
    written[unsafe_offset=0] = required
    return LIVE_LOG_RECORD_OK
