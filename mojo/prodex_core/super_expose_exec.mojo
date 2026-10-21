from std.memory import Pointer

from rich_text import rich_codepoint, rich_codepoint_width, rich_view_ptr, rich_view_valid, rich_view_matches_literal, rich_utf8_prefix_end
from rich_types import ProdexRichStringView
from parsed_json import pj_literal
from json_sink import JsonSink, js_literal, js_escaped, js_string
from optional_tools_policy import (
    optional_tool_class, OPTIONAL_TOOL_RTK, OPTIONAL_TOOL_CODEBASE_MEMORY,
    OPTIONAL_TOOL_PLAYWRIGHT, OPTIONAL_TOOL_CAVEMAN, OPTIONAL_TOOL_PONYTAIL, OPTIONAL_TOOL_PRESIDIO,
)

comptime SUPER_EXPOSE_EXEC_ABI_VERSION: Int64 = 1
comptime SUPER_EXPOSE_EXEC_PROGRAM: Int64 = 0
comptime SUPER_EXPOSE_EXEC_CWD: Int64 = 1
comptime SUPER_EXPOSE_EXEC_ARGUMENT_COUNT: Int64 = 2
comptime SUPER_EXPOSE_EXEC_ARGUMENT: Int64 = 3
comptime SUPER_EXPOSE_EXEC_ENV_COUNT: Int64 = 4
comptime SUPER_EXPOSE_EXEC_ENV_ENTRY: Int64 = 5
comptime SUPER_EXPOSE_EXEC_STDIN: Int64 = 6
comptime SUPER_EXPOSE_EXEC_TIMEOUT: Int64 = 7
comptime SUPER_EXPOSE_EXEC_TEXT_CUT: Int64 = 8

comptime SUPER_EXPOSE_EXEC_OK: Int64 = 1
comptime SUPER_EXPOSE_EXEC_INVALID: Int64 = 2
comptime SUPER_EXPOSE_EXEC_TOTAL_LIMIT: Int64 = 3
comptime SUPER_EXPOSE_EXEC_ARGUMENT_LIMIT: Int64 = 4
comptime SUPER_EXPOSE_EXEC_ENV_KEY: Int64 = 5
comptime SUPER_EXPOSE_EXEC_ENV_VALUE: Int64 = 6
comptime SUPER_EXPOSE_EXEC_COUNT_LIMIT: Int64 = 7
comptime SUPER_EXPOSE_EXEC_STDIN_LIMIT: Int64 = 8
comptime SUPER_EXPOSE_EXEC_TIMEOUT_LIMIT: Int64 = 9

comptime SUPER_EXPOSE_EXEC_MAX_PROGRAM_BYTES: Int64 = 4 * 1024
comptime SUPER_EXPOSE_EXEC_MAX_ARGUMENTS: UInt64 = 256
comptime SUPER_EXPOSE_EXEC_MAX_ARGUMENT_BYTES: UInt64 = 16 * 1024
comptime SUPER_EXPOSE_EXEC_MAX_ARGUMENTS_BYTES: UInt64 = 256 * 1024
comptime SUPER_EXPOSE_EXEC_MAX_CWD_BYTES: Int64 = 4 * 1024
comptime SUPER_EXPOSE_EXEC_MAX_ENV_ENTRIES: UInt64 = 128
comptime SUPER_EXPOSE_EXEC_MAX_ENV_KEY_BYTES: UInt64 = 256
comptime SUPER_EXPOSE_EXEC_MAX_ENV_VALUE_BYTES: UInt64 = 16 * 1024
comptime SUPER_EXPOSE_EXEC_MAX_STDIN_BYTES: UInt64 = 256 * 1024
comptime SUPER_EXPOSE_EXEC_MAX_TIMEOUT_MS: UInt64 = 120_000


def super_expose_exec_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


def super_expose_exec_has_control(view: ProdexRichStringView) -> Bool:
    var source = rich_view_ptr(view)
    var index: Int64 = 0
    while index < Int64(view.len):
        var width = rich_codepoint_width(source[unsafe_offset=index])
        var codepoint = rich_codepoint(source, index, width)
        if codepoint <= 31 or (codepoint >= 127 and codepoint <= 159):
            return True
        index += width
    return False


def super_expose_exec_has_nul(view: ProdexRichStringView) -> Bool:
    var source = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        if source[unsafe_offset=index] == 0:
            return True
    return False


def super_expose_exec_path_valid(
    view: ProdexRichStringView, max_bytes: Int64
) -> Bool:
    if view.len == 0 or view.len > UInt(max_bytes):
        return False
    if not rich_view_valid(view, max_bytes):
        return False
    return not super_expose_exec_has_nul(view) and not super_expose_exec_has_control(view)


def super_expose_exec_argument_valid(view: ProdexRichStringView) -> Bool:
    if view.len > UInt(SUPER_EXPOSE_EXEC_MAX_ARGUMENT_BYTES):
        return False
    if not rich_view_valid(view, Int64(SUPER_EXPOSE_EXEC_MAX_ARGUMENT_BYTES)):
        return False
    return not super_expose_exec_has_nul(view)


def super_expose_exec_env_key_valid(view: ProdexRichStringView) -> Bool:
    if view.len == 0 or view.len > UInt(SUPER_EXPOSE_EXEC_MAX_ENV_KEY_BYTES):
        return False
    if not rich_view_valid(view, Int64(SUPER_EXPOSE_EXEC_MAX_ENV_KEY_BYTES)):
        return False
    var source = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        if source[unsafe_offset=index] == 61:
            return False
    return not super_expose_exec_has_nul(view) and not super_expose_exec_has_control(view)


def super_expose_exec_env_value_valid(view: ProdexRichStringView) -> Bool:
    if view.len > UInt(SUPER_EXPOSE_EXEC_MAX_ENV_VALUE_BYTES):
        return False
    if not rich_view_valid(view, Int64(SUPER_EXPOSE_EXEC_MAX_ENV_VALUE_BYTES)):
        return False
    return not super_expose_exec_has_nul(view)


@export("prodex_mojo_super_expose_exec_validation_v1")
def prodex_mojo_super_expose_exec_validation_v1(
    abi_version: Int64,
    operation: Int64,
    input_address: UInt,
    input_length: Int64,
    secondary_address: UInt,
    secondary_length: Int64,
    numeric_value: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_EXEC_ABI_VERSION:
        return 4
    if (
        operation < SUPER_EXPOSE_EXEC_PROGRAM
        or operation > SUPER_EXPOSE_EXEC_TEXT_CUT
        or input_length < 0
        or secondary_length < 0
        or output_address == 0 or output_address % 8 != 0
        or (input_length > 0 and input_address == 0)
        or (secondary_length > 0 and secondary_address == 0)
    ):
        return 1

    if operation == SUPER_EXPOSE_EXEC_TEXT_CUT:
        if not rich_view_valid(super_expose_exec_view(input_address, input_length), 9223372036854775807):
            return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0

    if operation == SUPER_EXPOSE_EXEC_TEXT_CUT:
        output[unsafe_offset=0] = rich_utf8_prefix_end(super_expose_exec_view(input_address, input_length), numeric_value)
        return 0

    if operation == SUPER_EXPOSE_EXEC_PROGRAM:
        var value = super_expose_exec_view(input_address, input_length)
        if super_expose_exec_path_valid(value, SUPER_EXPOSE_EXEC_MAX_PROGRAM_BYTES):
            output[unsafe_offset=0] = SUPER_EXPOSE_EXEC_OK
        else:
            output[unsafe_offset=0] = SUPER_EXPOSE_EXEC_INVALID
        return 0

    if operation == SUPER_EXPOSE_EXEC_CWD:
        var value = super_expose_exec_view(input_address, input_length)
        if super_expose_exec_path_valid(value, SUPER_EXPOSE_EXEC_MAX_CWD_BYTES):
            output[unsafe_offset=0] = SUPER_EXPOSE_EXEC_OK
        else:
            output[unsafe_offset=0] = SUPER_EXPOSE_EXEC_INVALID
        return 0

    if operation == SUPER_EXPOSE_EXEC_ARGUMENT_COUNT:
        output[unsafe_offset=0] = (
            SUPER_EXPOSE_EXEC_OK
            if numeric_value <= SUPER_EXPOSE_EXEC_MAX_ARGUMENTS
            else SUPER_EXPOSE_EXEC_COUNT_LIMIT
        )
        return 0

    if operation == SUPER_EXPOSE_EXEC_ARGUMENT:
        var value = super_expose_exec_view(input_address, input_length)
        if not super_expose_exec_argument_valid(value):
            output[unsafe_offset=0] = SUPER_EXPOSE_EXEC_ARGUMENT_LIMIT
            return 0
        if numeric_value > SUPER_EXPOSE_EXEC_MAX_ARGUMENTS_BYTES:
            output[unsafe_offset=0] = SUPER_EXPOSE_EXEC_TOTAL_LIMIT
            return 0
        var next_total = numeric_value + UInt64(input_length)
        if next_total > SUPER_EXPOSE_EXEC_MAX_ARGUMENTS_BYTES:
            output[unsafe_offset=0] = SUPER_EXPOSE_EXEC_TOTAL_LIMIT
            return 0
        output[unsafe_offset=0] = SUPER_EXPOSE_EXEC_OK
        output[unsafe_offset=1] = Int64(next_total)
        return 0

    if operation == SUPER_EXPOSE_EXEC_ENV_COUNT:
        output[unsafe_offset=0] = (
            SUPER_EXPOSE_EXEC_OK
            if numeric_value <= SUPER_EXPOSE_EXEC_MAX_ENV_ENTRIES
            else SUPER_EXPOSE_EXEC_COUNT_LIMIT
        )
        return 0

    if operation == SUPER_EXPOSE_EXEC_ENV_ENTRY:
        var key = super_expose_exec_view(input_address, input_length)
        if not super_expose_exec_env_key_valid(key):
            output[unsafe_offset=0] = SUPER_EXPOSE_EXEC_ENV_KEY
            return 0
        var value = super_expose_exec_view(secondary_address, secondary_length)
        if not super_expose_exec_env_value_valid(value):
            output[unsafe_offset=0] = SUPER_EXPOSE_EXEC_ENV_VALUE
            return 0
        output[unsafe_offset=0] = SUPER_EXPOSE_EXEC_OK
        return 0

    if operation == SUPER_EXPOSE_EXEC_STDIN:
        output[unsafe_offset=0] = (
            SUPER_EXPOSE_EXEC_OK
            if numeric_value <= SUPER_EXPOSE_EXEC_MAX_STDIN_BYTES
            else SUPER_EXPOSE_EXEC_STDIN_LIMIT
        )
        return 0

    output[unsafe_offset=0] = (
        SUPER_EXPOSE_EXEC_OK
        if numeric_value >= 1 and numeric_value <= SUPER_EXPOSE_EXEC_MAX_TIMEOUT_MS
        else SUPER_EXPOSE_EXEC_TIMEOUT_LIMIT
    )
    return 0


def super_expose_write_optional_alias_plan(
    sink: Pointer[mut=True, JsonSink, _], alias: ProdexRichStringView,
):
    var kind = optional_tool_class(alias)
    if kind < 0:
        js_literal(sink, StringSlice("null"))
        return
    if kind == OPTIONAL_TOOL_RTK or kind == OPTIONAL_TOOL_CODEBASE_MEMORY:
        var name = pj_literal(StringSlice("rtk")) if kind == OPTIONAL_TOOL_RTK else pj_literal(StringSlice("codebase-memory-mcp"))
        js_literal(sink, StringSlice('{"tool":'))
        js_string(sink, name)
        js_literal(sink, StringSlice(',"program":{"kind":"tool_path","missing_path_message":"validated optional tool '))
        js_escaped(sink, name)
        js_literal(sink, StringSlice(' has no executable path"},"prefix":[]}'))
        return
    if kind == OPTIONAL_TOOL_PLAYWRIGHT and not rich_view_matches_literal["playwright"](alias, False):
        js_literal(sink, StringSlice('{"tool":"playwright-mcp","program":{"kind":"tool_path","missing_path_message":"validated Playwright MCP has no npx executable path"},"prefix":["--no-install","@playwright/mcp"]}'))
        return
    if kind == OPTIONAL_TOOL_PRESIDIO:
        js_literal(sink, StringSlice('{"tool":"presidio","program":{"kind":"prodex"},"prefix":["super","--no-sub-agent","--presidio"]}'))
        return
    if kind != OPTIONAL_TOOL_CAVEMAN and kind != OPTIONAL_TOOL_PONYTAIL and kind != OPTIONAL_TOOL_PLAYWRIGHT:
        js_literal(sink, StringSlice("null"))
        return
    var name = pj_literal(StringSlice("caveman"))
    var activation = name.copy()
    if kind == OPTIONAL_TOOL_PONYTAIL:
        name = pj_literal(StringSlice("ponytail"))
        activation = name.copy()
    elif kind == OPTIONAL_TOOL_PLAYWRIGHT:
        name = pj_literal(StringSlice("playwright-mcp"))
        activation = pj_literal(StringSlice("playwright"))
    js_literal(sink, StringSlice('{"tool":'))
    js_string(sink, name)
    js_literal(sink, StringSlice(',"program":{"kind":"prodex"},"prefix":["super","--no-sub-agent","--no-presidio","--tool",'))
    js_string(sink, activation)
    js_literal(sink, StringSlice("]}"))


@export("prodex_mojo_super_expose_optional_alias_plan_v1")
def prodex_mojo_super_expose_optional_alias_plan_v1(
    abi_version: Int64, alias_address: UInt64, alias_length: Int64,
    output_address: UInt64, capacity: Int64, written_address: UInt64,
) abi("C") -> Int64:
    if abi_version != SUPER_EXPOSE_EXEC_ABI_VERSION:
        return 4
    if alias_length < 0 or capacity < 0 or output_address == 0 or written_address == 0 or (alias_length > 0 and alias_address == 0):
        return 1
    var alias = ProdexRichStringView(UInt(alias_address), UInt(alias_length))
    if not rich_view_valid(alias, alias_length):
        return 1
    var measure = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), 0, 0, True, False)
    super_expose_write_optional_alias_plan(Pointer(to=measure), alias)
    if measure.failed or measure.written > capacity:
        return 2
    var sink = JsonSink(Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), capacity, 0, False, False)
    super_expose_write_optional_alias_plan(Pointer(to=sink), alias)
    if sink.failed or sink.written != measure.written:
        return 2
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))[] = sink.written
    return 0
