from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_utf8_valid
from rich_types import ProdexRichStringView


comptime DEEPSEEK_STREAM_POLICY_ABI_VERSION: Int64 = 1
comptime DEEPSEEK_STREAM_POLICY_OK: Int64 = 0
comptime DEEPSEEK_STREAM_POLICY_MISSING_NAME: Int64 = 1
comptime DEEPSEEK_STREAM_POLICY_INVALID_ARGUMENTS: Int64 = 2
comptime DEEPSEEK_STREAM_POLICY_ABI: Int64 = -4
comptime DEEPSEEK_STREAM_POLICY_INVALID: Int64 = -1


def deepseek_stream_policy_ptr(
    address: UInt,
) -> Pointer[mut=False, UInt8, ImmUntrackedOrigin]:
    return Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(address))


def deepseek_stream_policy_byte(
    ptr: Pointer[mut=False, UInt8, ImmUntrackedOrigin], index: Int64
) -> UInt8:
    return ptr[unsafe_offset=index]


def deepseek_stream_policy_skip_ws(
    ptr: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    start: Int64,
    end: Int64,
) -> Int64:
    var index = start
    while index < end:
        var value = deepseek_stream_policy_byte(ptr, index)
        if value != 9 and value != 10 and value != 13 and value != 32:
            break
        index += 1
    return index


def deepseek_stream_policy_hex(value: UInt8) -> Bool:
    return (
        value >= 48 and value <= 57
        or value >= 65 and value <= 70
        or value >= 97 and value <= 102
    )


def deepseek_stream_policy_string_end(
    ptr: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    start: Int64,
    end: Int64,
) -> Int64:
    if start >= end or deepseek_stream_policy_byte(ptr, start) != 34:
        return -1
    var index = start + 1
    while index < end:
        var value = deepseek_stream_policy_byte(ptr, index)
        if value == 34:
            return index + 1
        if value < 32:
            return -1
        if value == 92:
            if index + 1 >= end:
                return -1
            var escaped = deepseek_stream_policy_byte(ptr, index + 1)
            if escaped == 117:
                if index + 5 >= end:
                    return -1
                for offset in range(2, 6):
                    if not deepseek_stream_policy_hex(
                        deepseek_stream_policy_byte(ptr, index + Int64(offset))
                    ):
                        return -1
                index += 6
                continue
            if (
                escaped != 34
                and escaped != 92
                and escaped != 47
                and escaped != 98
                and escaped != 102
                and escaped != 110
                and escaped != 114
                and escaped != 116
            ):
                return -1
            index += 2
            continue
        index += 1
    return -1


def deepseek_stream_policy_number_end(
    ptr: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    start: Int64,
    end: Int64,
) -> Int64:
    var index = start
    if index < end and deepseek_stream_policy_byte(ptr, index) == 45:
        index += 1
    if index >= end:
        return -1
    var first = deepseek_stream_policy_byte(ptr, index)
    if first == 48:
        index += 1
        if index < end:
            var next = deepseek_stream_policy_byte(ptr, index)
            if next >= 48 and next <= 57:
                return -1
    elif first >= 49 and first <= 57:
        index += 1
        while index < end:
            var next = deepseek_stream_policy_byte(ptr, index)
            if next < 48 or next > 57:
                break
            index += 1
    else:
        return -1
    if index < end and deepseek_stream_policy_byte(ptr, index) == 46:
        index += 1
        var digits = index
        while index < end:
            var next = deepseek_stream_policy_byte(ptr, index)
            if next < 48 or next > 57:
                break
            index += 1
        if index == digits:
            return -1
    if index < end:
        var exponent = deepseek_stream_policy_byte(ptr, index)
        if exponent == 101 or exponent == 69:
            index += 1
            if index < end:
                var sign = deepseek_stream_policy_byte(ptr, index)
                if sign == 43 or sign == 45:
                    index += 1
            var digits = index
            while index < end:
                var next = deepseek_stream_policy_byte(ptr, index)
                if next < 48 or next > 57:
                    break
                index += 1
            if index == digits:
                return -1
        
    return index


def deepseek_stream_policy_literal_end(
    ptr: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Int64:
    var length = Int64(literal.byte_length())
    if start < 0 or start + length > end:
        return -1
    var target = literal.unsafe_ptr()
    for offset in range(length):
        if deepseek_stream_policy_byte(ptr, start + offset) != target[unsafe_offset=offset]:
            return -1
    return start + length


def deepseek_stream_policy_value_end(
    ptr: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    start: Int64,
    end: Int64,
    depth: Int64,
) -> Int64:
    if depth > 256:
        return -1
    var index = deepseek_stream_policy_skip_ws(ptr, start, end)
    if index >= end:
        return -1
    var opening = deepseek_stream_policy_byte(ptr, index)
    if opening == 34:
        return deepseek_stream_policy_string_end(ptr, index, end)
    if opening == 116:
        return deepseek_stream_policy_literal_end(ptr, index, end, StringSlice("true"))
    if opening == 102:
        return deepseek_stream_policy_literal_end(ptr, index, end, StringSlice("false"))
    if opening == 110:
        return deepseek_stream_policy_literal_end(ptr, index, end, StringSlice("null"))
    if opening != 91 and opening != 123:
        return deepseek_stream_policy_number_end(ptr, index, end)
    var closing = UInt8(93) if opening == 91 else UInt8(125)
    index += 1
    index = deepseek_stream_policy_skip_ws(ptr, index, end)
    if index < end and deepseek_stream_policy_byte(ptr, index) == closing:
        return index + 1
    while index < end:
        if opening == 123:
            var key_end = deepseek_stream_policy_string_end(ptr, index, end)
            if key_end < 0:
                return -1
            index = deepseek_stream_policy_skip_ws(ptr, key_end, end)
            if index >= end or deepseek_stream_policy_byte(ptr, index) != 58:
                return -1
            index += 1
        var value_end = deepseek_stream_policy_value_end(ptr, index, end, depth + 1)
        if value_end < 0:
            return -1
        index = deepseek_stream_policy_skip_ws(ptr, value_end, end)
        if index < end and deepseek_stream_policy_byte(ptr, index) == 44:
            index = deepseek_stream_policy_skip_ws(ptr, index + 1, end)
            continue
        if index < end and deepseek_stream_policy_byte(ptr, index) == closing:
            return index + 1
        return -1
    return -1


def deepseek_stream_policy_valid_json(
    address: UInt,
    length: Int64,
) -> Bool:
    if length < 0 or (length > 0 and address == 0):
        return False
    var source = ProdexRichStringView(address, UInt(length))
    if length == 0:
        return True
    var ptr = deepseek_stream_policy_ptr(address)
    if not rich_utf8_valid(ptr, length):
        return False
    # Rust's trim() only short-circuits an all-whitespace fragment; serde_json
    # itself accepts JSON's four ASCII whitespace bytes around a value.
    var unicode_bounds = rich_trim_bounds(source)
    if unicode_bounds[0] == unicode_bounds[1]:
        return True
    var start = deepseek_stream_policy_skip_ws(ptr, 0, length)
    var value_end = deepseek_stream_policy_value_end(ptr, start, length, 0)
    return value_end >= 0 and deepseek_stream_policy_skip_ws(ptr, value_end, length) == length


@export("prodex_deepseek_stream_tool_arguments_policy_v1")
def prodex_deepseek_stream_tool_arguments_policy_v1(
    abi_version: Int64,
    name_address: UInt,
    name_length: Int64,
    name_present: Int64,
    arguments_address: UInt,
    arguments_length: Int64,
) abi("C") -> Int64:
    if abi_version != DEEPSEEK_STREAM_POLICY_ABI_VERSION:
        return DEEPSEEK_STREAM_POLICY_ABI
    if (
        name_length < 0
        or arguments_length < 0
        or name_present < 0
        or name_present > 1
        or (name_length > 0 and name_address == 0)
        or (arguments_length > 0 and arguments_address == 0)
    ):
        return DEEPSEEK_STREAM_POLICY_INVALID
    if name_present == 0:
        return DEEPSEEK_STREAM_POLICY_MISSING_NAME
    if name_length > 0:
        var name = deepseek_stream_policy_ptr(name_address)
        if not rich_utf8_valid(name, name_length):
            return DEEPSEEK_STREAM_POLICY_MISSING_NAME
        var name_view = ProdexRichStringView(name_address, UInt(name_length))
        var name_bounds = rich_trim_bounds(name_view)
        if name_bounds[0] == name_bounds[1]:
            return DEEPSEEK_STREAM_POLICY_MISSING_NAME
    elif name_present == 1:
        return DEEPSEEK_STREAM_POLICY_MISSING_NAME
    if not deepseek_stream_policy_valid_json(arguments_address, arguments_length):
        return DEEPSEEK_STREAM_POLICY_INVALID_ARGUMENTS
    return DEEPSEEK_STREAM_POLICY_OK
