from std.memory import Pointer

from anthropic_request import (
    anthropic_request_byte,
    anthropic_request_object_field,
    anthropic_request_range_matches_literal,
    anthropic_request_skip_ws,
    anthropic_request_string_end,
    anthropic_request_value_end,
)
from rich_text import rich_view_matches_literal, rich_view_ptr, rich_view_valid
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

comptime RUNTIME_ANTHROPIC_FLAG_ERROR: Int64 = 1
comptime RUNTIME_ANTHROPIC_FLAG_MAX_OUTPUT_LENGTH: Int64 = 2
comptime RUNTIME_ANTHROPIC_FLAG_CACHED_TOKENS: Int64 = 4


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
    if not runtime_anthropic_put_byte(writer, 34):
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
    return runtime_anthropic_put_byte(writer, 34)


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
    return False


def runtime_anthropic_input_valid(
    input: ProdexRuntimeAnthropicKernelInput,
) -> Bool:
    return (
        input.operation > 0
        and input.operation <= RUNTIME_ANTHROPIC_TEXT_BLOCK
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
