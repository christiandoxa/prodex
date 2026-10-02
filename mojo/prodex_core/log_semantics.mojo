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
    output[unsafe_offset=0] = TRANSCRIPT_EVENT_UNKNOWN
    output[unsafe_offset=1] = TRANSCRIPT_PROTOCOL_TOOL

    if transcript_is_protocol_event(event):
        output[unsafe_offset=0] = TRANSCRIPT_EVENT_PROTOCOL
        output[unsafe_offset=1] = transcript_protocol_source(event)
        return 0
    if transcript_is_status_event(event):
        output[unsafe_offset=0] = (
            TRANSCRIPT_EVENT_STATUS_ERROR
            if transcript_status_is_error(event, status)
            else TRANSCRIPT_EVENT_STATUS_TERMINAL
        )
        return 0
    if transcript_view_equals["user_message"](event):
        output[unsafe_offset=0] = TRANSCRIPT_EVENT_USER
    elif transcript_view_equals["agent_message"](event):
        output[unsafe_offset=0] = TRANSCRIPT_EVENT_ASSISTANT
    elif transcript_view_equals["agent_reasoning"](event):
        output[unsafe_offset=0] = TRANSCRIPT_EVENT_REASONING
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
    output[unsafe_offset=0] = TRANSCRIPT_ITEM_UNKNOWN
    output[unsafe_offset=1] = TRANSCRIPT_PROTOCOL_TOOL
    if transcript_view_equals["message"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_MESSAGE
    elif transcript_view_equals["function_call"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_FUNCTION_CALL
    elif transcript_view_equals["function_call_output"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_FUNCTION_OUTPUT
    elif transcript_view_equals["custom_tool_call"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_CUSTOM_CALL
    elif transcript_view_equals["custom_tool_call_output"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_CUSTOM_OUTPUT
    elif (
        transcript_view_equals["local_shell_call"](item)
        or transcript_view_equals["shell_call"](item)
    ):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_SHELL_CALL
    elif (
        transcript_view_equals["local_shell_call_output"](item)
        or transcript_view_equals["shell_call_output"](item)
    ):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_SHELL_OUTPUT
    elif transcript_view_equals["reasoning"](item):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_REASONING
    elif (
        transcript_protocol_source(item) != TRANSCRIPT_PROTOCOL_TOOL
        or transcript_view_equals["computer_call"](item)
        or transcript_view_equals["computer_call_output"](item)
        or transcript_view_equals["web_search_call"](item)
        or transcript_view_equals["file_search_call"](item)
        or transcript_view_equals["code_interpreter_call"](item)
    ):
        output[unsafe_offset=0] = TRANSCRIPT_ITEM_PROTOCOL
        output[unsafe_offset=1] = transcript_protocol_source(item)
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
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0

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
    output[unsafe_offset=0] = end
    output[unsafe_offset=1] = Int64(count > PRODEX_TRANSCRIPT_OPERATION_CHARS)
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


comptime PRODEX_UPSTREAM_PAYLOAD_ABI_VERSION: Int64 = 1
comptime UPSTREAM_BINARY_UNKNOWN: Int64 = 0
comptime UPSTREAM_BINARY_PNG: Int64 = 1
comptime UPSTREAM_BINARY_JPEG: Int64 = 2
comptime UPSTREAM_BINARY_GIF: Int64 = 3
comptime UPSTREAM_BINARY_PDF: Int64 = 4
comptime UPSTREAM_BINARY_ZIP: Int64 = 5
comptime UPSTREAM_BINARY_GZIP: Int64 = 6
comptime UPSTREAM_BINARY_ZSTD: Int64 = 7
comptime UPSTREAM_BINARY_WEBP: Int64 = 8


def upstream_payload_prefix(
    ptr: Pointer[mut=False, UInt8, _], length: Int64, literal: StringSlice
) -> Bool:
    var expected_length = Int64(literal.byte_length())
    if expected_length > length:
        return False
    var expected = literal.unsafe_ptr()
    for index in range(expected_length):
        if ptr[unsafe_offset=index] != expected[unsafe_offset=index]:
            return False
    return True


def upstream_payload_magic_kind(
    ptr: Pointer[mut=False, UInt8, _], length: Int64
) -> Int64:
    if (
        length >= 8
        and ptr[unsafe_offset=0] == 0x89
        and ptr[unsafe_offset=1] == 0x50
        and ptr[unsafe_offset=2] == 0x4E
        and ptr[unsafe_offset=3] == 0x47
        and ptr[unsafe_offset=4] == 0x0D
        and ptr[unsafe_offset=5] == 0x0A
        and ptr[unsafe_offset=6] == 0x1A
        and ptr[unsafe_offset=7] == 0x0A
    ):
        return UPSTREAM_BINARY_PNG
    if length >= 3 and ptr[unsafe_offset=0] == 0xFF and ptr[unsafe_offset=1] == 0xD8 and ptr[unsafe_offset=2] == 0xFF:
        return UPSTREAM_BINARY_JPEG
    if (
        upstream_payload_prefix(ptr, length, StringSlice("GIF87a"))
        or upstream_payload_prefix(ptr, length, StringSlice("GIF89a"))
    ):
        return UPSTREAM_BINARY_GIF
    if upstream_payload_prefix(ptr, length, StringSlice("%PDF-")):
        return UPSTREAM_BINARY_PDF
    if length >= 4 and ptr[unsafe_offset=0] == 0x50 and ptr[unsafe_offset=1] == 0x4B and ptr[unsafe_offset=2] == 0x03 and ptr[unsafe_offset=3] == 0x04:
        return UPSTREAM_BINARY_ZIP
    if length >= 2 and ptr[unsafe_offset=0] == 0x1F and ptr[unsafe_offset=1] == 0x8B:
        return UPSTREAM_BINARY_GZIP
    if (
        length >= 4
        and ptr[unsafe_offset=0] == 0x28
        and ptr[unsafe_offset=1] == 0xB5
        and ptr[unsafe_offset=2] == 0x2F
        and ptr[unsafe_offset=3] == 0xFD
    ):
        return UPSTREAM_BINARY_ZSTD
    if (
        length >= 12
        and upstream_payload_prefix(ptr, length, StringSlice("RIFF"))
        and ptr[unsafe_offset=8] == 0x57
        and ptr[unsafe_offset=9] == 0x45
        and ptr[unsafe_offset=10] == 0x42
        and ptr[unsafe_offset=11] == 0x50
    ):
        return UPSTREAM_BINARY_WEBP
    return UPSTREAM_BINARY_UNKNOWN


@export("prodex_mojo_upstream_payload_classify_v1")
def prodex_mojo_upstream_payload_classify_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_UPSTREAM_PAYLOAD_ABI_VERSION
        or input_length < 0
        or output_address == 0
        or (input_length > 0 and input_address == 0)
    ):
        return 1

    var ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = upstream_payload_magic_kind(ptr, input_length)

    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, input_length):
        return 0

    var cursor: Int64 = 0
    while cursor < input_length:
        var width = rich_codepoint_width(ptr[unsafe_offset=cursor])
        var codepoint = rich_codepoint(ptr, cursor, width)
        if (
            transcript_codepoint_is_control(codepoint)
            and codepoint != 9
            and codepoint != 10
            and codepoint != 13
        ):
            return 0
        cursor += width
    output[unsafe_offset=0] = 1
    return 0

comptime PREVIOUS_RESPONSE_LOG_ABI_VERSION: Int64 = 1
comptime PREVIOUS_RESPONSE_LOG_NOT_FOUND: Int64 = 0
comptime PREVIOUS_RESPONSE_LOG_RETRY_IMMEDIATE: Int64 = 1
comptime PREVIOUS_RESPONSE_LOG_STALE_CONTINUATION: Int64 = 2
comptime PREVIOUS_RESPONSE_LOG_FRESH_FALLBACK: Int64 = 3
comptime PREVIOUS_RESPONSE_LOG_AFFINITY_RELEASED: Int64 = 4

@fieldwise_init
struct PreviousResponseLogWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def previous_response_log_put_byte(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _], byte: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = byte
    writer[].written += 1
    return True


def previous_response_log_put_literal(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _], value: StringSlice
) -> Bool:
    var source = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not previous_response_log_put_byte(
            writer, source[unsafe_offset=index]
        ):
            return False
    return True


def previous_response_log_put_view(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    value: ProdexRichStringView,
) -> Bool:
    var source = rich_view_ptr(value)
    for index in range(Int64(value.len)):
        if not previous_response_log_put_byte(
            writer, source[unsafe_offset=index]
        ):
            return False
    return True


def previous_response_log_put_u64(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return previous_response_log_put_byte(writer, UInt8(48))
    var divisor: UInt64 = 1
    while value / divisor >= UInt64(10):
        divisor *= UInt64(10)
    var remaining = value
    while divisor > 0:
        if not previous_response_log_put_byte(
            writer, UInt8(remaining / divisor) + UInt8(48)
        ):
            return False
        remaining %= divisor
        divisor //= UInt64(10)
    return True


def previous_response_log_text(
    address: UInt, index: Int64
) -> ProdexRichStringView:
    var values = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(address))
    return values[unsafe_offset=index].copy()


def previous_response_log_validate_texts(address: UInt, count: Int64) -> Bool:
    if count != 6 or address == 0:
        return False
    var values = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(address))
    for index in range(count):
        var value = values[unsafe_offset=index].copy()
        if not rich_view_valid(value, Int64(value.len)):
            return False
    return True


def previous_response_log_put_suffix(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    if presence & UInt64(2) == 0:
        return True
    return (
        previous_response_log_put_literal(writer, StringSlice(" via="))
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 2)
        )
    )


def previous_response_log_put_not_found_prefix(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    request_id: UInt64,
    websocket_session: UInt64,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    if not (
        previous_response_log_put_literal(writer, StringSlice("request="))
        and previous_response_log_put_u64(writer, request_id)
        and previous_response_log_put_literal(writer, StringSlice(" transport="))
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 0)
        )
        and previous_response_log_put_literal(writer, StringSlice(" route="))
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 1)
        )
    ):
        return False
    if presence & UInt64(1) != 0:
        return (
            previous_response_log_put_literal(
                writer, StringSlice(" websocket_session=")
            )
            and previous_response_log_put_u64(writer, websocket_session)
        )
    return True


def previous_response_log_put_event_prefix(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    request_id: UInt64,
    websocket_session: UInt64,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    if not (
        previous_response_log_put_literal(writer, StringSlice("request="))
        and previous_response_log_put_u64(writer, request_id)
    ):
        return False
    if presence & UInt64(1) != 0:
        return (
            previous_response_log_put_literal(
                writer, StringSlice(" websocket_session=")
            )
            and previous_response_log_put_u64(writer, websocket_session)
        )
    return (
        previous_response_log_put_literal(writer, StringSlice(" transport="))
        and previous_response_log_put_view(
            writer, previous_response_log_text(text_address, 0)
        )
    )


def previous_response_log_render(
    writer: Pointer[mut=True, PreviousResponseLogWriter, _],
    operation: Int64,
    request_id: UInt64,
    websocket_session: UInt64,
    retry_index: UInt64,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    var profile = previous_response_log_text(text_address, 3)
    if operation == PREVIOUS_RESPONSE_LOG_NOT_FOUND:
        return (
            previous_response_log_put_not_found_prefix(
                writer,
                request_id,
                websocket_session,
                text_address,
                presence,
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" previous_response_not_found profile=")
            )
            and previous_response_log_put_view(writer, profile)
            and previous_response_log_put_literal(
                writer, StringSlice(" retry_index=")
            )
            and previous_response_log_put_u64(writer, retry_index)
            and previous_response_log_put_literal(
                writer, StringSlice(" replay_turn_state=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 4)
            )
            and previous_response_log_put_suffix(
                writer, text_address, presence
            )
        )
    if operation == PREVIOUS_RESPONSE_LOG_RETRY_IMMEDIATE:
        return (
            previous_response_log_put_event_prefix(
                writer,
                request_id,
                websocket_session,
                text_address,
                presence,
            )
            and previous_response_log_put_literal(
                writer,
                StringSlice(" previous_response_retry_immediate profile="),
            )
            and previous_response_log_put_view(writer, profile)
            and previous_response_log_put_literal(
                writer, StringSlice(" delay_ms=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 4)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" reason=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 5)
            )
            and previous_response_log_put_suffix(
                writer, text_address, presence
            )
        )
    if operation == PREVIOUS_RESPONSE_LOG_STALE_CONTINUATION:
        return (
            previous_response_log_put_event_prefix(
                writer,
                request_id,
                websocket_session,
                text_address,
                presence,
            )
            and previous_response_log_put_literal(
                writer,
                StringSlice(
                    " stale_continuation reason=previous_response_not_found_locked_affinity profile="
                ),
            )
            and previous_response_log_put_view(writer, profile)
            and previous_response_log_put_suffix(
                writer, text_address, presence
            )
        )
    if operation == PREVIOUS_RESPONSE_LOG_FRESH_FALLBACK:
        if not previous_response_log_put_event_prefix(
            writer,
            request_id,
            websocket_session,
            text_address,
            presence,
        ):
            return False
        if presence & UInt64(4) != 0:
            if not previous_response_log_put_literal(
                writer,
                StringSlice(" previous_response_fresh_fallback_blocked"),
            ):
                return False
        elif not previous_response_log_put_literal(
            writer, StringSlice(" previous_response_fresh_fallback")
        ):
            return False
        return (
            previous_response_log_put_literal(
                writer,
                StringSlice(
                    " reason=previous_response_not_found request_shape="
                ),
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 4)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" outcome=")
            )
            and previous_response_log_put_view(
                writer, previous_response_log_text(text_address, 5)
            )
            and previous_response_log_put_literal(
                writer, StringSlice(" profile=")
            )
            and previous_response_log_put_view(writer, profile)
            and previous_response_log_put_suffix(
                writer, text_address, presence
            )
        )
    return (
        previous_response_log_put_event_prefix(
            writer,
            request_id,
            websocket_session,
            text_address,
            presence,
        )
        and previous_response_log_put_literal(
            writer, StringSlice(" previous_response_affinity_released profile=")
        )
        and previous_response_log_put_view(writer, profile)
        and previous_response_log_put_suffix(writer, text_address, presence)
    )


@export("prodex_mojo_previous_response_log_render_v1")
def prodex_mojo_previous_response_log_render_v1(
    abi_version: Int64,
    operation: Int64,
    request_id: UInt64,
    websocket_session: UInt64,
    retry_index: UInt64,
    text_address: UInt,
    text_count: Int64,
    presence: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PREVIOUS_RESPONSE_LOG_ABI_VERSION:
        return 4
    if (
        operation < PREVIOUS_RESPONSE_LOG_NOT_FOUND
        or operation > PREVIOUS_RESPONSE_LOG_AFFINITY_RELEASED
        or presence > UInt64(7)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
        or not previous_response_log_validate_texts(text_address, text_count)
    ):
        return 1
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var writer = PreviousResponseLogWriter(output, output_capacity, 0)
    var ok = previous_response_log_render(
        Pointer(to=writer),
        operation,
        request_id,
        websocket_session,
        retry_index,
        text_address,
        presence,
    )
    written[] = writer.written
    if not ok:
        return 2
    return 0
