from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_utf8_valid
from rich_types import ProdexRichStringView


comptime DEEPSEEK_STREAM_POLICY_ABI_VERSION: Int64 = 1
comptime DEEPSEEK_STREAM_POLICY_OK: Int64 = 0
comptime DEEPSEEK_STREAM_POLICY_MISSING_NAME: Int64 = 1
comptime DEEPSEEK_STREAM_POLICY_INVALID_ARGUMENTS: Int64 = 2
comptime DEEPSEEK_STREAM_POLICY_ABI: Int64 = -4
comptime DEEPSEEK_STREAM_POLICY_INVALID: Int64 = -1
comptime DEEPSEEK_STREAM_POLICY_MAX_DEPTH: Int64 = 127


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


def deepseek_stream_policy_hex_value(value: UInt8) -> Int64:
    if value >= 48 and value <= 57:
        return Int64(value - 48)
    if value >= 65 and value <= 70:
        return Int64(value - 55)
    if value >= 97 and value <= 102:
        return Int64(value - 87)
    return -1


def deepseek_stream_policy_unicode_unit(
    ptr: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    start: Int64,
    end: Int64,
) -> Int64:
    if start < 0 or start + 4 > end:
        return -1
    var value: Int64 = 0
    for offset in range(4):
        var digit = deepseek_stream_policy_hex_value(
            deepseek_stream_policy_byte(ptr, start + Int64(offset))
        )
        if digit < 0:
            return -1
        value = value * 16 + digit
    return value


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
                var unit = deepseek_stream_policy_unicode_unit(ptr, index + 2, end)
                if unit < 0:
                    return -1
                if unit >= 55296 and unit <= 56319:
                    var trailing_start = index + 6
                    if trailing_start + 6 > end or deepseek_stream_policy_byte(
                        ptr, trailing_start
                    ) != 92 or deepseek_stream_policy_byte(ptr, trailing_start + 1) != 117:
                        return -1
                    var trailing = deepseek_stream_policy_unicode_unit(
                        ptr, trailing_start + 2, end
                    )
                    if trailing < 56320 or trailing > 57343:
                        return -1
                    index = trailing_start + 6
                    continue
                if unit >= 56320 and unit <= 57343:
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

    return index if deepseek_stream_policy_number_in_range(ptr, start, index) else -1


def deepseek_stream_policy_number_in_range(
    ptr: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    start: Int64,
    end: Int64,
) -> Bool:
    var index = start
    if index < end and deepseek_stream_policy_byte(ptr, index) == 45:
        index += 1

    var digits_before_decimal: Int64 = 0
    var digit_position: Int64 = 0
    var first_nonzero: Int64 = -1
    while index < end:
        var value = deepseek_stream_policy_byte(ptr, index)
        if value < 48 or value > 57:
            break
        if first_nonzero < 0 and value != 48:
            first_nonzero = digit_position
        digits_before_decimal += 1
        digit_position += 1
        index += 1

    if index < end and deepseek_stream_policy_byte(ptr, index) == 46:
        index += 1
        while index < end:
            var value = deepseek_stream_policy_byte(ptr, index)
            if value < 48 or value > 57:
                break
            if first_nonzero < 0 and value != 48:
                first_nonzero = digit_position
            digit_position += 1
            index += 1

    if first_nonzero < 0:
        return True
    var exponent_negative = False
    var exponent: Int64 = 0
    if index < end and (
        deepseek_stream_policy_byte(ptr, index) == 101
        or deepseek_stream_policy_byte(ptr, index) == 69
    ):
        index += 1
        if index < end and (
            deepseek_stream_policy_byte(ptr, index) == 43
            or deepseek_stream_policy_byte(ptr, index) == 45
        ):
            exponent_negative = deepseek_stream_policy_byte(ptr, index) == 45
            index += 1
        while index < end:
            var value = deepseek_stream_policy_byte(ptr, index)
            if value < 48 or value > 57:
                break
            if exponent > 10000:
                return exponent_negative
            exponent = exponent * 10 + Int64(value - 48)
            index += 1

    var adjusted = digits_before_decimal - first_nonzero - 1
    if exponent_negative:
        adjusted -= exponent
    else:
        adjusted += exponent
    if adjusted < 308:
        return True
    if adjusted > 308:
        return False

    var limit = StringSlice("17976931348623157")
    var limit_ptr = limit.unsafe_ptr()
    index = start
    if index < end and deepseek_stream_policy_byte(ptr, index) == 45:
        index += 1
    var significant = False
    var compared: Int64 = 0
    while index < end:
        var value = deepseek_stream_policy_byte(ptr, index)
        if value == 101 or value == 69:
            break
        if value >= 48 and value <= 57:
            if not significant:
                if value == 48:
                    index += 1
                    continue
                significant = True
            if compared < 17:
                var expected = limit_ptr[unsafe_offset=compared]
                if value > expected:
                    return False
                if value < expected:
                    return True
                compared += 1
        index += 1
    return True


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
    if depth > DEEPSEEK_STREAM_POLICY_MAX_DEPTH:
        return -1
    var index = deepseek_stream_policy_skip_ws(ptr, start, end)
    if index >= end:
        return -1
    var opening = deepseek_stream_policy_byte(ptr, index)
    if depth == DEEPSEEK_STREAM_POLICY_MAX_DEPTH and (opening == 91 or opening == 123):
        return -1
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


# The streamed item kind is selected once by Mojo. Rust retains JSON
# materialization through existing compiled DeepSeek event kernels.
@export("prodex_deepseek_stream_tool_kind_v1")
def prodex_deepseek_stream_tool_kind_v1(
    abi_version: Int64,
    name_address: UInt,
    name_length: Int64,
) abi("C") -> Int64:
    if abi_version != DEEPSEEK_STREAM_POLICY_ABI_VERSION:
        return DEEPSEEK_STREAM_POLICY_ABI
    if name_length < 0 or (name_length > 0 and name_address == 0):
        return DEEPSEEK_STREAM_POLICY_INVALID
    var name = deepseek_stream_policy_ptr(name_address)
    if name_length > 0 and not rich_utf8_valid(name, name_length):
        return DEEPSEEK_STREAM_POLICY_INVALID
    if name_length == 11:
        var search = StringSlice("tool_search").unsafe_ptr()
        var match_search = True
        for index in range(11):
            if name[unsafe_offset=index] != search[unsafe_offset=index]:
                match_search = False
                break
        if match_search:
            return 0
    if name_length == 11:
        var patch = StringSlice("apply_patch").unsafe_ptr()
        var match_patch = True
        for index in range(11):
            if name[unsafe_offset=index] != patch[unsafe_offset=index]:
                match_patch = False
                break
        if match_patch:
            return 1
    return 2


def deepseek_stream_identifier_digits(value: UInt64) -> Int64:
    var digits: Int64 = 1
    var rest = value
    while rest >= UInt64(10):
        rest = rest // UInt64(10)
        digits += 1
    return digits


def deepseek_stream_identifier_emit_decimal(
    number: UInt64,
    output: Pointer[mut=True, UInt8, _],
    cursor: Pointer[mut=True, Int64, _],
):
    var divisor = UInt64(1)
    while number // divisor >= UInt64(10):
        divisor *= UInt64(10)
    var remaining = number
    while True:
        output[unsafe_offset=cursor[]] = UInt8(48) + UInt8(remaining // divisor)
        cursor[] += 1
        remaining = remaining % divisor
        if divisor == UInt64(1):
            break
        divisor = divisor // UInt64(10)


# Mode 0: call_{provider}_{request}_{index}
# Mode 1: msg_{provider}_{request}
# Mode 2: resp_{provider}_{request}
@export("prodex_deepseek_stream_identifier_v1")
def prodex_deepseek_stream_identifier_v1(
    abi_version: Int64,
    mode: Int64,
    provider_address: UInt,
    provider_length: Int64,
    request_id: UInt64,
    index: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != DEEPSEEK_STREAM_POLICY_ABI_VERSION:
        return DEEPSEEK_STREAM_POLICY_ABI
    if (
        mode < 0 or mode > 2
        or provider_length < 0
        or (provider_length > 0 and provider_address == 0)
        or written_address == 0
        or output_capacity < 0
    ):
        return DEEPSEEK_STREAM_POLICY_INVALID
    if provider_length > 0 and not rich_utf8_valid(
        deepseek_stream_policy_ptr(provider_address), provider_length
    ):
        return DEEPSEEK_STREAM_POLICY_INVALID
    var prefix = StringSlice("call_")
    if mode == 1:
        prefix = StringSlice("msg_")
    elif mode == 2:
        prefix = StringSlice("resp_")
    var required = Int64(prefix.byte_length()) + provider_length + 1
    required += deepseek_stream_identifier_digits(request_id)
    if mode == 0:
        required += 1 + deepseek_stream_identifier_digits(index)
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = required
    if output_address == 0 or output_capacity < required:
        return 3
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var cursor: Int64 = 0
    var prefix_bytes = prefix.unsafe_ptr()
    for j in range(Int64(prefix.byte_length())):
        output[unsafe_offset=cursor] = prefix_bytes[unsafe_offset=j]
        cursor += 1
    var provider = deepseek_stream_policy_ptr(provider_address)
    for j in range(provider_length):
        output[unsafe_offset=cursor] = provider[unsafe_offset=j]
        cursor += 1
    output[unsafe_offset=cursor] = 95
    cursor += 1
    deepseek_stream_identifier_emit_decimal(
        request_id, output, Pointer(to=cursor)
    )
    if mode == 0:
        output[unsafe_offset=cursor] = 95
        cursor += 1
        deepseek_stream_identifier_emit_decimal(
            index, output, Pointer(to=cursor)
        )
    written[] = cursor
    return DEEPSEEK_STREAM_POLICY_OK


# A streamed provider ID may supersede only the original generated
# fallback response ID. Mojo owns the exact byte prefix decision.
@export("prodex_deepseek_stream_response_id_fallback_v1")
def prodex_deepseek_stream_response_id_fallback_v1(
    abi_version: Int64,
    provider_address: UInt,
    provider_length: Int64,
    current_address: UInt,
    current_length: Int64,
) abi("C") -> Int64:
    if abi_version != DEEPSEEK_STREAM_POLICY_ABI_VERSION:
        return DEEPSEEK_STREAM_POLICY_ABI
    if (
        provider_length < 0
        or current_length < 0
        or (provider_length > 0 and provider_address == 0)
        or (current_length > 0 and current_address == 0)
    ):
        return DEEPSEEK_STREAM_POLICY_INVALID
    var provider = deepseek_stream_policy_ptr(provider_address)
    var current = deepseek_stream_policy_ptr(current_address)
    if (
        (provider_length > 0 and not rich_utf8_valid(provider, provider_length))
        or (current_length > 0 and not rich_utf8_valid(current, current_length))
    ):
        return DEEPSEEK_STREAM_POLICY_INVALID
    if current_length < 6 or current_length - 6 < provider_length:
        return 0
    var prefix = StringSlice("resp_").unsafe_ptr()
    for index in range(5):
        if current[unsafe_offset=index] != prefix[unsafe_offset=index]:
            return 0
    for index in range(provider_length):
        if current[unsafe_offset=index + 5] != provider[unsafe_offset=index]:
            return 0
    return Int64(current[unsafe_offset=provider_length + 5] == 95)


# Mode 0: accept tool-call delta if function object or prior call exists.
# Mode 1: retain incremental nonempty tool-argument fragment.
# Mode 2: retain nonempty optional argument delta.
# Mode 3: retain optional signature only if Unicode trim() is nonempty.
@export("prodex_deepseek_stream_delta_policy_v1")
def prodex_deepseek_stream_delta_policy_v1(
    abi_version: Int64,
    mode: Int64,
    text_address: UInt,
    text_length: Int64,
    text_present: Int64,
    existing_tool_call: Int64,
) abi("C") -> Int64:
    if abi_version != DEEPSEEK_STREAM_POLICY_ABI_VERSION:
        return DEEPSEEK_STREAM_POLICY_ABI
    if (
        mode < 0 or mode > 3
        or text_length < 0
        or (text_length > 0 and text_address == 0)
        or (text_present != 0 and text_present != 1)
        or (existing_tool_call != 0 and existing_tool_call != 1)
    ):
        return DEEPSEEK_STREAM_POLICY_INVALID
    if mode == 0:
        return Int64(text_present == 1 or existing_tool_call == 1)
    if mode == 1:
        return Int64(text_length > 0)
    if text_present == 0:
        return 0
    if mode == 2:
        return Int64(text_length > 0)
    var text = ProdexRichStringView(text_address, UInt(text_length))
    if text_length > 0 and not rich_utf8_valid(
        deepseek_stream_policy_ptr(text_address), text_length
    ):
        return DEEPSEEK_STREAM_POLICY_INVALID
    var bounds = rich_trim_bounds(text)
    return Int64(bounds[0] < bounds[1])
