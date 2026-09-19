from std.memory import Pointer

from rich_text import rich_view_matches_literal, rich_view_valid
from rich_types import ProdexRichStringView

comptime SUPER_EXPOSE_ABI_VERSION: Int64 = 1
comptime SUPER_EXPOSE_MAX_NAME_BYTES: Int64 = 128

comptime SUPER_EXPOSE_METHOD_UNKNOWN: Int64 = 0
comptime SUPER_EXPOSE_METHOD_INITIALIZE: Int64 = 1
comptime SUPER_EXPOSE_METHOD_PING: Int64 = 2
comptime SUPER_EXPOSE_METHOD_TOOLS_LIST: Int64 = 3
comptime SUPER_EXPOSE_METHOD_TOOLS_CALL: Int64 = 4
comptime SUPER_EXPOSE_METHOD_NOTIFICATION: Int64 = 5

comptime SUPER_EXPOSE_TOOL_UNKNOWN: Int64 = 0
comptime SUPER_EXPOSE_TOOL_START: Int64 = 1
comptime SUPER_EXPOSE_TOOL_STATUS: Int64 = 2
comptime SUPER_EXPOSE_TOOL_RESULT: Int64 = 3
comptime SUPER_EXPOSE_TOOL_CANCEL: Int64 = 4
comptime SUPER_EXPOSE_TOOL_LIST: Int64 = 5
comptime SUPER_EXPOSE_TOOL_EXEC: Int64 = 6

def super_expose_method(view: ProdexRichStringView) -> Int64:
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
