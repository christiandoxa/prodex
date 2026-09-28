from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
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
