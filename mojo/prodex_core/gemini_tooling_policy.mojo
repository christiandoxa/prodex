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
