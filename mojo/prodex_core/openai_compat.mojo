from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_matches_literal, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime OPENAI_COMPAT_KERNEL_MAX_BYTES: Int64 = 4_194_304
comptime OPENAI_COMPAT_STATUS_OK: Int64 = 0
comptime OPENAI_COMPAT_STATUS_INVALID: Int64 = 1
comptime OPENAI_COMPAT_STATUS_UTF8: Int64 = 2
comptime OPENAI_COMPAT_STATUS_CAPACITY: Int64 = 3
comptime OPENAI_COMPAT_STATUS_ABI: Int64 = 4
comptime OPENAI_COMPAT_STATUS_REJECTED: Int64 = 5

comptime OPENAI_COMPAT_VALIDATE_REQUEST: Int64 = 1
comptime OPENAI_COMPAT_PARAMETER_SUPPORT: Int64 = 2
comptime OPENAI_COMPAT_REQUEST_MESSAGE: Int64 = 3
comptime OPENAI_COMPAT_OUTPUT_TEXT: Int64 = 4
comptime OPENAI_COMPAT_RESPONSE_USAGE: Int64 = 5
comptime OPENAI_COMPAT_SPLIT_TOOL_NAME: Int64 = 6
comptime OPENAI_COMPAT_RTK_ARGUMENTS: Int64 = 7
comptime OPENAI_COMPAT_STREAM_EVENT: Int64 = 8

comptime OPENAI_COMPAT_SYSTEM_MESSAGE: Int64 = 1
comptime OPENAI_COMPAT_USER_MESSAGE: Int64 = 2
comptime OPENAI_COMPAT_ROLE_MESSAGE: Int64 = 3
comptime OPENAI_COMPAT_FUNCTION_CALL_MESSAGE: Int64 = 4
comptime OPENAI_COMPAT_FUNCTION_CALL_OUTPUT_MESSAGE: Int64 = 5

comptime OPENAI_COMPAT_DONE_EVENT: Int64 = 1
comptime OPENAI_COMPAT_TEXT_DELTA_EVENT: Int64 = 2
comptime OPENAI_COMPAT_FUNCTION_CALL_ARGUMENTS_DELTA_EVENT: Int64 = 3


@fieldwise_init
struct ProdexOpenAiCompatKernelInput(Copyable):
    var operation: Int64
    var message_kind: Int64
    var stream_kind: Int64
    var has_messages: Int64
    var has_response_format: Int64
    var has_reasoning: Int64
    var has_previous_response_id: Int64
    var has_text_format: Int64
    var n_gt_one: Int64
    var has_metadata: Int64
    var has_safety_identifier: Int64
    var has_web_search_options: Int64
    var tools_non_function: Int64
    var tool_choice_invalid: Int64
    var parallel_tool_calls_false: Int64
    var has_logprobs: Int64
    var has_top_logprobs: Int64
    var has_stop_sequences: Int64
    var input_custom_tool: Int64
    var input_non_text: Int64
    var input_tokens: UInt64
    var output_tokens: UInt64
    var total_tokens: UInt64
    var total_tokens_present: Int64
    var provider_present: Int64
    var role_present: Int64
    var text_present: Int64
    var call_id_present: Int64
    var namespace_present: Int64
    var name_present: Int64
    var arguments_present: Int64
    var delta_present: Int64
    var provider: ProdexRichStringView
    var role: ProdexRichStringView
    var text: ProdexRichStringView
    var call_id: ProdexRichStringView
    var namespace: ProdexRichStringView
    var name: ProdexRichStringView
    var arguments: ProdexRichStringView
    var delta: ProdexRichStringView


@fieldwise_init
struct OpenAiCompatWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def openai_compat_put_byte(
    writer: Pointer[mut=True, OpenAiCompatWriter, _], value: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True


def openai_compat_put_literal(
    writer: Pointer[mut=True, OpenAiCompatWriter, _], value: StringSlice
) -> Bool:
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not openai_compat_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def openai_compat_put_view(
    writer: Pointer[mut=True, OpenAiCompatWriter, _], view: ProdexRichStringView
) -> Bool:
    if view.len == 0:
        return True
    var ptr = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        if not openai_compat_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def openai_compat_put_view_range(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    if start == end:
        return True
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        if not openai_compat_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def openai_compat_put_hex_byte(
    writer: Pointer[mut=True, OpenAiCompatWriter, _], value: UInt8
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
    return openai_compat_put_byte(writer, high) and openai_compat_put_byte(writer, low)


def openai_compat_put_json_escaped_range(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    if start == end:
        return True
    var ptr = rich_view_ptr(view)
    for index in range(start, end):
        var value = ptr[unsafe_offset=index]
        if value == 34 or value == 92:
            if not openai_compat_put_byte(writer, 92) or not openai_compat_put_byte(writer, value):
                return False
        elif value == 8:
            if not openai_compat_put_literal(writer, StringSlice('\\b')):
                return False
        elif value == 9:
            if not openai_compat_put_literal(writer, StringSlice('\\t')):
                return False
        elif value == 10:
            if not openai_compat_put_literal(writer, StringSlice('\\n')):
                return False
        elif value == 12:
            if not openai_compat_put_literal(writer, StringSlice('\\f')):
                return False
        elif value == 13:
            if not openai_compat_put_literal(writer, StringSlice('\\r')):
                return False
        elif value < 32:
            if not openai_compat_put_literal(writer, StringSlice('\\u00')):
                return False
            if not openai_compat_put_hex_byte(writer, value):
                return False
        elif not openai_compat_put_byte(writer, value):
            return False
    return True


def openai_compat_put_json_string(
    writer: Pointer[mut=True, OpenAiCompatWriter, _], view: ProdexRichStringView
) -> Bool:
    return (
        openai_compat_put_byte(writer, 34)
        and openai_compat_put_json_escaped_range(writer, view, 0, Int64(view.len))
        and openai_compat_put_byte(writer, 34)
    )


def openai_compat_put_u64(
    writer: Pointer[mut=True, OpenAiCompatWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return openai_compat_put_byte(writer, 48)
    var divisor: UInt64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        if not openai_compat_put_byte(writer, UInt8(remaining / divisor) + 48):
            return False
        remaining %= divisor
        divisor /= 10
    return True


def openai_compat_put_default_json_string(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    present: Int64,
    view: ProdexRichStringView,
    default_value: StringSlice,
) -> Bool:
    if present == 1:
        return openai_compat_put_json_string(writer, view)
    return (
        openai_compat_put_byte(writer, 34)
        and openai_compat_put_literal(writer, default_value)
        and openai_compat_put_byte(writer, 34)
    )


def openai_compat_put_parameter(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    provider: ProdexRichStringView,
    field: StringSlice,
    reason_suffix: StringSlice,
) -> Bool:
    return (
        openai_compat_put_literal(writer, field)
        and openai_compat_put_byte(writer, 0)
        and openai_compat_put_view(writer, provider)
        and openai_compat_put_literal(writer, reason_suffix)
        and openai_compat_put_byte(writer, 0x1E)
    )


def openai_compat_reject(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    input: ProdexOpenAiCompatKernelInput,
    reason_suffix: StringSlice,
) -> Int64:
    if not openai_compat_put_view(writer, input.provider) or not openai_compat_put_literal(
        writer, reason_suffix
    ):
        return OPENAI_COMPAT_STATUS_CAPACITY
    return OPENAI_COMPAT_STATUS_REJECTED


def openai_compat_write_parameters(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    provider: ProdexRichStringView,
) -> Bool:
    return (
        openai_compat_put_parameter(writer, provider, StringSlice('input[*].content[type!=text]'), StringSlice(' Responses chat-compat currently translates only text input content'))
        and openai_compat_put_parameter(writer, provider, StringSlice('response_format.type'), StringSlice(' Responses chat-compat does not translate response_format controls'))
        and openai_compat_put_parameter(writer, provider, StringSlice('reasoning'), StringSlice(' Responses chat-compat does not map Responses reasoning controls'))
        and openai_compat_put_parameter(writer, provider, StringSlice('text.format'), StringSlice(' Responses chat-compat does not translate text.format controls'))
        and openai_compat_put_parameter(writer, provider, StringSlice('n>1'), StringSlice(' Responses chat-compat returns only the first choice and does not support n>1'))
        and openai_compat_put_parameter(writer, provider, StringSlice('metadata'), StringSlice(' Responses chat-compat does not translate request metadata'))
        and openai_compat_put_parameter(writer, provider, StringSlice('safety_identifier'), StringSlice(' Responses chat-compat does not translate safety_identifier'))
        and openai_compat_put_parameter(writer, provider, StringSlice('web_search_options'), StringSlice(' Responses chat-compat does not translate web_search_options'))
        and openai_compat_put_parameter(writer, provider, StringSlice('input[type=custom_tool_call|tool_search_call]'), StringSlice(' Responses chat-compat only translates message/function-call history items'))
        and openai_compat_put_parameter(writer, provider, StringSlice('messages'), StringSlice(' Responses chat-compat expects Responses input, not raw chat-completions messages'))
        and openai_compat_put_parameter(writer, provider, StringSlice('tools[type!=function]'), StringSlice(' Responses chat-compat only forwards function tools'))
        and openai_compat_put_parameter(writer, provider, StringSlice('tool_choice[type!=function]'), StringSlice(' Responses chat-compat only forwards function tool_choice controls'))
        and openai_compat_put_parameter(writer, provider, StringSlice('parallel_tool_calls=false'), StringSlice(' Responses chat-compat does not prove a compatible parallel_tool_calls=false control'))
        and openai_compat_put_parameter(writer, provider, StringSlice('logprobs/top_logprobs'), StringSlice(' Responses chat-compat does not translate logprobs controls'))
        and openai_compat_put_parameter(writer, provider, StringSlice('stop_sequences'), StringSlice(' Responses chat-compat does not translate stop_sequences'))
        and openai_compat_put_parameter(writer, provider, StringSlice('previous_response_id'), StringSlice(' Responses chat-compat does not map previous_response_id continuation state'))
    )


def openai_compat_validate_request(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    input: ProdexOpenAiCompatKernelInput,
) -> Int64:
    if input.has_messages == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat expects Responses input, not raw chat-completions messages'))
    if input.has_response_format == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat does not translate response_format controls'))
    if input.has_reasoning == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat does not map Responses reasoning controls'))
    if input.has_previous_response_id == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat does not map previous_response_id continuation state'))
    if input.has_text_format == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat does not translate text.format controls'))
    if input.n_gt_one == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat returns only the first choice and does not support n>1'))
    if input.has_metadata == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat does not translate request metadata'))
    if input.has_safety_identifier == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat does not translate safety_identifier'))
    if input.has_web_search_options == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat does not translate web_search_options'))
    if input.tools_non_function == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat only forwards function tools'))
    if input.tool_choice_invalid == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat only forwards function tool_choice controls'))
    if input.parallel_tool_calls_false == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat does not prove a compatible parallel_tool_calls=false control'))
    if input.has_logprobs == 1 or input.has_top_logprobs == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat does not translate logprobs controls'))
    if input.has_stop_sequences == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat does not translate stop_sequences'))
    if input.input_custom_tool == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat only translates message/function-call history items'))
    if input.input_non_text == 1:
        return openai_compat_reject(writer, input, StringSlice(' Responses chat-compat currently translates only text input content'))
    return OPENAI_COMPAT_STATUS_OK


def openai_compat_write_request_message(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    input: ProdexOpenAiCompatKernelInput,
) -> Bool:
    if input.message_kind == OPENAI_COMPAT_SYSTEM_MESSAGE:
        return (
            openai_compat_put_literal(writer, StringSlice('{"content":'))
            and openai_compat_put_json_string(writer, input.text)
            and openai_compat_put_literal(writer, StringSlice(',"role":"system"}'))
        )
    if input.message_kind == OPENAI_COMPAT_USER_MESSAGE:
        return (
            openai_compat_put_literal(writer, StringSlice('{"content":'))
            and openai_compat_put_json_string(writer, input.text)
            and openai_compat_put_literal(writer, StringSlice(',"role":"user"}'))
        )
    if input.message_kind == OPENAI_COMPAT_ROLE_MESSAGE:
        if not openai_compat_put_literal(writer, StringSlice('{"content":')) or not openai_compat_put_json_string(writer, input.text):
            return False
        if not openai_compat_put_literal(writer, StringSlice(',"role":')):
            return False
        if not openai_compat_put_default_json_string(writer, input.role_present, input.role, StringSlice('user')):
            return False
        return openai_compat_put_byte(writer, 125)
    if input.message_kind == OPENAI_COMPAT_FUNCTION_CALL_MESSAGE:
        if not openai_compat_put_literal(writer, StringSlice('{"content":"","role":"assistant","tool_calls":[{"function":{"arguments":')):
            return False
        if not openai_compat_put_default_json_string(writer, input.arguments_present, input.arguments, StringSlice('{}')):
            return False
        if not openai_compat_put_literal(writer, StringSlice(',"name":')):
            return False
        if input.namespace_present == 1:
            if not openai_compat_put_byte(writer, 34):
                return False
            if not openai_compat_put_json_escaped_range(writer, input.namespace, 0, Int64(input.namespace.len)):
                return False
            if not openai_compat_put_byte(writer, 46):
                return False
            if not openai_compat_put_json_escaped_range(writer, input.name, 0, Int64(input.name.len)):
                return False
            if not openai_compat_put_byte(writer, 34):
                return False
        else:
            if not openai_compat_put_json_string(writer, input.name):
                return False
        if not openai_compat_put_literal(writer, StringSlice('},"id":')):
            return False
        if not openai_compat_put_default_json_string(writer, input.call_id_present, input.call_id, StringSlice('call_1')):
            return False
        return openai_compat_put_literal(writer, StringSlice(',"type":"function"}]}'))
    if input.message_kind == OPENAI_COMPAT_FUNCTION_CALL_OUTPUT_MESSAGE:
        return (
            openai_compat_put_literal(writer, StringSlice('{"content":'))
            and openai_compat_put_json_string(writer, input.text)
            and openai_compat_put_literal(writer, StringSlice(',"role":"tool","tool_call_id":'))
            and openai_compat_put_json_string(writer, input.call_id)
            and openai_compat_put_byte(writer, 125)
        )
    return False


def openai_compat_write_output_text(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    input: ProdexOpenAiCompatKernelInput,
) -> Bool:
    return (
        openai_compat_put_literal(writer, StringSlice('{"text":'))
        and openai_compat_put_json_string(writer, input.text)
        and openai_compat_put_literal(writer, StringSlice(',"type":"output_text"}'))
    )


def openai_compat_write_usage(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    input: ProdexOpenAiCompatKernelInput,
) -> Bool:
    var total = input.total_tokens
    if input.total_tokens_present == 0:
        total = input.input_tokens + input.output_tokens
    return (
        openai_compat_put_literal(writer, StringSlice('{"input_tokens":'))
        and openai_compat_put_u64(writer, input.input_tokens)
        and openai_compat_put_literal(writer, StringSlice(',"output_tokens":'))
        and openai_compat_put_u64(writer, input.output_tokens)
        and openai_compat_put_literal(writer, StringSlice(',"total_tokens":'))
        and openai_compat_put_u64(writer, total)
        and openai_compat_put_byte(writer, 125)
    )


def openai_compat_find_dot(view: ProdexRichStringView) -> Int64:
    if view.len == 0:
        return -1
    var ptr = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        if ptr[unsafe_offset=index] == 46:
            return index
    return -1


def openai_compat_write_split_tool_name(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    input: ProdexOpenAiCompatKernelInput,
) -> Bool:
    var dot = openai_compat_find_dot(input.name)
    if dot < 0:
        return (
            openai_compat_put_literal(writer, StringSlice('{"name":'))
            and openai_compat_put_json_string(writer, input.name)
            and openai_compat_put_byte(writer, 125)
        )
    var prefix = ProdexRichStringView(input.name.ptr, UInt(dot))
    var prefix_bounds = rich_trim_bounds(prefix)
    var rest_start = dot + 1
    if prefix_bounds[0] == prefix_bounds[1] or rest_start >= Int64(input.name.len):
        return (
            openai_compat_put_literal(writer, StringSlice('{"name":'))
            and openai_compat_put_json_string(writer, input.name)
            and openai_compat_put_byte(writer, 125)
        )
    return (
        openai_compat_put_literal(writer, StringSlice('{"name":'))
        and openai_compat_put_byte(writer, 34)
        and openai_compat_put_json_escaped_range(writer, input.name, rest_start, Int64(input.name.len))
        and openai_compat_put_byte(writer, 34)
        and openai_compat_put_literal(writer, StringSlice(',"namespace":'))
        and openai_compat_put_byte(writer, 34)
        and openai_compat_put_json_escaped_range(writer, prefix, prefix_bounds[0], prefix_bounds[1])
        and openai_compat_put_literal(writer, StringSlice('"}'))
    )


def openai_compat_hex_value(value: UInt8) -> Int64:
    if value >= 48 and value <= 57:
        return Int64(value - 48)
    if value >= 65 and value <= 70:
        return Int64(value - 55)
    if value >= 97 and value <= 102:
        return Int64(value - 87)
    return -1


def openai_compat_json_string_end(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if start < 0 or start >= end or ptr[unsafe_offset=start] != 34:
        return -1
    var index = start + 1
    while index < end:
        var value = ptr[unsafe_offset=index]
        if value == 34:
            return index + 1
        if value < 32:
            return -1
        if value == 92:
            if index + 1 >= end:
                return -1
            var escaped = ptr[unsafe_offset=index + 1]
            if escaped == 117:
                if index + 5 >= end:
                    return -1
                for offset in range(2, 6):
                    if openai_compat_hex_value(ptr[unsafe_offset=index + Int64(offset)]) < 0:
                        return -1
                index += 6
            elif escaped == 34 or escaped == 92 or escaped == 47 or escaped == 98 or escaped == 102 or escaped == 110 or escaped == 114 or escaped == 116:
                index += 2
            else:
                return -1
        else:
            index += 1
    return -1


def openai_compat_json_skip_ws(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var index = start
    while index < end:
        var value = ptr[unsafe_offset=index]
        if value != 32 and value != 9 and value != 10 and value != 13:
            break
        index += 1
    return index


def openai_compat_json_number_end(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var index = start
    if index < end and ptr[unsafe_offset=index] == 45:
        index += 1
    if index >= end:
        return -1
    if ptr[unsafe_offset=index] == 48:
        index += 1
        if index < end and ptr[unsafe_offset=index] >= 48 and ptr[unsafe_offset=index] <= 57:
            return -1
    elif ptr[unsafe_offset=index] >= 49 and ptr[unsafe_offset=index] <= 57:
        index += 1
        while index < end and ptr[unsafe_offset=index] >= 48 and ptr[unsafe_offset=index] <= 57:
            index += 1
    else:
        return -1
    if index < end and ptr[unsafe_offset=index] == 46:
        index += 1
        if index >= end or ptr[unsafe_offset=index] < 48 or ptr[unsafe_offset=index] > 57:
            return -1
        while index < end and ptr[unsafe_offset=index] >= 48 and ptr[unsafe_offset=index] <= 57:
            index += 1
    if index < end and (ptr[unsafe_offset=index] == 69 or ptr[unsafe_offset=index] == 101):
        index += 1
        if index < end and (ptr[unsafe_offset=index] == 43 or ptr[unsafe_offset=index] == 45):
            index += 1
        if index >= end or ptr[unsafe_offset=index] < 48 or ptr[unsafe_offset=index] > 57:
            return -1
        while index < end and ptr[unsafe_offset=index] >= 48 and ptr[unsafe_offset=index] <= 57:
            index += 1
    return index


def openai_compat_json_literal_end(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if start + 4 <= end and ptr[unsafe_offset=start] == 116 and ptr[unsafe_offset=start + 1] == 114 and ptr[unsafe_offset=start + 2] == 117 and ptr[unsafe_offset=start + 3] == 101:
        return start + 4
    if start + 5 <= end and ptr[unsafe_offset=start] == 102 and ptr[unsafe_offset=start + 1] == 97 and ptr[unsafe_offset=start + 2] == 108 and ptr[unsafe_offset=start + 3] == 115 and ptr[unsafe_offset=start + 4] == 101:
        return start + 5
    if start + 4 <= end and ptr[unsafe_offset=start] == 110 and ptr[unsafe_offset=start + 1] == 117 and ptr[unsafe_offset=start + 2] == 108 and ptr[unsafe_offset=start + 3] == 108:
        return start + 4
    return -1


def openai_compat_json_value_end(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64, depth: Int64
) -> Int64:
    if depth > 128:
        return -1
    var index = openai_compat_json_skip_ws(ptr, start, end)
    if index >= end:
        return -1
    var value = ptr[unsafe_offset=index]
    if value == 34:
        return openai_compat_json_string_end(ptr, index, end)
    if value == 123:
        return openai_compat_json_object_end(ptr, index, end, depth + 1)
    if value == 91:
        return openai_compat_json_array_end(ptr, index, end, depth + 1)
    if value == 116 or value == 102 or value == 110:
        return openai_compat_json_literal_end(ptr, index, end)
    return openai_compat_json_number_end(ptr, index, end)


def openai_compat_json_object_end(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64, depth: Int64
) -> Int64:
    var index = openai_compat_json_skip_ws(ptr, start + 1, end)
    if index < end and ptr[unsafe_offset=index] == 125:
        return index + 1
    while index < end:
        if ptr[unsafe_offset=index] != 34:
            return -1
        var key_end = openai_compat_json_string_end(ptr, index, end)
        if key_end < 0:
            return -1
        index = openai_compat_json_skip_ws(ptr, key_end, end)
        if index >= end or ptr[unsafe_offset=index] != 58:
            return -1
        var value_end = openai_compat_json_value_end(ptr, index + 1, end, depth)
        if value_end < 0:
            return -1
        index = openai_compat_json_skip_ws(ptr, value_end, end)
        if index < end and ptr[unsafe_offset=index] == 125:
            return index + 1
        if index >= end or ptr[unsafe_offset=index] != 44:
            return -1
        index = openai_compat_json_skip_ws(ptr, index + 1, end)
    return -1


def openai_compat_json_array_end(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64, depth: Int64
) -> Int64:
    var index = openai_compat_json_skip_ws(ptr, start + 1, end)
    if index < end and ptr[unsafe_offset=index] == 93:
        return index + 1
    while index < end:
        var value_end = openai_compat_json_value_end(ptr, index, end, depth)
        if value_end < 0:
            return -1
        index = openai_compat_json_skip_ws(ptr, value_end, end)
        if index < end and ptr[unsafe_offset=index] == 93:
            return index + 1
        if index >= end or ptr[unsafe_offset=index] != 44:
            return -1
        index = openai_compat_json_skip_ws(ptr, index + 1, end)
    return -1


def openai_compat_json_key_is_cmd(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    if start >= end or ptr[unsafe_offset=start] != 34:
        return False
    var expected = InlineArray[UInt8, 3](fill=0)
    expected[0] = 99
    expected[1] = 109
    expected[2] = 100
    var index = start + 1
    var count: Int64 = 0
    while index < end - 1:
        var value = ptr[unsafe_offset=index]
        if value == 92:
            if index + 5 >= end or ptr[unsafe_offset=index + 1] != 117:
                return False
            var codepoint: Int64 = 0
            for offset in range(2, 6):
                codepoint = codepoint * 16 + openai_compat_hex_value(ptr[unsafe_offset=index + Int64(offset)])
            if codepoint < 0 or codepoint > 127:
                return False
            value = UInt8(codepoint)
            index += 6
        else:
            index += 1
        if count >= 3 or value != expected[count]:
            return False
        count += 1
    return count == 3 and ptr[unsafe_offset=end - 1] == 34


def openai_compat_rtk_bounds(
    view: ProdexRichStringView,
) -> InlineArray[Int64, 5]:
    var result = InlineArray[Int64, 5](fill=-1)
    if view.len == 0:
        return result^
    var ptr = rich_view_ptr(view)
    var end = Int64(view.len)
    if ptr[unsafe_offset=0] != 123:
        return result^
    var root_end = openai_compat_json_object_end(ptr, 0, end, 0)
    if root_end < 0 or openai_compat_json_skip_ws(ptr, root_end, end) != end:
        return result^
    var index = openai_compat_json_skip_ws(ptr, 1, end)
    if index < end and ptr[unsafe_offset=index] == 125:
        result[0] = 1
        return result^
    while index < end:
        var key_start = index
        var key_end = openai_compat_json_string_end(ptr, key_start, end)
        if key_end < 0:
            return InlineArray[Int64, 5](fill=-1)
        index = openai_compat_json_skip_ws(ptr, key_end, end)
        if index >= end or ptr[unsafe_offset=index] != 58:
            return InlineArray[Int64, 5](fill=-1)
        var value_start = openai_compat_json_skip_ws(ptr, index + 1, end)
        var value_end = openai_compat_json_value_end(ptr, value_start, end, 0)
        if value_end < 0:
            return InlineArray[Int64, 5](fill=-1)
        if openai_compat_json_key_is_cmd(ptr, key_start, key_end):
            result[1] = 1
            result[4] = 0
            if value_start < value_end and ptr[unsafe_offset=value_start] == 34:
                var escaped = False
                for offset in range(value_start + 1, value_end - 1):
                    if ptr[unsafe_offset=offset] == 92:
                        escaped = True
                        break
                if not escaped:
                    result[2] = value_start + 1
                    result[3] = value_end - 1
                    result[4] = 1
        index = openai_compat_json_skip_ws(ptr, value_end, end)
        if index < end and ptr[unsafe_offset=index] == 125:
            result[0] = 1
            return result^
        if index >= end or ptr[unsafe_offset=index] != 44:
            return InlineArray[Int64, 5](fill=-1)
        index = openai_compat_json_skip_ws(ptr, index + 1, end)
    return InlineArray[Int64, 5](fill=-1)


def openai_compat_command_needs_rtk(
    source: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    if end <= start:
        return False
    var command = ProdexRichStringView(
        source.ptr + UInt(start), UInt(end - start)
    )
    var bounds = rich_trim_bounds(command)
    if bounds[0] == bounds[1]:
        return False
    var ptr = rich_view_ptr(command)
    if bounds[1] - bounds[0] >= 4:
        var index = bounds[0]
        if ptr[unsafe_offset=index] == 114 and ptr[unsafe_offset=index + 1] == 116 and ptr[unsafe_offset=index + 2] == 107 and ptr[unsafe_offset=index + 3] == 32:
            return False
    return True


def openai_compat_name_is_exec_command(view: ProdexRichStringView) -> Bool:
    return rich_view_matches_literal['functions.exec_command'](view, False) or rich_view_matches_literal['exec_command'](view, False)


def openai_compat_put_rtk_raw(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    input: ProdexOpenAiCompatKernelInput,
) -> Bool:
    if not openai_compat_name_is_exec_command(input.name):
        return openai_compat_put_view(writer, input.arguments)
    var bounds = openai_compat_rtk_bounds(input.arguments)
    if bounds[0] != 1 or bounds[1] != 1 or bounds[4] != 1 or not openai_compat_command_needs_rtk(input.arguments, bounds[2], bounds[3]):
        return openai_compat_put_view(writer, input.arguments)
    return (
        openai_compat_put_view_range(writer, input.arguments, 0, bounds[2])
        and openai_compat_put_literal(writer, StringSlice('rtk '))
        and openai_compat_put_view_range(writer, input.arguments, bounds[2], bounds[3])
        and openai_compat_put_view_range(writer, input.arguments, bounds[3], Int64(input.arguments.len))
    )


def openai_compat_put_json_string_with_rtk(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    input: ProdexOpenAiCompatKernelInput,
) -> Bool:
    if not openai_compat_name_is_exec_command(input.name):
        return openai_compat_put_json_string(writer, input.delta)
    var bounds = openai_compat_rtk_bounds(input.delta)
    if bounds[0] != 1 or bounds[1] != 1 or bounds[4] != 1 or not openai_compat_command_needs_rtk(input.delta, bounds[2], bounds[3]):
        return openai_compat_put_json_string(writer, input.delta)
    return (
        openai_compat_put_byte(writer, 34)
        and openai_compat_put_json_escaped_range(writer, input.delta, 0, bounds[2])
        and openai_compat_put_literal(writer, StringSlice('rtk '))
        and openai_compat_put_json_escaped_range(writer, input.delta, bounds[2], bounds[3])
        and openai_compat_put_json_escaped_range(writer, input.delta, bounds[3], Int64(input.delta.len))
        and openai_compat_put_byte(writer, 34)
    )


def openai_compat_write_stream_event(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    input: ProdexOpenAiCompatKernelInput,
) -> Bool:
    if input.stream_kind == OPENAI_COMPAT_DONE_EVENT:
        return openai_compat_put_literal(writer, StringSlice('event: response.completed\ndata: {}\n\n'))
    if input.stream_kind == OPENAI_COMPAT_TEXT_DELTA_EVENT:
        return (
            openai_compat_put_literal(writer, StringSlice('event: response.output_text.delta\ndata: {"delta":'))
            and openai_compat_put_json_string(writer, input.delta)
            and openai_compat_put_literal(writer, StringSlice(',"type":"response.output_text.delta"}\n\n'))
        )
    if input.stream_kind == OPENAI_COMPAT_FUNCTION_CALL_ARGUMENTS_DELTA_EVENT:
        if not openai_compat_put_literal(writer, StringSlice('event: response.function_call_arguments.delta\ndata: {')):
            return False
        if input.call_id_present == 1:
            if not openai_compat_put_literal(writer, StringSlice('"call_id":')) or not openai_compat_put_json_string(writer, input.call_id):
                return False
            if not openai_compat_put_literal(writer, StringSlice(',"delta":')):
                return False
        else:
            if not openai_compat_put_literal(writer, StringSlice('"delta":')):
                return False
        if not openai_compat_put_json_string_with_rtk(writer, input):
            return False
        return openai_compat_put_literal(writer, StringSlice(',"type":"response.function_call_arguments.delta"}\n\n'))
    return False


def openai_compat_flag_valid(value: Int64) -> Bool:
    return value == 0 or value == 1


def openai_compat_input_valid(input: ProdexOpenAiCompatKernelInput) -> Bool:
    return (
        input.operation >= OPENAI_COMPAT_VALIDATE_REQUEST
        and input.operation <= OPENAI_COMPAT_STREAM_EVENT
        and input.message_kind >= 0
        and input.message_kind <= OPENAI_COMPAT_FUNCTION_CALL_OUTPUT_MESSAGE
        and input.stream_kind >= 0
        and input.stream_kind <= OPENAI_COMPAT_FUNCTION_CALL_ARGUMENTS_DELTA_EVENT
        and openai_compat_flag_valid(input.has_messages)
        and openai_compat_flag_valid(input.has_response_format)
        and openai_compat_flag_valid(input.has_reasoning)
        and openai_compat_flag_valid(input.has_previous_response_id)
        and openai_compat_flag_valid(input.has_text_format)
        and openai_compat_flag_valid(input.n_gt_one)
        and openai_compat_flag_valid(input.has_metadata)
        and openai_compat_flag_valid(input.has_safety_identifier)
        and openai_compat_flag_valid(input.has_web_search_options)
        and openai_compat_flag_valid(input.tools_non_function)
        and openai_compat_flag_valid(input.tool_choice_invalid)
        and openai_compat_flag_valid(input.parallel_tool_calls_false)
        and openai_compat_flag_valid(input.has_logprobs)
        and openai_compat_flag_valid(input.has_top_logprobs)
        and openai_compat_flag_valid(input.has_stop_sequences)
        and openai_compat_flag_valid(input.input_custom_tool)
        and openai_compat_flag_valid(input.input_non_text)
        and openai_compat_flag_valid(input.total_tokens_present)
        and openai_compat_flag_valid(input.provider_present)
        and openai_compat_flag_valid(input.role_present)
        and openai_compat_flag_valid(input.text_present)
        and openai_compat_flag_valid(input.call_id_present)
        and openai_compat_flag_valid(input.namespace_present)
        and openai_compat_flag_valid(input.name_present)
        and openai_compat_flag_valid(input.arguments_present)
        and openai_compat_flag_valid(input.delta_present)
        and rich_view_valid(input.provider, OPENAI_COMPAT_KERNEL_MAX_BYTES)
        and rich_view_valid(input.role, OPENAI_COMPAT_KERNEL_MAX_BYTES)
        and rich_view_valid(input.text, OPENAI_COMPAT_KERNEL_MAX_BYTES)
        and rich_view_valid(input.call_id, OPENAI_COMPAT_KERNEL_MAX_BYTES)
        and rich_view_valid(input.namespace, OPENAI_COMPAT_KERNEL_MAX_BYTES)
        and rich_view_valid(input.name, OPENAI_COMPAT_KERNEL_MAX_BYTES)
        and rich_view_valid(input.arguments, OPENAI_COMPAT_KERNEL_MAX_BYTES)
        and rich_view_valid(input.delta, OPENAI_COMPAT_KERNEL_MAX_BYTES)
    )


def openai_compat_writer_status(
    writer: Pointer[mut=True, OpenAiCompatWriter, _], success: Bool
) -> Int64:
    if success:
        return OPENAI_COMPAT_STATUS_OK
    if writer[].written >= writer[].capacity:
        return OPENAI_COMPAT_STATUS_CAPACITY
    return OPENAI_COMPAT_STATUS_INVALID


def openai_compat_write_operation(
    writer: Pointer[mut=True, OpenAiCompatWriter, _],
    input: ProdexOpenAiCompatKernelInput,
) -> Int64:
    if input.operation == OPENAI_COMPAT_VALIDATE_REQUEST:
        if input.provider_present != 1:
            return OPENAI_COMPAT_STATUS_INVALID
        return openai_compat_validate_request(writer, input)
    if input.operation == OPENAI_COMPAT_PARAMETER_SUPPORT:
        if input.provider_present != 1:
            return OPENAI_COMPAT_STATUS_INVALID
        return openai_compat_writer_status(writer, openai_compat_write_parameters(writer, input.provider))
    if input.operation == OPENAI_COMPAT_REQUEST_MESSAGE:
        if input.message_kind == 0:
            return OPENAI_COMPAT_STATUS_INVALID
        return openai_compat_writer_status(writer, openai_compat_write_request_message(writer, input))
    if input.operation == OPENAI_COMPAT_OUTPUT_TEXT:
        return openai_compat_writer_status(writer, openai_compat_write_output_text(writer, input))
    if input.operation == OPENAI_COMPAT_RESPONSE_USAGE:
        return openai_compat_writer_status(writer, openai_compat_write_usage(writer, input))
    if input.operation == OPENAI_COMPAT_SPLIT_TOOL_NAME:
        return openai_compat_writer_status(writer, openai_compat_write_split_tool_name(writer, input))
    if input.operation == OPENAI_COMPAT_RTK_ARGUMENTS:
        return openai_compat_writer_status(writer, openai_compat_put_rtk_raw(writer, input))
    if input.operation == OPENAI_COMPAT_STREAM_EVENT:
        if input.stream_kind == 0:
            return OPENAI_COMPAT_STATUS_INVALID
        return openai_compat_writer_status(writer, openai_compat_write_stream_event(writer, input))
    return OPENAI_COMPAT_STATUS_INVALID


def openai_compat_kernel_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return OPENAI_COMPAT_STATUS_ABI
    if input_address == 0 or output_address == 0 or written_address == 0 or output_capacity <= 0:
        return OPENAI_COMPAT_STATUS_INVALID
    var input = Pointer[
        mut=False, ProdexOpenAiCompatKernelInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not openai_compat_input_valid(input[].copy()):
        return OPENAI_COMPAT_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = OpenAiCompatWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    var status = openai_compat_write_operation(writer_ptr, input[].copy())
    written[] = writer.written
    return status
