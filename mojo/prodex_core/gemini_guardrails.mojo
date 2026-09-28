from std.memory import Pointer

from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_trim_bounds,
    rich_unicode_space,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView


comptime GEMINI_GUARDRAIL_ABI_VERSION: Int64 = 1
comptime GEMINI_GUARDRAIL_MAX_BYTES: Int64 = 9_223_372_036_854_775_807

comptime GEMINI_GUARDRAIL_WAIT_REASON: Int64 = 1
comptime GEMINI_GUARDRAIL_TOOL_INTENT: Int64 = 2
comptime GEMINI_GUARDRAIL_SUCCESS_CLAIM: Int64 = 3
comptime GEMINI_GUARDRAIL_TOOL_FAILURE: Int64 = 4
comptime GEMINI_GUARDRAIL_VERSION_LINES: Int64 = 5
comptime GEMINI_GUARDRAIL_VERIFICATION_MARKER: Int64 = 6
comptime GEMINI_GUARDRAIL_PROCESS_EXITED_ZERO: Int64 = 7
comptime GEMINI_GUARDRAIL_COMMAND_OUTPUT_ONLY: Int64 = 8


def guard_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def guard_range_contains(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var needle = Int64(literal.byte_length())
    if needle == 0:
        return True
    if start < 0 or end < start or end - start < needle:
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for offset in range(end - start - needle + 1):
        var matched = True
        for index in range(needle):
            if (
                guard_ascii_lower(source[unsafe_offset=start + offset + index])
                != wanted[unsafe_offset=index]
            ):
                matched = False
                break
        if matched:
            return True
    return False


def guard_range_starts_with(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end < start or end - start < length:
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(length):
        if (
            guard_ascii_lower(source[unsafe_offset=start + index])
            != wanted[unsafe_offset=index]
        ):
            return False
    return True


def guard_range_has_ascii_digit(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start:
        return False
    var source = rich_view_ptr(view)
    for index in range(start, end):
        var value = source[unsafe_offset=index]
        if value >= 48 and value <= 57:
            return True
    return False


def guard_token_boundary_before(
    view: ProdexRichStringView,
    start: Int64,
) -> Bool:
    if start <= 0:
        return True
    var value = rich_view_ptr(view)[unsafe_offset=start - 1]
    return not (
        value >= 48 and value <= 57
        or value >= 65 and value <= 90
        or value >= 97 and value <= 122
        or value == 95
    )


def guard_token_boundary_after(
    view: ProdexRichStringView,
    end: Int64,
    limit: Int64,
) -> Bool:
    if end >= limit:
        return True
    var value = rich_view_ptr(view)[unsafe_offset=end]
    return not (
        value >= 48 and value <= 57
        or value >= 65 and value <= 90
        or value >= 97 and value <= 122
        or value == 95
    )


def guard_range_contains_tool_token(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var needle = Int64(literal.byte_length())
    if needle == 0 or start < 0 or end < start or end - start < needle:
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for offset in range(end - start - needle + 1):
        var matched = True
        for index in range(needle):
            if (
                guard_ascii_lower(source[unsafe_offset=start + offset + index])
                != wanted[unsafe_offset=index]
            ):
                matched = False
                break
        if matched:
            var token_start = start + offset
            var token_end = token_start + needle
            if (
                guard_token_boundary_before(view, token_start)
                and guard_token_boundary_after(view, token_end, end)
            ):
                return True
    return False


def guard_wait_reason(view: ProdexRichStringView) -> Int64:
    var bounds = rich_trim_bounds(view)
    if bounds[1] - bounds[0] < 8:
        return -1
    var index: Int64 = 0
    for phrase in [
        StringSlice("i will poll"),
        StringSlice("i'll poll"),
        StringSlice("i need to wait"),
        StringSlice("let's wait"),
        StringSlice("still running"),
        StringSlice("is still running"),
        StringSlice("i will wait"),
        StringSlice("i'll wait"),
    ]:
        if guard_range_contains(view, bounds[0], bounds[1], phrase):
            return index
        index += 1
    return -1


def guard_future_tool_intent(view: ProdexRichStringView, start: Int64, end: Int64) -> Bool:
    for phrase in [
        StringSlice("i'll use"),
        StringSlice("i will use"),
        StringSlice("i'll call"),
        StringSlice("i will call"),
        StringSlice("i'll run"),
        StringSlice("i will run"),
        StringSlice("i'll search"),
        StringSlice("i will search"),
        StringSlice("i'll inspect"),
        StringSlice("i will inspect"),
        StringSlice("i'll read"),
        StringSlice("i will read"),
        StringSlice("i'm going to use"),
        StringSlice("i am going to use"),
        StringSlice("now, i'll use"),
        StringSlice("next, i'll use"),
    ]:
        if guard_range_contains(view, start, end, phrase):
            return True
    return False


def guard_tool_intent(view: ProdexRichStringView) -> Int64:
    var bounds = rich_trim_bounds(view)
    if bounds[1] - bounds[0] < 16:
        return -1
    if not guard_future_tool_intent(view, bounds[0], bounds[1]):
        return -1
    var index: Int64 = 0
    for tool in [
        StringSlice("exec_command"),
        StringSlice("write_stdin"),
        StringSlice("apply_patch"),
        StringSlice("sqz_grep"),
        StringSlice("sqz_read_file"),
        StringSlice("sqz_list_dir"),
        StringSlice("read_mcp_resource"),
        StringSlice("list_mcp_resources"),
        StringSlice("tool_search"),
        StringSlice("rg"),
        StringSlice("grep"),
    ]:
        if guard_range_contains_tool_token(view, bounds[0], bounds[1], tool):
            return index
        index += 1
    return -1


def guard_success_claim(view: ProdexRichStringView) -> Bool:
    for phrase in [
        StringSlice("blocker/unresolved: none"),
        StringSlice("blockers/unresolved: none"),
        StringSlice("unresolved: none"),
        StringSlice("everything is complete"),
        StringSlice("semua optional tools berhasil"),
        StringSlice("berhasil diupdate"),
        StringSlice("successfully updated all"),
        StringSlice("all optional tools"),
        StringSlice("up-to-date"),
        StringSlice("latest version"),
        StringSlice("latest versions"),
    ]:
        if guard_range_contains(view, 0, Int64(view.len), phrase):
            return True
    return False


def guard_tool_failure(view: ProdexRichStringView) -> Bool:
    for phrase in [
        StringSlice("process exited with code 1"),
        StringSlice("process exited with code 2"),
        StringSlice("process exited with code 127"),
        StringSlice("no such file or directory"),
        StringSlice("command not found"),
        StringSlice("error:"),
        StringSlice("failed"),
        StringSlice("not found"),
        StringSlice("virtual manifest"),
    ]:
        if guard_range_contains(view, 0, Int64(view.len), phrase):
            return True
    return False


def guard_version_lines(view: ProdexRichStringView) -> Bool:
    var length = Int64(view.len)
    var line_start: Int64 = 0
    var source = rich_view_ptr(view)
    while line_start <= length:
        var line_end = line_start
        while line_end < length and source[unsafe_offset=line_end] != 10:
            line_end += 1
        var line = ProdexRichStringView(
            view.ptr + UInt(line_start), UInt(line_end - line_start)
        )
        var bounds = rich_trim_bounds(line)
        var starts_with_tool = False
        for prefix in [
            StringSlice("rtk "),
            StringSlice("sqz "),
            StringSlice("sqz-mcp "),
            StringSlice("token-savior "),
            StringSlice("claw-compactor "),
            StringSlice("prodex "),
            StringSlice("codex "),
        ]:
            if guard_range_starts_with(line, bounds[0], bounds[1], prefix):
                starts_with_tool = True
                break
        if starts_with_tool and guard_range_has_ascii_digit(line, bounds[0], bounds[1]):
            return True
        if line_end >= length:
            break
        line_start = line_end + 1
    return False


def guard_verification_marker(view: ProdexRichStringView) -> Bool:
    for phrase in [
        StringSlice("--version"),
        StringSlice("version:"),
        StringSlice("verification:"),
        StringSlice("already up to date"),
        StringSlice("up-to-date"),
    ]:
        if guard_range_contains(view, 0, Int64(view.len), phrase):
            return True
    return guard_version_lines(view)


def guard_command_output_only(view: ProdexRichStringView) -> Bool:
    return (
        guard_range_contains(
            view, 0, Int64(view.len), StringSlice("only the command output")
        )
        or guard_range_contains(
            view, 0, Int64(view.len), StringSlice("command output only")
        )
        or guard_range_contains(
            view, 0, Int64(view.len), StringSlice("only with the command output")
        )
    )



def guard_range_find_folded(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Int64:
    var needle = Int64(literal.byte_length())
    if needle == 0:
        return start
    if start < 0 or end < start or end - start < needle:
        return -1
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for offset in range(end - start - needle + 1):
        var matched = True
        for index in range(needle):
            if (
                guard_ascii_lower(source[unsafe_offset=start + offset + index])
                != wanted[unsafe_offset=index]
            ):
                matched = False
                break
        if matched:
            return start + offset
    return -1


def guard_range_find_exact(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Int64:
    var needle = Int64(literal.byte_length())
    if needle == 0:
        return start
    if start < 0 or end < start or end - start < needle:
        return -1
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for offset in range(end - start - needle + 1):
        var matched = True
        for index in range(needle):
            if source[unsafe_offset=start + offset + index] != wanted[unsafe_offset=index]:
                matched = False
                break
        if matched:
            return start + offset
    return -1


def guard_range_rfind_exact(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Int64:
    var needle = Int64(literal.byte_length())
    if needle == 0:
        return end
    if start < 0 or end < start or end - start < needle:
        return -1
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    var offset = end - start - needle
    while offset >= 0:
        var matched = True
        for index in range(needle):
            if source[unsafe_offset=start + offset + index] != wanted[unsafe_offset=index]:
                matched = False
                break
        if matched:
            return start + offset
        offset -= 1
    return -1


def guard_range_starts_with_exact(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
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


def guard_trim_absolute(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Tuple[Int64, Int64]:
    if start < 0 or end < start:
        return (Int64(-1), Int64(0))
    var subview = ProdexRichStringView(
        view.ptr + UInt(start), UInt(end - start)
    )
    var bounds = rich_trim_bounds(subview)
    return (start + bounds[0], start + bounds[1])


def guard_line_after_folded_marker(
    view: ProdexRichStringView,
    marker: StringSlice,
) -> Tuple[Int64, Int64]:
    var length = Int64(view.len)
    var marker_index = guard_range_find_folded(view, 0, length, marker)
    if marker_index < 0:
        return (Int64(-1), Int64(0))
    var cursor = marker_index + Int64(marker.byte_length())
    var source = rich_view_ptr(view)
    while cursor <= length:
        var line_end = cursor
        while line_end < length and source[unsafe_offset=line_end] != 10:
            line_end += 1
        var bounds = guard_trim_absolute(view, cursor, line_end)
        if bounds[1] > bounds[0]:
            return bounds
        if line_end >= length:
            break
        cursor = line_end + 1
    return (Int64(-1), Int64(0))


def guard_inline_after_folded_marker(
    view: ProdexRichStringView,
    marker: StringSlice,
) -> Tuple[Int64, Int64]:
    var length = Int64(view.len)
    var marker_index = guard_range_find_folded(view, 0, length, marker)
    if marker_index < 0:
        return (Int64(-1), Int64(0))
    var cursor = marker_index + Int64(marker.byte_length())
    var source = rich_view_ptr(view)
    while cursor <= length:
        var segment_end = cursor
        while (
            segment_end < length
            and source[unsafe_offset=segment_end] != 46
            and source[unsafe_offset=segment_end] != 10
        ):
            segment_end += 1
        var bounds = guard_trim_absolute(view, cursor, segment_end)
        if bounds[1] > bounds[0]:
            return bounds
        if segment_end >= length:
            break
        cursor = segment_end + 1
    return (Int64(-1), Int64(0))


def guard_required_exact_output_command(
    view: ProdexRichStringView,
) -> Tuple[Int64, Int64]:
    for marker in [
        StringSlice("run exactly:"),
        StringSlice("verification command from the workspace:"),
        StringSlice("verification command:"),
    ]:
        var bounds = guard_line_after_folded_marker(view, marker)
        if bounds[0] >= 0:
            return bounds
    return guard_inline_after_folded_marker(view, StringSlice("then run "))


def guard_extract_command_output(
    view: ProdexRichStringView,
) -> Tuple[Int64, Int64]:
    var length = Int64(view.len)
    var marker = StringSlice("Output:\n")
    var output_start = guard_range_rfind_exact(view, 0, length, marker)
    if output_start >= 0:
        output_start += Int64(marker.byte_length())
    else:
        output_start = 0
    var bounds = guard_trim_absolute(view, output_start, length)
    if bounds[1] <= bounds[0]:
        return (Int64(-1), Int64(0))

    for delimiter in [
        StringSlice("\n\ndiff --git "),
        StringSlice("\ndiff --git "),
    ]:
        var diff_index = guard_range_find_exact(view, bounds[0], bounds[1], delimiter)
        if diff_index >= 0:
            bounds = guard_trim_absolute(view, bounds[0], diff_index)
            break

    if bounds[1] <= bounds[0]:
        return (Int64(-1), Int64(0))
    if (
        guard_range_starts_with_exact(
            view,
            bounds[0],
            bounds[1],
            StringSlice("Success. Updated the following files:"),
        )
        or guard_range_starts_with_exact(
            view,
            bounds[0],
            bounds[1],
            StringSlice("Success. No files changed."),
        )
    ):
        return (Int64(-1), Int64(0))
    return bounds


def guard_ascii_token_byte(value: UInt8) -> Bool:
    return (
        value >= 48 and value <= 57
        or value >= 65 and value <= 90
        or value >= 97 and value <= 122
        or value == 95
    )


def guard_exact_marker_match(
    required: ProdexRichStringView,
    text: ProdexRichStringView,
) -> Bool:
    var required_ptr = rich_view_ptr(required)
    var required_len = Int64(required.len)
    var cursor: Int64 = 0
    while cursor < required_len:
        while cursor < required_len and not guard_ascii_token_byte(
            required_ptr[unsafe_offset=cursor]
        ):
            cursor += 1
        var token_start = cursor
        while cursor < required_len and guard_ascii_token_byte(
            required_ptr[unsafe_offset=cursor]
        ):
            cursor += 1
        var token_end = cursor
        if token_end - token_start >= 16 and token_end - token_start >= 7:
            var prefix = StringSlice("PRODEX_")
            var prefix_ptr = prefix.unsafe_ptr()
            var matches_prefix = True
            for index in range(7):
                if required_ptr[unsafe_offset=token_start + Int64(index)] != prefix_ptr[unsafe_offset=index]:
                    matches_prefix = False
                    break
            if matches_prefix:
                var text_ptr = rich_view_ptr(text)
                var text_len = Int64(text.len)
                var token_len = token_end - token_start
                if token_len <= text_len:
                    for offset in range(text_len - token_len + 1):
                        var matched = True
                        for index in range(token_len):
                            if (
                                text_ptr[unsafe_offset=offset + index]
                                != required_ptr[unsafe_offset=token_start + index]
                            ):
                                matched = False
                                break
                        if matched:
                            return True
    return False


def guard_normalize_command(
    view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
) -> Tuple[Int64, Int64]:
    if capacity < Int64(view.len):
        return (Int64(-1), Int64(-1))
    var source = rich_view_ptr(view)
    var index: Int64 = 0
    var written: Int64 = 0
    var in_token = False
    var wrote_token = False
    while index < Int64(view.len):
        var width = rich_codepoint_width(source[unsafe_offset=index])
        var codepoint = rich_codepoint(source, index, width)
        if rich_unicode_space(codepoint):
            in_token = False
        else:
            if not in_token and wrote_token:
                output[unsafe_offset=written] = 32
                written += 1
            for byte_index in range(width):
                output[unsafe_offset=written] = source[unsafe_offset=index + byte_index]
                written += 1
            in_token = True
            wrote_token = True
        index += width

    var start: Int64 = 0
    var end = written
    while start < end and (
        output[unsafe_offset=start] == 34 or output[unsafe_offset=start] == 39
    ):
        start += 1
    while end > start and (
        output[unsafe_offset=end - 1] == 34 or output[unsafe_offset=end - 1] == 39
    ):
        end -= 1
    return (start, end)


def guard_buffer_equal(
    left: Pointer[mut=True, UInt8, _],
    left_start: Int64,
    left_end: Int64,
    right: Pointer[mut=True, UInt8, _],
    right_start: Int64,
    right_end: Int64,
) -> Bool:
    if left_end - left_start != right_end - right_start:
        return False
    for index in range(left_end - left_start):
        if left[unsafe_offset=left_start + index] != right[unsafe_offset=right_start + index]:
            return False
    return True


def guard_buffer_contains(
    haystack: Pointer[mut=True, UInt8, _],
    haystack_start: Int64,
    haystack_end: Int64,
    needle: Pointer[mut=True, UInt8, _],
    needle_start: Int64,
    needle_end: Int64,
) -> Bool:
    var needle_len = needle_end - needle_start
    if needle_len == 0:
        return True
    if haystack_end - haystack_start < needle_len:
        return False
    for offset in range(haystack_end - haystack_start - needle_len + 1):
        var matched = True
        for index in range(needle_len):
            if (
                haystack[unsafe_offset=haystack_start + offset + index]
                != needle[unsafe_offset=needle_start + index]
            ):
                matched = False
                break
        if matched:
            return True
    return False


def guard_buffer_marker_match(
    required: Pointer[mut=True, UInt8, _],
    required_start: Int64,
    required_end: Int64,
    command: Pointer[mut=True, UInt8, _],
    command_start: Int64,
    command_end: Int64,
) -> Bool:
    var cursor = required_start
    while cursor < required_end:
        while cursor < required_end and not guard_ascii_token_byte(
            required[unsafe_offset=cursor]
        ):
            cursor += 1
        var token_start = cursor
        while cursor < required_end and guard_ascii_token_byte(
            required[unsafe_offset=cursor]
        ):
            cursor += 1
        var token_end = cursor
        if token_end - token_start >= 16 and token_end - token_start >= 7:
            var prefix = StringSlice("PRODEX_")
            var prefix_ptr = prefix.unsafe_ptr()
            var matches_prefix = True
            for index in range(7):
                if required[unsafe_offset=token_start + Int64(index)] != prefix_ptr[unsafe_offset=index]:
                    matches_prefix = False
                    break
            if matches_prefix and guard_buffer_contains(
                command,
                command_start,
                command_end,
                required,
                token_start,
                token_end,
            ):
                return True
    return False


@export("prodex_mojo_gemini_guardrail_slice_v1")
def prodex_mojo_gemini_guardrail_slice_v1(
    abi_version: Int64,
    operation: Int64,
    input_address: UInt,
    input_length: Int64,
    output_start_address: UInt,
    output_length_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_GUARDRAIL_ABI_VERSION:
        return -2
    if (
        operation < 1
        or operation > 2
        or input_length < 0
        or (input_length > 0 and input_address == 0)
        or output_start_address == 0
        or output_length_address == 0
    ):
        return -2
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, GEMINI_GUARDRAIL_MAX_BYTES):
        return -2
    var bounds = (
        guard_required_exact_output_command(view)
        if operation == 1
        else guard_extract_command_output(view)
    )
    var output_start = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_start_address)
    )
    var output_length = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_length_address)
    )
    output_start[] = bounds[0]
    output_length[] = 0 if bounds[0] < 0 else bounds[1] - bounds[0]
    return 0


@export("prodex_mojo_gemini_guardrail_marker_match_v1")
def prodex_mojo_gemini_guardrail_marker_match_v1(
    abi_version: Int64,
    required_address: UInt,
    required_length: Int64,
    text_address: UInt,
    text_length: Int64,
) abi("C") -> Int64:
    if abi_version != GEMINI_GUARDRAIL_ABI_VERSION:
        return -2
    if (
        required_length < 0
        or text_length < 0
        or (required_length > 0 and required_address == 0)
        or (text_length > 0 and text_address == 0)
    ):
        return -2
    var required = ProdexRichStringView(required_address, UInt(required_length))
    var text = ProdexRichStringView(text_address, UInt(text_length))
    if (
        not rich_view_valid(required, GEMINI_GUARDRAIL_MAX_BYTES)
        or not rich_view_valid(text, GEMINI_GUARDRAIL_MAX_BYTES)
    ):
        return -2
    return Int64(guard_exact_marker_match(required, text))


@export("prodex_mojo_gemini_guardrail_command_empty_v1")
def prodex_mojo_gemini_guardrail_command_empty_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    scratch_address: UInt,
    scratch_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != GEMINI_GUARDRAIL_ABI_VERSION:
        return -2
    if (
        input_length < 0
        or scratch_capacity < input_length
        or (input_length > 0 and input_address == 0)
        or scratch_address == 0
    ):
        return -2
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, GEMINI_GUARDRAIL_MAX_BYTES):
        return -2
    var scratch = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(scratch_address)
    )
    var bounds = guard_normalize_command(view, scratch, scratch_capacity)
    if bounds[0] < 0:
        return -2
    return Int64(bounds[1] == bounds[0])


@export("prodex_mojo_gemini_guardrail_command_match_v1")
def prodex_mojo_gemini_guardrail_command_match_v1(
    abi_version: Int64,
    required_address: UInt,
    required_length: Int64,
    command_address: UInt,
    command_length: Int64,
    required_scratch_address: UInt,
    required_scratch_capacity: Int64,
    command_scratch_address: UInt,
    command_scratch_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != GEMINI_GUARDRAIL_ABI_VERSION:
        return -2
    if (
        required_length < 0
        or command_length < 0
        or required_scratch_capacity < required_length
        or command_scratch_capacity < command_length
        or (required_length > 0 and required_address == 0)
        or (command_length > 0 and command_address == 0)
        or required_scratch_address == 0
        or command_scratch_address == 0
    ):
        return -2
    var required = ProdexRichStringView(required_address, UInt(required_length))
    var command = ProdexRichStringView(command_address, UInt(command_length))
    if (
        not rich_view_valid(required, GEMINI_GUARDRAIL_MAX_BYTES)
        or not rich_view_valid(command, GEMINI_GUARDRAIL_MAX_BYTES)
    ):
        return -2

    var required_scratch = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(required_scratch_address)
    )
    var command_scratch = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(command_scratch_address)
    )
    var required_bounds = guard_normalize_command(
        required, required_scratch, required_scratch_capacity
    )
    var command_bounds = guard_normalize_command(
        command, command_scratch, command_scratch_capacity
    )
    if required_bounds[0] < 0 or command_bounds[0] < 0:
        return -2
    if required_bounds[1] == required_bounds[0]:
        return 1
    return Int64(
        guard_buffer_equal(
            command_scratch,
            command_bounds[0],
            command_bounds[1],
            required_scratch,
            required_bounds[0],
            required_bounds[1],
        )
        or guard_buffer_contains(
            command_scratch,
            command_bounds[0],
            command_bounds[1],
            required_scratch,
            required_bounds[0],
            required_bounds[1],
        )
        or guard_buffer_contains(
            required_scratch,
            required_bounds[0],
            required_bounds[1],
            command_scratch,
            command_bounds[0],
            command_bounds[1],
        )
        or guard_buffer_marker_match(
            required_scratch,
            required_bounds[0],
            required_bounds[1],
            command_scratch,
            command_bounds[0],
            command_bounds[1],
        )
    )


@export("prodex_mojo_gemini_guardrail_text_v1")
def prodex_mojo_gemini_guardrail_text_v1(
    abi_version: Int64,
    operation: Int64,
    input_address: UInt,
    input_length: Int64,
) abi("C") -> Int64:
    if abi_version != GEMINI_GUARDRAIL_ABI_VERSION:
        return -2
    if (
        operation < GEMINI_GUARDRAIL_WAIT_REASON
        or operation > GEMINI_GUARDRAIL_COMMAND_OUTPUT_ONLY
        or input_length < 0
        or (input_length > 0 and input_address == 0)
    ):
        return -2
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, GEMINI_GUARDRAIL_MAX_BYTES):
        return -2
    if operation == GEMINI_GUARDRAIL_WAIT_REASON:
        return guard_wait_reason(view)
    if operation == GEMINI_GUARDRAIL_TOOL_INTENT:
        return guard_tool_intent(view)
    if operation == GEMINI_GUARDRAIL_SUCCESS_CLAIM:
        return Int64(guard_success_claim(view))
    if operation == GEMINI_GUARDRAIL_TOOL_FAILURE:
        return Int64(guard_tool_failure(view))
    if operation == GEMINI_GUARDRAIL_VERSION_LINES:
        return Int64(guard_version_lines(view))
    if operation == GEMINI_GUARDRAIL_VERIFICATION_MARKER:
        return Int64(guard_verification_marker(view))
    if operation == GEMINI_GUARDRAIL_PROCESS_EXITED_ZERO:
        return Int64(
            guard_range_contains(
                view,
                0,
                Int64(view.len),
                StringSlice("process exited with code 0"),
            )
        )
    return Int64(guard_command_output_only(view))
