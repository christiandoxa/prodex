from std.memory import Pointer

from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_unicode_space,
    rich_trim_bounds,
    rich_view_ptr,
    rich_view_valid,
    rich_view_matches_literal,
)
from rich_types import ProdexRichStringView
from rtk_noisy import (
    rtk_noisy_separator_length, rtk_noisy_skip_whitespace, rtk_noisy_token_end,
    rtk_noisy_raw_env_assignment, rtk_noisy_command_kind, rtk_noisy_subcommand,
)

comptime GEMINI_TOOLING_ABI_VERSION: Int64 = 1
comptime GEMINI_TOOLING_OK: Int64 = 0
comptime GEMINI_TOOLING_INVALID: Int64 = 1
comptime GEMINI_TOOLING_CAPACITY: Int64 = 3
comptime GEMINI_TOOLING_ABI: Int64 = 4

# Index contract with the Gemini request bridge:
# 0 computerUse, 1 codeExecution, 2 urlContext, 3 googleSearch.
# Error classification remains with Mojo's provider-error kernel. This
# kernel exclusively chooses which rejected, not-yet-attempted builtin wins.
comptime GEMINI_UNSUPPORTED_TOOL_RETRY_MASK: Int64 = 0xF


@export("prodex_gemini_unsupported_tool_retry_choice_v1")
def prodex_gemini_unsupported_tool_retry_choice_v1(
    abi_version: Int64,
    rejected_mask: Int64,
    attempted_mask: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_TOOLING_ABI_VERSION:
        return GEMINI_TOOLING_ABI
    if (
        result_address == 0 or rejected_mask < 0 or attempted_mask < 0
        or rejected_mask > GEMINI_UNSUPPORTED_TOOL_RETRY_MASK
        or attempted_mask > GEMINI_UNSUPPORTED_TOOL_RETRY_MASK
    ):
        return GEMINI_TOOLING_INVALID
    var selected: Int64 = -1
    for index in range(4):
        var bit = Int64(1) << Int64(index)
        if (rejected_mask & bit) != 0 and (attempted_mask & bit) == 0:
            selected = Int64(index)
            break
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )[] = selected
    return GEMINI_TOOLING_OK


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


@fieldwise_init
struct ToolingNamePlan(Copyable):
    var status: Int64
    var written: Int64
    var suffix_start: Int64
    var alias_mask: Int64
    var flags: Int64


def tooling_name_policy(
    view: ProdexRichStringView, output: Pointer[mut=True, UInt8, _], capacity: Int64,
) -> ToolingNamePlan:
    var plan = ToolingNamePlan(GEMINI_TOOLING_OK, 0, 0, 0, 0)
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
    if required > capacity:
        plan.status = GEMINI_TOOLING_CAPACITY
        return plan^
    for index in range(start, bounds[1]):
        var value = tooling_ascii_lower(source[unsafe_offset=index])
        if value == 45:
            value = 95
        output[unsafe_offset=plan.written] = value
        plan.written += 1

    if plan.written >= 2:
        for index in range(plan.written - 1):
            if output[unsafe_offset=index] == 95 and output[unsafe_offset=index + 1] == 95:
                plan.suffix_start = index + 2

    plan.alias_mask = tooling_alias_mask(output, plan.written)
    var mutating = tooling_range_is_mutating(output, 0, plan.written)
    if plan.suffix_start > 0:
        mutating = mutating or tooling_range_is_mutating(
            output, plan.suffix_start, plan.written
        )
    if mutating:
        plan.flags |= TOOL_FLAG_MUTATING
    if tooling_output_equals["run_shell_command"](output, plan.written):
        plan.flags |= TOOL_FLAG_CANONICAL_EXEC
    return plan^


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
    var plan = tooling_name_policy(view, output, output_capacity)
    written[] = plan.written
    suffix_start[] = plan.suffix_start
    alias_mask[] = plan.alias_mask
    flags[] = plan.flags
    return plan.status


def tooling_model_uses_gemini3(view: ProdexRichStringView) -> Bool:
    return (
        tooling_view_contains_folded["gemini-3"](view)
        or tooling_view_range_equals_folded["auto"](view, 0, Int64(view.len))
        or tooling_view_contains_folded["auto-gemini-3"](view)
    )


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
    return Int64(tooling_model_uses_gemini3(view))


comptime GEMINI_PATCH_MAX_INPUT_BYTES: Int64 = 4_194_304
comptime GEMINI_PATCH_MAX_OUTPUT_BYTES: Int64 = 16_777_216
comptime GEMINI_PATCH_NO_MATCH: Int64 = 2


def tooling_patch_space(value: UInt8) -> Bool:
    return value == 9 or value == 10 or value == 11 or value == 12 or value == 13 or value == 32


def tooling_patch_line(
    view: ProdexRichStringView, index: Int64
) -> Array[Int64, 3]:
    var length = Int64(view.len)
    var result = Array[Int64, 3](fill=length)
    if index >= length:
        return result^
    var source = rich_view_ptr(view)
    var cursor = index
    while cursor < length:
        var value = source[unsafe_offset=cursor]
        if value == 10 or value == 13:
            break
        cursor += 1
    var next = cursor
    if next < length:
        if (
            source[unsafe_offset=next] == 13
            and next + 1 < length
            and source[unsafe_offset=next + 1] == 10
        ):
            next += 2
        else:
            next += 1
    result[0] = index
    result[1] = cursor
    result[2] = next
    return result^


def tooling_patch_trim_start(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    var cursor = start
    var source = rich_view_ptr(view)
    while cursor < end:
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        if not rich_unicode_space(rich_codepoint(source, cursor, width)):
            break
        cursor += width
    return cursor


def tooling_patch_range_starts[literal: StaticString](
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end < start or end - start < length:
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(length):
        if source[unsafe_offset=start + index] != wanted[unsafe_offset=index]:
            return False
    return True


def tooling_patch_range_equals[literal: StaticString](
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    return (
        end - start == Int64(literal.byte_length())
        and tooling_patch_range_starts[literal](view, start, end)
    )


def tooling_patch_ranges_equal(
    view: ProdexRichStringView,
    left_start: Int64,
    left_end: Int64,
    right_start: Int64,
    right_end: Int64,
) -> Bool:
    if left_end - left_start != right_end - right_start:
        return False
    var source = rich_view_ptr(view)
    for index in range(left_end - left_start):
        if (
            source[unsafe_offset=left_start + index]
            != source[unsafe_offset=right_start + index]
        ):
            return False
    return True


def tooling_patch_put_byte(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: UInt8,
) -> Bool:
    if written[] >= capacity:
        return False
    output[unsafe_offset=written[]] = value
    written[] += 1
    return True


def tooling_patch_put_literal[literal: StaticString](
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var source = literal.unsafe_ptr()
    for index in range(Int64(literal.byte_length())):
        if not tooling_patch_put_byte(
            output, capacity, written, source[unsafe_offset=index]
        ):
            return False
    return True


def tooling_patch_put_range(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var source = rich_view_ptr(view)
    for index in range(start, end):
        if not tooling_patch_put_byte(
            output, capacity, written, source[unsafe_offset=index]
        ):
            return False
    return True


def tooling_patch_newline(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    return tooling_patch_put_byte(output, capacity, written, 10)


def tooling_patch_put_output_line_literal[literal: StaticString](
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    return (
        tooling_patch_newline(output, capacity, written)
        and tooling_patch_put_literal[literal](output, capacity, written)
    )


def tooling_patch_put_output_line_range(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    return (
        tooling_patch_newline(output, capacity, written)
        and tooling_patch_put_range(
            output, capacity, written, view, start, end
        )
    )


def tooling_patch_put_output_line_prefixed_range(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    prefix: UInt8,
) -> Bool:
    return (
        tooling_patch_newline(output, capacity, written)
        and tooling_patch_put_byte(output, capacity, written, prefix)
        and tooling_patch_put_range(output, capacity, written, view, start, end)
    )


def tooling_patch_put_prefixed_lines(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    view: ProdexRichStringView,
    prefix: UInt8,
    emit_empty: Bool,
) -> Bool:
    var length = Int64(view.len)
    if length == 0:
        if emit_empty:
            return tooling_patch_put_output_line_prefixed_range(
                output, capacity, written, view, 0, 0, prefix
            )
        return True
    var cursor: Int64 = 0
    while cursor < length:
        var line = tooling_patch_line(view, cursor)
        if not tooling_patch_put_output_line_prefixed_range(
            output, capacity, written, view, line[0], line[1], prefix
        ):
            return False
        if line[2] <= cursor:
            break
        cursor = line[2]
    return True


def tooling_patch_is_header_pair(
    view: ProdexRichStringView, line: Array[Int64, 3]
) -> Bool:
    if not tooling_patch_range_starts["--- "](view, line[0], line[1]):
        return False
    var next = tooling_patch_line(view, line[2])
    return tooling_patch_range_starts["+++ "](view, next[0], next[1])


def tooling_patch_path(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Array[Int64, 3]:
    # Result: start, end, present.
    var result = Array[Int64, 3](fill=0)
    if end - start < 4:
        return result^
    var source = rich_view_ptr(view)
    var left = start + 4
    var right = end

    # Match Rust trim_matches('"') before final whitespace trim.
    if (
        right > left
        and source[unsafe_offset=left] == 34
        and source[unsafe_offset=right - 1] == 34
    ):
        left += 1
        right -= 1

    if tooling_patch_range_equals["/dev/null"](view, left, right):
        return result^

    if (
        right - left >= 2
        and (source[unsafe_offset=left] == 97 or source[unsafe_offset=left] == 98)
        and source[unsafe_offset=left + 1] == 47
    ):
        left += 2

    while left < right and tooling_patch_space(source[unsafe_offset=left]):
        left += 1
    while right > left and tooling_patch_space(source[unsafe_offset=right - 1]):
        right -= 1
    if right <= left:
        return result^
    result[0] = left
    result[1] = right
    result[2] = 1
    return result^


def tooling_patch_find_next_diff(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    var cursor = start
    while cursor < end:
        var line = tooling_patch_line(view, cursor)
        if tooling_patch_range_starts["diff --git "](view, line[0], line[1]):
            return line[0]
        if line[2] <= cursor:
            break
        cursor = line[2]
    return end


def tooling_patch_find_next_header_or_diff(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    var cursor = start
    while cursor < end:
        var line = tooling_patch_line(view, cursor)
        if (
            tooling_patch_range_starts["diff --git "](view, line[0], line[1])
            or tooling_patch_is_header_pair(view, line)
        ):
            return line[0]
        if line[2] <= cursor:
            break
        cursor = line[2]
    return end


def tooling_patch_hunk_header(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    var source = rich_view_ptr(view)
    var second: Int64 = -1
    var cursor = start + 2
    while cursor + 1 < end:
        if (
            source[unsafe_offset=cursor] == 64
            and source[unsafe_offset=cursor + 1] == 64
        ):
            second = cursor
            break
        cursor += 1
    if not tooling_patch_newline(output, capacity, written):
        return False
    if second < 0:
        return tooling_patch_put_literal["@@"](output, capacity, written)

    var context_start = second + 2
    var context_end = end
    while (
        context_start < context_end
        and tooling_patch_space(source[unsafe_offset=context_start])
    ):
        context_start += 1
    while (
        context_end > context_start
        and tooling_patch_space(source[unsafe_offset=context_end - 1])
    ):
        context_end -= 1
    if not tooling_patch_put_literal["@@"](output, capacity, written):
        return False
    if context_start >= context_end:
        return True
    return (
        tooling_patch_put_byte(output, capacity, written, 32)
        and tooling_patch_put_range(
            output, capacity, written, view, context_start, context_end
        )
    )


def tooling_patch_convert_section(
    view: ProdexRichStringView,
    section_start: Int64,
    section_end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    var cursor = section_start
    var old_header = Array[Int64, 3](fill=-1)
    var new_header = Array[Int64, 3](fill=-1)
    while cursor < section_end:
        var line = tooling_patch_line(view, cursor)
        if tooling_patch_range_starts["--- "](view, line[0], line[1]):
            var next = tooling_patch_line(view, line[2])
            if (
                next[0] < section_end
                and tooling_patch_range_starts["+++ "](
                    view, next[0], next[1]
                )
            ):
                old_header = line.copy()
                new_header = next.copy()
                break
        if line[2] <= cursor:
            break
        cursor = line[2]

    if old_header[0] < 0:
        return 0

    var old_path = tooling_patch_path(view, old_header[0], old_header[1])
    var new_path = tooling_patch_path(view, new_header[0], new_header[1])
    var body_start = new_header[2]
    var rollback = written[]

    if old_path[2] == 0 and new_path[2] == 1:
        if (
            not tooling_patch_put_output_line_literal["*** Add File: "](
                output, capacity, written
            )
            or not tooling_patch_put_range(
                output,
                capacity,
                written,
                view,
                new_path[0],
                new_path[1],
            )
        ):
            return -1
        var saw_added = False
        cursor = body_start
        while cursor < section_end:
            var line = tooling_patch_line(view, cursor)
            if (
                tooling_patch_range_starts["+"](view, line[0], line[1])
                and not tooling_patch_range_starts["+++"](
                    view, line[0], line[1]
                )
            ):
                if not tooling_patch_put_output_line_range(
                    output, capacity, written, view, line[0], line[1]
                ):
                    return -1
                saw_added = True
            if line[2] <= cursor:
                break
            cursor = line[2]
        if saw_added:
            return 1
        written[] = rollback
        return 0

    if old_path[2] == 1 and new_path[2] == 0:
        if (
            not tooling_patch_put_output_line_literal["*** Delete File: "](
                output, capacity, written
            )
            or not tooling_patch_put_range(
                output,
                capacity,
                written,
                view,
                old_path[0],
                old_path[1],
            )
        ):
            return -1
        return 1

    if old_path[2] == 0 or new_path[2] == 0:
        return 0

    if (
        not tooling_patch_put_output_line_literal["*** Update File: "](
            output, capacity, written
        )
        or not tooling_patch_put_range(
            output, capacity, written, view, old_path[0], old_path[1]
        )
    ):
        return -1
    if not tooling_patch_ranges_equal(
        view, old_path[0], old_path[1], new_path[0], new_path[1]
    ):
        if (
            not tooling_patch_put_output_line_literal["*** Move to: "](
                output, capacity, written
            )
            or not tooling_patch_put_range(
                output, capacity, written, view, new_path[0], new_path[1]
            )
        ):
            return -1

    var saw_change = False
    var saw_hunk = False
    cursor = body_start
    var source = rich_view_ptr(view)
    while cursor < section_end:
        var line = tooling_patch_line(view, cursor)
        if tooling_patch_range_starts["@@"](view, line[0], line[1]):
            if not tooling_patch_hunk_header(
                output, capacity, written, view, line[0], line[1]
            ):
                return -1
            saw_hunk = True
        elif (
            saw_hunk
            and line[1] > line[0]
            and (
                source[unsafe_offset=line[0]] == 43
                or source[unsafe_offset=line[0]] == 45
                or source[unsafe_offset=line[0]] == 32
            )
        ):
            if not tooling_patch_put_output_line_range(
                output, capacity, written, view, line[0], line[1]
            ):
                return -1
            if (
                source[unsafe_offset=line[0]] == 43
                or source[unsafe_offset=line[0]] == 45
            ):
                saw_change = True
        elif (
            saw_hunk
            and tooling_patch_range_equals["\\ No newline at end of file"](
                view, line[0], line[1]
            )
        ):
            if not tooling_patch_put_output_line_literal["*** End of File"](
                output, capacity, written
            ):
                return -1
        if line[2] <= cursor:
            break
        cursor = line[2]

    if saw_change:
        return 1
    written[] = rollback
    return 0


def tooling_patch_normalize_block(
    view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Int64:
    var begin: Int64 = -1
    var finish: Int64 = -1
    var cursor: Int64 = 0
    var length = Int64(view.len)
    while cursor < length:
        var line = tooling_patch_line(view, cursor)
        var trimmed = tooling_patch_trim_start(view, line[0], line[1])
        if begin < 0 and tooling_patch_range_starts["*** Begin Patch"](
            view, trimmed, line[1]
        ):
            begin = line[0]
        elif begin >= 0 and tooling_patch_range_starts["*** End Patch"](
            view, trimmed, line[1]
        ):
            finish = line[1]
            break
        if line[2] <= cursor:
            break
        cursor = line[2]

    if begin < 0 or finish < begin:
        return GEMINI_PATCH_NO_MATCH

    cursor = begin
    var first = True
    var in_add_file = False
    while cursor < length:
        var line = tooling_patch_line(view, cursor)
        if line[0] >= finish:
            break
        if not first and not tooling_patch_newline(output, capacity, written):
            return -1
        first = False
        var add_file = tooling_patch_range_starts["*** Add File: "](
            view, line[0], line[1]
        )
        if add_file:
            in_add_file = True
        elif tooling_patch_range_starts["*** "](view, line[0], line[1]):
            in_add_file = False
        if in_add_file and not add_file and not tooling_patch_range_starts["+"](
            view, line[0], line[1]
        ):
            if not tooling_patch_put_byte(output, capacity, written, 43):
                return -1
        if not tooling_patch_put_range(
            output, capacity, written, view, line[0], line[1]
        ):
            return -1
        if line[1] >= finish or line[2] <= cursor:
            break
        cursor = line[2]
    return 1


@export("prodex_gemini_unified_diff_to_apply_patch_v1")
def prodex_gemini_unified_diff_to_apply_patch_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_TOOLING_ABI_VERSION:
        return GEMINI_TOOLING_ABI
    if (
        input_length < 0
        or input_length > GEMINI_PATCH_MAX_INPUT_BYTES
        or output_capacity <= 0
        or output_capacity > GEMINI_PATCH_MAX_OUTPUT_BYTES
        or output_address == 0
        or written_address == 0
        or (input_length > 0 and input_address == 0)
    ):
        return GEMINI_TOOLING_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, GEMINI_PATCH_MAX_INPUT_BYTES):
        return GEMINI_TOOLING_INVALID

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not tooling_patch_put_literal["*** Begin Patch"](
        output, output_capacity, written
    ):
        return GEMINI_TOOLING_CAPACITY

    var converted: Int64 = 0
    var cursor: Int64 = 0
    while cursor < input_length:
        var line = tooling_patch_line(view, cursor)
        if tooling_patch_range_starts["diff --git "](
            view, line[0], line[1]
        ):
            var section_end = tooling_patch_find_next_diff(
                view, line[2], input_length
            )
            var result = tooling_patch_convert_section(
                view,
                line[0],
                section_end,
                output,
                output_capacity,
                written,
            )
            if result < 0:
                return GEMINI_TOOLING_CAPACITY
            converted += result
            cursor = section_end
            continue
        if tooling_patch_is_header_pair(view, line):
            var next = tooling_patch_line(view, line[2])
            var section_end = tooling_patch_find_next_header_or_diff(
                view, next[2], input_length
            )
            var result = tooling_patch_convert_section(
                view,
                line[0],
                section_end,
                output,
                output_capacity,
                written,
            )
            if result < 0:
                return GEMINI_TOOLING_CAPACITY
            converted += result
            cursor = section_end
            continue
        if line[2] <= cursor:
            break
        cursor = line[2]

    if converted == 0:
        written[] = 0
        return GEMINI_PATCH_NO_MATCH
    if not tooling_patch_put_output_line_literal["*** End Patch"](
        output, output_capacity, written
    ):
        return GEMINI_TOOLING_CAPACITY
    return GEMINI_TOOLING_OK


@export("prodex_gemini_apply_patch_block_v1")
def prodex_gemini_apply_patch_block_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_TOOLING_ABI_VERSION:
        return GEMINI_TOOLING_ABI
    if (
        input_length < 0
        or input_length > GEMINI_PATCH_MAX_INPUT_BYTES
        or output_capacity <= 0
        or output_capacity > GEMINI_PATCH_MAX_OUTPUT_BYTES
        or output_address == 0
        or written_address == 0
        or (input_length > 0 and input_address == 0)
    ):
        return GEMINI_TOOLING_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, GEMINI_PATCH_MAX_INPUT_BYTES):
        return GEMINI_TOOLING_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var result = tooling_patch_normalize_block(
        view, output, output_capacity, written
    )
    if result == -1:
        return GEMINI_TOOLING_CAPACITY
    if result == GEMINI_PATCH_NO_MATCH:
        written[] = 0
        return GEMINI_PATCH_NO_MATCH
    return GEMINI_TOOLING_OK


@export("prodex_gemini_edit_patch_v1")
def prodex_gemini_edit_patch_v1(
    abi_version: Int64,
    path_address: UInt,
    path_length: Int64,
    old_address: UInt,
    old_length: Int64,
    old_present: Int64,
    new_address: UInt,
    new_length: Int64,
    new_present: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_TOOLING_ABI_VERSION:
        return GEMINI_TOOLING_ABI
    if (
        path_length <= 0
        or path_length > GEMINI_PATCH_MAX_INPUT_BYTES
        or old_length < 0
        or old_length > GEMINI_PATCH_MAX_INPUT_BYTES
        or new_length < 0
        or new_length > GEMINI_PATCH_MAX_INPUT_BYTES
        or old_present < 0
        or old_present > 1
        or new_present < 0
        or new_present > 1
        or output_capacity <= 0
        or output_capacity > GEMINI_PATCH_MAX_OUTPUT_BYTES
        or path_address == 0
        or output_address == 0
        or written_address == 0
        or (old_length > 0 and old_address == 0)
        or (new_length > 0 and new_address == 0)
        or new_present == 0
    ):
        return GEMINI_PATCH_NO_MATCH

    var path = ProdexRichStringView(path_address, UInt(path_length))
    var old = ProdexRichStringView(old_address, UInt(old_length))
    var new = ProdexRichStringView(new_address, UInt(new_length))
    if (
        not rich_view_valid(path, GEMINI_PATCH_MAX_INPUT_BYTES)
        or not rich_view_valid(old, GEMINI_PATCH_MAX_INPUT_BYTES)
        or not rich_view_valid(new, GEMINI_PATCH_MAX_INPUT_BYTES)
    ):
        return GEMINI_TOOLING_INVALID

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not tooling_patch_put_literal["*** Begin Patch"](
        output, output_capacity, written
    ):
        return GEMINI_TOOLING_CAPACITY

    var add_file = old_present == 0 or old_length == 0
    if add_file:
        if not tooling_patch_put_output_line_literal["*** Add File: "](
            output, output_capacity, written
        ) or not tooling_patch_put_range(
            output, output_capacity, written, path, 0, path_length
        ):
            return GEMINI_TOOLING_CAPACITY
        if not tooling_patch_put_prefixed_lines(
            output, output_capacity, written, new, 43, new_length == 0
        ):
            return GEMINI_TOOLING_CAPACITY
    else:
        if not tooling_patch_put_output_line_literal["*** Update File: "](
            output, output_capacity, written
        ) or not tooling_patch_put_range(
            output, output_capacity, written, path, 0, path_length
        ) or not tooling_patch_put_output_line_literal["@@"](
            output, output_capacity, written
        ):
            return GEMINI_TOOLING_CAPACITY
        if not tooling_patch_put_prefixed_lines(
            output, output_capacity, written, old, 45, False
        ) or not tooling_patch_put_prefixed_lines(
            output, output_capacity, written, new, 43, False
        ):
            return GEMINI_TOOLING_CAPACITY

    if not tooling_patch_put_output_line_literal["*** End Patch"](
        output, output_capacity, written
    ):
        return GEMINI_TOOLING_CAPACITY
    return GEMINI_TOOLING_OK


# Gemini historically scans raw shell words; unlike the generic RTK parser
# it does not unquote executable names, wraps only the first eligible segment,
# excludes echo, and includes its automatic compactor helper. Preserve that
# boundary while sharing whitespace, command catalog and subcommand policy.
def gemini_rtk_range(view: ProdexRichStringView, start: Int64, end: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(view.ptr + UInt(start), UInt(end - start))


def gemini_rtk_trim_punctuation(byte: UInt8) -> Bool:
    return byte == 34 or byte == 39 or byte == 40 or byte == 41 or byte == 123 or byte == 125


def gemini_rtk_segment_index(view: ProdexRichStringView, start: Int64, end: Int64) -> Int64:
    var source = rich_view_ptr(view)
    var cursor = rtk_noisy_skip_whitespace(source, start, end)
    if cursor >= end:
        return -1
    var word_end = rtk_noisy_token_end(source, cursor, end)
    if rich_view_matches_literal["rtk"](gemini_rtk_range(view, cursor, word_end), False):
        return -1
    while rtk_noisy_raw_env_assignment(source, cursor, word_end):
        cursor = rtk_noisy_skip_whitespace(source, word_end, end)
        if cursor >= end:
            return -1
        word_end = rtk_noisy_token_end(source, cursor, end)
    var component = cursor
    for offset in range(cursor, word_end):
        if source[unsafe_offset=offset] == 47:
            component = offset + 1
    var name = gemini_rtk_range(view, component, word_end)
    if rich_view_matches_literal["prodex-claw-compactor-auto"](name, False):
        return cursor
    var kind = rtk_noisy_command_kind(rich_view_ptr(name), Int64(name.len))
    if kind < 0 or kind == 14:
        return -1
    if kind == 6 or kind == 10 or kind == 11 or kind == 12 or kind == 13:
        return cursor
    var command_start = cursor
    cursor = rtk_noisy_skip_whitespace(source, word_end, end)
    while cursor < end:
        word_end = rtk_noisy_token_end(source, cursor, end)
        var begin = cursor
        var finish = word_end
        while begin < finish and gemini_rtk_trim_punctuation(source[unsafe_offset=begin]):
            begin += 1
        while finish > begin and gemini_rtk_trim_punctuation(source[unsafe_offset=finish - 1]):
            finish -= 1
        var candidate = gemini_rtk_range(view, begin, finish)
        if rtk_noisy_subcommand(kind, rich_view_ptr(candidate), Int64(candidate.len)):
            return command_start
        if kind == 15 and (
            rich_view_matches_literal["compact"](candidate, False)
            or rich_view_matches_literal["summarize"](candidate, False)
        ):
            return command_start
        cursor = rtk_noisy_skip_whitespace(source, word_end, end)
    return -1


def gemini_rtk_command_index(view: ProdexRichStringView) -> Int64:
    var source = rich_view_ptr(view)
    var end = Int64(view.len)
    var start: Int64 = 0
    var cursor: Int64 = 0
    while cursor < end:
        var separator = rtk_noisy_separator_length(source, cursor, end)
        if separator > 0:
            var found = gemini_rtk_segment_index(view, start, cursor)
            if found >= 0:
                return found
            cursor += separator
            start = cursor
        else:
            cursor += rich_codepoint_width(source[unsafe_offset=cursor])
    return gemini_rtk_segment_index(view, start, end)


@export("prodex_gemini_rtk_shell_command_v1")
def prodex_gemini_rtk_shell_command_v1(
    abi_version: Int64, input_address: UInt64, input_length: Int64,
    output_address: UInt64, output_capacity: Int64, written_address: UInt64,
) abi("C") -> Int64:
    if abi_version != GEMINI_TOOLING_ABI_VERSION:
        return GEMINI_TOOLING_ABI
    if input_length < 0 or input_length > 0x7FFFFFFFFFFFFFFF - 4 or output_address == 0 or written_address == 0 or output_capacity < 0:
        return GEMINI_TOOLING_INVALID
    var view = ProdexRichStringView(UInt(input_address), UInt(input_length))
    if not rich_view_valid(view, 0x7FFFFFFFFFFFFFFF):
        return GEMINI_TOOLING_INVALID
    var at = gemini_rtk_command_index(view)
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    if at < 0:
        written[] = 0
        return GEMINI_TOOLING_OK
    if output_capacity < input_length + 4:
        return GEMINI_TOOLING_CAPACITY
    var source = rich_view_ptr(view)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    for index in range(at):
        output[unsafe_offset=index] = source[unsafe_offset=index]
    output[unsafe_offset=at] = 114
    output[unsafe_offset=at + 1] = 116
    output[unsafe_offset=at + 2] = 107
    output[unsafe_offset=at + 3] = 32
    for index in range(at, input_length):
        output[unsafe_offset=index + 4] = source[unsafe_offset=index]
    written[] = input_length + 4
    return GEMINI_TOOLING_OK


# Borrowed patch argument fields selected by compiled Mojo, not Rust.
# Slots: direct text, input/patch/diff/text, file_path/path/filename,
# old_string/old/search/find, new_string/new/replace/replacement/content.
# All present slots are UTF-8 strings; non-string Serde fields stay absent.
comptime GEMINI_PATCH_SOURCES_FIELD_COUNT: Int64 = 17


def tooling_patch_first_string(
    fields: Pointer[mut=False, UInt64, _], start: Int64, end: Int64
) -> Int64:
    for index in range(start, end):
        if fields[unsafe_offset=index * 3] == 1:
            return index
    return -1


@export("prodex_gemini_apply_patch_sources_v1")
def prodex_gemini_apply_patch_sources_v1(
    abi_version: Int64, fields_address: UInt, field_count: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_TOOLING_ABI_VERSION:
        return GEMINI_TOOLING_ABI
    if (
        field_count != GEMINI_PATCH_SOURCES_FIELD_COUNT
        or fields_address == 0 or fields_address % 8 != 0
        or result_address == 0 or result_address % 8 != 0
    ):
        return GEMINI_TOOLING_INVALID
    var fields = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    for index in range(GEMINI_PATCH_SOURCES_FIELD_COUNT):
        var offset = index * 3
        var present = fields[unsafe_offset=offset]
        var address = fields[unsafe_offset=offset + 1]
        var length = fields[unsafe_offset=offset + 2]
        if present > 1 or (present == 0 and (address != 0 or length != 0)):
            return GEMINI_TOOLING_INVALID
        if present == 1 and not rich_view_valid(
            ProdexRichStringView(UInt(address), UInt(length)),
            0x7FFFFFFFFFFFFFFF,
        ):
            return GEMINI_TOOLING_INVALID

    var explicit = tooling_patch_first_string(fields, 0, 1)
    if explicit < 0:
        explicit = tooling_patch_first_string(fields, 1, 5)
    var path = tooling_patch_first_string(fields, 5, 8)
    var path_start = Int64(0)
    var path_end = Int64(0)
    if path >= 0:
        var offset = path * 3
        var view = ProdexRichStringView(
            UInt(fields[unsafe_offset=offset + 1]),
            UInt(fields[unsafe_offset=offset + 2]),
        )
        var bounds = rich_trim_bounds(view)
        if bounds[0] < bounds[1]:
            path_start = bounds[0]
            path_end = bounds[1]
        else:
            # An earlier whitespace-only path blocks later aliases.
            path = -1
    var old_text = tooling_patch_first_string(fields, 8, 12)
    var new_text = tooling_patch_first_string(fields, 12, 17)
    var content = tooling_patch_first_string(fields, 16, 17)
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = explicit
    result[unsafe_offset=1] = path
    result[unsafe_offset=2] = path_start
    result[unsafe_offset=3] = path_end
    result[unsafe_offset=4] = old_text
    result[unsafe_offset=5] = new_text
    result[unsafe_offset=6] = content
    return GEMINI_TOOLING_OK
