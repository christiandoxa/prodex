from std.memory import Pointer

from rich_text import rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime PRODEX_RICH_ABI_VERSION: Int64 = 6

comptime GEMINI_KERNEL_MAX_BYTES: Int64 = 4_194_304
comptime GEMINI_KERNEL_STATUS_OK: Int64 = 0
comptime GEMINI_KERNEL_STATUS_INVALID: Int64 = 1
comptime GEMINI_KERNEL_STATUS_UTF8: Int64 = 2
comptime GEMINI_KERNEL_STATUS_CAPACITY: Int64 = 3
comptime GEMINI_KERNEL_STATUS_ABI: Int64 = 4

comptime GEMINI_RESPONSE_CREATED: Int64 = 1
comptime GEMINI_RESPONSE_COMPLETED: Int64 = 2
comptime GEMINI_RESPONSE_INCOMPLETE: Int64 = 3
comptime GEMINI_RESPONSE_METADATA: Int64 = 4
comptime GEMINI_OUTPUT_ITEM_ADDED: Int64 = 5
comptime GEMINI_OUTPUT_ITEM_DONE: Int64 = 6
comptime GEMINI_FUNCTION_CALL_ARGUMENTS_DELTA: Int64 = 7
comptime GEMINI_OUTPUT_TEXT_DELTA: Int64 = 8
comptime GEMINI_REASONING_SUMMARY_PART_ADDED: Int64 = 9
comptime GEMINI_REASONING_SUMMARY_TEXT_DELTA: Int64 = 10
comptime GEMINI_TEXT_SOURCE: Int64 = 11
comptime GEMINI_REASONING_SOURCE: Int64 = 12
comptime GEMINI_FUNCTION_CALL_SOURCE: Int64 = 13
comptime GEMINI_OUTPUT_TEXT_CONTENT: Int64 = 14
comptime GEMINI_MESSAGE_ITEM: Int64 = 15
comptime GEMINI_OUTPUT_MESSAGE_ITEM: Int64 = 16
comptime GEMINI_RESPONSE_VALUE: Int64 = 17
comptime GEMINI_FUNCTION_CALL_ITEM: Int64 = 18
comptime GEMINI_RAW_FUNCTION_CALL_ITEM: Int64 = 19
comptime GEMINI_ADDED_FUNCTION_CALL_ITEM: Int64 = 20
comptime GEMINI_CHAT_FUNCTION_CALL_ITEM: Int64 = 21
comptime GEMINI_RESPONSE_USAGE: Int64 = 22
comptime GEMINI_STREAM_TEXT_DELTA: Int64 = 23
comptime GEMINI_STREAM_REASONING_DELTA: Int64 = 24
comptime GEMINI_FUNCTION_CALL_ARGUMENTS_DELTA_WITHOUT_SEQUENCE: Int64 = 25
comptime GEMINI_BUFFERED_RESPONSE: Int64 = 26
comptime GEMINI_CITATION_TEXT: Int64 = 27
comptime GEMINI_WEB_SEARCH_CALL: Int64 = 28
comptime GEMINI_STREAM_ASSISTANT_MESSAGE: Int64 = 29
comptime GEMINI_STREAM_OUTPUT_ITEMS: Int64 = 30


@fieldwise_init
struct ProdexGeminiResponseKernelInput(Copyable):
    var operation: Int64
    var sequence_number: UInt64
    var created_at: UInt64
    var summary_index: UInt64
    var prompt_token_count: UInt64
    var candidate_token_count: UInt64
    var total_token_count: UInt64
    var total_token_count_present: Int64
    var cached_content_token_count: UInt64
    var thoughts_token_count: UInt64
    var tool_use_prompt_token_count: UInt64
    var response_id_present: Int64
    var call_id_present: Int64
    var model_present: Int64
    var usage_present: Int64
    var metadata_present: Int64
    var signature_present: Int64
    var namespace_present: Int64
    var response_id: ProdexRichStringView
    var call_id: ProdexRichStringView
    var name: ProdexRichStringView
    var delta: ProdexRichStringView
    var reason: ProdexRichStringView
    var message: ProdexRichStringView
    var item: ProdexRichStringView
    var metadata: ProdexRichStringView
    var response: ProdexRichStringView
    var content: ProdexRichStringView
    var output: ProdexRichStringView
    var model: ProdexRichStringView
    var usage: ProdexRichStringView
    var signature: ProdexRichStringView
    var namespace: ProdexRichStringView
    var arguments: ProdexRichStringView
    var created_at_present: Int64
    var include_empty_usage: Int64
    var include_empty_metadata: Int64
    var citations: ProdexRichStringView
    var reason_present: Int64


@fieldwise_init
struct GeminiResponseWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def gemini_put_byte(
    writer: Pointer[mut=True, GeminiResponseWriter, _], value: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True


def gemini_put_literal(
    writer: Pointer[mut=True, GeminiResponseWriter, _], value: StringSlice
) -> Bool:
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not gemini_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def gemini_put_hex_byte(
    writer: Pointer[mut=True, GeminiResponseWriter, _], value: UInt8
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
    return gemini_put_byte(writer, high) and gemini_put_byte(writer, low)


def gemini_put_json_escaped(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if view.len > 0:
        var ptr = rich_view_ptr(view)
        for index in range(Int64(view.len)):
            var value = ptr[unsafe_offset=index]
            if value == 34 or value == 92:
                if not gemini_put_byte(writer, 92) or not gemini_put_byte(writer, value):
                    return False
            elif value == 8:
                if not gemini_put_literal(writer, StringSlice("\\b")):
                    return False
            elif value == 9:
                if not gemini_put_literal(writer, StringSlice("\\t")):
                    return False
            elif value == 10:
                if not gemini_put_literal(writer, StringSlice("\\n")):
                    return False
            elif value == 12:
                if not gemini_put_literal(writer, StringSlice("\\f")):
                    return False
            elif value == 13:
                if not gemini_put_literal(writer, StringSlice("\\r")):
                    return False
            elif value < 32:
                if not gemini_put_literal(writer, StringSlice("\\u00")) or not gemini_put_hex_byte(writer, value):
                    return False
            elif not gemini_put_byte(writer, value):
                return False
    return True


def gemini_put_json_string(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    return gemini_put_byte(writer, 34) and gemini_put_json_escaped(writer, view) and gemini_put_byte(writer, 34)


def gemini_put_json_string_prefix(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    prefix: StringSlice,
    view: ProdexRichStringView,
) -> Bool:
    return gemini_put_byte(writer, 34) and gemini_put_literal(writer, prefix) and gemini_put_json_escaped(writer, view) and gemini_put_byte(writer, 34)


def gemini_put_view(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if view.len == 0:
        return True
    var ptr = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        if not gemini_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def gemini_put_view_range(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    if end == start:
        return True
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        if not gemini_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def gemini_put_array_items(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if view.len < 2:
        return False
    var ptr = rich_view_ptr(view)
    if ptr[unsafe_offset=0] != 91 or ptr[unsafe_offset=Int64(view.len) - 1] != 93:
        return False
    return gemini_put_view_range(writer, view, 1, Int64(view.len) - 1)


def gemini_put_u64(
    writer: Pointer[mut=True, GeminiResponseWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return gemini_put_byte(writer, 48)
    var divisor: UInt64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        if not gemini_put_byte(writer, UInt8(remaining / divisor) + 48):
            return False
        remaining %= divisor
        divisor /= 10
    return True


def gemini_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if left > 18_446_744_073_709_551_615 - right:
        return 18_446_744_073_709_551_615
    return left + right


def gemini_put_event_prefix(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    event_type: StringSlice,
    sequence_number: UInt64,
    created_at: UInt64,
    include_created_at: Bool,
) -> Bool:
    if not gemini_put_literal(writer, StringSlice('{"type":"')):
        return False
    if not gemini_put_literal(writer, event_type):
        return False
    if not gemini_put_literal(writer, StringSlice('","sequence_number":')):
        return False
    if not gemini_put_u64(writer, sequence_number):
        return False
    if include_created_at:
        if not gemini_put_literal(writer, StringSlice(',"created_at":')):
            return False
        if not gemini_put_u64(writer, created_at):
            return False
    return True


def gemini_views_valid(input: ProdexGeminiResponseKernelInput) -> Bool:
    return (
        input.operation >= GEMINI_RESPONSE_CREATED
        and input.operation <= GEMINI_STREAM_OUTPUT_ITEMS
        and input.response_id_present >= 0
        and input.response_id_present <= 1
        and input.call_id_present >= 0
        and input.call_id_present <= 1
        and input.model_present >= 0
        and input.model_present <= 1
        and input.usage_present >= 0
        and input.usage_present <= 1
        and input.metadata_present >= 0
        and input.metadata_present <= 1
        and input.signature_present >= 0
        and input.signature_present <= 1
        and input.namespace_present >= 0
        and input.namespace_present <= 1
        and input.total_token_count_present >= 0
        and input.total_token_count_present <= 1
        and input.created_at_present >= 0
        and input.created_at_present <= 1
        and input.include_empty_usage >= 0
        and input.include_empty_usage <= 1
        and input.include_empty_metadata >= 0
        and input.include_empty_metadata <= 1
        and input.reason_present >= 0
        and input.reason_present <= 1
        and rich_view_valid(input.response_id, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.call_id, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.name, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.delta, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.reason, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.message, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.item, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.metadata, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.response, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.content, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.output, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.model, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.usage, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.signature, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.namespace, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.arguments, GEMINI_KERNEL_MAX_BYTES)
        and rich_view_valid(input.citations, GEMINI_KERNEL_MAX_BYTES)
    )


def gemini_put_tool_item(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if not gemini_put_literal(writer, StringSlice('{"type":"function_call","call_id":')):
        return False
    if not gemini_put_json_string(writer, input.call_id):
        return False
    if not gemini_put_literal(writer, StringSlice(',"name":')) or not gemini_put_json_string(writer, input.name):
        return False
    if not gemini_put_literal(writer, StringSlice(',"arguments":')) or not gemini_put_json_string(writer, input.arguments):
        return False
    if input.namespace_present == 1:
        if not gemini_put_literal(writer, StringSlice(',"namespace":')) or not gemini_put_json_string(writer, input.namespace):
            return False
    if input.signature_present == 1:
        if not gemini_put_literal(writer, StringSlice(',"gemini_thought_signature":')) or not gemini_put_json_string(writer, input.signature):
            return False
    return gemini_put_byte(writer, 125)


def gemini_put_buffered_message(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if not gemini_put_literal(writer, StringSlice('{"type":"message","role":"assistant","content":[')):
        return False
    var first = True
    if input.delta.len > 0:
        if not gemini_put_literal(writer, StringSlice('{"type":"output_text","text":')) or not gemini_put_json_string(writer, input.delta) or not gemini_put_byte(writer, 125):
            return False
        first = False
    if input.content.len > 0:
        if not first and not gemini_put_byte(writer, 44):
            return False
        if not gemini_put_array_items(writer, input.content):
            return False
    return gemini_put_literal(writer, StringSlice("]}"))


def gemini_put_buffered_response(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if not gemini_put_literal(writer, StringSlice('{"id":')) or not gemini_put_json_string(writer, input.response_id):
        return False
    if not gemini_put_literal(writer, StringSlice(',"object":"response","model":')) or not gemini_put_json_string(writer, input.model):
        return False
    if not gemini_put_literal(writer, StringSlice(',"output":[')):
        return False
    var has_message = input.delta.len > 0 or input.content.len > 0
    var output_has_items = input.output.len > 2
    if has_message:
        if not gemini_put_buffered_message(writer, input):
            return False
        if output_has_items and not gemini_put_byte(writer, 44):
            return False
    if not gemini_put_array_items(writer, input.output):
        return False
    if input.citations.len > 0:
        if (has_message or output_has_items) and not gemini_put_byte(writer, 44):
            return False
        if not gemini_put_literal(writer, StringSlice('{"type":"message","role":"assistant","content":[{"type":"output_text","text":')):
            return False
        if not gemini_put_json_string(writer, input.citations) or not gemini_put_literal(writer, StringSlice("}]}")):
            return False
    if not gemini_put_byte(writer, 93):
        return False
    if input.created_at_present == 1:
        if not gemini_put_literal(writer, StringSlice(',"created_at":')) or not gemini_put_u64(writer, input.created_at):
            return False
    if input.usage.len > 0:
        if not gemini_put_literal(writer, StringSlice(',"usage":')) or not gemini_put_view(writer, input.usage):
            return False
    elif input.include_empty_usage == 1 and not gemini_put_literal(writer, StringSlice(',"usage":{}')):
        return False
    if input.metadata.len > 0:
        if not gemini_put_literal(writer, StringSlice(',"metadata":')) or not gemini_put_view(writer, input.metadata):
            return False
    elif input.include_empty_metadata == 1 and not gemini_put_literal(writer, StringSlice(',"metadata":{}')):
        return False
    return gemini_put_byte(writer, 125)


def gemini_put_web_search_call(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if not gemini_put_literal(writer, StringSlice('{"type":"web_search_call","id":')):
        return False
    if not gemini_put_json_string_prefix(writer, StringSlice("ws_"), input.response_id):
        return False
    if not gemini_put_literal(writer, StringSlice(',"status":"completed","action":')):
        return False
    if input.delta.len > 0:
        return (
            gemini_put_literal(writer, StringSlice('{"type":"open_page","url":'))
            and gemini_put_json_string(writer, input.delta)
            and gemini_put_literal(writer, StringSlice(',"sources":'))
            and gemini_put_view(writer, input.output)
            and gemini_put_literal(writer, StringSlice("}}"))
        )
    return (
        gemini_put_literal(writer, StringSlice('{"type":"search","queries":'))
        and gemini_put_view(writer, input.content)
        and gemini_put_literal(writer, StringSlice(',"sources":'))
        and gemini_put_view(writer, input.output)
        and gemini_put_literal(writer, StringSlice("}}"))
    )


def gemini_put_stream_assistant_message(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if not gemini_put_literal(writer, StringSlice('{"role":"assistant","content":')):
        return False
    if input.delta.len > 0:
        if not gemini_put_json_string(writer, input.delta):
            return False
    elif input.arguments.len > 0:
        if not gemini_put_literal(writer, StringSlice('""')):
            return False
    elif not gemini_put_literal(writer, StringSlice("null")):
        return False
    if input.reason.len > 0:
        if not gemini_put_literal(writer, StringSlice(',"reasoning_content":')) or not gemini_put_json_string(writer, input.reason):
            return False
    if input.content.len > 0:
        if not gemini_put_literal(writer, StringSlice(',"gemini_media_content":')) or not gemini_put_view(writer, input.content):
            return False
    if input.item.len > 0:
        if not gemini_put_literal(writer, StringSlice(',"gemini_native_parts":')) or not gemini_put_view(writer, input.item):
            return False
    if input.output.len > 0:
        if not gemini_put_literal(writer, StringSlice(',"gemini_image_generation":')) or not gemini_put_view(writer, input.output):
            return False
    if input.metadata.len > 0:
        if not gemini_put_literal(writer, StringSlice(',"gemini_metadata":')) or not gemini_put_view(writer, input.metadata):
            return False
    if input.arguments.len > 0:
        if not gemini_put_literal(writer, StringSlice(',"tool_calls":')) or not gemini_put_view(writer, input.arguments):
            return False
    return gemini_put_byte(writer, 125)


def gemini_put_stream_output_items(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if not gemini_put_byte(writer, 91):
        return False
    var first = True
    if input.response.len > 0:
        if not gemini_put_view(writer, input.response):
            return False
        first = False
    if input.output.len > 2:
        if not first and not gemini_put_byte(writer, 44):
            return False
        if not gemini_put_array_items(writer, input.output):
            return False
        first = False
    if input.delta.len > 0 or input.content.len > 0:
        if not first and not gemini_put_byte(writer, 44):
            return False
        if not gemini_put_literal(writer, StringSlice('{"type":"message","role":"assistant","content":[')):
            return False
        var content_first = True
        if input.delta.len > 0:
            if not gemini_put_literal(writer, StringSlice('{"type":"output_text","text":')) or not gemini_put_json_string(writer, input.delta) or not gemini_put_byte(writer, 125):
                return False
            content_first = False
        if input.content.len > 0:
            if not content_first and not gemini_put_byte(writer, 44):
                return False
            if not gemini_put_array_items(writer, input.content):
                return False
        if not gemini_put_literal(writer, StringSlice("]}")):
            return False
        first = False
    if input.reason_present == 1:
        if not first and not gemini_put_byte(writer, 44):
            return False
        if not gemini_put_literal(writer, StringSlice('{"type":"message","role":"assistant","content":[{"type":"output_text","text":')):
            return False
        if not gemini_put_json_string(writer, input.reason) or not gemini_put_literal(writer, StringSlice("}]}")):
            return False
        first = False
    if input.arguments.len > 2:
        if not first and not gemini_put_byte(writer, 44):
            return False
        if not gemini_put_array_items(writer, input.arguments):
            return False
    return gemini_put_byte(writer, 93)


def gemini_write_operation(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    var operation = input.operation
    if operation == GEMINI_RESPONSE_CREATED:
        return (
            gemini_put_event_prefix(writer, StringSlice("response.created"), input.sequence_number, input.created_at, True)
            and gemini_put_literal(writer, StringSlice(',"response":{"id":'))
            and gemini_put_json_string(writer, input.response_id)
            and gemini_put_literal(writer, StringSlice("}}"))
        )
    if operation == GEMINI_RESPONSE_COMPLETED:
        return (
            gemini_put_event_prefix(writer, StringSlice("response.completed"), input.sequence_number, input.created_at, True)
            and gemini_put_literal(writer, StringSlice(',"response":'))
            and gemini_put_view(writer, input.response)
            and gemini_put_byte(writer, 125)
        )
    if operation == GEMINI_RESPONSE_INCOMPLETE:
        return (
            gemini_put_event_prefix(writer, StringSlice("response.incomplete"), input.sequence_number, input.created_at, True)
            and gemini_put_literal(writer, StringSlice(',"response":{"id":'))
            and gemini_put_json_string(writer, input.response_id)
            and gemini_put_literal(writer, StringSlice(',"status":"incomplete","incomplete_details":{"reason":'))
            and gemini_put_json_string(writer, input.reason)
            and gemini_put_literal(writer, StringSlice(',"message":'))
            and gemini_put_json_string(writer, input.message)
            and gemini_put_literal(writer, StringSlice("}}}"))
        )
    if operation == GEMINI_RESPONSE_METADATA:
        return (
            gemini_put_event_prefix(writer, StringSlice("response.metadata"), input.sequence_number, input.created_at, True)
            and gemini_put_literal(writer, StringSlice(',"response_id":'))
            and gemini_put_json_string(writer, input.response_id)
            and gemini_put_literal(writer, StringSlice(',"metadata":'))
            and gemini_put_view(writer, input.metadata)
            and gemini_put_byte(writer, 125)
        )
    if operation == GEMINI_OUTPUT_ITEM_ADDED:
        return (
            gemini_put_event_prefix(writer, StringSlice("response.output_item.added"), input.sequence_number, input.created_at, False)
            and gemini_put_literal(writer, StringSlice(',"item":'))
            and gemini_put_view(writer, input.item)
            and gemini_put_byte(writer, 125)
        )
    if operation == GEMINI_OUTPUT_ITEM_DONE:
        if not gemini_put_event_prefix(writer, StringSlice("response.output_item.done"), input.sequence_number, input.created_at, False):
            return False
        if not gemini_put_literal(writer, StringSlice(',"item":')) or not gemini_put_view(writer, input.item):
            return False
        if input.response_id_present == 1:
            if not gemini_put_literal(writer, StringSlice(',"response_id":')) or not gemini_put_json_string(writer, input.response_id):
                return False
        return gemini_put_byte(writer, 125)
    if operation == GEMINI_FUNCTION_CALL_ARGUMENTS_DELTA:
        if not gemini_put_event_prefix(writer, StringSlice("response.function_call_arguments.delta"), input.sequence_number, input.created_at, False):
            return False
        if input.call_id_present == 1:
            if not gemini_put_literal(writer, StringSlice(',"call_id":')) or not gemini_put_json_string(writer, input.call_id):
                return False
        return (
            gemini_put_literal(writer, StringSlice(',"delta":'))
            and gemini_put_json_string(writer, input.delta)
            and gemini_put_byte(writer, 125)
        )
    if operation == GEMINI_FUNCTION_CALL_ARGUMENTS_DELTA_WITHOUT_SEQUENCE:
        if not gemini_put_literal(
            writer, StringSlice('{"type":"response.function_call_arguments.delta"')
        ):
            return False
        if input.call_id_present == 1:
            if not gemini_put_literal(writer, StringSlice(',"call_id":')) or not gemini_put_json_string(writer, input.call_id):
                return False
        return (
            gemini_put_literal(writer, StringSlice(',"delta":'))
            and gemini_put_json_string(writer, input.delta)
            and gemini_put_byte(writer, 125)
        )
    if operation == GEMINI_OUTPUT_TEXT_DELTA:
        return (
            gemini_put_event_prefix(writer, StringSlice("response.output_text.delta"), input.sequence_number, input.created_at, True)
            and gemini_put_literal(writer, StringSlice(',"response_id":'))
            and gemini_put_json_string(writer, input.response_id)
            and gemini_put_literal(writer, StringSlice(',"delta":'))
            and gemini_put_json_string(writer, input.delta)
            and gemini_put_byte(writer, 125)
        )
    if operation == GEMINI_REASONING_SUMMARY_PART_ADDED:
        return (
            gemini_put_event_prefix(writer, StringSlice("response.reasoning_summary_part.added"), input.sequence_number, input.created_at, False)
            and gemini_put_literal(writer, StringSlice(',"response_id":'))
            and gemini_put_json_string(writer, input.response_id)
            and gemini_put_literal(writer, StringSlice(',"summary_index":'))
            and gemini_put_u64(writer, input.summary_index)
            and gemini_put_byte(writer, 125)
        )
    if operation == GEMINI_REASONING_SUMMARY_TEXT_DELTA:
        return (
            gemini_put_event_prefix(writer, StringSlice("response.reasoning_summary_text.delta"), input.sequence_number, input.created_at, False)
            and gemini_put_literal(writer, StringSlice(',"response_id":'))
            and gemini_put_json_string(writer, input.response_id)
            and gemini_put_literal(writer, StringSlice(',"summary_index":'))
            and gemini_put_u64(writer, input.summary_index)
            and gemini_put_literal(writer, StringSlice(',"delta":'))
            and gemini_put_json_string(writer, input.delta)
            and gemini_put_byte(writer, 125)
        )
    if operation == GEMINI_TEXT_SOURCE or operation == GEMINI_REASONING_SOURCE:
        if not gemini_put_literal(writer, StringSlice('{"candidates":[{"content":{"parts":[{"text":')):
            return False
        if not gemini_put_json_string(writer, input.delta):
            return False
        if operation == GEMINI_REASONING_SOURCE and not gemini_put_literal(writer, StringSlice(',"thought":true')):
            return False
        return gemini_put_literal(writer, StringSlice("}]}}]}"))
    if operation == GEMINI_FUNCTION_CALL_SOURCE:
        return (
            gemini_put_literal(writer, StringSlice('{"candidates":[{"content":{"parts":[{"functionCall":{"id":'))
            and gemini_put_json_string(writer, input.call_id)
            and gemini_put_literal(writer, StringSlice(',"name":'))
            and gemini_put_json_string(writer, input.name)
            and gemini_put_literal(writer, StringSlice(',"args":'))
            and gemini_put_view(writer, input.arguments)
            and gemini_put_literal(writer, StringSlice("}}]}}]}"))
        )
    if operation == GEMINI_OUTPUT_TEXT_CONTENT:
        return (
            gemini_put_literal(writer, StringSlice('{"type":"output_text","text":'))
            and gemini_put_json_string(writer, input.delta)
            and gemini_put_byte(writer, 125)
        )
    if operation == GEMINI_MESSAGE_ITEM:
        return (
            gemini_put_literal(writer, StringSlice('{"id":'))
            and gemini_put_json_string(writer, input.response_id)
            and gemini_put_literal(writer, StringSlice(',"type":"message","role":"assistant","content":'))
            and gemini_put_view(writer, input.content)
            and gemini_put_byte(writer, 125)
        )
    if operation == GEMINI_OUTPUT_MESSAGE_ITEM:
        return (
            gemini_put_literal(writer, StringSlice('{"type":"message","role":"assistant","content":'))
            and gemini_put_view(writer, input.content)
            and gemini_put_byte(writer, 125)
        )
    if operation == GEMINI_BUFFERED_RESPONSE:
        return gemini_put_buffered_response(writer, input)
    if operation == GEMINI_CITATION_TEXT:
        return gemini_put_json_string_prefix(writer, StringSlice("Citations:\\n"), input.delta)
    if operation == GEMINI_WEB_SEARCH_CALL:
        return gemini_put_web_search_call(writer, input)
    if operation == GEMINI_STREAM_ASSISTANT_MESSAGE:
        return gemini_put_stream_assistant_message(writer, input)
    if operation == GEMINI_STREAM_OUTPUT_ITEMS:
        return gemini_put_stream_output_items(writer, input)
    if operation == GEMINI_RESPONSE_VALUE:
        if not gemini_put_literal(writer, StringSlice('{"id":')) or not gemini_put_json_string(writer, input.response_id):
            return False
        if not gemini_put_literal(writer, StringSlice(',"output":')) or not gemini_put_view(writer, input.output):
            return False
        if input.model_present == 1:
            if not gemini_put_literal(writer, StringSlice(',"model":')) or not gemini_put_json_string(writer, input.model):
                return False
        if input.usage_present == 1:
            if not gemini_put_literal(writer, StringSlice(',"usage":')) or not gemini_put_view(writer, input.usage):
                return False
        if input.metadata_present == 1:
            if not gemini_put_literal(writer, StringSlice(',"metadata":')) or not gemini_put_view(writer, input.metadata):
                return False
        return gemini_put_byte(writer, 125)
    if operation == GEMINI_FUNCTION_CALL_ITEM:
        return gemini_put_tool_item(writer, input)
    if operation == GEMINI_RAW_FUNCTION_CALL_ITEM:
        return gemini_put_tool_item(writer, input)
    if operation == GEMINI_ADDED_FUNCTION_CALL_ITEM:
        if not gemini_put_literal(writer, StringSlice('{"type":"function_call","call_id":')):
            return False
        if not gemini_put_json_string(writer, input.call_id) or not gemini_put_literal(writer, StringSlice(',"name":')):
            return False
        if not gemini_put_json_string(writer, input.name):
            return False
        if input.namespace_present == 1:
            if not gemini_put_literal(writer, StringSlice(',"namespace":')) or not gemini_put_json_string(writer, input.namespace):
                return False
        if input.signature_present == 1:
            if not gemini_put_literal(writer, StringSlice(',"gemini_thought_signature":')) or not gemini_put_json_string(writer, input.signature):
                return False
        return gemini_put_byte(writer, 125)
    if operation == GEMINI_CHAT_FUNCTION_CALL_ITEM:
        if not gemini_put_literal(writer, StringSlice('{"id":')) or not gemini_put_json_string(writer, input.call_id):
            return False
        if not gemini_put_literal(writer, StringSlice(',"type":"function","function":{"name":')):
            return False
        if not gemini_put_json_string(writer, input.name) or not gemini_put_literal(writer, StringSlice(',"arguments":')):
            return False
        if not gemini_put_json_string(writer, input.arguments) or not gemini_put_literal(writer, StringSlice("}")):
            return False
        if input.signature_present == 1:
            if not gemini_put_literal(writer, StringSlice(',"gemini_thought_signature":')) or not gemini_put_json_string(writer, input.signature):
                return False
        return gemini_put_byte(writer, 125)
    if operation == GEMINI_RESPONSE_USAGE:
        var total = input.total_token_count
        if input.total_token_count_present == 0:
            total = gemini_saturating_add(
                input.prompt_token_count, input.candidate_token_count
            )
        return (
            gemini_put_literal(writer, StringSlice('{"input_tokens":'))
            and gemini_put_u64(writer, input.prompt_token_count)
            and gemini_put_literal(writer, StringSlice(',"input_tokens_details":{"cached_tokens":'))
            and gemini_put_u64(writer, input.cached_content_token_count)
            and gemini_put_literal(writer, StringSlice(',"tool_tokens":'))
            and gemini_put_u64(writer, input.tool_use_prompt_token_count)
            and gemini_put_literal(writer, StringSlice('},"output_tokens":'))
            and gemini_put_u64(writer, input.candidate_token_count)
            and gemini_put_literal(writer, StringSlice(',"output_tokens_details":{"reasoning_tokens":'))
            and gemini_put_u64(writer, input.thoughts_token_count)
            and gemini_put_literal(writer, StringSlice('},"total_tokens":'))
            and gemini_put_u64(writer, total)
            and gemini_put_literal(writer, StringSlice("}"))
        )
    if operation == GEMINI_STREAM_TEXT_DELTA or operation == GEMINI_STREAM_REASONING_DELTA:
        if operation == GEMINI_STREAM_TEXT_DELTA:
            if not gemini_put_literal(writer, StringSlice('{"type":"response.output_text.delta","delta":')):
                return False
        elif not gemini_put_literal(writer, StringSlice('{"type":"response.reasoning_summary_text.delta","delta":')):
            return False
        return gemini_put_json_string(writer, input.delta) and gemini_put_byte(writer, 125)
    return False


def gemini_response_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return GEMINI_KERNEL_STATUS_ABI
    if input_address == 0 or output_address == 0 or written_address == 0 or output_capacity <= 0:
        return GEMINI_KERNEL_STATUS_INVALID
    var input = Pointer[
        mut=False, ProdexGeminiResponseKernelInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not gemini_views_valid(input[].copy()):
        return GEMINI_KERNEL_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = GeminiResponseWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    if not gemini_write_operation(writer_ptr, input[].copy()):
        if writer.written >= output_capacity:
            written[] = writer.written
            return GEMINI_KERNEL_STATUS_CAPACITY
        return GEMINI_KERNEL_STATUS_INVALID
    written[] = writer.written
    return GEMINI_KERNEL_STATUS_OK
