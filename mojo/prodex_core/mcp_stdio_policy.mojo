from std.memory import Pointer

from rich_text import rich_codepoint, rich_codepoint_width, rich_unicode_space, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime MCP_STDIO_ABI_VERSION: Int64 = 1
comptime MCP_STDIO_OK: Int64 = 0
comptime MCP_STDIO_INVALID: Int64 = 1
comptime MCP_STDIO_ABI: Int64 = 4
comptime MCP_STDIO_MAX_HEADER_BYTES: Int64 = 65_536
comptime MCP_STDIO_U64_MAX: UInt64 = 18_446_744_073_709_551_615


def mcp_ascii_lower(byte: UInt8) -> UInt8:
    if byte >= 65 and byte <= 90:
        return byte + 32
    return byte


def mcp_range_prefix_content_length(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    var literal = StringSlice("content-length:")
    var length = Int64(literal.byte_length())
    if end - start < length:
        return False
    var source = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if mcp_ascii_lower(source[unsafe_offset=start + index]) != expected[unsafe_offset=index]:
            return False
    return True


def mcp_trim_bounds(view: ProdexRichStringView) -> Tuple[Int64, Int64]:
    var start: Int64 = 0
    var end = Int64(view.len)
    var source = rich_view_ptr(view)
    while start < end:
        var width = rich_codepoint_width(source[unsafe_offset=start])
        if not rich_unicode_space(rich_codepoint(source, start, width)):
            break
        start += width
    while end > start:
        var cursor = end - 1
        while cursor > start and source[unsafe_offset=cursor] >= 0x80 and source[unsafe_offset=cursor] <= 0xBF:
            cursor -= 1
        var width = end - cursor
        if not rich_unicode_space(rich_codepoint(source, cursor, width)):
            break
        end = cursor
    return (start, end)


@export("prodex_mcp_header_is_content_length_v1")
def prodex_mcp_header_is_content_length_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    trim_first: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != MCP_STDIO_ABI_VERSION:
        return MCP_STDIO_ABI
    if (
        input_length < 0
        or input_length > MCP_STDIO_MAX_HEADER_BYTES
        or (input_length > 0 and input_address == 0)
        or (trim_first != 0 and trim_first != 1)
        or output_address == 0
    ):
        return MCP_STDIO_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, MCP_STDIO_MAX_HEADER_BYTES):
        return MCP_STDIO_INVALID
    var start: Int64 = 0
    var end = input_length
    if trim_first == 1:
        var bounds = mcp_trim_bounds(view)
        start = bounds[0]
        end = bounds[1]
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = Int64(mcp_range_prefix_content_length(view, start, end))
    return MCP_STDIO_OK


@export("prodex_mcp_content_length_parse_v1")
def prodex_mcp_content_length_parse_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    parse_kind_address: UInt,
    value_address: UInt,
) abi("C") -> Int64:
    if abi_version != MCP_STDIO_ABI_VERSION:
        return MCP_STDIO_ABI
    if (
        input_length < 0
        or input_length > MCP_STDIO_MAX_HEADER_BYTES
        or (input_length > 0 and input_address == 0)
        or parse_kind_address == 0
        or value_address == 0
    ):
        return MCP_STDIO_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, MCP_STDIO_MAX_HEADER_BYTES):
        return MCP_STDIO_INVALID

    var kind = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(parse_kind_address)
    )
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(value_address)
    )
    kind[] = 0
    output[] = 0

    var source = rich_view_ptr(view)
    var colon: Int64 = -1
    for index in range(input_length):
        if source[unsafe_offset=index] == 58:
            colon = index
            break
    if colon < 0:
        kind[] = 1
        return MCP_STDIO_OK

    var value_view = ProdexRichStringView(
        input_address + UInt(colon + 1),
        UInt(input_length - colon - 1),
    )
    var bounds = mcp_trim_bounds(value_view)
    var start = bounds[0]
    var end = bounds[1]
    var value_source = rich_view_ptr(value_view)
    if start < end and value_source[unsafe_offset=start] == 43:
        start += 1
    if start >= end:
        kind[] = 2
        return MCP_STDIO_OK

    var value: UInt64 = 0
    for index in range(start, end):
        var byte = value_source[unsafe_offset=index]
        if byte < 48 or byte > 57:
            kind[] = 2
            return MCP_STDIO_OK
        var digit = UInt64(byte - 48)
        if (
            value > MCP_STDIO_U64_MAX // UInt64(10)
            or (
                value == MCP_STDIO_U64_MAX // UInt64(10)
                and digit > MCP_STDIO_U64_MAX % UInt64(10)
            )
        ):
            kind[] = 2
            return MCP_STDIO_OK
        value = value * UInt64(10) + digit
    output[] = value
    return MCP_STDIO_OK
