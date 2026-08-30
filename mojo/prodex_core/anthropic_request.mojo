from std.memory import Pointer

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView


comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime ANTHROPIC_REQUEST_KERNEL_MAX_BYTES: Int64 = 4_194_304
comptime ANTHROPIC_REQUEST_KERNEL_MAX_DEPTH: Int64 = 128
comptime ANTHROPIC_REQUEST_KERNEL_STATUS_OK: Int64 = 0
comptime ANTHROPIC_REQUEST_KERNEL_STATUS_INVALID: Int64 = 1
comptime ANTHROPIC_REQUEST_KERNEL_STATUS_UTF8: Int64 = 2
comptime ANTHROPIC_REQUEST_KERNEL_STATUS_CAPACITY: Int64 = 3
comptime ANTHROPIC_REQUEST_KERNEL_STATUS_ABI: Int64 = 4

comptime ANTHROPIC_REQUEST_BODY: Int64 = 1
comptime ANTHROPIC_MESSAGE: Int64 = 2
comptime ANTHROPIC_TEXT_BLOCK: Int64 = 3
comptime ANTHROPIC_TOOL_USE_BLOCK: Int64 = 4
comptime ANTHROPIC_TOOL_RESULT_BLOCK: Int64 = 5
comptime ANTHROPIC_TOOL_DECLARATION: Int64 = 6
comptime ANTHROPIC_TOOL_CHOICE: Int64 = 7
comptime ANTHROPIC_WEB_SEARCH_TOOL: Int64 = 8
comptime ANTHROPIC_WEB_SEARCH_CALL: Int64 = 9
comptime ANTHROPIC_TOOL_USE_ITEM: Int64 = 10
comptime ANTHROPIC_TOOL_USAGE: Int64 = 11
comptime ANTHROPIC_APPEND_MESSAGE: Int64 = 12
comptime ANTHROPIC_RESPONSE_MESSAGE: Int64 = 13
comptime ANTHROPIC_RESPONSE_REASONING: Int64 = 14
comptime ANTHROPIC_STREAM_MESSAGE_START: Int64 = 20
comptime ANTHROPIC_STREAM_TEXT_START: Int64 = 21
comptime ANTHROPIC_STREAM_TOOL_START: Int64 = 22
comptime ANTHROPIC_STREAM_WEB_SEARCH_START: Int64 = 23
comptime ANTHROPIC_STREAM_THINKING_START: Int64 = 24
comptime ANTHROPIC_STREAM_TEXT_DELTA: Int64 = 25
comptime ANTHROPIC_STREAM_ARGUMENTS_DELTA: Int64 = 26
comptime ANTHROPIC_STREAM_THINKING_DELTA: Int64 = 27
comptime ANTHROPIC_STREAM_COMPLETED: Int64 = 28
comptime ANTHROPIC_STREAM_ERROR: Int64 = 29


@fieldwise_init
struct ProdexAnthropicRequestKernelInput(Copyable):
    var operation: Int64
    var stream: Int64
    var choice_kind: Int64
    var created_at: UInt64
    var count: UInt64
    var index: ProdexRichStringView
    var model: ProdexRichStringView
    var system: ProdexRichStringView
    var messages: ProdexRichStringView
    var max_tokens: ProdexRichStringView
    var temperature: ProdexRichStringView
    var top_p: ProdexRichStringView
    var stop_sequences: ProdexRichStringView
    var tools: ProdexRichStringView
    var tool_choice: ProdexRichStringView
    var role: ProdexRichStringView
    var blocks: ProdexRichStringView
    var id: ProdexRichStringView
    var name: ProdexRichStringView
    var namespace: ProdexRichStringView
    var input: ProdexRichStringView
    var content: ProdexRichStringView
    var arguments: ProdexRichStringView
    var delta: ProdexRichStringView
    var error: ProdexRichStringView
    var queries: ProdexRichStringView
    var allowed_domains: ProdexRichStringView
    var blocked_domains: ProdexRichStringView
    var user_location: ProdexRichStringView
    var max_uses: ProdexRichStringView
    var tool_use_id: ProdexRichStringView


@fieldwise_init
struct AnthropicRequestKernelWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def anthropic_request_byte(view: ProdexRichStringView, index: Int64) -> UInt8:
    return rich_view_ptr(view)[unsafe_offset=index]


def anthropic_request_put_byte(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _], value: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True


def anthropic_request_put_literal(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _], value: StringSlice
) -> Bool:
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not anthropic_request_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def anthropic_request_put_view(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if view.len == 0:
        return True
    var ptr = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        if not anthropic_request_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def anthropic_request_put_view_range(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var ptr = rich_view_ptr(view)
    for index in range(end - start):
        if not anthropic_request_put_byte(
            writer, ptr[unsafe_offset=start + index]
        ):
            return False
    return True


def anthropic_request_put_u64(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return anthropic_request_put_byte(writer, 48)
    var divisor: UInt64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        if not anthropic_request_put_byte(writer, UInt8(remaining / divisor) + 48):
            return False
        remaining %= divisor
        divisor /= 10
    return True


def anthropic_request_put_optional(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    key: StringSlice,
    view: ProdexRichStringView,
) -> Bool:
    if view.len == 0:
        return True
    return anthropic_request_put_literal(writer, key) and anthropic_request_put_view(
        writer, view
    )


def anthropic_request_put_name(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    namespace: ProdexRichStringView,
    name: ProdexRichStringView,
) -> Bool:
    if namespace.len == 0:
        return anthropic_request_put_view(writer, name)
    if namespace.len < 2 or name.len < 2:
        return False
    if (
        anthropic_request_byte(namespace, 0) != 34
        or anthropic_request_byte(namespace, Int64(namespace.len) - 1) != 34
        or anthropic_request_byte(name, 0) != 34
        or anthropic_request_byte(name, Int64(name.len) - 1) != 34
    ):
        return False
    return (
        anthropic_request_put_byte(writer, 34)
        and anthropic_request_put_view_range(
            writer, namespace, 1, Int64(namespace.len) - 1
        )
        and anthropic_request_put_literal(writer, StringSlice("--"))
        and anthropic_request_put_view_range(writer, name, 1, Int64(name.len) - 1)
        and anthropic_request_put_byte(writer, 34)
    )


def anthropic_request_put_event_prefix(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _], event: StringSlice
) -> Bool:
    return anthropic_request_put_literal(writer, StringSlice("event: ")) and anthropic_request_put_literal(
        writer, event
    ) and anthropic_request_put_literal(writer, StringSlice("\ndata: "))


def anthropic_request_finish_event(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _]
) -> Bool:
    return anthropic_request_put_literal(writer, StringSlice("\n\n"))


def anthropic_request_write_body(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if (
        input.model.len == 0
        or input.messages.len == 0
        or input.max_tokens.len == 0
        or input.stream < 0
        or input.stream > 1
    ):
        return False
    if not anthropic_request_put_literal(writer, StringSlice('{"model":')):
        return False
    if not anthropic_request_put_view(writer, input.model):
        return False
    if not anthropic_request_put_literal(writer, StringSlice(',"messages":')):
        return False
    if not anthropic_request_put_view(writer, input.messages):
        return False
    if not anthropic_request_put_literal(writer, StringSlice(',"max_tokens":')):
        return False
    if not anthropic_request_put_view(writer, input.max_tokens):
        return False
    if not anthropic_request_put_literal(writer, StringSlice(',"stream":')):
        return False
    if input.stream == 1:
        if not anthropic_request_put_literal(writer, StringSlice("true")):
            return False
    elif not anthropic_request_put_literal(writer, StringSlice("false")):
        return False
    if not anthropic_request_put_optional(writer, StringSlice(',"system":'), input.system):
        return False
    if not anthropic_request_put_optional(
        writer, StringSlice(',"temperature":'), input.temperature
    ):
        return False
    if not anthropic_request_put_optional(writer, StringSlice(',"top_p":'), input.top_p):
        return False
    if not anthropic_request_put_optional(
        writer, StringSlice(',"stop_sequences":'), input.stop_sequences
    ):
        return False
    if not anthropic_request_put_optional(writer, StringSlice(',"tools":'), input.tools):
        return False
    if not anthropic_request_put_optional(
        writer, StringSlice(',"tool_choice":'), input.tool_choice
    ):
        return False
    return anthropic_request_put_byte(writer, 125)


def anthropic_request_write_message(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.role.len == 0 or input.blocks.len == 0:
        return False
    return (
        anthropic_request_put_literal(writer, StringSlice('{"role":'))
        and anthropic_request_put_view(writer, input.role)
        and anthropic_request_put_literal(writer, StringSlice(',"content":'))
        and anthropic_request_put_view(writer, input.blocks)
        and anthropic_request_put_byte(writer, 125)
    )


def anthropic_request_write_text_block(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.content.len == 0:
        return False
    return (
        anthropic_request_put_literal(writer, StringSlice('{"type":"text","text":'))
        and anthropic_request_put_view(writer, input.content)
        and anthropic_request_put_byte(writer, 125)
    )


def anthropic_request_write_tool_use_block(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.id.len == 0 or input.name.len == 0 or input.input.len == 0:
        return False
    if not anthropic_request_put_literal(
        writer, StringSlice('{"type":"tool_use","id":')
    ) or not anthropic_request_put_view(writer, input.id):
        return False
    if not anthropic_request_put_literal(writer, StringSlice(',"name":')):
        return False
    if not anthropic_request_put_name(writer, input.namespace, input.name):
        return False
    return (
        anthropic_request_put_literal(writer, StringSlice(',"input":'))
        and anthropic_request_put_view(writer, input.input)
        and anthropic_request_put_byte(writer, 125)
    )


def anthropic_request_write_tool_result_block(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.tool_use_id.len == 0 or input.content.len == 0:
        return False
    return (
        anthropic_request_put_literal(
            writer, StringSlice('{"type":"tool_result","tool_use_id":')
        )
        and anthropic_request_put_view(writer, input.tool_use_id)
        and anthropic_request_put_literal(writer, StringSlice(',"content":'))
        and anthropic_request_put_view(writer, input.content)
        and anthropic_request_put_byte(writer, 125)
    )


def anthropic_request_write_tool_declaration(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.name.len == 0 or input.input.len == 0:
        return False
    if not anthropic_request_put_literal(writer, StringSlice('{"name":')):
        return False
    if not anthropic_request_put_name(writer, input.namespace, input.name):
        return False
    if not anthropic_request_put_literal(writer, StringSlice(',"input_schema":')):
        return False
    if not anthropic_request_put_view(writer, input.input):
        return False
    if not anthropic_request_put_optional(
        writer, StringSlice(',"description":'), input.content
    ):
        return False
    return anthropic_request_put_byte(writer, 125)


def anthropic_request_write_tool_choice(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.choice_kind == 1:
        return anthropic_request_put_literal(writer, StringSlice('{"type":"auto"}'))
    if input.choice_kind == 2:
        return anthropic_request_put_literal(writer, StringSlice('{"type":"any"}'))
    if input.choice_kind != 3 or input.name.len == 0:
        return False
    if not anthropic_request_put_literal(writer, StringSlice('{"type":"tool","name":')):
        return False
    if not anthropic_request_put_name(writer, input.namespace, input.name):
        return False
    return anthropic_request_put_byte(writer, 125)


def anthropic_request_write_web_search_tool(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if not anthropic_request_put_literal(
        writer, StringSlice('{"type":"web_search_20250305","name":"web_search"')
    ):
        return False
    if not anthropic_request_put_optional(
        writer, StringSlice(',"allowed_domains":'), input.allowed_domains
    ):
        return False
    if not anthropic_request_put_optional(
        writer, StringSlice(',"blocked_domains":'), input.blocked_domains
    ):
        return False
    if not anthropic_request_put_optional(
        writer, StringSlice(',"user_location":'), input.user_location
    ):
        return False
    if not anthropic_request_put_optional(writer, StringSlice(',"max_uses":'), input.max_uses):
        return False
    return anthropic_request_put_byte(writer, 125)


def anthropic_request_write_web_search_call(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.id.len == 0 or input.queries.len == 0 or input.choice_kind < 0 or input.choice_kind > 1:
        return False
    if not anthropic_request_put_literal(
        writer, StringSlice('{"type":"web_search_call","id":')
    ) or not anthropic_request_put_view(writer, input.id):
        return False
    if input.choice_kind == 1:
        if not anthropic_request_put_literal(writer, StringSlice(',"status":"in_progress"')):
            return False
    elif not anthropic_request_put_literal(writer, StringSlice(',"status":"completed"')):
        return False
    return (
        anthropic_request_put_literal(
            writer, StringSlice(',"action":{"type":"search","queries":')
        )
        and anthropic_request_put_view(writer, input.queries)
        and anthropic_request_put_literal(writer, StringSlice(',"sources":[]}'))
        and anthropic_request_put_byte(writer, 125)
    )


def anthropic_request_write_tool_use_item(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.id.len == 0 or input.name.len == 0 or input.arguments.len == 0:
        return False
    if not anthropic_request_put_literal(
        writer, StringSlice('{"type":"function_call","call_id":')
    ) or not anthropic_request_put_view(writer, input.id):
        return False
    if not anthropic_request_put_literal(writer, StringSlice(',"name":')):
        return False
    if not anthropic_request_put_view(writer, input.name):
        return False
    if not anthropic_request_put_literal(writer, StringSlice(',"arguments":')):
        return False
    if not anthropic_request_put_view(writer, input.arguments):
        return False
    if input.namespace.len > 0:
        if not anthropic_request_put_literal(writer, StringSlice(',"namespace":')):
            return False
        if not anthropic_request_put_view(writer, input.namespace):
            return False
    return anthropic_request_put_byte(writer, 125)


def anthropic_request_write_tool_usage(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    return (
        anthropic_request_put_literal(
            writer, StringSlice('{"web_search":{"num_requests":')
        )
        and anthropic_request_put_u64(writer, input.count)
        and anthropic_request_put_literal(writer, StringSlice("}}"))
    )


def anthropic_request_skip_ws(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    var index = start
    while index < end:
        var value = anthropic_request_byte(view, index)
        if value != 9 and value != 10 and value != 13 and value != 32:
            break
        index += 1
    return index


def anthropic_request_string_end(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    if start < 0 or start >= end or anthropic_request_byte(view, start) != 34:
        return -1
    var index = start + 1
    while index < end:
        var value = anthropic_request_byte(view, index)
        if value == 92:
            if index + 1 >= end:
                return -1
            index += 2
        elif value == 34:
            return index + 1
        elif value < 32:
            return -1
        else:
            index += 1
    return -1


def anthropic_request_value_end(
    view: ProdexRichStringView, start: Int64, end: Int64, depth: Int64
) -> Int64:
    if depth > ANTHROPIC_REQUEST_KERNEL_MAX_DEPTH:
        return -1
    var index = anthropic_request_skip_ws(view, start, end)
    if index >= end:
        return -1
    var opening = anthropic_request_byte(view, index)
    if opening == 34:
        return anthropic_request_string_end(view, index, end)
    if opening == 91:
        index += 1
        index = anthropic_request_skip_ws(view, index, end)
        if index < end and anthropic_request_byte(view, index) == 93:
            return index + 1
        while index < end:
            var value_end = anthropic_request_value_end(view, index, end, depth + 1)
            if value_end < 0:
                return -1
            index = anthropic_request_skip_ws(view, value_end, end)
            if index < end and anthropic_request_byte(view, index) == 44:
                index = anthropic_request_skip_ws(view, index + 1, end)
                continue
            if index < end and anthropic_request_byte(view, index) == 93:
                return index + 1
            return -1
        return -1
    if opening == 123:
        index += 1
        index = anthropic_request_skip_ws(view, index, end)
        if index < end and anthropic_request_byte(view, index) == 125:
            return index + 1
        while index < end:
            var key_end = anthropic_request_string_end(view, index, end)
            if key_end < 0:
                return -1
            index = anthropic_request_skip_ws(view, key_end, end)
            if index >= end or anthropic_request_byte(view, index) != 58:
                return -1
            index = anthropic_request_skip_ws(view, index + 1, end)
            var value_end = anthropic_request_value_end(view, index, end, depth + 1)
            if value_end < 0:
                return -1
            index = anthropic_request_skip_ws(view, value_end, end)
            if index < end and anthropic_request_byte(view, index) == 44:
                index = anthropic_request_skip_ws(view, index + 1, end)
                continue
            if index < end and anthropic_request_byte(view, index) == 125:
                return index + 1
            return -1
        return -1
    while index < end:
        var value = anthropic_request_byte(view, index)
        if value == 44 or value == 93 or value == 125 or value == 9 or value == 10 or value == 13 or value == 32:
            break
        index += 1
    if index == start:
        return -1
    return index


def anthropic_request_json_valid(view: ProdexRichStringView) -> Bool:
    if view.len == 0:
        return True
    return anthropic_request_value_end(view, 0, Int64(view.len), 0) == Int64(view.len)


def anthropic_request_range_equal(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    other: ProdexRichStringView,
) -> Bool:
    if end - start != Int64(other.len) or start < 0 or end > Int64(view.len):
        return False
    var left = rich_view_ptr(view)
    var right = rich_view_ptr(other)
    for index in range(end - start):
        if left[unsafe_offset=start + index] != right[unsafe_offset=index]:
            return False
    return True


def anthropic_request_object_field(
    view: ProdexRichStringView,
    object_start: Int64,
    object_end: Int64,
    key: StringSlice,
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    if (
        object_start < 0
        or object_end <= object_start + 1
        or anthropic_request_byte(view, object_start) != 123
        or anthropic_request_byte(view, object_end - 1) != 125
    ):
        return result^
    var index = anthropic_request_skip_ws(view, object_start + 1, object_end - 1)
    while index < object_end - 1:
        var key_start = index
        var key_end = anthropic_request_string_end(view, key_start, object_end - 1)
        if key_end < 0:
            return result^
        index = anthropic_request_skip_ws(view, key_end, object_end - 1)
        if index >= object_end - 1 or anthropic_request_byte(view, index) != 58:
            return result^
        var value_start = anthropic_request_skip_ws(view, index + 1, object_end - 1)
        var value_end = anthropic_request_value_end(view, value_start, object_end - 1, 0)
        if value_end < 0:
            return result^
        var key_ptr = key.unsafe_ptr()
        var key_matches = key_end - key_start == Int64(key.byte_length())
        if key_matches:
            var left = rich_view_ptr(view)
            for offset in range(key_end - key_start):
                if left[unsafe_offset=key_start + offset] != key_ptr[unsafe_offset=offset]:
                    key_matches = False
                    break
        if key_matches:
            result[0] = value_start
            result[1] = value_end
            return result^
        index = anthropic_request_skip_ws(view, value_end, object_end - 1)
        if index < object_end - 1 and anthropic_request_byte(view, index) == 44:
            index = anthropic_request_skip_ws(view, index + 1, object_end - 1)
        else:
            break
    return result^


def anthropic_request_array_last_item(
    view: ProdexRichStringView,
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    if view.len < 2 or anthropic_request_byte(view, 0) != 91 or anthropic_request_byte(view, Int64(view.len) - 1) != 93:
        return result^
    var index = anthropic_request_skip_ws(view, 1, Int64(view.len) - 1)
    if index >= Int64(view.len) - 1:
        return result^
    while index < Int64(view.len) - 1:
        var item_start = index
        var item_end = anthropic_request_value_end(view, item_start, Int64(view.len) - 1, 0)
        if item_end < 0:
            return InlineArray[Int64, 2](fill=-1)
        result[0] = item_start
        result[1] = item_end
        index = anthropic_request_skip_ws(view, item_end, Int64(view.len) - 1)
        if index < Int64(view.len) - 1 and anthropic_request_byte(view, index) == 44:
            index = anthropic_request_skip_ws(view, index + 1, Int64(view.len) - 1)
            continue
        if index == Int64(view.len) - 1:
            return result^
        return InlineArray[Int64, 2](fill=-1)
    return result^


def anthropic_request_array_first_item(
    view: ProdexRichStringView,
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    if view.len < 2 or anthropic_request_byte(view, 0) != 91 or anthropic_request_byte(view, Int64(view.len) - 1) != 93:
        return result^
    var start = anthropic_request_skip_ws(view, 1, Int64(view.len) - 1)
    if start >= Int64(view.len) - 1:
        return result^
    var end = anthropic_request_value_end(view, start, Int64(view.len) - 1, 0)
    if end < 0:
        return result^
    result[0] = start
    result[1] = end
    return result^


def anthropic_request_item_is_tool_result(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    var kind = anthropic_request_object_field(view, start, end, StringSlice('"type"'))
    return kind[0] >= 0 and anthropic_request_range_equal(
        view, kind[0], kind[1], ProdexRichStringView(0, 0)
    )


def anthropic_request_range_matches_literal(
    view: ProdexRichStringView, start: Int64, end: Int64, literal: StringSlice
) -> Bool:
    if end - start != Int64(literal.byte_length()) or start < 0 or end > Int64(view.len):
        return False
    var left = rich_view_ptr(view)
    var right = literal.unsafe_ptr()
    for index in range(end - start):
        if left[unsafe_offset=start + index] != right[unsafe_offset=index]:
            return False
    return True


def anthropic_request_item_is_tool_result_value(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    var kind = anthropic_request_object_field(view, start, end, StringSlice('"type"'))
    return kind[0] >= 0 and anthropic_request_range_matches_literal(
        view, kind[0], kind[1], StringSlice('"tool_result"')
    )


def anthropic_request_write_response_message(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if (
        input.blocks.len < 2
        or anthropic_request_byte(input.blocks, 0) != 91
        or anthropic_request_byte(input.blocks, Int64(input.blocks.len) - 1) != 93
    ):
        return False
    var end = Int64(input.blocks.len) - 1
    var index = anthropic_request_skip_ws(input.blocks, 1, end)
    if index >= end:
        return False
    if not anthropic_request_put_literal(
        writer, StringSlice('{"type":"message","role":"assistant","content":[')
    ):
        return False
    var first = True
    while index < end:
        var item_end = anthropic_request_value_end(input.blocks, index, end, 0)
        if item_end < 0:
            return False
        var kind = anthropic_request_object_field(
            input.blocks, index, item_end, StringSlice('"type"')
        )
        if kind[0] < 0 or not anthropic_request_range_matches_literal(
            input.blocks, kind[0], kind[1], StringSlice('"text"')
        ):
            return False
        var text = anthropic_request_object_field(
            input.blocks, index, item_end, StringSlice('"text"')
        )
        if text[0] < 0:
            return False
        if not first and not anthropic_request_put_byte(writer, 44):
            return False
        first = False
        if not anthropic_request_put_literal(
            writer, StringSlice('{"type":"output_text","text":')
        ) or not anthropic_request_put_view_range(
            writer, input.blocks, text[0], text[1]
        ) or not anthropic_request_put_byte(writer, 125):
            return False
        index = anthropic_request_skip_ws(input.blocks, item_end, end)
        if index < end and anthropic_request_byte(input.blocks, index) == 44:
            index = anthropic_request_skip_ws(input.blocks, index + 1, end)
            if index >= end:
                return False
        elif index != end:
            return False
    return not first and anthropic_request_put_literal(writer, StringSlice("]}"))


def anthropic_request_write_response_reasoning(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.content.len < 2:
        return False
    var kind = anthropic_request_object_field(
        input.content, 0, Int64(input.content.len), StringSlice('"type"')
    )
    var thinking = anthropic_request_object_field(
        input.content, 0, Int64(input.content.len), StringSlice('"thinking"')
    )
    if kind[0] < 0 or not anthropic_request_range_matches_literal(
        input.content, kind[0], kind[1], StringSlice('"thinking"')
    ) or thinking[0] < 0:
        return False
    return (
        anthropic_request_put_literal(
            writer, StringSlice('{"type":"reasoning","summary":[{"type":"summary_text","text":')
        )
        and anthropic_request_put_view_range(writer, input.content, thinking[0], thinking[1])
        and anthropic_request_put_literal(writer, StringSlice("}]}"))
    )


def anthropic_request_put_new_message(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    role: ProdexRichStringView,
    blocks: ProdexRichStringView,
) -> Bool:
    return (
        anthropic_request_put_literal(writer, StringSlice('{"role":'))
        and anthropic_request_put_view(writer, role)
        and anthropic_request_put_literal(writer, StringSlice(',"content":'))
        and anthropic_request_put_view(writer, blocks)
        and anthropic_request_put_byte(writer, 125)
    )


def anthropic_request_put_array_items(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    first: Pointer[mut=True, Bool, _],
) -> Bool:
    var index = anthropic_request_skip_ws(view, start, end)
    while index < end:
        var item_end = anthropic_request_value_end(view, index, end, 0)
        if item_end < 0:
            return False
        if not first[] and not anthropic_request_put_byte(writer, 44):
            return False
        first[] = False
        if not anthropic_request_put_view_range(writer, view, index, item_end):
            return False
        index = anthropic_request_skip_ws(view, item_end, end)
        if index < end and anthropic_request_byte(view, index) == 44:
            index = anthropic_request_skip_ws(view, index + 1, end)
        else:
            break
    return True


def anthropic_request_put_merged_content(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    old_view: ProdexRichStringView,
    old_start: Int64,
    old_end: Int64,
    new_view: ProdexRichStringView,
    insert_before_non_tool_result: Bool,
) -> Bool:
    var first = True
    var first_ptr = Pointer(to=first)
    var inserted = not insert_before_non_tool_result
    var index = anthropic_request_skip_ws(old_view, old_start, old_end)
    while index < old_end:
        var item_end = anthropic_request_value_end(old_view, index, old_end, 0)
        if item_end < 0:
            return False
        if not inserted and not anthropic_request_item_is_tool_result_value(old_view, index, item_end):
            if not anthropic_request_put_array_items(
                writer, new_view, 1, Int64(new_view.len) - 1, first_ptr
            ):
                return False
            inserted = True
        if not anthropic_request_put_array_items(writer, old_view, index, item_end, first_ptr):
            return False
        index = anthropic_request_skip_ws(old_view, item_end, old_end)
        if index < old_end and anthropic_request_byte(old_view, index) == 44:
            index = anthropic_request_skip_ws(old_view, index + 1, old_end)
        else:
            break
    if not inserted and not anthropic_request_put_array_items(
        writer, new_view, 1, Int64(new_view.len) - 1, first_ptr
    ):
        return False
    return True


def anthropic_request_append_message(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.messages.len < 2 or input.role.len == 0 or input.blocks.len < 2:
        return False
    var last = anthropic_request_array_last_item(input.messages)
    if last[0] < 0:
        return (
            anthropic_request_put_byte(writer, 91)
            and anthropic_request_put_new_message(writer, input.role, input.blocks)
            and anthropic_request_put_byte(writer, 93)
        )

    var previous_role = anthropic_request_object_field(
        input.messages, last[0], last[1], StringSlice('"role"')
    )
    var previous_content = anthropic_request_object_field(
        input.messages, last[0], last[1], StringSlice('"content"')
    )
    var same_role = previous_role[0] >= 0 and anthropic_request_range_equal(
        input.messages, previous_role[0], previous_role[1], input.role
    )
    var content_is_array = previous_content[0] >= 0 and previous_content[1] > previous_content[0] and anthropic_request_byte(
        input.messages, previous_content[0]
    ) == 91 and anthropic_request_byte(input.messages, previous_content[1] - 1) == 93
    if not same_role or not content_is_array:
        if not anthropic_request_put_view_range(writer, input.messages, 0, Int64(input.messages.len) - 1):
            return False
        if not anthropic_request_put_byte(writer, 44):
            return False
        if not anthropic_request_put_new_message(writer, input.role, input.blocks):
            return False
        return anthropic_request_put_byte(writer, 93)

    if not anthropic_request_put_view_range(writer, input.messages, 0, last[0]):
        return False
    if not anthropic_request_put_view_range(
        writer, input.messages, last[0], previous_content[0] + 1
    ):
        return False
    var first_new_is_tool_result = anthropic_request_array_first_item(input.blocks)
    var insert_before = first_new_is_tool_result[0] >= 0 and anthropic_request_item_is_tool_result_value(
        input.blocks, first_new_is_tool_result[0], first_new_is_tool_result[1]
    )
    if not anthropic_request_put_merged_content(
        writer,
        input.messages,
        previous_content[0] + 1,
        previous_content[1] - 1,
        input.blocks,
        insert_before,
    ):
        return False
    return anthropic_request_put_view_range(
        writer, input.messages, previous_content[1] - 1, Int64(input.messages.len)
    )


def anthropic_request_write_stream(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.operation == ANTHROPIC_STREAM_MESSAGE_START:
        if not anthropic_request_put_event_prefix(writer, StringSlice("response.created")):
            return False
        if not anthropic_request_put_literal(
            writer, StringSlice('{"type":"response.created","response":{"id":')
        ) or not anthropic_request_put_view(writer, input.id):
            return False
        if not anthropic_request_put_literal(
            writer, StringSlice(',"object":"response","created_at":')
        ) or not anthropic_request_put_u64(writer, input.created_at):
            return False
        if not anthropic_request_put_literal(writer, StringSlice(',"model":')) or not anthropic_request_put_view(
            writer, input.model
        ):
            return False
        return anthropic_request_put_literal(writer, StringSlice(',"output":[]}}')) and anthropic_request_finish_event(writer)
    if input.operation == ANTHROPIC_STREAM_TEXT_START:
        if not anthropic_request_put_event_prefix(
            writer, StringSlice("response.output_item.added")
        ):
            return False
        return (
            anthropic_request_put_literal(
                writer, StringSlice('{"type":"response.output_item.added","output_index":')
            )
            and anthropic_request_put_view(writer, input.index)
            and anthropic_request_put_literal(
                writer, StringSlice(',"item":{"type":"message","role":"assistant","content":[]}}')
            )
            and anthropic_request_finish_event(writer)
        )
    if input.operation == ANTHROPIC_STREAM_TOOL_START:
        if not anthropic_request_put_event_prefix(
            writer, StringSlice("response.output_item.added")
        ):
            return False
        return (
            anthropic_request_put_literal(
                writer, StringSlice('{"type":"response.output_item.added","output_index":')
            )
            and anthropic_request_put_view(writer, input.index)
            and anthropic_request_put_literal(writer, StringSlice(',"item":{"type":"function_call","call_id":'))
            and anthropic_request_put_view(writer, input.id)
            and anthropic_request_put_literal(writer, StringSlice(',"name":'))
            and anthropic_request_put_view(writer, input.name)
            and anthropic_request_put_literal(writer, StringSlice(',"arguments":""}}'))
            and anthropic_request_finish_event(writer)
        )
    if input.operation == ANTHROPIC_STREAM_WEB_SEARCH_START:
        if not anthropic_request_put_event_prefix(
            writer, StringSlice("response.output_item.added")
        ):
            return False
        return (
            anthropic_request_put_literal(
                writer, StringSlice('{"type":"response.output_item.added","output_index":')
            )
            and anthropic_request_put_view(writer, input.index)
            and anthropic_request_put_literal(writer, StringSlice(',"item":{"type":"web_search_call","id":'))
            and anthropic_request_put_view(writer, input.id)
            and anthropic_request_put_literal(writer, StringSlice(',"status":"in_progress","action":{"type":"search","queries":'))
            and anthropic_request_put_view(writer, input.queries)
            and anthropic_request_put_literal(writer, StringSlice(',"sources":[]}}}'))
            and anthropic_request_finish_event(writer)
        )
    if input.operation == ANTHROPIC_STREAM_THINKING_START:
        if not anthropic_request_put_event_prefix(
            writer, StringSlice("response.output_item.added")
        ):
            return False
        return (
            anthropic_request_put_literal(
                writer, StringSlice('{"type":"response.output_item.added","output_index":')
            )
            and anthropic_request_put_view(writer, input.index)
            and anthropic_request_put_literal(writer, StringSlice(',"item":{"type":"reasoning","summary":[]}}'))
            and anthropic_request_finish_event(writer)
        )
    if input.operation == ANTHROPIC_STREAM_TEXT_DELTA or input.operation == ANTHROPIC_STREAM_ARGUMENTS_DELTA or input.operation == ANTHROPIC_STREAM_THINKING_DELTA:
        var event = StringSlice("response.output_text.delta")
        var prefix = StringSlice('{"type":"response.output_text.delta","output_index":')
        if input.operation == ANTHROPIC_STREAM_ARGUMENTS_DELTA:
            event = StringSlice("response.function_call_arguments.delta")
            prefix = StringSlice(
                '{"type":"response.function_call_arguments.delta","output_index":'
            )
        elif input.operation == ANTHROPIC_STREAM_THINKING_DELTA:
            event = StringSlice("response.reasoning_summary_text.delta")
            prefix = StringSlice(
                '{"type":"response.reasoning_summary_text.delta","output_index":'
            )
        if not anthropic_request_put_event_prefix(writer, event):
            return False
        return (
            anthropic_request_put_literal(writer, prefix)
            and anthropic_request_put_view(writer, input.index)
            and anthropic_request_put_literal(writer, StringSlice(',"delta":'))
            and anthropic_request_put_view(writer, input.delta)
            and anthropic_request_put_byte(writer, 125)
            and anthropic_request_finish_event(writer)
        )
    if input.operation == ANTHROPIC_STREAM_COMPLETED:
        return (
            anthropic_request_put_event_prefix(writer, StringSlice("response.completed"))
            and anthropic_request_put_literal(writer, StringSlice('{"type":"response.completed"}'))
            and anthropic_request_finish_event(writer)
        )
    if input.operation == ANTHROPIC_STREAM_ERROR:
        return (
            anthropic_request_put_event_prefix(writer, StringSlice("error"))
            and anthropic_request_put_literal(writer, StringSlice('{"type":"error","error":'))
            and anthropic_request_put_view(writer, input.error)
            and anthropic_request_put_byte(writer, 125)
            and anthropic_request_finish_event(writer)
        )
    return False


def anthropic_request_view_valid(view: ProdexRichStringView) -> Bool:
    return rich_view_valid(view, ANTHROPIC_REQUEST_KERNEL_MAX_BYTES) and anthropic_request_json_valid(view)


def anthropic_request_input_valid(input: ProdexAnthropicRequestKernelInput) -> Bool:
    if input.operation < ANTHROPIC_REQUEST_BODY or input.operation > ANTHROPIC_STREAM_ERROR:
        return False
    if input.stream < 0 or input.stream > 1 or input.choice_kind < -1 or input.choice_kind > 3:
        return False
    return (
        anthropic_request_view_valid(input.index)
        and anthropic_request_view_valid(input.model)
        and anthropic_request_view_valid(input.system)
        and anthropic_request_view_valid(input.messages)
        and anthropic_request_view_valid(input.max_tokens)
        and anthropic_request_view_valid(input.temperature)
        and anthropic_request_view_valid(input.top_p)
        and anthropic_request_view_valid(input.stop_sequences)
        and anthropic_request_view_valid(input.tools)
        and anthropic_request_view_valid(input.tool_choice)
        and anthropic_request_view_valid(input.role)
        and anthropic_request_view_valid(input.blocks)
        and anthropic_request_view_valid(input.id)
        and anthropic_request_view_valid(input.name)
        and anthropic_request_view_valid(input.namespace)
        and anthropic_request_view_valid(input.input)
        and anthropic_request_view_valid(input.content)
        and anthropic_request_view_valid(input.arguments)
        and anthropic_request_view_valid(input.delta)
        and anthropic_request_view_valid(input.error)
        and anthropic_request_view_valid(input.queries)
        and anthropic_request_view_valid(input.allowed_domains)
        and anthropic_request_view_valid(input.blocked_domains)
        and anthropic_request_view_valid(input.user_location)
        and anthropic_request_view_valid(input.max_uses)
        and anthropic_request_view_valid(input.tool_use_id)
    )


def anthropic_request_write_operation(
    writer: Pointer[mut=True, AnthropicRequestKernelWriter, _],
    input: ProdexAnthropicRequestKernelInput,
) -> Bool:
    if input.operation == ANTHROPIC_REQUEST_BODY:
        return anthropic_request_write_body(writer, input)
    if input.operation == ANTHROPIC_MESSAGE:
        return anthropic_request_write_message(writer, input)
    if input.operation == ANTHROPIC_TEXT_BLOCK:
        return anthropic_request_write_text_block(writer, input)
    if input.operation == ANTHROPIC_TOOL_USE_BLOCK:
        return anthropic_request_write_tool_use_block(writer, input)
    if input.operation == ANTHROPIC_TOOL_RESULT_BLOCK:
        return anthropic_request_write_tool_result_block(writer, input)
    if input.operation == ANTHROPIC_TOOL_DECLARATION:
        return anthropic_request_write_tool_declaration(writer, input)
    if input.operation == ANTHROPIC_TOOL_CHOICE:
        return anthropic_request_write_tool_choice(writer, input)
    if input.operation == ANTHROPIC_WEB_SEARCH_TOOL:
        return anthropic_request_write_web_search_tool(writer, input)
    if input.operation == ANTHROPIC_WEB_SEARCH_CALL:
        return anthropic_request_write_web_search_call(writer, input)
    if input.operation == ANTHROPIC_TOOL_USE_ITEM:
        return anthropic_request_write_tool_use_item(writer, input)
    if input.operation == ANTHROPIC_TOOL_USAGE:
        return anthropic_request_write_tool_usage(writer, input)
    if input.operation == ANTHROPIC_APPEND_MESSAGE:
        return anthropic_request_append_message(writer, input)
    if input.operation == ANTHROPIC_RESPONSE_MESSAGE:
        return anthropic_request_write_response_message(writer, input)
    if input.operation == ANTHROPIC_RESPONSE_REASONING:
        return anthropic_request_write_response_reasoning(writer, input)
    if input.operation >= ANTHROPIC_STREAM_MESSAGE_START:
        return anthropic_request_write_stream(writer, input)
    return False


def anthropic_request_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return ANTHROPIC_REQUEST_KERNEL_STATUS_ABI
    if input_address == 0 or output_address == 0 or written_address == 0 or output_capacity <= 0:
        return ANTHROPIC_REQUEST_KERNEL_STATUS_INVALID
    var input = Pointer[
        mut=False, ProdexAnthropicRequestKernelInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not anthropic_request_input_valid(input[].copy()):
        return ANTHROPIC_REQUEST_KERNEL_STATUS_UTF8
    var writer = AnthropicRequestKernelWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    if not anthropic_request_write_operation(writer_ptr, input[].copy()):
        if writer.written >= output_capacity:
            written[] = writer.written
            return ANTHROPIC_REQUEST_KERNEL_STATUS_CAPACITY
        return ANTHROPIC_REQUEST_KERNEL_STATUS_INVALID
    written[] = writer.written
    return ANTHROPIC_REQUEST_KERNEL_STATUS_OK
