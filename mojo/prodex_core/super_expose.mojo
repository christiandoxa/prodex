from std.collections import Array

from std.memory import Pointer

from rich_text import rich_view_matches_literal, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime SUPER_EXPOSE_ABI_VERSION: Int64 = 1
comptime SUPER_EXPOSE_MAX_NAME_BYTES: Int64 = 128

comptime SUPER_EXPOSE_METHOD_UNKNOWN: Int64 = 0
comptime SUPER_EXPOSE_METHOD_INITIALIZE: Int64 = 1
comptime SUPER_EXPOSE_METHOD_PING: Int64 = 2
comptime SUPER_EXPOSE_METHOD_TOOLS_LIST: Int64 = 3
comptime SUPER_EXPOSE_METHOD_TOOLS_CALL: Int64 = 4
comptime SUPER_EXPOSE_METHOD_NOTIFICATION: Int64 = 5
comptime SUPER_EXPOSE_METHOD_SERVER_DISCOVER: Int64 = 6

comptime SUPER_EXPOSE_TOOL_UNKNOWN: Int64 = 0
comptime SUPER_EXPOSE_TOOL_START: Int64 = 1
comptime SUPER_EXPOSE_TOOL_STATUS: Int64 = 2
comptime SUPER_EXPOSE_TOOL_RESULT: Int64 = 3
comptime SUPER_EXPOSE_TOOL_CANCEL: Int64 = 4
comptime SUPER_EXPOSE_TOOL_LIST: Int64 = 5
comptime SUPER_EXPOSE_TOOL_EXEC: Int64 = 6
comptime SUPER_EXPOSE_TOOL_EVENTS: Int64 = 7
comptime SUPER_EXPOSE_TOOL_SESSION_PROMPT_WRITE: Int64 = 8
comptime SUPER_EXPOSE_TOOL_SESSION_PREEMPT: Int64 = 9
comptime SUPER_EXPOSE_TOOL_SESSION_OUTPUT_READ: Int64 = 10

def super_expose_method(view: ProdexRichStringView) -> Int64:
    if rich_view_matches_literal["server/discover"](view, False):
        return SUPER_EXPOSE_METHOD_SERVER_DISCOVER
    if rich_view_matches_literal["initialize"](view, False):
        return SUPER_EXPOSE_METHOD_INITIALIZE
    if rich_view_matches_literal["ping"](view, False):
        return SUPER_EXPOSE_METHOD_PING
    if rich_view_matches_literal["tools/list"](view, False):
        return SUPER_EXPOSE_METHOD_TOOLS_LIST
    if rich_view_matches_literal["tools/call"](view, False):
        return SUPER_EXPOSE_METHOD_TOOLS_CALL
    if (
        rich_view_matches_literal["notifications/initialized"](view, False)
        or rich_view_matches_literal["notifications/cancelled"](view, False)
    ):
        return SUPER_EXPOSE_METHOD_NOTIFICATION
    return SUPER_EXPOSE_METHOD_UNKNOWN

def super_expose_tool(view: ProdexRichStringView) -> Int64:
    if rich_view_matches_literal["prodex_super_start"](view, False):
        return SUPER_EXPOSE_TOOL_START
    if rich_view_matches_literal["prodex_super_status"](view, False):
        return SUPER_EXPOSE_TOOL_STATUS
    if rich_view_matches_literal["prodex_super_result"](view, False):
        return SUPER_EXPOSE_TOOL_RESULT
    if rich_view_matches_literal["prodex_super_cancel"](view, False):
        return SUPER_EXPOSE_TOOL_CANCEL
    if rich_view_matches_literal["prodex_super_list"](view, False):
        return SUPER_EXPOSE_TOOL_LIST
    if rich_view_matches_literal["prodex_super_exec"](view, False):
        return SUPER_EXPOSE_TOOL_EXEC
    if rich_view_matches_literal["prodex_super_events"](view, False):
        return SUPER_EXPOSE_TOOL_EVENTS
    if rich_view_matches_literal["prodex_session_prompt_write"](view, False):
        return SUPER_EXPOSE_TOOL_SESSION_PROMPT_WRITE
    if rich_view_matches_literal["prodex_session_preempt"](view, False):
        return SUPER_EXPOSE_TOOL_SESSION_PREEMPT
    if rich_view_matches_literal["prodex_session_output_read"](view, False):
        return SUPER_EXPOSE_TOOL_SESSION_OUTPUT_READ
    return SUPER_EXPOSE_TOOL_UNKNOWN

@export("prodex_mojo_super_expose_route_v1")
def prodex_mojo_super_expose_route_v1(
    abi_version: Int64,
    method_address: UInt,
    method_length: Int64,
    tool_address: UInt,
    tool_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        method_length < 0
        or method_length > SUPER_EXPOSE_MAX_NAME_BYTES
        or tool_length < 0
        or tool_length > SUPER_EXPOSE_MAX_NAME_BYTES
        or output_address == 0
        or (method_length > 0 and method_address == 0)
        or (tool_length > 0 and tool_address == 0)
    ):
        return 1
    var method = ProdexRichStringView(method_address, UInt(method_length))
    var tool = ProdexRichStringView(tool_address, UInt(tool_length))
    if (
        not rich_view_valid(method, SUPER_EXPOSE_MAX_NAME_BYTES)
        or not rich_view_valid(tool, SUPER_EXPOSE_MAX_NAME_BYTES)
    ):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = super_expose_method(method)
    output[unsafe_offset=1] = super_expose_tool(tool)
    return 0

comptime SUPER_EXPOSE_MODE_FULL: Int64 = 0
comptime SUPER_EXPOSE_MODE_EXEC: Int64 = 1

@export("prodex_mojo_super_expose_tool_allowed_v1")
def prodex_mojo_super_expose_tool_allowed_v1(
    abi_version: Int64,
    mode: Int64,
    tool_address: UInt,
    tool_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        mode < SUPER_EXPOSE_MODE_FULL
        or mode > SUPER_EXPOSE_MODE_EXEC
        or tool_length < 0
        or tool_length > SUPER_EXPOSE_MAX_NAME_BYTES
        or output_address == 0
        or (tool_length > 0 and tool_address == 0)
    ):
        return 1
    var tool = ProdexRichStringView(tool_address, UInt(tool_length))
    if not rich_view_valid(tool, SUPER_EXPOSE_MAX_NAME_BYTES):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var tool_kind = super_expose_tool(tool)
    if mode == SUPER_EXPOSE_MODE_EXEC:
        output[] = 1 if tool_kind == SUPER_EXPOSE_TOOL_EXEC else 0
    else:
        output[] = 1 if tool_kind != SUPER_EXPOSE_TOOL_UNKNOWN else 0
    return 0


comptime SUPER_EXPOSE_TUNNEL_ID_BYTES: Int64 = 39
comptime SUPER_EXPOSE_TUNNEL_PREFIX_BYTES: Int64 = 7

def super_expose_tunnel_id_valid(view: ProdexRichStringView) -> Bool:
    if Int64(view.len) != SUPER_EXPOSE_TUNNEL_ID_BYTES or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    var prefix = StringSlice("tunnel_").unsafe_ptr()
    for index in range(SUPER_EXPOSE_TUNNEL_PREFIX_BYTES):
        if ptr[unsafe_offset=index] != prefix[unsafe_offset=index]:
            return False
    for index in range(SUPER_EXPOSE_TUNNEL_PREFIX_BYTES, SUPER_EXPOSE_TUNNEL_ID_BYTES):
        var byte = ptr[unsafe_offset=index]
        var lowercase = byte >= 97 and byte <= 122
        var digit = byte >= 48 and byte <= 57
        if not lowercase and not digit:
            return False
    return True

@export("prodex_mojo_super_expose_tunnel_id_valid_v1")
def prodex_mojo_super_expose_tunnel_id_valid_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        value_length < 0
        or value_length > SUPER_EXPOSE_MAX_NAME_BYTES
        or output_address == 0
        or (value_length > 0 and value_address == 0)
    ):
        return 1
    var value = ProdexRichStringView(value_address, UInt(value_length))
    if not rich_view_valid(value, SUPER_EXPOSE_MAX_NAME_BYTES):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = 1 if super_expose_tunnel_id_valid(value) else 0
    return 0



def super_expose_ascii_trim_bounds(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Array[Int64, 2]:
    var left = start
    var right = end
    var ptr = rich_view_ptr(view)
    while left < right:
        var byte = ptr[unsafe_offset=left]
        if byte == 9 or byte == 10 or byte == 13 or byte == 32:
            left += 1
        else:
            break
    while right > left:
        var byte = ptr[unsafe_offset=right - 1]
        if byte == 9 or byte == 10 or byte == 13 or byte == 32:
            right -= 1
        else:
            break
    var bounds = Array[Int64, 2](fill=0)
    bounds[0] = left
    bounds[1] = right
    return bounds^

def super_expose_range_matches_literal(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var length = end - start
    if length != Int64(literal.byte_length()) or start < 0:
        return False
    var ptr = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(length):
        if ptr[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True

def super_expose_ascii_digit(byte: UInt8) -> Bool:
    return byte >= 48 and byte <= 57

def super_expose_ascii_hex(byte: UInt8) -> Bool:
    return (
        (byte >= 48 and byte <= 57)
        or (byte >= 97 and byte <= 102)
        or (byte >= 65 and byte <= 70)
    )

def super_expose_tunnel_client_version_line_valid(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end <= start:
        return False
    var ptr = rich_view_ptr(view)
    var cursor = start
    for component in range(3):
        var digits: Int64 = 0
        while cursor < end and super_expose_ascii_digit(ptr[unsafe_offset=cursor]):
            cursor += 1
            digits += 1
        if digits == 0:
            return False
        if component < 2:
            if cursor >= end or ptr[unsafe_offset=cursor] != 46:
                return False
            cursor += 1
    if cursor >= end or ptr[unsafe_offset=cursor] != 43:
        return False
    cursor += 1
    var first_sha = cursor
    for _ in range(40):
        if cursor >= end or not super_expose_ascii_hex(ptr[unsafe_offset=cursor]):
            return False
        cursor += 1
    var marker = StringSlice(" (git sha: ")
    var marker_len = Int64(marker.byte_length())
    if not super_expose_range_matches_literal(
        view, cursor, cursor + marker_len, marker
    ):
        return False
    cursor += marker_len
    var second_sha = cursor
    for index in range(40):
        if cursor >= end or not super_expose_ascii_hex(ptr[unsafe_offset=cursor]):
            return False
        if ptr[unsafe_offset=first_sha + index] != ptr[unsafe_offset=second_sha + index]:
            return False
        cursor += 1
    if cursor >= end or ptr[unsafe_offset=cursor] != 41:
        return False
    cursor += 1
    return cursor == end

def super_expose_tunnel_client_version_output_valid(
    view: ProdexRichStringView,
) -> Bool:
    var length = Int64(view.len)
    var line_start: Int64 = 0
    var cursor: Int64 = 0
    while cursor <= length:
        if cursor == length or rich_view_ptr(view)[unsafe_offset=cursor] == 10:
            var bounds = super_expose_ascii_trim_bounds(view, line_start, cursor)
            if super_expose_tunnel_client_version_line_valid(
                view, bounds[0], bounds[1]
            ):
                return True
            line_start = cursor + 1
        cursor += 1
    return False

@export("prodex_mojo_super_expose_tunnel_client_version_valid_v1")
def prodex_mojo_super_expose_tunnel_client_version_valid_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_ABI_VERSION:
        return 4
    if (
        value_length < 0
        or value_length > 16_384
        or output_address == 0
        or (value_length > 0 and value_address == 0)
    ):
        return 1
    var value = ProdexRichStringView(value_address, UInt(value_length))
    if not rich_view_valid(value, 16_384):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[] = (
        1 if super_expose_tunnel_client_version_output_valid(value) else 0
    )
    return 0
