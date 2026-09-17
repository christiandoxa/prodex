from std.memory import Pointer

from anthropic_request import (
    anthropic_request_byte,
    anthropic_request_object_field,
    anthropic_request_range_matches_literal,
    anthropic_request_skip_ws,
    anthropic_request_string_end,
    anthropic_request_value_end,
)
from rich_text import rich_trim_bounds, rich_view_matches_literal, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView


comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES: Int64 = 4_194_304
comptime RUNTIME_ANTHROPIC_KERNEL_STATUS_OK: Int64 = 0
comptime RUNTIME_ANTHROPIC_KERNEL_STATUS_INVALID: Int64 = 1
comptime RUNTIME_ANTHROPIC_KERNEL_STATUS_UTF8: Int64 = 2
comptime RUNTIME_ANTHROPIC_KERNEL_STATUS_CAPACITY: Int64 = 3
comptime RUNTIME_ANTHROPIC_KERNEL_STATUS_ABI: Int64 = 4

comptime RUNTIME_ANTHROPIC_SSE_EVENT: Int64 = 1
comptime RUNTIME_ANTHROPIC_MESSAGE_SSE: Int64 = 2
comptime RUNTIME_ANTHROPIC_RESPONSE_MESSAGE: Int64 = 3
comptime RUNTIME_ANTHROPIC_USAGE: Int64 = 4
comptime RUNTIME_ANTHROPIC_INPUT_TEXT: Int64 = 5
comptime RUNTIME_ANTHROPIC_IMAGE_PART: Int64 = 6
comptime RUNTIME_ANTHROPIC_FUNCTION_CALL: Int64 = 7
comptime RUNTIME_ANTHROPIC_FUNCTION_CALL_OUTPUT: Int64 = 8
comptime RUNTIME_ANTHROPIC_SHELL_TOOL_RESULT: Int64 = 9
comptime RUNTIME_ANTHROPIC_COMPUTER_TOOL_RESULT: Int64 = 10
comptime RUNTIME_ANTHROPIC_TOOL_USE_BLOCK: Int64 = 11
comptime RUNTIME_ANTHROPIC_MCP_CALL_BLOCKS: Int64 = 12
comptime RUNTIME_ANTHROPIC_MCP_APPROVAL_BLOCK: Int64 = 13
comptime RUNTIME_ANTHROPIC_MCP_LIST_TOOLS_BLOCK: Int64 = 14
comptime RUNTIME_ANTHROPIC_SERVER_TOOL_BLOCK: Int64 = 15
comptime RUNTIME_ANTHROPIC_THINKING_BLOCK: Int64 = 16
comptime RUNTIME_ANTHROPIC_TEXT_BLOCK: Int64 = 17
comptime RUNTIME_ANTHROPIC_TOOL_RESULT_TEXT_PLAN: Int64 = 18
comptime RUNTIME_ANTHROPIC_COMPUTER_ACTION: Int64 = 19
comptime RUNTIME_ANTHROPIC_COMPUTER_TOOL_INPUT: Int64 = 20
comptime RUNTIME_ANTHROPIC_SERVER_TOOL_USAGE: Int64 = 21
comptime RUNTIME_ANTHROPIC_CARRIED_SERVER_TOOL_USAGE: Int64 = 22
comptime RUNTIME_ANTHROPIC_SERVER_TOOL_REGISTRATIONS: Int64 = 23
comptime RUNTIME_ANTHROPIC_MESSAGE_HAS_TOOL_CHAIN: Int64 = 24
comptime RUNTIME_ANTHROPIC_SERVER_TOOL_NAME_KIND: Int64 = 25
comptime RUNTIME_ANTHROPIC_CLIENT_TOOL_DESCRIPTION: Int64 = 26
comptime RUNTIME_ANTHROPIC_CLIENT_TOOL_SCHEMA: Int64 = 27
comptime RUNTIME_ANTHROPIC_UNVERSIONED_TOOL_TYPE: Int64 = 28
comptime RUNTIME_ANTHROPIC_CLIENT_TOOL_NAME_FROM_TYPE: Int64 = 29
comptime RUNTIME_ANTHROPIC_TOOL_VERSION: Int64 = 30
comptime RUNTIME_ANTHROPIC_SERVER_TOOL_NAME_FROM_TYPE: Int64 = 31
comptime RUNTIME_ANTHROPIC_CLIENT_TOOL_NAME: Int64 = 32
comptime RUNTIME_ANTHROPIC_IS_TOOL_USE_BLOCK_TYPE: Int64 = 33
comptime RUNTIME_ANTHROPIC_IS_TOOL_RESULT_BLOCK_TYPE: Int64 = 34
comptime RUNTIME_ANTHROPIC_TRANSLATE_REASONING_EFFORT: Int64 = 35

comptime RUNTIME_ANTHROPIC_FLAG_ERROR: Int64 = 1
comptime RUNTIME_ANTHROPIC_FLAG_MAX_OUTPUT_LENGTH: Int64 = 2
comptime RUNTIME_ANTHROPIC_FLAG_CACHED_TOKENS: Int64 = 4
comptime RUNTIME_ANTHROPIC_FLAG_SUPPORTS_XHIGH: Int64 = 8


@fieldwise_init
struct ProdexRuntimeAnthropicKernelInput(Copyable):
    var operation: Int64
    var index: UInt64
    var flags: Int64
    var input_tokens: UInt64
    var output_tokens: UInt64
    var cached_tokens: UInt64
    var web_search_requests: UInt64
    var web_fetch_requests: UInt64
    var code_execution_requests: UInt64
    var tool_search_requests: UInt64
    var max_output_length: UInt64
    var max_output_length_present: Int64
    var id_present: Int64
    var name_present: Int64
    var block_type_present: Int64
    var server_name_present: Int64
    var text_present: Int64
    var input_present: Int64
    var output_present: Int64
    var content_present: Int64
    var usage_present: Int64
    var stop_reason_present: Int64
    var message_present: Int64
    var id: ProdexRichStringView
    var name: ProdexRichStringView
    var block_type: ProdexRichStringView
    var server_name: ProdexRichStringView
    var text: ProdexRichStringView
    var input: ProdexRichStringView
    var output: ProdexRichStringView
    var content: ProdexRichStringView
    var usage: ProdexRichStringView
    var stop_reason: ProdexRichStringView
    var message: ProdexRichStringView


@fieldwise_init
struct RuntimeAnthropicKernelWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def runtime_anthropic_put_byte(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _], value: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True


def runtime_anthropic_put_literal(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _], value: StringSlice
) -> Bool:
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not runtime_anthropic_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def runtime_anthropic_put_view_range(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    if end == start:
        return True
    var ptr = rich_view_ptr(view)
    for offset in range(end - start):
        if not runtime_anthropic_put_byte(writer, ptr[unsafe_offset=start + offset]):
            return False
    return True


def runtime_anthropic_put_view(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    return runtime_anthropic_put_view_range(writer, view, 0, Int64(view.len))


def runtime_anthropic_put_u64(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return runtime_anthropic_put_byte(writer, 48)
    var divisor: UInt64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        if not runtime_anthropic_put_byte(writer, UInt8(remaining / divisor) + 48):
            return False
        remaining %= divisor
        divisor /= 10
    return True


def runtime_anthropic_put_hex_byte(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _], value: UInt8
) -> Bool:
    var high = (value >> 4) & 15
    var low = value & 15
    if high < 10:
        high += 48
    else:
        high += 87
    if low < 10:
        low += 48
    else:
        low += 87
    return runtime_anthropic_put_byte(writer, high) and runtime_anthropic_put_byte(
        writer, low
    )


def runtime_anthropic_put_json_string(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if not runtime_anthropic_put_byte(writer, 34):
        return False
    if view.len > 0:
        var ptr = rich_view_ptr(view)
        for index in range(Int64(view.len)):
            var value = ptr[unsafe_offset=index]
            if value == 34 or value == 92:
                if not runtime_anthropic_put_byte(writer, 92) or not runtime_anthropic_put_byte(
                    writer, value
                ):
                    return False
            elif value == 8:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\b")):
                    return False
            elif value == 9:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\t")):
                    return False
            elif value == 10:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\n")):
                    return False
            elif value == 12:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\f")):
                    return False
            elif value == 13:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\r")):
                    return False
            elif value < 32:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\u00")) or not runtime_anthropic_put_hex_byte(
                    writer, value
                ):
                    return False
            elif not runtime_anthropic_put_byte(writer, value):
                return False
    return runtime_anthropic_put_byte(writer, 34)


def runtime_anthropic_put_json_string_range(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    return (
        runtime_anthropic_put_byte(writer, 34)
        and runtime_anthropic_put_json_range_content(writer, view, start, end)
        and runtime_anthropic_put_byte(writer, 34)
    )


def runtime_anthropic_put_json_range_content(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    if end > start:
        var ptr = rich_view_ptr(view)
        for index in range(end - start):
            var value = ptr[unsafe_offset=start + index]
            if value == 34 or value == 92:
                if not runtime_anthropic_put_byte(writer, 92) or not runtime_anthropic_put_byte(
                    writer, value
                ):
                    return False
            elif value == 8:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\b")):
                    return False
            elif value == 9:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\t")):
                    return False
            elif value == 10:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\n")):
                    return False
            elif value == 12:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\f")):
                    return False
            elif value == 13:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\r")):
                    return False
            elif value < 32:
                if not runtime_anthropic_put_literal(writer, StringSlice("\\u00")) or not runtime_anthropic_put_hex_byte(
                    writer, value
                ):
                    return False
            elif not runtime_anthropic_put_byte(writer, value):
                return False
    return True


def runtime_anthropic_json_string_valid(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    return (
        start >= 0
        and end > start
        and anthropic_request_byte(view, start) == 34
        and anthropic_request_string_end(view, start, end) == end
    )


def runtime_anthropic_put_field_or_literal(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    field: InlineArray[Int64, 2],
    fallback: StringSlice,
) -> Bool:
    if field[0] >= 0:
        return runtime_anthropic_put_view_range(writer, view, field[0], field[1])
    return runtime_anthropic_put_literal(writer, fallback)


def runtime_anthropic_put_string_field_or_literal(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    field: InlineArray[Int64, 2],
    fallback: StringSlice,
) -> Bool:
    if runtime_anthropic_json_string_valid(view, field[0], field[1]):
        return runtime_anthropic_put_view_range(writer, view, field[0], field[1])
    return runtime_anthropic_put_literal(writer, fallback)


def runtime_anthropic_put_input_json_string(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    field: InlineArray[Int64, 2],
) -> Bool:
    if field[0] >= 0:
        return runtime_anthropic_put_json_string_range(writer, view, field[0], field[1])
    return runtime_anthropic_put_literal(writer, StringSlice("\"{}\""))


def runtime_anthropic_put_tool_result_id(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    kind: InlineArray[Int64, 2],
    tool_use_id: InlineArray[Int64, 2],
) -> Bool:
    if tool_use_id[0] >= 0:
        return runtime_anthropic_put_view_range(
            writer, view, tool_use_id[0], tool_use_id[1]
        )
    if not runtime_anthropic_put_byte(writer, 34):
        return False
    if runtime_anthropic_json_string_valid(view, kind[0], kind[1]):
        if not runtime_anthropic_put_view_range(writer, view, kind[0] + 1, kind[1] - 1):
            return False
        if not runtime_anthropic_put_literal(writer, StringSlice("_call")):
            return False
    else:
        if not runtime_anthropic_put_literal(writer, StringSlice("tool_result_call")):
            return False
    return runtime_anthropic_put_byte(writer, 34)


def runtime_anthropic_event_start(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _], event: StringSlice
) -> Bool:
    return runtime_anthropic_put_literal(writer, StringSlice("event: ")) and runtime_anthropic_put_literal(
        writer, event
    ) and runtime_anthropic_put_literal(writer, StringSlice("\ndata: "))


def runtime_anthropic_event_end(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _]
) -> Bool:
    return runtime_anthropic_put_literal(writer, StringSlice("\n\n"))


def runtime_anthropic_write_usage(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if not runtime_anthropic_put_literal(writer, StringSlice("{\"input_tokens\":")) or not runtime_anthropic_put_u64(
        writer, input.input_tokens
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"output_tokens\":")) or not runtime_anthropic_put_u64(
        writer, input.output_tokens
    ):
        return False
    if input.flags & RUNTIME_ANTHROPIC_FLAG_CACHED_TOKENS != 0:
        if not runtime_anthropic_put_literal(writer, StringSlice(",\"cache_read_input_tokens\":")) or not runtime_anthropic_put_u64(
            writer, input.cached_tokens
        ):
            return False
    if not runtime_anthropic_put_literal(writer, StringSlice(",\"server_tool_use\":{\"web_search_requests\":")) or not runtime_anthropic_put_u64(
        writer, input.web_search_requests
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"web_fetch_requests\":")) or not runtime_anthropic_put_u64(
        writer, input.web_fetch_requests
    ):
        return False
    if input.code_execution_requests > 0:
        if not runtime_anthropic_put_literal(writer, StringSlice(",\"code_execution_requests\":")) or not runtime_anthropic_put_u64(
            writer, input.code_execution_requests
        ):
            return False
    if input.tool_search_requests > 0:
        if not runtime_anthropic_put_literal(writer, StringSlice(",\"tool_search_requests\":")) or not runtime_anthropic_put_u64(
            writer, input.tool_search_requests
        ):
            return False
    return runtime_anthropic_put_literal(writer, StringSlice("}}"))


def runtime_anthropic_write_response_message(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if (
        input.id_present == 0
        or input.name_present == 0
        or input.content_present == 0
        or input.usage_present == 0
        or input.stop_reason_present == 0
    ):
        return False
    return (
        runtime_anthropic_put_literal(writer, StringSlice("{\"id\":"))
        and runtime_anthropic_put_json_string(writer, input.id)
        and runtime_anthropic_put_literal(
            writer, StringSlice(",\"type\":\"message\",\"role\":\"assistant\",\"content\":")
        )
        and runtime_anthropic_put_view(writer, input.content)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"model\":"))
        and runtime_anthropic_put_json_string(writer, input.name)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"stop_reason\":"))
        and runtime_anthropic_put_json_string(writer, input.stop_reason)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"stop_sequence\":null,\"usage\":"))
        and runtime_anthropic_put_view(writer, input.usage)
        and runtime_anthropic_put_byte(writer, 125)
    )


def runtime_anthropic_write_empty_content_block(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    index: UInt64,
    kind: StringSlice,
    field: StringSlice,
) -> Bool:
    return (
        runtime_anthropic_event_start(writer, StringSlice("content_block_start"))
        and runtime_anthropic_put_literal(
            writer, StringSlice("{\"type\":\"content_block_start\",\"index\":")
        )
        and runtime_anthropic_put_u64(writer, index)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"content_block\":{\"type\":"))
        and runtime_anthropic_put_literal(writer, kind)
        and runtime_anthropic_put_literal(writer, StringSlice(",\""))
        and runtime_anthropic_put_literal(writer, field)
        and runtime_anthropic_put_literal(writer, StringSlice("\":\"\"}}"))
        and runtime_anthropic_event_end(writer)
    )


def runtime_anthropic_write_input_block_start(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    index: UInt64,
    kind: StringSlice,
    id_field: InlineArray[Int64, 2],
    name_field: InlineArray[Int64, 2],
    server_field: InlineArray[Int64, 2],
    id_fallback: StringSlice,
    name_fallback: StringSlice,
    include_server: Bool,
) -> Bool:
    if not runtime_anthropic_event_start(writer, StringSlice("content_block_start")) or not runtime_anthropic_put_literal(
        writer, StringSlice("{\"type\":\"content_block_start\",\"index\":")
    ) or not runtime_anthropic_put_u64(writer, index) or not runtime_anthropic_put_literal(
        writer, StringSlice(",\"content_block\":{\"type\":")
    ) or not runtime_anthropic_put_literal(writer, kind):
        return False
    if not runtime_anthropic_put_literal(writer, StringSlice(",\"id\":")) or not runtime_anthropic_put_field_or_literal(
        writer, view, id_field, id_fallback
    ):
        return False
    if not runtime_anthropic_put_literal(writer, StringSlice(",\"name\":")) or not runtime_anthropic_put_field_or_literal(
        writer, view, name_field, name_fallback
    ):
        return False
    if include_server:
        if not runtime_anthropic_put_literal(writer, StringSlice(",\"server_name\":")) or not runtime_anthropic_put_field_or_literal(
            writer, view, server_field, StringSlice("\"mcp\"")
        ):
            return False
    return runtime_anthropic_put_literal(writer, StringSlice(",\"input\":{}}}")) and runtime_anthropic_event_end(
        writer
    )


def runtime_anthropic_write_input_delta(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    index: UInt64,
    input_field: InlineArray[Int64, 2],
) -> Bool:
    return (
        runtime_anthropic_event_start(writer, StringSlice("content_block_delta"))
        and runtime_anthropic_put_literal(
            writer, StringSlice("{\"type\":\"content_block_delta\",\"index\":")
        )
        and runtime_anthropic_put_u64(writer, index)
        and runtime_anthropic_put_literal(
            writer, StringSlice(",\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":")
        )
        and runtime_anthropic_put_input_json_string(writer, view, input_field)
        and runtime_anthropic_put_literal(writer, StringSlice("}}"))
        and runtime_anthropic_event_end(writer)
    )


def runtime_anthropic_write_content_block_stop(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _], index: UInt64
) -> Bool:
    return (
        runtime_anthropic_event_start(writer, StringSlice("content_block_stop"))
        and runtime_anthropic_put_literal(
            writer, StringSlice("{\"type\":\"content_block_stop\",\"index\":")
        )
        and runtime_anthropic_put_u64(writer, index)
        and runtime_anthropic_put_literal(writer, StringSlice("}"))
        and runtime_anthropic_event_end(writer)
    )


def runtime_anthropic_write_text_delta(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    index: UInt64,
    text_field: InlineArray[Int64, 2],
) -> Bool:
    return (
        runtime_anthropic_event_start(writer, StringSlice("content_block_delta"))
        and runtime_anthropic_put_literal(
            writer, StringSlice("{\"type\":\"content_block_delta\",\"index\":")
        )
        and runtime_anthropic_put_u64(writer, index)
        and runtime_anthropic_put_literal(
            writer, StringSlice(",\"delta\":{\"type\":\"text_delta\",\"text\":")
        )
        and runtime_anthropic_put_string_field_or_literal(
            writer, view, text_field, StringSlice("\"\"")
        )
        and runtime_anthropic_put_literal(writer, StringSlice("}}"))
        and runtime_anthropic_event_end(writer)
    )


def runtime_anthropic_write_thinking_delta(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    index: UInt64,
    thinking_field: InlineArray[Int64, 2],
) -> Bool:
    return (
        runtime_anthropic_event_start(writer, StringSlice("content_block_delta"))
        and runtime_anthropic_put_literal(
            writer, StringSlice("{\"type\":\"content_block_delta\",\"index\":")
        )
        and runtime_anthropic_put_u64(writer, index)
        and runtime_anthropic_put_literal(
            writer, StringSlice(",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":")
        )
        and runtime_anthropic_put_string_field_or_literal(
            writer, view, thinking_field, StringSlice("\"\"")
        )
        and runtime_anthropic_put_literal(writer, StringSlice("}}"))
        and runtime_anthropic_event_end(writer)
    )


def runtime_anthropic_write_message_sse_block(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    index: UInt64,
) -> Bool:
    var kind = anthropic_request_object_field(view, start, end, StringSlice("\"type\""))
    var id = anthropic_request_object_field(view, start, end, StringSlice("\"id\""))
    var name = anthropic_request_object_field(view, start, end, StringSlice("\"name\""))
    var server = anthropic_request_object_field(view, start, end, StringSlice("\"server_name\""))
    var input = anthropic_request_object_field(view, start, end, StringSlice("\"input\""))
    var thinking = anthropic_request_object_field(view, start, end, StringSlice("\"thinking\""))
    var text = anthropic_request_object_field(view, start, end, StringSlice("\"text\""))
    var tool_use_id = anthropic_request_object_field(view, start, end, StringSlice("\"tool_use_id\""))
    var content = anthropic_request_object_field(view, start, end, StringSlice("\"content\""))

    if kind[0] >= 0 and anthropic_request_range_matches_literal(
        view, kind[0], kind[1], StringSlice("\"thinking\"")
    ):
        if not runtime_anthropic_write_empty_content_block(
            writer, index, StringSlice("\"thinking\""), StringSlice("thinking")
        ) or not runtime_anthropic_write_thinking_delta(writer, view, index, thinking):
            return False
    elif kind[0] >= 0 and anthropic_request_range_matches_literal(
        view, kind[0], kind[1], StringSlice("\"tool_use\"")
    ):
        if not runtime_anthropic_write_input_block_start(
            writer,
            view,
            index,
            StringSlice("\"tool_use\""),
            id,
            name,
            server,
            StringSlice("\"tool_use\""),
            StringSlice("\"tool\""),
            False,
        ) or not runtime_anthropic_write_input_delta(writer, view, index, input):
            return False
    elif kind[0] >= 0 and anthropic_request_range_matches_literal(
        view, kind[0], kind[1], StringSlice("\"server_tool_use\"")
    ):
        if not runtime_anthropic_write_input_block_start(
            writer,
            view,
            index,
            StringSlice("\"server_tool_use\""),
            id,
            name,
            server,
            StringSlice("\"server_tool_use\""),
            StringSlice("\"web_search\""),
            False,
        ) or not runtime_anthropic_write_input_delta(writer, view, index, input):
            return False
    elif kind[0] >= 0 and anthropic_request_range_matches_literal(
        view, kind[0], kind[1], StringSlice("\"mcp_tool_use\"")
    ):
        if not runtime_anthropic_write_input_block_start(
            writer,
            view,
            index,
            StringSlice("\"mcp_tool_use\""),
            id,
            name,
            server,
            StringSlice("\"mcp_tool_use\""),
            StringSlice("\"mcp_tool\""),
            True,
        ) or not runtime_anthropic_write_input_delta(writer, view, index, input):
            return False
    elif kind[0] >= 0 and kind[1] - kind[0] >= 13 and anthropic_request_range_matches_literal(
        view,
        kind[1] - 13,
        kind[1],
        StringSlice("_tool_result\""),
    ):
        if not runtime_anthropic_event_start(writer, StringSlice("content_block_start")) or not runtime_anthropic_put_literal(
            writer, StringSlice("{\"type\":\"content_block_start\",\"index\":")
        ) or not runtime_anthropic_put_u64(writer, index) or not runtime_anthropic_put_literal(
            writer, StringSlice(",\"content_block\":{\"type\":")
        ) or not runtime_anthropic_put_view_range(writer, view, kind[0], kind[1]) or not runtime_anthropic_put_literal(
            writer, StringSlice(",\"tool_use_id\":")
        ) or not runtime_anthropic_put_tool_result_id(
            writer, view, kind, tool_use_id
        ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"content\":")):
            return False
        if content[0] >= 0:
            if not runtime_anthropic_put_view_range(writer, view, content[0], content[1]):
                return False
        elif not runtime_anthropic_put_literal(writer, StringSlice("null")):
            return False
        if not runtime_anthropic_put_literal(writer, StringSlice("}}")) or not runtime_anthropic_event_end(
            writer
        ):
            return False
    elif kind[0] >= 0 and (
        anthropic_request_range_matches_literal(
            view, kind[0], kind[1], StringSlice("\"mcp_approval_request\"")
        )
        or anthropic_request_range_matches_literal(
            view, kind[0], kind[1], StringSlice("\"mcp_list_tools\"")
        )
    ):
        if not runtime_anthropic_event_start(writer, StringSlice("content_block_start")) or not runtime_anthropic_put_literal(
            writer, StringSlice("{\"type\":\"content_block_start\",\"index\":")
        ) or not runtime_anthropic_put_u64(writer, index) or not runtime_anthropic_put_literal(
            writer, StringSlice(",\"content_block\":")
        ) or not runtime_anthropic_put_view_range(writer, view, start, end) or not runtime_anthropic_put_literal(
            writer, StringSlice("}")
        ) or not runtime_anthropic_event_end(writer):
            return False
    else:
        if not runtime_anthropic_write_empty_content_block(
            writer, index, StringSlice("\"text\""), StringSlice("text")
        ) or not runtime_anthropic_write_text_delta(writer, view, index, text):
            return False
    return runtime_anthropic_write_content_block_stop(writer, index)


def runtime_anthropic_write_message_start(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    var id = anthropic_request_object_field(view, 0, Int64(view.len), StringSlice("\"id\""))
    var model = anthropic_request_object_field(view, 0, Int64(view.len), StringSlice("\"model\""))
    var usage = anthropic_request_object_field(view, 0, Int64(view.len), StringSlice("\"usage\""))
    var server_usage = InlineArray[Int64, 2](fill=-1)
    if usage[0] >= 0 and anthropic_request_byte(view, usage[0]) == 123:
        server_usage = anthropic_request_object_field(
            view, usage[0], usage[1], StringSlice("\"server_tool_use\"")
        )
    if not runtime_anthropic_event_start(writer, StringSlice("message_start")) or not runtime_anthropic_put_literal(
        writer, StringSlice("{\"type\":\"message_start\",\"message\":{\"id\":")
    ) or not runtime_anthropic_put_string_field_or_literal(
        writer, view, id, StringSlice("\"msg_prodex\"")
    ) or not runtime_anthropic_put_literal(
        writer, StringSlice(",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":")
    ) or not runtime_anthropic_put_string_field_or_literal(
        writer, view, model, StringSlice("\"claude-sonnet-4-6\"")
    ) or not runtime_anthropic_put_literal(
        writer, StringSlice(",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":0,\"output_tokens\":0,\"server_tool_use\":")
    ):
        return False
    if server_usage[0] >= 0:
        if not runtime_anthropic_put_view_range(writer, view, server_usage[0], server_usage[1]):
            return False
    elif not runtime_anthropic_put_literal(
        writer,
        StringSlice(
            "{\"web_search_requests\":0,\"web_fetch_requests\":0,\"code_execution_requests\":0,\"tool_search_requests\":0}",
        ),
    ):
        return False
    return runtime_anthropic_put_literal(writer, StringSlice("}}}")) and runtime_anthropic_event_end(
        writer
    )


def runtime_anthropic_write_message_sse(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if view.len < 2 or anthropic_request_byte(view, 0) != 123 or anthropic_request_byte(
        view, Int64(view.len) - 1
    ) != 125:
        return False
    if not runtime_anthropic_write_message_start(writer, view):
        return False
    var content = anthropic_request_object_field(view, 0, Int64(view.len), StringSlice("\"content\""))
    if content[0] >= 0 and anthropic_request_byte(view, content[0]) == 91 and anthropic_request_byte(
        view, content[1] - 1
    ) == 93:
        var index = anthropic_request_skip_ws(view, content[0] + 1, content[1] - 1)
        var block_index: UInt64 = 0
        while index < content[1] - 1:
            var block_end = anthropic_request_value_end(view, index, content[1] - 1, 0)
            if block_end < 0:
                return False
            if not runtime_anthropic_write_message_sse_block(
                writer, view, index, block_end, block_index
            ):
                return False
            block_index += 1
            index = anthropic_request_skip_ws(view, block_end, content[1] - 1)
            if index < content[1] - 1 and anthropic_request_byte(view, index) == 44:
                index = anthropic_request_skip_ws(view, index + 1, content[1] - 1)
            elif index != content[1] - 1:
                return False
    var stop_reason = anthropic_request_object_field(
        view, 0, Int64(view.len), StringSlice("\"stop_reason\"")
    )
    var stop_sequence = anthropic_request_object_field(
        view, 0, Int64(view.len), StringSlice("\"stop_sequence\"")
    )
    var usage = anthropic_request_object_field(view, 0, Int64(view.len), StringSlice("\"usage\""))
    if not runtime_anthropic_event_start(writer, StringSlice("message_delta")) or not runtime_anthropic_put_literal(
        writer, StringSlice("{\"type\":\"message_delta\",\"delta\":{\"stop_reason\":")
    ) or not runtime_anthropic_put_field_or_literal(writer, view, stop_reason, StringSlice("null")) or not runtime_anthropic_put_literal(
        writer, StringSlice(",\"stop_sequence\":")
    ) or not runtime_anthropic_put_field_or_literal(writer, view, stop_sequence, StringSlice("null")) or not runtime_anthropic_put_literal(
        writer, StringSlice("},\"usage\":")
    ) or not runtime_anthropic_put_field_or_literal(writer, view, usage, StringSlice("{}")) or not runtime_anthropic_put_literal(
        writer, StringSlice("}")
    ) or not runtime_anthropic_event_end(writer):
        return False
    return runtime_anthropic_event_start(writer, StringSlice("message_stop")) and runtime_anthropic_put_literal(
        writer, StringSlice("{\"type\":\"message_stop\"}")
    ) and runtime_anthropic_event_end(writer)


def runtime_anthropic_write_tool_use_block(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.id_present == 0 or input.name_present == 0 or input.input_present == 0:
        return False
    var kind = StringSlice("\"tool_use\"")
    if input.block_type_present != 0:
        if rich_view_matches_literal["mcp_tool_use"](input.block_type, False):
            kind = StringSlice("\"mcp_tool_use\"")
        else:
            kind = StringSlice("\"server_tool_use\"")
    if not runtime_anthropic_put_literal(writer, StringSlice("{\"type\":")) or not runtime_anthropic_put_literal(
        writer, kind
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"id\":")) or not runtime_anthropic_put_json_string(
        writer, input.id
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"name\":")) or not runtime_anthropic_put_json_string(
        writer, input.name
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"input\":")) or not runtime_anthropic_put_view(
        writer, input.input
    ):
        return False
    if input.server_name_present != 0:
        if not runtime_anthropic_put_literal(writer, StringSlice(",\"server_name\":")) or not runtime_anthropic_put_json_string(
            writer, input.server_name
        ):
            return False
    return runtime_anthropic_put_byte(writer, 125)


def runtime_anthropic_write_function_call(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.id_present == 0 or input.name_present == 0 or input.input_present == 0:
        return False
    return (
        runtime_anthropic_put_literal(writer, StringSlice("{\"type\":\"function_call\",\"call_id\":"))
        and runtime_anthropic_put_json_string(writer, input.id)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"name\":"))
        and runtime_anthropic_put_json_string(writer, input.name)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"arguments\":"))
        and runtime_anthropic_put_json_string_range(
            writer, input.input, 0, Int64(input.input.len)
        )
        and runtime_anthropic_put_byte(writer, 125)
    )


def runtime_anthropic_write_function_call_output(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.id_present == 0 or input.text_present == 0:
        return False
    if not runtime_anthropic_put_literal(writer, StringSlice("[{\"type\":\"function_call_output\",\"call_id\":")) or not runtime_anthropic_put_json_string(
        writer, input.id
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"output\":")) or not runtime_anthropic_put_json_string(
        writer, input.text
    ) or not runtime_anthropic_put_byte(writer, 125):
        return False
    if input.content_present != 0 and input.content.len > 2:
        if not runtime_anthropic_put_literal(writer, StringSlice(",{\"role\":\"user\",\"content\":")) or not runtime_anthropic_put_view(
            writer, input.content
        ) or not runtime_anthropic_put_byte(writer, 125):
            return False
    return runtime_anthropic_put_byte(writer, 93)


def runtime_anthropic_write_shell_tool_result(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.id_present == 0 or input.text_present == 0:
        return False
    var failed = input.flags & RUNTIME_ANTHROPIC_FLAG_ERROR != 0
    if not runtime_anthropic_put_literal(writer, StringSlice("[{\"type\":\"shell_call_output\",\"call_id\":")) or not runtime_anthropic_put_json_string(
        writer, input.id
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"output\":[{\"stdout\":")):
        return False
    if failed:
        if not runtime_anthropic_put_literal(writer, StringSlice("\"\",\"stderr\":")) or not runtime_anthropic_put_json_string(
            writer, input.text
        ):
            return False
    elif not runtime_anthropic_put_json_string(writer, input.text) or not runtime_anthropic_put_literal(
        writer, StringSlice(",\"stderr\":\"\"")
    ):
        return False
    if not runtime_anthropic_put_literal(
        writer, StringSlice(",\"outcome\":{\"type\":\"exit\",\"exit_code\":")
    ) or not runtime_anthropic_put_u64(writer, UInt64(1 if failed else 0)) or not runtime_anthropic_put_literal(
        writer, StringSlice("}}]")
    ):
        return False
    if input.max_output_length_present != 0:
        if not runtime_anthropic_put_literal(writer, StringSlice(",\"max_output_length\":")) or not runtime_anthropic_put_u64(
            writer, input.max_output_length
        ):
            return False
    if not runtime_anthropic_put_byte(writer, 125):
        return False
    if input.content_present != 0 and input.content.len > 2:
        if not runtime_anthropic_put_literal(writer, StringSlice(",{\"role\":\"user\",\"content\":")) or not runtime_anthropic_put_view(
            writer, input.content
        ) or not runtime_anthropic_put_byte(writer, 125):
            return False
    return runtime_anthropic_put_byte(writer, 93)


def runtime_anthropic_write_computer_tool_result(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.id_present == 0 or input.text_present == 0:
        return False
    return (
        runtime_anthropic_put_literal(writer, StringSlice("[{\"type\":\"computer_call_output\",\"call_id\":"))
        and runtime_anthropic_put_json_string(writer, input.id)
        and runtime_anthropic_put_literal(
            writer, StringSlice(",\"output\":{\"type\":\"computer_screenshot\",\"image_url\":")
        )
        )
        and runtime_anthropic_put_json_string(writer, input.text)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"detail\":\"original\"}}]")
    )


def runtime_anthropic_write_mcp_call_blocks(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.id_present == 0 or input.name_present == 0 or input.server_name_present == 0 or input.input_present == 0:
        return False
    if not runtime_anthropic_put_literal(writer, StringSlice("[{\"type\":\"mcp_tool_use\",\"id\":")) or not runtime_anthropic_put_json_string(
        writer, input.id
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"name\":")) or not runtime_anthropic_put_json_string(
        writer, input.name
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"server_name\":")) or not runtime_anthropic_put_json_string(
        writer, input.server_name
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"input\":")) or not runtime_anthropic_put_view(
        writer, input.input
    ) or not runtime_anthropic_put_literal(writer, StringSlice("}")):
        return False
    if input.output_present == 0 and input.text_present == 0:
        return runtime_anthropic_put_byte(writer, 93)
    if not runtime_anthropic_put_literal(writer, StringSlice(",{\"type\":\"mcp_tool_result\",\"tool_use_id\":")) or not runtime_anthropic_put_json_string(
        writer, input.id
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"is_error\":")):
        return False
    if input.flags & RUNTIME_ANTHROPIC_FLAG_ERROR != 0:
        if not runtime_anthropic_put_literal(writer, StringSlice("true")):
            return False
    elif not runtime_anthropic_put_literal(writer, StringSlice("false")):
        return False
    if not runtime_anthropic_put_literal(writer, StringSlice(",\"content\":[")):
        return False
    var first = True
    if input.output_present != 0:
        if not runtime_anthropic_put_literal(writer, StringSlice("{\"type\":\"text\",\"text\":")) or not runtime_anthropic_put_json_string(
            writer, input.output
        ) or not runtime_anthropic_put_byte(writer, 125):
            return False
        first = False
    if input.text_present != 0:
        if not first and not runtime_anthropic_put_byte(writer, 44):
            return False
        if not runtime_anthropic_put_literal(writer, StringSlice("{\"type\":\"text\",\"text\":")) or not runtime_anthropic_put_json_string(
            writer, input.text
        ) or not runtime_anthropic_put_byte(writer, 125):
            return False
    return runtime_anthropic_put_literal(writer, StringSlice("]}")) and runtime_anthropic_put_byte(
        writer, 93
    )


def runtime_anthropic_write_mcp_approval_block(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.id_present == 0 or input.name_present == 0 or input.server_name_present == 0 or input.text_present == 0 or input.input_present == 0:
        return False
    return (
        runtime_anthropic_put_literal(writer, StringSlice("{\"type\":\"mcp_approval_request\",\"id\":"))
        and runtime_anthropic_put_json_string(writer, input.id)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"name\":"))
        and runtime_anthropic_put_json_string(writer, input.name)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"server_name\":"))
        and runtime_anthropic_put_json_string(writer, input.server_name)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"server_label\":"))
        and runtime_anthropic_put_json_string(writer, input.server_name)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"arguments\":"))
        and runtime_anthropic_put_json_string(writer, input.text)
        and runtime_anthropic_put_literal(writer, StringSlice(",\"input\":"))
        and runtime_anthropic_put_view(writer, input.input)
        and runtime_anthropic_put_byte(writer, 125)
    )


def runtime_anthropic_write_mcp_list_tools_block(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.id_present == 0 or input.server_name_present == 0:
        return False
    if not runtime_anthropic_put_literal(writer, StringSlice("{\"type\":\"mcp_list_tools\",\"id\":")) or not runtime_anthropic_put_json_string(
        writer, input.id
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"server_name\":")) or not runtime_anthropic_put_json_string(
        writer, input.server_name
    ) or not runtime_anthropic_put_literal(writer, StringSlice(",\"server_label\":")) or not runtime_anthropic_put_json_string(
        writer, input.server_name
    ):
        return False
    if input.content_present != 0:
        if not runtime_anthropic_put_literal(writer, StringSlice(",\"tools\":")) or not runtime_anthropic_put_view(
            writer, input.content
        ):
            return False
    if input.text_present != 0:
        if not runtime_anthropic_put_literal(writer, StringSlice(",\"error\":")) or not runtime_anthropic_put_json_string(
            writer, input.text
        ):
            return False
    return runtime_anthropic_put_byte(writer, 125)


def runtime_anthropic_range_starts_literal(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var length = Int64(literal.byte_length())
    if start < 0 or end - start < length:
        return False
    var ptr = rich_view_ptr(view)
    var other = literal.unsafe_ptr()
    for index in range(length):
        if ptr[unsafe_offset=start + index] != other[unsafe_offset=index]:
            return False
    return True


def runtime_anthropic_trim_range(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=start)
    if start < 0 or end < start or end > Int64(view.len):
        result[0] = -1
        result[1] = -1
        return result^
    var part = ProdexRichStringView(view.ptr + UInt(start), UInt(end - start))
    var bounds = rich_trim_bounds(part)
    result[0] = start + bounds[0]
    result[1] = start + bounds[1]
    return result^


def runtime_anthropic_tool_result_query(
    view: ProdexRichStringView,
) -> InlineArray[Int64, 2]:
    var missing = InlineArray[Int64, 2](fill=-1)
    var whole = runtime_anthropic_trim_range(view, 0, Int64(view.len))
    var prefix = StringSlice("Web search results for query:")
    if not runtime_anthropic_range_starts_literal(
        view, whole[0], whole[1], prefix
    ):
        return missing^
    var start = whole[0] + Int64(prefix.byte_length())
    var line_end = start
    while line_end < whole[1] and anthropic_request_byte(view, line_end) != 10:
        line_end += 1
    var line = runtime_anthropic_trim_range(view, start, line_end)
    if line[0] >= line[1]:
        return missing^
    if anthropic_request_byte(view, line[0]) == 34:
        var quote = line[0] + 1
        while quote < line[1] and anthropic_request_byte(view, quote) != 34:
            quote += 1
        if quote < line[1]:
            line = runtime_anthropic_trim_range(view, line[0] + 1, quote)
            if line[0] < line[1]:
                return line^
            return missing^
    while line[0] < line[1] and anthropic_request_byte(view, line[0]) == 34:
        line[0] += 1
    while line[1] > line[0] and anthropic_request_byte(view, line[1] - 1) == 34:
        line[1] -= 1
    line = runtime_anthropic_trim_range(view, line[0], line[1])
    if line[0] >= line[1]:
        return missing^
    return line^


def runtime_anthropic_put_compact_summary(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    start: Int64,
) -> Bool:
    if not runtime_anthropic_put_byte(writer, 34):
        return False
    var cursor = start
    var wrote = False
    var pending_blank = False
    while cursor < Int64(view.len):
        var line_end = cursor
        while line_end < Int64(view.len) and anthropic_request_byte(view, line_end) != 10:
            line_end += 1
        var line = runtime_anthropic_trim_range(view, cursor, line_end)
        cursor = line_end + 1
        if line[0] == line[1]:
            if wrote:
                pending_blank = True
            continue
        if anthropic_request_range_matches_literal(
            view, line[0], line[1], StringSlice("No links found.")
        ) or runtime_anthropic_range_starts_literal(
            view, line[0], line[1], StringSlice("Link:")
        ):
            continue
        if anthropic_request_range_matches_literal(
            view, line[0], line[1], StringSlice("Sources:")
        ) or runtime_anthropic_range_starts_literal(
            view, line[0], line[1], StringSlice("REMINDER:")
        ) or runtime_anthropic_range_starts_literal(
            view, line[0], line[1], StringSlice("Kalau mau, saya bisa lanjutkan")
        ) or runtime_anthropic_range_starts_literal(
            view, line[0], line[1], StringSlice("If you'd like")
        ) or runtime_anthropic_range_starts_literal(
            view, line[0], line[1], StringSlice("If you want,")
        ):
            break
        if wrote and not runtime_anthropic_put_literal(writer, StringSlice("\\n")):
            return False
        if pending_blank and not runtime_anthropic_put_literal(writer, StringSlice("\\n")):
            return False
        if not runtime_anthropic_put_json_range_content(
            writer, view, line[0], line[1]
        ):
            return False
        wrote = True
        pending_blank = False
    return runtime_anthropic_put_byte(writer, 34)


def runtime_anthropic_write_tool_result_text_plan(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.text_present == 0 or input.index > UInt64(input.text.len):
        return False
    var query = InlineArray[Int64, 2](fill=0)
    if input.flags & 1 == 0:
        query = runtime_anthropic_tool_result_query(input.text)
        if query[0] < 0:
            return runtime_anthropic_put_literal(writer, StringSlice("null"))
    if not runtime_anthropic_put_literal(writer, StringSlice('{"query":')):
        return False
    if input.flags & 1 != 0:
        if not runtime_anthropic_put_literal(writer, StringSlice('""')):
            return False
    elif not runtime_anthropic_put_json_string_range(
        writer, input.text, query[0], query[1]
    ):
        return False
    return (
        runtime_anthropic_put_literal(writer, StringSlice(',"summary":'))
        and runtime_anthropic_put_compact_summary(
            writer, input.text, Int64(input.index)
        )
        and runtime_anthropic_put_byte(writer, 125)
    )


def runtime_anthropic_normalized_name_matches(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    expected: StringSlice,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var source = rich_view_ptr(view)
    var target = expected.unsafe_ptr()
    var matched: Int64 = 0
    for index in range(start, end):
        var value = source[unsafe_offset=index]
        var alphanumeric = (
            (value >= 48 and value <= 57)
            or (value >= 65 and value <= 90)
            or (value >= 97 and value <= 122)
        )
        if not alphanumeric:
            continue
        if value >= 65 and value <= 90:
            value += 32
        if matched >= Int64(expected.byte_length()) or value != target[unsafe_offset=matched]:
            return False
        matched += 1
    return matched == Int64(expected.byte_length())


def runtime_anthropic_normalized_json_name_matches(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    expected: StringSlice,
) -> Bool:
    return runtime_anthropic_json_string_valid(view, start, end) and runtime_anthropic_normalized_name_matches(
        view, start + 1, end - 1, expected
    )


def runtime_anthropic_server_tool_name_kind_raw(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Int64:
    if runtime_anthropic_normalized_name_matches(
        view, start, end, StringSlice("websearch")
    ):
        return 1
    if runtime_anthropic_normalized_name_matches(
        view, start, end, StringSlice("webfetch")
    ):
        return 2
    if runtime_anthropic_normalized_name_matches(
        view, start, end, StringSlice("codeexecution")
    ):
        return 3
    if runtime_anthropic_normalized_name_matches(
        view, start, end, StringSlice("bashcodeexecution")
    ):
        return 4
    if runtime_anthropic_normalized_name_matches(
        view, start, end, StringSlice("texteditorcodeexecution")
    ):
        return 5
    if runtime_anthropic_normalized_name_matches(
        view, start, end, StringSlice("toolsearchtoolregex")
    ):
        return 6
    if runtime_anthropic_normalized_name_matches(
        view, start, end, StringSlice("toolsearchtoolbm25")
    ):
        return 7
    return 0


def runtime_anthropic_server_tool_usage_kind(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Int64:
    var block_type = anthropic_request_object_field(
        view, start, end, StringSlice('"type"')
    )
    if block_type[0] < 0 or not (
        anthropic_request_range_matches_literal(
            view, block_type[0], block_type[1], StringSlice('"tool_use"')
        )
        or anthropic_request_range_matches_literal(
            view, block_type[0], block_type[1], StringSlice('"server_tool_use"')
        )
        or anthropic_request_range_matches_literal(
            view, block_type[0], block_type[1], StringSlice('"mcp_tool_use"')
        )
    ):
        return 0
    var name = anthropic_request_object_field(view, start, end, StringSlice('"name"'))
    return runtime_anthropic_server_tool_name_kind(view, name)


def runtime_anthropic_server_tool_name_kind(
    view: ProdexRichStringView,
    name: InlineArray[Int64, 2],
) -> Int64:
    if not runtime_anthropic_json_string_valid(view, name[0], name[1]):
        return 0
    var kind = runtime_anthropic_server_tool_name_kind_raw(
        view, name[0] + 1, name[1] - 1
    )
    if kind >= 3 and kind <= 5:
        return 3
    if kind >= 6:
        return 4
    return kind


def runtime_anthropic_is_tool_result_kind(
    view: ProdexRichStringView,
    kind: InlineArray[Int64, 2],
) -> Bool:
    return kind[0] >= 0 and (
        anthropic_request_range_matches_literal(
            view, kind[0], kind[1], StringSlice('"tool_result"')
        )
        or (
            kind[1] - kind[0] >= 13
            and anthropic_request_range_matches_literal(
                view, kind[1] - 13, kind[1], StringSlice('_tool_result"')
            )
        )
    )


def runtime_anthropic_write_server_usage_counts(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    web_search: UInt64,
    web_fetch: UInt64,
    code_execution: UInt64,
    tool_search: UInt64,
) -> Bool:
    return (
        runtime_anthropic_put_literal(
            writer, StringSlice('{"web_search_requests":')
        )
        and runtime_anthropic_put_u64(writer, web_search)
        and runtime_anthropic_put_literal(
            writer, StringSlice(',"web_fetch_requests":')
        )
        and runtime_anthropic_put_u64(writer, web_fetch)
        and runtime_anthropic_put_literal(
            writer, StringSlice(',"code_execution_requests":')
        )
        and runtime_anthropic_put_u64(writer, code_execution)
        and runtime_anthropic_put_literal(
            writer, StringSlice(',"tool_search_requests":')
        )
        and runtime_anthropic_put_u64(writer, tool_search)
        and runtime_anthropic_put_byte(writer, 125)
    )


def runtime_anthropic_write_server_tool_usage(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if view.len < 2:
        return False
    var kind = runtime_anthropic_server_tool_usage_kind(
        view, 0, Int64(view.len)
    )
    return runtime_anthropic_write_server_usage_counts(
        writer,
        UInt64(kind == 1),
        UInt64(kind == 2),
        UInt64(kind == 3),
        UInt64(kind == 4),
    )


def runtime_anthropic_write_carried_server_tool_usage(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    messages: ProdexRichStringView,
) -> Bool:
    if messages.len < 2 or anthropic_request_byte(messages, 0) != 91 or anthropic_request_byte(
        messages, Int64(messages.len) - 1
    ) != 93:
        return False
    var web_search: UInt64 = 0
    var web_fetch: UInt64 = 0
    var code_execution: UInt64 = 0
    var tool_search: UInt64 = 0
    var barrier = False
    var end = Int64(messages.len) - 1
    var cursor = anthropic_request_skip_ws(messages, 1, end)
    while cursor < end:
        var message_end = anthropic_request_value_end(messages, cursor, end, 0)
        if message_end < 0:
            return False
        var content = anthropic_request_object_field(
            messages, cursor, message_end, StringSlice('"content"')
        )
        var saw_chain = False
        var saw_unaccounted_tool_use = False
        var message_web_search: UInt64 = 0
        var message_web_fetch: UInt64 = 0
        var message_code_execution: UInt64 = 0
        var message_tool_search: UInt64 = 0
        if content[0] >= 0 and anthropic_request_byte(messages, content[0]) == 91:
            var content_end = content[1] - 1
            var block = anthropic_request_skip_ws(messages, content[0] + 1, content_end)
            while block < content_end:
                var block_end = anthropic_request_value_end(
                    messages, block, content_end, 0
                )
                if block_end < 0:
                    return False
                var usage_kind = runtime_anthropic_server_tool_usage_kind(
                    messages, block, block_end
                )
                if usage_kind == 1:
                    message_web_search += 1
                    saw_chain = True
                elif usage_kind == 2:
                    message_web_fetch += 1
                    saw_chain = True
                elif usage_kind == 3:
                    message_code_execution += 1
                    saw_chain = True
                elif usage_kind == 4:
                    message_tool_search += 1
                    saw_chain = True
                var block_type = anthropic_request_object_field(
                    messages, block, block_end, StringSlice('"type"')
                )
                if runtime_anthropic_is_tool_result_kind(messages, block_type):
                    saw_chain = True
                elif usage_kind == 0 and block_type[0] >= 0 and (
                    anthropic_request_range_matches_literal(
                        messages,
                        block_type[0],
                        block_type[1],
                        StringSlice('"tool_use"'),
                    )
                    or anthropic_request_range_matches_literal(
                        messages,
                        block_type[0],
                        block_type[1],
                        StringSlice('"server_tool_use"'),
                    )
                    or anthropic_request_range_matches_literal(
                        messages,
                        block_type[0],
                        block_type[1],
                        StringSlice('"mcp_tool_use"'),
                    )
                ):
                    saw_unaccounted_tool_use = True
                block = anthropic_request_skip_ws(messages, block_end, content_end)
                if block < content_end and anthropic_request_byte(messages, block) == 44:
                    block = anthropic_request_skip_ws(messages, block + 1, content_end)
                elif block != content_end:
                    return False
        if saw_chain:
            if barrier:
                web_search = 0
                web_fetch = 0
                code_execution = 0
                tool_search = 0
            web_search += message_web_search
            web_fetch += message_web_fetch
            code_execution += message_code_execution
            tool_search += message_tool_search
            barrier = False
        elif saw_unaccounted_tool_use:
            web_search = 0
            web_fetch = 0
            code_execution = 0
            tool_search = 0
            barrier = False
        else:
            barrier = True
        cursor = anthropic_request_skip_ws(messages, message_end, end)
        if cursor < end and anthropic_request_byte(messages, cursor) == 44:
            cursor = anthropic_request_skip_ws(messages, cursor + 1, end)
        elif cursor != end:
            return False
    return runtime_anthropic_write_server_usage_counts(
        writer, web_search, web_fetch, code_execution, tool_search
    )


def runtime_anthropic_write_canonical_server_tool_name(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    name: InlineArray[Int64, 2],
) -> Bool:
    var kind = runtime_anthropic_server_tool_name_kind(view, name)
    if kind == 1:
        return runtime_anthropic_put_literal(writer, StringSlice('"web_search"'))
    if kind == 2:
        return runtime_anthropic_put_literal(writer, StringSlice('"web_fetch"'))
    if kind == 3:
        if runtime_anthropic_normalized_json_name_matches(
            view, name[0], name[1], StringSlice("bashcodeexecution")
        ):
            return runtime_anthropic_put_literal(
                writer, StringSlice('"bash_code_execution"')
            )
        if runtime_anthropic_normalized_json_name_matches(
            view, name[0], name[1], StringSlice("texteditorcodeexecution")
        ):
            return runtime_anthropic_put_literal(
                writer, StringSlice('"text_editor_code_execution"')
            )
        return runtime_anthropic_put_literal(writer, StringSlice('"code_execution"'))
    if kind == 4:
        if runtime_anthropic_normalized_json_name_matches(
            view, name[0], name[1], StringSlice("toolsearchtoolbm25")
        ):
            return runtime_anthropic_put_literal(
                writer, StringSlice('"tool_search_tool_bm25"')
            )
        return runtime_anthropic_put_literal(
            writer, StringSlice('"tool_search_tool_regex"')
        )
    return runtime_anthropic_put_view_range(writer, view, name[0], name[1])


def runtime_anthropic_put_server_tool_registration(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    block_type: InlineArray[Int64, 2],
    name: InlineArray[Int64, 2],
    first: Pointer[mut=True, Bool, _],
) -> Bool:
    if not runtime_anthropic_json_string_valid(view, name[0], name[1]):
        return True
    if not first[] and not runtime_anthropic_put_byte(writer, 44):
        return False
    first[] = False
    return (
        runtime_anthropic_put_literal(writer, StringSlice('{"tool_name":'))
        and runtime_anthropic_put_view_range(writer, view, name[0], name[1])
        and runtime_anthropic_put_literal(writer, StringSlice(',"response_name":'))
        and runtime_anthropic_write_canonical_server_tool_name(writer, view, name)
        and runtime_anthropic_put_literal(writer, StringSlice(',"block_type":'))
        and runtime_anthropic_put_view_range(
            writer, view, block_type[0], block_type[1]
        )
        and runtime_anthropic_put_byte(writer, 125)
    )


def runtime_anthropic_write_server_tool_registrations(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    messages: ProdexRichStringView,
) -> Bool:
    if messages.len < 2 or anthropic_request_byte(messages, 0) != 91 or anthropic_request_byte(
        messages, Int64(messages.len) - 1
    ) != 93 or not runtime_anthropic_put_byte(writer, 91):
        return False
    var first = True
    var first_ptr = Pointer(to=first)
    var end = Int64(messages.len) - 1
    var cursor = anthropic_request_skip_ws(messages, 1, end)
    while cursor < end:
        var message_end = anthropic_request_value_end(messages, cursor, end, 0)
        if message_end < 0:
            return False
        var content = anthropic_request_object_field(
            messages, cursor, message_end, StringSlice('"content"')
        )
        if content[0] >= 0 and anthropic_request_byte(messages, content[0]) == 91:
            var content_end = content[1] - 1
            var block = anthropic_request_skip_ws(messages, content[0] + 1, content_end)
            while block < content_end:
                var block_end = anthropic_request_value_end(
                    messages, block, content_end, 0
                )
                if block_end < 0:
                    return False
                var block_type = anthropic_request_object_field(
                    messages, block, block_end, StringSlice('"type"')
                )
                if (
                    anthropic_request_range_matches_literal(
                        messages,
                        block_type[0],
                        block_type[1],
                        StringSlice('"server_tool_use"'),
                    )
                    or anthropic_request_range_matches_literal(
                        messages,
                        block_type[0],
                        block_type[1],
                        StringSlice('"mcp_tool_use"'),
                    )
                ):
                    var name = anthropic_request_object_field(
                        messages, block, block_end, StringSlice('"name"')
                    )
                    if not runtime_anthropic_put_server_tool_registration(
                        writer, messages, block_type, name, first_ptr
                    ):
                        return False
                block = anthropic_request_skip_ws(messages, block_end, content_end)
                if block < content_end and anthropic_request_byte(messages, block) == 44:
                    block = anthropic_request_skip_ws(messages, block + 1, content_end)
                elif block != content_end:
                    return False
        cursor = anthropic_request_skip_ws(messages, message_end, end)
        if cursor < end and anthropic_request_byte(messages, cursor) == 44:
            cursor = anthropic_request_skip_ws(messages, cursor + 1, end)
        elif cursor != end:
            return False
    return runtime_anthropic_put_byte(writer, 93)


def runtime_anthropic_write_message_has_tool_chain(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    message: ProdexRichStringView,
) -> Bool:
    if message.len < 2:
        return False
    var content = anthropic_request_object_field(
        message, 0, Int64(message.len), StringSlice('"content"')
    )
    if content[0] < 0 or anthropic_request_byte(message, content[0]) != 91:
        return runtime_anthropic_put_literal(writer, StringSlice("false"))
    var end = content[1] - 1
    var block = anthropic_request_skip_ws(message, content[0] + 1, end)
    while block < end:
        var block_end = anthropic_request_value_end(message, block, end, 0)
        if block_end < 0:
            return False
        var block_type = anthropic_request_object_field(
            message, block, block_end, StringSlice('"type"')
        )
        if runtime_anthropic_is_tool_result_kind(message, block_type) or (
            block_type[0] >= 0
            and (
                anthropic_request_range_matches_literal(
                    message, block_type[0], block_type[1], StringSlice('"tool_use"')
                )
                or anthropic_request_range_matches_literal(
                    message,
                    block_type[0],
                    block_type[1],
                    StringSlice('"server_tool_use"'),
                )
                or anthropic_request_range_matches_literal(
                    message,
                    block_type[0],
                    block_type[1],
                    StringSlice('"mcp_tool_use"'),
                )
            )
        ):
            return runtime_anthropic_put_literal(writer, StringSlice("true"))
        block = anthropic_request_skip_ws(message, block_end, end)
        if block < end and anthropic_request_byte(message, block) == 44:
            block = anthropic_request_skip_ws(message, block + 1, end)
        elif block != end:
            return False
    return runtime_anthropic_put_literal(writer, StringSlice("false"))


def runtime_anthropic_write_server_tool_name_kind(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    name: ProdexRichStringView,
) -> Bool:
    return runtime_anthropic_put_u64(
        writer,
        UInt64(
            runtime_anthropic_server_tool_name_kind_raw(
                name, 0, Int64(name.len)
            )
        ),
    )


def runtime_anthropic_coordinate_ranges(
    view: ProdexRichStringView,
) -> InlineArray[Int64, 4]:
    var result = InlineArray[Int64, 4](fill=-1)
    if view.len < 2 or anthropic_request_byte(view, 0) != 91:
        return result^
    var end = Int64(view.len) - 1
    if anthropic_request_byte(view, end) != 93:
        return result^
    var first = anthropic_request_skip_ws(view, 1, end)
    var first_end = anthropic_request_value_end(view, first, end, 0)
    if first_end < 0:
        return result^
    var comma = anthropic_request_skip_ws(view, first_end, end)
    if comma >= end or anthropic_request_byte(view, comma) != 44:
        return result^
    var second = anthropic_request_skip_ws(view, comma + 1, end)
    var second_end = anthropic_request_value_end(view, second, end, 0)
    if second_end < 0:
        return result^
    result[0] = first
    result[1] = first_end
    result[2] = second
    result[3] = second_end
    return result^


def runtime_anthropic_put_uppercase_key(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if not runtime_anthropic_put_byte(writer, 34):
        return False
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        var value = ptr[unsafe_offset=index]
        if value >= 97 and value <= 122:
            value -= 32
        if value == 34 or value == 92:
            if not runtime_anthropic_put_byte(writer, 92):
                return False
        if not runtime_anthropic_put_byte(writer, value):
            return False
    return runtime_anthropic_put_byte(writer, 34)


def runtime_anthropic_write_keypress_keys(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    keys: ProdexRichStringView,
) -> Bool:
    if not runtime_anthropic_put_byte(writer, 91):
        return False
    var start: Int64 = 0
    var first = True
    while start <= Int64(keys.len):
        var end = start
        while end < Int64(keys.len) and anthropic_request_byte(keys, end) != 43:
            end += 1
        var part = runtime_anthropic_trim_range(keys, start, end)
        if part[0] < part[1]:
            if not first and not runtime_anthropic_put_byte(writer, 44):
                return False
            first = False
            if not runtime_anthropic_put_uppercase_key(
                writer, keys, part[0], part[1]
            ):
                return False
        if end >= Int64(keys.len):
            break
        start = end + 1
    return not first and runtime_anthropic_put_byte(writer, 93)


def runtime_anthropic_keypress_has_key(keys: ProdexRichStringView) -> Bool:
    var start: Int64 = 0
    while start <= Int64(keys.len):
        var end = start
        while end < Int64(keys.len) and anthropic_request_byte(keys, end) != 43:
            end += 1
        var part = runtime_anthropic_trim_range(keys, start, end)
        if part[0] < part[1]:
            return True
        if end >= Int64(keys.len):
            break
        start = end + 1
    return False


def runtime_anthropic_write_computer_action(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.block_type_present == 0:
        return runtime_anthropic_put_literal(writer, StringSlice("null"))
    if rich_view_matches_literal["screenshot"](input.block_type, False):
        return runtime_anthropic_put_literal(writer, StringSlice('{"type":"screenshot"}'))
    if rich_view_matches_literal["wait"](input.block_type, False):
        return runtime_anthropic_put_literal(writer, StringSlice('{"type":"wait"}'))
    if rich_view_matches_literal["type"](input.block_type, False):
        if input.text_present == 0:
            return runtime_anthropic_put_literal(writer, StringSlice("null"))
        return (
            runtime_anthropic_put_literal(writer, StringSlice('{"type":"type","text":'))
            and runtime_anthropic_put_json_string(writer, input.text)
            and runtime_anthropic_put_byte(writer, 125)
        )
    if rich_view_matches_literal["key"](input.block_type, False):
        if input.name_present == 0 or not runtime_anthropic_keypress_has_key(input.name):
            return runtime_anthropic_put_literal(writer, StringSlice("null"))
        if not runtime_anthropic_put_literal(
            writer, StringSlice('{"type":"keypress","keys":')
        ):
            return False
        if not runtime_anthropic_write_keypress_keys(writer, input.name):
            return False
        return runtime_anthropic_put_byte(writer, 125)
    var coordinates = runtime_anthropic_coordinate_ranges(input.input)
    if input.input_present == 0 or coordinates[0] < 0:
        return runtime_anthropic_put_literal(writer, StringSlice("null"))
    var kind = StringSlice("")
    var button = StringSlice("")
    if rich_view_matches_literal["left_click"](input.block_type, False):
        kind = StringSlice("click")
        button = StringSlice("left")
    elif rich_view_matches_literal["right_click"](input.block_type, False):
        kind = StringSlice("click")
        button = StringSlice("right")
    elif rich_view_matches_literal["middle_click"](input.block_type, False):
        kind = StringSlice("click")
        button = StringSlice("middle")
    elif rich_view_matches_literal["double_click"](input.block_type, False):
        kind = StringSlice("double_click")
    elif rich_view_matches_literal["mouse_move"](input.block_type, False):
        kind = StringSlice("move")
    else:
        return runtime_anthropic_put_literal(writer, StringSlice("null"))
    if not runtime_anthropic_put_literal(writer, StringSlice('{"type":"')) or not runtime_anthropic_put_literal(
        writer, kind
    ) or not runtime_anthropic_put_byte(writer, 34):
        return False
    if button.byte_length() > 0 and not (
        runtime_anthropic_put_literal(writer, StringSlice(',"button":"'))
        and runtime_anthropic_put_literal(writer, button)
        and runtime_anthropic_put_byte(writer, 34)
    ):
        return False
    return (
        runtime_anthropic_put_literal(writer, StringSlice(',"x":'))
        and runtime_anthropic_put_view_range(
            writer, input.input, coordinates[0], coordinates[1]
        )
        and runtime_anthropic_put_literal(writer, StringSlice(',"y":'))
        and runtime_anthropic_put_view_range(
            writer, input.input, coordinates[2], coordinates[3]
        )
        and runtime_anthropic_put_byte(writer, 125)
    )


def runtime_anthropic_put_lowercase_string(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if not runtime_anthropic_put_byte(writer, 34):
        return False
    var ptr = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        var value = ptr[unsafe_offset=index]
        if value >= 65 and value <= 90:
            value += 32
        if value == 34 or value == 92:
            if not runtime_anthropic_put_byte(writer, 92):
                return False
        if not runtime_anthropic_put_byte(writer, value):
            return False
    return runtime_anthropic_put_byte(writer, 34)


def runtime_anthropic_write_computer_tool_input(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.block_type_present == 0:
        return runtime_anthropic_put_literal(writer, StringSlice("null"))
    if rich_view_matches_literal["screenshot"](input.block_type, False):
        return runtime_anthropic_put_literal(writer, StringSlice('{"action":"screenshot"}'))
    if rich_view_matches_literal["wait"](input.block_type, False):
        return runtime_anthropic_put_literal(writer, StringSlice('{"action":"wait"}'))
    if rich_view_matches_literal["type"](input.block_type, False):
        if input.text_present == 0:
            return runtime_anthropic_put_literal(writer, StringSlice("null"))
        return (
            runtime_anthropic_put_literal(writer, StringSlice('{"action":"type","text":'))
            and runtime_anthropic_put_json_string(writer, input.text)
            and runtime_anthropic_put_byte(writer, 125)
        )
    if rich_view_matches_literal["keypress"](input.block_type, False):
        if input.output_present == 0:
            return runtime_anthropic_put_literal(writer, StringSlice("null"))
        return (
            runtime_anthropic_put_literal(writer, StringSlice('{"action":"key","key":'))
            and runtime_anthropic_put_lowercase_string(writer, input.output)
            and runtime_anthropic_put_byte(writer, 125)
        )
    var coordinates = runtime_anthropic_coordinate_ranges(input.input)
    if input.input_present == 0 or coordinates[0] < 0:
        return runtime_anthropic_put_literal(writer, StringSlice("null"))
    var action = StringSlice("")
    if rich_view_matches_literal["click"](input.block_type, False):
        if input.name_present == 0 or rich_view_matches_literal["left"](input.name, False):
            action = StringSlice("left_click")
        elif rich_view_matches_literal["right"](input.name, False):
            action = StringSlice("right_click")
        elif rich_view_matches_literal["middle"](input.name, False):
            action = StringSlice("middle_click")
        else:
            return runtime_anthropic_put_literal(writer, StringSlice("null"))
    elif rich_view_matches_literal["double_click"](input.block_type, False):
        action = StringSlice("double_click")
    elif rich_view_matches_literal["move"](input.block_type, False):
        action = StringSlice("mouse_move")
    else:
        return runtime_anthropic_put_literal(writer, StringSlice("null"))
    return (
        runtime_anthropic_put_literal(writer, StringSlice('{"action":"'))
        and runtime_anthropic_put_literal(writer, action)
        and runtime_anthropic_put_literal(writer, StringSlice('","coordinate":['))
        and runtime_anthropic_put_view_range(
            writer, input.input, coordinates[0], coordinates[1]
        )
        and runtime_anthropic_put_byte(writer, 44)
        and runtime_anthropic_put_view_range(
            writer, input.input, coordinates[2], coordinates[3]
        )
        and runtime_anthropic_put_literal(writer, StringSlice("]}"))
    )


def runtime_anthropic_tool_json_root(
    input: ProdexRuntimeAnthropicKernelInput,
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    if input.input_present == 0 or input.input.len == 0:
        return result^
    var start = anthropic_request_skip_ws(input.input, 0, Int64(input.input.len))
    var end = anthropic_request_value_end(input.input, start, Int64(input.input.len), 0)
    if (
        start >= 0
        and end > start
        and anthropic_request_skip_ws(input.input, end, Int64(input.input.len)) == Int64(input.input.len)
        and anthropic_request_byte(input.input, start) == 123
    ):
        result[0] = start
        result[1] = end
    return result^


def runtime_anthropic_tool_json_field(
    input: ProdexRuntimeAnthropicKernelInput,
    key: StringSlice,
) -> InlineArray[Int64, 2]:
    var root = runtime_anthropic_tool_json_root(input)
    if root[0] < 0:
        return InlineArray[Int64, 2](fill=-1)^
    return anthropic_request_object_field(input.input, root[0], root[1], key)


def runtime_anthropic_tool_json_true(
    input: ProdexRuntimeAnthropicKernelInput,
    key: StringSlice,
) -> Bool:
    var field = runtime_anthropic_tool_json_field(input, key)
    return (
        field[0] >= 0
        and anthropic_request_range_matches_literal(
            input.input, field[0], field[1], StringSlice("true")
        )
    )


def runtime_anthropic_write_client_tool_description(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.name_present == 0:
        return False
    if rich_view_matches_literal["bash"](input.name, False):
        return runtime_anthropic_put_literal(
            writer, StringSlice("Run shell commands on the local machine.")
        )
    if rich_view_matches_literal["memory"](input.name, False):
        return runtime_anthropic_put_literal(
            writer,
            StringSlice(
                "Store and retrieve information across conversations using persistent memory files."
            ),
        )
    if rich_view_matches_literal["str_replace_based_edit_tool"](input.name, False):
        if not runtime_anthropic_put_literal(
            writer, StringSlice("Edit local text files with string replacement operations.")
        ):
            return False
        if input.index >= 20250728:
            var max_characters = runtime_anthropic_tool_json_field(
                input, StringSlice("\"max_characters\"")
            )
            if max_characters[0] >= 0:
                return (
                    runtime_anthropic_put_literal(
                        writer, StringSlice(" View results may be truncated to ")
                    )
                    and runtime_anthropic_put_view_range(
                        writer, input.input, max_characters[0], max_characters[1]
                    )
                    and runtime_anthropic_put_literal(writer, StringSlice(" characters."))
                )
        return True
    if rich_view_matches_literal["computer"](input.name, False):
        if not runtime_anthropic_put_literal(
            writer, StringSlice("Interact with the graphical computer display.")
        ):
            return False
        var width = runtime_anthropic_tool_json_field(
            input, StringSlice("\"display_width_px\"")
        )
        var height = runtime_anthropic_tool_json_field(
            input, StringSlice("\"display_height_px\"")
        )
        if width[0] >= 0 and height[0] >= 0:
            if not (
                runtime_anthropic_put_literal(writer, StringSlice(" Display resolution: "))
                and runtime_anthropic_put_view_range(
                    writer, input.input, width[0], width[1]
                )
                and runtime_anthropic_put_byte(writer, 120)
                and runtime_anthropic_put_view_range(
                    writer, input.input, height[0], height[1]
                )
                and runtime_anthropic_put_literal(writer, StringSlice(" pixels."))
            ):
                return False
        var display_number = runtime_anthropic_tool_json_field(
            input, StringSlice("\"display_number\"")
        )
        if display_number[0] >= 0:
            if not (
                runtime_anthropic_put_literal(writer, StringSlice(" Display number: "))
                and runtime_anthropic_put_view_range(
                    writer, input.input, display_number[0], display_number[1]
                )
                and runtime_anthropic_put_byte(writer, 46)
            ):
                return False
        if input.index >= 20251124 and runtime_anthropic_tool_json_true(
            input, StringSlice("\"enable_zoom\"")
        ):
            return runtime_anthropic_put_literal(writer, StringSlice(" Zoom action enabled."))
        return True
    return False


def runtime_anthropic_write_text_editor_schema(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _], version: UInt64
) -> Bool:
    if not runtime_anthropic_put_literal(
        writer,
        StringSlice(
            "{\"type\":\"object\",\"properties\":{\"command\":{\"type\":\"string\",\"enum\":[\"view\",\"create\",\"str_replace\",\"insert\""
        ),
    ):
        return False
    if version < 20250429 and not runtime_anthropic_put_literal(
        writer, StringSlice(",\"undo_edit\"")
    ):
        return False
    return runtime_anthropic_put_literal(
        writer,
        StringSlice(
            "],\"description\":\"Text editor action to perform.\"},\"path\":{\"type\":\"string\",\"description\":\"Path of the file to inspect or edit.\"},\"file_text\":{\"type\":\"string\",\"description\":\"File contents used when creating a new file.\"},\"old_str\":{\"type\":\"string\",\"description\":\"Existing text to replace.\"},\"new_str\":{\"type\":\"string\",\"description\":\"Replacement text.\"},\"insert_line\":{\"type\":\"integer\",\"description\":\"Line number where new text should be inserted.\"},\"insert_text\":{\"type\":\"string\",\"description\":\"Text to insert at the requested line.\"},\"view_range\":{\"type\":\"array\",\"items\":{\"type\":\"integer\"},\"minItems\":2,\"maxItems\":2,\"description\":\"Optional inclusive start and end line numbers for view.\"}},\"additionalProperties\":true}"
        ),
    )


def runtime_anthropic_write_memory_schema(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _]
) -> Bool:
    return runtime_anthropic_put_literal(
        writer,
        StringSlice(
            "{\"type\":\"object\",\"properties\":{\"command\":{\"type\":\"string\",\"enum\":[\"view\",\"create\",\"str_replace\",\"insert\",\"delete\",\"rename\"],\"description\":\"Memory operation to perform.\"},\"path\":{\"type\":\"string\",\"description\":\"Path within /memories to inspect or modify.\"},\"view_range\":{\"type\":\"array\",\"items\":{\"type\":\"integer\"},\"minItems\":2,\"maxItems\":2,\"description\":\"Optional inclusive start and end line numbers for view.\"},\"file_text\":{\"type\":\"string\",\"description\":\"File contents used when creating a new memory file.\"},\"old_str\":{\"type\":\"string\",\"description\":\"Existing text to replace in a memory file.\"},\"new_str\":{\"type\":\"string\",\"description\":\"Replacement text for a memory file edit.\"},\"insert_line\":{\"type\":\"integer\",\"description\":\"Line number where new text should be inserted.\"},\"insert_text\":{\"type\":\"string\",\"description\":\"Text to insert at the requested line.\"},\"old_path\":{\"type\":\"string\",\"description\":\"Existing memory path to rename or move.\"},\"new_path\":{\"type\":\"string\",\"description\":\"Destination memory path for a rename or move.\"}},\"additionalProperties\":true}"
        ),
    )


def runtime_anthropic_write_bash_schema(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _]
) -> Bool:
    return runtime_anthropic_put_literal(
        writer,
        StringSlice(
            "{\"type\":\"object\",\"properties\":{\"command\":{\"type\":\"string\",\"description\":\"Shell command to execute.\"},\"restart\":{\"type\":\"boolean\",\"description\":\"Restart the shell session before executing the command.\"},\"timeout_ms\":{\"type\":\"integer\",\"minimum\":1,\"description\":\"Maximum time to wait for the command to finish.\"},\"max_output_length\":{\"type\":\"integer\",\"minimum\":1,\"description\":\"Maximum number of output characters to return.\"}},\"additionalProperties\":true}"
        ),
    )


def runtime_anthropic_write_computer_schema(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if not runtime_anthropic_put_literal(
        writer,
        StringSlice(
            "{\"type\":\"object\",\"properties\":{\"action\":{\"type\":\"string\",\"enum\":[\"screenshot\",\"left_click\",\"type\",\"key\",\"mouse_move\""
        ),
    ):
        return False
    if input.index >= 20250124 and not runtime_anthropic_put_literal(
        writer,
        StringSlice(
            ",\"scroll\",\"left_click_drag\",\"right_click\",\"middle_click\",\"double_click\",\"triple_click\",\"left_mouse_down\",\"left_mouse_up\",\"hold_key\",\"wait\""
        ),
    ):
        return False
    if input.index >= 20251124 and runtime_anthropic_tool_json_true(
        input, StringSlice("\"enable_zoom\"")
    ) and not runtime_anthropic_put_literal(writer, StringSlice(",\"zoom\"")):
        return False
    return runtime_anthropic_put_literal(
        writer,
        StringSlice(
            "],\"description\":\"Computer action to perform.\"},\"coordinate\":{\"type\":\"array\",\"items\":{\"type\":\"integer\"},\"minItems\":2,\"maxItems\":2,\"description\":\"Screen coordinate pair in pixels.\"},\"text\":{\"type\":\"string\",\"description\":\"Text to type into the active application.\"},\"key\":{\"type\":\"string\",\"description\":\"Single key or key chord to press.\"},\"keys\":{\"type\":\"array\",\"items\":{\"type\":\"string\"},\"description\":\"Keys to hold while performing another action.\"},\"scroll_direction\":{\"type\":\"string\",\"enum\":[\"up\",\"down\",\"left\",\"right\"],\"description\":\"Scroll direction.\"},\"scroll_amount\":{\"type\":\"integer\",\"description\":\"Distance to scroll.\"},\"duration_ms\":{\"type\":\"integer\",\"minimum\":0,\"description\":\"Optional wait duration in milliseconds.\"},\"region\":{\"type\":\"array\",\"items\":{\"type\":\"integer\"},\"minItems\":4,\"maxItems\":4,\"description\":\"Optional screen region [x1, y1, x2, y2] for zoom actions.\"}},\"additionalProperties\":true}"
        ),
    )


def runtime_anthropic_write_client_tool_schema(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.name_present == 0:
        return False
    if rich_view_matches_literal["bash"](input.name, False):
        return runtime_anthropic_write_bash_schema(writer)
    if rich_view_matches_literal["memory"](input.name, False):
        return runtime_anthropic_write_memory_schema(writer)
    if rich_view_matches_literal["str_replace_based_edit_tool"](input.name, False):
        return runtime_anthropic_write_text_editor_schema(writer, input.index)
    if rich_view_matches_literal["computer"](input.name, False):
        return runtime_anthropic_write_computer_schema(writer, input)
    return False



def runtime_anthropic_ascii_lower_range(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        var value = ptr[unsafe_offset=index]
        if value >= 65 and value <= 90:
            value += 32
        if not runtime_anthropic_put_byte(writer, value):
            return False
    return True


def runtime_anthropic_trimmed_view(view: ProdexRichStringView) -> ProdexRichStringView:
    var bounds = rich_trim_bounds(view)
    return ProdexRichStringView(
        view.ptr + UInt(bounds[0]), UInt(bounds[1] - bounds[0])
    )


def runtime_anthropic_versioned_type_base_range(
    view: ProdexRichStringView,
) -> InlineArray[Int64, 3]:
    var result = InlineArray[Int64, 3](fill=-1)
    var bounds = rich_trim_bounds(view)
    var start = bounds[0]
    var end = bounds[1]
    result[0] = start
    result[1] = end
    if end - start < 9:
        return result^
    var ptr = rich_view_ptr(view)
    var underscore: Int64 = -1
    for index in range(start, end):
        if ptr[unsafe_offset=index] == 95:
            underscore = index
    if underscore < start or end - underscore - 1 != 8:
        return result^
    for index in range(underscore + 1, end):
        var value = ptr[unsafe_offset=index]
        if value < 48 or value > 57:
            return result^
    result[1] = underscore
    result[2] = underscore + 1
    return result^


def runtime_anthropic_type_kind(view: ProdexRichStringView) -> Int64:
    var base = runtime_anthropic_versioned_type_base_range(view)
    if base[0] < 0:
        return 0
    var normalized = ProdexRichStringView(
        view.ptr + UInt(base[0]), UInt(base[1] - base[0])
    )
    if rich_view_matches_literal["bash"](normalized, True):
        return 1
    if rich_view_matches_literal["computer"](normalized, True):
        return 2
    if rich_view_matches_literal["memory"](normalized, True):
        return 3
    if rich_view_matches_literal["text_editor"](normalized, True):
        return 4
    if rich_view_matches_literal["web_search"](normalized, True):
        return 5
    if rich_view_matches_literal["web_fetch"](normalized, True):
        return 6
    if rich_view_matches_literal["code_execution"](normalized, True):
        return 7
    if rich_view_matches_literal["tool_search_tool_regex"](normalized, True):
        return 8
    if rich_view_matches_literal["tool_search_tool_bm25"](normalized, True):
        return 9
    return 0


def runtime_anthropic_write_unversioned_tool_type(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    var base = runtime_anthropic_versioned_type_base_range(view)
    return base[0] >= 0 and runtime_anthropic_ascii_lower_range(
        writer, view, base[0], base[1]
    )


def runtime_anthropic_write_tool_version(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    var base = runtime_anthropic_versioned_type_base_range(view)
    if base[2] < 0:
        return True
    var bounds = rich_trim_bounds(view)
    return runtime_anthropic_put_view_range(writer, view, base[2], bounds[1])


def runtime_anthropic_write_client_tool_kind(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    var kind = runtime_anthropic_type_kind(view)
    if kind >= 1 and kind <= 4:
        return runtime_anthropic_put_u64(writer, UInt64(kind))
    return runtime_anthropic_put_byte(writer, 48)


def runtime_anthropic_write_server_tool_type_kind(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    var kind = runtime_anthropic_type_kind(view)
    if kind >= 5:
        return runtime_anthropic_put_u64(writer, UInt64(kind - 4))
    return runtime_anthropic_put_byte(writer, 48)


def runtime_anthropic_write_client_tool_name_kind(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    var trimmed = runtime_anthropic_trimmed_view(view)
    if rich_view_matches_literal["bash"](trimmed, True):
        return runtime_anthropic_put_byte(writer, 49)
    if rich_view_matches_literal["computer"](trimmed, True):
        return runtime_anthropic_put_byte(writer, 50)
    if rich_view_matches_literal["memory"](trimmed, True):
        return runtime_anthropic_put_byte(writer, 51)
    if rich_view_matches_literal["str_replace_based_edit_tool"](trimmed, True):
        return runtime_anthropic_put_byte(writer, 52)
    return runtime_anthropic_write_client_tool_kind(writer, view)


def runtime_anthropic_view_ends_literal(
    view: ProdexRichStringView, literal: StringSlice
) -> Bool:
    if Int64(view.len) < Int64(literal.byte_length()):
        return False
    var ptr = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    var start = Int64(view.len) - Int64(literal.byte_length())
    for index in range(Int64(literal.byte_length())):
        if ptr[unsafe_offset=start + index] != expected[unsafe_offset=index]:
            return False
    return True


def runtime_anthropic_write_reasoning_effort(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    effort: ProdexRichStringView,
    supports_xhigh: Bool,
) -> Bool:
    var trimmed = runtime_anthropic_trimmed_view(effort)
    if rich_view_matches_literal["low"](trimmed, True):
        return runtime_anthropic_put_literal(writer, StringSlice("low"))
    if rich_view_matches_literal["medium"](trimmed, True):
        return runtime_anthropic_put_literal(writer, StringSlice("medium"))
    if rich_view_matches_literal["high"](trimmed, True):
        return runtime_anthropic_put_literal(writer, StringSlice("high"))
    if rich_view_matches_literal["max"](trimmed, True):
        if supports_xhigh:
            return runtime_anthropic_put_literal(writer, StringSlice("xhigh"))
        return runtime_anthropic_put_literal(writer, StringSlice("high"))
    return True

def runtime_anthropic_write_operation(
    writer: Pointer[mut=True, RuntimeAnthropicKernelWriter, _],
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    if input.operation == RUNTIME_ANTHROPIC_SSE_EVENT:
        if input.text_present == 0 or input.message_present == 0:
            return False
        return (
            runtime_anthropic_put_literal(writer, StringSlice("event: "))
            and runtime_anthropic_put_view(writer, input.text)
            and runtime_anthropic_put_literal(writer, StringSlice("\ndata: "))
            and runtime_anthropic_put_view(writer, input.message)
            and runtime_anthropic_event_end(writer)
        )
    if input.operation == RUNTIME_ANTHROPIC_MESSAGE_SSE:
        if input.message_present == 0:
            return False
        return runtime_anthropic_write_message_sse(writer, input.message)
    if input.operation == RUNTIME_ANTHROPIC_RESPONSE_MESSAGE:
        return runtime_anthropic_write_response_message(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_USAGE:
        return runtime_anthropic_write_usage(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_INPUT_TEXT:
        if input.text_present == 0:
            return False
        return runtime_anthropic_put_literal(writer, StringSlice("{\"type\":\"input_text\",\"text\":")) and runtime_anthropic_put_json_string(
            writer, input.text
        ) and runtime_anthropic_put_byte(writer, 125)
    if input.operation == RUNTIME_ANTHROPIC_IMAGE_PART:
        if input.text_present == 0:
            return False
        return runtime_anthropic_put_literal(writer, StringSlice("{\"type\":\"input_image\",\"image_url\":")) and runtime_anthropic_put_json_string(
            writer, input.text
        ) and runtime_anthropic_put_byte(writer, 125)
    if input.operation == RUNTIME_ANTHROPIC_FUNCTION_CALL:
        return runtime_anthropic_write_function_call(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_FUNCTION_CALL_OUTPUT:
        return runtime_anthropic_write_function_call_output(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_SHELL_TOOL_RESULT:
        return runtime_anthropic_write_shell_tool_result(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_COMPUTER_TOOL_RESULT:
        return runtime_anthropic_write_computer_tool_result(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_TOOL_USE_BLOCK or input.operation == RUNTIME_ANTHROPIC_SERVER_TOOL_BLOCK:
        return runtime_anthropic_write_tool_use_block(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_MCP_CALL_BLOCKS:
        return runtime_anthropic_write_mcp_call_blocks(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_MCP_APPROVAL_BLOCK:
        return runtime_anthropic_write_mcp_approval_block(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_MCP_LIST_TOOLS_BLOCK:
        return runtime_anthropic_write_mcp_list_tools_block(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_THINKING_BLOCK:
        if input.text_present == 0:
            return False
        return runtime_anthropic_put_literal(writer, StringSlice("{\"type\":\"thinking\",\"thinking\":")) and runtime_anthropic_put_json_string(
            writer, input.text
        ) and runtime_anthropic_put_byte(writer, 125)
    if input.operation == RUNTIME_ANTHROPIC_TEXT_BLOCK:
        if input.text_present == 0:
            return False
        return runtime_anthropic_put_literal(writer, StringSlice("{\"type\":\"text\",\"text\":")) and runtime_anthropic_put_json_string(
            writer, input.text
        ) and runtime_anthropic_put_byte(writer, 125)
    if input.operation == RUNTIME_ANTHROPIC_TOOL_RESULT_TEXT_PLAN:
        return runtime_anthropic_write_tool_result_text_plan(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_COMPUTER_ACTION:
        return runtime_anthropic_write_computer_action(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_COMPUTER_TOOL_INPUT:
        return runtime_anthropic_write_computer_tool_input(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_SERVER_TOOL_USAGE:
        if input.message_present == 0:
            return False
        return runtime_anthropic_write_server_tool_usage(writer, input.message)
    if input.operation == RUNTIME_ANTHROPIC_CARRIED_SERVER_TOOL_USAGE:
        if input.message_present == 0:
            return False
        return runtime_anthropic_write_carried_server_tool_usage(writer, input.message)
    if input.operation == RUNTIME_ANTHROPIC_SERVER_TOOL_REGISTRATIONS:
        if input.message_present == 0:
            return False
        return runtime_anthropic_write_server_tool_registrations(writer, input.message)
    if input.operation == RUNTIME_ANTHROPIC_MESSAGE_HAS_TOOL_CHAIN:
        if input.message_present == 0:
            return False
        return runtime_anthropic_write_message_has_tool_chain(writer, input.message)
    if input.operation == RUNTIME_ANTHROPIC_SERVER_TOOL_NAME_KIND:
        if input.name_present == 0:
            return False
        return runtime_anthropic_write_server_tool_name_kind(writer, input.name)
    if input.operation == RUNTIME_ANTHROPIC_CLIENT_TOOL_DESCRIPTION:
        return runtime_anthropic_write_client_tool_description(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_CLIENT_TOOL_SCHEMA:
        return runtime_anthropic_write_client_tool_schema(writer, input)
    if input.operation == RUNTIME_ANTHROPIC_UNVERSIONED_TOOL_TYPE:
        return input.name_present != 0 and runtime_anthropic_write_unversioned_tool_type(writer, input.name)
    if input.operation == RUNTIME_ANTHROPIC_CLIENT_TOOL_NAME_FROM_TYPE:
        return input.name_present != 0 and runtime_anthropic_write_client_tool_kind(writer, input.name)
    if input.operation == RUNTIME_ANTHROPIC_TOOL_VERSION:
        return input.name_present != 0 and runtime_anthropic_write_tool_version(writer, input.name)
    if input.operation == RUNTIME_ANTHROPIC_SERVER_TOOL_NAME_FROM_TYPE:
        return input.name_present != 0 and runtime_anthropic_write_server_tool_type_kind(writer, input.name)
    if input.operation == RUNTIME_ANTHROPIC_CLIENT_TOOL_NAME:
        return input.name_present != 0 and runtime_anthropic_write_client_tool_name_kind(writer, input.name)
    if input.operation == RUNTIME_ANTHROPIC_IS_TOOL_USE_BLOCK_TYPE:
        if input.block_type_present == 0:
            return False
        var matched = rich_view_matches_literal["tool_use"](input.block_type, False) or rich_view_matches_literal["server_tool_use"](input.block_type, False) or rich_view_matches_literal["mcp_tool_use"](input.block_type, False)
        return runtime_anthropic_put_byte(writer, UInt8(49 if matched else 48))
    if input.operation == RUNTIME_ANTHROPIC_IS_TOOL_RESULT_BLOCK_TYPE:
        if input.block_type_present == 0:
            return False
        var matched = rich_view_matches_literal["tool_result"](input.block_type, False) or runtime_anthropic_view_ends_literal(input.block_type, StringSlice("_tool_result"))
        return runtime_anthropic_put_byte(writer, UInt8(49 if matched else 48))
    if input.operation == RUNTIME_ANTHROPIC_TRANSLATE_REASONING_EFFORT:
        return input.name_present != 0 and runtime_anthropic_write_reasoning_effort(writer, input.name, (input.flags & RUNTIME_ANTHROPIC_FLAG_SUPPORTS_XHIGH) != 0)
    return False


def runtime_anthropic_input_valid(
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    return (
        input.operation > 0
        and input.operation <= RUNTIME_ANTHROPIC_TRANSLATE_REASONING_EFFORT
        and rich_view_valid(input.id, RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES)
        and rich_view_valid(input.name, RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES)
        and rich_view_valid(input.block_type, RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES)
        and rich_view_valid(input.server_name, RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES)
        and rich_view_valid(input.text, RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES)
        and rich_view_valid(input.input, RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES)
        and rich_view_valid(input.output, RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES)
        and rich_view_valid(input.content, RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES)
        and rich_view_valid(input.usage, RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES)
        and rich_view_valid(input.stop_reason, RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES)
        and rich_view_valid(input.message, RUNTIME_ANTHROPIC_KERNEL_MAX_BYTES)
    )


def runtime_anthropic_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return RUNTIME_ANTHROPIC_KERNEL_STATUS_ABI
    if input_address == 0 or output_address == 0 or written_address == 0 or output_capacity <= 0:
        return RUNTIME_ANTHROPIC_KERNEL_STATUS_INVALID
    var input = Pointer[
        mut=False, ProdexRuntimeAnthropicKernelInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not runtime_anthropic_input_valid(input[].copy()):
        return RUNTIME_ANTHROPIC_KERNEL_STATUS_UTF8
    var writer = RuntimeAnthropicKernelWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    if not runtime_anthropic_write_operation(writer_ptr, input[].copy()):
        if writer.written >= output_capacity:
            written[] = writer.written
            return RUNTIME_ANTHROPIC_KERNEL_STATUS_CAPACITY
        return RUNTIME_ANTHROPIC_KERNEL_STATUS_INVALID
    written[] = writer.written
    return RUNTIME_ANTHROPIC_KERNEL_STATUS_OK
