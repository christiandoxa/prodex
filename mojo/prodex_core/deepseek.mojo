from std.memory import Pointer

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

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


# These scanners keep JSON parsing at the Rust boundary while letting the
# DeepSeek kernel own the bounded rewrites that only need structural offsets.
def deepseek_json_byte(view: ProdexRichStringView, index: Int64) -> UInt8:
    return rich_view_ptr(view)[unsafe_offset=index]


def deepseek_json_skip_ws(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    var index = start
    while index < end:
        var value = deepseek_json_byte(view, index)
        if value != 9 and value != 10 and value != 13 and value != 32:
            break
        index += 1
    return index


def deepseek_json_string_end(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Int64:
    if start < 0 or start >= end or deepseek_json_byte(view, start) != 34:
        return -1
    var index = start + 1
    while index < end:
        var value = deepseek_json_byte(view, index)
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


def deepseek_json_value_end(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    depth: Int64,
) -> Int64:
    if depth > DEEPSEEK_JSON_MAX_DEPTH:
        return -1
    var index = deepseek_json_skip_ws(view, start, end)
    if index >= end:
        return -1
    var opening = deepseek_json_byte(view, index)
    if opening == 34:
        return deepseek_json_string_end(view, index, end)
    if opening == 91:
        index += 1
        index = deepseek_json_skip_ws(view, index, end)
        if index < end and deepseek_json_byte(view, index) == 93:
            return index + 1
        while index < end:
            var value_end = deepseek_json_value_end(view, index, end, depth + 1)
            if value_end < 0:
                return -1
            index = deepseek_json_skip_ws(view, value_end, end)
            if index < end and deepseek_json_byte(view, index) == 44:
                index = deepseek_json_skip_ws(view, index + 1, end)
                continue
            if index < end and deepseek_json_byte(view, index) == 93:
                return index + 1
            return -1
        return -1
    if opening == 123:
        index += 1
        index = deepseek_json_skip_ws(view, index, end)
        if index < end and deepseek_json_byte(view, index) == 125:
            return index + 1
        while index < end:
            var key_end = deepseek_json_string_end(view, index, end)
            if key_end < 0:
                return -1
            index = deepseek_json_skip_ws(view, key_end, end)
            if index >= end or deepseek_json_byte(view, index) != 58:
                return -1
            index = deepseek_json_skip_ws(view, index + 1, end)
            var value_end = deepseek_json_value_end(view, index, end, depth + 1)
            if value_end < 0:
                return -1
            index = deepseek_json_skip_ws(view, value_end, end)
            if index < end and deepseek_json_byte(view, index) == 44:
                index = deepseek_json_skip_ws(view, index + 1, end)
                continue
            if index < end and deepseek_json_byte(view, index) == 125:
                return index + 1
            return -1
        return -1
    var primitive_start = index
    while index < end:
        var value = deepseek_json_byte(view, index)
        if value == 9 or value == 10 or value == 13 or value == 32 or value == 44 or value == 93 or value == 125:
            break
        index += 1
    if index == primitive_start:
        return -1
    return index


def deepseek_json_object_member(
    view: ProdexRichStringView,
    object_start: Int64,
    object_end: Int64,
    key: StringSlice,
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    if object_start < 0 or object_end > Int64(view.len) or object_end <= object_start + 1:
        return result^
    if deepseek_json_byte(view, object_start) != 123 or deepseek_json_byte(view, object_end - 1) != 125:
        return result^
    var index = deepseek_json_skip_ws(view, object_start + 1, object_end - 1)
    while index < object_end - 1:
        var key_start = index
        var key_end = deepseek_json_string_end(view, key_start, object_end - 1)
        if key_end < 0:
            return InlineArray[Int64, 2](fill=-1)^
        index = deepseek_json_skip_ws(view, key_end, object_end - 1)
        if index >= object_end - 1 or deepseek_json_byte(view, index) != 58:
            return InlineArray[Int64, 2](fill=-1)^
        var value_start = deepseek_json_skip_ws(view, index + 1, object_end - 1)
        var value_end = deepseek_json_value_end(view, value_start, object_end - 1, 0)
        if value_end < 0:
            return InlineArray[Int64, 2](fill=-1)^
        if deepseek_json_raw_equals(view, key_start, key_end, key):
            result[0] = value_start
            result[1] = value_end
        index = deepseek_json_skip_ws(view, value_end, object_end - 1)
        if index < object_end - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, object_end - 1)
            continue
        if index == object_end - 1:
            break
        return InlineArray[Int64, 2](fill=-1)^
    return result^


def deepseek_json_raw_equals(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    if start < 0 or end < start + 2:
        return False
    if deepseek_json_byte(view, start) != 34 or deepseek_json_byte(view, end - 1) != 34:
        return False
    var expected_length = Int64(literal.byte_length())
    if end - start - 2 != expected_length:
        return False
    var expected = literal.unsafe_ptr()
    var actual = rich_view_ptr(view)
    for index in range(expected_length):
        if actual[unsafe_offset=start + 1 + index] != expected[unsafe_offset=index]:
            return False
    return True


def deepseek_json_fragment_valid(view: ProdexRichStringView) -> Bool:
    if view.len == 0:
        return False
    var start = deepseek_json_skip_ws(view, 0, Int64(view.len))
    var value_end = deepseek_json_value_end(view, start, Int64(view.len), 0)
    return value_end >= 0 and deepseek_json_skip_ws(view, value_end, Int64(view.len)) == Int64(view.len)


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


def deepseek_write_operation(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    input: ProdexDeepSeekKernelInput,
) -> Bool:
    var operation = input.operation
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
        and input.operation <= DEEPSEEK_USER_ID
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
