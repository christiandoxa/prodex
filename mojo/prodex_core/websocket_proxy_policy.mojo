from std.memory import Pointer
from std.utils import StaticTuple

from rich_text import rich_trim_bounds, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime WEBSOCKET_PROXY_ABI_VERSION: Int64 = 1
comptime WEBSOCKET_PROXY_OK: Int64 = 0
comptime WEBSOCKET_PROXY_INVALID: Int64 = 1
comptime WEBSOCKET_PROXY_CAPACITY: Int64 = 2
comptime WEBSOCKET_PROXY_ABI: Int64 = 4


def websocket_proxy_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


def websocket_proxy_copy_range(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var length = end - start
    if length > capacity:
        return False
    var source = rich_view_ptr(view)
    for index in range(length):
        output[unsafe_offset=index] = source[unsafe_offset=start + index]
    written[] = length
    return True


def websocket_proxy_put_literal(
    literal: StringSlice,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var offset = written[]
    var length = Int64(literal.byte_length())
    if offset < 0 or offset > capacity or length > capacity - offset:
        return False
    var source = literal.unsafe_ptr()
    for index in range(length):
        output[unsafe_offset=offset + index] = source[unsafe_offset=index]
    written[] = offset + length
    return True


def websocket_proxy_append_range(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var offset = written[]
    var length = end - start
    if offset < 0 or offset > capacity or length > capacity - offset:
        return False
    var source = rich_view_ptr(view)
    for index in range(length):
        output[unsafe_offset=offset + index] = source[unsafe_offset=start + index]
    written[] = offset + length
    return True


def websocket_proxy_ascii_lower(value: UInt8) -> UInt8:
    return value + 32 if value >= 65 and value <= 90 else value


def websocket_proxy_range_equals(
    left: ProdexRichStringView,
    left_start: Int64,
    left_end: Int64,
    right: ProdexRichStringView,
    right_start: Int64,
    right_end: Int64,
) -> Bool:
    var length = left_end - left_start
    if length != right_end - right_start or length < 0:
        return False
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    for index in range(length):
        if websocket_proxy_ascii_lower(left_ptr[unsafe_offset=left_start + index]) != websocket_proxy_ascii_lower(right_ptr[unsafe_offset=right_start + index]):
            return False
    return True


def websocket_proxy_range_suffix(
    host: ProdexRichStringView,
    host_start: Int64,
    host_end: Int64,
    pattern: ProdexRichStringView,
    pattern_start: Int64,
    pattern_end: Int64,
) -> Bool:
    var pattern_length = pattern_end - pattern_start
    var host_length = host_end - host_start
    if pattern_length < 0 or host_length < pattern_length + 1:
        return False
    var suffix_start = host_end - pattern_length - 1
    var host_ptr = rich_view_ptr(host)
    if host_ptr[unsafe_offset=suffix_start] != 46:
        return False
    return websocket_proxy_range_equals(
        host,
        suffix_start + 1,
        host_end,
        pattern,
        pattern_start,
        pattern_end,
    )


def websocket_proxy_strip_brackets(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Tuple[Int64, Int64]:
    var source = rich_view_ptr(view)
    var left = start
    var right = end
    while left < right and (
        source[unsafe_offset=left] == 91 or source[unsafe_offset=left] == 93
    ):
        left += 1
    while right > left and (
        source[unsafe_offset=right - 1] == 91 or source[unsafe_offset=right - 1] == 93
    ):
        right -= 1
    return (left, right)


def websocket_proxy_parse_u16(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Tuple[Bool, Int64]:
    if start < 0 or end <= start or end > Int64(view.len):
        return (False, Int64(0))
    var source = rich_view_ptr(view)
    var index = start
    if source[unsafe_offset=index] == 43:
        index += 1
        if index == end:
            return (False, Int64(0))
    var value: Int64 = 0
    while index < end:
        var byte = source[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return (False, Int64(0))
        var digit = Int64(byte - 48)
        if value > (65535 - digit) // 10:
            return (False, Int64(0))
        value = value * 10 + digit
        index += 1
    return (True, value)


def websocket_proxy_pattern_parts(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Tuple[Int64, Int64, Int64, Int64]:
    # host_start, host_end, port_present, port
    if start < 0 or end < start or end > Int64(view.len):
        return (-1, -1, 0, 0)
    var source = rich_view_ptr(view)
    if start < end and source[unsafe_offset=start] == 91:
        var close: Int64 = -1
        for index in range(start + 1, end):
            if source[unsafe_offset=index] == 93:
                close = index
                break
        if close >= 0:
            var port_present: Int64 = 0
            var port: Int64 = 0
            if close + 1 < end and source[unsafe_offset=close + 1] == 58:
                var parsed = websocket_proxy_parse_u16(view, close + 2, end)
                if parsed[0]:
                    port_present = 1
                    port = parsed[1]
            return (start + 1, close, port_present, port)

    var colon_count: Int64 = 0
    var colon_index: Int64 = -1
    for index in range(start, end):
        if source[unsafe_offset=index] == 58:
            colon_count += 1
            colon_index = index
    if colon_count == 1:
        var parsed = websocket_proxy_parse_u16(view, colon_index + 1, end)
        if parsed[0]:
            return (start, colon_index, 1, parsed[1])
    return (start, end, 0, 0)


def websocket_proxy_pattern_matches_range(
    pattern: ProdexRichStringView,
    start: Int64,
    end: Int64,
    host: ProdexRichStringView,
    port: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(pattern.len):
        return False
    var sub = ProdexRichStringView(
        UInt(Int(pattern.ptr) + Int(start)),
        UInt(end - start),
    )
    var bounds = rich_trim_bounds(sub)
    var pattern_start = start + bounds[0]
    var pattern_end = start + bounds[1]
    if pattern_end <= pattern_start:
        return False
    var pattern_ptr = rich_view_ptr(pattern)
    if pattern_end - pattern_start == 1 and pattern_ptr[unsafe_offset=pattern_start] == 42:
        return True

    var parts = websocket_proxy_pattern_parts(pattern, pattern_start, pattern_end)
    if parts[0] < 0:
        return False
    if parts[2] == 1 and parts[3] != port:
        return False

    var normalized_pattern = websocket_proxy_strip_brackets(pattern, parts[0], parts[1])
    var normalized_host = websocket_proxy_strip_brackets(host, 0, Int64(host.len))
    var normalized_pattern_start = normalized_pattern[0]
    while normalized_pattern_start < normalized_pattern[1] and pattern_ptr[unsafe_offset=normalized_pattern_start] == 46:
        normalized_pattern_start += 1

    return websocket_proxy_range_equals(
        host,
        normalized_host[0],
        normalized_host[1],
        pattern,
        normalized_pattern_start,
        normalized_pattern[1],
    ) or websocket_proxy_range_suffix(
        host,
        normalized_host[0],
        normalized_host[1],
        pattern,
        normalized_pattern_start,
        normalized_pattern[1],
    )


@export("prodex_websocket_proxy_default_port_v1")
def prodex_websocket_proxy_default_port_v1(
    abi_version: Int64,
    scheme_address: UInt,
    scheme_length: Int64,
    scheme_present: Int64,
) abi("C") -> Int64:
    if abi_version != WEBSOCKET_PROXY_ABI_VERSION:
        return -4
    if scheme_present != 0 and scheme_present != 1:
        return -1
    if scheme_present == 0:
        return 80
    if scheme_length < 0:
        return -1
    var view = websocket_proxy_view(scheme_address, scheme_length)
    if not rich_view_valid(view, 64):
        return -1
    var source = rich_view_ptr(view)
    if scheme_length == 3 and source[unsafe_offset=0] == 119 and source[unsafe_offset=1] == 115 and source[unsafe_offset=2] == 115:
        return 443
    if scheme_length == 5 and source[unsafe_offset=0] == 104 and source[unsafe_offset=1] == 116 and source[unsafe_offset=2] == 116 and source[unsafe_offset=3] == 112 and source[unsafe_offset=4] == 115:
        return 443
    return 80


@export("prodex_websocket_proxy_url_candidate_v1")
def prodex_websocket_proxy_url_candidate_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    present_address: UInt,
) abi("C") -> Int64:
    if abi_version != WEBSOCKET_PROXY_ABI_VERSION:
        return WEBSOCKET_PROXY_ABI
    if length < 0 or output_address == 0 or output_capacity < 0 or written_address == 0 or present_address == 0:
        return WEBSOCKET_PROXY_INVALID
    var view = websocket_proxy_view(address, length)
    if not rich_view_valid(view, 65536):
        return WEBSOCKET_PROXY_INVALID
    var bounds = rich_trim_bounds(view)
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    var present = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(present_address))
    written[] = 0
    present[] = 0
    if bounds[1] <= bounds[0]:
        return WEBSOCKET_PROXY_OK

    var source = rich_view_ptr(view)
    var has_scheme = False
    if bounds[1] - bounds[0] >= 3:
        for index in range(bounds[0], bounds[1] - 2):
            if source[unsafe_offset=index] == 58 and source[unsafe_offset=index + 1] == 47 and source[unsafe_offset=index + 2] == 47:
                has_scheme = True
                break

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    if not has_scheme:
        if not websocket_proxy_put_literal(StringSlice("http://"), output, output_capacity, written):
            return WEBSOCKET_PROXY_CAPACITY
    if not websocket_proxy_append_range(view, bounds[0], bounds[1], output, output_capacity, written):
        return WEBSOCKET_PROXY_CAPACITY
    present[] = 1
    return WEBSOCKET_PROXY_OK


@export("prodex_websocket_proxy_pattern_plan_v1")
def prodex_websocket_proxy_pattern_plan_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != WEBSOCKET_PROXY_ABI_VERSION:
        return WEBSOCKET_PROXY_ABI
    if length < 0 or output_address == 0:
        return WEBSOCKET_PROXY_INVALID
    var view = websocket_proxy_view(address, length)
    if not rich_view_valid(view, 65536):
        return WEBSOCKET_PROXY_INVALID
    var parts = websocket_proxy_pattern_parts(view, 0, length)
    if parts[0] < 0:
        return WEBSOCKET_PROXY_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = parts[0]
    output[unsafe_offset=1] = parts[1]
    output[unsafe_offset=2] = parts[2]
    output[unsafe_offset=3] = parts[3]
    return WEBSOCKET_PROXY_OK


@export("prodex_websocket_proxy_pattern_matches_v1")
def prodex_websocket_proxy_pattern_matches_v1(
    abi_version: Int64,
    pattern_address: UInt,
    pattern_length: Int64,
    host_address: UInt,
    host_length: Int64,
    port: Int64,
) abi("C") -> Int64:
    if abi_version != WEBSOCKET_PROXY_ABI_VERSION:
        return -4
    if pattern_length < 0 or host_length < 0 or port < 0 or port > 65535:
        return -1
    var pattern = websocket_proxy_view(pattern_address, pattern_length)
    var host = websocket_proxy_view(host_address, host_length)
    if not rich_view_valid(pattern, 65536) or not rich_view_valid(host, 65536):
        return -1
    return Int64(websocket_proxy_pattern_matches_range(pattern, 0, pattern_length, host, port))


@export("prodex_websocket_proxy_value_matches_v1")
def prodex_websocket_proxy_value_matches_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    host_address: UInt,
    host_length: Int64,
    port: Int64,
) abi("C") -> Int64:
    if abi_version != WEBSOCKET_PROXY_ABI_VERSION:
        return -4
    if value_length < 0 or host_length < 0 or port < 0 or port > 65535:
        return -1
    var value = websocket_proxy_view(value_address, value_length)
    var host = websocket_proxy_view(host_address, host_length)
    if not rich_view_valid(value, 65536) or not rich_view_valid(host, 65536):
        return -1
    var source = rich_view_ptr(value)
    var start: Int64 = 0
    var index: Int64 = 0
    while index <= value_length:
        if index == value_length or source[unsafe_offset=index] == 44:
            if websocket_proxy_pattern_matches_range(value, start, index, host, port):
                return 1
            start = index + 1
        index += 1
    return 0


@export("prodex_websocket_proxy_normalize_host_v1")
def prodex_websocket_proxy_normalize_host_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != WEBSOCKET_PROXY_ABI_VERSION:
        return WEBSOCKET_PROXY_ABI
    if length < 0 or output_address == 0 or output_capacity < 0 or written_address == 0:
        return WEBSOCKET_PROXY_INVALID
    var view = websocket_proxy_view(address, length)
    if not rich_view_valid(view, 65536):
        return WEBSOCKET_PROXY_INVALID
    var bounds = websocket_proxy_strip_brackets(view, 0, length)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = 0
    return WEBSOCKET_PROXY_OK if websocket_proxy_copy_range(view, bounds[0], bounds[1], output, output_capacity, written) else WEBSOCKET_PROXY_CAPACITY


@export("prodex_websocket_proxy_authority_v1")
def prodex_websocket_proxy_authority_v1(
    abi_version: Int64,
    host_address: UInt,
    host_length: Int64,
    port: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != WEBSOCKET_PROXY_ABI_VERSION:
        return WEBSOCKET_PROXY_ABI
    if host_length < 0 or port < 0 or port > 65535 or output_address == 0 or output_capacity < 0 or written_address == 0:
        return WEBSOCKET_PROXY_INVALID
    var host = websocket_proxy_view(host_address, host_length)
    if not rich_view_valid(host, 65536):
        return WEBSOCKET_PROXY_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = 0
    var source = rich_view_ptr(host)
    var has_colon = False
    for index in range(host_length):
        if source[unsafe_offset=index] == 58:
            has_colon = True
            break
    if has_colon and not websocket_proxy_put_literal(StringSlice("["), output, output_capacity, written):
        return WEBSOCKET_PROXY_CAPACITY
    if not websocket_proxy_append_range(host, 0, host_length, output, output_capacity, written):
        return WEBSOCKET_PROXY_CAPACITY
    if has_colon and not websocket_proxy_put_literal(StringSlice("]"), output, output_capacity, written):
        return WEBSOCKET_PROXY_CAPACITY
    if not websocket_proxy_put_literal(StringSlice(":"), output, output_capacity, written):
        return WEBSOCKET_PROXY_CAPACITY

    # Write decimal port without allocations.
    var digits = StaticTuple[UInt8, 5](0, 0, 0, 0, 0)
    var count: Int64 = 0
    var value = port
    if value == 0:
        digits[0] = 48
        count = 1
    else:
        while value > 0:
            digits[count] = UInt8(48 + value % 10)
            count += 1
            value //= 10
    var offset = written[]
    if count > output_capacity - offset:
        return WEBSOCKET_PROXY_CAPACITY
    for index in range(count):
        output[unsafe_offset=offset + index] = digits[count - 1 - index]
    written[] = offset + count
    return WEBSOCKET_PROXY_OK
