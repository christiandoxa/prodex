
from std.memory import Pointer

from rich_types import ProdexRichStringView, rich_view_ptr


comptime COMPATIBILITY_SURFACE_ABI_VERSION: Int64 = 1
comptime COMPATIBILITY_SURFACE_MAX_TOOL_LABELS: Int64 = 1_024

comptime COMPAT_ROUTE_RESPONSES: Int64 = 0
comptime COMPAT_ROUTE_COMPACT: Int64 = 1
comptime COMPAT_ROUTE_CHAT_COMPLETIONS: Int64 = 2
comptime COMPAT_ROUTE_STANDARD: Int64 = 3

comptime COMPAT_FAMILY_UNKNOWN: Int64 = 0
comptime COMPAT_FAMILY_CODEX: Int64 = 1
comptime COMPAT_FAMILY_OPENAI_COMPATIBLE: Int64 = 2

comptime COMPAT_CLIENT_UNKNOWN: Int64 = 0
comptime COMPAT_CLIENT_CODEX_SUBAGENT: Int64 = 1
comptime COMPAT_CLIENT_CODEX_CLI: Int64 = 2
comptime COMPAT_CLIENT_CHAT_COMPLETIONS: Int64 = 3
comptime COMPAT_CLIENT_RESPONSES: Int64 = 4

comptime COMPAT_STREAM_UNARY: Int64 = 0
comptime COMPAT_STREAM_STREAMING: Int64 = 1

comptime COMPAT_TOOL_TOOLS: Int64 = 1
comptime COMPAT_TOOL_WEB: Int64 = 2
comptime COMPAT_TOOL_MCP: Int64 = 4
comptime COMPAT_TOOL_COMPUTER: Int64 = 8
comptime COMPAT_TOOL_SHELL: Int64 = 16
comptime COMPAT_TOOL_APPROVAL: Int64 = 32

comptime COMPAT_CONTINUATION_PREVIOUS_RESPONSE: Int64 = 1
comptime COMPAT_CONTINUATION_TURN_STATE: Int64 = 2
comptime COMPAT_CONTINUATION_SESSION: Int64 = 4

comptime COMPAT_WARNING_UNKNOWN_CLIENT: Int64 = 1
comptime COMPAT_WARNING_WEBSOCKET_PREVIOUS_RESPONSE_WITHOUT_TURN_STATE: Int64 = 2

comptime COMPAT_ORIGIN_EXTERNAL: Int64 = 0
comptime COMPAT_ORIGIN_INTERNAL: Int64 = 1


def compat_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def compat_view_contains(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    var needle_len = Int64(literal.byte_length())
    var view_len = Int64(view.len)
    if needle_len == 0:
        return True
    if view_len < needle_len or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    var needle = literal.unsafe_ptr()
    for start in range(view_len - needle_len + 1):
        var matched = True
        for offset in range(needle_len):
            if compat_ascii_lower(ptr[unsafe_offset=start + offset]) != needle[unsafe_offset=offset]:
                matched = False
                break
        if matched:
            return True
    return False


def compat_tool_flags(
    tool_views_address: UInt,
    tool_count: Int64,
    tools_present: Int64,
) -> Int64:
    var flags: Int64 = 0
    if tools_present == 1:
        flags |= COMPAT_TOOL_TOOLS
    if tool_count <= 0:
        return flags
    var views = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(tool_views_address)
    )
    for index in range(tool_count):
        var view = views[unsafe_offset=index].copy()
        if compat_view_contains(view, StringSlice("web")):
            flags |= COMPAT_TOOL_WEB
        if compat_view_contains(view, StringSlice("mcp")):
            flags |= COMPAT_TOOL_MCP
        if compat_view_contains(view, StringSlice("computer")):
            flags |= COMPAT_TOOL_COMPUTER
        if compat_view_contains(view, StringSlice("shell")) or compat_view_contains(
            view, StringSlice("bash")
        ):
            flags |= COMPAT_TOOL_SHELL
        if compat_view_contains(view, StringSlice("approval")):
            flags |= COMPAT_TOOL_APPROVAL
    return flags


@export("prodex_runtime_compatibility_surface_plan_v1")
def prodex_runtime_compatibility_surface_plan_v1(
    abi_version: Int64,
    route_kind: Int64,
    transport_websocket: Int64,
    codex_headers: Int64,
    subagent_header: Int64,
    internal_origin: Int64,
    explicit_stream: Int64,
    previous_response_present: Int64,
    turn_state_present: Int64,
    session_present: Int64,
    tools_present: Int64,
    user_agent_address: UInt,
    user_agent_length: Int64,
    tool_views_address: UInt,
    tool_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != COMPATIBILITY_SURFACE_ABI_VERSION:
        return 4
    if (
        route_kind < COMPAT_ROUTE_RESPONSES
        or route_kind > COMPAT_ROUTE_STANDARD
        or transport_websocket < 0
        or transport_websocket > 1
        or codex_headers < 0
        or codex_headers > 1
        or subagent_header < 0
        or subagent_header > 1
        or internal_origin < 0
        or internal_origin > 1
        or explicit_stream < -1
        or explicit_stream > 1
        or previous_response_present < 0
        or previous_response_present > 1
        or turn_state_present < 0
        or turn_state_present > 1
        or session_present < 0
        or session_present > 1
        or tools_present < 0
        or tools_present > 1
        or user_agent_length < 0
        or (user_agent_length > 0 and user_agent_address == 0)
        or tool_count < 0
        or tool_count > COMPATIBILITY_SURFACE_MAX_TOOL_LABELS
        or (tool_count > 0 and tool_views_address == 0)
        or output_address == 0
    ):
        return 1

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var user_agent = ProdexRichStringView(user_agent_address, UInt(user_agent_length))
    var codex_client = (
        codex_headers == 1
        or transport_websocket == 1
        or compat_view_contains(user_agent, StringSlice("codex"))
    )

    var family = COMPAT_FAMILY_UNKNOWN
    var client = COMPAT_CLIENT_UNKNOWN
    var warnings: Int64 = 0
    if codex_client:
        family = COMPAT_FAMILY_CODEX
        client = (
            COMPAT_CLIENT_CODEX_SUBAGENT
            if subagent_header == 1
            else COMPAT_CLIENT_CODEX_CLI
        )
    elif route_kind != COMPAT_ROUTE_STANDARD:
        family = COMPAT_FAMILY_OPENAI_COMPATIBLE
        client = (
            COMPAT_CLIENT_CHAT_COMPLETIONS
            if route_kind == COMPAT_ROUTE_CHAT_COMPLETIONS
            else COMPAT_CLIENT_RESPONSES
        )
    else:
        warnings |= COMPAT_WARNING_UNKNOWN_CLIENT

    var stream = COMPAT_STREAM_UNARY
    if route_kind != COMPAT_ROUTE_COMPACT and (
        transport_websocket == 1
        or explicit_stream == 1
        or (route_kind == COMPAT_ROUTE_RESPONSES and explicit_stream != 0)
    ):
        stream = COMPAT_STREAM_STREAMING

    var continuation: Int64 = 0
    if previous_response_present == 1:
        continuation |= COMPAT_CONTINUATION_PREVIOUS_RESPONSE
    if turn_state_present == 1:
        continuation |= COMPAT_CONTINUATION_TURN_STATE
    if session_present == 1:
        continuation |= COMPAT_CONTINUATION_SESSION

    if (
        transport_websocket == 1
        and previous_response_present == 1
        and turn_state_present == 0
    ):
        warnings |= COMPAT_WARNING_WEBSOCKET_PREVIOUS_RESPONSE_WITHOUT_TURN_STATE

    output[unsafe_offset=0] = family
    output[unsafe_offset=1] = client
    output[unsafe_offset=2] = stream
    output[unsafe_offset=3] = compat_tool_flags(
        tool_views_address, tool_count, tools_present
    )
    output[unsafe_offset=4] = continuation
    output[unsafe_offset=5] = warnings
    output[unsafe_offset=6] = (
        COMPAT_ORIGIN_INTERNAL if internal_origin == 1 else COMPAT_ORIGIN_EXTERNAL
    )
    return 0
