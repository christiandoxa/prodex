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


def deepseek_put_json_string(
    writer: Pointer[mut=True, DeepSeekResponseWriter, _],
    view: ProdexRichStringView,
) -> Bool:
    if not deepseek_put_byte(writer, 34):
        return False
    if view.len > 0:
        var ptr = rich_view_ptr(view)
        for index in range(Int64(view.len)):
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
