# User-facing diagnostics for compiled optimizer preflight decisions.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid
from parsed_json import pj_literal
from json_sink import JsonSink, js_literal, js_view


def optional_tool_shape_key(issue: Int64) -> ProdexRichStringView:
    if issue == 1:
        return pj_literal("features")
    if issue == 2:
        return pj_literal("marketplaces")
    if issue == 3:
        return pj_literal("plugins")
    if issue == 4:
        return pj_literal("ponytail")
    if issue == 5:
        return pj_literal("ponytail@ponytail")
    if issue == 6:
        return pj_literal("mcp_servers")
    return pj_literal("mcp_servers.codebase-memory-mcp") if issue == 7 else pj_literal("mcp_servers.playwright")


def optional_tool_diagnostic(
    writer: Pointer[mut=True, JsonSink, _], group: Int64, issue: Int64,
    name: ProdexRichStringView,
):
    if group == 0:
        if issue <= 5:
            js_literal(writer, StringSlice("configuration entry `"))
        js_view(writer, optional_tool_shape_key(issue))
        if issue <= 5:
            js_literal(writer, StringSlice("`"))
        js_literal(writer, StringSlice(" must be a TOML table"))
        return
    js_literal(writer, StringSlice("required MCP server `mcp_servers."))
    js_view(writer, name)
    if issue == 1:
        js_literal(writer, StringSlice("` is disabled (`enabled = false`); refusing to override user configuration"))
    elif issue == 2:
        js_literal(writer, StringSlice("` cannot be safely verified: `enabled` must be a boolean"))
    else:
        js_literal(writer, StringSlice("` cannot be safely verified: inherited configuration is custom or incomplete; refusing to replace user configuration"))


@export("prodex_optional_tool_diagnostic_v1")
def prodex_optional_tool_diagnostic_v1(
    version: Int64, group: Int64, issue: Int64, name_address: UInt,
    name_length: Int64, output_address: UInt, capacity: Int64, written_address: UInt,
) abi("C") -> Int64:
    if version != 1:
        return 4
    if (group < 0 or group > 1 or issue < 1 or issue > Int64(8 if group == 0 else 3)
        or name_length < 0 or group == 0 and name_length != 0
        or capacity < 0 or written_address == 0 or written_address % 8 != 0):
        return 1
    var name = ProdexRichStringView(name_address, UInt(name_length))
    if not rich_view_valid(name, 0x7FFFFFFFFFFFFFFF):
        return 1
    var writer = JsonSink(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=1), 0, 0, True, False,
    )
    optional_tool_diagnostic(Pointer(to=writer), group, issue, name)
    if writer.failed or writer.written > capacity:
        return 3
    if output_address == 0:
        return 1
    writer.output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    writer.capacity = capacity
    writer.written = 0
    writer.measuring = False
    optional_tool_diagnostic(Pointer(to=writer), group, issue, name)
    if writer.failed:
        return 3
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))[] = writer.written
    return 0
