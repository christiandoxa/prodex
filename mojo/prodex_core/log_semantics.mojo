from std.memory import Pointer

from rich_text import rich_codepoint, rich_codepoint_width, rich_trim_bounds, rich_view_matches_literal, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime PRODEX_LOG_LEVEL_ABI_VERSION: Int64 = 1
comptime PRODEX_LOG_LEVEL_MAX_BYTES: Int64 = 4096


def log_view_part_matches(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, part: StringSlice
) -> Bool:
    var right = part.unsafe_ptr()
    for index in range(Int64(part.byte_length())):
        if ptr[unsafe_offset=start + index] != right[unsafe_offset=index]:
            return False
    return True


def log_view_starts_sequence2(
    view: ProdexRichStringView, first: StringSlice, second: StringSlice
) -> Bool:
    var total = Int64(first.byte_length()) + Int64(second.byte_length())
    if total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    return log_view_part_matches(ptr, 0, first) and log_view_part_matches(
        ptr, Int64(first.byte_length()), second
    )


def log_view_starts_sequence3(
    view: ProdexRichStringView,
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
) -> Bool:
    var first_length = Int64(first.byte_length())
    var second_length = Int64(second.byte_length())
    var total = first_length + second_length + Int64(third.byte_length())
    if total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    return log_view_part_matches(ptr, 0, first) and log_view_part_matches(
        ptr, first_length, second
    ) and log_view_part_matches(ptr, first_length + second_length, third)


def log_view_contains_sequence3(
    view: ProdexRichStringView,
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
) -> Bool:
    var first_length = Int64(first.byte_length())
    var second_length = Int64(second.byte_length())
    var third_length = Int64(third.byte_length())
    var total = first_length + second_length + third_length
    if total <= 0 or total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    for start in range(Int64(view.len) - total + 1):
        if log_view_part_matches(ptr, start, first) and log_view_part_matches(
            ptr, start + first_length, second
        ) and log_view_part_matches(
            ptr, start + first_length + second_length, third
        ):
            return True
    return False


def log_view_contains_sequence5(
    view: ProdexRichStringView,
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
    fourth: StringSlice,
    fifth: StringSlice,
) -> Bool:
    var first_length = Int64(first.byte_length())
    var second_length = Int64(second.byte_length())
    var third_length = Int64(third.byte_length())
    var fourth_length = Int64(fourth.byte_length())
    var total = (
        first_length
        + second_length
        + third_length
        + fourth_length
        + Int64(fifth.byte_length())
    )
    if total <= 0 or total > Int64(view.len) or view.ptr == 0:
        return False
    var ptr = rich_view_ptr(view)
    for start in range(Int64(view.len) - total + 1):
        if not log_view_part_matches(ptr, start, first):
            continue
        var second_start = start + first_length
        if not log_view_part_matches(ptr, second_start, second):
            continue
        var third_start = second_start + second_length
        if not log_view_part_matches(ptr, third_start, third):
            continue
        var fourth_start = third_start + third_length
        if not log_view_part_matches(ptr, fourth_start, fourth):
            continue
        if log_view_part_matches(ptr, fourth_start + fourth_length, fifth):
            return True
    return False


def log_view_contains_json_level(
    view: ProdexRichStringView, field: StringSlice, level: StringSlice
) -> Bool:
    return log_view_contains_sequence5(
        view,
        StringSlice("\""),
        field,
        StringSlice("\":\""),
        level,
        StringSlice("\""),
    ) or log_view_contains_sequence5(
        view,
        StringSlice("\""),
        field,
        StringSlice("\": \""),
        level,
        StringSlice("\""),
    )


def log_view_contains_kv_level(
    view: ProdexRichStringView, field: StringSlice, level: StringSlice
) -> Bool:
    return log_view_contains_sequence3(view, field, StringSlice("="), level) or log_view_contains_sequence3(
        view, field, StringSlice(": "), level
    )


def log_view_level_matches(view: ProdexRichStringView, level_id: Int64) -> Bool:
    if level_id < 1 or level_id > 7:
        return False
    var level = StringSlice("fatal")
    if level_id == 2:
        level = StringSlice("error")
    elif level_id == 3:
        level = StringSlice("warn")
    elif level_id == 4:
        level = StringSlice("warning")
    elif level_id == 5:
        level = StringSlice("info")
    elif level_id == 6:
        level = StringSlice("debug")
    elif level_id == 7:
        level = StringSlice("trace")

    for field in [StringSlice("level"), StringSlice("severity"), StringSlice("status")]:
        if log_view_contains_json_level(view, field, level) or log_view_contains_kv_level(
            view, field, level
        ):
            return True
    return log_view_starts_sequence2(view, level, StringSlice(" ")) or log_view_starts_sequence2(
        view, level, StringSlice(":")
    ) or log_view_starts_sequence2(view, level, StringSlice("\t")) or log_view_starts_sequence3(
        view, StringSlice("["), level, StringSlice("]")
    ) or log_view_contains_sequence3(
        view, StringSlice(" "), level, StringSlice(" ")
    ) or log_view_contains_sequence3(
        view, StringSlice(" "), level, StringSlice(":")
    ) or log_view_contains_sequence3(
        view, StringSlice(" ["), level, StringSlice("]")
    ) or log_view_contains_sequence3(
        view, StringSlice(" "), level, StringSlice("\t")
    )


@export("prodex_mojo_log_level_classify_v1")
def prodex_mojo_log_level_classify_v1(
    abi_version: Int64, event_address: UInt, level_address: UInt
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_LOG_LEVEL_ABI_VERSION
        or event_address == 0
        or level_address == 0
    ):
        return 1
    var event_ptr = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(event_address))
    var event = event_ptr[].copy()
    if not rich_view_valid(event, PRODEX_LOG_LEVEL_MAX_BYTES):
        return 2

    var level = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(level_address)
    )
    level[] = 0
    var candidate: Int64 = 1
    while candidate <= 7:
        if log_view_level_matches(event, candidate):
            if candidate >= 4:
                level[] = candidate - 1
            else:
                level[] = candidate
            return 0
        candidate += 1
    return 0


comptime PRODEX_TRANSCRIPT_ABI_VERSION: Int64 = 1
comptime PRODEX_TRANSCRIPT_TOOL_NAME_CHARS: Int64 = 96
comptime PRODEX_TRANSCRIPT_OPERATION_CHARS: Int64 = 192

comptime TRANSCRIPT_EVENT_UNKNOWN: Int64 = 0
comptime TRANSCRIPT_EVENT_PROTOCOL: Int64 = 1
comptime TRANSCRIPT_EVENT_STATUS_TERMINAL: Int64 = 2
comptime TRANSCRIPT_EVENT_STATUS_ERROR: Int64 = 3
comptime TRANSCRIPT_EVENT_USER: Int64 = 4
comptime TRANSCRIPT_EVENT_ASSISTANT: Int64 = 5
comptime TRANSCRIPT_EVENT_REASONING: Int64 = 6

comptime TRANSCRIPT_PROTOCOL_TOOL: Int64 = 0
comptime TRANSCRIPT_PROTOCOL_MCP: Int64 = 1
comptime TRANSCRIPT_PROTOCOL_AGENT: Int64 = 2

comptime TRANSCRIPT_ITEM_UNKNOWN: Int64 = 0
comptime TRANSCRIPT_ITEM_MESSAGE: Int64 = 1
comptime TRANSCRIPT_ITEM_FUNCTION_CALL: Int64 = 2
comptime TRANSCRIPT_ITEM_FUNCTION_OUTPUT: Int64 = 3
comptime TRANSCRIPT_ITEM_CUSTOM_CALL: Int64 = 4
comptime TRANSCRIPT_ITEM_CUSTOM_OUTPUT: Int64 = 5
comptime TRANSCRIPT_ITEM_SHELL_CALL: Int64 = 6
comptime TRANSCRIPT_ITEM_SHELL_OUTPUT: Int64 = 7
comptime TRANSCRIPT_ITEM_REASONING: Int64 = 8
comptime TRANSCRIPT_ITEM_PROTOCOL: Int64 = 9


def transcript_ascii_contains(
    view: ProdexRichStringView, needle: StringSlice
) -> Bool:
    var needle_length = Int64(needle.byte_length())
    if needle_length == 0 or needle_length > Int64(view.len):
        return False
    var ptr = rich_view_ptr(view)
    var expected = needle.unsafe_ptr()
    for start in range(Int64(view.len) - needle_length + 1):
        var matched = True
        for offset in range(needle_length):
            if ptr[unsafe_offset=start + offset] != expected[unsafe_offset=offset]:
                matched = False
                break
        if matched:
            return True
    return False


def transcript_view_equals[
    literal: StaticString
](view: ProdexRichStringView) -> Bool:
    return rich_view_matches_literal[literal](view, False)


def transcript_protocol_source(view: ProdexRichStringView) -> Int64:
    if transcript_ascii_contains(view, StringSlice("mcp")):
        return TRANSCRIPT_PROTOCOL_MCP
    if (
        transcript_ascii_contains(view, StringSlice("subagent"))
        or transcript_ascii_contains(view, StringSlice("sub_agent"))
    ):
        return TRANSCRIPT_PROTOCOL_AGENT
    return TRANSCRIPT_PROTOCOL_TOOL


def transcript_is_protocol_event(view: ProdexRichStringView) -> Bool:
    return (
        transcript_protocol_source(view) != TRANSCRIPT_PROTOCOL_TOOL
        or transcript_ascii_contains(view, StringSlice("tool_call"))
    )


def transcript_is_status_event(view: ProdexRichStringView) -> Bool:
    if (
        transcript_view_equals["task_started"](view)
        or transcript_view_equals["task_complete"](view)
        or transcript_view_equals["task_completed"](view)
        or transcript_view_equals["turn_started"](view)
        or transcript_view_equals["turn_complete"](view)
        or transcript_view_equals["turn_completed"](view)
        or transcript_view_equals["turn_aborted"](view)
        or transcript_view_equals["turn_cancelled"](view)
        or transcript_view_equals["turn_interrupted"](view)
        or transcript_view_equals["turn_failed"](view)
        or transcript_view_equals["command_execution_started"](view)
        or transcript_view_equals["command_execution_completed"](view)
        or transcript_view_equals["command_execution_finished"](view)
        or transcript_view_equals["command_execution_output"](view)
        or transcript_view_equals["exec_command_begin"](view)
        or transcript_view_equals["exec_command_end"](view)
        or transcript_view_equals["error"](view)
    ):
        return True
    return (
        transcript_ascii_contains(view, StringSlice("command"))
        and transcript_ascii_contains(view, StringSlice("status"))
    )


def transcript_status_is_error(
    event: ProdexRichStringView, status: ProdexRichStringView
) -> Bool:
    return (
        transcript_ascii_contains(event, StringSlice("fail"))
        or transcript_ascii_contains(event, StringSlice("abort"))
        or transcript_view_equals["error"](event)
        or transcript_ascii_contains(status, StringSlice("fail"))
        or transcript_ascii_contains(status, StringSlice("error"))
    )


@export("prodex_mojo_transcript_event_classify_v1")
def prodex_mojo_transcript_event_classify_v1(
    abi_version: Int64,
    event_address: UInt,
    event_length: Int64,
    status_address: UInt,
    status_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or event_length < 0
        or status_length < 0
        or output_address == 0
        or (event_length > 0 and event_address == 0)
        or (status_length > 0 and status_address == 0)
    ):
        return 1
    var event = ProdexRichStringView(event_address, UInt(event_length))
    var status = ProdexRichStringView(status_address, UInt(status_length))
    if (
        not rich_view_valid(event, event_length)
        or not rich_view_valid(status, status_length)
    ):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[0] = TRANSCRIPT_EVENT_UNKNOWN
    output[1] = TRANSCRIPT_PROTOCOL_TOOL

    if transcript_is_protocol_event(event):
        output[0] = TRANSCRIPT_EVENT_PROTOCOL
        output[1] = transcript_protocol_source(event)
        return 0
    if transcript_is_status_event(event):
        output[0] = (
            TRANSCRIPT_EVENT_STATUS_ERROR
            if transcript_status_is_error(event, status)
            else TRANSCRIPT_EVENT_STATUS_TERMINAL
        )
        return 0
    if transcript_view_equals["user_message"](event):
        output[0] = TRANSCRIPT_EVENT_USER
    elif transcript_view_equals["agent_message"](event):
        output[0] = TRANSCRIPT_EVENT_ASSISTANT
    elif transcript_view_equals["agent_reasoning"](event):
        output[0] = TRANSCRIPT_EVENT_REASONING
    return 0


@export("prodex_mojo_transcript_item_classify_v1")
def prodex_mojo_transcript_item_classify_v1(
    abi_version: Int64,
    item_address: UInt,
    item_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or item_length < 0
        or output_address == 0
        or (item_length > 0 and item_address == 0)
    ):
        return 1
    var item = ProdexRichStringView(item_address, UInt(item_length))
    if not rich_view_valid(item, item_length):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[0] = TRANSCRIPT_ITEM_UNKNOWN
    output[1] = TRANSCRIPT_PROTOCOL_TOOL
    if transcript_view_equals["message"](item):
        output[0] = TRANSCRIPT_ITEM_MESSAGE
    elif transcript_view_equals["function_call"](item):
        output[0] = TRANSCRIPT_ITEM_FUNCTION_CALL
    elif transcript_view_equals["function_call_output"](item):
        output[0] = TRANSCRIPT_ITEM_FUNCTION_OUTPUT
    elif transcript_view_equals["custom_tool_call"](item):
        output[0] = TRANSCRIPT_ITEM_CUSTOM_CALL
    elif transcript_view_equals["custom_tool_call_output"](item):
        output[0] = TRANSCRIPT_ITEM_CUSTOM_OUTPUT
    elif (
        transcript_view_equals["local_shell_call"](item)
        or transcript_view_equals["shell_call"](item)
    ):
        output[0] = TRANSCRIPT_ITEM_SHELL_CALL
    elif (
        transcript_view_equals["local_shell_call_output"](item)
        or transcript_view_equals["shell_call_output"](item)
    ):
        output[0] = TRANSCRIPT_ITEM_SHELL_OUTPUT
    elif transcript_view_equals["reasoning"](item):
        output[0] = TRANSCRIPT_ITEM_REASONING
    elif (
        transcript_protocol_source(item) != TRANSCRIPT_PROTOCOL_TOOL
        or transcript_view_equals["computer_call"](item)
        or transcript_view_equals["computer_call_output"](item)
        or transcript_view_equals["web_search_call"](item)
        or transcript_view_equals["file_search_call"](item)
        or transcript_view_equals["code_interpreter_call"](item)
    ):
        output[0] = TRANSCRIPT_ITEM_PROTOCOL
        output[1] = transcript_protocol_source(item)
    return 0


def transcript_codepoint_is_control(codepoint: Int64) -> Bool:
    return codepoint <= 31 or (codepoint >= 127 and codepoint <= 159)


@export("prodex_mojo_transcript_operation_span_v1")
def prodex_mojo_transcript_operation_span_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or value_length < 0
        or output_address == 0
        or (value_length > 0 and value_address == 0)
    ):
        return 1
    var view = ProdexRichStringView(value_address, UInt(value_length))
    if not rich_view_valid(view, value_length):
        return 2
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[0] = 0
    output[1] = 0

    var bounds = rich_trim_bounds(view)
    if bounds[0] >= bounds[1]:
        return 0

    var ptr = rich_view_ptr(view)
    var cursor: Int64 = 0
    var count: Int64 = 0
    var end: Int64 = 0
    while cursor < value_length:
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        var codepoint = rich_codepoint(ptr, cursor, width)
        if transcript_codepoint_is_control(codepoint):
            return 0
        if count < PRODEX_TRANSCRIPT_OPERATION_CHARS:
            end = cursor + width
        count += 1
        cursor += width
    output[0] = end
    output[1] = Int64(count > PRODEX_TRANSCRIPT_OPERATION_CHARS)
    return 0


@export("prodex_mojo_transcript_tool_name_v1")
def prodex_mojo_transcript_tool_name_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or value_length < 0
        or output_address == 0
        or output_capacity < PRODEX_TRANSCRIPT_TOOL_NAME_CHARS
        or written_address == 0
        or (value_length > 0 and value_address == 0)
    ):
        return 1
    var view = ProdexRichStringView(value_address, UInt(value_length))
    if not rich_view_valid(view, value_length):
        return 2
    var ptr = rich_view_ptr(view)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var cursor: Int64 = 0
    var count: Int64 = 0
    while cursor < value_length and count < PRODEX_TRANSCRIPT_TOOL_NAME_CHARS:
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        var codepoint = rich_codepoint(ptr, cursor, width)
        var emitted: UInt8 = 95
        if codepoint < 128:
            var byte = ptr[unsafe_offset=cursor]
            if (
                (byte >= 48 and byte <= 57)
                or (byte >= 65 and byte <= 90)
                or (byte >= 97 and byte <= 122)
                or byte == 95
                or byte == 45
                or byte == 46
                or byte == 58
            ):
                emitted = byte
        output[unsafe_offset=written[]] = emitted
        written[] += 1
        count += 1
        cursor += width
    if written[] == 0:
        var fallback = StringSlice("tool").unsafe_ptr()
        for index in range(4):
            output[unsafe_offset=index] = fallback[unsafe_offset=index]
        written[] = 4
    return 0
