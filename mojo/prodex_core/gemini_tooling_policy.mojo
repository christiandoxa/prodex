from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime GEMINI_TOOLING_ABI_VERSION: Int64 = 1
comptime GEMINI_TOOLING_OK: Int64 = 0
comptime GEMINI_TOOLING_INVALID: Int64 = 1
comptime GEMINI_TOOLING_CAPACITY: Int64 = 3
comptime GEMINI_TOOLING_ABI: Int64 = 4

comptime TOOL_ALIAS_EXEC_COMMAND: Int64 = 1 << 0
comptime TOOL_ALIAS_RUN_SHELL_COMMAND: Int64 = 1 << 1
comptime TOOL_ALIAS_SHELL: Int64 = 1 << 2
comptime TOOL_ALIAS_BASH: Int64 = 1 << 3
comptime TOOL_ALIAS_APPLY_PATCH: Int64 = 1 << 4
comptime TOOL_ALIAS_EDIT: Int64 = 1 << 5
comptime TOOL_ALIAS_REPLACE: Int64 = 1 << 6
comptime TOOL_ALIAS_READ_FILE: Int64 = 1 << 7
comptime TOOL_ALIAS_READ: Int64 = 1 << 8
comptime TOOL_ALIAS_READ_MANY_FILES: Int64 = 1 << 9
comptime TOOL_ALIAS_GLOB: Int64 = 1 << 10
comptime TOOL_ALIAS_GREP: Int64 = 1 << 11
comptime TOOL_ALIAS_RIP_GREP: Int64 = 1 << 12
comptime TOOL_ALIAS_RG: Int64 = 1 << 13
comptime TOOL_ALIAS_SEARCH: Int64 = 1 << 14
comptime TOOL_ALIAS_WRITE_FILE: Int64 = 1 << 15
comptime TOOL_ALIAS_WRITE: Int64 = 1 << 16

comptime TOOL_FLAG_MUTATING: Int64 = 1
comptime TOOL_FLAG_CANONICAL_EXEC: Int64 = 2


def tooling_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def tooling_output_range_equals[literal: StaticString](
    output: Pointer[mut=True, UInt8, _],
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end - start != Int64(literal.byte_length()):
        return False
    var wanted = literal.unsafe_ptr()
    for index in range(end - start):
        if output[unsafe_offset=start + index] != wanted[unsafe_offset=index]:
            return False
    return True


def tooling_output_equals[literal: StaticString](
    output: Pointer[mut=True, UInt8, _], length: Int64
) -> Bool:
    return tooling_output_range_equals[literal](output, 0, length)


def tooling_alias_mask(
    output: Pointer[mut=True, UInt8, _], length: Int64
) -> Int64:
    if (
        tooling_output_equals["exec_command"](output, length)
        or tooling_output_equals["run_shell_command"](output, length)
        or tooling_output_equals["shell"](output, length)
        or tooling_output_equals["bash"](output, length)
    ):
        return (
            TOOL_ALIAS_EXEC_COMMAND
            | TOOL_ALIAS_RUN_SHELL_COMMAND
            | TOOL_ALIAS_SHELL
            | TOOL_ALIAS_BASH
        )
    if (
        tooling_output_equals["apply_patch"](output, length)
        or tooling_output_equals["edit"](output, length)
        or tooling_output_equals["replace"](output, length)
    ):
        return TOOL_ALIAS_APPLY_PATCH | TOOL_ALIAS_EDIT | TOOL_ALIAS_REPLACE
    if (
        tooling_output_equals["read_file"](output, length)
        or tooling_output_equals["read"](output, length)
    ):
        return TOOL_ALIAS_READ_FILE | TOOL_ALIAS_READ
    if (
        tooling_output_equals["read_many_files"](output, length)
        or tooling_output_equals["glob"](output, length)
    ):
        return TOOL_ALIAS_READ_MANY_FILES | TOOL_ALIAS_GLOB
    if (
        tooling_output_equals["grep"](output, length)
        or tooling_output_equals["rip_grep"](output, length)
        or tooling_output_equals["rg"](output, length)
        or tooling_output_equals["search"](output, length)
    ):
        return TOOL_ALIAS_GREP | TOOL_ALIAS_RIP_GREP | TOOL_ALIAS_RG | TOOL_ALIAS_SEARCH
    if (
        tooling_output_equals["write_file"](output, length)
        or tooling_output_equals["write"](output, length)
    ):
        return TOOL_ALIAS_WRITE_FILE | TOOL_ALIAS_WRITE
    return 0


def tooling_range_is_mutating(
    output: Pointer[mut=True, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return (
        tooling_output_range_equals["apply_patch"](output, start, end)
        or tooling_output_range_equals["edit"](output, start, end)
        or tooling_output_range_equals["replace"](output, start, end)
        or tooling_output_range_equals["write"](output, start, end)
        or tooling_output_range_equals["write_file"](output, start, end)
        or tooling_output_range_equals["exec_command"](output, start, end)
        or tooling_output_range_equals["run_shell_command"](output, start, end)
        or tooling_output_range_equals["shell"](output, start, end)
        or tooling_output_range_equals["bash"](output, start, end)
    )


def tooling_view_range_equals_folded[literal: StaticString](
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    if start < 0 or end < start or end - start != Int64(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(end - start):
        if tooling_ascii_lower(source[unsafe_offset=start + index]) != wanted[unsafe_offset=index]:
            return False
    return True


def tooling_view_contains_folded[literal: StaticString](
    view: ProdexRichStringView
) -> Bool:
    var literal_length = Int64(literal.byte_length())
    if literal_length == 0:
        return True
    if Int64(view.len) < literal_length:
        return False
    var end = Int64(view.len) - literal_length
    for start in range(end + 1):
        if tooling_view_range_equals_folded[literal](
            view, start, start + literal_length
        ):
            return True
    return False


@export("prodex_gemini_tool_name_policy_v1")
def prodex_gemini_tool_name_policy_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    suffix_start_address: UInt,
    alias_mask_address: UInt,
    flags_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_TOOLING_ABI_VERSION:
        return GEMINI_TOOLING_ABI
    if (
        input_length < 0
        or output_capacity < 0
        or written_address == 0
        or suffix_start_address == 0
        or alias_mask_address == 0
        or flags_address == 0
        or (input_length > 0 and input_address == 0)
        or (output_capacity > 0 and output_address == 0)
    ):
        return GEMINI_TOOLING_INVALID

    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, input_length):
        return GEMINI_TOOLING_INVALID

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var suffix_start = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(suffix_start_address)
    )
    var alias_mask = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(alias_mask_address)
    )
    var flags = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(flags_address)
    )
    written[] = 0
    suffix_start[] = 0
    alias_mask[] = 0
    flags[] = 0

    var bounds = rich_trim_bounds(view)
    var source = rich_view_ptr(view)
    var start = bounds[0]
    var cursor = bounds[1]
    while cursor > bounds[0]:
        cursor -= 1
        if source[unsafe_offset=cursor] == 46:
            start = cursor + 1
            break

    var required = bounds[1] - start
    if required > output_capacity:
        return GEMINI_TOOLING_CAPACITY
    for index in range(start, bounds[1]):
        var value = tooling_ascii_lower(source[unsafe_offset=index])
        if value == 45:
            value = 95
        output[unsafe_offset=written[]] = value
        written[] += 1

    if written[] >= 2:
        for index in range(written[] - 1):
            if output[unsafe_offset=index] == 95 and output[unsafe_offset=index + 1] == 95:
                suffix_start[] = index + 2

    alias_mask[] = tooling_alias_mask(output, written[])
    var mutating = tooling_range_is_mutating(output, 0, written[])
    if suffix_start[] > 0:
        mutating = mutating or tooling_range_is_mutating(
            output, suffix_start[], written[]
        )
    if mutating:
        flags[] |= TOOL_FLAG_MUTATING
    if tooling_output_equals["run_shell_command"](output, written[]):
        flags[] |= TOOL_FLAG_CANONICAL_EXEC
    return GEMINI_TOOLING_OK


@export("prodex_gemini_tooling_model_policy_v1")
def prodex_gemini_tooling_model_policy_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
) abi("C") -> Int64:
    if abi_version != GEMINI_TOOLING_ABI_VERSION:
        return -2
    if input_length < 0 or (input_length > 0 and input_address == 0):
        return -2
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, input_length):
        return -2
    if tooling_view_contains_folded["gemini-3"](view):
        return 1
    if tooling_view_range_equals_folded["auto"](view, 0, input_length):
        return 1
    if tooling_view_contains_folded["auto-gemini-3"](view):
        return 1
    return 0
