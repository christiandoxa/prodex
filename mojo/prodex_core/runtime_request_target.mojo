from std.memory import Pointer

comptime RUNTIME_REQUEST_TARGET_ABI_VERSION: Int64 = 1
comptime RUNTIME_REQUEST_TARGET_OK: Int64 = 0
comptime RUNTIME_REQUEST_TARGET_INVALID: Int64 = 1
comptime RUNTIME_REQUEST_TARGET_ABI: Int64 = 4
comptime RUNTIME_REQUEST_TARGET_MAX_BYTES: Int64 = 8 * 1024


def runtime_request_target_hex(value: UInt8) -> Int64:
    if value >= 48 and value <= 57:
        return Int64(value - 48)
    if value >= 65 and value <= 70:
        return Int64(value - 65 + 10)
    if value >= 97 and value <= 102:
        return Int64(value - 97 + 10)
    return -1


def runtime_request_target_valid(
    source: Pointer[mut=False, UInt8, ImmUntrackedOrigin], length: Int64
) -> Bool:
    if length == 0 or length > RUNTIME_REQUEST_TARGET_MAX_BYTES:
        return False
    if source[unsafe_offset=0] != 47 or (
        length > 1 and source[unsafe_offset=1] == 47
    ):
        return False

    var path_length = length
    var index: Int64 = 0
    while index < length:
        var value = source[unsafe_offset=index]
        if value > 127 or value <= 32 or value == 127 or value == 35 or value == 92:
            return False
        if value == 63:
            path_length = index
            break
        index += 1

    index = path_length
    while index < length:
        var value = source[unsafe_offset=index]
        if value > 127 or value <= 32 or value == 127 or value == 35:
            return False
        index += 1

    var segment_start: Int64 = 0
    index = 0
    while index <= path_length:
        if index == path_length or source[unsafe_offset=index] == 47:
            var segment_length = index - segment_start
            if (
                segment_length == 1
                and source[unsafe_offset=segment_start] == 46
            ) or (
                segment_length == 2
                and source[unsafe_offset=segment_start] == 46
                and source[unsafe_offset=segment_start + 1] == 46
            ):
                return False
            if index < path_length and index > 0 and source[unsafe_offset=index - 1] == 47:
                return False
            segment_start = index + 1
        index += 1

    index = 0
    while index < length:
        if source[unsafe_offset=index] != 37:
            index += 1
            continue
        if index + 2 >= length:
            return False
        var high = runtime_request_target_hex(source[unsafe_offset=index + 1])
        var low = runtime_request_target_hex(source[unsafe_offset=index + 2])
        if high < 0 or low < 0:
            return False
        var decoded = (high << 4) | low
        if index < path_length and (
            decoded >= 128
            or decoded <= 32
            or decoded == 127
            or decoded == 47
            or decoded == 92
            or decoded == 37
            or decoded == 63
            or decoded == 35
            or decoded == 46
        ):
            return False
        index += 3
    return True


@export("prodex_runtime_request_target_valid_v1")
def prodex_runtime_request_target_valid_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != RUNTIME_REQUEST_TARGET_ABI_VERSION
    ):
        return RUNTIME_REQUEST_TARGET_ABI
    if (
        length < 0
        or (length > 0 and address == 0)
        or output_address == 0
    ):
        return RUNTIME_REQUEST_TARGET_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = 0
    if length <= RUNTIME_REQUEST_TARGET_MAX_BYTES:
        var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(address)
        )
        output[] = Int64(runtime_request_target_valid(source, length))
    return RUNTIME_REQUEST_TARGET_OK
