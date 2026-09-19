from std.memory import Pointer

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView
from json_view import (
    deepseek_json_byte,
    deepseek_json_fragment_valid,
    deepseek_json_object_member,
    deepseek_json_raw_equals,
    deepseek_json_skip_ws,
    deepseek_json_string_end,
    deepseek_json_value_end,
)

comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime DEEPSEEK_KERNEL_MAX_BYTES: Int64 = 4_194_304
comptime DEEPSEEK_KERNEL_STATUS_OK: Int64 = 0
comptime DEEPSEEK_KERNEL_STATUS_INVALID: Int64 = 1
comptime DEEPSEEK_KERNEL_STATUS_UTF8: Int64 = 2
comptime DEEPSEEK_KERNEL_STATUS_CAPACITY: Int64 = 3
comptime DEEPSEEK_KERNEL_STATUS_ABI: Int64 = 4

comptime DEEPSEEK_REQUEST_BODY: Int64 = 1
comptime DEEPSEEK_SYSTEM_MESSAGE: Int64 = 2
comptime DEEPSEEK_USER_MESSAGE: Int64 = 3
comptime DEEPSEEK_MESSAGE: Int64 = 4
comptime DEEPSEEK_TOOL_CALL_MESSAGE: Int64 = 5
comptime DEEPSEEK_TOOL_MESSAGE: Int64 = 6
comptime DEEPSEEK_RESPONSE_VALUE: Int64 = 7
comptime DEEPSEEK_BUFFERED_RESPONSE: Int64 = 8
comptime DEEPSEEK_RESPONSE_CREATED_EVENT: Int64 = 9
comptime DEEPSEEK_RESPONSE_COMPLETED_EVENT: Int64 = 10
comptime DEEPSEEK_OUTPUT_ITEM_ADDED_EVENT: Int64 = 11
comptime DEEPSEEK_OUTPUT_ITEM_DONE_EVENT: Int64 = 12
comptime DEEPSEEK_FUNCTION_CALL_ARGUMENTS_DELTA_EVENT: Int64 = 13
comptime DEEPSEEK_OUTPUT_TEXT_DELTA_EVENT: Int64 = 14
comptime DEEPSEEK_OUTPUT_TEXT_ITEM: Int64 = 15
comptime DEEPSEEK_STREAM_RESPONSE_VALUE: Int64 = 16
comptime DEEPSEEK_STREAM_ASSISTANT_MESSAGE: Int64 = 17
comptime DEEPSEEK_FUNCTION_CALL_ITEM: Int64 = 18
comptime DEEPSEEK_ADDED_FUNCTION_CALL_ITEM: Int64 = 19
comptime DEEPSEEK_TOOL_SEARCH_ITEM: Int64 = 20
comptime DEEPSEEK_CUSTOM_TOOL_CALL_ITEM: Int64 = 21
comptime DEEPSEEK_FUNCTION_CALL_ARGUMENTS_DELTA_SOURCE: Int64 = 22
comptime DEEPSEEK_TEXT_DELTA_SOURCE: Int64 = 23
comptime DEEPSEEK_SSE_FUNCTION_CALL_DELTA: Int64 = 24
comptime DEEPSEEK_SSE_TEXT_DELTA: Int64 = 25
comptime DEEPSEEK_RESPONSE_METADATA: Int64 = 26
comptime DEEPSEEK_STRICT_FUNCTION_SCHEMA: Int64 = 27
comptime DEEPSEEK_PRIMITIVE_REQUEST_FIELDS: Int64 = 28
comptime DEEPSEEK_REASONING_PARAMETERS: Int64 = 29
comptime DEEPSEEK_RESPONSE_FORMAT: Int64 = 30
comptime DEEPSEEK_USER_ID: Int64 = 31
comptime DEEPSEEK_STREAM_TOOL_CALL_DELTA: Int64 = 32
comptime DEEPSEEK_STREAM_CHUNK_METADATA: Int64 = 33
comptime DEEPSEEK_STREAM_CHOICE_METADATA: Int64 = 34
comptime DEEPSEEK_STREAM_CHOICE_DELTA: Int64 = 35
comptime DEEPSEEK_STREAM_RESPONSE_METADATA: Int64 = 36
comptime DEEPSEEK_RAW_COMMON_REQUEST: Int64 = 37
comptime DEEPSEEK_REQUEST_METADATA: Int64 = 38
comptime DEEPSEEK_JSON_MAX_DEPTH: Int64 = 256


@fieldwise_init
struct ProdexDeepSeekKernelInput(Copyable):
    var operation: Int64
    var sequence_number: UInt64
    var created_at: UInt64
    var stream: Int64
    var response_id_present: Int64
    var call_id_present: Int64
    var model_present: Int64
    var role_present: Int64
    var content_present: Int64
    var reasoning_content_present: Int64
    var name_present: Int64
    var namespace_present: Int64
    var arguments_present: Int64
    var signature_present: Int64
    var delta_present: Int64
    var messages_present: Int64
    var tools_present: Int64
    var tool_choice_present: Int64
    var extra_present: Int64
    var output_present: Int64
    var usage_present: Int64
    var metadata_present: Int64
    var item_present: Int64
    var response_present: Int64
    var tool_calls_present: Int64
    var input_present: Int64
    var error_code_present: Int64
    var error_message_present: Int64
    var response_id: ProdexRichStringView
    var call_id: ProdexRichStringView
    var model: ProdexRichStringView
    var role: ProdexRichStringView
    var content: ProdexRichStringView
    var reasoning_content: ProdexRichStringView
    var name: ProdexRichStringView
    var namespace: ProdexRichStringView
    var arguments: ProdexRichStringView
    var signature: ProdexRichStringView
    var delta: ProdexRichStringView
    var messages: ProdexRichStringView
    var tools: ProdexRichStringView
    var tool_choice: ProdexRichStringView
    var extra: ProdexRichStringView
    var output: ProdexRichStringView
    var usage: ProdexRichStringView
    var metadata: ProdexRichStringView
    var item: ProdexRichStringView
    var response: ProdexRichStringView
    var tool_calls: ProdexRichStringView
    var input: ProdexRichStringView
    var error_code: ProdexRichStringView
    var error_message: ProdexRichStringView


@fieldwise_init
struct DeepSeekResponseWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def deepseek_put_byte(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _], value: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True


def deepseek_put_literal(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _], value: StringSlice
) -> Bool:
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not deepseek_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def deepseek_put_hex_byte(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _], value: UInt8
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
    return deepseek_put_byte(writer, high) and deepseek_put_byte(writer, low)


def deepseek_put_json_string_range(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    if not deepseek_put_byte(writer, 34):
        return False
    if end > start:
        var ptr = rich_view_ptr(view)
        for index in range(start, end):
            var value = ptr[unsafe_offset=index]
            if value == 34 or value == 92:
                if not deepseek_put_byte(writer, 92) or not deepseek_put_byte(writer, value):
                    return False
            elif value == 8:
                if not deepseek_put_literal(writer, StringSlice("\\b")):
                    return False
            elif value == 9:
                if not deepseek_put_literal(writer, StringSlice("\\t")):
                    return False
            elif value == 10:
                if not deepseek_put_literal(writer, StringSlice("\\n")):
                    return False
            elif value == 12:
                if not deepseek_put_literal(writer, StringSlice("\\f")):
                    return False
            elif value == 13:
                if not deepseek_put_literal(writer, StringSlice("\\r")):
                    return False
            elif value < 32:
                if not deepseek_put_literal(writer, StringSlice("\\u00")) or not deepseek_put_hex_byte(writer, value):
                    return False
            elif not deepseek_put_byte(writer, value):
                return False
    return deepseek_put_byte(writer, 34)


def deepseek_put_json_string(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    return deepseek_put_json_string_range(writer, view, 0, Int64(view.len))


def deepseek_put_view(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if view.len == 0:
        return True
    var ptr = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        if not deepseek_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def deepseek_put_u64(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return deepseek_put_byte(writer, 48)
    var divisor: UInt64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        if not deepseek_put_byte(writer, UInt8(remaining / divisor) + 48):
            return False
        remaining %= divisor
        divisor /= 10
    return True


def deepseek_put_optional_string(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    key: StringSlice,
    present: Int64,
    view: ProdexRichStringView,
) -> Bool:
    if present == 0:
        return True
    return deepseek_put_literal(writer, key) and deepseek_put_json_string(writer, view)


def deepseek_put_optional_view(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    key: StringSlice,
    present: Int64,
    view: ProdexRichStringView,
) -> Bool:
    if present == 0:
        return True
    return deepseek_put_literal(writer, key) and deepseek_put_view(writer, view)


def deepseek_put_extra_fields(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    present: Int64,
    view: ProdexRichStringView,
) -> Bool:
    if present == 0:
        return True
    if view.len < 2:
        return False
    if view.len == 2:
        return True
    return deepseek_put_byte(writer, 44) and deepseek_put_view_range(writer, view, 1, Int64(view.len) - 1)


def deepseek_put_view_range(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        if not deepseek_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True



def deepseek_put_raw_json_member(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    key: StringSlice,
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    return deepseek_put_literal(writer, key) and deepseek_put_view_range(writer, view, start, end)


def deepseek_put_request_raw_member(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    key: StringSlice,
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
    first: Pointer[mut=True, Bool, _],
) -> Bool:
    if bounds[0] < 0 or bounds[1] < bounds[0]:
        return True
    if not first[] and not deepseek_put_byte(writer, 44):
        return False
    first[] = False
    return deepseek_put_raw_json_member(writer, key, view, bounds[0], bounds[1])


def deepseek_schema_write_any_of_array(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    depth: Int64,
) -> Bool:
    if start < 0 or end <= start + 1 or deepseek_json_byte(view, start) != 91 or deepseek_json_byte(view, end - 1) != 93:
        return False
    if not deepseek_put_byte(writer, 91):
        return False
    var first = True
    var index = deepseek_json_skip_ws(view, start + 1, end - 1)
    if index < end - 1 and deepseek_json_byte(view, index) == 93:
        return deepseek_put_byte(writer, 93)
    while index < end - 1:
        var value_end = deepseek_json_value_end(view, index, end - 1, depth + 1)
        if value_end < 0:
            return False
        if not first and not deepseek_put_byte(writer, 44):
            return False
        first = False
        if not deepseek_schema_write(view, index, value_end, writer, depth + 1):
            return False
        index = deepseek_json_skip_ws(view, value_end, end - 1)
        if index < end - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, end - 1)
            continue
        if index < end - 1 and deepseek_json_byte(view, index) == 93:
            return deepseek_put_byte(writer, 93)
        return False
    return False


def deepseek_schema_write_any_of(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
) -> Bool:
    if not deepseek_put_byte(writer, 123):
        return False
    var first = True
    var index = deepseek_json_skip_ws(view, start + 1, end - 1)
    while index < end - 1:
        var key_start = index
        var key_end = deepseek_json_string_end(view, key_start, end - 1)
        if key_end < 0:
            return False
        index = deepseek_json_skip_ws(view, key_end, end - 1)
        if index >= end - 1 or deepseek_json_byte(view, index) != 58:
            return False
        var value_start = deepseek_json_skip_ws(view, index + 1, end - 1)
        var value_end = deepseek_json_value_end(view, value_start, end - 1, 0)
        if value_end < 0:
            return False
        if not first and not deepseek_put_byte(writer, 44):
            return False
        first = False
        if not deepseek_put_view_range(writer, view, key_start, key_end) or not deepseek_put_byte(writer, 58):
            return False
        if deepseek_json_raw_equals(view, key_start, key_end, StringSlice("anyOf")):
            if not deepseek_schema_write_any_of_array(view, value_start, value_end, writer, 0):
                return False
        elif not deepseek_put_view_range(writer, view, value_start, value_end):
            return False
        index = deepseek_json_skip_ws(view, value_end, end - 1)
        if index < end - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, end - 1)
            continue
        if index == end - 1:
            break
        return False
    return deepseek_put_byte(writer, 125)


def deepseek_schema_write_properties(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    depth: Int64,
) -> Bool:
    if start < 0:
        return deepseek_put_literal(writer, StringSlice("{}"))
    if end <= start + 1 or deepseek_json_byte(view, start) != 123 or deepseek_json_byte(view, end - 1) != 125:
        return False
    if not deepseek_put_byte(writer, 123):
        return False
    var first = True
    var index = deepseek_json_skip_ws(view, start + 1, end - 1)
    while index < end - 1:
        var key_start = index
        var key_end = deepseek_json_string_end(view, key_start, end - 1)
        if key_end < 0:
            return False
        index = deepseek_json_skip_ws(view, key_end, end - 1)
        if index >= end - 1 or deepseek_json_byte(view, index) != 58:
            return False
        var value_start = deepseek_json_skip_ws(view, index + 1, end - 1)
        var value_end = deepseek_json_value_end(view, value_start, end - 1, depth + 1)
        if value_end < 0:
            return False
        if not first and not deepseek_put_byte(writer, 44):
            return False
        first = False
        if not deepseek_put_view_range(writer, view, key_start, key_end) or not deepseek_put_byte(writer, 58):
            return False
        if not deepseek_schema_write(view, value_start, value_end, writer, depth + 1):
            return False
        index = deepseek_json_skip_ws(view, value_end, end - 1)
        if index < end - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, end - 1)
            continue
        if index == end - 1:
            break
        return False
    return deepseek_put_byte(writer, 125)


def deepseek_schema_write_required(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
) -> Bool:
    if start < 0:
        return deepseek_put_literal(writer, StringSlice("[]"))
    if end <= start + 1 or deepseek_json_byte(view, start) != 123 or deepseek_json_byte(view, end - 1) != 125:
        return False
    if not deepseek_put_byte(writer, 91):
        return False
    var first = True
    var index = deepseek_json_skip_ws(view, start + 1, end - 1)
    while index < end - 1:
        var key_start = index
        var key_end = deepseek_json_string_end(view, key_start, end - 1)
        if key_end < 0:
            return False
        index = deepseek_json_skip_ws(view, key_end, end - 1)
        if index >= end - 1 or deepseek_json_byte(view, index) != 58:
            return False
        var value_start = deepseek_json_skip_ws(view, index + 1, end - 1)
        var value_end = deepseek_json_value_end(view, value_start, end - 1, 0)
        if value_end < 0:
            return False
        if not first and not deepseek_put_byte(writer, 44):
            return False
        first = False
        if not deepseek_put_view_range(writer, view, key_start, key_end):
            return False
        index = deepseek_json_skip_ws(view, value_end, end - 1)
        if index < end - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, end - 1)
            continue
        if index == end - 1:
            break
        return False
    return deepseek_put_byte(writer, 93)


def deepseek_schema_write(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    depth: Int64,
) -> Bool:
    if depth > DEEPSEEK_JSON_MAX_DEPTH or start < 0 or end <= start or deepseek_json_byte(view, start) != 123:
        return False
    var any_of = deepseek_json_object_member(view, start, end, StringSlice("anyOf"))
    if any_of[0] >= 0:
        return deepseek_schema_write_any_of(view, start, end, writer)

    var type_bounds = deepseek_json_object_member(view, start, end, StringSlice("type"))
    var is_object = type_bounds[0] < 0
    var is_array = False
    if type_bounds[0] >= 0:
        if deepseek_json_byte(view, type_bounds[0]) != 34:
            return False
        if deepseek_json_raw_equals(view, type_bounds[0], type_bounds[1], StringSlice("object")):
            is_object = True
        elif deepseek_json_raw_equals(view, type_bounds[0], type_bounds[1], StringSlice("array")):
            is_array = True
        elif not (
            deepseek_json_raw_equals(view, type_bounds[0], type_bounds[1], StringSlice("string"))
            or deepseek_json_raw_equals(view, type_bounds[0], type_bounds[1], StringSlice("number"))
            or deepseek_json_raw_equals(view, type_bounds[0], type_bounds[1], StringSlice("integer"))
            or deepseek_json_raw_equals(view, type_bounds[0], type_bounds[1], StringSlice("boolean"))
        ):
            return False
    if is_object:
        var properties = deepseek_json_object_member(view, start, end, StringSlice("properties"))
        if properties[0] >= 0 and deepseek_json_byte(view, properties[0]) != 123:
            return False
        if not deepseek_put_literal(writer, StringSlice('{"type":"object"')):
            return False
        var index = deepseek_json_skip_ws(view, start + 1, end - 1)
        while index < end - 1:
            var key_start = index
            var key_end = deepseek_json_string_end(view, key_start, end - 1)
            if key_end < 0:
                return False
            index = deepseek_json_skip_ws(view, key_end, end - 1)
            if index >= end - 1 or deepseek_json_byte(view, index) != 58:
                return False
            var value_start = deepseek_json_skip_ws(view, index + 1, end - 1)
            var value_end = deepseek_json_value_end(view, value_start, end - 1, 0)
            if value_end < 0:
                return False
            if not (
                deepseek_json_raw_equals(view, key_start, key_end, StringSlice("type"))
                or deepseek_json_raw_equals(view, key_start, key_end, StringSlice("properties"))
                or deepseek_json_raw_equals(view, key_start, key_end, StringSlice("required"))
                or deepseek_json_raw_equals(view, key_start, key_end, StringSlice("additionalProperties"))
            ):
                if not deepseek_put_byte(writer, 44) or not deepseek_put_view_range(writer, view, key_start, key_end) or not deepseek_put_byte(writer, 58) or not deepseek_put_view_range(writer, view, value_start, value_end):
                    return False
            index = deepseek_json_skip_ws(view, value_end, end - 1)
            if index < end - 1 and deepseek_json_byte(view, index) == 44:
                index = deepseek_json_skip_ws(view, index + 1, end - 1)
                continue
            if index == end - 1:
                break
            return False
        if not deepseek_put_literal(writer, StringSlice(',"properties":')) or not deepseek_schema_write_properties(view, properties[0], properties[1], writer, depth + 1):
            return False
        if not deepseek_put_literal(writer, StringSlice(',"required":')) or not deepseek_schema_write_required(view, properties[0], properties[1], writer):
            return False
        return deepseek_put_literal(writer, StringSlice(',"additionalProperties":false}'))
    if is_array:
        var items = deepseek_json_object_member(view, start, end, StringSlice("items"))
        if items[0] < 0:
            return False
        if not deepseek_put_byte(writer, 123):
            return False
        var first = True
        var index = deepseek_json_skip_ws(view, start + 1, end - 1)
        while index < end - 1:
            var key_start = index
            var key_end = deepseek_json_string_end(view, key_start, end - 1)
            if key_end < 0:
                return False
            index = deepseek_json_skip_ws(view, key_end, end - 1)
            if index >= end - 1 or deepseek_json_byte(view, index) != 58:
                return False
            var value_start = deepseek_json_skip_ws(view, index + 1, end - 1)
            var value_end = deepseek_json_value_end(view, value_start, end - 1, 0)
            if value_end < 0:
                return False
            if deepseek_json_raw_equals(view, key_start, key_end, StringSlice("items")):
                if not first and not deepseek_put_byte(writer, 44):
                    return False
                if not deepseek_put_literal(writer, StringSlice('"items":')) or not deepseek_schema_write(view, value_start, value_end, writer, depth + 1):
                    return False
            else:
                if not first and not deepseek_put_byte(writer, 44):
                    return False
                if not deepseek_put_view_range(writer, view, key_start, key_end) or not deepseek_put_byte(writer, 58) or not deepseek_put_view_range(writer, view, value_start, value_end):
                    return False
            first = False
            index = deepseek_json_skip_ws(view, value_end, end - 1)
            if index < end - 1 and deepseek_json_byte(view, index) == 44:
                index = deepseek_json_skip_ws(view, index + 1, end - 1)
                continue
            if index == end - 1:
                break
            return False
        return deepseek_put_byte(writer, 125)
    if not deepseek_put_byte(writer, 123):
        return False
    var first = True
    var index = deepseek_json_skip_ws(view, start + 1, end - 1)
    while index < end - 1:
        var key_start = index
        var key_end = deepseek_json_string_end(view, key_start, end - 1)
        if key_end < 0:
            return False
        index = deepseek_json_skip_ws(view, key_end, end - 1)
        if index >= end - 1 or deepseek_json_byte(view, index) != 58:
            return False
        var value_start = deepseek_json_skip_ws(view, index + 1, end - 1)
        var value_end = deepseek_json_value_end(view, value_start, end - 1, 0)
        if value_end < 0:
            return False
        if not first and not deepseek_put_byte(writer, 44):
            return False
        first = False
        if not deepseek_put_view_range(writer, view, key_start, key_end) or not deepseek_put_byte(writer, 58) or not deepseek_put_view_range(writer, view, value_start, value_end):
            return False
        index = deepseek_json_skip_ws(view, value_end, end - 1)
        if index < end - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, end - 1)
            continue
        if index == end - 1:
            break
        return False
    return deepseek_put_byte(writer, 125)



def deepseek_put_projection_raw_member(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    key: StringSlice,
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
    first: Pointer[mut=True, Bool, _],
) -> Bool:
    if bounds[0] < 0 or bounds[1] < bounds[0]:
        return True
    if not first[] and not deepseek_put_byte(writer, 44):
        return False
    first[] = False
    return deepseek_put_literal(writer, key) and deepseek_put_view_range(
        writer, view, bounds[0], bounds[1]
    )


def deepseek_input_object_bounds(input: ProdexRichStringView) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    if not deepseek_json_fragment_valid(input):
        return result^
    var start = deepseek_json_skip_ws(input, 0, Int64(input.len))
    var end = deepseek_json_value_end(input, start, Int64(input.len), 0)
    if start >= 0 and end == Int64(input.len) and deepseek_json_byte(input, start) == 123:
        result[0] = start
        result[1] = end
    return result^


def deepseek_put_stream_tool_call_delta(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    if input.input_present != 1:
        return False
    var source = input.input.copy()
    var root = deepseek_input_object_bounds(source)
    if root[0] < 0:
        return False
    var index = deepseek_json_object_member(source, root[0], root[1], StringSlice("index"))
    var call_id = deepseek_json_object_member(source, root[0], root[1], StringSlice("id"))
    var function = deepseek_json_object_member(source, root[0], root[1], StringSlice("function"))
    var name = InlineArray[Int64, 2](fill=-1)
    var arguments = InlineArray[Int64, 2](fill=-1)
    if function[0] >= 0:
        name = deepseek_json_object_member(source, function[0], function[1], StringSlice("name"))
        arguments = deepseek_json_object_member(source, function[0], function[1], StringSlice("arguments"))
    var extra = deepseek_json_object_member(source, root[0], root[1], StringSlice("extra_content"))
    var google = InlineArray[Int64, 2](fill=-1)
    var signature = InlineArray[Int64, 2](fill=-1)
    if extra[0] >= 0:
        google = deepseek_json_object_member(source, extra[0], extra[1], StringSlice("google"))
    if google[0] >= 0:
        signature = deepseek_json_object_member(source, google[0], google[1], StringSlice("thought_signature"))
    if not deepseek_put_literal(writer, StringSlice('{"index":')):
        return False
    if index[0] >= 0:
        if not deepseek_put_view_range(writer, source, index[0], index[1]):
            return False
    elif not deepseek_put_byte(writer, 48):
        return False
    var first = False
    var first_ptr = Pointer(to=first)
    if not deepseek_put_projection_raw_member(writer, StringSlice('"id":'), source, call_id, first_ptr):
        return False
    if not deepseek_put_projection_raw_member(writer, StringSlice('"name":'), source, name, first_ptr):
        return False
    if not deepseek_put_projection_raw_member(writer, StringSlice('"arguments":'), source, arguments, first_ptr):
        return False
    if not deepseek_put_projection_raw_member(writer, StringSlice('"thought_signature":'), source, signature, first_ptr):
        return False
    return deepseek_put_byte(writer, 125)


def deepseek_put_stream_chunk_metadata(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    if input.input_present != 1:
        return False
    var source = input.input.copy()
    var root = deepseek_input_object_bounds(source)
    if root[0] < 0:
        return False
    if not deepseek_put_byte(writer, 123):
        return False
    var first = True
    for key in [StringSlice("model"), StringSlice("created"), StringSlice("system_fingerprint"), StringSlice("usage")]:
        var bounds = deepseek_json_object_member(source, root[0], root[1], key)
        if bounds[0] < 0:
            continue
        if not first and not deepseek_put_byte(writer, 44):
            return False
        first = False
        if not deepseek_put_byte(writer, 34) or not deepseek_put_literal(writer, key) or not deepseek_put_literal(writer, StringSlice('":')):
            return False
        if not deepseek_put_view_range(writer, source, bounds[0], bounds[1]):
            return False
    return deepseek_put_byte(writer, 125)


def deepseek_put_stream_choice_metadata(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    if input.input_present != 1:
        return False
    var source = input.input.copy()
    var root = deepseek_input_object_bounds(source)
    if root[0] < 0:
        return False
    var logprobs = deepseek_json_object_member(source, root[0], root[1], StringSlice("logprobs"))
    var finish_reason = deepseek_json_object_member(source, root[0], root[1], StringSlice("finish_reason"))
    if not deepseek_put_byte(writer, 123):
        return False
    var first = True
    var first_ptr = Pointer(to=first)
    if not deepseek_put_projection_raw_member(writer, StringSlice('"logprobs":'), source, logprobs, first_ptr):
        return False
    if not deepseek_put_projection_raw_member(writer, StringSlice('"finish_reason":'), source, finish_reason, first_ptr):
        return False
    return deepseek_put_byte(writer, 125)


def deepseek_put_stream_choice_delta(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    if input.input_present != 1:
        return False
    var source = input.input.copy()
    var root = deepseek_input_object_bounds(source)
    if root[0] < 0:
        return False
    var delta = deepseek_json_object_member(source, root[0], root[1], StringSlice("delta"))
    if not deepseek_put_byte(writer, 123):
        return False
    if delta[0] < 0 or deepseek_json_byte(source, delta[0]) != 123:
        return deepseek_put_byte(writer, 125)
    var first = True
    for key in [StringSlice("reasoning_content"), StringSlice("refusal"), StringSlice("annotations"), StringSlice("content"), StringSlice("tool_calls")]:
        var bounds = deepseek_json_object_member(source, delta[0], delta[1], key)
        if bounds[0] < 0:
            continue
        if not first and not deepseek_put_byte(writer, 44):
            return False
        first = False
        if not deepseek_put_byte(writer, 34) or not deepseek_put_literal(writer, key) or not deepseek_put_literal(writer, StringSlice('":')):
            return False
        if not deepseek_put_view_range(writer, source, bounds[0], bounds[1]):
            return False
    return deepseek_put_byte(writer, 125)


def deepseek_put_stream_response_metadata(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    if input.role_present != 1 or input.role.len == 0:
        return False
    var has_metadata = (
        input.metadata_present == 1
        or input.reasoning_content_present == 1
        or input.content_present == 1
        or input.item_present == 1
        or input.name_present == 1
        or input.signature_present == 1
    )
    if not has_metadata:
        return deepseek_put_literal(writer, StringSlice("null"))
    if input.metadata_present == 1 and not deepseek_json_fragment_valid(input.metadata):
        return False
    if input.item_present == 1 and not deepseek_json_fragment_valid(input.item):
        return False
    if not deepseek_put_byte(writer, 123) or not deepseek_put_json_string(writer, input.role) or not deepseek_put_literal(writer, StringSlice(":{")):
        return False
    var first = True
    if input.metadata_present == 1:
        if not deepseek_put_literal(writer, StringSlice('"logprobs":')) or not deepseek_put_view(writer, input.metadata):
            return False
        first = False
    if input.reasoning_content_present == 1:
        if not first and not deepseek_put_byte(writer, 44):
            return False
        if not deepseek_put_literal(writer, StringSlice('"reasoning_content":')) or not deepseek_put_json_string(writer, input.reasoning_content):
            return False
        first = False
    if input.content_present == 1:
        if not first and not deepseek_put_byte(writer, 44):
            return False
        if not deepseek_put_literal(writer, StringSlice('"refusal":')) or not deepseek_put_json_string(writer, input.content):
            return False
        first = False
    if input.item_present == 1:
        if not first and not deepseek_put_byte(writer, 44):
            return False
        if not deepseek_put_literal(writer, StringSlice('"annotations":')) or not deepseek_put_view(writer, input.item):
            return False
        first = False
    if input.name_present == 1:
        if not first and not deepseek_put_byte(writer, 44):
            return False
        if not deepseek_put_literal(writer, StringSlice('"finish_reason":')) or not deepseek_put_json_string(writer, input.name):
            return False
        first = False
    if input.signature_present == 1:
        if not first and not deepseek_put_byte(writer, 44):
            return False
        if not deepseek_put_literal(writer, StringSlice('"system_fingerprint":')) or not deepseek_put_json_string(writer, input.signature):
            return False
    return deepseek_put_literal(writer, StringSlice("}}"))

def deepseek_put_primitive_request_fields(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    if input.input_present != 1 or not deepseek_json_fragment_valid(input.input):
        return False
    var source = input.input.copy()
    var source_start = deepseek_json_skip_ws(source, 0, Int64(source.len))
    var source_end = deepseek_json_value_end(source, source_start, Int64(source.len), 0)
    if source_end != Int64(source.len) or source_start < 0 or deepseek_json_byte(source, source_start) != 123:
        return False
    var temperature = deepseek_json_object_member(source, source_start, source_end, StringSlice("temperature"))
    var top_p = deepseek_json_object_member(source, source_start, source_end, StringSlice("top_p"))
    var logprobs = deepseek_json_object_member(source, source_start, source_end, StringSlice("logprobs"))
    var max_start: Int64 = -1
    var max_end: Int64 = -1
    for key in [StringSlice("max_output_tokens"), StringSlice("max_tokens"), StringSlice("max_completion_tokens")]:
        var bounds = deepseek_json_object_member(source, source_start, source_end, key)
        if bounds[0] >= 0:
            max_start = bounds[0]
            max_end = bounds[1]
    if not deepseek_put_byte(writer, 123):
        return False
    var first = True
    var first_ptr = Pointer(to=first)
    if not deepseek_put_request_raw_member(writer, StringSlice('"temperature":'), source, temperature, first_ptr):
        return False
    if not deepseek_put_request_raw_member(writer, StringSlice('"top_p":'), source, top_p, first_ptr):
        return False
    var max_bounds = InlineArray[Int64, 2](fill=-1)
    max_bounds[0] = max_start
    max_bounds[1] = max_end
    if not deepseek_put_request_raw_member(writer, StringSlice('"max_tokens":'), source, max_bounds, first_ptr):
        return False
    if not deepseek_put_request_raw_member(writer, StringSlice('"logprobs":'), source, logprobs, first_ptr):
        return False
    return deepseek_put_byte(writer, 125)


def deepseek_trim_start(view: ProdexRichStringView) -> Int64:
    var start: Int64 = 0
    while start < Int64(view.len):
        var value = deepseek_json_byte(view, start)
        if value != 9 and value != 10 and value != 13 and value != 32:
            break
        start += 1
    return start


def deepseek_trim_end(view: ProdexRichStringView, start: Int64) -> Int64:
    var end = Int64(view.len)
    while end > start:
        var value = deepseek_json_byte(view, end - 1)
        if value != 9 and value != 10 and value != 13 and value != 32:
            break
        end -= 1
    return end


def deepseek_trimmed_ascii_equals(
    view: ProdexRichStringView, literal: StringSlice
) -> Bool:
    var start = deepseek_trim_start(view)
    var end = deepseek_trim_end(view, start)
    var length = Int64(literal.byte_length())
    if end - start != length:
        return False
    var expected = literal.unsafe_ptr()
    var actual = rich_view_ptr(view)
    for index in range(length):
        var value = actual[unsafe_offset=start + index]
        var wanted = expected[unsafe_offset=index]
        if value >= 65 and value <= 90:
            value += 32
        if wanted >= 65 and wanted <= 90:
            wanted += 32
        if value != wanted:
            return False
    return True


def deepseek_put_reasoning_parameters(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    if input.reasoning_content_present != 1 or input.reasoning_content.len == 0:
        return False
    var effort = input.reasoning_content.copy()
    var is_xhigh = deepseek_trimmed_ascii_equals(effort, StringSlice("xhigh"))
    var is_max = deepseek_trimmed_ascii_equals(effort, StringSlice("max"))
    var is_high = deepseek_trimmed_ascii_equals(effort, StringSlice("high"))
    var is_medium = deepseek_trimmed_ascii_equals(effort, StringSlice("medium"))
    var is_low = deepseek_trimmed_ascii_equals(effort, StringSlice("low"))
    var is_minimal = deepseek_trimmed_ascii_equals(effort, StringSlice("minimal"))
    var is_none = deepseek_trimmed_ascii_equals(effort, StringSlice("none"))
    if input.stream == 1:
        if is_xhigh or is_max or is_high:
            return deepseek_put_literal(writer, StringSlice('{"reasoning_effort":"high"}'))
        if is_medium:
            return deepseek_put_literal(writer, StringSlice('{"reasoning_effort":"medium"}'))
        if is_low:
            return deepseek_put_literal(writer, StringSlice('{"reasoning_effort":"low"}'))
        if is_minimal:
            return deepseek_put_literal(writer, StringSlice('{"reasoning_effort":"minimal"}'))
        if is_none:
            return deepseek_put_literal(writer, StringSlice('{"reasoning_effort":"none"}'))
        return False
    if is_xhigh or is_max:
        return deepseek_put_literal(writer, StringSlice('{"thinking":{"type":"enabled"},"reasoning_effort":"max"}'))
    if is_high or is_medium or is_low:
        return deepseek_put_literal(writer, StringSlice('{"thinking":{"type":"enabled"},"reasoning_effort":"high"}'))
    if is_minimal or is_none:
        return deepseek_put_literal(writer, StringSlice('{"thinking":{"type":"disabled"}}'))
    return False


def deepseek_put_response_format(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    if input.role_present != 1 or input.role.len == 0:
        return False
    return deepseek_put_literal(writer, StringSlice('{"type":"json_object"}'))


def deepseek_put_user_id(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    if input.input_present != 1:
        return False
    var start = deepseek_trim_start(input.input)
    var end = deepseek_trim_end(input.input, start)
    return deepseek_put_json_string_range(writer, input.input, start, end)


def deepseek_put_function_call(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
    include_arguments: Bool,
) -> Bool:
    if not deepseek_put_literal(writer, StringSlice('{"type":"function_call","call_id":')):
        return False
    if not deepseek_put_json_string(writer, input.call_id):
        return False
    if not deepseek_put_literal(writer, StringSlice(',"name":')) or not deepseek_put_json_string(writer, input.name):
        return False
    if include_arguments:
        if not deepseek_put_literal(writer, StringSlice(',"arguments":')) or not deepseek_put_json_string(writer, input.arguments):
            return False
    if input.namespace_present == 1:
        if not deepseek_put_literal(writer, StringSlice(',"namespace":')) or not deepseek_put_json_string(writer, input.namespace):
            return False
    if input.signature_present == 1:
        if not deepseek_put_literal(writer, StringSlice(',"gemini_thought_signature":')) or not deepseek_put_json_string(writer, input.signature):
            return False
    return deepseek_put_byte(writer, 125)


def deepseek_put_event_prefix(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    event_type: StringSlice,
    sequence_number: UInt64,
    created_at: UInt64,
    include_created_at: Bool,
) -> Bool:
    if not deepseek_put_literal(writer, StringSlice('{"type":"')):
        return False
    if not deepseek_put_literal(writer, event_type):
        return False
    if not deepseek_put_literal(writer, StringSlice('","sequence_number":')) or not deepseek_put_u64(writer, sequence_number):
        return False
    if include_created_at:
        if not deepseek_put_literal(writer, StringSlice(',"created_at":')) or not deepseek_put_u64(writer, created_at):
            return False
    return True



# Request-policy operation ids are intentionally separate from JSON-shaping ids.
# The policy ABI returns decisions and byte ranges into caller-owned JSON.
comptime DEEPSEEK_POLICY_REQUEST_FIELDS: Int64 = 1
comptime DEEPSEEK_POLICY_BETA_FIELDS: Int64 = 2
comptime DEEPSEEK_POLICY_REASONING_SHAPE: Int64 = 3
comptime DEEPSEEK_POLICY_SIMPLE_REQUEST: Int64 = 4


def deepseek_policy_set(
    output: Pointer[mut=True, Int64, _], tag: Int64, start: Int64 = -1, end: Int64 = -1
):
    output[unsafe_offset=0] = tag
    output[unsafe_offset=1] = start
    output[unsafe_offset=2] = end


def deepseek_json_is_true(view: ProdexRichStringView, bounds: InlineArray[Int64, 2]) -> Bool:
    if bounds[0] < 0 or bounds[1] - bounds[0] != 4:
        return False
    var ptr = rich_view_ptr(view)
    return ptr[unsafe_offset=bounds[0]] == 116 and ptr[unsafe_offset=bounds[0] + 1] == 114 and ptr[unsafe_offset=bounds[0] + 2] == 117 and ptr[unsafe_offset=bounds[0] + 3] == 101


def deepseek_json_is_false(view: ProdexRichStringView, bounds: InlineArray[Int64, 2]) -> Bool:
    if bounds[0] < 0 or bounds[1] - bounds[0] != 5:
        return False
    var ptr = rich_view_ptr(view)
    return ptr[unsafe_offset=bounds[0]] == 102 and ptr[unsafe_offset=bounds[0] + 1] == 97 and ptr[unsafe_offset=bounds[0] + 2] == 108 and ptr[unsafe_offset=bounds[0] + 3] == 115 and ptr[unsafe_offset=bounds[0] + 4] == 101


def deepseek_json_is_null(view: ProdexRichStringView, bounds: InlineArray[Int64, 2]) -> Bool:
    if bounds[0] < 0 or bounds[1] - bounds[0] != 4:
        return False
    var ptr = rich_view_ptr(view)
    return ptr[unsafe_offset=bounds[0]] == 110 and ptr[unsafe_offset=bounds[0] + 1] == 117 and ptr[unsafe_offset=bounds[0] + 2] == 108 and ptr[unsafe_offset=bounds[0] + 3] == 108


def deepseek_json_is_bool(view: ProdexRichStringView, bounds: InlineArray[Int64, 2]) -> Bool:
    return deepseek_json_is_true(view, bounds) or deepseek_json_is_false(view, bounds)


def deepseek_json_bounds_is_kind(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2], opening: UInt8
) -> Bool:
    return bounds[0] >= 0 and bounds[1] > bounds[0] and deepseek_json_byte(view, bounds[0]) == opening


def deepseek_json_string_nonempty(view: ProdexRichStringView, bounds: InlineArray[Int64, 2]) -> Bool:
    if not deepseek_json_bounds_is_kind(view, bounds, 34):
        return False
    var ptr = rich_view_ptr(view)
    var index = bounds[0] + 1
    var end = bounds[1] - 1
    while index < end:
        var value = ptr[unsafe_offset=index]
        if value == 92:
            return index + 1 < end
        if value != 9 and value != 10 and value != 13 and value != 32:
            return True
        index += 1
    return False


def deepseek_policy_object_only_key(
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
    allowed: StringSlice,
    output: Pointer[mut=True, Int64, _],
    error_tag: Int64,
) -> Bool:
    if not deepseek_json_bounds_is_kind(view, bounds, 123):
        return False
    var index = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
    while index < bounds[1] - 1:
        var key_start = index
        var key_end = deepseek_json_string_end(view, key_start, bounds[1] - 1)
        if key_end < 0:
            return False
        index = deepseek_json_skip_ws(view, key_end, bounds[1] - 1)
        if index >= bounds[1] - 1 or deepseek_json_byte(view, index) != 58:
            return False
        var value_start = deepseek_json_skip_ws(view, index + 1, bounds[1] - 1)
        var value_end = deepseek_json_value_end(view, value_start, bounds[1] - 1, 0)
        if value_end < 0:
            return False
        if not deepseek_json_raw_equals(view, key_start, key_end, allowed):
            deepseek_policy_set(output, error_tag, key_start, key_end)
            return True
        index = deepseek_json_skip_ws(view, value_end, bounds[1] - 1)
        if index < bounds[1] - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, bounds[1] - 1)
            continue
        if index == bounds[1] - 1:
            break
        return False
    return True


def deepseek_policy_array_all_text(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    if not deepseek_json_bounds_is_kind(view, bounds, 91):
        return False
    var index = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
    while index < bounds[1] - 1:
        var value_end = deepseek_json_value_end(view, index, bounds[1] - 1, 0)
        if value_end < 0 or not deepseek_json_raw_equals(view, index, value_end, StringSlice("text")):
            return False
        index = deepseek_json_skip_ws(view, value_end, bounds[1] - 1)
        if index < bounds[1] - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, bounds[1] - 1)
            continue
        if index == bounds[1] - 1:
            break
        return False
    return True

def deepseek_request_fields_plan(
    view: ProdexRichStringView, output: Pointer[mut=True, Int64, _]
) -> Bool:
    var root = deepseek_input_object_bounds(view)
    if root[0] < 0:
        return False
    if deepseek_json_object_member(view, root[0], root[1], StringSlice("frequency_penalty"))[0] >= 0:
        deepseek_policy_set(output, 1)
        return True
    if deepseek_json_object_member(view, root[0], root[1], StringSlice("presence_penalty"))[0] >= 0:
        deepseek_policy_set(output, 2)
        return True
    var unsupported_index: Int64 = 0
    for key in [StringSlice("n"), StringSlice("seed"), StringSlice("service_tier"), StringSlice("prediction"), StringSlice("logit_bias"), StringSlice("functions"), StringSlice("function_call")]:
        var bounds = deepseek_json_object_member(view, root[0], root[1], key)
        if bounds[0] >= 0:
            deepseek_policy_set(output, 10 + unsupported_index)
            return True
        unsupported_index += 1
    var include = deepseek_json_object_member(view, root[0], root[1], StringSlice("include"))
    if include[0] >= 0 and not deepseek_json_bounds_is_kind(view, include, 91):
        deepseek_policy_set(output, 20)
        return True
    var store = deepseek_json_object_member(view, root[0], root[1], StringSlice("store"))
    if store[0] >= 0 and not deepseek_json_is_bool(view, store):
        deepseek_policy_set(output, 21)
        return True
    var background = deepseek_json_object_member(view, root[0], root[1], StringSlice("background"))
    if background[0] >= 0:
        if deepseek_json_is_true(view, background):
            deepseek_policy_set(output, 22)
            return True
        if not deepseek_json_is_false(view, background):
            deepseek_policy_set(output, 23)
            return True
    var truncation = deepseek_json_object_member(view, root[0], root[1], StringSlice("truncation"))
    if truncation[0] >= 0:
        if not deepseek_json_bounds_is_kind(view, truncation, 34):
            deepseek_policy_set(output, 26)
            return True
        if deepseek_json_raw_equals(view, truncation[0], truncation[1], StringSlice("auto")):
            deepseek_policy_set(output, 24)
            return True
        if not deepseek_json_raw_equals(view, truncation[0], truncation[1], StringSlice("disabled")):
            deepseek_policy_set(output, 25, truncation[0], truncation[1])
            return True
    if deepseek_json_object_member(view, root[0], root[1], StringSlice("max_tool_calls"))[0] >= 0:
        deepseek_policy_set(output, 27)
        return True
    var text = deepseek_json_object_member(view, root[0], root[1], StringSlice("text"))
    if text[0] >= 0:
        if not deepseek_json_bounds_is_kind(view, text, 123):
            deepseek_policy_set(output, 28)
            return True
        if not deepseek_policy_object_only_key(view, text, StringSlice("format"), output, 29):
            return False
        if output[unsafe_offset=0] != 0:
            return True
    var parallel = deepseek_json_object_member(view, root[0], root[1], StringSlice("parallel_tool_calls"))
    if parallel[0] >= 0:
        if deepseek_json_is_false(view, parallel):
            deepseek_policy_set(output, 30)
            return True
        if not deepseek_json_is_true(view, parallel):
            deepseek_policy_set(output, 31)
            return True
    var stream_options = deepseek_json_object_member(view, root[0], root[1], StringSlice("stream_options"))
    if stream_options[0] >= 0:
        if not deepseek_json_bounds_is_kind(view, stream_options, 123):
            deepseek_policy_set(output, 32)
            return True
        var stream = deepseek_json_object_member(view, root[0], root[1], StringSlice("stream"))
        if not deepseek_json_is_true(view, stream):
            deepseek_policy_set(output, 33)
            return True
        if not deepseek_policy_object_only_key(view, stream_options, StringSlice("include_usage"), output, 34):
            return False
        if output[unsafe_offset=0] != 0:
            return True
        var include_usage = deepseek_json_object_member(view, stream_options[0], stream_options[1], StringSlice("include_usage"))
        if include_usage[0] >= 0:
            if deepseek_json_is_false(view, include_usage):
                deepseek_policy_set(output, 35)
                return True
            if not deepseek_json_is_true(view, include_usage):
                deepseek_policy_set(output, 36)
                return True
    var modalities = deepseek_json_object_member(view, root[0], root[1], StringSlice("modalities"))
    if modalities[0] >= 0:
        if not deepseek_json_bounds_is_kind(view, modalities, 91):
            deepseek_policy_set(output, 37)
            return True
        if not deepseek_policy_array_all_text(view, modalities):
            deepseek_policy_set(output, 38)
            return True
    if deepseek_json_object_member(view, root[0], root[1], StringSlice("audio"))[0] >= 0:
        deepseek_policy_set(output, 39)
        return True
    return True


def deepseek_beta_fields_plan(
    view: ProdexRichStringView, output: Pointer[mut=True, Int64, _]
) -> Bool:
    var root = deepseek_input_object_bounds(view)
    if root[0] < 0:
        return False
    if deepseek_json_object_member(view, root[0], root[1], StringSlice("prefix"))[0] >= 0:
        deepseek_policy_set(output, 1)
    elif deepseek_json_object_member(view, root[0], root[1], StringSlice("suffix"))[0] >= 0:
        deepseek_policy_set(output, 2)
    elif deepseek_json_object_member(view, root[0], root[1], StringSlice("prompt"))[0] >= 0:
        deepseek_policy_set(output, 3)
    return True


def deepseek_reasoning_shape_plan(
    view: ProdexRichStringView, output: Pointer[mut=True, Int64, _]
) -> Bool:
    var root = deepseek_input_object_bounds(view)
    if root[0] < 0:
        return False
    var reasoning = deepseek_json_object_member(view, root[0], root[1], StringSlice("reasoning"))
    if reasoning[0] >= 0:
        if not deepseek_json_bounds_is_kind(view, reasoning, 123):
            deepseek_policy_set(output, 1)
            return True
        if not deepseek_policy_object_only_key(view, reasoning, StringSlice("effort"), output, 2):
            return False
        if output[unsafe_offset=0] != 0:
            return True
        var effort = deepseek_json_object_member(view, reasoning[0], reasoning[1], StringSlice("effort"))
        if effort[0] >= 0:
            if not deepseek_json_bounds_is_kind(view, effort, 34):
                deepseek_policy_set(output, 3)
                return True
            deepseek_policy_set(output, 10, effort[0], effort[1])
            return True
    var effort = deepseek_json_object_member(view, root[0], root[1], StringSlice("reasoning_effort"))
    if effort[0] >= 0:
        if not deepseek_json_bounds_is_kind(view, effort, 34):
            deepseek_policy_set(output, 4)
            return True
        deepseek_policy_set(output, 10, effort[0], effort[1])
    return True

def deepseek_simple_content_item(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    if start < 0 or end <= start or deepseek_json_byte(view, start) != 123:
        return False
    var kind = deepseek_json_object_member(view, start, end, StringSlice("type"))
    if kind[0] >= 0 and not (
        deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("input_text"))
        or deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("output_text"))
        or deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("text"))
    ):
        return False
    for key in [StringSlice("text"), StringSlice("input_text"), StringSlice("output_text")]:
        var value = deepseek_json_object_member(view, start, end, key)
        if deepseek_json_bounds_is_kind(view, value, 34):
            return True
    return False


def deepseek_simple_content(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    if bounds[0] < 0:
        return True
    if deepseek_json_bounds_is_kind(view, bounds, 34):
        return True
    if not deepseek_json_bounds_is_kind(view, bounds, 91):
        return False
    var index = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
    while index < bounds[1] - 1:
        var value_end = deepseek_json_value_end(view, index, bounds[1] - 1, 0)
        if value_end < 0 or not deepseek_simple_content_item(view, index, value_end):
            return False
        index = deepseek_json_skip_ws(view, value_end, bounds[1] - 1)
        if index < bounds[1] - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, bounds[1] - 1)
            continue
        if index == bounds[1] - 1:
            break
        return False
    return True


def deepseek_simple_has_string_member(
    view: ProdexRichStringView, start: Int64, end: Int64, key: StringSlice
) -> Bool:
    return deepseek_json_bounds_is_kind(view, deepseek_json_object_member(view, start, end, key), 34)


def deepseek_simple_call_id_shape(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    for key in [StringSlice("call_id"), StringSlice("tool_call_id"), StringSlice("id")]:
        var bounds = deepseek_json_object_member(view, start, end, key)
        if bounds[0] >= 0:
            return deepseek_json_bounds_is_kind(view, bounds, 34)
    return True


def deepseek_simple_has_name(
    view: ProdexRichStringView, start: Int64, end: Int64, include_function: Bool
) -> Bool:
    for key in [StringSlice("name"), StringSlice("tool_name")]:
        if deepseek_simple_has_string_member(view, start, end, key):
            return True
    if include_function:
        var function = deepseek_json_object_member(view, start, end, StringSlice("function"))
        if deepseek_json_bounds_is_kind(view, function, 123):
            return deepseek_simple_has_string_member(view, function[0], function[1], StringSlice("name"))
    return False


def deepseek_simple_has_result(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    for key in [StringSlice("output"), StringSlice("content"), StringSlice("result"), StringSlice("error")]:
        if deepseek_json_object_member(view, start, end, key)[0] >= 0:
            return True
    return False


def deepseek_simple_has_any_string_id(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    for key in [StringSlice("call_id"), StringSlice("tool_call_id"), StringSlice("id")]:
        if deepseek_simple_has_string_member(view, start, end, key):
            return True
    return False


def deepseek_simple_string_array(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    if not deepseek_json_bounds_is_kind(view, bounds, 91):
        return False
    var index = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
    while index < bounds[1] - 1:
        var value_end = deepseek_json_value_end(view, index, bounds[1] - 1, 0)
        if value_end < 0 or deepseek_json_byte(view, index) != 34:
            return False
        index = deepseek_json_skip_ws(view, value_end, bounds[1] - 1)
        if index < bounds[1] - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, bounds[1] - 1)
            continue
        if index == bounds[1] - 1:
            break
        return False
    return True


def deepseek_simple_input_item(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    if start < 0 or end <= start or deepseek_json_byte(view, start) != 123:
        return False
    var kind = deepseek_json_object_member(view, start, end, StringSlice("type"))
    if kind[0] >= 0:
        if deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("function_call_output")):
            return deepseek_simple_has_string_member(view, start, end, StringSlice("call_id")) and deepseek_json_object_member(view, start, end, StringSlice("output"))[0] >= 0
        if deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("mcp_tool_result")) or deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("mcp_call_output")):
            return deepseek_simple_has_any_string_id(view, start, end) and deepseek_simple_call_id_shape(view, start, end) and deepseek_simple_has_result(view, start, end)
        if deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("custom_tool_call_output")):
            return deepseek_simple_has_string_member(view, start, end, StringSlice("call_id")) and deepseek_simple_has_result(view, start, end)
        if deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("function_call")) or deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("mcp_call")):
            return deepseek_simple_has_name(view, start, end, True) and deepseek_simple_call_id_shape(view, start, end)
        if deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("custom_tool_call")):
            return deepseek_simple_has_name(view, start, end, False) and deepseek_simple_call_id_shape(view, start, end)
        if deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("local_shell_call")):
            var command = deepseek_json_object_member(view, start, end, StringSlice("command"))
            var has_command = deepseek_json_bounds_is_kind(view, command, 34)
            if not has_command:
                var action = deepseek_json_object_member(view, start, end, StringSlice("action"))
                if deepseek_json_bounds_is_kind(view, action, 123):
                    var nested = deepseek_json_object_member(view, action[0], action[1], StringSlice("command"))
                    has_command = deepseek_simple_string_array(view, nested)
            return has_command and deepseek_simple_call_id_shape(view, start, end)
        if not deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("message")):
            return False
    var role = deepseek_json_object_member(view, start, end, StringSlice("role"))
    if role[0] >= 0 and not (
        deepseek_json_raw_equals(view, role[0], role[1], StringSlice("system"))
        or deepseek_json_raw_equals(view, role[0], role[1], StringSlice("user"))
        or deepseek_json_raw_equals(view, role[0], role[1], StringSlice("assistant"))
        or deepseek_json_raw_equals(view, role[0], role[1], StringSlice("tool"))
    ):
        return False
    return deepseek_simple_content(view, deepseek_json_object_member(view, start, end, StringSlice("content")))

def deepseek_simple_tools(view: ProdexRichStringView, bounds: InlineArray[Int64, 2]) -> Bool:
    if not deepseek_json_bounds_is_kind(view, bounds, 91):
        return False
    var index = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
    while index < bounds[1] - 1:
        var value_end = deepseek_json_value_end(view, index, bounds[1] - 1, 0)
        if value_end < 0 or deepseek_json_byte(view, index) != 123:
            return False
        var kind = deepseek_json_object_member(view, index, value_end, StringSlice("type"))
        var function = deepseek_json_object_member(view, index, value_end, StringSlice("function"))
        if not deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("function")) or not deepseek_json_bounds_is_kind(view, function, 123):
            return False
        var name = deepseek_json_object_member(view, function[0], function[1], StringSlice("name"))
        if not deepseek_json_string_nonempty(view, name):
            return False
        index = deepseek_json_skip_ws(view, value_end, bounds[1] - 1)
        if index < bounds[1] - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, bounds[1] - 1)
            continue
        if index == bounds[1] - 1:
            break
        return False
    return True


def deepseek_simple_tool_choice(view: ProdexRichStringView, bounds: InlineArray[Int64, 2]) -> Bool:
    if bounds[0] < 0 or deepseek_json_is_null(view, bounds):
        return True
    if deepseek_json_bounds_is_kind(view, bounds, 34):
        return (
            deepseek_json_raw_equals(view, bounds[0], bounds[1], StringSlice("auto"))
            or deepseek_json_raw_equals(view, bounds[0], bounds[1], StringSlice("none"))
            or deepseek_json_raw_equals(view, bounds[0], bounds[1], StringSlice("required"))
        )
    if not deepseek_json_bounds_is_kind(view, bounds, 123):
        return False
    var kind = deepseek_json_object_member(view, bounds[0], bounds[1], StringSlice("type"))
    var name = deepseek_json_object_member(view, bounds[0], bounds[1], StringSlice("name"))
    return deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("function")) and deepseek_json_string_nonempty(view, name)


def deepseek_simple_request_plan(
    view: ProdexRichStringView,
    previous_response_bound: Bool,
    output: Pointer[mut=True, Int64, _],
) -> Bool:
    var root = deepseek_input_object_bounds(view)
    if root[0] < 0:
        return False
    if previous_response_bound or deepseek_json_object_member(view, root[0], root[1], StringSlice("web_search_options"))[0] >= 0 or deepseek_json_object_member(view, root[0], root[1], StringSlice("safety_identifier"))[0] >= 0:
        deepseek_policy_set(output, 1)
        return True
    var response_format = deepseek_json_object_member(view, root[0], root[1], StringSlice("response_format"))
    if response_format[0] >= 0:
        if not deepseek_json_bounds_is_kind(view, response_format, 123):
            deepseek_policy_set(output, 1)
            return True
        var kind = deepseek_json_object_member(view, response_format[0], response_format[1], StringSlice("type"))
        if not (
            deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("text"))
            or deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("json_object"))
            or deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("json_schema"))
            or deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("json"))
            or deepseek_json_raw_equals(view, kind[0], kind[1], StringSlice("structured_output"))
        ):
            deepseek_policy_set(output, 1)
            return True
    var tools = deepseek_json_object_member(view, root[0], root[1], StringSlice("tools"))
    if tools[0] >= 0 and not deepseek_simple_tools(view, tools):
        deepseek_policy_set(output, 1)
        return True
    var tool_choice = deepseek_json_object_member(view, root[0], root[1], StringSlice("tool_choice"))
    if tool_choice[0] >= 0 and not deepseek_simple_tool_choice(view, tool_choice):
        deepseek_policy_set(output, 1)
        return True
    var input = deepseek_json_object_member(view, root[0], root[1], StringSlice("input"))
    if deepseek_json_bounds_is_kind(view, input, 34):
        return True
    if not deepseek_json_bounds_is_kind(view, input, 91):
        deepseek_policy_set(output, 1)
        return True
    var index = deepseek_json_skip_ws(view, input[0] + 1, input[1] - 1)
    while index < input[1] - 1:
        var value_end = deepseek_json_value_end(view, index, input[1] - 1, 0)
        if value_end < 0 or not deepseek_simple_input_item(view, index, value_end):
            deepseek_policy_set(output, 1)
            return True
        index = deepseek_json_skip_ws(view, value_end, input[1] - 1)
        if index < input[1] - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, input[1] - 1)
            continue
        if index == input[1] - 1:
            break
        deepseek_policy_set(output, 1)
        return True
    return True


def deepseek_request_policy_v1(
    abi_version: Int64,
    operation: Int64,
    input_address: UInt,
    input_length: Int64,
    flag: Int64,
    scalar: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return DEEPSEEK_KERNEL_STATUS_ABI
    if input_length < 0 or input_length > DEEPSEEK_KERNEL_MAX_BYTES or output_address == 0:
        return DEEPSEEK_KERNEL_STATUS_INVALID
    if input_length > 0 and input_address == 0:
        return DEEPSEEK_KERNEL_STATUS_INVALID
    if flag < 0 or flag > 1 or scalar < 0:
        return DEEPSEEK_KERNEL_STATUS_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    deepseek_policy_set(output, 0)
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, DEEPSEEK_KERNEL_MAX_BYTES):
        return DEEPSEEK_KERNEL_STATUS_UTF8
    var ok = False
    if operation == DEEPSEEK_POLICY_REQUEST_FIELDS:
        ok = deepseek_request_fields_plan(view, output)
    elif operation == DEEPSEEK_POLICY_BETA_FIELDS:
        ok = deepseek_beta_fields_plan(view, output)
    elif operation == DEEPSEEK_POLICY_REASONING_SHAPE:
        ok = deepseek_reasoning_shape_plan(view, output)
    elif operation == DEEPSEEK_POLICY_SIMPLE_REQUEST:
        ok = deepseek_simple_request_plan(view, flag == 1, output)
    else:
        return DEEPSEEK_KERNEL_STATUS_INVALID
    return DEEPSEEK_KERNEL_STATUS_OK if ok else DEEPSEEK_KERNEL_STATUS_INVALID


def deepseek_write_operation(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    var operation = input.operation
    if operation == DEEPSEEK_RAW_COMMON_REQUEST:
        return deepseek_raw_common_request(writer, input)
    if operation == DEEPSEEK_REQUEST_METADATA:
        return deepseek_put_request_metadata(writer, input)
    if operation == DEEPSEEK_STRICT_FUNCTION_SCHEMA:
        if not deepseek_json_fragment_valid(input.input):
            return False
        var start = deepseek_json_skip_ws(input.input, 0, Int64(input.input.len))
        var end = deepseek_json_value_end(input.input, start, Int64(input.input.len), 0)
        if end != Int64(input.input.len):
            return False
        return deepseek_schema_write(input.input, start, end, writer, 0)
    if operation == DEEPSEEK_PRIMITIVE_REQUEST_FIELDS:
        return deepseek_put_primitive_request_fields(writer, input)
    if operation == DEEPSEEK_REASONING_PARAMETERS:
        return deepseek_put_reasoning_parameters(writer, input)
    if operation == DEEPSEEK_RESPONSE_FORMAT:
        return deepseek_put_response_format(writer, input)
    if operation == DEEPSEEK_USER_ID:
        return deepseek_put_user_id(writer, input)
    if operation == DEEPSEEK_REQUEST_BODY:
        if not deepseek_put_literal(writer, StringSlice('{"model":')) or not deepseek_put_json_string(writer, input.model):
            return False
        if not deepseek_put_literal(writer, StringSlice(',"stream":')):
            return False
        if input.stream == 1:
            if not deepseek_put_literal(writer, StringSlice("true")):
                return False
        else:
            if not deepseek_put_literal(writer, StringSlice("false")):
                return False
        if not deepseek_put_literal(writer, StringSlice(',"messages":')) or not deepseek_put_view(writer, input.messages):
            return False
        if not deepseek_put_optional_view(writer, StringSlice(',"tools":'), input.tools_present, input.tools):
            return False
        if not deepseek_put_optional_view(writer, StringSlice(',"tool_choice":'), input.tool_choice_present, input.tool_choice):
            return False
        if not deepseek_put_extra_fields(writer, input.extra_present, input.extra):
            return False
        return deepseek_put_byte(writer, 125)
    if operation == DEEPSEEK_SYSTEM_MESSAGE or operation == DEEPSEEK_USER_MESSAGE:
        var role = StringSlice("user")
        if operation == DEEPSEEK_SYSTEM_MESSAGE:
            role = StringSlice("system")
        return (
            deepseek_put_literal(writer, StringSlice('{"role":"'))
            and deepseek_put_literal(writer, role)
            and deepseek_put_literal(writer, StringSlice('","content":'))
            and deepseek_put_json_string(writer, input.content)
            and deepseek_put_byte(writer, 125)
        )
    if operation == DEEPSEEK_MESSAGE:
        if not deepseek_put_literal(writer, StringSlice('{"role":')) or not deepseek_put_json_string(writer, input.role):
            return False
        if not deepseek_put_literal(writer, StringSlice(',"content":')) or not deepseek_put_json_string(writer, input.content):
            return False
        if input.call_id_present == 1:
            if not deepseek_put_literal(writer, StringSlice(',"tool_call_id":')) or not deepseek_put_json_string(writer, input.call_id):
                return False
        if not deepseek_put_optional_view(writer, StringSlice(',"tool_calls":'), input.tool_calls_present, input.tool_calls):
            return False
        return deepseek_put_byte(writer, 125)
    if operation == DEEPSEEK_TOOL_CALL_MESSAGE:
        if not deepseek_put_literal(writer, StringSlice('{"role":"assistant","content":"","tool_calls":[{')):
            return False
        if not deepseek_put_literal(writer, StringSlice('"id":')) or not deepseek_put_json_string(writer, input.call_id):
            return False
        if not deepseek_put_literal(writer, StringSlice(',"type":"function","function":{"name":')) or not deepseek_put_json_string(writer, input.name):
            return False
        if not deepseek_put_literal(writer, StringSlice(',"arguments":')) or not deepseek_put_json_string(writer, input.arguments):
            return False
        if not deepseek_put_literal(writer, StringSlice("}")):
            return False
        if input.signature_present == 1:
            if not deepseek_put_literal(writer, StringSlice(',"gemini_thought_signature":')) or not deepseek_put_json_string(writer, input.signature):
                return False
        return deepseek_put_literal(writer, StringSlice("}]}"))
    if operation == DEEPSEEK_TOOL_MESSAGE:
        if not deepseek_put_literal(writer, StringSlice('{"role":"tool","tool_call_id":')):
            return False
        if not deepseek_put_json_string(writer, input.call_id) or not deepseek_put_literal(writer, StringSlice(',"content":')):
            return False
        if input.input_present == 1:
            if not deepseek_put_view(writer, input.input):
                return False
        elif not deepseek_put_json_string(writer, input.content):
            return False
        return deepseek_put_byte(writer, 125)
    if operation == DEEPSEEK_RESPONSE_VALUE or operation == DEEPSEEK_STREAM_RESPONSE_VALUE:
        if not deepseek_put_literal(writer, StringSlice('{"id":')) or not deepseek_put_json_string(writer, input.response_id):
            return False
        if not deepseek_put_literal(writer, StringSlice(',"output":')) or not deepseek_put_view(writer, input.output):
            return False
        if not deepseek_put_optional_string(writer, StringSlice(',"model":'), input.model_present, input.model):
            return False
        if not deepseek_put_optional_view(writer, StringSlice(',"usage":'), input.usage_present, input.usage):
            return False
        if not deepseek_put_optional_view(writer, StringSlice(',"metadata":'), input.metadata_present, input.metadata):
            return False
        return deepseek_put_byte(writer, 125)
    if operation == DEEPSEEK_BUFFERED_RESPONSE:
        if not deepseek_put_literal(writer, StringSlice('{"id":')) or not deepseek_put_json_string(writer, input.response_id):
            return False
        if not deepseek_put_literal(writer, StringSlice(',"object":"response","created_at":')) or not deepseek_put_u64(writer, input.created_at):
            return False
        if not deepseek_put_literal(writer, StringSlice(',"model":')) or not deepseek_put_json_string(writer, input.model):
            return False
        if not deepseek_put_literal(writer, StringSlice(',"output":')) or not deepseek_put_view(writer, input.output):
            return False
        if input.error_code_present == 1 or input.error_message_present == 1:
            if not deepseek_put_literal(writer, StringSlice(',"status":"failed","error":{"code":')):
                return False
            if not deepseek_put_json_string(writer, input.error_code):
                return False
            if not deepseek_put_literal(writer, StringSlice(',"message":')) or not deepseek_put_json_string(writer, input.error_message):
                return False
            if not deepseek_put_literal(writer, StringSlice("}")):
                return False
        if not deepseek_put_optional_view(writer, StringSlice(',"usage":'), input.usage_present, input.usage):
            return False
        if not deepseek_put_optional_view(writer, StringSlice(',"metadata":'), input.metadata_present, input.metadata):
            return False
        return deepseek_put_byte(writer, 125)
    if operation == DEEPSEEK_RESPONSE_CREATED_EVENT:
        return (
            deepseek_put_event_prefix(writer, StringSlice("response.created"), input.sequence_number, input.created_at, True)
            and deepseek_put_literal(writer, StringSlice(',"response":{"id":'))
            and deepseek_put_json_string(writer, input.response_id)
            and deepseek_put_literal(writer, StringSlice("}}"))
        )
    if operation == DEEPSEEK_RESPONSE_COMPLETED_EVENT:
        return (
            deepseek_put_event_prefix(writer, StringSlice("response.completed"), input.sequence_number, input.created_at, True)
            and deepseek_put_literal(writer, StringSlice(',"response":'))
            and deepseek_put_view(writer, input.response)
            and deepseek_put_byte(writer, 125)
        )
    if operation == DEEPSEEK_OUTPUT_ITEM_ADDED_EVENT or operation == DEEPSEEK_OUTPUT_ITEM_DONE_EVENT:
        var event_type = StringSlice("response.output_item.added")
        if operation == DEEPSEEK_OUTPUT_ITEM_DONE_EVENT:
            event_type = StringSlice("response.output_item.done")
        return (
            deepseek_put_event_prefix(writer, event_type, input.sequence_number, input.created_at, False)
            and deepseek_put_literal(writer, StringSlice(',"item":'))
            and deepseek_put_view(writer, input.item)
            and deepseek_put_byte(writer, 125)
        )
    if operation == DEEPSEEK_FUNCTION_CALL_ARGUMENTS_DELTA_EVENT:
        return (
            deepseek_put_event_prefix(writer, StringSlice("response.function_call_arguments.delta"), input.sequence_number, input.created_at, False)
            and deepseek_put_literal(writer, StringSlice(',"call_id":'))
            and deepseek_put_json_string(writer, input.call_id)
            and deepseek_put_literal(writer, StringSlice(',"delta":'))
            and deepseek_put_json_string(writer, input.delta)
            and deepseek_put_byte(writer, 125)
        )
    if operation == DEEPSEEK_OUTPUT_TEXT_DELTA_EVENT:
        return (
            deepseek_put_event_prefix(writer, StringSlice("response.output_text.delta"), input.sequence_number, input.created_at, True)
            and deepseek_put_literal(writer, StringSlice(',"response_id":'))
            and deepseek_put_json_string(writer, input.response_id)
            and deepseek_put_literal(writer, StringSlice(',"delta":'))
            and deepseek_put_json_string(writer, input.delta)
            and deepseek_put_byte(writer, 125)
        )
    if operation == DEEPSEEK_OUTPUT_TEXT_ITEM:
        return (
            deepseek_put_literal(writer, StringSlice('{"type":"message","role":"assistant","content":[{"type":"output_text","text":'))
            and deepseek_put_json_string(writer, input.delta)
            and deepseek_put_literal(writer, StringSlice("}]}"))
        )
    if operation == DEEPSEEK_STREAM_ASSISTANT_MESSAGE:
        if not deepseek_put_literal(writer, StringSlice('{"role":"assistant","content":')):
            return False
        if input.content_present == 1:
            if not deepseek_put_json_string(writer, input.content):
                return False
        else:
            if input.tool_calls_present == 1:
                if not deepseek_put_literal(writer, StringSlice('""')):
                    return False
            elif not deepseek_put_literal(writer, StringSlice("null")):
                return False
        if input.reasoning_content_present == 1:
            if not deepseek_put_literal(writer, StringSlice(',"reasoning_content":')) or not deepseek_put_json_string(writer, input.reasoning_content):
                return False
        if not deepseek_put_optional_view(writer, StringSlice(',"tool_calls":'), input.tool_calls_present, input.tool_calls):
            return False
        return deepseek_put_byte(writer, 125)
    if operation == DEEPSEEK_FUNCTION_CALL_ITEM:
        return deepseek_put_function_call(writer, input, True)
    if operation == DEEPSEEK_ADDED_FUNCTION_CALL_ITEM:
        return deepseek_put_function_call(writer, input, False)
    if operation == DEEPSEEK_TOOL_SEARCH_ITEM:
        return (
            deepseek_put_literal(writer, StringSlice('{"type":"tool_search_call","call_id":'))
            and deepseek_put_json_string(writer, input.call_id)
            and deepseek_put_literal(writer, StringSlice(',"execution":"client","arguments":'))
            and deepseek_put_view(writer, input.arguments)
            and deepseek_put_byte(writer, 125)
        )
    if operation == DEEPSEEK_CUSTOM_TOOL_CALL_ITEM:
        return (
            deepseek_put_literal(writer, StringSlice('{"type":"custom_tool_call","call_id":'))
            and deepseek_put_json_string(writer, input.call_id)
            and deepseek_put_literal(writer, StringSlice(',"name":'))
            and deepseek_put_json_string(writer, input.name)
            and deepseek_put_literal(writer, StringSlice(',"input":'))
            and deepseek_put_json_string(writer, input.input)
            and deepseek_put_byte(writer, 125)
        )
    if operation == DEEPSEEK_FUNCTION_CALL_ARGUMENTS_DELTA_SOURCE:
        return (
            deepseek_put_literal(writer, StringSlice('{"choices":[{"delta":{"tool_calls":[{"id":'))
            and deepseek_put_json_string(writer, input.call_id)
            and deepseek_put_literal(writer, StringSlice(',"function":{"arguments":'))
            and deepseek_put_json_string(writer, input.arguments)
            and deepseek_put_literal(writer, StringSlice("}}]}}]}"))
        )
    if operation == DEEPSEEK_TEXT_DELTA_SOURCE or operation == DEEPSEEK_SSE_TEXT_DELTA:
        if operation == DEEPSEEK_TEXT_DELTA_SOURCE:
            return (
                deepseek_put_literal(writer, StringSlice('{"choices":[{"delta":{"content":'))
                and deepseek_put_json_string(writer, input.delta)
                and deepseek_put_literal(writer, StringSlice("}}]}"))
            )
        return (
            deepseek_put_literal(writer, StringSlice('{"type":"response.output_text.delta","delta":'))
            and deepseek_put_json_string(writer, input.delta)
            and deepseek_put_byte(writer, 125)
        )
    if operation == DEEPSEEK_SSE_FUNCTION_CALL_DELTA:
        if not deepseek_put_literal(writer, StringSlice('{"type":"response.function_call_arguments.delta","delta":')):
            return False
        if not deepseek_put_json_string(writer, input.delta):
            return False
        if input.call_id_present == 1:
            if not deepseek_put_literal(writer, StringSlice(',"call_id":')) or not deepseek_put_json_string(writer, input.call_id):
                return False
        return deepseek_put_byte(writer, 125)
    if operation == DEEPSEEK_STREAM_TOOL_CALL_DELTA:
        return deepseek_put_stream_tool_call_delta(writer, input)
    if operation == DEEPSEEK_STREAM_CHUNK_METADATA:
        return deepseek_put_stream_chunk_metadata(writer, input)
    if operation == DEEPSEEK_STREAM_CHOICE_METADATA:
        return deepseek_put_stream_choice_metadata(writer, input)
    if operation == DEEPSEEK_STREAM_CHOICE_DELTA:
        return deepseek_put_stream_choice_delta(writer, input)
    if operation == DEEPSEEK_STREAM_RESPONSE_METADATA:
        return deepseek_put_stream_response_metadata(writer, input)
    if operation == DEEPSEEK_RESPONSE_METADATA:
        return (
            deepseek_put_literal(writer, StringSlice("{"))
            and deepseek_put_json_string(writer, input.role)
            and deepseek_put_byte(writer, 58)
            and deepseek_put_view(writer, input.metadata)
            and deepseek_put_byte(writer, 125)
        )
    return False


def deepseek_flag_valid(value: Int64) -> Bool:
    return value == 0 or value == 1


def deepseek_input_valid(input: ProdexDeepSeekKernelInput) -> Bool:
    return (
        input.operation >= DEEPSEEK_REQUEST_BODY
        and input.operation <= DEEPSEEK_REQUEST_METADATA
        and
        deepseek_flag_valid(input.stream)
        and deepseek_flag_valid(input.response_id_present)
        and deepseek_flag_valid(input.call_id_present)
        and deepseek_flag_valid(input.model_present)
        and deepseek_flag_valid(input.role_present)
        and deepseek_flag_valid(input.content_present)
        and deepseek_flag_valid(input.reasoning_content_present)
        and deepseek_flag_valid(input.name_present)
        and deepseek_flag_valid(input.namespace_present)
        and deepseek_flag_valid(input.arguments_present)
        and deepseek_flag_valid(input.signature_present)
        and deepseek_flag_valid(input.delta_present)
        and deepseek_flag_valid(input.messages_present)
        and deepseek_flag_valid(input.tools_present)
        and deepseek_flag_valid(input.tool_choice_present)
        and deepseek_flag_valid(input.extra_present)
        and deepseek_flag_valid(input.output_present)
        and deepseek_flag_valid(input.usage_present)
        and deepseek_flag_valid(input.metadata_present)
        and deepseek_flag_valid(input.item_present)
        and deepseek_flag_valid(input.response_present)
        and deepseek_flag_valid(input.tool_calls_present)
        and deepseek_flag_valid(input.input_present)
        and deepseek_flag_valid(input.error_code_present)
        and deepseek_flag_valid(input.error_message_present)
        and rich_view_valid(input.response_id, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.call_id, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.model, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.role, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.content, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.reasoning_content, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.name, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.namespace, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.arguments, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.signature, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.delta, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.messages, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.tools, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.tool_choice, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.extra, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.output, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.usage, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.metadata, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.item, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.response, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.tool_calls, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.input, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.error_code, DEEPSEEK_KERNEL_MAX_BYTES)
        and rich_view_valid(input.error_message, DEEPSEEK_KERNEL_MAX_BYTES)
    )


def deepseek_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return DEEPSEEK_KERNEL_STATUS_ABI
    if input_address == 0 or output_address == 0 or written_address == 0 or output_capacity <= 0:
        return DEEPSEEK_KERNEL_STATUS_INVALID
    var input = Pointer[
        mut=False, ProdexDeepSeekKernelInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not deepseek_input_valid(input[].copy()):
        return DEEPSEEK_KERNEL_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = DeepSeekResponseWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    if not deepseek_write_operation(writer_ptr, input[].copy()):
        if writer.written >= output_capacity:
            written[] = writer.written
            return DEEPSEEK_KERNEL_STATUS_CAPACITY
        return DEEPSEEK_KERNEL_STATUS_INVALID
    written[] = writer.written
    return DEEPSEEK_KERNEL_STATUS_OK

# Coarse Responses -> Chat Completions request shaping. Rust validates public
# error semantics; Mojo owns deterministic JSON transformation.
comptime DEEPSEEK_RAW_REQUEST_RESPONSE_FORMAT_NONE: Int64 = 0
comptime DEEPSEEK_RAW_REQUEST_RESPONSE_FORMAT_JSON_OBJECT: Int64 = 1

def deepseek_raw_present(bounds: InlineArray[Int64, 2]) -> Bool:
    return bounds[0] >= 0 and bounds[1] > bounds[0]

def deepseek_raw_root(view: ProdexRichStringView) -> InlineArray[Int64, 2]:
    var missing = InlineArray[Int64, 2](fill=-1)
    var start = deepseek_json_skip_ws(view, 0, Int64(view.len))
    var end = deepseek_json_value_end(view, start, Int64(view.len), 0)
    if (
        start < 0
        or end != Int64(view.len)
        or deepseek_json_byte(view, start) != 123
    ):
        return missing^
    var root = InlineArray[Int64, 2](fill=-1)
    root[0] = start
    root[1] = end
    return root^

def deepseek_raw_member(
    view: ProdexRichStringView,
    root: InlineArray[Int64, 2],
    key: StringSlice,
) -> InlineArray[Int64, 2]:
    return deepseek_json_object_member(view, root[0], root[1], key)

def deepseek_raw_string_nonempty(
    view: ProdexRichStringView, bounds: InlineArray[Int64, 2]
) -> Bool:
    return (
        deepseek_raw_present(bounds)
        and deepseek_json_byte(view, bounds[0]) == 34
        and bounds[1] - bounds[0] > 2
    )

def deepseek_raw_put_default_or_string(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
    default_value: StringSlice,
) -> Bool:
    if deepseek_raw_present(bounds) and deepseek_json_byte(view, bounds[0]) == 34:
        return deepseek_put_view_range(writer, view, bounds[0], bounds[1])
    return deepseek_put_literal(writer, default_value)

def deepseek_raw_first_of(
    view: ProdexRichStringView,
    root: InlineArray[Int64, 2],
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
) -> InlineArray[Int64, 2]:
    var value = deepseek_raw_member(view, root, first)
    if deepseek_raw_present(value):
        return value^
    value = deepseek_raw_member(view, root, second)
    if deepseek_raw_present(value):
        return value^
    return deepseek_raw_member(view, root, third)

def deepseek_raw_put_message(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    role: StringSlice,
    view: ProdexRichStringView,
    content: InlineArray[Int64, 2],
    call_id: InlineArray[Int64, 2],
    tool_calls: InlineArray[Int64, 2],
) -> Bool:
    if (
        not deepseek_put_literal(writer, StringSlice('{"role":"'))
        or not deepseek_put_literal(writer, role)
        or not deepseek_put_literal(writer, StringSlice('","content":'))
    ):
        return False
    if deepseek_raw_present(content) and deepseek_json_byte(view, content[0]) == 34:
        if not deepseek_put_view_range(writer, view, content[0], content[1]):
            return False
    else:
        if not deepseek_put_literal(writer, StringSlice('""')):
            return False
    if deepseek_raw_present(call_id):
        if (
            not deepseek_put_literal(writer, StringSlice(',"tool_call_id":'))
            or not deepseek_raw_put_default_or_string(
                writer, view, call_id, StringSlice('"call_1"')
            )
        ):
            return False
    if deepseek_raw_present(tool_calls) and deepseek_json_byte(view, tool_calls[0]) == 91:
        if (
            not deepseek_put_literal(writer, StringSlice(',"tool_calls":'))
            or not deepseek_put_view_range(
                writer, view, tool_calls[0], tool_calls[1]
            )
        ):
            return False
    return deepseek_put_byte(writer, 125)

def deepseek_raw_text_from_content(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
) -> Bool:
    if not deepseek_raw_present(bounds):
        return deepseek_put_literal(writer, StringSlice('""'))
    if deepseek_json_byte(view, bounds[0]) == 34:
        return deepseek_put_view_range(writer, view, bounds[0], bounds[1])
    if deepseek_json_byte(view, bounds[0]) != 91:
        return deepseek_put_literal(writer, StringSlice('""'))
    if not deepseek_put_byte(writer, 34):
        return False
    var first_text = True
    var cursor = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
    while cursor < bounds[1] - 1:
        var item_end = deepseek_json_value_end(view, cursor, bounds[1] - 1, 0)
        if item_end < 0:
            return False
        var text = InlineArray[Int64, 2](fill=-1)
        if deepseek_json_byte(view, cursor) == 34:
            text[0] = cursor
            text[1] = item_end
        elif deepseek_json_byte(view, cursor) == 123:
            text = deepseek_json_object_member(
                view, cursor, item_end, StringSlice("text")
            )
            if not deepseek_raw_present(text):
                text = deepseek_json_object_member(
                    view, cursor, item_end, StringSlice("input_text")
                )
            if not deepseek_raw_present(text):
                text = deepseek_json_object_member(
                    view, cursor, item_end, StringSlice("output_text")
                )
        if deepseek_raw_present(text) and deepseek_json_byte(view, text[0]) == 34:
            if not first_text and not deepseek_put_literal(writer, StringSlice("\n")):
                return False
            first_text = False
            if not deepseek_put_view_range(
                writer, view, text[0] + 1, text[1] - 1
            ):
                return False
        cursor = deepseek_json_skip_ws(view, item_end, bounds[1] - 1)
        if cursor < bounds[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, bounds[1] - 1)
            continue
        break
    return deepseek_put_byte(writer, 34)

def deepseek_raw_function_tool_count(
    view: ProdexRichStringView, tools: InlineArray[Int64, 2]
) -> Int64:
    if not deepseek_raw_present(tools) or deepseek_json_byte(view, tools[0]) != 91:
        return 0
    var count: Int64 = 0
    var cursor = deepseek_json_skip_ws(view, tools[0] + 1, tools[1] - 1)
    while cursor < tools[1] - 1:
        var item_end = deepseek_json_value_end(view, cursor, tools[1] - 1, 0)
        if item_end < 0:
            return -1
        if deepseek_json_byte(view, cursor) == 123:
            var kind = deepseek_json_object_member(
                view, cursor, item_end, StringSlice("type")
            )
            if deepseek_raw_present(kind) and deepseek_json_raw_equals(
                view, kind[0], kind[1], StringSlice("function")
            ):
                count += 1
        cursor = deepseek_json_skip_ws(view, item_end, tools[1] - 1)
        if cursor < tools[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, tools[1] - 1)
            continue
        if cursor != tools[1] - 1:
            return -1
        break
    return count

def deepseek_raw_put_function_tools(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    tools: InlineArray[Int64, 2],
) -> Bool:
    if not deepseek_put_byte(writer, 91):
        return False
    var first = True
    var cursor = deepseek_json_skip_ws(view, tools[0] + 1, tools[1] - 1)
    while cursor < tools[1] - 1:
        var item_end = deepseek_json_value_end(view, cursor, tools[1] - 1, 0)
        if item_end < 0:
            return False
        if deepseek_json_byte(view, cursor) == 123:
            var kind = deepseek_json_object_member(
                view, cursor, item_end, StringSlice("type")
            )
            if deepseek_raw_present(kind) and deepseek_json_raw_equals(
                view, kind[0], kind[1], StringSlice("function")
            ):
                if not first and not deepseek_put_byte(writer, 44):
                    return False
                first = False
                if not deepseek_put_view_range(writer, view, cursor, item_end):
                    return False
        cursor = deepseek_json_skip_ws(view, item_end, tools[1] - 1)
        if cursor < tools[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, tools[1] - 1)
            continue
        if cursor != tools[1] - 1:
            return False
        break
    return deepseek_put_byte(writer, 93)

def deepseek_raw_put_common_messages(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    root: InlineArray[Int64, 2],
    normalized_instructions: ProdexRichStringView,
    instructions_present: Bool,
) -> Bool:
    var messages = deepseek_raw_member(view, root, StringSlice("messages"))
    var input = deepseek_raw_member(view, root, StringSlice("input"))
    if not deepseek_put_byte(writer, 91):
        return False
    var has_previous = False
    if instructions_present:
        if (
            not deepseek_put_literal(writer, StringSlice('{"role":"system","content":'))
            or not deepseek_put_json_string(writer, normalized_instructions)
            or not deepseek_put_byte(writer, 125)
        ):
            return False
        has_previous = True
    if deepseek_raw_present(messages) and deepseek_json_byte(view, messages[0]) == 91:
        if messages[1] - messages[0] > 2:
            if has_previous and not deepseek_put_byte(writer, 44):
                return False
            if not deepseek_put_view_range(
                writer, view, messages[0] + 1, messages[1] - 1
            ):
                return False
        return deepseek_put_byte(writer, 93)
    if deepseek_raw_present(input) and deepseek_json_byte(view, input[0]) == 91:
        if not deepseek_raw_put_input_items(
            writer, view, input, has_previous
        ):
            return False
        return deepseek_put_byte(writer, 93)
    if has_previous and not deepseek_put_byte(writer, 44):
        return False
    if deepseek_raw_present(input) and deepseek_json_byte(view, input[0]) == 34:
        if (
            not deepseek_put_literal(writer, StringSlice('{"role":"user","content":'))
            or not deepseek_put_view_range(writer, view, input[0], input[1])
            or not deepseek_put_byte(writer, 125)
        ):
            return False
    else:
        if not deepseek_put_literal(
            writer, StringSlice('{"role":"user","content":""}')
        ):
            return False
    return deepseek_put_byte(writer, 93)

def deepseek_raw_put_tool_choice(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    choice: InlineArray[Int64, 2],
) -> Bool:
    if not deepseek_raw_present(choice):
        return False
    if deepseek_json_byte(view, choice[0]) == 34:
        if (
            deepseek_json_raw_equals(view, choice[0], choice[1], StringSlice("auto"))
            or deepseek_json_raw_equals(view, choice[0], choice[1], StringSlice("none"))
            or deepseek_json_raw_equals(view, choice[0], choice[1], StringSlice("required"))
        ):
            return deepseek_put_view_range(writer, view, choice[0], choice[1])
        return False
    if deepseek_json_byte(view, choice[0]) != 123:
        return False
    var choice_type = deepseek_json_object_member(
        view, choice[0], choice[1], StringSlice("type")
    )
    if not deepseek_raw_present(choice_type) or not deepseek_json_raw_equals(
        view, choice_type[0], choice_type[1], StringSlice("function")
    ):
        return False
    var name = deepseek_json_object_member(
        view, choice[0], choice[1], StringSlice("name")
    )
    if not deepseek_raw_present(name):
        var function = deepseek_json_object_member(
            view, choice[0], choice[1], StringSlice("function")
        )
        if deepseek_raw_present(function) and deepseek_json_byte(view, function[0]) == 123:
            name = deepseek_json_object_member(
                view, function[0], function[1], StringSlice("name")
            )
    if not deepseek_raw_string_nonempty(view, name):
        return False
    return (
        deepseek_put_literal(writer, StringSlice('{"type":"function","function":{"name":'))
        and deepseek_put_view_range(writer, view, name[0], name[1])
        and deepseek_put_literal(writer, StringSlice("}}"))
    )

def deepseek_raw_put_optional_member(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    key: StringSlice,
    bounds: InlineArray[Int64, 2],
) -> Bool:
    if not deepseek_raw_present(bounds):
        return True
    return (
        deepseek_put_literal(writer, key)
        and deepseek_put_view_range(writer, view, bounds[0], bounds[1])
    )


def deepseek_raw_common_request(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    if input.input_present != 1:
        return False
    var source = input.input.copy()
    var root = deepseek_raw_root(source)
    if root[0] < 0:
        return False
    var model = deepseek_raw_member(source, root, StringSlice("model"))
    var stream = deepseek_raw_member(source, root, StringSlice("stream"))
    var tools = deepseek_raw_member(source, root, StringSlice("tools"))
    var tool_choice = deepseek_raw_member(source, root, StringSlice("tool_choice"))
    var temperature = deepseek_raw_member(source, root, StringSlice("temperature"))
    var top_p = deepseek_raw_member(source, root, StringSlice("top_p"))
    var logprobs = deepseek_raw_member(source, root, StringSlice("logprobs"))
    var top_logprobs = deepseek_raw_member(source, root, StringSlice("top_logprobs"))
    var stop = deepseek_raw_first_of(
        source,
        root,
        StringSlice("stop"),
        StringSlice("stop_sequences"),
        StringSlice("stopSequences"),
    )
    var max_tokens = deepseek_raw_member(
        source, root, StringSlice("max_output_tokens")
    )
    var next_max = deepseek_raw_member(source, root, StringSlice("max_tokens"))
    if deepseek_raw_present(next_max):
        max_tokens = next_max.copy()
    next_max = deepseek_raw_member(
        source, root, StringSlice("max_completion_tokens")
    )
    if deepseek_raw_present(next_max):
        max_tokens = next_max.copy()
    var tool_count = deepseek_raw_function_tool_count(source, tools)
    if tool_count < 0:
        return False

    if (
        not deepseek_put_literal(writer, StringSlice('{"model":'))
        or not deepseek_raw_put_default_or_string(
            writer, source, model, StringSlice('"deepseek-chat"')
        )
        or not deepseek_put_literal(writer, StringSlice(',"stream":'))
    ):
        return False
    if deepseek_json_is_true(source, stream):
        if not deepseek_put_literal(writer, StringSlice("true")):
            return False
    elif not deepseek_put_literal(writer, StringSlice("false")):
        return False
    if not deepseek_put_literal(writer, StringSlice(',"messages":')):
        return False
    if not deepseek_raw_put_common_messages(
        writer,
        source,
        root,
        input.reasoning_content,
        input.reasoning_content_present == 1,
    ):
        return False
    if tool_count > 0:
        if (
            not deepseek_put_literal(writer, StringSlice(',"tools":'))
            or not deepseek_raw_put_function_tools(writer, source, tools)
        ):
            return False
    if deepseek_raw_present(tool_choice):
        var saved = writer[].written
        if not deepseek_put_literal(writer, StringSlice(',"tool_choice":')):
            return False
        if not deepseek_raw_put_tool_choice(writer, source, tool_choice):
            writer[].written = saved
    if not deepseek_raw_put_optional_member(
        writer, source, StringSlice(',"temperature":'), temperature
    ):
        return False
    if not deepseek_raw_put_optional_member(
        writer, source, StringSlice(',"top_p":'), top_p
    ):
        return False
    if not deepseek_raw_put_optional_member(
        writer, source, StringSlice(',"max_tokens":'), max_tokens
    ):
        return False
    if not deepseek_raw_put_optional_member(
        writer, source, StringSlice(',"logprobs":'), logprobs
    ):
        return False
    if not deepseek_raw_put_optional_member(
        writer, source, StringSlice(',"top_logprobs":'), top_logprobs
    ):
        return False
    if not deepseek_raw_put_optional_member(
        writer, source, StringSlice(',"stop":'), stop
    ):
        return False
    if input.content_present == 1:
        if (
            not deepseek_put_literal(writer, StringSlice(',"user_id":'))
            or not deepseek_put_json_string(writer, input.content)
        ):
            return False
    if input.sequence_number == 1:
        if not deepseek_put_literal(
            writer, StringSlice(',"response_format":{"type":"json_object"}')
        ):
            return False
    return deepseek_put_byte(writer, 125)

def deepseek_raw_first_member3(
    view: ProdexRichStringView,
    root: InlineArray[Int64, 2],
    first: StringSlice,
    second: StringSlice,
    third: StringSlice,
) -> InlineArray[Int64, 2]:
    var value = deepseek_raw_member(view, root, first)
    if deepseek_raw_present(value):
        return value^
    value = deepseek_raw_member(view, root, second)
    if deepseek_raw_present(value):
        return value^
    return deepseek_raw_member(view, root, third)

def deepseek_raw_function_member(
    view: ProdexRichStringView,
    root: InlineArray[Int64, 2],
    key: StringSlice,
) -> InlineArray[Int64, 2]:
    var function = deepseek_raw_member(view, root, StringSlice("function"))
    if not deepseek_raw_present(function) or deepseek_json_byte(view, function[0]) != 123:
        return InlineArray[Int64, 2](fill=-1)^
    return deepseek_raw_member(view, function, key)

def deepseek_raw_thought_signature(
    view: ProdexRichStringView,
    root: InlineArray[Int64, 2],
) -> InlineArray[Int64, 2]:
    for key in [
        StringSlice("gemini_thought_signature"),
        StringSlice("thought_signature"),
        StringSlice("thoughtSignature"),
    ]:
        var value = deepseek_raw_member(view, root, key)
        if deepseek_raw_string_nonempty(view, value):
            return value^
    var provider = deepseek_raw_member(
        view, root, StringSlice("provider_specific_fields")
    )
    if deepseek_raw_present(provider) and deepseek_json_byte(view, provider[0]) == 123:
        var value = deepseek_raw_member(
            view, provider, StringSlice("thought_signature")
        )
        if deepseek_raw_string_nonempty(view, value):
            return value^
    var extra = deepseek_raw_member(view, root, StringSlice("extra_content"))
    if deepseek_raw_present(extra) and deepseek_json_byte(view, extra[0]) == 123:
        var google = deepseek_raw_member(view, extra, StringSlice("google"))
        if deepseek_raw_present(google) and deepseek_json_byte(view, google[0]) == 123:
            var value = deepseek_raw_member(
                view, google, StringSlice("thought_signature")
            )
            if deepseek_raw_string_nonempty(view, value):
                return value^
    return InlineArray[Int64, 2](fill=-1)^

def deepseek_raw_put_argument_string(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
) -> Bool:
    if not deepseek_raw_present(bounds):
        return deepseek_put_literal(writer, StringSlice('"{}"'))
    if deepseek_json_byte(view, bounds[0]) == 34:
        return deepseek_put_view_range(writer, view, bounds[0], bounds[1])
    return deepseek_put_json_string_range(writer, view, bounds[0], bounds[1])

def deepseek_raw_put_tool_call_message_from_item(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    item: InlineArray[Int64, 2],
) -> Bool:
    var call_id = deepseek_raw_first_member3(
        view,
        item,
        StringSlice("call_id"),
        StringSlice("tool_call_id"),
        StringSlice("id"),
    )
    var name = deepseek_raw_member(view, item, StringSlice("name"))
    if not deepseek_raw_present(name):
        name = deepseek_raw_member(view, item, StringSlice("tool_name"))
    if not deepseek_raw_present(name):
        name = deepseek_raw_function_member(view, item, StringSlice("name"))
    if not deepseek_raw_string_nonempty(view, name):
        return deepseek_put_literal(writer, StringSlice('{"role":"assistant","content":""}'))
    var arguments = deepseek_raw_member(view, item, StringSlice("arguments"))
    if not deepseek_raw_present(arguments):
        arguments = deepseek_raw_member(view, item, StringSlice("input"))
    if not deepseek_raw_present(arguments):
        arguments = deepseek_raw_function_member(
            view, item, StringSlice("arguments")
        )
    var signature = deepseek_raw_thought_signature(view, item)
    if (
        not deepseek_put_literal(writer, StringSlice('{"role":"assistant","content":"","tool_calls":[{"id":'))
        or not deepseek_raw_put_default_or_string(
            writer, view, call_id, StringSlice('"call_1"')
        )
        or not deepseek_put_literal(
            writer, StringSlice(',"type":"function","function":{"name":')
        )
        or not deepseek_put_view_range(writer, view, name[0], name[1])
        or not deepseek_put_literal(writer, StringSlice(',"arguments":'))
        or not deepseek_raw_put_argument_string(writer, view, arguments)
        or not deepseek_put_byte(writer, 125)
    ):
        return False
    if deepseek_raw_present(signature):
        if (
            not deepseek_put_literal(
                writer, StringSlice(',"gemini_thought_signature":')
            )
            or not deepseek_put_view_range(
                writer, view, signature[0], signature[1]
            )
        ):
            return False
    return deepseek_put_literal(writer, StringSlice("}]}"))

def deepseek_raw_put_tool_output_message_from_item(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    item: InlineArray[Int64, 2],
) -> Bool:
    var call_id = deepseek_raw_first_member3(
        view,
        item,
        StringSlice("call_id"),
        StringSlice("tool_call_id"),
        StringSlice("id"),
    )
    var output = deepseek_raw_member(view, item, StringSlice("output"))
    if not deepseek_raw_present(output):
        output = deepseek_raw_member(view, item, StringSlice("content"))
    if not deepseek_raw_present(output):
        output = deepseek_raw_member(view, item, StringSlice("result"))
    if not deepseek_raw_present(output):
        output = deepseek_raw_member(view, item, StringSlice("error"))
    if (
        not deepseek_put_literal(writer, StringSlice('{"role":"tool","tool_call_id":'))
        or not deepseek_raw_put_default_or_string(
            writer, view, call_id, StringSlice('"call_1"')
        )
        or not deepseek_put_literal(writer, StringSlice(',"content":'))
    ):
        return False
    if deepseek_raw_present(output):
        if not deepseek_raw_put_content_text_string(writer, view, output):
            return False
    elif not deepseek_put_literal(writer, StringSlice('""')):
        return False
    return deepseek_put_byte(writer, 125)

def deepseek_raw_put_array_separator(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    first: Pointer[mut=True, Bool, _],
) -> Bool:
    if first[]:
        first[] = False
        return True
    return deepseek_put_byte(writer, 44)

def deepseek_raw_put_generic_input_message(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    item: InlineArray[Int64, 2],
) -> Bool:
    var role = deepseek_raw_member(view, item, StringSlice("role"))
    var content = deepseek_raw_member(view, item, StringSlice("content"))
    if not deepseek_raw_present(content):
        content = deepseek_raw_member(view, item, StringSlice("text"))
    if not deepseek_put_literal(writer, StringSlice('{"role":')):
        return False
    if not deepseek_raw_put_default_or_string(
        writer, view, role, StringSlice('"user"')
    ):
        return False
    if not deepseek_put_literal(writer, StringSlice(',"content":')):
        return False
    if not deepseek_raw_text_from_content(writer, view, content):
        return False
    if (
        deepseek_raw_present(role)
        and deepseek_json_raw_equals(view, role[0], role[1], StringSlice("tool"))
    ):
        var call_id = deepseek_raw_first_member3(
            view,
            item,
            StringSlice("call_id"),
            StringSlice("tool_call_id"),
            StringSlice("id"),
        )
        if deepseek_raw_present(call_id):
            if (
                not deepseek_put_literal(writer, StringSlice(',"tool_call_id":'))
                or not deepseek_raw_put_default_or_string(
                    writer, view, call_id, StringSlice('"call_1"')
                )
            ):
                return False
    var calls = deepseek_raw_member(view, item, StringSlice("tool_calls"))
    if deepseek_raw_present(calls):
        if not deepseek_raw_put_generic_tool_calls(writer, view, calls):
            return False
    return deepseek_put_byte(writer, 125)

def deepseek_raw_mcp_has_result(
    view: ProdexRichStringView, item: InlineArray[Int64, 2]
) -> Bool:
    return (
        deepseek_raw_present(
            deepseek_raw_member(view, item, StringSlice("output"))
        )
        or deepseek_raw_present(
            deepseek_raw_member(view, item, StringSlice("content"))
        )
        or deepseek_raw_present(
            deepseek_raw_member(view, item, StringSlice("result"))
        )
        or deepseek_raw_present(
            deepseek_raw_member(view, item, StringSlice("error"))
        )
    )

def deepseek_raw_put_input_items(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    items: InlineArray[Int64, 2],
    has_prefix: Bool,
) -> Bool:
    var first = not has_prefix
    var first_ptr = Pointer(to=first)
    var cursor = deepseek_json_skip_ws(view, items[0] + 1, items[1] - 1)
    while cursor < items[1] - 1:
        var item_end = deepseek_json_value_end(view, cursor, items[1] - 1, 0)
        if item_end < 0:
            return False
        if deepseek_json_byte(view, cursor) != 123:
            return False
        var item = InlineArray[Int64, 2](fill=-1)
        item[0] = cursor
        item[1] = item_end
        var kind = deepseek_raw_member(view, item, StringSlice("type"))
        var is_function = deepseek_raw_present(kind) and deepseek_json_raw_equals(
            view, kind[0], kind[1], StringSlice("function_call")
        )
        var is_mcp = deepseek_raw_present(kind) and deepseek_json_raw_equals(
            view, kind[0], kind[1], StringSlice("mcp_call")
        )
        var is_custom = deepseek_raw_present(kind) and deepseek_json_raw_equals(
            view, kind[0], kind[1], StringSlice("custom_tool_call")
        )
        var is_local_shell = deepseek_raw_present(kind) and deepseek_json_raw_equals(
            view, kind[0], kind[1], StringSlice("local_shell_call")
        )
        var is_output = False
        if deepseek_raw_present(kind):
            is_output = (
                deepseek_json_raw_equals(
                    view,
                    kind[0],
                    kind[1],
                    StringSlice("function_call_output"),
                )
                or deepseek_json_raw_equals(
                    view,
                    kind[0],
                    kind[1],
                    StringSlice("custom_tool_call_output"),
                )
                or deepseek_json_raw_equals(
                    view,
                    kind[0],
                    kind[1],
                    StringSlice("mcp_tool_result"),
                )
                or deepseek_json_raw_equals(
                    view,
                    kind[0],
                    kind[1],
                    StringSlice("mcp_call_output"),
                )
            )
        if is_function or is_mcp:
            if (
                not deepseek_raw_put_array_separator(writer, first_ptr)
                or not deepseek_raw_put_tool_call_message_from_item(
                    writer, view, item
                )
            ):
                return False
            if is_mcp and deepseek_raw_mcp_has_result(view, item):
                if (
                    not deepseek_raw_put_array_separator(writer, first_ptr)
                    or not deepseek_raw_put_mcp_output_message_from_item(
                        writer, view, item
                    )
                ):
                    return False
        elif is_custom:
            if (
                not deepseek_raw_put_array_separator(writer, first_ptr)
                or not deepseek_raw_put_custom_tool_call_message_from_item(
                    writer, view, item
                )
            ):
                return False
        elif is_local_shell:
            if (
                not deepseek_raw_put_array_separator(writer, first_ptr)
                or not deepseek_raw_put_local_shell_call_message_from_item(
                    writer, view, item
                )
            ):
                return False
        elif is_output:
            if (
                not deepseek_raw_put_array_separator(writer, first_ptr)
                or not deepseek_raw_put_tool_output_message_from_item(
                    writer, view, item
                )
            ):
                return False
        else:
            if (
                not deepseek_raw_put_array_separator(writer, first_ptr)
                or not deepseek_raw_put_generic_input_message(
                    writer, view, item
                )
            ):
                return False
        cursor = deepseek_json_skip_ws(view, item_end, items[1] - 1)
        if cursor < items[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, items[1] - 1)
            continue
        if cursor != items[1] - 1:
            return False
        break
    return True

def deepseek_raw_put_mcp_output_message_from_item(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    item: InlineArray[Int64, 2],
) -> Bool:
    var call_id = deepseek_raw_first_member3(
        view,
        item,
        StringSlice("call_id"),
        StringSlice("tool_call_id"),
        StringSlice("id"),
    )
    var output = deepseek_raw_member(view, item, StringSlice("output"))
    if not deepseek_raw_present(output):
        output = deepseek_raw_member(view, item, StringSlice("content"))
    if not deepseek_raw_present(output):
        output = deepseek_raw_member(view, item, StringSlice("result"))
    if not deepseek_raw_present(output):
        output = deepseek_raw_member(view, item, StringSlice("error"))
    if (
        not deepseek_put_literal(writer, StringSlice('{"role":"tool","tool_call_id":'))
        or not deepseek_raw_put_default_or_string(
            writer, view, call_id, StringSlice('"call_1"')
        )
        or not deepseek_put_literal(writer, StringSlice(',"content":'))
    ):
        return False
    if deepseek_raw_present(output):
        if not deepseek_put_view_range(writer, view, output[0], output[1]):
            return False
    elif not deepseek_put_literal(writer, StringSlice('""')):
        return False
    return deepseek_put_byte(writer, 125)

def deepseek_raw_object_has_fields(view: ProdexRichStringView) -> Bool:
    if not deepseek_json_fragment_valid(view):
        return False
    var root = deepseek_raw_root(view)
    return (
        root[0] >= 0
        and deepseek_json_skip_ws(view, root[0] + 1, root[1] - 1)
        < root[1] - 1
    )

def deepseek_put_object_interior(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if not deepseek_json_fragment_valid(view):
        return False
    var root = deepseek_raw_root(view)
    if root[0] < 0:
        return False
    var start = deepseek_json_skip_ws(view, root[0] + 1, root[1] - 1)
    if start >= root[1] - 1:
        return True
    return deepseek_put_view_range(writer, view, start, root[1] - 1)

def deepseek_metadata_separator(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    has_fields: Pointer[mut=True, Bool, _],
) -> Bool:
    if has_fields[]:
        return deepseek_put_byte(writer, 44)
    has_fields[] = True
    return True

def deepseek_put_request_metadata(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    # input.extra: base metadata object without provider subobject
    # input.metadata: existing provider-specific metadata object
    # input.item: client_metadata JSON object
    # input.content: normalized prompt_cache_key
    # input.reasoning_content: prompt_cache_retention
    # input.response: degraded response-format source label
    # input.error_message: degraded response-format reason
    # input.tool_choice: original tool_choice JSON
    # input.arguments: omitted tool-choice reason
    # input.name: provider metadata key
    if not deepseek_put_byte(writer, 123):
        return False
    var has_fields = False
    var has_fields_ptr = Pointer(to=has_fields)

    if input.extra_present == 1 and deepseek_raw_object_has_fields(input.extra):
        if not deepseek_put_object_interior(writer, input.extra):
            return False
        has_fields = True

    if input.item_present == 1:
        if (
            not deepseek_metadata_separator(writer, has_fields_ptr)
            or not deepseek_put_literal(writer, StringSlice('"client_metadata":'))
            or not deepseek_put_view(writer, input.item)
        ):
            return False
    if input.content_present == 1 and input.content.len > 0:
        if (
            not deepseek_metadata_separator(writer, has_fields_ptr)
            or not deepseek_put_literal(writer, StringSlice('"prompt_cache_key":'))
            or not deepseek_put_json_string(writer, input.content)
        ):
            return False
    if input.reasoning_content_present == 1:
        if (
            not deepseek_metadata_separator(writer, has_fields_ptr)
            or not deepseek_put_literal(
                writer, StringSlice('"prompt_cache_retention":')
            )
            or not deepseek_put_json_string(writer, input.reasoning_content)
        ):
            return False

    var provider_has_fields = (
        input.metadata_present == 1
        and deepseek_raw_object_has_fields(input.metadata)
    )
    var has_degraded = (
        input.response_present == 1 and input.error_message_present == 1
    )
    var has_omitted = (
        input.stream == 1
        and input.tool_choice_present == 1
        and input.arguments_present == 1
    )
    if provider_has_fields or has_degraded or has_omitted:
        if (
            input.name_present != 1
            or not deepseek_metadata_separator(writer, has_fields_ptr)
            or not deepseek_put_json_string(writer, input.name)
            or not deepseek_put_literal(writer, StringSlice(":{"))
        ):
            return False
        var provider_written = False
        if provider_has_fields:
            if not deepseek_put_object_interior(writer, input.metadata):
                return False
            provider_written = True
        if has_degraded:
            if provider_written and not deepseek_put_byte(writer, 44):
                return False
            if (
                not deepseek_put_literal(
                    writer,
                    StringSlice('"degraded_response_format":{"from":'),
                )
                or not deepseek_put_json_string(writer, input.response)
                or not deepseek_put_literal(
                    writer, StringSlice(',"to":"json_object","reason":')
                )
                or not deepseek_put_json_string(writer, input.error_message)
                or not deepseek_put_byte(writer, 125)
            ):
                return False
            provider_written = True
        if has_omitted:
            if provider_written and not deepseek_put_byte(writer, 44):
                return False
            if (
                not deepseek_put_literal(
                    writer, StringSlice('"omitted_tool_choice":{"from":')
                )
                or not deepseek_put_view(writer, input.tool_choice)
                or not deepseek_put_literal(writer, StringSlice(',"reason":'))
                or not deepseek_put_json_string(writer, input.arguments)
                or not deepseek_put_byte(writer, 125)
            ):
                return False
        if not deepseek_put_byte(writer, 125):
            return False
    return deepseek_put_byte(writer, 125)

def deepseek_raw_put_content_text_string(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    bounds: InlineArray[Int64, 2],
) -> Bool:
    if not deepseek_raw_present(bounds):
        return deepseek_put_literal(writer, StringSlice('""'))
    if deepseek_json_byte(view, bounds[0]) == 34:
        return deepseek_put_view_range(writer, view, bounds[0], bounds[1])
    if deepseek_json_byte(view, bounds[0]) != 91:
        return deepseek_put_json_string_range(writer, view, bounds[0], bounds[1])
    if not deepseek_put_byte(writer, 34):
        return False
    var first = True
    var cursor = deepseek_json_skip_ws(view, bounds[0] + 1, bounds[1] - 1)
    while cursor < bounds[1] - 1:
        var item_end = deepseek_json_value_end(view, cursor, bounds[1] - 1, 0)
        if item_end < 0:
            return False
        var text = InlineArray[Int64, 2](fill=-1)
        if deepseek_json_byte(view, cursor) == 34:
            text[0] = cursor
            text[1] = item_end
        elif deepseek_json_byte(view, cursor) == 123:
            text = deepseek_json_object_member(
                view, cursor, item_end, StringSlice("text")
            )
            if not deepseek_raw_present(text):
                text = deepseek_json_object_member(
                    view, cursor, item_end, StringSlice("input_text")
                )
            if not deepseek_raw_present(text):
                text = deepseek_json_object_member(
                    view, cursor, item_end, StringSlice("output_text")
                )
        if deepseek_raw_present(text) and deepseek_json_byte(view, text[0]) == 34:
            if not first and not deepseek_put_literal(writer, StringSlice("\n")):
                return False
            first = False
            if not deepseek_put_view_range(writer, view, text[0] + 1, text[1] - 1):
                return False
        cursor = deepseek_json_skip_ws(view, item_end, bounds[1] - 1)
        if cursor < bounds[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, bounds[1] - 1)
            continue
        if cursor != bounds[1] - 1:
            return False
        break
    return deepseek_put_byte(writer, 34)

def deepseek_raw_put_custom_tool_call_message_from_item(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    item: InlineArray[Int64, 2],
) -> Bool:
    var call_id = deepseek_raw_first_member3(
        view,
        item,
        StringSlice("call_id"),
        StringSlice("tool_call_id"),
        StringSlice("id"),
    )
    var name = deepseek_raw_member(view, item, StringSlice("name"))
    if not deepseek_raw_present(name):
        name = deepseek_raw_member(view, item, StringSlice("tool_name"))
    if not deepseek_raw_string_nonempty(view, name):
        return deepseek_put_literal(writer, StringSlice('{"role":"assistant","content":""}'))
    var input = deepseek_raw_member(view, item, StringSlice("input"))
    var signature = deepseek_raw_thought_signature(view, item)
    if (
        not deepseek_put_literal(writer, StringSlice('{"role":"assistant","content":"","tool_calls":[{"id":'))
        or not deepseek_raw_put_default_or_string(
            writer, view, call_id, StringSlice('"call_1"')
        )
        or not deepseek_put_literal(
            writer, StringSlice(',"type":"function","function":{"name":')
        )
        or not deepseek_put_view_range(writer, view, name[0], name[1])
        or not deepseek_put_literal(writer, StringSlice(',"arguments":"{\\"input\\":'))
    ):
        return False

    # Arguments must itself be a JSON string containing {"input": <string>}.
    var temp_start = writer[].written
    if deepseek_raw_present(input) and deepseek_json_byte(view, input[0]) == 34:
        if not deepseek_put_view_range(writer, view, input[0], input[1]):
            return False
    elif deepseek_raw_present(input) and deepseek_json_byte(view, input[0]) == 91:
        # Reuse Responses content flattening, then escape the resulting quoted string
        # into the arguments string.
        var temp = DeepSeekResponseWriter(
            writer[].output + writer[].written,
            writer[].capacity - writer[].written,
            0,
        )
        var temp_ptr = Pointer(to=temp)
        if not deepseek_raw_put_content_text_string(temp_ptr, view, input):
            return False
        var nested = ProdexRichStringView(
            UInt(writer[].output + writer[].written),
            UInt(temp.written),
        )
        writer[].written = temp_start
        if not deepseek_put_json_string(writer, nested):
            return False
    elif deepseek_raw_present(input):
        if (
            not deepseek_put_literal(writer, StringSlice('"'))
            or not deepseek_put_view_range(writer, view, input[0], input[1])
            or not deepseek_put_literal(writer, StringSlice('"'))
        ):
            return False
    elif not deepseek_put_literal(writer, StringSlice('""')):
        return False
    if not deepseek_put_literal(writer, StringSlice('}"')):
        return False
    if not deepseek_put_byte(writer, 125):
        return False
    if deepseek_raw_present(signature):
        if (
            not deepseek_put_literal(
                writer, StringSlice(',"gemini_thought_signature":')
            )
            or not deepseek_put_view_range(writer, view, signature[0], signature[1])
        ):
            return False
    return deepseek_put_literal(writer, StringSlice("}]}"))

def deepseek_raw_shell_command(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    item: InlineArray[Int64, 2],
) -> Bool:
    var action = deepseek_raw_member(view, item, StringSlice("action"))
    var command = InlineArray[Int64, 2](fill=-1)
    if deepseek_raw_present(action) and deepseek_json_byte(view, action[0]) == 123:
        command = deepseek_raw_member(view, action, StringSlice("command"))
    if deepseek_raw_present(command) and deepseek_json_byte(view, command[0]) == 91:
        if not deepseek_put_byte(writer, 34):
            return False
        var first = True
        var cursor = deepseek_json_skip_ws(view, command[0] + 1, command[1] - 1)
        while cursor < command[1] - 1:
            var part_end = deepseek_json_value_end(view, cursor, command[1] - 1, 0)
            if part_end < 0:
                return False
            if deepseek_json_byte(view, cursor) == 34:
                if not first and not deepseek_put_byte(writer, 32):
                    return False
                first = False
                if not deepseek_put_view_range(writer, view, cursor + 1, part_end - 1):
                    return False
            cursor = deepseek_json_skip_ws(view, part_end, command[1] - 1)
            if cursor < command[1] - 1 and deepseek_json_byte(view, cursor) == 44:
                cursor = deepseek_json_skip_ws(view, cursor + 1, command[1] - 1)
                continue
            break
        return deepseek_put_byte(writer, 34)
    command = deepseek_raw_member(view, item, StringSlice("command"))
    if deepseek_raw_string_nonempty(view, command):
        return deepseek_put_view_range(writer, view, command[0], command[1])
    return False

def deepseek_raw_shell_optional(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    item: InlineArray[Int64, 2],
    key: StringSlice,
) -> Bool:
    var value = deepseek_raw_member(view, item, key)
    if not deepseek_raw_present(value):
        var action = deepseek_raw_member(view, item, StringSlice("action"))
        if deepseek_raw_present(action) and deepseek_json_byte(view, action[0]) == 123:
            value = deepseek_raw_member(view, action, key)
    if not deepseek_raw_present(value):
        return True
    return (
        deepseek_put_literal(writer, StringSlice(","))
        and deepseek_put_json_string_range(writer, key, 0, Int64(key.byte_length()))
        and deepseek_put_byte(writer, 58)
        and deepseek_put_view_range(writer, view, value[0], value[1])
    )

def deepseek_raw_put_local_shell_call_message_from_item(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    item: InlineArray[Int64, 2],
) -> Bool:
    var call_id = deepseek_raw_first_member3(
        view,
        item,
        StringSlice("call_id"),
        StringSlice("tool_call_id"),
        StringSlice("id"),
    )
    if (
        not deepseek_put_literal(writer, StringSlice('{"role":"assistant","content":"","tool_calls":[{"id":'))
        or not deepseek_raw_put_default_or_string(
            writer, view, call_id, StringSlice('"call_1"')
        )
        or not deepseek_put_literal(
            writer, StringSlice(',"type":"function","function":{"name":"shell_command","arguments":"{\\"command\\":')
        )
    ):
        return False

    # Emit command as a JSON string embedded in the arguments string.
    var command_start = writer[].written
    var temp = DeepSeekResponseWriter(
        writer[].output + writer[].written,
        writer[].capacity - writer[].written,
        0,
    )
    var temp_ptr = Pointer(to=temp)
    if not deepseek_raw_shell_command(temp_ptr, view, item):
        return deepseek_put_literal(writer, StringSlice('"}"}]}'))
    var command_view = ProdexRichStringView(
        UInt(writer[].output + command_start), UInt(temp.written)
    )
    writer[].written = command_start
    if not deepseek_put_json_string(writer, command_view):
        return False
    if not deepseek_put_literal(writer, StringSlice('}"')):
        return False
    return deepseek_put_literal(writer, StringSlice("}]}"))

def deepseek_raw_put_generic_tool_calls(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
    calls: InlineArray[Int64, 2],
) -> Bool:
    if not deepseek_raw_present(calls) or deepseek_json_byte(view, calls[0]) != 91:
        return True
    if not deepseek_put_literal(writer, StringSlice(',"tool_calls":[')):
        return False
    var first = True
    var cursor = deepseek_json_skip_ws(view, calls[0] + 1, calls[1] - 1)
    while cursor < calls[1] - 1:
        var call_end = deepseek_json_value_end(view, cursor, calls[1] - 1, 0)
        if call_end < 0:
            return False
        if deepseek_json_byte(view, cursor) == 123:
            var function = deepseek_json_object_member(
                view, cursor, call_end, StringSlice("function")
            )
            if deepseek_raw_present(function) and deepseek_json_byte(view, function[0]) == 123:
                var name = deepseek_json_object_member(
                    view, function[0], function[1], StringSlice("name")
                )
                if deepseek_raw_string_nonempty(view, name):
                    if not first and not deepseek_put_byte(writer, 44):
                        return False
                    first = False
                    var call_id = deepseek_json_object_member(
                        view, cursor, call_end, StringSlice("id")
                    )
                    var arguments = deepseek_json_object_member(
                        view, function[0], function[1], StringSlice("arguments")
                    )
                    if not deepseek_put_byte(writer, 123):
                        return False
                    if deepseek_raw_present(call_id):
                        if (
                            not deepseek_put_literal(writer, StringSlice('"id":'))
                            or not deepseek_put_view_range(writer, view, call_id[0], call_id[1])
                            or not deepseek_put_byte(writer, 44)
                        ):
                            return False
                    if (
                        not deepseek_put_literal(writer, StringSlice('"type":"function","function":{"name":'))
                        or not deepseek_put_view_range(writer, view, name[0], name[1])
                        or not deepseek_put_literal(writer, StringSlice(',"arguments":'))
                        or not deepseek_raw_put_argument_string(writer, view, arguments)
                        or not deepseek_put_literal(writer, StringSlice("}}"))
                    ):
                        return False
        cursor = deepseek_json_skip_ws(view, call_end, calls[1] - 1)
        if cursor < calls[1] - 1 and deepseek_json_byte(view, cursor) == 44:
            cursor = deepseek_json_skip_ws(view, cursor + 1, calls[1] - 1)
            continue
        break
    return deepseek_put_byte(writer, 93)
