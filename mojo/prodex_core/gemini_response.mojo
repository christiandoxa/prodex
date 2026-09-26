from std.collections import Array

from std.memory import Pointer

from json_view import (
    deepseek_json_byte,
    deepseek_json_object_member,
    deepseek_json_raw_equals,
    deepseek_json_skip_ws,
    deepseek_json_value_end,
)
from rich_text import rich_trim_bounds, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime PRODEX_RICH_ABI_VERSION: Int64 = 6

comptime GEMINI_KERNEL_MAX_BYTES: Int64 = 4_194_304
comptime GEMINI_BUFFERED_RESPONSE_ABI_VERSION: Int64 = 2
comptime GEMINI_BUFFERED_RESPONSE_MAX_BYTES: Int64 = 16_777_216
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
comptime GEMINI_TOOL_SEARCH_CALL_ITEM: Int64 = 31
comptime GEMINI_CUSTOM_TOOL_CALL_ITEM: Int64 = 32
comptime GEMINI_FINISH_REASON_FAILURE: Int64 = 33
comptime GEMINI_FINISH_REASON_INCOMPLETE: Int64 = 34
comptime GEMINI_PROMPT_FEEDBACK_FAILURE: Int64 = 35
comptime GEMINI_STREAM_FUNCTION_CALL_DELTA: Int64 = 36
comptime GEMINI_STREAM_TOOL_CALL: Int64 = 37
comptime GEMINI_STREAM_OUTPUT_TEXT_ITEM_ID: Int64 = 38
comptime GEMINI_STREAM_MEDIA_ITEM_ID: Int64 = 39
comptime GEMINI_STREAM_CITATION_ITEM_ID: Int64 = 40
comptime GEMINI_STREAM_FALLBACK_RESPONSE_ID: Int64 = 41
comptime GEMINI_STREAM_FALLBACK_TOOL_CALL_ID: Int64 = 42
comptime GEMINI_STREAM_SHOULD_EMIT_ARGUMENTS_DELTA: Int64 = 43
comptime GEMINI_STREAM_RESPONSE_ID: Int64 = 44
comptime GEMINI_RAW_TEXT_RESPONSE: Int64 = 45
comptime GEMINI_STREAM_EVENT_TRANSFORM: Int64 = 46


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


def gemini_put_json_escaped_range(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    if end > start:
        var ptr = rich_view_ptr(view)
        for index in range(start, end):
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


def gemini_put_json_escaped(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    return gemini_put_json_escaped_range(writer, view, 0, Int64(view.len))


def gemini_put_json_string(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    return gemini_put_byte(writer, 34) and gemini_put_json_escaped(writer, view) and gemini_put_byte(writer, 34)


def gemini_put_json_string_range(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    return gemini_put_byte(writer, 34) and gemini_put_json_escaped_range(writer, view, start, end) and gemini_put_byte(writer, 34)


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


def gemini_views_valid(
    input: ProdexGeminiResponseKernelInput, maximum_bytes: Int64
) -> Bool:
    return (
        input.operation >= GEMINI_RESPONSE_CREATED
        and input.operation <= GEMINI_STREAM_EVENT_TRANSFORM
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
        and rich_view_valid(input.response_id, maximum_bytes)
        and rich_view_valid(input.call_id, maximum_bytes)
        and rich_view_valid(input.name, maximum_bytes)
        and rich_view_valid(input.delta, maximum_bytes)
        and rich_view_valid(input.reason, maximum_bytes)
        and rich_view_valid(input.message, maximum_bytes)
        and rich_view_valid(input.item, maximum_bytes)
        and rich_view_valid(input.metadata, maximum_bytes)
        and rich_view_valid(input.response, maximum_bytes)
        and rich_view_valid(input.content, maximum_bytes)
        and rich_view_valid(input.output, maximum_bytes)
        and rich_view_valid(input.model, maximum_bytes)
        and rich_view_valid(input.usage, maximum_bytes)
        and rich_view_valid(input.signature, maximum_bytes)
        and rich_view_valid(input.namespace, maximum_bytes)
        and rich_view_valid(input.arguments, maximum_bytes)
        and rich_view_valid(input.citations, maximum_bytes)
    )


def gemini_tool_name_split(view: ProdexRichStringView) -> Array[Int64, 2]:
    var result = Array[Int64, 2](fill=-1)
    var length = Int64(view.len)
    if length < 2:
        return result^
    var ptr = rich_view_ptr(view)
    var index = length - 2
    while index >= 0:
        if ptr[unsafe_offset=index] == 45 and ptr[unsafe_offset=index + 1] == 45:
            var prefix = ProdexRichStringView(view.ptr, UInt(index))
            var suffix = ProdexRichStringView(
                view.ptr + UInt(index + 2), UInt(length - index - 2)
            )
            var prefix_bounds = rich_trim_bounds(prefix)
            var suffix_bounds = rich_trim_bounds(suffix)
            if prefix_bounds[0] < prefix_bounds[1] and suffix_bounds[0] < suffix_bounds[1]:
                result[0] = index
                result[1] = index + 2
                return result^
        index -= 1
    if length < 5 or ptr[unsafe_offset=0] != 109 or ptr[unsafe_offset=1] != 99 or ptr[unsafe_offset=2] != 112 or ptr[unsafe_offset=3] != 95 or ptr[unsafe_offset=4] != 95:
        return result^
    index = length - 2
    while index >= 5:
        if ptr[unsafe_offset=index] == 95 and ptr[unsafe_offset=index + 1] == 95:
            var suffix = ProdexRichStringView(
                view.ptr + UInt(index + 2), UInt(length - index - 2)
            )
            var suffix_bounds = rich_trim_bounds(suffix)
            if suffix_bounds[0] < suffix_bounds[1]:
                result[0] = index
                result[1] = index + 2
                return result^
        index -= 1
    return result^


def gemini_view_equals(view: ProdexRichStringView, literal: StringSlice) -> Bool:
    if view.len != UInt(literal.byte_length()):
        return False
    var actual = rich_view_ptr(view)
    var expected = literal.unsafe_ptr()
    for index in range(Int64(view.len)):
        if actual[unsafe_offset=index] != expected[unsafe_offset=index]:
            return False
    return True


def gemini_view_starts_with(view: ProdexRichStringView, prefix: StringSlice) -> Bool:
    if view.len < UInt(prefix.byte_length()):
        return False
    var actual = rich_view_ptr(view)
    var expected = prefix.unsafe_ptr()
    for index in range(Int64(prefix.byte_length())):
        if actual[unsafe_offset=index] != expected[unsafe_offset=index]:
            return False
    return True


def gemini_put_reason_result(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    code: StringSlice,
    prefix: StringSlice,
    reason: ProdexRichStringView,
) -> Bool:
    return (
        gemini_put_literal(writer, StringSlice('["'))
        and gemini_put_literal(writer, code)
        and gemini_put_literal(writer, StringSlice('","'))
        and gemini_put_literal(writer, prefix)
        and gemini_put_json_escaped(writer, reason)
        and gemini_put_literal(writer, StringSlice('"]'))
    )


def gemini_put_finish_reason_failure(
    writer: Pointer[mut=True, GeminiResponseWriter, _], reason: ProdexRichStringView
) -> Bool:
    var code = StringSlice("")
    if gemini_view_equals(reason, StringSlice("MALFORMED_FUNCTION_CALL")):
        code = StringSlice("gemini_malformed_function_call")
    elif gemini_view_equals(reason, StringSlice("UNEXPECTED_TOOL_CALL")):
        code = StringSlice("gemini_unexpected_tool_call")
    elif gemini_view_equals(reason, StringSlice("OTHER")):
        code = StringSlice("gemini_finish_other")
    elif gemini_view_equals(reason, StringSlice("NO_IMAGE")):
        code = StringSlice("gemini_no_image")
    elif gemini_view_equals(reason, StringSlice("SAFETY")) or gemini_view_equals(reason, StringSlice("RECITATION")) or gemini_view_equals(reason, StringSlice("LANGUAGE")) or gemini_view_equals(reason, StringSlice("BLOCKLIST")) or gemini_view_equals(reason, StringSlice("PROHIBITED_CONTENT")) or gemini_view_equals(reason, StringSlice("SPII")) or gemini_view_equals(reason, StringSlice("IMAGE_SAFETY")) or gemini_view_equals(reason, StringSlice("IMAGE_PROHIBITED_CONTENT")):
        code = StringSlice("invalid_prompt")
    else:
        return gemini_put_literal(writer, StringSlice("null"))
    return gemini_put_reason_result(
        writer, code, StringSlice("Gemini ended the stream with finishReason="), reason
    )


def gemini_put_tool_name(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if input.namespace_present == 1:
        return gemini_put_json_string(writer, input.name)
    var bounds = gemini_tool_name_split(input.name)
    if bounds[0] < 0:
        return gemini_put_json_string(writer, input.name)
    return gemini_put_json_string_range(writer, input.name, bounds[1], Int64(input.name.len))


def gemini_put_tool_namespace(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if input.namespace_present == 1:
        return gemini_put_literal(writer, StringSlice(',"namespace":')) and gemini_put_json_string(writer, input.namespace)
    var bounds = gemini_tool_name_split(input.name)
    if bounds[0] < 0:
        return True
    return (
        gemini_put_literal(writer, StringSlice(',"namespace":'))
        and gemini_put_json_string_range(writer, input.name, 0, bounds[0])
    )


def gemini_put_tool_item(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if not gemini_put_literal(writer, StringSlice('{"type":"function_call","call_id":')):
        return False
    if not gemini_put_json_string(writer, input.call_id):
        return False
    if not gemini_put_literal(writer, StringSlice(',"name":')) or not gemini_put_tool_name(writer, input):
        return False
    if not gemini_put_literal(writer, StringSlice(',"arguments":')) or not gemini_put_json_string(writer, input.arguments):
        return False
    if not gemini_put_tool_namespace(writer, input):
        return False
    if input.signature_present == 1:
        if not gemini_put_literal(writer, StringSlice(',"gemini_thought_signature":')) or not gemini_put_json_string(writer, input.signature):
            return False
    return gemini_put_byte(writer, 125)


def gemini_put_tool_search_item(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    return (
        gemini_put_literal(writer, StringSlice('{"type":"tool_search_call","call_id":'))
        and gemini_put_json_string(writer, input.call_id)
        and gemini_put_literal(writer, StringSlice(',"execution":"client","arguments":'))
        and gemini_put_view(writer, input.arguments)
        and gemini_put_byte(writer, 125)
    )


def gemini_put_custom_tool_item(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    return (
        gemini_put_literal(writer, StringSlice('{"type":"custom_tool_call","call_id":'))
        and gemini_put_json_string(writer, input.call_id)
        and gemini_put_literal(writer, StringSlice(',"name":'))
        and gemini_put_json_string(writer, input.name)
        and gemini_put_literal(writer, StringSlice(',"input":'))
        and gemini_put_json_string(writer, input.arguments)
        and gemini_put_byte(writer, 125)
    )


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


def gemini_put_stream_identifier(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    prefix: StringSlice,
    request_id: UInt64,
    item_index: UInt64,
    include_item_index: Bool,
) -> Bool:
    if not gemini_put_byte(writer, 34) or not gemini_put_literal(writer, prefix):
        return False
    if not gemini_put_u64(writer, request_id):
        return False
    if include_item_index:
        if not gemini_put_byte(writer, 95) or not gemini_put_u64(writer, item_index):
            return False
    return gemini_put_byte(writer, 34)


def gemini_put_stream_name(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    name: ProdexRichStringView,
    name_present: Bool,
) -> Bool:
    if not name_present:
        return gemini_put_literal(writer, StringSlice('"tool_call"'))
    return gemini_put_json_string(writer, name)


def gemini_put_stream_function_call_delta(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if not gemini_put_byte(writer, 123):
        return False
    if input.call_id_present == 1:
        if not gemini_put_literal(writer, StringSlice('"explicit_call_id":')) or not gemini_put_json_string(writer, input.call_id) or not gemini_put_byte(writer, 44):
            return False
    if not gemini_put_literal(writer, StringSlice('"name":')) or not gemini_put_stream_name(writer, input.name, input.reason_present == 1):
        return False
    return (
        gemini_put_literal(writer, StringSlice(',"arguments":'))
        and gemini_put_json_string(writer, input.arguments)
        and gemini_put_byte(writer, 125)
    )


def gemini_put_stream_tool_call(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if not gemini_put_literal(writer, StringSlice('{"call_id":')):
        return False
    if input.call_id_present == 1:
        if not gemini_put_json_string(writer, input.call_id):
            return False
    elif not gemini_put_stream_identifier(
        writer, StringSlice("call_gemini_"), input.sequence_number, input.summary_index, True
    ):
        return False
    if not gemini_put_literal(writer, StringSlice(',"name":')) or not gemini_put_stream_name(writer, input.name, input.reason_present == 1):
        return False
    if not gemini_put_literal(writer, StringSlice(',"arguments":')) or not gemini_put_json_string(writer, input.arguments):
        return False
    if input.signature_present == 1:
        if not gemini_put_literal(writer, StringSlice(',"thought_signature":')) or not gemini_put_json_string(writer, input.signature):
            return False
    return gemini_put_byte(writer, 125)



def gemini_raw_bounds_present(bounds: Array[Int64, 2]) -> Bool:
    return bounds[0] >= 0 and bounds[1] > bounds[0]


def gemini_raw_root(view: ProdexRichStringView) -> Array[Int64, 2]:
    var missing = Array[Int64, 2](fill=-1)
    var start = deepseek_json_skip_ws(view, 0, Int64(view.len))
    if start >= Int64(view.len) or deepseek_json_byte(view, start) != 123:
        return missing^
    var end = deepseek_json_value_end(view, start, Int64(view.len), 0)
    if end < 0 or deepseek_json_skip_ws(view, end, Int64(view.len)) != Int64(view.len):
        return missing^
    var root = Array[Int64, 2](fill=-1)
    root[0] = start
    root[1] = end
    return root^


def gemini_raw_member(
    view: ProdexRichStringView,
    object: Array[Int64, 2],
    key: StringSlice,
) -> Array[Int64, 2]:
    if not gemini_raw_bounds_present(object) or deepseek_json_byte(view, object[0]) != 123:
        return Array[Int64, 2](fill=-1)^
    return deepseek_json_object_member(view, object[0], object[1], key)


def gemini_raw_first_array_item(
    view: ProdexRichStringView,
    array: Array[Int64, 2],
) -> Array[Int64, 2]:
    var missing = Array[Int64, 2](fill=-1)
    if not gemini_raw_bounds_present(array) or deepseek_json_byte(view, array[0]) != 91:
        return missing^
    var start = deepseek_json_skip_ws(view, array[0] + 1, array[1] - 1)
    if start >= array[1] - 1:
        return missing^
    var end = deepseek_json_value_end(view, start, array[1] - 1, 0)
    if end < 0:
        return missing^
    var item = Array[Int64, 2](fill=-1)
    item[0] = start
    item[1] = end
    return item^


def gemini_raw_string_nonempty(
    view: ProdexRichStringView, bounds: Array[Int64, 2]
) -> Bool:
    return (
        gemini_raw_bounds_present(bounds)
        and deepseek_json_byte(view, bounds[0]) == 34
        and bounds[1] - bounds[0] > 2
    )



def gemini_raw_part_supported(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    if deepseek_json_byte(view, start) != 123:
        return False
    for key in [
        StringSlice("functionCall"),
        StringSlice("inlineData"),
        StringSlice("inline_data"),
        StringSlice("fileData"),
        StringSlice("file_data"),
        StringSlice("executableCode"),
        StringSlice("codeExecutionResult"),
        StringSlice("videoMetadata"),
    ]:
        if deepseek_json_object_member(view, start, end, key)[0] >= 0:
            return False
    var text = deepseek_json_object_member(view, start, end, StringSlice("text"))
    if text[0] >= 0 and deepseek_json_byte(view, text[0]) != 34:
        return False
    return True


def gemini_raw_parts_supported(
    view: ProdexRichStringView, parts: Array[Int64, 2]
) -> Bool:
    if not gemini_raw_bounds_present(parts) or deepseek_json_byte(view, parts[0]) != 91:
        return False
    var index = deepseek_json_skip_ws(view, parts[0] + 1, parts[1] - 1)
    while index < parts[1] - 1 and deepseek_json_byte(view, index) != 93:
        var item_end = deepseek_json_value_end(view, index, parts[1] - 1, 0)
        if item_end < 0 or not gemini_raw_part_supported(view, index, item_end):
            return False
        index = deepseek_json_skip_ws(view, item_end, parts[1] - 1)
        if index < parts[1] - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, parts[1] - 1)
        elif index != parts[1] - 1:
            return False
    return True


def gemini_raw_part_is_thought(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    var thought = deepseek_json_object_member(view, start, end, StringSlice("thought"))
    if thought[0] < 0:
        return False
    return (
        thought[1] - thought[0] == 4
        and deepseek_json_byte(view, thought[0]) == 116
        and deepseek_json_byte(view, thought[0] + 1) == 114
        and deepseek_json_byte(view, thought[0] + 2) == 117
        and deepseek_json_byte(view, thought[0] + 3) == 101
    )


def gemini_raw_has_visible_text(
    view: ProdexRichStringView, parts: Array[Int64, 2]
) -> Bool:
    var index = deepseek_json_skip_ws(view, parts[0] + 1, parts[1] - 1)
    while index < parts[1] - 1 and deepseek_json_byte(view, index) != 93:
        var item_end = deepseek_json_value_end(view, index, parts[1] - 1, 0)
        if item_end < 0:
            return False
        if not gemini_raw_part_is_thought(view, index, item_end):
            var text = deepseek_json_object_member(view, index, item_end, StringSlice("text"))
            if gemini_raw_string_nonempty(view, text):
                return True
        index = deepseek_json_skip_ws(view, item_end, parts[1] - 1)
        if index < parts[1] - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, parts[1] - 1)
        else:
            break
    return False


def gemini_raw_put_visible_text(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
    parts: Array[Int64, 2],
) -> Bool:
    var index = deepseek_json_skip_ws(view, parts[0] + 1, parts[1] - 1)
    while index < parts[1] - 1 and deepseek_json_byte(view, index) != 93:
        var item_end = deepseek_json_value_end(view, index, parts[1] - 1, 0)
        if item_end < 0:
            return False
        if not gemini_raw_part_is_thought(view, index, item_end):
            var text = deepseek_json_object_member(view, index, item_end, StringSlice("text"))
            if gemini_raw_string_nonempty(view, text):
                if not gemini_put_view_range(writer, view, text[0] + 1, text[1] - 1):
                    return False
        index = deepseek_json_skip_ws(view, item_end, parts[1] - 1)
        if index < parts[1] - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, parts[1] - 1)
        else:
            break
    return True


def gemini_raw_u64(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
    default_value: UInt64,
) -> UInt64:
    if not gemini_raw_bounds_present(bounds):
        return default_value
    var value: UInt64 = 0
    var index = bounds[0]
    if index >= bounds[1]:
        return default_value
    while index < bounds[1]:
        var byte = deepseek_json_byte(view, index)
        if byte < 48 or byte > 57:
            return default_value
        var digit = UInt64(byte - 48)
        if value > 1844674407370955161 or (
            value == 1844674407370955161 and digit > 5
        ):
            return default_value
        value = value * 10 + digit
        index += 1
    return value


def gemini_raw_put_usage(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
    usage: Array[Int64, 2],
) -> Bool:
    if not gemini_raw_bounds_present(usage) or deepseek_json_byte(view, usage[0]) != 123:
        return gemini_put_literal(writer, StringSlice("{}"))
    var prompt = gemini_raw_u64(
        view, gemini_raw_member(view, usage, StringSlice("promptTokenCount")), 0
    )
    var output = gemini_raw_u64(
        view, gemini_raw_member(view, usage, StringSlice("candidatesTokenCount")), 0
    )
    var total_bounds = gemini_raw_member(view, usage, StringSlice("totalTokenCount"))
    var total = gemini_raw_u64(view, total_bounds, prompt + output)
    var cached = gemini_raw_u64(
        view, gemini_raw_member(view, usage, StringSlice("cachedContentTokenCount")), 0
    )
    var thoughts = gemini_raw_u64(
        view, gemini_raw_member(view, usage, StringSlice("thoughtsTokenCount")), 0
    )
    var tools = gemini_raw_u64(
        view, gemini_raw_member(view, usage, StringSlice("toolUsePromptTokenCount")), 0
    )
    return (
        gemini_put_literal(writer, StringSlice('{"input_tokens":'))
        and gemini_put_u64(writer, prompt)
        and gemini_put_literal(writer, StringSlice(',"input_tokens_details":{"cached_tokens":'))
        and gemini_put_u64(writer, cached)
        and gemini_put_literal(writer, StringSlice(',"tool_tokens":'))
        and gemini_put_u64(writer, tools)
        and gemini_put_literal(writer, StringSlice('},"output_tokens":'))
        and gemini_put_u64(writer, output)
        and gemini_put_literal(writer, StringSlice(',"output_tokens_details":{"reasoning_tokens":'))
        and gemini_put_u64(writer, thoughts)
        and gemini_put_literal(writer, StringSlice('},"total_tokens":'))
        and gemini_put_u64(writer, total)
        and gemini_put_byte(writer, 125)
    )

def gemini_raw_metadata_supported(
    view: ProdexRichStringView,
    root: Array[Int64, 2],
    candidate: Array[Int64, 2],
) -> Bool:
    var feedback = gemini_raw_member(view, root, StringSlice("promptFeedback"))
    if gemini_raw_bounds_present(feedback) and not (
        feedback[1] - feedback[0] == 4
        and deepseek_json_byte(view, feedback[0]) == 110
        and deepseek_json_byte(view, feedback[0] + 1) == 117
        and deepseek_json_byte(view, feedback[0] + 2) == 108
        and deepseek_json_byte(view, feedback[0] + 3) == 108
    ):
        return False
    var finish = gemini_raw_member(view, candidate, StringSlice("finishReason"))
    if gemini_raw_bounds_present(finish) and not deepseek_json_raw_equals(
        view, finish[0], finish[1], StringSlice("STOP")
    ):
        return False
    for key in [
        StringSlice("finishMessage"),
        StringSlice("safetyRatings"),
        StringSlice("citationMetadata"),
        StringSlice("groundingMetadata"),
        StringSlice("urlContextMetadata"),
        StringSlice("avgLogprobs"),
        StringSlice("logprobsResult"),
    ]:
        if gemini_raw_bounds_present(gemini_raw_member(view, candidate, key)):
            return False
    return True


def gemini_raw_put_metadata(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    view: ProdexRichStringView,
    root: Array[Int64, 2],
    candidate: Array[Int64, 2],
) -> Bool:
    var usage = gemini_raw_member(view, root, StringSlice("usageMetadata"))
    var finish = gemini_raw_member(view, candidate, StringSlice("finishReason"))
    if not gemini_raw_bounds_present(usage) and not gemini_raw_bounds_present(finish):
        return gemini_put_literal(writer, StringSlice("{}"))
    if not gemini_put_literal(writer, StringSlice('{"gemini":{')):
        return False
    var first = True
    if gemini_raw_bounds_present(usage):
        if (
            not gemini_put_literal(writer, StringSlice('"usageMetadata":'))
            or not gemini_put_view_range(writer, view, usage[0], usage[1])
        ):
            return False
        first = False
    if gemini_raw_bounds_present(finish):
        if not first and not gemini_put_byte(writer, 44):
            return False
        if (
            not gemini_put_literal(writer, StringSlice('"finishReason":'))
            or not gemini_put_view_range(writer, view, finish[0], finish[1])
        ):
            return False
    return gemini_put_literal(writer, StringSlice("}}"))


def gemini_put_raw_text_response(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if input.response.len == 0:
        return gemini_put_literal(writer, StringSlice("null"))
    var source = input.response.copy()
    var root = gemini_raw_root(source)
    if not gemini_raw_bounds_present(root):
        return gemini_put_literal(writer, StringSlice("null"))
    var candidates = gemini_raw_member(source, root, StringSlice("candidates"))
    var candidate = gemini_raw_first_array_item(source, candidates)
    if not gemini_raw_bounds_present(candidate) or deepseek_json_byte(source, candidate[0]) != 123:
        return gemini_put_literal(writer, StringSlice("null"))
    if not gemini_raw_metadata_supported(source, root, candidate):
        return gemini_put_literal(writer, StringSlice("null"))
    var content = gemini_raw_member(source, candidate, StringSlice("content"))
    var parts = gemini_raw_member(source, content, StringSlice("parts"))
    if not gemini_raw_parts_supported(source, parts) or not gemini_raw_has_visible_text(source, parts):
        return gemini_put_literal(writer, StringSlice("null"))

    var response_id = gemini_raw_member(source, root, StringSlice("responseId"))
    if not gemini_raw_string_nonempty(source, response_id):
        response_id = gemini_raw_member(source, root, StringSlice("id"))
    var model = gemini_raw_member(source, root, StringSlice("modelVersion"))
    if not gemini_raw_string_nonempty(source, model):
        model = gemini_raw_member(source, root, StringSlice("model"))

    if not gemini_put_literal(writer, StringSlice('{"id":')):
        return False
    if gemini_raw_string_nonempty(source, response_id):
        if not gemini_put_view_range(writer, source, response_id[0], response_id[1]):
            return False
    elif input.response_id_present == 1:
        if not gemini_put_json_string(writer, input.response_id):
            return False
    else:
        if not gemini_put_literal(writer, StringSlice('"gemini_resp_prodex"')):
            return False

    if not gemini_put_literal(writer, StringSlice(',"object":"response","model":')):
        return False
    if gemini_raw_string_nonempty(source, model):
        if not gemini_put_view_range(writer, source, model[0], model[1]):
            return False
    elif input.model_present == 1:
        if not gemini_put_json_string(writer, input.model):
            return False
    else:
        if not gemini_put_literal(writer, StringSlice('"gemini-2.5-pro"')):
            return False

    if not gemini_put_literal(
        writer,
        StringSlice(',"output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"'),
    ):
        return False
    if not gemini_raw_put_visible_text(writer, source, parts):
        return False
    if not gemini_put_literal(writer, StringSlice('"}]}]')):
        return False

    var usage = gemini_raw_member(source, root, StringSlice("usageMetadata"))
    if not gemini_put_literal(writer, StringSlice(',"usage":')):
        return False
    if not gemini_raw_put_usage(writer, source, usage):
        return False

    if not gemini_put_literal(writer, StringSlice(',"metadata":')):
        return False
    if not gemini_raw_put_metadata(writer, source, root, candidate):
        return False
    return gemini_put_byte(writer, 125)


def gemini_raw_string_present(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
) -> Bool:
    return (
        gemini_raw_bounds_present(bounds)
        and bounds[1] - bounds[0] >= 2
        and deepseek_json_byte(view, bounds[0]) == 34
        and deepseek_json_byte(view, bounds[1] - 1) == 34
    )


def gemini_put_stream_transform_status(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    status: StringSlice,
) -> Bool:
    return (
        gemini_put_literal(writer, StringSlice('{"status":"'))
        and gemini_put_literal(writer, status)
        and gemini_put_literal(writer, StringSlice('"}'))
    )


def gemini_put_stream_transform_event_prefix(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    event_type: StringSlice,
) -> Bool:
    return (
        gemini_put_literal(writer, StringSlice('{"status":"ok","event":"'))
        and gemini_put_literal(writer, event_type)
        and gemini_put_literal(writer, StringSlice('","value":{"type":"'))
        and gemini_put_literal(writer, event_type)
        and gemini_put_byte(writer, 34)
    )


def gemini_put_raw_stream_event_transform(
    writer: Pointer[mut=True, GeminiResponseWriter, _],
    input: ProdexGeminiResponseKernelInput,
) -> Bool:
    if input.response.len == 0:
        return gemini_put_stream_transform_status(writer, StringSlice("invalid"))
    var source = input.response.copy()
    var root = gemini_raw_root(source)
    if not gemini_raw_bounds_present(root):
        return gemini_put_stream_transform_status(writer, StringSlice("invalid"))

    var candidates = gemini_raw_member(source, root, StringSlice("candidates"))
    var candidate = gemini_raw_first_array_item(source, candidates)
    var content = gemini_raw_member(source, candidate, StringSlice("content"))
    var parts = gemini_raw_member(source, content, StringSlice("parts"))
    var part = gemini_raw_first_array_item(source, parts)
    if not gemini_raw_bounds_present(part) or deepseek_json_byte(source, part[0]) != 123:
        return gemini_put_stream_transform_status(writer, StringSlice("unsupported"))

    var function_call = gemini_raw_member(source, part, StringSlice("functionCall"))
    if gemini_raw_bounds_present(function_call):
        if not gemini_put_stream_transform_event_prefix(
            writer, StringSlice("response.function_call_arguments.delta")
        ):
            return False
        var call_id = gemini_raw_member(source, function_call, StringSlice("id"))
        if gemini_raw_string_present(source, call_id):
            if (
                not gemini_put_literal(writer, StringSlice(',"call_id":'))
                or not gemini_put_view_range(writer, source, call_id[0], call_id[1])
            ):
                return False
        if not gemini_put_literal(writer, StringSlice(',"delta":')):
            return False
        var args = gemini_raw_member(source, function_call, StringSlice("args"))
        if gemini_raw_bounds_present(args):
            if not gemini_put_json_string_range(writer, source, args[0], args[1]):
                return False
        elif not gemini_put_literal(writer, StringSlice('"{}"')):
            return False
        return gemini_put_literal(writer, StringSlice("}}"))

    var text = gemini_raw_member(source, part, StringSlice("text"))
    if not gemini_raw_string_present(source, text):
        return gemini_put_stream_transform_status(writer, StringSlice("unsupported"))

    if gemini_raw_part_is_thought(source, part[0], part[1]):
        if not gemini_put_stream_transform_event_prefix(
            writer, StringSlice("response.reasoning_summary_text.delta")
        ):
            return False
    elif not gemini_put_stream_transform_event_prefix(
        writer, StringSlice("response.output_text.delta")
    ):
        return False
    return (
        gemini_put_literal(writer, StringSlice(',"delta":'))
        and gemini_put_view_range(writer, source, text[0], text[1])
        and gemini_put_literal(writer, StringSlice("}}"))
    )


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
            and (input.signature_present == 0 or (
                gemini_put_literal(writer, StringSlice(',"thought_signature":'))
                and gemini_put_json_string(writer, input.signature)
            ))
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
            and (input.signature_present == 0 or (
                gemini_put_literal(writer, StringSlice(',"thought_signature":'))
                and gemini_put_json_string(writer, input.signature)
            ))
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
    if operation == GEMINI_TOOL_SEARCH_CALL_ITEM:
        return gemini_put_tool_search_item(writer, input)
    if operation == GEMINI_CUSTOM_TOOL_CALL_ITEM:
        return gemini_put_custom_tool_item(writer, input)
    if operation == GEMINI_ADDED_FUNCTION_CALL_ITEM:
        if not gemini_put_literal(writer, StringSlice('{"type":"function_call","call_id":')):
            return False
        if not gemini_put_json_string(writer, input.call_id) or not gemini_put_literal(writer, StringSlice(',"name":')):
            return False
        if not gemini_put_tool_name(writer, input):
            return False
        if not gemini_put_tool_namespace(writer, input):
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
    if operation == GEMINI_FINISH_REASON_FAILURE:
        return gemini_put_finish_reason_failure(writer, input.reason)
    if operation == GEMINI_FINISH_REASON_INCOMPLETE:
        if not gemini_view_equals(input.reason, StringSlice("MAX_TOKENS")):
            return gemini_put_literal(writer, StringSlice("null"))
        return gemini_put_literal(writer, StringSlice('["max_output_tokens","Gemini stopped because it reached the maximum output token limit."]'))
    if operation == GEMINI_PROMPT_FEEDBACK_FAILURE:
        return gemini_put_reason_result(
            writer, StringSlice("gemini_prompt_blocked"), StringSlice("Gemini blocked the prompt: "), input.reason
        )
    if operation == GEMINI_STREAM_FUNCTION_CALL_DELTA:
        return gemini_put_stream_function_call_delta(writer, input)
    if operation == GEMINI_STREAM_TOOL_CALL:
        return gemini_put_stream_tool_call(writer, input)
    if operation == GEMINI_STREAM_OUTPUT_TEXT_ITEM_ID:
        return gemini_put_stream_identifier(writer, StringSlice("msg_gemini_"), input.sequence_number, 0, False)
    if operation == GEMINI_STREAM_MEDIA_ITEM_ID:
        return gemini_put_stream_identifier(writer, StringSlice("msg_gemini_media_"), input.sequence_number, 0, False)
    if operation == GEMINI_STREAM_CITATION_ITEM_ID:
        return gemini_put_stream_identifier(writer, StringSlice("msg_gemini_citations_"), input.sequence_number, 0, False)
    if operation == GEMINI_STREAM_FALLBACK_RESPONSE_ID:
        return gemini_put_stream_identifier(writer, StringSlice("resp_gemini_"), input.sequence_number, 0, False)
    if operation == GEMINI_STREAM_FALLBACK_TOOL_CALL_ID:
        return gemini_put_stream_identifier(writer, StringSlice("call_gemini_"), input.sequence_number, input.summary_index, True)
    if operation == GEMINI_STREAM_SHOULD_EMIT_ARGUMENTS_DELTA:
        if gemini_view_equals(input.name, StringSlice("tool_search")) or gemini_view_equals(input.name, StringSlice("apply_patch")):
            return gemini_put_literal(writer, StringSlice("false"))
        return gemini_put_literal(writer, StringSlice("true"))
    if operation == GEMINI_STREAM_RESPONSE_ID:
        if input.call_id_present == 1 and gemini_view_starts_with(input.response_id, StringSlice("resp_gemini_")):
            return gemini_put_json_string(writer, input.call_id)
        return gemini_put_literal(writer, StringSlice("null"))
    if operation == GEMINI_RAW_TEXT_RESPONSE:
        return gemini_put_raw_text_response(writer, input)
    if operation == GEMINI_STREAM_EVENT_TRANSFORM:
        return gemini_put_raw_stream_event_transform(writer, input)
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
    if not gemini_views_valid(input[].copy(), GEMINI_KERNEL_MAX_BYTES):
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


def gemini_buffered_response_kernel_v2(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_BUFFERED_RESPONSE_ABI_VERSION:
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
    if input[].operation != GEMINI_BUFFERED_RESPONSE:
        return GEMINI_KERNEL_STATUS_INVALID
    if not gemini_views_valid(input[].copy(), GEMINI_BUFFERED_RESPONSE_MAX_BYTES):
        return GEMINI_KERNEL_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = GeminiResponseWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    if not gemini_put_buffered_response(writer_ptr, input[].copy()):
        if writer.written >= output_capacity:
            written[] = writer.written
            return GEMINI_KERNEL_STATUS_CAPACITY
        return GEMINI_KERNEL_STATUS_INVALID
    written[] = writer.written
    return GEMINI_KERNEL_STATUS_OK
